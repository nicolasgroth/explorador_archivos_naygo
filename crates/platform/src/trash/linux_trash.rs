// Naygo — papelera FreeDesktop para Linux.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Backend Linux de Papelera. Cada operación corre en el worker del controlador; este módulo
//! implementa almacenamiento local por filesystem y recibos restaurables, sin borrado permanente.

use super::{CancelProbe, TrashError, TrashItemFailure, TrashProgress, TrashReceipt};
use chrono::Local;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

pub fn move_to_trash(paths: &[PathBuf]) -> Result<Vec<TrashReceipt>, TrashError> {
    let (tx, _rx) = std::sync::mpsc::channel();
    move_to_trash_with_progress(paths, tx, std::sync::Arc::new(|| false))
}

pub fn move_to_trash_with_progress(
    paths: &[PathBuf],
    progress: std::sync::mpsc::Sender<TrashProgress>,
    cancelled: CancelProbe,
) -> Result<Vec<TrashReceipt>, TrashError> {
    move_items(paths, progress, cancelled, trash_one)
}

fn move_items(
    paths: &[PathBuf],
    progress: std::sync::mpsc::Sender<TrashProgress>,
    cancelled: CancelProbe,
    mut move_one: impl FnMut(&Path) -> io::Result<TrashReceipt>,
) -> Result<Vec<TrashReceipt>, TrashError> {
    let mut receipts = Vec::new();
    let mut failures = Vec::new();
    for (index, requested) in paths.iter().enumerate() {
        if cancelled() {
            failures.extend(
                paths
                    .iter()
                    .enumerate()
                    .skip(index)
                    .map(|(source_index, path)| TrashItemFailure {
                        source_index,
                        original: path.clone(),
                        detail: "operación cancelada antes de enviar a Papelera".into(),
                    }),
            );
            break;
        }
        match move_one(requested) {
            Ok(receipt) => receipts.push(receipt),
            Err(error) => failures.push(TrashItemFailure {
                source_index: index,
                original: requested.clone(),
                detail: error.to_string(),
            }),
        }
        let _ = progress.send(TrashProgress {
            work_total: paths.len().min(u32::MAX as usize) as u32,
            work_done: (index + 1).min(u32::MAX as usize) as u32,
        });
    }
    if failures.is_empty() {
        Ok(receipts)
    } else {
        Err(TrashError::Partial { receipts, failures })
    }
}

fn trash_one(requested: &Path) -> io::Result<TrashReceipt> {
    let original = absolute_entry_path(requested)?;
    let source_metadata = fs::symlink_metadata(&original)?;
    let source_dev = source_metadata.dev();
    let uid = unsafe { libc::getuid() };
    let data_home = xdg_data_home()?;
    fs::create_dir_all(&data_home)?;
    let home_dev = fs::metadata(&data_home)?.dev();
    let trash_root = if source_dev == home_dev {
        ensure_private_dir(&data_home.join("Trash"), uid)?
    } else {
        let topdir = filesystem_topdir(&original, source_dev)?;
        filesystem_trash_root(&topdir, uid)?
    };
    trash_to_root(&original, &trash_root, uid, source_dev)
}

fn trash_to_root(
    original: &Path,
    trash_root: &Path,
    uid: u32,
    source_dev: u64,
) -> io::Result<TrashReceipt> {
    ensure_private_dir(&trash_root.join("files"), uid)?;
    ensure_private_dir(&trash_root.join("info"), uid)?;
    let files_dir = trash_root.join("files");
    let info_dir = trash_root.join("info");
    if fs::metadata(&files_dir)?.dev() != source_dev {
        return Err(io::Error::new(
            io::ErrorKind::CrossesDevices,
            "la Papelera seleccionada no está en el mismo filesystem",
        ));
    }

    let basename = original.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "no se puede enviar la raíz a Papelera",
        )
    })?;
    for suffix in 0..10_000u32 {
        let candidate = unique_name(basename, suffix);
        let info_path = info_dir.join(append_suffix(&candidate, ".trashinfo"));
        let mut info = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&info_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let body = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            encode_path(original),
            Local::now().format("%Y-%m-%dT%H:%M:%S")
        );
        if let Err(error) = info
            .write_all(body.as_bytes())
            .and_then(|_| info.sync_all())
        {
            drop(info);
            let _ = fs::remove_file(&info_path);
            return Err(error);
        }
        drop(info);

        let trashed_path = files_dir.join(&candidate);
        match rename_no_replace(original, &trashed_path) {
            Ok(()) => {
                return Ok(TrashReceipt {
                    original: original.to_path_buf(),
                    trashed_path: Some(trashed_path),
                });
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&info_path);
            }
            Err(error) => {
                let _ = fs::remove_file(&info_path);
                return Err(error);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no se encontró un nombre único en la Papelera",
    ))
}

