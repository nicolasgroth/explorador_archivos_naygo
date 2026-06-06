// Naygo — motor de listado por streaming incremental, cancelable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lee un directorio en un hilo worker y emite cada entrada por un canal a
//! medida que la descubre, sin acumular todo antes de responder. Chequea el
//! `CancellationToken` entre cada entrada: cancelar corta el listado al instante.
//! El hilo de UI nunca llama a estas funciones directamente sobre el disco; usa
//! `spawn_listing`, que devuelve el receptor del canal.

use crate::cancel::CancellationToken;
use crate::fs_model::{Entry, EntryKind};
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

/// Lanza el listado de `dir` en un hilo worker. Devuelve el receptor del canal
/// (la UI lo drena frame a frame) y el `JoinHandle` (por si se quiere unir).
pub fn spawn_listing(
    dir: PathBuf,
    token: CancellationToken,
) -> (Receiver<ListingMsg>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        list_into(&dir, &token, &tx);
    });
    (rx, handle)
}

/// Cuerpo del worker: recorre el directorio emitiendo por `tx`. Extraído para
/// poder testearlo de forma síncrona sin spawnear un hilo.
fn list_into(dir: &Path, token: &CancellationToken, tx: &mpsc::Sender<ListingMsg>) {
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

/// Construye un `Entry` a partir de un `DirEntry`, tolerando metadata ausente.
fn entry_from_dirent(dirent: &std::fs::DirEntry) -> Entry {
    let path = dirent.path();
    let name = dirent.file_name().to_string_lossy().into_owned();
    let metadata = dirent.metadata().ok();

    let kind = match &metadata {
        Some(m) if m.is_dir() => EntryKind::Directory,
        Some(m) if m.is_file() => EntryKind::File,
        Some(_) => EntryKind::Other,
        None => EntryKind::Other,
    };

    let size = match (&metadata, kind) {
        (Some(m), EntryKind::File) => Some(m.len()),
        _ => None,
    };

    let modified = metadata.as_ref().and_then(|m| m.modified().ok());

    Entry {
        name,
        path,
        kind,
        size,
        modified,
        created: None,
        hidden: false,
    }
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
