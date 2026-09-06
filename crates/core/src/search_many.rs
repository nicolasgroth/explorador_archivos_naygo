// Naygo — búsqueda multiraíz incremental, sin índice residente ni recorridos al cargar criterios.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use crate::saved_search::{path_key, SavedSearch};
use crate::search::{matches_query, SearchMsg, MAX_CONTENT_FILE_BYTES, MAX_HITS};
use crate::CancellationToken;
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant, UNIX_EPOCH};

pub const MAX_DIRECTORIES: usize = 100_000;
#[cfg(test)]
#[path = "search_many_tests.rs"]
mod tests;

pub fn spawn(
    query: SavedSearch,
    now: i64,
    offset: i64,
    token: CancellationToken,
) -> Receiver<SearchMsg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        run(
            &query,
            now,
            query.modified.lower_bound(now, offset),
            &token,
            &tx,
        )
    });
    rx
}

/// El límite UTC se resuelve fuera de core, con la zona local vigente en esa fecha.
pub fn run(
    query: &SavedSearch,
    now: i64,
    lower: Option<i64>,
    token: &CancellationToken,
    tx: &Sender<SearchMsg>,
) {
    visit(query, now, lower, token, |msg| tx.send(msg).is_ok());
}

/// Mismo recorrido, con consumidor síncrono: permite componer planes sin acumular canales.
pub fn visit(
    query: &SavedSearch,
    now: i64,
    lower: Option<i64>,
    token: &CancellationToken,
    mut emit: impl FnMut(SearchMsg) -> bool,
) {
    let mut unreadable = usize::from(query.validate().is_err());
    let mut skipped = 0;
    let mut dirs = 0;
    let mut cap = false;
    let mut hits = HashSet::new();
    let mut visited = HashSet::new();
    let mut last = Instant::now();
    let mut stack = if unreadable == 0 {
        // Conservar raíces hijas explícitas: pueden ser junctions o ser accesibles
        // aunque no se pueda listar el padre. Deduplicar al recorrer, no presumir cobertura.
        query.roots.clone()
    } else {
        vec![]
    };
    stack.reverse();
    'walk: while let Some(dir) = stack.pop() {
        if token.is_cancelled() {
            break;
        }
        if visited.contains(&path_key(&dir)) {
            continue;
        }
        if dirs >= MAX_DIRECTORIES {
            cap = true;
            break;
        }
        dirs += 1;
        visited.insert(path_key(&dir));
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        for item in entries {
            if token.is_cancelled() {
                break 'walk;
            }
            if last.elapsed() >= Duration::from_millis(150) {
                if !emit(SearchMsg::Progress { dirs_scanned: dirs }) {
                    return;
                }
                last = Instant::now();
            }
            let item = match item {
                Ok(e) => e,
                Err(_) => {
                    unreadable += 1;
                    continue;
                }
            };
            let path = item.path();
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => {
                    unreadable += 1;
                    continue;
                }
            };
            // Nunca seguir symlinks/junctions descubiertos durante el recorrido.
            let linked = meta.file_type().is_symlink() || reparse(&meta);
            if meta.is_dir() && query.options.recursive && !linked {
                if stack.len() + dirs >= MAX_DIRECTORIES {
                    cap = true;
                } else {
                    stack.push(path.clone());
                }
            }
            if !matches_query(
                &item.file_name().to_string_lossy(),
                &query.options.name_query,
                query.options.ignore_case,
                query.options.use_wildcards,
            ) {
                continue;
            }
            if (query.min_bytes.is_some() || query.max_bytes.is_some())
                && (!meta.is_file()
                    || query.min_bytes.is_some_and(|n| meta.len() < n)
                    || query.max_bytes.is_some_and(|n| meta.len() > n))
            {
                continue;
            }
            if let Some(lower) = lower {
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
                let Some(modified) = modified else {
                    unreadable += 1;
                    continue;
                };
                if modified < lower || modified > now {
                    continue;
                }
            }
            if !query.options.content_query.is_empty() {
                if !meta.is_file() {
                    continue;
                }
                if linked {
                    skipped += 1;
                    continue;
                }
                match contains(
                    &path,
                    &query.options.content_query,
                    query.options.ignore_case,
                    meta.len(),
                    token,
                ) {
                    Some(true) => {}
                    Some(false) => continue,
                    None => {
                        skipped += 1;
                        continue;
                    }
                }
            }
            if hits.insert(path_key(&path)) {
                if !emit(SearchMsg::Hit(crate::listing::entry_from_path(
                    &path,
                    Some(&meta),
                ))) {
                    return;
                }
                if hits.len() >= MAX_HITS {
                    break 'walk;
                }
            }
        }
    }
    emit(SearchMsg::Progress { dirs_scanned: dirs });
    emit(SearchMsg::Coverage {
        unreadable,
        content_skipped: skipped,
        directory_cap: cap,
    });
    emit(if token.is_cancelled() {
        SearchMsg::Cancelled
    } else {
        SearchMsg::Done {
            partial: unreadable > 0 || skipped > 0 || cap,
            hit_cap: hits.len() >= MAX_HITS,
        }
    });
}

pub(crate) fn contains(
    path: &Path,
    needle: &str,
    ignore_case: bool,
    size: u64,
    token: &CancellationToken,
) -> Option<bool> {
    if size > MAX_CONTENT_FILE_BYTES {
        return None;
    }
    let mut file = std::fs::File::open(path)
        .ok()?
        .take(MAX_CONTENT_FILE_BYTES + 1);
    let mut bytes = Vec::new();
    let mut block = [0u8; 64 * 1024];
    loop {
        if token.is_cancelled() {
            return None;
        }
        let n = file.read(&mut block).ok()?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&block[..n]);
        if bytes.len() as u64 > MAX_CONTENT_FILE_BYTES {
            return None;
        }
    }
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return None;
    }
    let text = std::str::from_utf8(&bytes).ok()?;
    Some(if ignore_case {
        text.to_lowercase().contains(&needle.to_lowercase())
    } else {
        text.contains(needle)
    })
}

#[cfg(windows)]
pub(crate) fn reparse(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
pub(crate) fn reparse(_: &std::fs::Metadata) -> bool {
    false
}