pub fn restore_from_trash(receipts: &[TrashReceipt]) -> Result<(), TrashError> {
    let mut restored = Vec::new();
    let mut failures = Vec::new();
    for (source_index, receipt) in receipts.iter().enumerate() {
        match restore_one(receipt) {
            Ok(()) => restored.push(receipt.clone()),
            Err(error) => failures.push(TrashItemFailure {
                source_index,
                original: receipt.original.clone(),
                detail: error.to_string(),
            }),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else if restored.is_empty() {
        Err(TrashError::Failed(
            failures
                .iter()
                .map(|failure| format!("{}: {}", failure.original.display(), failure.detail))
                .collect::<Vec<_>>()
                .join("; "),
        ))
    } else {
        Err(TrashError::Partial {
            receipts: restored,
            failures,
        })
    }
}

fn restore_one(receipt: &TrashReceipt) -> io::Result<()> {
    let trashed = receipt.trashed_path.as_deref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "el recibo no contiene la ruta de Papelera",
        )
    })?;
    let files_dir = trashed
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "ruta de Papelera inválida"))?;
    if !trashed.is_absolute() || files_dir.file_name().is_none_or(|name| name != "files") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ruta de Papelera inválida",
        ));
    }
    let name = trashed.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "ítem de Papelera sin nombre")
    })?;
    let trash_root = files_dir
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "raíz de Papelera inválida"))?;
    let uid = unsafe { libc::getuid() };
    ensure_private_dir(trash_root, uid)?;
    ensure_private_dir(files_dir, uid)?;
    let info_dir = trash_root.join("info");
    ensure_private_dir(&info_dir, uid)?;
    let info_path = info_dir.join(append_suffix(name, ".trashinfo"));
    let info_metadata = fs::symlink_metadata(&info_path)?;
    if info_metadata.file_type().is_symlink()
        || !info_metadata.is_file()
        || info_metadata.uid() != uid
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "recibo trashinfo inseguro",
        ));
    }
    let stored_original = read_original_path(&info_path)?;
    if stored_original != receipt.original {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "la ubicación original no coincide con el recibo",
        ));
    }
    if fs::symlink_metadata(&receipt.original).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "el destino original está ocupado",
        ));
    }
    let parent = receipt.original.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "destino original sin carpeta")
    })?;
    if fs::metadata(parent)?.dev() != fs::symlink_metadata(trashed)?.dev() {
        return Err(io::Error::new(
            io::ErrorKind::CrossesDevices,
            "el filesystem del destino original cambió",
        ));
    }
    rename_no_replace(trashed, &receipt.original)?;
    if let Err(error) = fs::remove_file(&info_path) {
        tracing::warn!(%error, path = %info_path.display(), "no se pudo limpiar el recibo tras restaurar");
    }
    Ok(())
}

fn rename_no_replace(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "ruta contiene NUL"))?;
    let target = std::ffi::CString::new(target.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "ruta contiene NUL"))?;
    // Linux renameat2 con RENAME_NOREPLACE conserva el destino si otra app lo crea en la carrera.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn xdg_data_home() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Ok(path);
        }
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME no está definido"))?;
    Ok(PathBuf::from(home).join(".local/share"))
}

fn filesystem_topdir(path: &Path, device: u64) -> io::Result<PathBuf> {
    let canonical_parent = fs::canonicalize(path.parent().unwrap_or(Path::new("/")))?;
    let mut top = canonical_parent;
    loop {
        let Some(parent) = top.parent() else {
            return Ok(top);
        };
        if fs::metadata(parent)?.dev() != device {
            return Ok(top);
        }
        top = parent.to_path_buf();
    }
}

fn filesystem_trash_root(topdir: &Path, uid: u32) -> io::Result<PathBuf> {
    let shared = topdir.join(".Trash");
    let shared_is_valid = match fs::symlink_metadata(&shared) {
        Ok(metadata) => {
            !metadata.file_type().is_symlink() && metadata.is_dir() && metadata.mode() & 0o1000 != 0
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    if shared_is_valid {
        let user_root = shared.join(uid.to_string());
        if ensure_private_dir(&user_root, uid).is_ok() {
            return Ok(user_root);
        }
    }
    ensure_private_dir(&topdir.join(format!(".Trash-{uid}")), uid)
}

fn ensure_private_dir(path: &Path, uid: u32) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || metadata.uid() != uid
                || metadata.mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("directorio de Papelera inseguro: {}", path.display()),
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            builder.create(path)?;
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || metadata.uid() != uid
                || metadata.mode() & 0o077 != 0
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "permisos inesperados al crear la Papelera",
                ));
            }
        }
        Err(error) => return Err(error),
    }
    Ok(path.to_path_buf())
}

