// Naygo — copia de archivos con CopyFile2: metadatos y ADS preservados, publicación atómica.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Implementación Windows de `NoReplaceRename::copy_and_publish`.
//!
//! `CopyFile2` copia `from` a un hermano temporal `.naygo-copy-<id>` del destino, preservando
//! fechas, atributos (oculto, sistema, solo lectura) y flujos alternativos de datos (ADS, incluida
//! la marca de la web `Zone.Identifier`); en recursos SMB aprovecha el offload del servidor. Su
//! callback de progreso respeta la pausa y la cancelación del token. Luego, si `durable`, el
//! temporal se fuerza a disco, y se publica con `MoveFileExW` (con `MOVEFILE_REPLACE_EXISTING`
//! solo si se autorizó sobrescribir: si no, un destino que apareció durante la copia hace fallar
//! la publicación sin pisarlo). Ante error o cancelación el temporal se borra.
//!
//! Todas las rutas pasan a forma verbatim (`\\?\`) para no depender del límite de MAX_PATH.

use crate::long_path::to_verbatim;
use crate::move_rename::windows_error_to_io;
use naygo_core::cancel::CancellationToken;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_FILE_EXISTS, ERROR_REQUEST_ABORTED};
use windows::Win32::Storage::FileSystem::{
    CopyFile2, MoveFileExW, COPYFILE2_CALLBACK_CHUNK_FINISHED, COPYFILE2_EXTENDED_PARAMETERS,
    COPYFILE2_MESSAGE, COPYFILE2_MESSAGE_ACTION, COPYFILE2_PROGRESS_CANCEL,
    COPYFILE2_PROGRESS_CONTINUE, COPY_FILE_ALLOW_DECRYPTED_DESTINATION, COPY_FILE_FAIL_IF_EXISTS,
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MOVE_FILE_FLAGS,
};

/// Prefijo compartido con la copia por defecto de `core`: identifica temporales de Naygo.
const TEMP_PREFIX: &str = ".naygo-copy-";
/// Intentos de nombre temporal ante una colisión (prácticamente imposible: pid + reloj + contador).
const TEMP_NAME_ATTEMPTS: usize = 8;
/// Cadencia del progreso por bytes, igual que la copia por buffers de `core`.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Implementación Windows de `NoReplaceRename::copy_and_publish`.
pub(crate) fn copy_and_publish(
    from: &Path,
    to: &Path,
    overwrite: bool,
    durable: bool,
    token: &CancellationToken,
    on_bytes: &mut dyn FnMut(u64),
) -> std::io::Result<bool> {
    copy_and_publish_with_hook(from, to, overwrite, durable, token, on_bytes, &mut || {})
}

