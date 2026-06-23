// Naygo — caché LRU de listados de carpetas (solo memoria, puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Caché en memoria de carpetas ya visitadas para navegación instantánea
//! (*stale-while-revalidate*): al volver a una carpeta cacheada la UI pinta las
//! entries guardadas EN EL MISMO FRAME y lanza igual el listado real, que corrige
//! cualquier diferencia al completar. Nada persiste a disco.
//!
//! Límites duros: nº de carpetas y nº TOTAL de entries (el que se exceda primero
//! desaloja por LRU). Una carpeta más grande que el tope total no se cachea.

use crate::fs_model::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Un listado cacheado (las entries finales de un listado COMPLETO y exitoso).
#[derive(Clone, Debug)]
pub struct CachedListing {
    pub entries: Vec<Entry>,
}

/// Caché LRU por carpeta. `get` marca como recién usada; `put` desaloja las menos
/// usadas hasta respetar ambos topes.
pub struct ListingCache {
    map: HashMap<PathBuf, CachedListing>,
    /// Orden de uso: el FRENTE es lo menos recientemente usado.
    lru: Vec<PathBuf>,
    max_dirs: usize,
    max_total_entries: usize,
    total_entries: usize,
}

impl ListingCache {
    /// `max_dirs == 0` desactiva el caché (todo `put` es no-op, todo `get` es miss).
    pub fn new(max_dirs: usize, max_total_entries: usize) -> Self {
        Self {
            map: HashMap::new(),
            lru: Vec::new(),
            max_dirs,
            max_total_entries,
            total_entries: 0,
        }
    }

    /// Listado cacheado de `dir`, marcándolo como recién usado.
    pub fn get(&mut self, dir: &Path) -> Option<&CachedListing> {
        if !self.map.contains_key(dir) {
            return None;
        }
        self.touch(dir);
        self.map.get(dir)
    }

    /// Guarda (o reemplaza) el listado de `dir` y desaloja LRU si excede topes.
    pub fn put(&mut self, dir: PathBuf, entries: Vec<Entry>) {
        if self.max_dirs == 0 || entries.len() > self.max_total_entries {
            return;
        }
        self.invalidate(&dir);
        self.total_entries += entries.len();
        self.map.insert(dir.clone(), CachedListing { entries });
        self.lru.push(dir);
        // Desalojar lo menos usado hasta respetar ambos topes.
        while self.map.len() > self.max_dirs || self.total_entries > self.max_total_entries {
            let oldest = self.lru.remove(0);
            if let Some(c) = self.map.remove(&oldest) {
                self.total_entries -= c.entries.len();
            }
        }
    }

    /// Olvida el listado de `dir` (si estaba).
    pub fn invalidate(&mut self, dir: &Path) {
        if let Some(c) = self.map.remove(dir) {
            self.total_entries -= c.entries.len();
            self.lru.retain(|p| p != dir);
        }
    }

    /// Vacía el caché por completo (cambio de configuración).
    pub fn clear(&mut self) {
        self.map.clear();
        self.lru.clear();
        self.total_entries = 0;
    }

    /// Mueve `dir` al final del orden LRU (lo más recientemente usado).
    fn touch(&mut self, dir: &Path) {
        if let Some(i) = self.lru.iter().position(|p| p == dir) {
            let p = self.lru.remove(i);
            self.lru.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_model::EntryKind;

    fn entries(n: usize) -> Vec<Entry> {
        (0..n)
            .map(|i| Entry {
                name: format!("f{i}"),
                path: PathBuf::from(format!("D:/x/f{i}")),
                kind: EntryKind::File,
                size: Some(1),
                modified: None,
                created: None,
                hidden: false,
                system: false,
            })
            .collect()
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn hit_y_miss() {
        let mut c = ListingCache::new(4, 100);
        assert!(c.get(&p("D:/a")).is_none());
        c.put(p("D:/a"), entries(3));
        assert_eq!(c.get(&p("D:/a")).unwrap().entries.len(), 3);
    }

    #[test]
    fn desaloja_lru_por_numero_de_carpetas() {
        let mut c = ListingCache::new(2, 100);
        c.put(p("D:/a"), entries(1));
        c.put(p("D:/b"), entries(1));
        // Tocar `a` la vuelve la más reciente → al exceder se va `b`.
        c.get(&p("D:/a"));
        c.put(p("D:/c"), entries(1));
        assert!(c.get(&p("D:/a")).is_some());
        assert!(c.get(&p("D:/b")).is_none());
        assert!(c.get(&p("D:/c")).is_some());
    }

    #[test]
    fn desaloja_por_tope_total_de_entries() {
        let mut c = ListingCache::new(10, 10);
        c.put(p("D:/a"), entries(6));
        c.put(p("D:/b"), entries(6)); // 12 > 10 → `a` se desaloja
        assert!(c.get(&p("D:/a")).is_none());
        assert!(c.get(&p("D:/b")).is_some());
    }

    #[test]
    fn carpeta_mas_grande_que_el_tope_no_se_cachea() {
        let mut c = ListingCache::new(10, 5);
        c.put(p("D:/a"), entries(6));
        assert!(c.get(&p("D:/a")).is_none());
    }

    #[test]
    fn put_reemplaza_e_invalidate_olvida() {
        let mut c = ListingCache::new(4, 100);
        c.put(p("D:/a"), entries(3));
        c.put(p("D:/a"), entries(5));
        assert_eq!(c.get(&p("D:/a")).unwrap().entries.len(), 5);
        c.invalidate(&p("D:/a"));
        assert!(c.get(&p("D:/a")).is_none());
        // El conteo total se descuenta bien: cabe otra carpeta de 5.
        c.put(p("D:/b"), entries(5));
        assert!(c.get(&p("D:/b")).is_some());
    }

    #[test]
    fn max_dirs_cero_desactiva() {
        let mut c = ListingCache::new(0, 100);
        c.put(p("D:/a"), entries(1));
        assert!(c.get(&p("D:/a")).is_none());
    }
}
