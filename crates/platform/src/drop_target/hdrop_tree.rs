// Naygo — captura acotada de árboles temporales de archivadores en Windows.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Primitiva de captura y materialización para el receptor OLE. No toca el hilo UI.
//! Los handles de archivo comparten lectura y borrado, pero no escritura; los de carpetas
//! impiden sustituir sus ancestros mientras se recorre el árbol. Rechaza reparse points.
//! El propietario del staging debe limpiarlo en todos los caminos de error y cancelación.

use naygo_core::cancel::CancellationToken;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use windows::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};

const REPARSE: u32 = 0x400;
const OPEN_REPARSE: u32 = 0x0020_0000;
const BACKUP: u32 = 0x0200_0000;
const READ_ATTRIBUTES: u32 = 0x80;
const LIST_DIRECTORY: u32 = 0x1;
const SHARE_READ_WRITE: u32 = 3;
const SHARE_READ_DELETE: u32 = 5;
const MAX_FILES: usize = 50_000;
const MAX_DIRECTORIES: usize = 50_000;
const MAX_DEPTH: usize = 128;
const COPY_BUFFER: usize = 128 * 1024;

struct RetainedFile {
    relative: PathBuf,
    file: File,
}

pub(super) struct CapturedDrop {
    roots: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    files: Vec<RetainedFile>,
    guards: Vec<File>,
}

fn cancelled(token: &CancellationToken) -> io::Result<()> {
    token.wait_if_paused();
    if token.is_cancelled() {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "captura cancelada",
        ))
    } else {
        Ok(())
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left: Vec<_> = left.as_os_str().encode_wide().collect();
    let right: Vec<_> = right.as_os_str().encode_wide().collect();
    // SAFETY: ambas secuencias UTF-16 viven durante la llamada y se pasan con longitud explícita.
    unsafe { CompareStringOrdinal(&left, &right, true) == CSTR_EQUAL }
}

fn open_checked(path: &Path, directory: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(OPEN_REPARSE);
    if directory {
        options
            .access_mode(READ_ATTRIBUTES | LIST_DIRECTORY)
            .share_mode(SHARE_READ_WRITE)
            .custom_flags(OPEN_REPARSE | BACKUP);
    } else {
        // Permite que 7-Zip elimine su nombre, pero impide escritores existentes o nuevos.
        options.share_mode(SHARE_READ_DELETE);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if metadata.file_attributes() & REPARSE != 0
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(io::Error::other("reparse point o tipo inesperado"));
    }
    Ok(file)
}

fn collect(
    absolute: &Path,
    relative: &Path,
    depth: usize,
    result: &mut CapturedDrop,
    token: &CancellationToken,
) -> io::Result<()> {
    cancelled(token)?;
    if depth > MAX_DEPTH {
        return Err(io::Error::other("profundidad máxima excedida"));
    }
    let metadata = fs::symlink_metadata(absolute)?;
    if metadata.file_attributes() & REPARSE != 0 {
        return Err(io::Error::other("reparse point rechazado"));
    }
    if metadata.is_dir() {
        if result.directories.len() >= MAX_DIRECTORIES {
            return Err(io::Error::other("límite de carpetas excedido"));
        }
        result.guards.push(open_checked(absolute, true)?);
        result.directories.push(relative.to_owned());
        for entry in fs::read_dir(absolute)? {
            let entry = entry?;
            collect(
                &entry.path(),
                &relative.join(entry.file_name()),
                depth + 1,
                result,
                token,
            )?;
        }
    } else if metadata.is_file() {
        if result.files.len() >= MAX_FILES {
            return Err(io::Error::other("límite de archivos excedido"));
        }
        result.files.push(RetainedFile {
            relative: relative.to_owned(),
            file: open_checked(absolute, false)?,
        });
    } else {
        return Err(io::Error::other("tipo de entrada no admitido"));
    }
    Ok(())
}

fn copy_retained(
    input: &mut impl Read,
    output: &mut impl Write,
    buffer: &mut [u8],
    token: &CancellationToken,
) -> io::Result<()> {
    loop {
        cancelled(token)?;
        let size = input.read(buffer)?;
        if size == 0 {
            break;
        }
        cancelled(token)?;
        output.write_all(&buffer[..size])?;
    }
    output.flush()
}

