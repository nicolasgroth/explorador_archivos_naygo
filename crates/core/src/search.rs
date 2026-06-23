// Naygo — búsqueda recursiva por nombre: recorrido cancelable con streaming de resultados.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Busca archivos y carpetas cuyo NOMBRE coincide con una consulta, recorriendo la
//! carpeta raíz y todas sus subcarpetas. Igual que `sizing`, usa una pila propia (sin
//! recursión de stack, soporta árboles profundos), NO sigue symlinks/junctions de
//! Windows (evita loops), y marca el resultado como "parcial" si alguna carpeta fue
//! ilegible. Igual que `listing`, emite cada coincidencia por un canal a medida que la
//! descubre (streaming incremental): la UI las muestra en vivo sin esperar al final.
//! Cada búsqueda recibe un `CancellationToken` y aborta limpio en cuanto se cancela.
//!
//! El emparejado de nombres (`matches_query`) es PURO y testeable: insensible a
//! mayúsculas, con comodín `*` (cualquier secuencia). Una consulta vacía no coincide
//! con nada (evita devolver el árbol entero).

use crate::cancel::CancellationToken;
use crate::fs_model::{Entry, EntryKind};
use crate::listing::entry_from_path;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Throttle del mensaje `Progress` (cuántas carpetas se llevan recorridas).
const PROGRESS_THROTTLE: Duration = Duration::from_millis(150);

/// Tope de coincidencias emitidas antes de cortar (evita inundar la UI con árboles
/// gigantes). Al alcanzarlo, el worker termina con `Done { hit_cap: true }`.
pub const MAX_HITS: usize = 5000;

/// Mensajes del worker de búsqueda hacia la UI.
/// (No deriva `Eq` porque `Entry` contiene tiempos del sistema; `PartialEq` basta para tests.)
#[derive(Debug, Clone, PartialEq)]
pub enum SearchMsg {
    /// Una coincidencia recién descubierta (archivo o carpeta).
    Hit(Entry),
    /// Avance: cuántas carpetas se han recorrido hasta ahora (throttled).
    Progress { dirs_scanned: usize },
    /// Terminó de forma natural. `partial` = hubo carpetas ilegibles (permiso/desaparición);
    /// `hit_cap` = se cortó por alcanzar `MAX_HITS`.
    Done { partial: bool, hit_cap: bool },
    /// Se abortó porque el token fue cancelado.
    Cancelled,
}

/// ¿El `name` coincide con `query`? Insensible a mayúsculas. Si `query` contiene `*`,
/// se interpreta como comodín (cualquier secuencia, incluso vacía) y debe calzar el
/// nombre COMPLETO; si no, se busca como subcadena en cualquier posición. Una `query`
/// vacía (tras quitar espacios) no coincide con nada.
pub fn matches_query(name: &str, query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return false;
    }
    let name_l = name.to_lowercase();
    let query_l = q.to_lowercase();
    if query_l.contains('*') {
        wildcard_match(&name_l, &query_l)
    } else {
        name_l.contains(&query_l)
    }
}

/// Empareja `name` contra un patrón con comodines `*` (cada `*` = cualquier secuencia).
/// Debe calzar el nombre completo. Ambos argumentos ya vienen en minúsculas. Algoritmo
/// greedy clásico con backtracking sobre el último `*`, lineal en la práctica.
fn wildcard_match(name: &str, pattern: &str) -> bool {
    let n: Vec<char> = name.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let (mut i, mut j) = (0usize, 0usize); // índices en name y pattern
    let (mut star, mut mark) = (None, 0usize); // posición del último '*' y match candidato
    while i < n.len() {
        if j < p.len() && p[j] == '*' {
            star = Some(j);
            mark = i;
            j += 1;
        } else if j < p.len() && p[j] == n[i] {
            i += 1;
            j += 1;
        } else if let Some(s) = star {
            // Retroceder: el '*' absorbe un carácter más.
            j = s + 1;
            mark += 1;
            i = mark;
        } else {
            return false;
        }
    }
    // Consumir '*' sobrantes al final del patrón.
    while j < p.len() && p[j] == '*' {
        j += 1;
    }
    j == p.len()
}

/// Una entrada cruda de un directorio para la búsqueda: ruta + tipo + si es symlink.
/// (Reusa la idea de `sizing::WalkEntry` pero local a búsqueda para no acoplar módulos.)
#[derive(Clone)]
pub(crate) struct DirEntryRaw {
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
    pub(crate) is_symlink: bool,
}

/// Resultado de listar un directorio para la búsqueda: las entradas, o `None` si no se
/// pudo leer (permiso/desaparición) → cuenta como "parcial".
pub(crate) type ListResult = Option<Vec<DirEntryRaw>>;

/// Lanza la búsqueda de `query` bajo `root` (recursiva) en un worker. Devuelve el receptor
/// del canal (la UI lo drena frame a frame) y el `JoinHandle`. Cancelable vía `token`.
pub fn spawn_search(
    root: PathBuf,
    query: String,
    token: CancellationToken,
) -> (Receiver<SearchMsg>, JoinHandle<()>) {
    let (tx, rx) = channel();
    let handle = thread::spawn(move || {
        search_walk(&root, &query, &fs_lister, &token, &tx);
    });
    (rx, handle)
}

