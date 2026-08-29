// Naygo — bandeja temporal de archivos de distintas carpetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Colección temporal, ordenada y deduplicada de rutas absolutas. No toca el filesystem al
//! agregar: una ruta puede desaparecer después y la UI la presenta como ausente.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectionBasket {
    items: Vec<PathBuf>,
    keys: HashSet<String>,
}

impl SelectionBasket {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn items(&self) -> &[PathBuf] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Agrega rutas nuevas conservando el orden de llegada. Devuelve cuántas se agregaron.
    pub fn add<I>(&mut self, paths: I) -> usize
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let mut added = 0;
        for path in paths {
            let key = path_key(&path);
            if self.keys.insert(key) {
                self.items.push(path);
                added += 1;
            }
        }
        added
    }

    pub fn remove(&mut self, path: &Path) -> bool {
        let key = path_key(path);
        let Some(index) = self.items.iter().position(|item| path_key(item) == key) else {
            return false;
        };
        self.items.remove(index);
        self.keys.remove(&key);
        true
    }

    pub fn remove_indices(&mut self, indices: &[usize]) -> usize {
        let wanted: HashSet<usize> = indices.iter().copied().collect();
        let before = self.items.len();
        self.items = self
            .items
            .drain(..)
            .enumerate()
            .filter_map(|(index, path)| (!wanted.contains(&index)).then_some(path))
            .collect();
        self.rebuild_keys();
        before - self.items.len()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.keys.clear();
    }

    /// Descarta rutas que ya no existen. Debe llamarse desde un worker, nunca desde la UI.
    pub fn retain_existing(&mut self) -> usize {
        let before = self.items.len();
        self.items.retain(|path| path.exists());
        self.rebuild_keys();
        before - self.items.len()
    }

    fn rebuild_keys(&mut self) {
        self.keys = self.items.iter().map(|path| path_key(path)).collect();
    }
}

fn path_key(path: &Path) -> String {
    let key = path.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplica_y_conserva_orden() {
        let mut basket = SelectionBasket::new();
        assert_eq!(
            basket.add([PathBuf::from("C:/a.txt"), PathBuf::from("C:/b.txt")]),
            2
        );
        assert_eq!(basket.add([PathBuf::from("C:/a.txt")]), 0);
        assert_eq!(basket.items()[0], PathBuf::from("C:/a.txt"));
        assert_eq!(basket.items()[1], PathBuf::from("C:/b.txt"));
    }

    #[test]
    fn quitar_indices_reconstruye_la_deduplicacion() {
        let mut basket = SelectionBasket::new();
        basket.add([PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")]);
        assert_eq!(basket.remove_indices(&[0, 2]), 2);
        assert_eq!(basket.items(), &[PathBuf::from("b")]);
        assert_eq!(basket.add([PathBuf::from("a")]), 1);
    }
}