/// Captura selecciones cuyos elementos son hijos directos de un mismo `Temp/7zE…`.
/// La comparación de rutas se hace en UTF-16 sin pérdida. No mueve ni elimina el origen.
pub(super) fn capture(paths: &[PathBuf], token: &CancellationToken) -> io::Result<CapturedDrop> {
    cancelled(token)?;
    let first = paths
        .first()
        .ok_or_else(|| io::Error::other("selección vacía"))?;
    if !first.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "origen no absoluto",
        ));
    }
    let source_parent = first
        .parent()
        .ok_or_else(|| io::Error::other("origen sin padre"))?;
    let folder_name = source_parent
        .file_name()
        .ok_or_else(|| io::Error::other("temporal sin nombre"))?;
    if !folder_name
        .to_string_lossy()
        .to_ascii_lowercase()
        .starts_with("7ze")
    {
        return Err(io::Error::other("origen no es un temporal compatible"));
    }
    let temp = fs::canonicalize(std::env::temp_dir())?;
    let temp_guard = open_checked(&temp, true)?;
    let parent = temp.join(folder_name);
    let parent_guard = open_checked(&parent, true)?;
    let parent_canonical = fs::canonicalize(&parent)?;
    if !same_path(&fs::canonicalize(source_parent)?, &parent_canonical) {
        return Err(io::Error::other(
            "el origen no está en el temporal esperado",
        ));
    }
    let mut result = CapturedDrop {
        roots: Vec::new(),
        directories: Vec::new(),
        files: Vec::new(),
        guards: vec![temp_guard, parent_guard],
    };
    let mut seen = HashSet::new();
    for path in paths {
        cancelled(token)?;
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "origen no absoluto",
            ));
        }
        let supplied_parent = path
            .parent()
            .ok_or_else(|| io::Error::other("origen sin padre"))?;
        if !same_path(supplied_parent, source_parent)
            && !same_path(&fs::canonicalize(supplied_parent)?, &parent_canonical)
        {
            return Err(io::Error::other("selección con temporales distintos"));
        }
        let name = path
            .file_name()
            .ok_or_else(|| io::Error::other("origen sin nombre"))?;
        if !seen.insert(name.to_os_string()) {
            return Err(io::Error::other("origen duplicado"));
        }
        let relative = PathBuf::from(name);
        collect(&parent.join(name), &relative, 0, &mut result, token)?;
        result.roots.push(relative);
    }
    Ok(result)
}

impl CapturedDrop {
    /// Libera los bloqueos de carpetas antes de retornar de Drop, para que el proveedor limpie.
    /// Los archivos retenidos siguen legibles y no admiten escrituras hasta terminar la copia.
    pub(super) fn release_directories(&mut self) {
        self.guards.clear();
    }

