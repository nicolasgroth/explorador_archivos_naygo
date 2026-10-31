// Naygo — motor de listado por streaming incremental, cancelable.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Lee un directorio en un hilo worker y emite cada entrada por un canal a
//! medida que la descubre, sin acumular todo antes de responder. Chequea el
//! `CancellationToken` entre cada entrada: cancelar corta el listado al instante.
//! El hilo de UI nunca llama a estas funciones directamente sobre el disco; usa
//! `spawn_listing`, que devuelve el receptor del canal.

use crate::cancel::CancellationToken;
use crate::fs_model::{Entry, EntryKind};
use std::fs::Metadata;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

const LISTING_CHANNEL_CAPACITY: usize = 1_024;

/// Admisión compartida para limitar listados vivos. El permiso se mueve al hilo y solo se libera
/// cuando el worker termina de verdad; descartar el receptor no libera capacidad.
#[derive(Clone)]
pub struct ListingAdmission {
    active: Arc<AtomicUsize>,
    maximum: usize,
}

impl ListingAdmission {
    pub fn new(maximum: usize) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            maximum: maximum.max(1),
        }
    }

    pub fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }

    pub fn has_capacity(&self) -> bool {
        self.active() < self.maximum
    }

    pub fn try_acquire(&self) -> Option<ListingPermit> {
        let mut active = self.active.load(Ordering::Acquire);
        loop {
            if active >= self.maximum {
                return None;
            }
            match self.active.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(ListingPermit(Arc::clone(&self.active))),
                Err(actual) => active = actual,
            }
        }
    }
}

pub struct ListingPermit(Arc<AtomicUsize>);

impl Drop for ListingPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug)]
pub enum ListingStartError {
    Capacity,
    Spawn(io::Error),
}

/// Mensajes que el worker de listado emite hacia la UI.
#[derive(Debug)]
pub enum ListingMsg {
    /// Una entrada recién descubierta.
    Entry(Entry),
    /// Resultado terminal. Se emite exactamente una vez tras las entradas encontradas.
    Finished(ListingOutcome),
}

/// Canales separados para que el resultado terminal nunca compita con la cola acotada de filas.
/// El consumidor consulta el terminal solo cuando ya drenó las filas disponibles, preservando
/// el orden observable aunque el productor termine entre dos polls.
pub struct ListingReceiver {
    entries: Receiver<Entry>,
    terminal: Receiver<ListingOutcome>,
}

impl ListingReceiver {
    pub fn new(entries: Receiver<Entry>, terminal: Receiver<ListingOutcome>) -> Self {
        Self { entries, terminal }
    }

    pub fn try_recv_entry(&self) -> Result<Entry, TryRecvError> {
        self.entries.try_recv()
    }

    pub fn try_recv_terminal(&self) -> Result<ListingOutcome, TryRecvError> {
        self.terminal.try_recv()
    }
}

trait ListingSink {
    fn entry(&mut self, entry: Entry, token: &CancellationToken) -> bool;
    fn terminal(&mut self, outcome: ListingOutcome);
}

struct BoundedListingSink {
    entries: SyncSender<Entry>,
    terminal: mpsc::Sender<ListingOutcome>,
}

impl ListingSink for BoundedListingSink {
    fn entry(&mut self, mut entry: Entry, token: &CancellationToken) -> bool {
        loop {
            if token.is_cancelled() {
                return false;
            }
            match self.entries.try_send(entry) {
                Ok(()) => return true,
                Err(TrySendError::Full(returned)) => {
                    entry = returned;
                    // Revisa cancelación cada milisegundo; nunca retiene un send bloqueante.
                    thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(TrySendError::Disconnected(_)) => return false,
            }
        }
    }

    fn terminal(&mut self, outcome: ListingOutcome) {
        let _ = self.terminal.send(outcome);
    }
}

#[cfg(test)]
struct TestListingSink<'a> {
    tx: &'a mpsc::Sender<ListingMsg>,
}

#[cfg(test)]
impl ListingSink for TestListingSink<'_> {
    fn entry(&mut self, entry: Entry, _token: &CancellationToken) -> bool {
        self.tx.send(ListingMsg::Entry(entry)).is_ok()
    }

    fn terminal(&mut self, outcome: ListingOutcome) {
        let _ = self.tx.send(ListingMsg::Finished(outcome));
    }
}

