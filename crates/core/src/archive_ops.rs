// Naygo — comprimir/extraer .zip: lógica pura (sin UI ni Windows), cancelable y testeable.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Crear y extraer archivos `.zip`. Puro: recibe rutas + un `CancellationToken` + callbacks de
//! progreso/conflicto; no conoce egui/Slint/Win32. El crate `zip` ya es dependencia (lo usa el
//! preview). Protección zip-slip al extraer (mismo criterio que `icon_pack::import`).

use crate::cancel::CancellationToken;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

/// Resultado de UNA entrada procesada (para el resumen y el deshacer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveOpItem {
    /// Ruta REAL escrita: el `.zip` (comprimir) o el archivo/carpeta extraído (extraer).
    pub path: PathBuf,
    pub outcome: ArchiveOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchiveOutcome {
    Done,
    Skipped,
    Failed(String),
}

/// Decisión ante un conflicto al extraer (un archivo destino ya existe).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractConflict {
    Overwrite,
    Skip,
    KeepBoth,
    Cancel,
}

/// Error tipado de las operaciones de archivo comprimido. Ningún panic.
#[derive(Debug)]
pub enum ArchiveError {
    Io(std::io::Error),
    Zip(String),
    Cancelled,
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Io(e) => write!(f, "io: {e}"),
            ArchiveError::Zip(s) => write!(f, "zip: {s}"),
            ArchiveError::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl From<std::io::Error> for ArchiveError {
    fn from(e: std::io::Error) -> Self {
        ArchiveError::Io(e)
    }
}

/// Nombre por defecto del `.zip` a crear desde `sources`:
/// - 1 ítem (archivo o carpeta) → su nombre base + ".zip".
/// - varios ítems → "archivos.zip".
///
/// El nombre base de un archivo CONSERVA su parte sin extensión: "informe.txt" → "informe.zip".
/// Una carpeta usa su nombre tal cual: "proyecto" → "proyecto.zip".
pub fn default_zip_name(sources: &[PathBuf]) -> String {
    if sources.len() == 1 {
        let p = &sources[0];
        // file_stem para archivos con extensión; para carpetas (sin "extensión") file_stem == nombre.
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            return format!("{stem}.zip");
        }
    }
    "archivos.zip".to_string()
}

/// Recorre `sources` (archivos y/o carpetas, recursivo) y los empaqueta en `dest_zip`.
/// Progreso por BYTES. Cancelar → aborta y borra el `.zip` parcial. Un source ilegible se
/// registra Failed y la op continúa.
pub fn compress_zip(
    sources: &[PathBuf],
    dest_zip: &Path,
    on_progress: &mut dyn FnMut(u64, u64),
    token: &CancellationToken,
) -> Result<Vec<ArchiveOpItem>, ArchiveError> {
    let mut entries: Vec<(PathBuf, String)> = Vec::new();
    for src in sources {
        let base = src.parent().unwrap_or(Path::new(""));
        collect_entries(src, base, &mut entries);
    }
    let total: u64 = entries
        .iter()
        .map(|(p, _)| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();

    let file = std::fs::File::create(dest_zip)?;
    let mut zipw = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut done: u64 = 0;
    let mut items: Vec<ArchiveOpItem> = Vec::new();
    for (disk, internal) in &entries {
        if token.is_cancelled() {
            drop(zipw);
            let _ = std::fs::remove_file(dest_zip);
            return Err(ArchiveError::Cancelled);
        }
        token.wait_if_paused();
        if disk.is_dir() {
            let dir_name = format!("{}/", internal.trim_end_matches('/'));
            if zipw.add_directory(dir_name, opts).is_err() {
                items.push(ArchiveOpItem {
                    path: disk.clone(),
                    outcome: ArchiveOutcome::Failed("add_directory".into()),
                });
            }
            continue;
        }
        match std::fs::File::open(disk) {
            Ok(mut f) => {
                if zipw.start_file(internal.clone(), opts).is_err() {
                    items.push(ArchiveOpItem {
                        path: disk.clone(),
                        outcome: ArchiveOutcome::Failed("start_file".into()),
                    });
                    continue;
                }
                let mut buf = [0u8; 64 * 1024];
                let mut failed = false;
                loop {
                    if token.is_cancelled() {
                        drop(zipw);
                        let _ = std::fs::remove_file(dest_zip);
                        return Err(ArchiveError::Cancelled);
                    }
                    let n = match f.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => {
                            failed = true;
                            break;
                        }
                    };
                    if zipw.write_all(&buf[..n]).is_err() {
                        failed = true;
                        break;
                    }
                    done += n as u64;
                    on_progress(done, total);
                }
                items.push(ArchiveOpItem {
                    path: disk.clone(),
                    outcome: if failed {
                        ArchiveOutcome::Failed("read/write".into())
                    } else {
                        ArchiveOutcome::Done
                    },
                });
            }
            Err(e) => items.push(ArchiveOpItem {
                path: disk.clone(),
                outcome: ArchiveOutcome::Failed(e.to_string()),
            }),
        }
    }
    zipw.finish().map_err(|e| ArchiveError::Zip(e.to_string()))?;
    Ok(items)
}

