// Naygo — revisión acotada de archivos fallidos, sin repetir árboles ni pasos completados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::{OpPlan, OpStep};
use crate::CancellationToken;
use std::{io, path::PathBuf, time::SystemTime};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryFile {
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    canonical: PathBuf,
    bytes: u64,
    modified: SystemTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryReview {
    pub files: Vec<RetryFile>,
    sources: Vec<Stamp>,
    destinations: Vec<Option<Stamp>>,
    destination_parents: Vec<PathBuf>,
}

fn stamp(path: &std::path::Path) -> io::Result<Stamp> {
    let meta = std::fs::symlink_metadata(path)?;
    // No se reinterpreta un fallo de archivo como una copia recursiva o un enlace.
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Not a regular file",
        ));
    }
    Ok(Stamp {
        canonical: path.canonicalize()?,
        bytes: meta.len(),
        modified: meta.modified()?,
    })
}

// Canoniza el ancestro existente sin crear carpetas. Detecta que un padre se
// redirigió entre revisar y confirmar, incluso si el archivo destino no existe.
fn destination_parent(path: &std::path::Path) -> io::Result<PathBuf> {
    let mut current = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let mut missing = Vec::new();
    loop {
        match current.canonicalize() {
            Ok(mut resolved) => {
                if !resolved.is_dir() {
                    return Err(io::ErrorKind::NotADirectory.into());
                }
                for part in missing.into_iter().rev() {
                    resolved.push(part);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing.push(
                    current
                        .file_name()
                        .ok_or(io::ErrorKind::InvalidInput)?
                        .to_os_string(),
                );
                current = current.parent().ok_or(io::ErrorKind::InvalidInput)?;
            }
            Err(error) => return Err(error),
        }
    }
}

impl RetryReview {
    pub fn plan(&self) -> OpPlan {
        OpPlan {
            steps: self
                .files
                .iter()
                .zip(&self.sources)
                .map(|(f, s)| OpStep {
                    from: Some(f.source.clone()),
                    to: f.destination.clone(),
                    bytes: s.bytes,
                    is_dir: false,
                })
                .collect(),
            total_bytes: self.sources.iter().map(|s| s.bytes).sum(),
            total_files: self.files.len(),
            pre_delete: Vec::new(),
        }
    }
}

/// Worker únicamente. Relee origen/destino sin modificarlos. La segunda revisión
/// debe coincidir antes de ejecutar; el motor vuelve a consultar los conflictos.
pub fn review(files: Vec<RetryFile>, token: &CancellationToken) -> io::Result<RetryReview> {
    if files.is_empty() || files.len() > 10_000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid retry size",
        ));
    }
    let mut sources = Vec::with_capacity(files.len());
    let mut destinations = Vec::with_capacity(files.len());
    let mut destination_parents = Vec::with_capacity(files.len());
    let mut targets = std::collections::HashSet::new();
    for file in &files {
        if token.is_cancelled() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let source = stamp(&file.source)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", file.source.display())))?;
        destination_parents.push(destination_parent(&file.destination)?);
        if !targets.insert(file.destination.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Repeated destination",
            ));
        }
        let destination = match stamp(&file.destination) {
            Ok(dest) if dest.canonical == source.canonical => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Source equals destination",
                ));
            }
            Ok(dest) => Some(dest),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        sources.push(source);
        destinations.push(destination);
    }
    if token.is_cancelled() {
        return Err(io::ErrorKind::Interrupted.into());
    }
    // Evita un desbordamiento al construir los totales del motor.
    sources
        .iter()
        .try_fold(0u64, |n, s| n.checked_add(s.bytes))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Size overflow"))?;
    Ok(RetryReview {
        files,
        sources,
        destinations,
        destination_parents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_destinations_and_changes_are_reviewed_without_writes() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.txt");
        let destination = dir.path().join("nested/renamed.txt");
        std::fs::write(&source, b"abc").unwrap();
        let files = vec![RetryFile {
            source: source.clone(),
            destination: destination.clone(),
        }];
        let first = review(files.clone(), &CancellationToken::new()).unwrap();
        assert_eq!(first.plan().steps[0].to, destination);
        assert!(first.plan().pre_delete.is_empty());
        assert!(!destination.exists());
        std::fs::write(&source, b"changed").unwrap();
        assert_ne!(
            first,
            review(files.clone(), &CancellationToken::new()).unwrap()
        );
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(review(files.clone(), &cancelled).is_err());
        std::fs::remove_file(source).unwrap();
        assert!(review(files, &CancellationToken::new()).is_err());
    }
    #[test]
    fn rejects_same_path_directories_and_duplicate_targets() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a");
        std::fs::write(&source, b"a").unwrap();
        let token = CancellationToken::new();
        assert!(review(
            vec![RetryFile {
                source: source.clone(),
                destination: source.clone()
            }],
            &token
        )
        .is_err());
        assert!(review(
            vec![RetryFile {
                source: dir.path().to_path_buf(),
                destination: source.clone()
            }],
            &token
        )
        .is_err());
        let file = RetryFile {
            source,
            destination: dir.path().join("b"),
        };
        assert!(review(vec![file.clone(), file], &token).is_err());
    }

    #[test]
    fn destination_appearance_and_changes_invalidate_review() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("dest");
        std::fs::write(&source, b"source").unwrap();
        let files = vec![RetryFile {
            source,
            destination: destination.clone(),
        }];
        let token = CancellationToken::new();
        let absent = review(files.clone(), &token).unwrap();
        std::fs::write(&destination, b"new target").unwrap();
        let appeared = review(files.clone(), &token).unwrap();
        assert_ne!(absent, appeared);
        std::fs::write(&destination, b"changed target contents").unwrap();
        assert_ne!(appeared, review(files, &token).unwrap());
    }
}