fn absolute_entry_path(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let Some(name) = absolute.file_name() else {
        return fs::canonicalize(absolute);
    };
    let parent = absolute.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "la ruta no tiene carpeta padre",
        )
    })?;
    let resolved_parent = fs::canonicalize(parent)?;
    let mut resolved = resolved_parent;
    resolved.push(name);
    Ok(resolved)
}

fn unique_name(name: &std::ffi::OsStr, suffix: u32) -> OsString {
    let mut result = name.to_os_string();
    if suffix > 0 {
        result.push(format!(".{suffix}"));
    }
    result
}

fn append_suffix(name: &std::ffi::OsStr, suffix: &str) -> OsString {
    let mut result = name.to_os_string();
    result.push(suffix);
    result
}

fn encode_path(path: &Path) -> String {
    let mut output = String::new();
    for byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~".contains(byte) {
            output.push(*byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(output, "%{byte:02X}");
        }
    }
    output
}

fn read_original_path(info_path: &Path) -> io::Result<PathBuf> {
    let bytes = fs::read(info_path)?;
    let mut lines = bytes.split(|byte| *byte == b'\n');
    if lines.next() != Some(b"[Trash Info]".as_slice()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cabecera trashinfo inválida",
        ));
    }
    for line in lines {
        let Some(value) = line.strip_prefix(b"Path=") else {
            continue;
        };
        let decoded = decode_path(value.strip_suffix(b"\r").unwrap_or(value))?;
        let original = PathBuf::from(OsString::from_vec(decoded));
        if !original.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Path de trashinfo no absoluto",
            ));
        }
        return Ok(original);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "falta Path en trashinfo",
    ))
}

fn decode_path(value: &[u8]) -> io::Result<Vec<u8>> {
    let mut decoded = Vec::with_capacity(value.len());
    let mut index = 0;
    while index < value.len() {
        if value[index] == b'%' {
            if index + 2 >= value.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "escape URL incompleto",
                ));
            }
            let high = hex(value[index + 1])?;
            let low = hex(value[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(value[index]);
            index += 1;
        }
    }
    Ok(decoded)
}