/// Expande `path` a entradas (disk, internal). `base` es la carpeta padre del source de nivel
/// superior; la ruta interna en el zip es `path` relativa a `base` (con `/` como separador).
fn collect_entries(path: &Path, base: &Path, out: &mut Vec<(PathBuf, String)>) {
    let internal = path
        .strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    if path.is_dir() {
        out.push((path.to_path_buf(), internal));
        if let Ok(rd) = std::fs::read_dir(path) {
            let mut children: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            children.sort();
            for c in children {
                collect_entries(&c, base, out);
            }
        }
    } else {
        out.push((path.to_path_buf(), internal));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_zip_name_un_archivo_usa_su_stem() {
        assert_eq!(default_zip_name(&[PathBuf::from("C:/x/informe.txt")]), "informe.zip");
    }

    #[test]
    fn default_zip_name_una_carpeta_usa_su_nombre() {
        assert_eq!(default_zip_name(&[PathBuf::from("C:/x/proyecto")]), "proyecto.zip");
    }

    #[test]
    fn default_zip_name_varios_es_generico() {
        let v = vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")];
        assert_eq!(default_zip_name(&v), "archivos.zip");
    }

    fn noop_progress() -> impl FnMut(u64, u64) {
        |_done, _total| {}
    }

    #[test]
    fn compress_un_archivo_crea_zip_leible() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("hola.txt");
        std::fs::write(&src, b"contenido").unwrap();
        let zip_path = dir.path().join("out.zip");
        let token = CancellationToken::new();
        let items = compress_zip(&[src.clone()], &zip_path, &mut noop_progress(), &token).unwrap();
        assert!(zip_path.exists());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].outcome, ArchiveOutcome::Done);
        let f = std::fs::File::open(&zip_path).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        assert!(z.by_name("hola.txt").is_ok());
    }

    #[test]
    fn compress_carpeta_anidada_incluye_todo_recursivo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proyecto");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("README.md"), b"x").unwrap();
        std::fs::write(root.join("src/main.rs"), b"fn main(){}").unwrap();
        let zip_path = dir.path().join("p.zip");
        let token = CancellationToken::new();
        compress_zip(&[root.clone()], &zip_path, &mut noop_progress(), &token).unwrap();
        let f = std::fs::File::open(&zip_path).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
        assert!(names.iter().any(|n| n.ends_with("proyecto/README.md")));
        assert!(names.iter().any(|n| n.ends_with("proyecto/src/main.rs")));
    }

    #[test]
    fn compress_cancelado_borra_el_parcial() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        std::fs::write(&src, vec![0u8; 1024]).unwrap();
        let zip_path = dir.path().join("c.zip");
        let token = CancellationToken::new();
        token.cancel();
        let r = compress_zip(&[src], &zip_path, &mut noop_progress(), &token);
        assert!(matches!(r, Err(ArchiveError::Cancelled)));
        assert!(!zip_path.exists(), "el .zip parcial se borra al cancelar");
    }
}