/// Estado terminal del listado. `Partial` significa que faltan entradas y por ello no sirve
/// como snapshot completo para comparar eliminaciones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListingOutcome {
    Complete,
    Cancelled,
    Failed(ListingFailure),
    Partial {
        skipped: usize,
        first_error: ListingFailure,
    },
}

/// Error tipado de filesystem; conserva `ErrorKind` y un detalle legible para diagnóstico.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListingFailure {
    pub kind: io::ErrorKind,
    pub message: String,
}

impl From<io::Error> for ListingFailure {
    fn from(error: io::Error) -> Self {
        Self {
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

/// Qué entradas emite un listado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListingFilter {
    /// Todas las entradas (comportamiento del file panel).
    All,
    /// Solo directorios (para el árbol de carpetas).
    DirsOnly,
}

/// Lanza el listado de `dir` en un hilo worker. Devuelve el receptor del canal
/// (la UI lo drena frame a frame) y el `JoinHandle` (por si se quiere unir).
pub fn spawn_listing(dir: PathBuf, token: CancellationToken) -> (ListingReceiver, JoinHandle<()>) {
    spawn_listing_filtered(dir, token, ListingFilter::All)
}

/// Lanza el listado de `dir` con un filtro. Igual que `spawn_listing` pero pudiendo
/// emitir solo directorios (para el árbol).
pub fn spawn_listing_filtered(
    dir: PathBuf,
    token: CancellationToken,
    filter: ListingFilter,
) -> (ListingReceiver, JoinHandle<()>) {
    spawn_listing_filtered_with_guard(dir, token, filter, ()).expect("crear hilo worker de listado")
}

/// Igual que `spawn_listing_filtered`, pero el valor `guard` vive dentro del hilo hasta su salida.
/// La variante admite devolver el error de creación para poder liberar el permiso sin panic.
pub fn spawn_listing_filtered_with_guard<G: Send + 'static>(
    dir: PathBuf,
    token: CancellationToken,
    filter: ListingFilter,
    guard: G,
) -> io::Result<(ListingReceiver, JoinHandle<()>)> {
    spawn_listing_filtered_with_guard_and_preflight(dir, token, filter, guard, |_, _| Ok(()))
}

fn spawn_listing_filtered_with_guard_and_preflight<G, F>(
    dir: PathBuf,
    token: CancellationToken,
    filter: ListingFilter,
    guard: G,
    preflight: F,
) -> io::Result<(ListingReceiver, JoinHandle<()>)>
where
    G: Send + 'static,
    F: FnOnce(&Path, &CancellationToken) -> io::Result<()> + Send + 'static,
{
    let (entries_tx, entries_rx) = mpsc::sync_channel(LISTING_CHANNEL_CAPACITY);
    let (terminal_tx, terminal_rx) = mpsc::channel();
    let rx = ListingReceiver::new(entries_rx, terminal_rx);
    let handle = thread::Builder::new()
        .name("naygo-listing".into())
        .spawn(move || {
            let _guard = guard;
            let mut sink = BoundedListingSink {
                entries: entries_tx,
                terminal: terminal_tx,
            };
            if let Err(error) = preflight(&dir, &token) {
                sink.terminal(ListingOutcome::Failed(error.into()));
                return;
            }
            list_into_filtered(&dir, &token, &mut sink, filter);
        })?;
    Ok((rx, handle))
}

/// Inicia un listado sólo si hay una plaza. La plaza se conserva aunque se descarte el receptor
/// o se cancele un `read_dir` bloqueado, y se libera al salir el hilo (incluido unwind por panic).
pub fn try_spawn_listing_filtered(
    dir: PathBuf,
    token: CancellationToken,
    filter: ListingFilter,
    admission: &ListingAdmission,
) -> Result<(ListingReceiver, JoinHandle<()>), ListingStartError> {
    try_spawn_listing_filtered_with_preflight(dir, token, filter, admission, |_, _| Ok(()))
}

/// Variante que ejecuta una comprobación cancelable en el mismo worker antes de leer el directorio.
/// Se usa para recuperación local de transacciones sin añadir lecturas al hilo de UI.
pub fn try_spawn_listing_filtered_with_preflight<F>(
    dir: PathBuf,
    token: CancellationToken,
    filter: ListingFilter,
    admission: &ListingAdmission,
    preflight: F,
) -> Result<(ListingReceiver, JoinHandle<()>), ListingStartError>
where
    F: FnOnce(&Path, &CancellationToken) -> io::Result<()> + Send + 'static,
{
    let permit = admission.try_acquire().ok_or(ListingStartError::Capacity)?;
    spawn_listing_filtered_with_guard_and_preflight(dir, token, filter, permit, preflight)
        .map_err(ListingStartError::Spawn)
}

/// Cuerpo del worker: recorre el directorio emitiendo por `tx`, aplicando `filter`.
/// Extraído para testearlo síncrono sin spawnear un hilo.
fn list_into_filtered(
    dir: &Path,
    token: &CancellationToken,
    sink: &mut impl ListingSink,
    filter: ListingFilter,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            sink.terminal(ListingOutcome::Failed(e.into()));
            return;
        }
    };

    let mut skipped = 0usize;
    let mut first_error = None;

    for dirent in read_dir {
        if token.is_cancelled() {
            sink.terminal(ListingOutcome::Cancelled);
            return;
        }

        let dirent = match dirent {
            Ok(d) => d,
            // Una entrada ilegible no aborta todo el listado: se salta.
            Err(error) => {
                skipped += 1;
                first_error.get_or_insert_with(|| ListingFailure::from(error));
                continue;
            }
        };

        // Reutiliza la misma ruta para filtrar temporales y construir Entry; evita materializarla
        // dos veces por fila en el listado de carpetas grandes.
        let path = dirent.path();
        if is_internal_replacement_entry(&path) {
            continue;
        }

        let (entry, metadata_error) = entry_from_dirent(&dirent, path);
        if let Some(error) = metadata_error {
            skipped += 1;
            first_error.get_or_insert_with(|| ListingFailure::from(error));
        }
        // Con DirsOnly (árbol) se omiten archivos y otros tipos no-directorio.
        if filter == ListingFilter::DirsOnly && entry.kind != EntryKind::Directory {
            continue;
        }
        // Si el receptor se cayó (la UI cambió de carpeta), dejar de trabajar.
        if !sink.entry(entry, token) {
            if token.is_cancelled() {
                sink.terminal(ListingOutcome::Cancelled);
            }
            return;
        }
    }

    let outcome = if token.is_cancelled() {
        ListingOutcome::Cancelled
    } else if let Some(first_error) = first_error {
        ListingOutcome::Partial {
            skipped,
            first_error,
        }
    } else {
        ListingOutcome::Complete
    };
    sink.terminal(outcome);
}

