// Naygo — motor de listado por streaming incremental, cancelable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lee un directorio en un hilo worker y emite cada entrada por un canal a
//! medida que la descubre, sin acumular todo antes de responder. Chequea el
//! `CancellationToken` entre cada entrada: cancelar corta el listado al instante.
//! El hilo de UI nunca llama a estas funciones directamente sobre el disco; usa
//! `spawn_listing`, que devuelve el receptor del canal.

use crate::cancel::CancellationToken;
use crate::fs_model::{Entry, EntryKind};
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

/// Mensajes que el worker de listado emite hacia la UI.
#[derive(Debug)]
pub enum ListingMsg {
    /// Una entrada recién descubierta.
    Entry(Entry),
    /// El directorio no se pudo abrir (permiso, ruta inexistente, disco caído).
    Error(String),
    /// El listado terminó de forma natural (recorrió todo).
    Done,
    /// El listado se abortó porque el token fue cancelado.
    Cancelled,
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
pub fn spawn_listing(
    dir: PathBuf,
    token: CancellationToken,
) -> (Receiver<ListingMsg>, JoinHandle<()>) {
    spawn_listing_filtered(dir, token, ListingFilter::All)
}

/// Lanza el listado de `dir` con un filtro. Igual que `spawn_listing` pero pudiendo
/// emitir solo directorios (para el árbol).
pub fn spawn_listing_filtered(
    dir: PathBuf,
    token: CancellationToken,
    filter: ListingFilter,
) -> (Receiver<ListingMsg>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        list_into_filtered(&dir, &token, &tx, filter);
    });
    (rx, handle)
}

/// Cuerpo del worker: recorre el directorio emitiendo por `tx`, aplicando `filter`.
/// Extraído para testearlo síncrono sin spawnear un hilo.
fn list_into_filtered(
    dir: &Path,
    token: &CancellationToken,
    tx: &mpsc::Sender<ListingMsg>,
    filter: ListingFilter,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            let _ = tx.send(ListingMsg::Error(e.to_string()));
            return;
        }
    };

    for dirent in read_dir {
        if token.is_cancelled() {
            let _ = tx.send(ListingMsg::Cancelled);
            return;
        }

        let dirent = match dirent {
            Ok(d) => d,
            // Una entrada ilegible no aborta todo el listado: se salta.
            Err(_) => continue,
        };

        let entry = entry_from_dirent(&dirent);
        // Con DirsOnly (árbol) se omiten archivos y otros tipos no-directorio.
        if filter == ListingFilter::DirsOnly && entry.kind != EntryKind::Directory {
            continue;
        }
        // Si el receptor se cayó (la UI cambió de carpeta), dejar de trabajar.
        if tx.send(ListingMsg::Entry(entry)).is_err() {
            return;
        }
    }

    if token.is_cancelled() {
        let _ = tx.send(ListingMsg::Cancelled);
    } else {
        let _ = tx.send(ListingMsg::Done);
    }
}

/// Wrapper de compatibilidad para tests: lista todo.
#[cfg(test)]
fn list_into(dir: &Path, token: &CancellationToken, tx: &mpsc::Sender<ListingMsg>) {
    list_into_filtered(dir, token, tx, ListingFilter::All);
}

/// Construye un `Entry` desde una ruta + su metadata (ya leída). Tolerante a metadata
/// ausente. Compartido por el listado inicial y por el merge incremental del watcher.
pub fn entry_from_path(path: &Path, metadata: Option<&Metadata>) -> Entry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
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

    Entry {
        name,
        path: path.to_path_buf(),
        kind,
        size,
        modified,
        created,
        hidden: false,
    }
}

/// Construye un `Entry` a partir de un `DirEntry`, tolerando metadata ausente.
fn entry_from_dirent(dirent: &std::fs::DirEntry) -> Entry {
    let path = dirent.path();
    let metadata = dirent.metadata().ok();
    entry_from_path(&path, metadata.as_ref())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
                ListingMsg::Done => done = true,
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
            msgs.iter().any(|m| matches!(m, ListingMsg::Cancelled)),
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
        list_into_filtered(dir.path(), &token, &tx, ListingFilter::DirsOnly);
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
        list_into_filtered(dir.path(), &token, &tx, ListingFilter::All);
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
            matches!(msgs.first(), Some(ListingMsg::Error(_))),
            "debe emitir Error, got {:?}",
            msgs
        );
    }
}