    /// Copia desde handles a una carpeta privada recién creada. El caller limpia el staging ante
    /// errores o cancelación; no reemplaza archivos existentes ni sigue rutas ascendentes.
    pub(super) fn materialize(
        mut self,
        destination: &Path,
        token: &CancellationToken,
    ) -> io::Result<Vec<PathBuf>> {
        let safe = |path: &Path| {
            !path.as_os_str().is_empty()
                && path
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
        };
        for relative in &self.directories {
            cancelled(token)?;
            if !safe(relative) {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "ruta inválida"));
            }
            fs::create_dir(destination.join(relative))?;
        }
        let mut buffer = vec![0u8; COPY_BUFFER];
        for item in &mut self.files {
            cancelled(token)?;
            if !safe(&item.relative) {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "ruta inválida"));
            }
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination.join(&item.relative))?;
            copy_retained(&mut item.file, &mut output, &mut buffer, token)?;
        }
        cancelled(token)?;
        Ok(self
            .roots
            .iter()
            .map(|name| destination.join(name))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_fixture() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("7zE-naygo-test-")
            .tempdir_in(std::env::temp_dir())
            .unwrap()
    }

    #[test]
    fn captures_mixed_tree_and_materializes_after_source_cleanup() {
        let source = source_fixture();
        fs::create_dir_all(source.path().join("folder/nested/empty")).unwrap();
        fs::write(
            source.path().join("folder/nested/file.txt"),
            b"archivo anidado",
        )
        .unwrap();
        fs::write(source.path().join("loose.txt"), b"archivo suelto").unwrap();
        let token = CancellationToken::new();
        let mut captured = capture(
            &[
                source.path().join("folder"),
                source.path().join("loose.txt"),
            ],
            &token,
        )
        .unwrap();
        captured.release_directories();
        let resolved = fs::canonicalize(source.path()).unwrap();
        assert!(same_path(
            resolved.parent().unwrap(),
            &fs::canonicalize(std::env::temp_dir()).unwrap()
        ));
        assert!(resolved
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("7zE-naygo-test-"));
        fs::remove_dir_all(&resolved).unwrap();
        let stage = tempfile::tempdir().unwrap();
        let roots = captured.materialize(stage.path(), &token).unwrap();
        assert_eq!(
            roots,
            [stage.path().join("folder"), stage.path().join("loose.txt")]
        );
        assert_eq!(
            fs::read(stage.path().join("folder/nested/file.txt")).unwrap(),
            b"archivo anidado"
        );
        assert_eq!(
            fs::read(stage.path().join("loose.txt")).unwrap(),
            b"archivo suelto"
        );
        assert!(stage.path().join("folder/nested/empty").is_dir());
    }

    #[test]
    fn rejects_writer_and_preserves_original() {
        let source = source_fixture();
        let path = source.path().join("file.txt");
        fs::write(&path, b"original").unwrap();
        let writer = OpenOptions::new()
            .write(true)
            .share_mode(7)
            .open(&path)
            .unwrap();
        assert!(capture(std::slice::from_ref(&path), &CancellationToken::new()).is_err());
        drop(writer);
        assert_eq!(fs::read(path).unwrap(), b"original");
    }

    #[test]
    fn cancellation_and_collision_do_not_overwrite_destination() {
        let source = source_fixture();
        let path = source.path().join("file.txt");
        fs::write(&path, b"source").unwrap();
        let token = CancellationToken::new();
        let mut captured = capture(std::slice::from_ref(&path), &token).unwrap();
        captured.release_directories();
        let stage = tempfile::tempdir().unwrap();
        fs::write(stage.path().join("file.txt"), b"existing").unwrap();
        assert_eq!(
            captured
                .materialize(stage.path(), &token)
                .unwrap_err()
                .kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            fs::read(stage.path().join("file.txt")).unwrap(),
            b"existing"
        );
        token.cancel();
        assert_eq!(
            capture(&[path], &token).err().unwrap().kind(),
            io::ErrorKind::Interrupted
        );
    }

    #[test]
    fn cancelled_after_capture_does_not_create_staged_file() {
        let source = source_fixture();
        let path = source.path().join("file.txt");
        fs::write(&path, b"source").unwrap();
        let token = CancellationToken::new();
        let mut captured = capture(&[path], &token).unwrap();
        captured.release_directories();
        let stage = tempfile::tempdir().unwrap();
        token.cancel();
        assert_eq!(
            captured
                .materialize(stage.path(), &token)
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted
        );
        assert!(!stage.path().join("file.txt").exists());
    }

    #[test]
    fn cancellation_during_read_stops_before_writing_the_chunk() {
        struct CancellingReader(CancellationToken, usize);
        impl Read for CancellingReader {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                self.1 += 1;
                buffer[0] = 42;
                self.0.cancel();
                Ok(1)
            }
        }
        let token = CancellationToken::new();
        let mut input = CancellingReader(token.clone(), 0);
        let mut output = Vec::new();
        let mut buffer = [0; 16];
        let error = copy_retained(&mut input, &mut output, &mut buffer, &token).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(input.1, 1);
        assert!(output.is_empty());
    }

    #[test]
    fn guarded_directory_cannot_be_renamed_until_capture_is_released() {
        let source = source_fixture();
        let folder = source.path().join("folder");
        let renamed = source.path().join("renamed");
        fs::create_dir(&folder).unwrap();
        let resolved_parent = fs::canonicalize(source.path()).unwrap();
        let resolved_folder = fs::canonicalize(&folder).unwrap();
        assert!(same_path(
            resolved_folder.parent().unwrap(),
            &resolved_parent
        ));
        assert!(same_path(
            &fs::canonicalize(renamed.parent().unwrap()).unwrap(),
            &resolved_parent
        ));
        let mut captured =
            capture(std::slice::from_ref(&folder), &CancellationToken::new()).unwrap();
        let error = fs::rename(&folder, &renamed).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(32));
        captured.release_directories();
        fs::rename(&folder, &renamed).unwrap();
        let stage = tempfile::tempdir().unwrap();
        captured
            .materialize(stage.path(), &CancellationToken::new())
            .unwrap();
        assert!(stage.path().join("folder").is_dir());
    }

    #[test]
    fn rejects_junction_without_touching_its_target() {
        use std::process::Command;
        let source = source_fixture();
        let target = tempfile::tempdir().unwrap();
        let junction = source.path().join("junction");
        let sentinel = target.path().join("keep.txt");
        fs::write(&sentinel, b"conservar").unwrap();
        let output = Command::new("cmd.exe")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                target.path().to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "no se pudo crear junction de prueba"
        );
        assert!(capture(std::slice::from_ref(&junction), &CancellationToken::new()).is_err());
        // Elimina solo el junction (no recursivo); el destino pertenece a otro tempfile.
        fs::remove_dir(&junction).unwrap();
        assert_eq!(fs::read(sentinel).unwrap(), b"conservar");
    }
}