/// Reconoce solo los temporales reservados de Replace y el directorio con sentinel de recuperación.
fn is_internal_replacement_entry(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
        return false;
    };
    name == ".naygo-recovery-naygo-v1" && crate::ops::engine::owns_tree_recovery_dir(path)
        || name.starts_with(".naygo-replace-")
}

/// Wrapper de compatibilidad para tests: lista todo.
#[cfg(test)]
fn list_into(dir: &Path, token: &CancellationToken, tx: &mpsc::Sender<ListingMsg>) {
    list_into_filtered(dir, token, &mut TestListingSink { tx }, ListingFilter::All);
}

/// Lee los atributos "oculto" y "de sistema" de la metadata de Windows.
/// Devuelve `(hidden, system)`. Si no hay metadata, ambos son `false`.
/// `file_attributes()` es de `std::os::windows::fs::MetadataExt` (std puro,
/// no requiere la crate `windows`).
#[cfg(windows)]
fn attrs_of(metadata: Option<&Metadata>) -> (bool, bool) {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    match metadata {
        Some(m) => {
            let a = m.file_attributes();
            (
                a & FILE_ATTRIBUTE_HIDDEN != 0,
                a & FILE_ATTRIBUTE_SYSTEM != 0,
            )
        }
        None => (false, false),
    }
}

