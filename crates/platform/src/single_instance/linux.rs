// Naygo — instancia única Linux mediante Unix socket y lock por usuario.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! IPC local para reabrir/mostrar la instancia Linux sin bloquear la UI.
//!
//! El socket y el lock viven en el `XDG_RUNTIME_DIR` del usuario, que se valida como directorio
//! real, privado (0700) y propiedad del UID efectivo. `File::try_lock` es el árbitro frente a
//! carreras de inicio; después de adquirirlo, la primaria retira únicamente un socket obsoleto y
//! enlaza el listener. Los mensajes son binarios, acotados y conservan los bytes POSIX de la ruta.

use super::{Guard, Instance};
use crate::dir_watch::Waker;
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const LOCK_NAME: &str = "naygo-single-instance.lock";
const SOCKET_NAME: &str = "naygo-single-instance.sock";
const MAX_PATH_BYTES: usize = 4096;
const READ_TIMEOUT: Duration = Duration::from_millis(500);
const CONNECT_ATTEMPTS: usize = 12;
const CONNECT_RETRY: Duration = Duration::from_millis(25);
const COMMAND_SHOW: u8 = 1;
const COMMAND_SHUTDOWN: u8 = 2;
const COMMAND_STOP: u8 = 0;

static PENDING_OPEN_REQUEST: OnceLock<Mutex<VecDeque<PathBuf>>> = OnceLock::new();

/// Recursos de la instancia primaria. El archivo bloqueado vive hasta que termina el proceso.
pub(super) struct LinuxGuard {
    _lock: File,
    listener: UnixListener,
    socket: PathBuf,
    stopping: Arc<AtomicBool>,
}

impl LinuxGuard {
    /// Arranca el listener en un hilo dedicado. Los streams tienen timeout y payload limitado.
    pub(super) fn watch(
        &self,
        show_requested: Arc<AtomicBool>,
        shutdown_requested: Arc<AtomicBool>,
        waker: Waker,
    ) {
        if self.stopping.load(Ordering::SeqCst) {
            return;
        }
        let stopping = self.stopping.clone();
        let listener = match self.listener.try_clone() {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("naygo: no se pudo clonar el socket de instancia única: {error}");
                return;
            }
        };

        let spawn = std::thread::Builder::new()
            .name("naygo-single-instance".into())
            .spawn(move || loop {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => match receive_request(&mut stream) {
                        Ok(Request::Show(path)) => {
                            if let Some(path) = path {
                                push_open_request(path);
                            }
                            show_requested.store(true, Ordering::SeqCst);
                            waker();
                        }
                        Ok(Request::Shutdown) => {
                            shutdown_requested.store(true, Ordering::SeqCst);
                            waker();
                        }
                        Ok(Request::Stop) => break,
                        Err(error) => {
                            tracing::debug!("se descartó un mensaje IPC de instancia: {error}");
                        }
                    },
                    Err(error) => {
                        tracing::warn!("falló accept del socket de instancia única: {error}");
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            });

        if let Err(error) = spawn {
            eprintln!("naygo: no se pudo iniciar el worker IPC de instancia única: {error}");
        }
    }
}

impl Drop for LinuxGuard {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        // Despierta accept() para que el hilo detached pueda terminar antes de soltar el lock.
        if let Ok(mut stream) = UnixStream::connect(&self.socket) {
            let _ = stream.write_all(&[COMMAND_STOP, 0, 0, 0, 0]);
        }
    }
}

enum Request {
    Show(Option<PathBuf>),
    Shutdown,
    Stop,
}

#[derive(Debug)]
struct RuntimePaths {
    lock: PathBuf,
    socket: PathBuf,
}