/// Núcleo PURO de la búsqueda: recorre el árbol bajo `root` con una pila propia, emitiendo
/// por `tx` cada entrada cuyo nombre cumpla `matches_query`. `lister` produce las entradas
/// de un directorio (en producción lee el FS; en tests, un closure). Chequea `token` entre
/// directorios. NO desciende a symlinks/junctions. Corta al llegar a `MAX_HITS`.
fn search_walk(
    root: &Path,
    query: &str,
    lister: &dyn Fn(&Path) -> ListResult,
    token: &CancellationToken,
    tx: &Sender<SearchMsg>,
) {
    let mut partial = false;
    let mut hits = 0usize;
    let mut dirs_scanned = 0usize;
    let mut last_progress = Instant::now();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if token.is_cancelled() {
            let _ = tx.send(SearchMsg::Cancelled);
            return;
        }
        match lister(&dir) {
            None => partial = true,
            Some(entries) => {
                dirs_scanned += 1;
                for e in entries {
                    // Coincidencia por nombre del archivo/carpeta (no de la ruta completa).
                    let name = e
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    if matches_query(&name, query) {
                        let entry = make_entry(&e.path, e.is_dir);
                        // Si el receptor se cayó (la UI cerró la búsqueda), dejar de trabajar.
                        if tx.send(SearchMsg::Hit(entry)).is_err() {
                            return;
                        }
                        hits += 1;
                        if hits >= MAX_HITS {
                            let _ = tx.send(SearchMsg::Done {
                                partial,
                                hit_cap: true,
                            });
                            return;
                        }
                    }
                    // Descender a subcarpetas (jamás a symlinks/junctions: evita loops).
                    if e.is_dir && !e.is_symlink {
                        stack.push(e.path);
                    }
                }
                if last_progress.elapsed() >= PROGRESS_THROTTLE {
                    let _ = tx.send(SearchMsg::Progress { dirs_scanned });
                    last_progress = Instant::now();
                }
            }
        }
    }

    if token.is_cancelled() {
        let _ = tx.send(SearchMsg::Cancelled);
    } else {
        let _ = tx.send(SearchMsg::Done {
            partial,
            hit_cap: false,
        });
    }
}

/// Construye el `Entry` de una coincidencia leyendo su metadata real (tamaño/fechas). Si la
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
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            size: None,
            modified: None,
            created: None,
            hidden: false,
            system: false,
        },
    }
}

/// ¿La metadata corresponde a un reparse point de Windows (symlink O junction)? Igual criterio
/// que `sizing`: en Windows miramos `FILE_ATTRIBUTE_REPARSE_POINT`; en otras plataformas
/// `is_symlink()` ya cubre los enlaces.
#[cfg(windows)]
fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &std::fs::Metadata) -> bool {
    false
}

