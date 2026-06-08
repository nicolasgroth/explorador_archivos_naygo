// Naygo — tamaño de carpeta bajo demanda: recorrido recursivo cancelable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Calcula el tamaño total de una carpeta sumando sus archivos descendientes. El
//! recorrido es CANCELABLE (token) y usa una pila propia (no recursión de stack, para
//! árboles profundos). NO sigue symlinks/junctions (evita loops y doble conteo). Una
//! entrada ilegible se salta marcando el resultado como "parcial". El worker
//! (`spawn_dir_size`) corre fuera del hilo de UI; `dir_size_walk` es la lógica pura.

use crate::cancel::CancellationToken;
use std::path::PathBuf;

/// Mensaje del worker de cálculo de tamaño.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SizeMsg {
    /// Acumulado parcial (bytes) mientras avanza (throttled por el worker).
    Progress { bytes: u64 },
    /// Terminó. `total` bytes; `partial` = hubo entradas saltadas (permiso/error).
    Done { total: u64, partial: bool },
    /// Cancelado; `bytes` = lo acumulado hasta el corte.
    Cancelled { bytes: u64 },
}

/// Una entrada lista por el "lister": (ruta, es_dir, es_symlink, tamaño_si_archivo).
/// `size` es `Some` para archivos (bytes) y `None` para carpetas/inaccesibles.
pub struct WalkEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: Option<u64>,
}

/// Resultado de listar un directorio: las entradas, o `None` si no se pudo leer
/// (permiso denegado, desapareció) → cuenta como "parcial".
pub type ListResult = Option<Vec<WalkEntry>>;

/// Suma recursiva PURA del tamaño de `root`. `lister(&Path) -> ListResult` produce las
/// entradas de un directorio (en producción lee el FS; en tests, un closure). Si
/// `recursive`, baja a subcarpetas (salvo symlinks). Usa una pila propia. Chequea
/// `token` entre directorios. `on_progress(bytes)` se llama con el acumulado (el worker
/// le pone throttle; en tests puede no hacer nada). Devuelve `(total, partial, cancelled)`.
pub fn dir_size_walk(
    root: &std::path::Path,
    recursive: bool,
    lister: &dyn Fn(&std::path::Path) -> ListResult,
    token: &CancellationToken,
    on_progress: &mut dyn FnMut(u64),
) -> (u64, bool, bool) {
    let mut total: u64 = 0;
    let mut partial = false;
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if token.is_cancelled() {
            return (total, partial, true);
        }
        match lister(&dir) {
            None => {
                partial = true;
            }
            Some(entries) => {
                for e in entries {
                    if e.is_symlink {
                        continue;
                    }
                    if e.is_dir {
                        if recursive {
                            stack.push(e.path);
                        }
                    } else {
                        match e.size {
                            Some(b) => total = total.saturating_add(b),
                            None => partial = true,
                        }
                    }
                }
                on_progress(total);
            }
        }
    }
    (total, partial, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lister_from(map: &HashMap<PathBuf, Vec<WalkEntry>>) -> impl Fn(&std::path::Path) -> ListResult + '_ {
        move |p: &std::path::Path| {
            map.get(p).map(|v| {
                v.iter()
                    .map(|e| WalkEntry {
                        path: e.path.clone(),
                        is_dir: e.is_dir,
                        is_symlink: e.is_symlink,
                        size: e.size,
                    })
                    .collect()
            })
        }
    }

    fn file(p: &str, size: u64) -> WalkEntry {
        WalkEntry { path: PathBuf::from(p), is_dir: false, is_symlink: false, size: Some(size) }
    }
    fn dir(p: &str) -> WalkEntry {
        WalkEntry { path: PathBuf::from(p), is_dir: true, is_symlink: false, size: None }
    }

    #[test]
    fn suma_recursiva() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![file("root/a.txt", 10), dir("root/sub")]);
        m.insert(PathBuf::from("root/sub"), vec![file("root/sub/b.txt", 20), file("root/sub/c.txt", 5)]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, partial, cancelled) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert_eq!(total, 35);
        assert!(!partial && !cancelled);
    }

    #[test]
    fn no_recursivo_solo_primer_nivel() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![file("root/a.txt", 10), dir("root/sub")]);
        m.insert(PathBuf::from("root/sub"), vec![file("root/sub/b.txt", 20)]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, _, _) = dir_size_walk(std::path::Path::new("root"), false, &lister, &token, &mut prog);
        assert_eq!(total, 10);
    }

    #[test]
    fn no_sigue_symlink() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        let link = WalkEntry { path: PathBuf::from("root/link"), is_dir: true, is_symlink: true, size: None };
        m.insert(PathBuf::from("root"), vec![file("root/a.txt", 10), link]);
        m.insert(PathBuf::from("root/link"), vec![file("root/link/big.bin", 9999)]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, _, _) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert_eq!(total, 10);
    }

    #[test]
    fn subdir_ilegible_marca_parcial() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![file("root/a.txt", 10), dir("root/secret")]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, partial, _) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert_eq!(total, 10);
        assert!(partial);
    }

    #[test]
    fn carpeta_vacia_es_cero() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, partial, _) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert_eq!(total, 0);
        assert!(!partial);
    }

    #[test]
    fn token_cancelado_corta() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![dir("root/sub")]);
        m.insert(PathBuf::from("root/sub"), vec![file("root/sub/x", 100)]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        token.cancel();
        let mut prog = |_| {};
        let (_, _, cancelled) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert!(cancelled);
    }
}
