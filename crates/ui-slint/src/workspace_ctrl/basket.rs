// Naygo — integración de la bandeja temporal con paneles y operaciones.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use crate::BasketRowVm;
use slint::SharedString;

impl WorkspaceCtrl {
    pub fn basket_add_selected(&mut self) -> usize {
        let paths = self.selected_paths();
        self.basket.add(paths)
    }

    pub fn basket_remove(&mut self, index: usize) -> bool {
        let removed = self.basket.remove_indices(&[index]) > 0;
        if self.basket.is_empty() {
            self.basket_staging.clear();
        }
        removed
    }

    pub fn basket_clear(&mut self) {
        self.basket.clear();
        self.basket_staging.clear();
    }

    pub fn basket_rows(&mut self) -> Vec<BasketRowVm> {
        let paths = self.basket.items().to_vec();
        paths
            .into_iter()
            .map(|path| {
                let name = path
                    .file_name()
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                let ext = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                let icon = self.icons.get(naygo_core::IconKey::File(
                    naygo_core::category_for_extension(ext),
                ));
                BasketRowVm {
                    name: SharedString::from(name),
                    path: SharedString::from(path.to_string_lossy().as_ref()),
                    icon,
                }
            })
            .collect()
    }

    pub fn basket_transfer_to(&mut self, dest: PathBuf, move_files: bool) -> bool {
        let paths = self.basket.items().to_vec();
        if paths.is_empty() {
            return false;
        }
        let label = if move_files {
            self.config.t("ops.file_kind_move")
        } else {
            self.config.t("ops.file_kind_copy")
        };
        self.ensure_ops_pane();
        self.ops.start_op_with_staging(
            naygo_core::ops::transfer(move_files, paths, dest),
            label,
            true,
            self.basket_staging.clone(),
        );
        if move_files {
            self.basket.clear();
            self.basket_staging.clear();
        }
        true
    }

    pub fn basket_delete(&mut self) -> bool {
        // Los ítems provenientes de un .zip/WinRAR viven en staging: «eliminarlos» significa
        // soltarlos de la bandeja y dejar que el guard limpie el temporal, no mandar basura
        // interna de Naygo a la Papelera. Solo las rutas reales llegan a la operación Shell.
        let virtual_roots: Vec<&std::path::Path> = self
            .basket_staging
            .iter()
            .map(|guard| guard.root())
            .collect();
        let paths: Vec<PathBuf> = self
            .basket
            .items()
            .iter()
            .filter(|path| !virtual_roots.iter().any(|root| path.starts_with(root)))
            .cloned()
            .collect();
        let had_items = !self.basket.is_empty();
        if paths.is_empty() {
            self.basket.clear();
            self.basket_staging.clear();
            return had_items;
        }
        self.ensure_ops_pane();
        let label = self.config.t("ops.file_kind_delete");
        self.ops
            .start_op(naygo_core::ops::delete(paths, true), label, true);
        self.basket.clear();
        self.basket_staging.clear();
        true
    }
}
