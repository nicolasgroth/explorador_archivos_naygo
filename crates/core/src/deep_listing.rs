// Naygo — listado profundo: recorrido recursivo cancelable de TODO el árbol, con streaming.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lista todo el contenido bajo una carpeta raíz, recorriendo subcarpetas a cualquier
//! profundidad. Gemelo del recorrido de `search`, pero SIN filtro de nombre (emite cada
//! entrada) y SIN tope (`MAX_HITS`): el control de tamaño es el streaming + la cancelación.
//! Usa una pila propia (no recursión de stack), NO desciende a symlinks/junctions (evita
//! loops), y marca el resultado como "parcial" si alguna carpeta fue ilegible. Cada entrada
//! se emite con su PROFUNDIDAD (0 = hijo directo de la raíz) y su RUTA RELATIVA a la raíz,
//! para que la UI pueda sangrar y mostrar el origen. La consume el modo "vista profunda".

use crate::cancel::CancellationToken;
use crate::fs_model::Entry;
use crate::listing::entry_from_path;
use crate::search::{fs_lister, ListResult};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Throttle del mensaje `Progress` (cuántas carpetas se llevan recorridas).
const PROGRESS_THROTTLE: Duration = Duration::from_millis(150);

/// Una entrada del recorrido profundo: la entrada normal + su lugar en el árbol.
#[derive(Debug, Clone, PartialEq)]
pub struct DeepEntry {
    /// La entrada en sí (path absoluto, nombre, tipo, tamaño, fecha…).
    pub entry: Entry,
    /// Profundidad relativa a la raíz: 0 = hijo directo de la raíz, 1 = nieto, etc.
    pub depth: u32,
    /// Ruta relativa a la raíz, con separadores del SO (p. ej. "2025\\enero\\informe.pdf").
    pub rel_path: String,
}

/// Mensajes del worker de listado profundo hacia la UI. Espeja a `SearchMsg` pero sin
/// `hit_cap` (no hay tope): emite TODO hasta agotar el árbol o ser cancelado.
#[derive(Debug, Clone, PartialEq)]
pub enum DeepMsg {
    Entry(DeepEntry),
    Progress { dirs_scanned: usize },
    Done { partial: bool },
    Cancelled,
}

/// Lanza el listado profundo bajo `root` en un worker. La UI drena el receptor frame a
/// frame. Cancelable vía `token`.
pub fn spawn_deep_listing(
    root: PathBuf,
    token: CancellationToken,
) -> (Receiver<DeepMsg>, JoinHandle<()>) {
    let (tx, rx) = channel();
    let handle = thread::spawn(move || {
        deep_walk(&root, &fs_lister, &token, &tx);
    });
    (rx, handle)
}

/// Núcleo PURO: recorre el árbol bajo `root` con una pila de `(carpeta, profundidad)`,
/// emitiendo por `tx` CADA entrada (no filtra) con su `depth` y `rel_path`. `lister` produce
/// las entradas de un directorio (en producción lee el FS; en tests, un closure). Chequea
/// `token` entre directorios. NO desciende a symlinks/junctions. Sin tope.
fn deep_walk(
    root: &Path,
    lister: &dyn Fn(&Path) -> ListResult,
    token: &CancellationToken,
    tx: &Sender<DeepMsg>,
) {
    let mut partial = false;
    let mut dirs_scanned = 0usize;
    let mut last_progress = Instant::now();
    // La pila lleva (carpeta a listar, profundidad de SUS HIJOS).
    let mut stack: Vec<(PathBuf, u32)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if token.is_cancelled() {
            let _ = tx.send(DeepMsg::Cancelled);
            return;
        }
        match lister(&dir) {
            None => partial = true,
            Some(entries) => {
                dirs_scanned += 1;
                for e in entries {
                    let rel_path = e
                        .path
                        .strip_prefix(root)
                        .unwrap_or(&e.path)
                        .to_string_lossy()
                        .into_owned();
                    let entry = make_entry(&e.path, e.is_dir);
                    if tx
                        .send(DeepMsg::Entry(DeepEntry {
                            entry,
                            depth,
                            rel_path,
                        }))
                        .is_err()
                    {
                        // El receptor se cayó (la UI cerró el modo): dejar de trabajar.
                        return;
                    }
                    // Descender a subcarpetas (jamás a symlinks/junctions: evita loops).
                    if e.is_dir && !e.is_symlink {
                        stack.push((e.path, depth + 1));
                    }
                }
                if last_progress.elapsed() >= PROGRESS_THROTTLE {
                    let _ = tx.send(DeepMsg::Progress { dirs_scanned });
                    last_progress = Instant::now();
                }
            }
        }
    }

    if token.is_cancelled() {
        let _ = tx.send(DeepMsg::Cancelled);
    } else {
        let _ = tx.send(DeepMsg::Done { partial });
    }
}

