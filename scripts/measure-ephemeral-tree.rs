// Naygo — experimento local de retención de árboles temporales para el drop de archivadores.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
// No implementa OLE ni modifica Naygo. Conserva sus resultados bajo target/agent-out.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const SHARE_READ: u32 = 1;
const SHARE_WRITE: u32 = 2;
const SHARE_DELETE: u32 = 4;
const BACKUP_SEMANTICS: u32 = 0x0200_0000;
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
const FILE_LIST_DIRECTORY: u32 = 0x0000_0001;

fn open_without_reparse(path: &Path, directory: bool) -> io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.read(true).custom_flags(OPEN_REPARSE_POINT);
    if directory {
        // En el fixture impidió borrar/renombrar directorios retenidos; no protege antecesores.
        options
            .access_mode(FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY)
            .share_mode(SHARE_READ | SHARE_WRITE)
            .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS);
    } else {
        // Permite que el origen borre el nombre después de capturar el contenido.
        options.share_mode(SHARE_READ | SHARE_WRITE | SHARE_DELETE);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if metadata.file_attributes() & ATTRIBUTE_REPARSE_POINT != 0 || metadata.is_dir() != directory {
        return Err(io::Error::other("punto de reanálisis o tipo inesperado"));
    }
    Ok(file)
}

fn capture(
    root: &Path,
    dir: &Path,
    dirs: &mut Vec<PathBuf>,
    files: &mut Vec<(PathBuf, File)>,
    guards: &mut Vec<File>,
) -> io::Result<()> {
    // Se retiene el directorio durante `read_dir` y apertura de hijos; aún faltan pruebas de carrera.
    guards.push(open_without_reparse(dir, true)?);
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .to_owned();
        if ty.is_symlink() {
            return Err(io::Error::other("se rechazó enlace"));
        }
        if ty.is_dir() {
            dirs.push(relative);
            capture(root, &path, dirs, files, guards)?;
        } else if ty.is_file() {
            files.push((relative, open_without_reparse(&path, false)?));
        } else {
            return Err(io::Error::other("tipo no soportado"));
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let first = args.next().unwrap_or_else(|| "1000".into());
    if first == "--check" {
        let root = PathBuf::from(args.next().ok_or_else(|| io::Error::other("falta ruta"))?);
        let base = fs::canonicalize("target/agent-out")?;
        let parent = fs::canonicalize(
            root.parent()
                .ok_or_else(|| io::Error::other("falta padre"))?,
        )?;
        if !parent.starts_with(&base) {
            return Err(io::Error::other("ruta fuera de target/agent-out"));
        }
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        let mut guards = Vec::new();
        capture(&root, &root, &mut dirs, &mut files, &mut guards)?;
        println!("check_ok files={} dirs={}", files.len(), dirs.len());
        return Ok(());
    }
    let count = first.parse::<usize>().map_err(io::Error::other)?;
    if count == 0 || count > 10_000 {
        return Err(io::Error::other("usar entre 1 y 10000 archivos"));
    }
    let base = fs::canonicalize("target/agent-out")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let run = base.join(format!("ephemeral-tree-{}-{nonce}", std::process::id()));
    fs::create_dir(&run)?;
    let source = run.join("source");
    let stage = run.join("retained");
    fs::create_dir(&source)?;
    fs::create_dir(&stage)?;
    fs::create_dir_all(source.join("empty/nested"))?;
    for i in 0..count {
        let directory = source.join(format!("group-{:03}", i / 100));
        fs::create_dir_all(&directory)?;
        fs::write(
            directory.join(format!("file-{i:05}.txt")),
            format!("fixture-{i}\n"),
        )?;
    }
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut guards = Vec::new();
    let started = Instant::now();
    capture(&source, &source, &mut dirs, &mut files, &mut guards)?;
    let elapsed = started.elapsed();
    if files.len() != count {
        return Err(io::Error::other("recuento incompleto"));
    }
    // Borrar solo el árbol sintético recién creado, tras verificar sus rutas absolutas.
    let resolved_run = fs::canonicalize(&run)?;
    let resolved_source = fs::canonicalize(&source)?;
    if resolved_run.parent() != Some(base.as_path())
        || resolved_source.parent() != Some(resolved_run.as_path())
        || resolved_source.file_name().and_then(|s| s.to_str()) != Some("source")
    {
        return Err(io::Error::other(
            "origen temporal fuera de la carpeta esperada",
        ));
    }
    // Solo se intenta sobre la carpeta vacía sintética; el handle debe impedir su eliminación.
    let empty_parent = fs::canonicalize(resolved_source.join("empty"))?;
    let empty = fs::canonicalize(empty_parent.join("nested"))?;
    if empty_parent.parent() != Some(resolved_source.as_path())
        || empty.parent() != Some(empty_parent.as_path())
    {
        return Err(io::Error::other(
            "carpeta de prueba fuera del origen esperado",
        ));
    }
    let lock_error =
        fs::remove_dir(&empty).expect_err("el handle no impidió eliminar el directorio");
    if lock_error.raw_os_error() != Some(32) {
        return Err(io::Error::other(format!(
            "fallo de bloqueo inesperado: {lock_error}"
        )));
    }
    let renamed = empty_parent.join("renamed");
    if renamed.parent() != Some(empty_parent.as_path()) || renamed.exists() {
        return Err(io::Error::other("destino de prueba inesperado"));
    }
    let rename_error =
        fs::rename(&empty, &renamed).expect_err("el handle no impidió renombrar el directorio");
    if rename_error.raw_os_error() != Some(32) {
        return Err(io::Error::other(format!(
            "fallo de renombre inesperado: {rename_error}"
        )));
    }
    // Liberar directorios antes de simular la limpieza del archivador; los archivos quedan abiertos.
    drop(guards);
    fs::remove_dir_all(&resolved_source)?;
    for dir in &dirs {
        fs::create_dir_all(stage.join(dir))?;
    }
    for (relative, mut input) in files {
        let target = stage.join(relative);
        let mut output = File::create(&target)?;
        io::copy(&mut input, &mut output)?;
        output.flush()?;
    }
    if !stage.join("empty/nested").is_dir() {
        return Err(io::Error::other("faltó carpeta vacía"));
    }
    for i in 0..count {
        let mut contents = String::new();
        File::open(stage.join(format!("group-{:03}/file-{i:05}.txt", i / 100)))?
            .read_to_string(&mut contents)?;
        if contents != format!("fixture-{i}\n") {
            return Err(io::Error::other("contenido incorrecto"));
        }
    }
    println!(
        "files={count} dirs={} capture_ms={:.3} paths_removed=true verified=true fixture={}",
        dirs.len(),
        elapsed.as_secs_f64() * 1000.0,
        run.display()
    );
    Ok(())
}