/// Igual que [`copy_and_publish`] con un gancho tras cada bloque copiado (para los tests:
/// cancelar o crear el destino a media copia de forma determinista).
fn copy_and_publish_with_hook(
    from: &Path,
    to: &Path,
    overwrite: bool,
    durable: bool,
    token: &CancellationToken,
    on_bytes: &mut dyn FnMut(u64),
    after_chunk: &mut dyn FnMut(),
) -> std::io::Result<bool> {
    let source_len = std::fs::metadata(from)?.len();
    let from = to_verbatim(from);
    let to = to_verbatim(to);
    let parent = to
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let mut progress = ProgressContext {
        token,
        on_bytes,
        after_chunk,
        source_len,
        last_emit: Instant::now(),
        panicked: false,
    };

    // 1) Copiar al temporal. Un nombre ocupado (ERROR_FILE_EXISTS por FAIL_IF_EXISTS) no es
    //    nuestro: no se borra y se prueba otro nombre.
    let mut temp = None;
    for _ in 0..TEMP_NAME_ATTEMPTS {
        let candidate = parent.join(unique_temp_name());
        match copy_file2(&from, &candidate, &mut progress) {
            Ok(()) => {
                temp = Some(TempGuard::new(candidate));
                break;
            }
            Err(error) if error.raw_os_error() == Some(ERROR_FILE_EXISTS.0 as i32) => continue,
            Err(error) => {
                // CopyFile2 borra el parcial al cancelar o fallar; el guard cubre el resto.
                drop(TempGuard::new(candidate));
                if progress.panicked {
                    return Err(std::io::Error::other(
                        "falla interna en el progreso de la copia",
                    ));
                }
                if error.raw_os_error() == Some(ERROR_REQUEST_ABORTED.0 as i32) {
                    return Ok(false);
                }
                return Err(std::io::Error::new(
                    error.kind(),
                    format!("no se pudo copiar: {error}"),
                ));
            }
        }
    }
    let Some(temp) = temp else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "no se encontró un nombre temporal libre junto al destino",
        ));
    };
    // Una cancelación que llegó después del último callback (o un archivo vacío, que quizá no
    // genere callbacks de bloque) igual se respeta antes de publicar.
    if token.is_cancelled() {
        return Ok(false);
    }

    // 2) Durable: el contenido llega a disco antes de publicar (un Move borra el origen después).
    if durable {
        sync_file(temp.path()).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("no se pudo asegurar la copia en disco: {error}"),
            )
        })?;
    }

    // 3) Publicar. Sin REPLACE_EXISTING, MoveFileExW falla con ERROR_ALREADY_EXISTS si el destino
    //    apareció durante la copia, sin pisarlo. WRITE_THROUGH: el rename también es durable.
    let mut flags = MOVE_FILE_FLAGS(0);
    if overwrite {
        flags |= MOVEFILE_REPLACE_EXISTING;
    }
    if durable {
        flags |= MOVEFILE_WRITE_THROUGH;
    }
    let publish = || move_file(temp.path(), &to, flags);
    let published = match publish() {
        Err(error) if overwrite && error.kind() == std::io::ErrorKind::PermissionDenied => {
            // El usuario autorizó sobrescribir y `MoveFileExW(REPLACE_EXISTING)` no reemplaza un
            // destino de solo lectura (ERROR_ACCESS_DENIED). Si ese es el motivo, se le quita el
            // atributo y se reintenta UNA vez; si el reintento falla, se restaura.
            naygo_core::ops::retry_over_readonly_destination(&to, error, publish)
        }
        other => other,
    };
    published.map_err(|error| {
        std::io::Error::new(
            error.kind(),
            naygo_core::ops::publish_failed_failure(&error.to_string()),
        )
    })?;
    temp.disarm();

    // Emisión final sólo después de que la publicación tuvo éxito.
    (progress.on_bytes)(source_len);
    Ok(true)
}

/// Estado que el callback de `CopyFile2` recibe por `pvCallbackContext`.
struct ProgressContext<'a> {
    token: &'a CancellationToken,
    on_bytes: &'a mut dyn FnMut(u64),
    after_chunk: &'a mut dyn FnMut(),
    /// Tamaño del flujo principal: el progreso de CopyFile2 suma también los ADS y se acota a
    /// este valor para no pasarse del total que el motor calculó.
    source_len: u64,
    last_emit: Instant,
    /// Un panic dentro del callback se convierte en cancelación + error (nunca cruza el FFI).
    panicked: bool,
}