/// Fuera de Windows no hay atributos hidden/system del FS → ambos `false`.
#[cfg(not(windows))]
fn attrs_of(_metadata: Option<&Metadata>) -> (bool, bool) {
    (false, false)
}

/// Construye un `Entry` desde una ruta + su metadata (ya leída). Tolerante a metadata
/// ausente. Compartido por el listado inicial y por el merge incremental del watcher.
pub fn entry_from_path(path: &Path, metadata: Option<&Metadata>) -> Entry {
    let name = path
        .file_name()
        .map(crate::path_display::display_os_str)
        .unwrap_or_default();

    let kind = match metadata {
        Some(m) if m.is_dir() => EntryKind::Directory,
        Some(m) if m.is_file() => EntryKind::File,
        Some(_) => EntryKind::Other,
        None => EntryKind::Other,
    };

    let size = match (metadata, kind) {
        (Some(m), EntryKind::File) => Some(m.len()),
        _ => None,
    };

    let modified = metadata.and_then(|m| m.modified().ok());
    // La fecha de creación puede no estar soportada por el FS → None. Tolerante.
    let created = metadata.and_then(|m| m.created().ok());

    // Atributos reales del FS (Windows). Fuera de Windows quedan en false.
    let (hidden, system) = attrs_of(metadata);

    Entry {
        name,
        path: path.to_path_buf(),
        kind,
        size,
        modified,
        created,
        hidden,
        system,
    }
}

/// Construye un `Entry` a partir de un `DirEntry`, tolerando metadata ausente.
fn entry_from_dirent(dirent: &std::fs::DirEntry, path: PathBuf) -> (Entry, Option<std::io::Error>) {
    match dirent.metadata() {
        Ok(metadata) => (entry_from_path(&path, Some(&metadata)), None),
        Err(error) => (entry_from_path(&path, None), Some(error)),
    }
}

/// Cambio normalizado en una carpeta vigilada (producido por `platform::dir_watch`,
/// consumido por `apply_dir_events`). Tipo puro: sin notify ni Windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirEvent {
    Created(PathBuf),
    Removed(PathBuf),
    Modified(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

/// Resultado de aplicar un lote del watcher. `row_only_modified` solo contiene rutas cuya
/// identidad/nombre/tipo no cambiaron, por lo que pueden refrescarse sin reordenar la lista.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirEventApply {
    pub new_paths: Vec<PathBuf>,
    pub row_only_modified: Vec<PathBuf>,
}

/// Aplica `events` a `entries` SIN re-listar la carpeta. `read_entry` produce el `Entry`
/// de una ruta (en producción usa el FS vía `entry_from_path`; en tests, un closure).
/// Devuelve las rutas NUEVAS (Created, o Renamed cuyo `from` no estaba) para resaltarlas.
/// El llamador re-ordena/re-filtra la vista después.
pub fn apply_dir_events(
    entries: &mut Vec<Entry>,
    events: &[DirEvent],
    read_entry: &dyn Fn(&std::path::Path) -> Option<Entry>,
) -> Vec<PathBuf> {
    apply_dir_events_with_row_deltas(entries, events, read_entry).new_paths
}