/// Construye el `Entry` de una entrada leyendo su metadata real (tamaño/fechas). Si la
/// metadata no se puede leer, cae a un `Entry` mínimo con el tipo ya conocido del walk.
fn make_entry(path: &Path, is_dir: bool) -> Entry {
    match std::fs::metadata(path) {
        Ok(m) => entry_from_path(path, Some(&m)),
        Err(_) => Entry {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: path.to_path_buf(),
            kind: if is_dir {
                crate::fs_model::EntryKind::Directory
            } else {
                crate::fs_model::EntryKind::File
            },
            size: None,
            modified: None,
            created: None,
            hidden: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::DirEntryRaw;
    use std::collections::HashMap;
    use std::sync::mpsc::channel;

    fn raw(path: &str, is_dir: bool) -> DirEntryRaw {
        DirEntryRaw {
            path: PathBuf::from(path),
            is_dir,
            is_symlink: false,
        }
    }

    fn collect(root: &str, map: HashMap<PathBuf, Vec<DirEntryRaw>>) -> Vec<DeepEntry> {
        let lister = move |dir: &Path| map.get(dir).cloned();
        let (tx, rx) = channel();
        let token = CancellationToken::new();
        deep_walk(Path::new(root), &lister, &token, &tx);
        let mut out = Vec::new();
        for m in rx.iter() {
            match m {
                DeepMsg::Entry(e) => out.push(e),
                DeepMsg::Done { .. } | DeepMsg::Cancelled => break,
                DeepMsg::Progress { .. } => {}
            }
        }
        out
    }

    fn rel_norm(e: &DeepEntry) -> String {
        e.rel_path.replace('\\', "/")
    }

    #[test]
    fn emite_todo_el_arbol_con_depth_y_rel_path() {
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        map.insert(
            PathBuf::from("/root"),
            vec![raw("/root/a.txt", false), raw("/root/sub", true)],
        );
        map.insert(
            PathBuf::from("/root/sub"),
            vec![raw("/root/sub/b.txt", false), raw("/root/sub/deep", true)],
        );
        map.insert(
            PathBuf::from("/root/sub/deep"),
            vec![raw("/root/sub/deep/c.txt", false)],
        );
        let items = collect("/root", map);
        assert_eq!(items.len(), 5);
        let a = items.iter().find(|e| rel_norm(e) == "a.txt").unwrap();
        assert_eq!(a.depth, 0);
        let sub = items.iter().find(|e| rel_norm(e) == "sub").unwrap();
        assert_eq!(sub.depth, 0);
        let deep = items.iter().find(|e| rel_norm(e) == "sub/deep").unwrap();
        assert_eq!(deep.depth, 1);
        let c = items
            .iter()
            .find(|e| rel_norm(e) == "sub/deep/c.txt")
            .unwrap();
        assert_eq!(c.depth, 2);
    }

    #[test]
    fn no_desciende_a_symlinks() {
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        let mut link = raw("/root/link", true);
        link.is_symlink = true;
        map.insert(PathBuf::from("/root"), vec![link]);
        map.insert(
            PathBuf::from("/root/link"),
            vec![raw("/root/link/inside.txt", false)],
        );
        let items = collect("/root", map);
        assert_eq!(items.len(), 1);
        assert!(items.iter().all(|e| !rel_norm(e).contains("inside")));
    }

    #[test]
    fn carpeta_ilegible_no_aborta_y_marca_partial() {
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        map.insert(
            PathBuf::from("/root"),
            vec![raw("/root/ok.txt", false), raw("/root/bad", true)],
        );
        // "/root/bad" NO está en el mapa => lister da None => partial.
        let lister = move |dir: &Path| map.get(dir).cloned();
        let (tx, rx) = channel();
        let token = CancellationToken::new();
        deep_walk(Path::new("/root"), &lister, &token, &tx);
        let mut partial = None;
        let mut count = 0;
        for m in rx.iter() {
            match m {
                DeepMsg::Entry(_) => count += 1,
                DeepMsg::Done { partial: p } => {
                    partial = Some(p);
                    break;
                }
                DeepMsg::Cancelled => break,
                DeepMsg::Progress { .. } => {}
            }
        }
        assert_eq!(count, 2);
        assert_eq!(partial, Some(true));
    }

    #[test]
    fn token_cancelado_emite_cancelled() {
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        map.insert(PathBuf::from("/root"), vec![raw("/root/a.txt", false)]);
        let lister = move |dir: &Path| map.get(dir).cloned();
        let (tx, rx) = channel();
        let token = CancellationToken::new();
        token.cancel();
        deep_walk(Path::new("/root"), &lister, &token, &tx);
        assert_eq!(rx.recv().unwrap(), DeepMsg::Cancelled);
    }

    #[test]
    fn arbol_vacio_no_emite_entradas() {
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        map.insert(PathBuf::from("/root"), vec![]);
        let items = collect("/root", map);
        assert!(items.is_empty());
    }

    #[test]
    fn sin_tope_emite_mas_de_5000() {
        let big: Vec<DirEntryRaw> = (0..6000)
            .map(|i| raw(Box::leak(format!("/root/f{i}.txt").into_boxed_str()), false))
            .collect();
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        map.insert(PathBuf::from("/root"), big);
        let items = collect("/root", map);
        assert_eq!(items.len(), 6000);
    }
}
