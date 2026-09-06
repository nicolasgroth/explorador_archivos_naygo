// Naygo — publicación atómica de espacios de trabajo desde workers.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use naygo_core::task_space::{checkpoint, read, revision, SpaceError, TaskSpace};
use naygo_core::CancellationToken;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
// Serializa publicaciones dentro del proceso; la revisión detecta ediciones externas previas.
static WRITER: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
#[path = "task_space_store_tests.rs"]
mod tests;

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// `expected = None` crea sin reemplazar. Una actualización exige la revisión leída.
/// No inspecciona las rutas referenciadas y nunca borra archivos de usuario.
pub fn save(
    path: &Path,
    space: &TaskSpace,
    expected: Option<&str>,
    token: &CancellationToken,
) -> Result<String, SpaceError> {
    checkpoint(token)?;
    let bytes = space.to_bytes()?;
    if !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("naygospace"))
    {
        return Err(SpaceError::Invalid);
    }
    write_document(path, &bytes, expected, token, |path, token| {
        read(path, token).map(|(_, hash)| hash)
    })
}

/// Publicación compartida para documentos pequeños. La validación/revisión es del formato.
pub(crate) fn write_document(
    path: &Path,
    bytes: &[u8],
    expected: Option<&str>,
    token: &CancellationToken,
    current_revision: impl Fn(&Path, &CancellationToken) -> Result<String, SpaceError>,
) -> Result<String, SpaceError> {
    let _lock = WRITER.lock().map_err(|_| SpaceError::Invalid)?;
    checkpoint(token)?;
    let parent = path.parent().ok_or(SpaceError::Invalid)?;
    // La carpeta de destino la elige el usuario; no creamos jerarquías implícitas.
    let mut temporary = None;
    for _ in 0..16 {
        let candidate = parent.join(format!(
            ".naygo-space-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                let guard = Temporary(candidate);
                let result = file.write_all(bytes).and_then(|()| file.sync_all());
                drop(file);
                result?;
                temporary = Some(guard);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    let temporary = temporary.ok_or(SpaceError::Exists)?;
    checkpoint(token)?;
    if let Some(expected) = expected {
        // Un archivo corrupto o de otra versión tampoco se sobrescribe.
        let actual = current_revision(path, token)?;
        if actual != expected {
            return Err(SpaceError::Changed);
        }
    }
    checkpoint(token)?;
    publish(&temporary.0, path, expected.is_some())?;
    Ok(revision(bytes))
}

#[cfg(windows)]
fn publish(from: &Path, to: &Path, replace: bool) -> Result<(), SpaceError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MOVE_FILE_FLAGS,
    };
    let wide = |path: &Path| -> Result<Vec<u16>, SpaceError> {
        let mut s: Vec<_> = path.as_os_str().encode_wide().collect();
        if s.contains(&0) {
            return Err(SpaceError::Invalid);
        }
        s.push(0);
        Ok(s)
    };
    let from = wide(from)?;
    let to = wide(to)?;
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace {
            MOVEFILE_REPLACE_EXISTING
        } else {
            MOVE_FILE_FLAGS(0)
        };
    // SAFETY: buffers UTF-16 terminados en NUL, vivos hasta terminar la llamada.
    unsafe { MoveFileExW(PCWSTR(from.as_ptr()), PCWSTR(to.as_ptr()), flags) }.map_err(|_| {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            SpaceError::Exists
        } else {
            SpaceError::Io(error)
        }
    })
}

#[cfg(not(windows))]
fn publish(from: &Path, to: &Path, replace: bool) -> Result<(), SpaceError> {
    let result = if replace {
        std::fs::rename(from, to)
    } else {
        std::fs::hard_link(from, to)
    };
    result.map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            SpaceError::Exists
        } else {
            error.into()
        }
    })
}