/// Intenta crear la instancia primaria. Si el runtime dir no es seguro o el sistema no permite
/// IPC, Naygo sigue funcionando como instancia no coordinada en vez de abortar el arranque.
pub(super) fn acquire() -> Instance {
    let result = match runtime_paths() {
        Ok(paths) => acquire_at(&paths),
        Err(error) => Err(AcquireError::Unavailable(error)),
    };
    match result {
        Ok(guard) => Instance::Primary(Guard {
            _mutex: 0,
            event: 0,
            shutdown_event: 0,
            linux: Some(guard),
        }),
        Err(AcquireError::AlreadyRunning) => Instance::AlreadyRunning,
        Err(AcquireError::Unavailable(error)) => {
            eprintln!("naygo: instancia única Linux no disponible ({error}); se arranca sin IPC");
            Instance::Primary(Guard::inert())
        }
    }
}

#[derive(Debug)]
enum AcquireError {
    AlreadyRunning,
    Unavailable(io::Error),
}

impl From<io::Error> for AcquireError {
    fn from(error: io::Error) -> Self {
        Self::Unavailable(error)
    }
}

fn runtime_paths() -> Result<RuntimePaths, io::Error> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "XDG_RUNTIME_DIR no está definido")
    })?;
    paths_in(Path::new(&runtime))
}

fn paths_in(runtime_dir: &Path) -> Result<RuntimePaths, io::Error> {
    if !runtime_dir.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "XDG_RUNTIME_DIR debe ser absoluto",
        ));
    }
    let metadata = fs::symlink_metadata(runtime_dir)?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "XDG_RUNTIME_DIR debe ser un directorio real 0700 propiedad del usuario",
        ));
    }

    let socket = runtime_dir.join(SOCKET_NAME);
    // sockaddr_un.sun_path tiene 108 bytes en Linux, incluyendo el NUL final.
    if socket.as_os_str().as_bytes().len() >= 108 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "la ruta del socket excede el límite de Linux",
        ));
    }
    Ok(RuntimePaths {
        lock: runtime_dir.join(LOCK_NAME),
        socket,
    })
}

fn acquire_at(paths: &RuntimePaths) -> Result<LinuxGuard, AcquireError> {
    if let Ok(metadata) = fs::symlink_metadata(&paths.lock) {
        if !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "el lock de instancia no es un archivo regular",
            )
            .into());
        }
    }

    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&paths.lock)?;
    lock.set_permissions(fs::Permissions::from_mode(0o600))?;
    if lock.metadata()?.uid() != effective_uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "el lock de instancia pertenece a otro usuario",
        )
        .into());
    }
    match lock.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Err(AcquireError::AlreadyRunning),
        Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
    }

    remove_stale_socket(&paths.socket)?;
    let listener = UnixListener::bind(&paths.socket)?;
    if let Err(error) = fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600)) {
        let _ = fs::remove_file(&paths.socket);
        return Err(error.into());
    }
    if let Err(error) = listener.set_nonblocking(false) {
        let _ = fs::remove_file(&paths.socket);
        return Err(error.into());
    }

    Ok(LinuxGuard {
        _lock: lock,
        listener,
        socket: paths.socket.clone(),
        stopping: Arc::new(AtomicBool::new(false)),
    })
}

fn remove_stale_socket(path: &Path) -> Result<(), io::Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => fs::remove_file(path),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "la ruta del socket existe y no es un socket",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Envía la orden en startup de la secundaria; no se llama desde el hilo UI de la primaria.
pub(super) fn notify_running(folder: Option<&Path>) {
    let payload = match folder {
        Some(path) => match absolute_path(path) {
            Ok(path) => path.as_os_str().as_bytes().to_vec(),
            Err(error) => {
                eprintln!("naygo: no se pudo preparar la ruta para la instancia activa: {error}");
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    let _ = send_request(COMMAND_SHOW, &payload);
}

pub(super) fn notify_shutdown() {
    let _ = send_request(COMMAND_SHUTDOWN, &[]);
}

fn absolute_path(path: &Path) -> Result<PathBuf, io::Error> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn send_request(command: u8, payload: &[u8]) -> Result<(), io::Error> {
    if payload.len() > MAX_PATH_BYTES || payload.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mensaje IPC de instancia fuera de límites",
        ));
    }
    let paths = runtime_paths()?;
    let mut last_error = None;
    for attempt in 0..CONNECT_ATTEMPTS {
        match UnixStream::connect(&paths.socket) {
            Ok(mut stream) => {
                stream.set_write_timeout(Some(READ_TIMEOUT))?;
                stream.write_all(&[command])?;
                stream.write_all(&(payload.len() as u32).to_le_bytes())?;
                stream.write_all(payload)?;
                return Ok(());
            }
            Err(error)
                if attempt + 1 < CONNECT_ATTEMPTS
                    && matches!(
                        error.kind(),
                        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                    ) =>
            {
                last_error = Some(error);
                std::thread::sleep(CONNECT_RETRY);
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(io::ErrorKind::NotConnected, "no se pudo conectar a Naygo")
    }))
}

