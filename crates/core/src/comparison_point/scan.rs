// Naygo — captura acotada y cancelable, sin seguir enlaces descubiertos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn modified(meta: &fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .try_into()
        .ok()
}
fn link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
fn hash(
    path: &Path,
    before: &fs::Metadata,
    token: &CancellationToken,
) -> Result<Option<String>, SpaceError> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };
    let mut source = file.take(before.len().saturating_add(1));
    let mut digest = Sha256::new();
    let mut block = [0; 65536];
    let mut count = 0;
    loop {
        checkpoint(token)?;
        let n = match source.read(&mut block) {
            Ok(n) => n,
            Err(_) => return Ok(None),
        };
        if n == 0 {
            break;
        }
        digest.update(&block[..n]);
        count += n as u64;
    }
    let after = fs::symlink_metadata(path).ok();
    if count != before.len()
        || modified(before).is_none()
        || !after.is_some_and(|m| {
            m.is_file() && !link(&m) && m.len() == before.len() && modified(&m) == modified(before)
        })
    {
        return Ok(None);
    }
    Ok(Some(format!("{:x}", digest.finalize())))
}

/// Invocar en worker. Progress recibe entradas examinadas; nunca contiene contenido.
pub fn capture(
    scope: Scope,
    token: &CancellationToken,
    mut progress: impl FnMut(usize),
) -> Result<ComparisonPoint, SpaceError> {
    scope.validate()?;
    checkpoint(token)?;
    let mut point = ComparisonPoint {
        version: 1,
        captured_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        resolved_root: fs::canonicalize(&scope.root).ok(),
        scope,
        entries: vec![],
        gaps: vec![],
        truncated: false,
    };
    let mut gaps = HashSet::new();
    if point.resolved_root.is_none() {
        gaps.insert(PathBuf::new());
    }
    let mut pending = vec![PathBuf::new()];
    let mut path_bytes = 0;
    let mut seen = HashSet::new();
    'scan: while let Some(relative_dir) = pending.pop() {
        checkpoint(token)?;
        // Un directorio puede convertirse en enlace mientras esperaba en la pila.
        if !relative_dir.as_os_str().is_empty()
            && !fs::symlink_metadata(point.scope.root.join(&relative_dir))
                .is_ok_and(|m| m.is_dir() && !link(&m))
        {
            gaps.insert(relative_dir);
            continue;
        }
        let directory = match fs::read_dir(point.scope.root.join(&relative_dir)) {
            Ok(d) => d,
            Err(_) => {
                gaps.insert(relative_dir);
                continue;
            }
        };
        for result in directory {
            checkpoint(token)?;
            if point.entries.len() + gaps.len() >= MAX_ENTRIES || path_bytes >= MAX_PATH_BYTES / 2 {
                point.truncated = true;
                break 'scan;
            }
            let entry = match result {
                Ok(e) => e,
                Err(_) => {
                    gaps.insert(relative_dir.clone());
                    continue;
                }
            };
            let path = relative_dir.join(entry.file_name());
            if !relative(&path, false) {
                gaps.insert(relative_dir.clone());
                continue;
            }
            if !point.scope.includes(&path) {
                continue;
            }
            path_bytes += path.as_os_str().len() * 2;
            if !seen.insert(key(&path)) {
                gaps.insert(path);
                continue;
            }
            let meta = match fs::symlink_metadata(entry.path()) {
                Ok(m) => m,
                Err(_) => {
                    gaps.insert(path);
                    continue;
                }
            };
            let kind = if link(&meta) {
                EntryKind::Other
            } else if meta.is_dir() {
                EntryKind::Directory
            } else if meta.is_file() {
                EntryKind::File
            } else {
                EntryKind::Other
            };
            let sha256 = if point.scope.hashed && kind == EntryKind::File {
                hash(&entry.path(), &meta, token)?
            } else {
                None
            };
            if kind == EntryKind::Other
                || (point.scope.hashed && kind == EntryKind::File && sha256.is_none())
            {
                gaps.insert(path.clone());
            }
            if point.scope.recursive && kind == EntryKind::Directory {
                pending.push(path.clone());
            }
            point.entries.push(Record {
                path,
                kind,
                bytes: if kind == EntryKind::File {
                    meta.len()
                } else {
                    0
                },
                modified_ns: modified(&meta),
                sha256,
            });
            progress(point.entries.len());
        }
    }
    checkpoint(token)?;
    let final_root = fs::canonicalize(&point.scope.root).ok();
    if matches!((&point.resolved_root, &final_root), (Some(a), Some(b)) if key(a) != key(b)) {
        return Err(SpaceError::Changed);
    }
    if final_root.is_none() {
        gaps.insert(PathBuf::new());
    }
    point.entries.sort_by_cached_key(|e| key(&e.path));
    point.gaps = gaps.into_iter().collect();
    point.gaps.sort();
    point.validate()?;
    Ok(point)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_during_hash_and_cancel_are_not_verified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        fs::write(&path, "before").unwrap();
        let before = fs::metadata(&path).unwrap();
        fs::write(&path, "longer content").unwrap();
        assert!(hash(&path, &before, &CancellationToken::new())
            .unwrap()
            .is_none());
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            hash(&path, &before, &token),
            Err(SpaceError::Cancelled)
        ));
    }
}