impl ProgressContext<'_> {
    fn on_message(&mut self, message: &COPYFILE2_MESSAGE) -> COPYFILE2_MESSAGE_ACTION {
        // Pausa: bloquea la copia dentro del callback hasta reanudar o cancelar.
        self.token.wait_if_paused();
        if self.token.is_cancelled() {
            return COPYFILE2_PROGRESS_CANCEL;
        }
        if message.Type == COPYFILE2_CALLBACK_CHUNK_FINISHED {
            // SAFETY: `Type == CHUNK_FINISHED` indica que la variante activa de la unión es
            // `ChunkFinished` (contrato de COPYFILE2_MESSAGE).
            let transferred = unsafe { message.Info.ChunkFinished.uliTotalBytesTransferred };
            (self.after_chunk)();
            if self.last_emit.elapsed() >= PROGRESS_INTERVAL {
                (self.on_bytes)(transferred.min(self.source_len));
                self.last_emit = Instant::now();
            }
            if self.token.is_cancelled() {
                return COPYFILE2_PROGRESS_CANCEL;
            }
        }
        COPYFILE2_PROGRESS_CONTINUE
    }
}

/// Callback de progreso de `CopyFile2`. Nunca deja que un panic cruce la frontera FFI.
unsafe extern "system" fn progress_routine(
    message: *const COPYFILE2_MESSAGE,
    context: *const c_void,
) -> COPYFILE2_MESSAGE_ACTION {
    let context = context as *mut ProgressContext<'_>;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `context` es el `&mut ProgressContext` que `copy_file2` pasó en
        // `pvCallbackContext`; CopyFile2 es síncrona y llama al callback durante la llamada, en
        // la que nadie más accede al contexto. `message` es válido mientras dura el callback.
        let (context, message) = unsafe { (&mut *context, &*message) };
        context.on_message(message)
    }));
    match result {
        Ok(action) => action,
        Err(_) => {
            // SAFETY: mismo puntero válido y exclusivo que arriba; el borrow del closure terminó.
            unsafe { (*context).panicked = true };
            COPYFILE2_PROGRESS_CANCEL
        }
    }
}

/// Ejecuta `CopyFile2(from → to)` con FAIL_IF_EXISTS y el callback de progreso.
fn copy_file2(from: &Path, to: &Path, progress: &mut ProgressContext<'_>) -> std::io::Result<()> {
    let from_wide = wide(from);
    let to_wide = wide(to);
    let params = COPYFILE2_EXTENDED_PARAMETERS {
        dwSize: std::mem::size_of::<COPYFILE2_EXTENDED_PARAMETERS>() as u32,
        // ALLOW_DECRYPTED_DESTINATION: un origen EFS hacia un volumen sin EFS (USB FAT/exFAT) se
        // copia descifrado en vez de fallar, como hacía la copia por buffers.
        dwCopyFlags: COPY_FILE_FAIL_IF_EXISTS | COPY_FILE_ALLOW_DECRYPTED_DESTINATION,
        pfCancel: std::ptr::null_mut(),
        pProgressRoutine: Some(progress_routine),
        pvCallbackContext: progress as *mut ProgressContext<'_> as *mut c_void,
    };
    // SAFETY: rutas UTF-16 terminadas en NUL y `params` viven durante toda la llamada síncrona;
    // `pvCallbackContext` apunta a `progress`, prestado en exclusiva hasta que CopyFile2 retorna.
    unsafe {
        CopyFile2(
            PCWSTR(from_wide.as_ptr()),
            PCWSTR(to_wide.as_ptr()),
            Some(&params),
        )
    }
    .map_err(windows_error_to_io)
}

/// `MoveFileExW(from → to, flags)` con rutas UTF-16 propias.
fn move_file(from: &Path, to: &Path, flags: MOVE_FILE_FLAGS) -> std::io::Result<()> {
    let from_wide = wide(from);
    let to_wide = wide(to);
    // SAFETY: ambas rutas son UTF-16 terminadas en NUL que viven durante la llamada.
    unsafe { MoveFileExW(PCWSTR(from_wide.as_ptr()), PCWSTR(to_wide.as_ptr()), flags) }
        .map_err(windows_error_to_io)
}

