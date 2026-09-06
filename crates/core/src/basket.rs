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

/// Selección por identidad, independiente de índices que cambian al quitar referencias.
/// El foco gobierna Preview; las marcas gobiernan operaciones. No consulta el disco.
#[derive(Clone, Debug, Default)]
pub struct BasketSelection {
    pub focused: Option<PathBuf>,
    anchor: Option<PathBuf>,
    marked: HashSet<PathBuf>,
}

impl BasketSelection {
    pub fn select(&mut self, items: &[PathBuf], index: usize, control: bool, shift: bool) -> bool {
        let Some(path) = items.get(index) else {
            return false;
        };
        if shift {
            let anchor = self
                .anchor
                .as_ref()
                .or(self.focused.as_ref())
                .and_then(|p| items.iter().position(|item| item == p))
                .unwrap_or(index);
            if !control {
                self.marked.clear();
            }
            self.marked
                .extend(items[anchor.min(index)..=anchor.max(index)].iter().cloned());
            if self.anchor.is_none() {
                self.anchor = Some(items[anchor].clone());
            }
        } else {
            if control {
                if !self.marked.remove(path) {
                    self.marked.insert(path.clone());
                }
            } else {
                self.marked.clear();
                self.marked.insert(path.clone());
            }
            self.anchor = Some(path.clone());
        }
        self.focused = Some(path.clone());
        true
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.marked.contains(path)
    }

    pub fn count(&self) -> usize {
        self.marked.len()
    }

    pub fn clear_marks(&mut self) {
        self.marked.clear();
        self.anchor = self.focused.clone();
    }

    pub fn paths(&self, items: &[PathBuf]) -> Vec<PathBuf> {
        items
            .iter()
            .filter(|p| self.marked.contains(*p))
            .cloned()
            .collect()
    }

    pub fn select_all(&mut self, items: &[PathBuf]) {
        self.marked = items.iter().cloned().collect();
        if self.focused.is_none() {
            self.focused = items.first().cloned();
        }
        if self.anchor.is_none() {
            self.anchor = self.focused.clone();
        }
    }

    pub fn reconcile(&mut self, items: &[PathBuf]) {
        self.marked.retain(|p| items.contains(p));
        if self.focused.as_ref().is_some_and(|p| !items.contains(p)) {
            self.focused = None;
        }
        if self.anchor.as_ref().is_some_and(|p| !items.contains(p)) {
            self.anchor = self.focused.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seleccion_rango_ancla_ctrl_y_reconciliacion_por_ruta() {
        let items: Vec<_> = ["a", "b", "c", "d", "e"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
        let mut selection = BasketSelection::default();
        assert!(!selection.select(&items, 99, false, false));
        selection.select(&items, 1, false, false);
        selection.select(&items, 4, false, true);
        assert_eq!(selection.paths(&items), items[1..].to_vec());
        selection.select(&items, 2, false, true);
        assert_eq!(selection.paths(&items), items[1..3].to_vec());
        selection.select(&items, 0, true, false);
        selection.select(&items, 2, true, false);
        assert_eq!(selection.paths(&items), items[..2].to_vec());
        assert_eq!(selection.focused, Some(items[2].clone()));
        selection.reconcile(&items[1..]);
        assert_eq!(selection.paths(&items), vec![items[1].clone()]);
        selection.select_all(&items);
        assert_eq!(selection.paths(&items), items);
        selection.reconcile(&[]);
        assert!(selection.paths(&items).is_empty());
        assert!(selection.focused.is_none());
    }

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
