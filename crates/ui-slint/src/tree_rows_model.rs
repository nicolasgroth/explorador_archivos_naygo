// Naygo — modelo estable de filas del árbol de carpetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use crate::TreeRow;
use slint::{Model, ModelNotify, ModelTracker};
use std::cell::RefCell;

/// Edición por tramo para que Slint conserve los nodos vecinos y compense el scroll
/// cuando una rama se expande por encima del viewport. Un cambio grande sigue siendo lineal.
#[derive(Default)]
pub(crate) struct TreeRowsModel {
    rows: RefCell<Vec<TreeRow>>,
    notify: ModelNotify,
}

impl Model for TreeRowsModel {
    type Data = TreeRow;

    fn row_count(&self) -> usize {
        self.rows.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<TreeRow> {
        self.rows.borrow().get(row).cloned()
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}

impl TreeRowsModel {
    pub(crate) fn reconcile(&self, mut incoming: Vec<TreeRow>) {
        let (prefix, suffix) = {
            let current = self.rows.borrow();
            let prefix = current
                .iter()
                .zip(&incoming)
                .take_while(|(a, b)| a.path == b.path)
                .count();
            let suffix = current[prefix..]
                .iter()
                .rev()
                .zip(incoming[prefix..].iter().rev())
                .take_while(|(a, b)| a.path == b.path)
                .count();
            (prefix, suffix)
        };
        let removed = self.row_count() - prefix - suffix;
        if removed != 0 {
            self.rows.borrow_mut().drain(prefix..prefix + removed);
            self.notify.row_removed(prefix, removed);
        }
        let added = incoming.len() - prefix - suffix;
        if added != 0 {
            let middle = incoming.drain(prefix..prefix + added);
            self.rows.borrow_mut().splice(prefix..prefix, middle);
            self.notify.row_added(prefix, added);
        }
        // Los cambios de expansión, carga o icono en nodos conservados no mueven filas.
        for (index, row) in incoming.into_iter().enumerate() {
            let index = if index < prefix { index } else { index + added };
            let changed = {
                let mut current = self.rows.borrow_mut();
                current.get_mut(index).is_some_and(|old| {
                    if *old == row {
                        return false;
                    }
                    *old = row;
                    true
                })
            };
            if changed {
                self.notify.row_changed(index);
            }
        }
    }
}