/// Fuerza el contenido del temporal a disco (FlushFileBuffers vía `sync_all`). CopyFile2 copia
/// también el solo lectura, que impide abrir para escritura: se quita, se sincroniza y se repone.
fn sync_file(path: &Path) -> std::io::Result<()> {
    let permissions = std::fs::metadata(path)?.permissions();
    if !permissions.readonly() {
        return std::fs::OpenOptions::new()
            .write(true)
            .open(path)?
            .sync_all();
    }
    let mut writable = permissions.clone();
    #[allow(clippy::permissions_set_readonly_false)]
    // Windows: solo quita FILE_ATTRIBUTE_READONLY.
    writable.set_readonly(false);
    std::fs::set_permissions(path, writable)?;
    let synced = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|file| file.sync_all());
    std::fs::set_permissions(path, permissions)?;
    synced
}

fn unique_temp_name() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    format!(
        "{TEMP_PREFIX}{:x}-{nanos:x}-{:x}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Borra el temporal al soltarse salvo que la publicación lo haya consumido (`disarm`).
struct TempGuard {
    path: Option<PathBuf>,
}

impl TempGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn path(&self) -> &Path {
        self.path.as_deref().expect("TempGuard sin ruta")
    }

    fn disarm(mut self) {
        self.path = None;
    }
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        let Some(path) = self.path.take() else {
            return;
        };
        // Un temporal de solo lectura (copiado del origen) no se deja borrar: quitarlo primero.
        if let Ok(metadata) = std::fs::symlink_metadata(&path) {
            let mut permissions = metadata.permissions();
            if permissions.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                let _ = std::fs::set_permissions(&path, permissions);
            }
            if let Err(error) = std::fs::remove_file(&path) {
                tracing::warn!(
                    "no se pudo borrar el temporal de copia {}: {error}",
                    path.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::copy_and_publish_with_hook;
    use crate::move_rename::PlatformNoReplaceRename;
    use naygo_core::cancel::CancellationToken;
    use naygo_core::ops::NoReplaceRename;
    use std::fs;
    use std::path::Path;

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;

    fn sin_temporales(dir: &Path) -> bool {
        fs::read_dir(dir).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".naygo-copy-")
        })
    }

    fn marcar_oculto(path: &Path) {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            SetFileAttributesW, FILE_ATTRIBUTE_ARCHIVE, FILE_ATTRIBUTE_HIDDEN as HIDDEN,
        };
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `wide` es una ruta UTF-16 terminada en NUL que vive durante la llamada.
        unsafe { SetFileAttributesW(PCWSTR(wide.as_ptr()), HIDDEN | FILE_ATTRIBUTE_ARCHIVE) }
            .unwrap();
    }

    fn atributos(path: &Path) -> u32 {
        use std::os::windows::fs::MetadataExt;
        fs::metadata(path).unwrap().file_attributes()
    }

    #[test]
    fn copyfile2_preserva_oculto_y_ads() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("f.txt");
        let destination = dir.path().join("dst").join("f.txt");
        fs::create_dir(dir.path().join("dst")).unwrap();
        fs::write(&source, b"contenido").unwrap();
        let zone = "[ZoneTransfer]\r\nZoneId=3\r\nHostUrl=https://example.com/f.txt\r\n";
        fs::write(format!("{}:Zone.Identifier", source.display()), zone).unwrap();
        let fecha =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        fs::OpenOptions::new()
            .write(true)
            .open(&source)
            .unwrap()
            .set_modified(fecha)
            .unwrap();
        marcar_oculto(&source);

        let copied = PlatformNoReplaceRename
            .copy_and_publish(
                &source,
                &destination,
                false,
                false,
                &CancellationToken::new(),
                &mut |_| {},
            )
            .unwrap();

        assert!(copied);
        assert_eq!(fs::read(&destination).unwrap(), b"contenido");
        assert_ne!(
            atributos(&destination) & FILE_ATTRIBUTE_HIDDEN,
            0,
            "el atributo oculto se conserva"
        );
        assert_eq!(
            fs::read_to_string(format!("{}:Zone.Identifier", destination.display())).unwrap(),
            zone,
            "la marca de la web (Zone.Identifier) viaja con la copia"
        );
        assert_eq!(
            fs::metadata(&destination).unwrap().modified().unwrap(),
            fecha
        );
        assert!(sin_temporales(&dir.path().join("dst")));
    }

    #[test]
    fn copyfile2_cancelado_no_deja_temporal() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("grande.bin");
        let destination = dir.path().join("copia.bin");
        fs::write(&source, vec![0x3C; 16 * 1024 * 1024]).unwrap();
        let token = CancellationToken::new();
        let mut chunks = 0;

        let result = copy_and_publish_with_hook(
            &source,
            &destination,
            false,
            false,
            &token,
            &mut |_| {},
            &mut || {
                chunks += 1;
                token.cancel();
            },
        );

        assert!(
            chunks >= 1,
            "el callback de progreso de CopyFile2 se ejecutó"
        );
        assert!(
            matches!(result, Ok(false)),
            "cancelado → Ok(false): {result:?}"
        );
        assert!(!destination.exists(), "no se publica nada");
        assert!(sin_temporales(dir.path()), "el temporal se borra");
        assert!(source.exists());
    }

    fn set_readonly(path: &Path, readonly: bool) {
        let mut permissions = fs::metadata(path).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(readonly);
        fs::set_permissions(path, permissions).unwrap();
    }

    /// Con Sobrescribir autorizado, un destino de solo lectura (p. ej. una copia previa de un
    /// `.git`, que Naygo ahora crea con su atributo) se reemplaza en vez de fallar con acceso
    /// denegado.
    #[test]
    fn copyfile2_overwrite_reemplaza_destino_solo_lectura() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("nuevo.txt");
        let destination = dir.path().join("existente.txt");
        fs::write(&source, b"NUEVO").unwrap();
        fs::write(&destination, b"VIEJO").unwrap();
        set_readonly(&destination, true);

        let result = PlatformNoReplaceRename.copy_and_publish(
            &source,
            &destination,
            true,
            false,
            &CancellationToken::new(),
            &mut |_| {},
        );

        let content = fs::read(&destination).unwrap();
        let readonly = fs::metadata(&destination).unwrap().permissions().readonly();
        if readonly {
            set_readonly(&destination, false);
        }
        assert!(matches!(result, Ok(true)), "{result:?}");
        assert_eq!(content, b"NUEVO");
        assert!(!readonly, "la copia lleva los atributos del origen");
        assert!(sin_temporales(dir.path()));
    }

    /// Sin autorización de sobrescribir, un destino de solo lectura sigue sin tocarse.
    #[test]
    fn copyfile2_sin_overwrite_no_toca_destino_solo_lectura() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("nuevo.txt");
        let destination = dir.path().join("existente.txt");
        fs::write(&source, b"NUEVO").unwrap();
        fs::write(&destination, b"VIEJO").unwrap();
        set_readonly(&destination, true);

        let result = PlatformNoReplaceRename.copy_and_publish(
            &source,
            &destination,
            false,
            false,
            &CancellationToken::new(),
            &mut |_| {},
        );

        let content = fs::read(&destination).unwrap();
        let readonly = fs::metadata(&destination).unwrap().permissions().readonly();
        set_readonly(&destination, false);
        assert!(result.is_err(), "{result:?}");
        assert_eq!(content, b"VIEJO");
        assert!(readonly, "el atributo del destino no se modificó");
        assert!(sin_temporales(dir.path()));
    }

    /// Motor completo con el copiador de plataforma: copiar dos veces con Sobrescribir un origen
    /// de solo lectura (CopyFile2 conserva el atributo en la primera copia).
    #[test]
    fn motor_copiar_dos_veces_con_sobrescribir_origen_solo_lectura() {
        use naygo_core::ops::{ConflictPolicy, OpMsg};
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("objeto.pack");
        fs::write(&source, b"v1").unwrap();
        set_readonly(&source, true);
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let copied = dest.join("objeto.pack");
        let mut request = naygo_core::ops::transfer(false, vec![source.clone()], dest.clone());
        request.conflict = ConflictPolicy::Overwrite;
        let run = || {
            let plan = naygo_core::ops::plan(&request).unwrap();
            let (tx, _rx) = std::sync::mpsc::channel::<OpMsg>();
            let (_ctx, crx) = std::sync::mpsc::channel();
            naygo_core::ops::run_plan_with_no_replace_rename(
                &plan,
                &request.kind,
                request.conflict,
                &CancellationToken::new(),
                &tx,
                &crx,
                None,
                &PlatformNoReplaceRename,
            )
        };

        let first = run();
        set_readonly(&source, false);
        fs::write(&source, b"v2").unwrap();
        set_readonly(&source, true);
        let second = run();

        let content = fs::read(&copied).ok();
        let readonly = fs::metadata(&copied)
            .map(|m| m.permissions().readonly())
            .unwrap_or(false);
        for path in [&source, &copied] {
            if path.exists() {
                set_readonly(path, false);
            }
        }
        assert_eq!(first.count_done(), 1, "{first:?}");
        assert_eq!(second.count_failed(), 0, "{second:?}");
        assert_eq!(content.as_deref(), Some(&b"v2"[..]));
        assert!(
            readonly,
            "la copia nueva conserva el solo lectura del origen"
        );
    }

    #[test]
    fn copyfile2_overwrite_reemplaza() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("nuevo.txt");
        let destination = dir.path().join("existente.txt");
        fs::write(&source, b"NUEVO").unwrap();
        fs::write(&destination, b"VIEJO-MAS-LARGO").unwrap();
        let mut last = 0;

        let copied = PlatformNoReplaceRename
            .copy_and_publish(
                &source,
                &destination,
                true,
                true,
                &CancellationToken::new(),
                &mut |bytes| last = bytes,
            )
            .unwrap();

        assert!(copied);
        assert_eq!(fs::read(&destination).unwrap(), b"NUEVO");
        assert_eq!(last, 5, "la emisión final informa el tamaño del archivo");
        assert!(sin_temporales(dir.path()));
    }

    #[test]
    fn copyfile2_noclobber_falla_si_aparece_destino() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.bin");
        let destination = dir.path().join("destination.bin");
        fs::write(&source, vec![0x6B; 4 * 1024 * 1024]).unwrap();
        let mut created = false;

        let result = copy_and_publish_with_hook(
            &source,
            &destination,
            false,
            false,
            &CancellationToken::new(),
            &mut |_| {},
            &mut || {
                if !created {
                    fs::write(&destination, b"created by another process").unwrap();
                    created = true;
                }
            },
        );

        assert!(created, "el destino apareció durante la copia");
        assert!(
            matches!(&result, Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists),
            "la publicación falla sin reemplazar: {result:?}"
        );
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"created by another process"
        );
        assert!(sin_temporales(dir.path()), "el temporal se borra");
    }

    #[test]
    fn copyfile2_ruta_de_300_caracteres() {
        let dir = tempfile::tempdir().unwrap();
        let mut long = dir.path().to_path_buf();
        while long.as_os_str().len() < 300 {
            long.push("segmento_de_ruta_larga");
        }
        fs::create_dir_all(&long).unwrap();
        let source = dir.path().join("corto.txt");
        let destination = long.join("copia.txt");
        fs::write(&source, b"largo").unwrap();

        let copied = PlatformNoReplaceRename
            .copy_and_publish(
                &source,
                &destination,
                false,
                true,
                &CancellationToken::new(),
                &mut |_| {},
            )
            .unwrap();

        assert!(copied);
        assert_eq!(fs::read(&destination).unwrap(), b"largo");
        assert!(sin_temporales(&long));
    }
}