/// Lista un directorio del FS real para la búsqueda. `None` si no se puede leer (parcial).
/// Usa `symlink_metadata` (no sigue enlaces) y marca symlinks/junctions como no-descendibles.
pub(crate) fn fs_lister(dir: &Path) -> ListResult {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        let (is_dir, is_symlink) = match std::fs::symlink_metadata(&path) {
            Ok(m) => (
                m.is_dir(),
                m.file_type().is_symlink() || is_reparse_point(&m),
            ),
            // Entrada ilegible: la representamos como archivo no-symlink (no descenderá y se
            // intentará emparejar su nombre igual; su metadata se resolverá en make_entry).
            Err(_) => (false, false),
        };
        out.push(DirEntryRaw {
            path,
            is_dir,
            is_symlink,
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // --- matches_query (puro) ---

    #[test]
    fn subcadena_insensible_a_mayusculas() {
        assert!(matches_query("Informe_Final.PDF", "final"));
        assert!(matches_query("Informe_Final.PDF", "INFORME"));
        assert!(matches_query("a.txt", ".txt"));
        assert!(!matches_query("a.txt", "xml"));
    }

    #[test]
    fn consulta_vacia_no_coincide() {
        assert!(!matches_query("cualquier.cosa", ""));
        assert!(!matches_query("cualquier.cosa", "   "));
    }

    #[test]
    fn comodin_calza_nombre_completo() {
        assert!(matches_query("foto_2026.jpg", "*.jpg"));
        assert!(matches_query("foto_2026.jpg", "foto*"));
        assert!(matches_query("foto_2026.jpg", "*2026*"));
        assert!(matches_query("foto_2026.jpg", "foto*jpg"));
        // Con comodín se exige calzar TODO el nombre: ".jpg" sin '*' delante no calza.
        assert!(!matches_query("foto_2026.jpg", "*.png"));
        assert!(!matches_query("foto_2026.jpg", "bar*"));
    }

    #[test]
    fn comodin_solo_estrella_calza_todo() {
        assert!(matches_query("loquesea", "*"));
        assert!(matches_query("a.b.c", "*.*"));
    }

    // --- search_walk (con lister falso) ---

    fn raw(p: &str, is_dir: bool) -> DirEntryRaw {
        DirEntryRaw {
            path: PathBuf::from(p),
            is_dir,
            is_symlink: false,
        }
    }

    fn lister_from(
        map: HashMap<PathBuf, Vec<(&'static str, bool)>>,
    ) -> impl Fn(&Path) -> ListResult {
        move |p: &Path| {
            map.get(p)
                .map(|v| v.iter().map(|(path, is_dir)| raw(path, *is_dir)).collect())
        }
    }

    fn collect(rx: Receiver<SearchMsg>) -> Vec<SearchMsg> {
        rx.into_iter().collect()
    }

    #[test]
    fn encuentra_en_subcarpetas() {
        let mut map: HashMap<PathBuf, Vec<(&str, bool)>> = HashMap::new();
        map.insert(
            PathBuf::from("/root"),
            vec![("/root/a.txt", false), ("/root/sub", true)],
        );
        map.insert(
            PathBuf::from("/root/sub"),
            vec![
                ("/root/sub/objetivo.txt", false),
                ("/root/sub/b.log", false),
            ],
        );
        let lister = lister_from(map);
        let token = CancellationToken::new();
        let (tx, rx) = channel();
        search_walk(Path::new("/root"), "objetivo", &lister, &token, &tx);
        drop(tx);

        let msgs = collect(rx);
        let hits: Vec<&str> = msgs
            .iter()
            .filter_map(|m| match m {
                SearchMsg::Hit(e) => Some(e.name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(hits, vec!["objetivo.txt"]);
        assert!(msgs.iter().any(|m| matches!(
            m,
            SearchMsg::Done {
                partial: false,
                hit_cap: false
            }
        )));
    }

    #[test]
    fn coincide_tambien_con_carpetas() {
        let mut map: HashMap<PathBuf, Vec<(&str, bool)>> = HashMap::new();
        map.insert(
            PathBuf::from("/root"),
            vec![("/root/proyecto", true), ("/root/otro.txt", false)],
        );
        map.insert(PathBuf::from("/root/proyecto"), vec![]);
        let lister = lister_from(map);
        let token = CancellationToken::new();
        let (tx, rx) = channel();
        search_walk(Path::new("/root"), "proyecto", &lister, &token, &tx);
        drop(tx);

        let hits: Vec<String> = collect(rx)
            .into_iter()
            .filter_map(|m| match m {
                SearchMsg::Hit(e) => Some(e.name),
                _ => None,
            })
            .collect();
        assert_eq!(hits, vec!["proyecto"]);
    }

    #[test]
    fn carpeta_ilegible_marca_parcial() {
        let mut map: HashMap<PathBuf, Vec<(&str, bool)>> = HashMap::new();
        // /root tiene una subcarpeta que el lister NO conoce (devuelve None → ilegible).
        map.insert(PathBuf::from("/root"), vec![("/root/secreta", true)]);
        let lister = lister_from(map);
        let token = CancellationToken::new();
        let (tx, rx) = channel();
        search_walk(Path::new("/root"), "x", &lister, &token, &tx);
        drop(tx);

        assert!(collect(rx)
            .iter()
            .any(|m| matches!(m, SearchMsg::Done { partial: true, .. })));
    }

    #[test]
    fn token_cancelado_corta_y_emite_cancelled() {
        let mut map: HashMap<PathBuf, Vec<(&str, bool)>> = HashMap::new();
        map.insert(PathBuf::from("/root"), vec![("/root/a.txt", false)]);
        let lister = lister_from(map);
        let token = CancellationToken::new();
        token.cancel();
        let (tx, rx) = channel();
        search_walk(Path::new("/root"), "a", &lister, &token, &tx);
        drop(tx);

        let msgs = collect(rx);
        assert!(msgs.iter().any(|m| matches!(m, SearchMsg::Cancelled)));
        assert!(!msgs.iter().any(|m| matches!(m, SearchMsg::Hit(_))));
    }

    #[test]
    fn no_desciende_a_symlinks() {
        // /root tiene un symlink que apunta a una subcarpeta con un objetivo: NO debe entrar.
        let mut map: HashMap<PathBuf, Vec<(&str, bool)>> = HashMap::new();
        map.insert(PathBuf::from("/root"), vec![("/root/link", true)]);
        map.insert(
            PathBuf::from("/root/link"),
            vec![("/root/link/objetivo.txt", false)],
        );
        let lister = move |p: &Path| -> ListResult {
            if p == Path::new("/root") {
                Some(vec![DirEntryRaw {
                    path: PathBuf::from("/root/link"),
                    is_dir: true,
                    is_symlink: true, // marcado como enlace
                }])
            } else {
                map.get(p)
                    .map(|v| v.iter().map(|(p, d)| raw(p, *d)).collect())
            }
        };
        let token = CancellationToken::new();
        let (tx, rx) = channel();
        search_walk(Path::new("/root"), "objetivo", &lister, &token, &tx);
        drop(tx);

        assert!(!collect(rx).iter().any(|m| matches!(m, SearchMsg::Hit(_))));
    }
}