fn receive_request(stream: &mut UnixStream) -> Result<Request, io::Error> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    if !peer_is_current_user(stream) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "el peer IPC no pertenece al usuario actual",
        ));
    }

    let mut header = [0_u8; 5];
    stream.read_exact(&mut header)?;
    let command = header[0];
    let length =
        u32::from_le_bytes(header[1..5].try_into().expect("header length checked")) as usize;
    if length > MAX_PATH_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "payload IPC excede el límite de ruta",
        ));
    }
    match command {
        COMMAND_SHOW if length == 0 => Ok(Request::Show(None)),
        COMMAND_SHOW if length > 0 => {
            let mut bytes = vec![0; length];
            stream.read_exact(&mut bytes)?;
            if bytes.contains(&0) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "ruta IPC contiene NUL",
                ));
            }
            let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
            // No consultar `is_dir()` aquí: puede bloquear en un montaje de red caído. El panel
            // recibe la ruta y su listado asíncrono reporta si ya no es una carpeta accesible.
            if !path.is_absolute() {
                return Ok(Request::Show(None));
            }
            Ok(Request::Show(Some(path)))
        }
        COMMAND_SHUTDOWN if length == 0 => Ok(Request::Shutdown),
        COMMAND_STOP if length == 0 => Ok(Request::Stop),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "comando IPC desconocido o longitud inválida",
        )),
    }
}

fn peer_is_current_user(stream: &UnixStream) -> bool {
    // SAFETY: `credentials` y su longitud apuntan a memoria válida para getsockopt(SO_PEERCRED);
    // el descriptor pertenece al stream prestado y permanece abierto durante esta llamada.
    unsafe {
        let mut credentials: libc::ucred = std::mem::zeroed();
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        ) == 0
            && length as usize == std::mem::size_of::<libc::ucred>()
            && credentials.uid == effective_uid()
    }
}

fn effective_uid() -> u32 {
    // SAFETY: geteuid has no preconditions and returns the effective UID of this process.
    unsafe { libc::geteuid() }
}

fn push_open_request(path: PathBuf) {
    let queue = PENDING_OPEN_REQUEST.get_or_init(|| Mutex::new(VecDeque::new()));
    let mut queue = queue.lock().unwrap_or_else(|poison| poison.into_inner());
    // El evento UI actual consume una ruta por wake. Mantener una capacidad fija evita que una
    // ráfaga de lanzamientos locales crezca sin límite; cuando se llena, se conserva lo reciente.
    if queue.len() == 8 {
        queue.pop_front();
    }
    queue.push_back(path);
}