/// Variante que también conserva las rutas de modificaciones con identidad y nombre estables.
/// El llamador debe comprobar que el sort/filtros mantienen las posiciones antes de usar ese delta.
pub fn apply_dir_events_with_row_deltas(
    entries: &mut Vec<Entry>,
    events: &[DirEvent],
    read_entry: &dyn Fn(&std::path::Path) -> Option<Entry>,
) -> DirEventApply {
    let mut result = DirEventApply::default();
    for ev in events {
        match ev {
            DirEvent::Created(p) => {
                if entries.iter().any(|e| &e.path == p) {
                    continue;
                }
                if let Some(e) = read_entry(p) {
                    entries.push(e);
                    result.new_paths.push(p.clone());
                }
            }
            DirEvent::Removed(p) => {
                entries.retain(|e| &e.path != p);
            }
            DirEvent::Modified(p) => {
                if let Some(updated) = read_entry(p) {
                    if let Some(slot) = entries.iter_mut().find(|e| &e.path == p) {
                        if slot.path == updated.path
                            && slot.name == updated.name
                            && slot.kind == updated.kind
                            && slot.hidden == updated.hidden
                            && slot.system == updated.system
                        {
                            result.row_only_modified.push(p.clone());
                        }
                        *slot = updated;
                    }
                }
            }
            DirEvent::Renamed { from, to } => {
                if let Some(slot) = entries.iter_mut().find(|e| &e.path == from) {
                    slot.path = to.clone();
                    slot.name = to
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    result.new_paths.push(to.clone());
                } else if !entries.iter().any(|e| &e.path == to) {
                    if let Some(e) = read_entry(to) {
                        entries.push(e);
                        result.new_paths.push(to.clone());
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn preflight_del_worker_termina_antes_de_emitir_las_filas() {
        let dir = tempfile::tempdir().unwrap();
        let admission = ListingAdmission::new(1);
        let token = CancellationToken::new();
        let (rx, worker) = try_spawn_listing_filtered_with_preflight(
            dir.path().to_path_buf(),
            token,
            ListingFilter::All,
            &admission,
            |path, _| fs::write(path.join("ready.txt"), b"recovery completed"),
        )
        .unwrap();
        worker.join().unwrap();

        let mut names = Vec::new();
        while let Ok(entry) = rx.try_recv_entry() {
            names.push(entry.name);
        }
        assert_eq!(names, ["ready.txt"]);
        assert!(matches!(
            rx.try_recv_terminal(),
            Ok(ListingOutcome::Complete)
        ));
    }

    #[test]
    fn listado_oculta_solo_los_temporales_de_reemplazo_reservados() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".naygo-recovery-naygo-v1")).unwrap();
        let unowned_recovery = dir.path().join(".naygo-recovery-naygo-v1");
        fs::write(
            unowned_recovery.join(".owner"),
            b"Naygo tree replacement recovery directory v1",
        )
        .unwrap();
        fs::create_dir(dir.path().join(".naygo-replace-staging")).unwrap();
        fs::write(dir.path().join(".naygo-backup-real"), b"backup").unwrap();
        fs::write(dir.path().join(".naygo-replace-marker-old"), b"marker").unwrap();
        fs::write(dir.path().join(".naygo-recovery-notes"), b"user file").unwrap();
        let (tx, rx) = mpsc::channel();

        list_into(dir.path(), &CancellationToken::new(), &tx);
        drop(tx);
        let names: Vec<_> = rx
            .into_iter()
            .filter_map(|message| match message {
                ListingMsg::Entry(entry) => Some(entry.name),
                ListingMsg::Finished(_) => None,
            })
            .collect();
        assert!(names.contains(&".naygo-backup-real".to_owned()));
        assert!(names.contains(&".naygo-recovery-notes".to_owned()));
        assert!(!names.contains(&".naygo-recovery-naygo-v1".to_owned()));
        assert!(!names.contains(&".naygo-replace-staging".to_owned()));
        assert!(!names.contains(&".naygo-replace-marker-old".to_owned()));

        let user_dir = tempfile::tempdir().unwrap();
        let unowned_path = user_dir.path().join(".naygo-recovery-naygo-v1");
        fs::create_dir(&unowned_path).unwrap();
        assert!(
            !is_internal_replacement_entry(&unowned_path),
            "un directorio del usuario sin sentinel Naygo no se oculta"
        );
    }

    #[test]
    fn canal_lleno_revisa_cancelacion_y_terminal_usa_canal_separado() {
        let (entries, received) = mpsc::sync_channel(1);
        let (terminal_tx, terminal_rx) = mpsc::channel();
        let token = CancellationToken::new();
        let mut sink = BoundedListingSink {
            entries,
            terminal: terminal_tx,
        };
        let entry = |name: &str| Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind: EntryKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
            system: false,
        };

        assert!(sink.entry(entry("queued"), &token));
        let worker_token = token.clone();
        let worker = thread::spawn(move || {
            let sent = sink.entry(entry("waiting"), &worker_token);
            if !sent && worker_token.is_cancelled() {
                sink.terminal(ListingOutcome::Cancelled);
            }
            sent
        });
        thread::sleep(std::time::Duration::from_millis(5));
        token.cancel();
        assert!(!worker.join().unwrap());
        assert_eq!(received.try_recv().unwrap().name, "queued");
        assert_eq!(terminal_rx.try_recv(), Ok(ListingOutcome::Cancelled));
    }

    #[test]
    fn permiso_de_admision_vive_hasta_la_salida_real_del_worker() {
        use std::sync::mpsc;

        struct HeldPermit {
            _permit: ListingPermit,
            entered: mpsc::Sender<()>,
            release: mpsc::Receiver<()>,
        }
        impl Drop for HeldPermit {
            fn drop(&mut self) {
                let _ = self.entered.send(());
                let _ = self.release.recv();
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let admission = ListingAdmission::new(1);
        let permit = admission.try_acquire().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let guard = HeldPermit {
            _permit: permit,
            entered: entered_tx,
            release: release_rx,
        };
        let (_, worker) = spawn_listing_filtered_with_guard(
            dir.path().to_path_buf(),
            CancellationToken::new(),
            ListingFilter::All,
            guard,
        )
        .unwrap();

        entered_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert_eq!(admission.active(), 1);
        assert!(admission.try_acquire().is_none());
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        assert_eq!(admission.active(), 0);
    }

    #[test]
    fn lista_archivos_de_un_directorio() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"hola").unwrap();
        fs::create_dir(dir.path().join("subcarpeta")).unwrap();

        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        list_into(dir.path(), &token, &tx);
        drop(tx);

        let mut nombres = Vec::new();
        let mut done = false;
        for msg in rx {
            match msg {
                ListingMsg::Entry(e) => nombres.push(e.name),
                ListingMsg::Finished(ListingOutcome::Complete) => done = true,
                other => panic!("mensaje inesperado: {:?}", other),
            }
        }
        nombres.sort();
        assert_eq!(nombres, vec!["a.txt", "subcarpeta"]);
        assert!(done, "debe terminar con Done");
    }

    #[test]
    fn token_cancelado_antes_de_empezar_no_emite_entradas() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"x").unwrap();

        let token = CancellationToken::new();
        token.cancel();
        let (tx, rx) = mpsc::channel();
        list_into(dir.path(), &token, &tx);
        drop(tx);

        let msgs: Vec<_> = rx.into_iter().collect();
        // Con el token ya cancelado, el primer chequeo del loop aborta:
        // o no entró al loop (dir vacío del iterador) o cortó en la 1ª vuelta.
        assert!(
            msgs.iter()
                .any(|m| matches!(m, ListingMsg::Finished(ListingOutcome::Cancelled))),
            "debe emitir Cancelled, got {:?}",
            msgs
        );
        assert!(
            !msgs.iter().any(|m| matches!(m, ListingMsg::Entry(_))),
            "no debe emitir entradas tras cancelar"
        );
    }

    #[test]
    fn solo_directorios_filtra_los_archivos() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"x").unwrap();
        fs::write(dir.path().join("b.log"), b"y").unwrap();
        fs::create_dir(dir.path().join("sub1")).unwrap();
        fs::create_dir(dir.path().join("sub2")).unwrap();

        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        list_into_filtered(
            dir.path(),
            &token,
            &mut TestListingSink { tx: &tx },
            ListingFilter::DirsOnly,
        );
        drop(tx);

        let mut nombres = Vec::new();
        for msg in rx {
            if let ListingMsg::Entry(e) = msg {
                nombres.push(e.name);
            }
        }
        nombres.sort();
        assert_eq!(nombres, vec!["sub1", "sub2"]);
    }

    #[test]
    fn filtro_all_sigue_emitiendo_todo() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"x").unwrap();
        fs::create_dir(dir.path().join("sub1")).unwrap();

        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        list_into_filtered(
            dir.path(),
            &token,
            &mut TestListingSink { tx: &tx },
            ListingFilter::All,
        );
        drop(tx);

        let mut nombres = Vec::new();
        for msg in rx {
            if let ListingMsg::Entry(e) = msg {
                nombres.push(e.name);
            }
        }
        nombres.sort();
        assert_eq!(nombres, vec!["a.txt", "sub1"]);
    }

    #[test]
    fn entry_from_path_archivo() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"hola").unwrap(); // 4 bytes
        let meta = std::fs::metadata(&f).ok();
        let e = entry_from_path(&f, meta.as_ref());
        assert_eq!(e.name, "a.txt");
        assert_eq!(e.kind, EntryKind::File);
        assert_eq!(e.size, Some(4));
    }

    #[test]
    fn entry_from_path_carpeta() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let meta = std::fs::metadata(&sub).ok();
        let e = entry_from_path(&sub, meta.as_ref());
        assert_eq!(e.kind, EntryKind::Directory);
        assert_eq!(e.size, None);
    }

    #[test]
    fn directorio_inexistente_emite_error() {
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        list_into(Path::new("Z:/ruta/que/no/existe/naygo"), &token, &tx);
        drop(tx);

        let msgs: Vec<_> = rx.into_iter().collect();
        assert!(
            matches!(
                msgs.first(),
                Some(ListingMsg::Finished(ListingOutcome::Failed(_)))
            ),
            "debe emitir Error, got {:?}",
            msgs
        );
    }

    fn mk_entry(name: &str, dir: &std::path::Path) -> Entry {
        Entry {
            name: name.into(),
            path: dir.join(name),
            kind: EntryKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
            system: false,
        }
    }

    #[test]
    fn apply_created_inserta_y_reporta_nuevo() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("a.txt", dir)];
        let newp = dir.join("b.txt");
        let np = newp.clone();
        let read = move |p: &std::path::Path| {
            if p == np {
                Some(mk_entry("b.txt", dir))
            } else {
                None
            }
        };
        let nuevas = apply_dir_events(&mut entries, &[DirEvent::Created(newp.clone())], &read);
        assert_eq!(entries.len(), 2);
        assert_eq!(nuevas, vec![newp]);
    }

    #[test]
    fn apply_created_idempotente() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("a.txt", dir)];
        let p = dir.join("a.txt");
        let read = |_: &std::path::Path| Some(mk_entry("a.txt", dir));
        let nuevas = apply_dir_events(&mut entries, &[DirEvent::Created(p)], &read);
        assert_eq!(entries.len(), 1, "no duplica algo ya presente");
        assert!(nuevas.is_empty());
    }

    #[test]
    fn apply_removed_quita() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("a.txt", dir), mk_entry("b.txt", dir)];
        let read = |_: &std::path::Path| None;
        apply_dir_events(&mut entries, &[DirEvent::Removed(dir.join("a.txt"))], &read);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "b.txt");
    }

    #[test]
    fn apply_modified_actualiza_size() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("a.txt", dir)];
        let mut updated = mk_entry("a.txt", dir);
        updated.size = Some(999);
        let read = move |_: &std::path::Path| Some(updated.clone());
        apply_dir_events(
            &mut entries,
            &[DirEvent::Modified(dir.join("a.txt"))],
            &read,
        );
        assert_eq!(entries[0].size, Some(999));
    }

    #[test]
    fn modified_con_cambio_de_visibilidad_no_es_delta_de_fila() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("a.txt", dir)];
        let mut updated = mk_entry("a.txt", dir);
        updated.hidden = true;
        let read = move |_: &std::path::Path| Some(updated.clone());

        let result = apply_dir_events_with_row_deltas(
            &mut entries,
            &[DirEvent::Modified(dir.join("a.txt"))],
            &read,
        );

        assert!(result.row_only_modified.is_empty());
        assert!(entries[0].hidden);
    }

    #[test]
    fn apply_renamed_renombra() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("old.txt", dir)];
        let read = |_: &std::path::Path| None;
        let nuevas = apply_dir_events(
            &mut entries,
            &[DirEvent::Renamed {
                from: dir.join("old.txt"),
                to: dir.join("new.txt"),
            }],
            &read,
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "new.txt");
        assert_eq!(entries[0].path, dir.join("new.txt"));
        assert_eq!(nuevas, vec![dir.join("new.txt")]);
    }

    #[test]
    fn apply_renamed_from_ausente_es_created() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![];
        let read = |_: &std::path::Path| Some(mk_entry("new.txt", dir));
        let nuevas = apply_dir_events(
            &mut entries,
            &[DirEvent::Renamed {
                from: dir.join("ghost.txt"),
                to: dir.join("new.txt"),
            }],
            &read,
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(nuevas, vec![dir.join("new.txt")]);
    }
}