fn hex(byte: u8) -> io::Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "escape URL inválido",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trashinfo_codifica_y_recupera_ruta_no_utf8() {
        let original = PathBuf::from(OsString::from_vec(b"/tmp/foto \xff%\n.stl".to_vec()));
        let encoded = encode_path(&original);
        assert_eq!(encoded, "/tmp/foto%20%FF%25%0A.stl");
        assert_eq!(
            decode_path(encoded.as_bytes()).unwrap(),
            original.as_os_str().as_bytes()
        );
    }

    #[test]
    fn trashinfo_lee_la_primera_ruta_y_omite_claves_desconocidas() {
        let dir = tempfile::tempdir().unwrap();
        let info = dir.path().join("item.trashinfo");
        fs::write(
            &info,
            b"[Trash Info]\nOther=value\nPath=/tmp/first%20name\nPath=/tmp/ignored\nDeletionDate=2026-10-19T10:11:12\n",
        )
        .unwrap();
        assert_eq!(
            read_original_path(&info).unwrap(),
            PathBuf::from("/tmp/first name")
        );
    }

    #[test]
    fn nombres_unicos_conservan_bytes_y_agregan_sufijo() {
        let name = OsString::from_vec(b"pieza-\xfe.stl".to_vec());
        assert_eq!(unique_name(&name, 0).as_bytes(), b"pieza-\xfe.stl");
        assert_eq!(unique_name(&name, 3).as_bytes(), b"pieza-\xfe.stl.3");
    }

    #[test]
    fn resolver_padre_respeta_symlink_seguido_de_parent_dir_y_preserva_enlace_final() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        let child = real.join("child");
        fs::create_dir_all(&child).unwrap();
        let alias = dir.path().join("alias");
        symlink(&child, &alias).unwrap();
        let target = real.join("entry.txt");
        fs::write(&target, b"real parent").unwrap();
        fs::write(dir.path().join("entry.txt"), b"lexical parent").unwrap();

        let request = alias.join("..").join("entry.txt");
        assert_eq!(absolute_entry_path(&request).unwrap(), target);

        let link = real.join("final-link.txt");
        symlink(&target, &link).unwrap();
        let resolved_link = absolute_entry_path(&link).unwrap();
        assert_eq!(resolved_link, link);
        assert!(fs::symlink_metadata(resolved_link)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn homonimos_se_reciclan_sin_sobrescribir_y_se_restauran_por_recibo() {
        let dir = tempfile::tempdir().unwrap();
        let left = dir.path().join("left");
        let right = dir.path().join("right");
        fs::create_dir(&left).unwrap();
        fs::create_dir(&right).unwrap();
        let first = left.join("pieza.stl");
        let second = right.join("pieza.stl");
        fs::write(&first, b"first").unwrap();
        fs::write(&second, b"second").unwrap();
        let uid = unsafe { libc::getuid() };
        let trash_root = dir.path().join("trash");
        ensure_private_dir(&trash_root, uid).unwrap();
        let dev = fs::metadata(&first).unwrap().dev();

        let first_receipt = trash_to_root(&first, &trash_root, uid, dev).unwrap();
        let second_receipt = trash_to_root(&second, &trash_root, uid, dev).unwrap();
        let first_trashed = first_receipt.trashed_path.as_ref().unwrap();
        let second_trashed = second_receipt.trashed_path.as_ref().unwrap();
        assert_ne!(first_trashed, second_trashed);
        assert_eq!(fs::read(first_trashed).unwrap(), b"first");
        assert_eq!(fs::read(second_trashed).unwrap(), b"second");

        restore_one(&second_receipt).unwrap();
        restore_one(&first_receipt).unwrap();
        assert_eq!(fs::read(&first).unwrap(), b"first");
        assert_eq!(fs::read(&second).unwrap(), b"second");
    }

    #[test]
    fn restaurar_no_reemplaza_un_destino_ocupado_y_conserva_el_recibo() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original.txt");
        fs::write(&original, b"reciclado").unwrap();
        let uid = unsafe { libc::getuid() };
        let trash_root = dir.path().join("trash");
        ensure_private_dir(&trash_root, uid).unwrap();
        let dev = fs::metadata(&original).unwrap().dev();
        let receipt = trash_to_root(&original, &trash_root, uid, dev).unwrap();
        fs::write(&original, b"archivo nuevo").unwrap();

        let error = restore_one(&receipt).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&original).unwrap(), b"archivo nuevo");
        assert!(receipt.trashed_path.as_ref().unwrap().exists());
    }

    #[test]
    fn reciclar_enlace_simbolico_no_mueve_su_destino() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.txt");
        let link = dir.path().join("link.txt");
        fs::write(&target, b"target").unwrap();
        symlink(&target, &link).unwrap();
        let uid = unsafe { libc::getuid() };
        let trash_root = dir.path().join("trash");
        ensure_private_dir(&trash_root, uid).unwrap();
        let dev = fs::symlink_metadata(&link).unwrap().dev();

        let receipt = trash_to_root(&link, &trash_root, uid, dev).unwrap();
        assert!(target.exists());
        assert!(fs::symlink_metadata(receipt.trashed_path.unwrap())
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn papelera_compartida_symlink_usa_fallback_privado_por_uid() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let topdir = dir.path();
        let target = topdir.join("real-trash");
        ensure_private_dir(&target, unsafe { libc::getuid() }).unwrap();
        symlink(&target, topdir.join(".Trash")).unwrap();

        let uid = unsafe { libc::getuid() };
        let trash = filesystem_trash_root(topdir, uid).unwrap();
        assert_eq!(trash, topdir.join(format!(".Trash-{uid}")));
        assert!(!fs::symlink_metadata(&trash)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn cancelacion_de_lote_preserva_recibos_y_no_visita_los_siguientes() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let paths = [
            PathBuf::from("first"),
            PathBuf::from("second"),
            PathBuf::from("third"),
        ];
        let cancel_checks = std::sync::Arc::new(AtomicUsize::new(0));
        let cancel_probe = cancel_checks.clone();
        let (progress_tx, progress_rx) = std::sync::mpsc::channel();
        let result = move_items(
            &paths,
            progress_tx,
            std::sync::Arc::new(move || cancel_probe.fetch_add(1, Ordering::SeqCst) >= 1),
            |path| {
                Ok(TrashReceipt {
                    original: path.to_path_buf(),
                    trashed_path: Some(PathBuf::from("/tmp/trash/files/first")),
                })
            },
        );
        let Err(TrashError::Partial { receipts, failures }) = result else {
            panic!("la cancelación parcial debe conservar recibos y fallos");
        };
        assert_eq!(receipts.len(), 1);
        assert_eq!(
            failures
                .iter()
                .map(|failure| failure.source_index)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(progress_rx.recv().unwrap().work_done, 1);
    }
}