/// Extrae una ruta ya validada por el worker; esta función sólo toca memoria compartida.
pub(super) fn take_open_request() -> Option<PathBuf> {
    PENDING_OPEN_REQUEST
        .get()
        .and_then(|queue| queue.lock().ok()?.pop_front())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn test_runtime_paths() -> (tempfile::TempDir, RuntimePaths) {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let paths = paths_in(dir.path()).unwrap();
        (dir, paths)
    }

    #[test]
    fn runtime_dir_debe_ser_privado_y_propio() {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            paths_in(dir.path()).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );

        let symlink = dir.path().join("runtime-link");
        std::os::unix::fs::symlink(dir.path(), &symlink).unwrap();
        assert!(paths_in(&symlink).is_err());
    }

    #[test]
    fn solo_un_lock_permite_una_instancia_y_recupera_socket_obsoleto() {
        let (_dir, paths) = test_runtime_paths();
        let stale = UnixListener::bind(&paths.socket).unwrap();
        drop(stale);

        let primary = acquire_at(&paths).unwrap();
        assert!(paths.socket.exists());
        assert!(matches!(
            acquire_at(&paths),
            Err(AcquireError::AlreadyRunning)
        ));
        drop(primary);
    }

    #[test]
    fn secundaria_envia_shutdown_a_la_primaria() {
        let (_dir, paths) = test_runtime_paths();
        let primary = acquire_at(&paths).unwrap();
        let listener = primary.listener.try_clone().unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            tx.send(matches!(
                receive_request(&mut stream).unwrap(),
                Request::Shutdown
            ))
            .unwrap();
        });

        send_request_at(&paths, COMMAND_SHUTDOWN, &[]).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap());
        worker.join().unwrap();
    }

    #[test]
    fn ipc_despierta_primary_y_entrega_ruta_validada_desde_worker() {
        let (runtime, paths) = test_runtime_paths();
        let primary = acquire_at(&paths).unwrap();
        let show_requested = Arc::new(AtomicBool::new(false));
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let (wake_tx, wake_rx) = mpsc::channel();
        let waker: Waker = Arc::new(move || {
            let _ = wake_tx.send(());
        });
        primary.watch(show_requested.clone(), shutdown_requested.clone(), waker);

        send_request_at(&paths, COMMAND_SHOW, runtime.path().as_os_str().as_bytes()).unwrap();
        wake_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(show_requested.load(Ordering::SeqCst));
        assert!(!shutdown_requested.load(Ordering::SeqCst));
        assert_eq!(take_open_request(), Some(runtime.path().to_path_buf()));
    }

    #[test]
    fn ruta_no_utf8_se_transporta_y_se_valida_sin_perder_bytes() {
        let (runtime, paths) = test_runtime_paths();
        let non_utf8 = runtime.path().join(std::ffi::OsString::from_vec(
            [b"folder-".as_slice(), &[0xff]].concat(),
        ));
        fs::create_dir(&non_utf8).unwrap();

        let primary = acquire_at(&paths).unwrap();
        let listener = primary.listener.try_clone().unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            tx.send(receive_request(&mut stream).unwrap()).unwrap();
        });

        send_request_at(&paths, COMMAND_SHOW, non_utf8.as_os_str().as_bytes()).unwrap();
        match rx.recv_timeout(Duration::from_secs(1)).unwrap() {
            Request::Show(Some(received)) => {
                assert_eq!(
                    received.as_os_str().as_bytes(),
                    non_utf8.as_os_str().as_bytes()
                );
            }
            _ => panic!("se esperaba la ruta no UTF-8"),
        }
        worker.join().unwrap();
    }

    #[test]
    fn mensaje_malformado_no_se_acepta() {
        let (_dir, paths) = test_runtime_paths();
        let primary = acquire_at(&paths).unwrap();
        let listener = primary.listener.try_clone().unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            tx.send(receive_request(&mut stream).is_err()).unwrap();
        });
        let mut stream = UnixStream::connect(&paths.socket).unwrap();
        stream.write_all(&[COMMAND_SHOW, 1, 0, 0, 0]).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap());
        worker.join().unwrap();
    }

    fn send_request_at(paths: &RuntimePaths, command: u8, payload: &[u8]) -> Result<(), io::Error> {
        let mut stream = UnixStream::connect(&paths.socket)?;
        stream.write_all(&[command])?;
        stream.write_all(&(payload.len() as u32).to_le_bytes())?;
        stream.write_all(payload)
    }
}
