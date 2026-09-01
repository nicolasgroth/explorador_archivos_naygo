// Naygo — integración de la bandeja temporal con paneles y operaciones.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use crate::BasketRowVm;
use slint::SharedString;

/// Resultado de leer una `.naygolist`, preparado enteramente fuera del hilo de UI. La existencia
/// se consulta aquí —no al pintar filas— para que un disco de red lento no congele la navegación.
pub struct BasketImport {
    paths: Vec<PathBuf>,
    missing: std::collections::HashSet<PathBuf>,
}

impl WorkspaceCtrl {
    /// Serializa la bandeja sin tocar el filesystem. El callback de UI decide dónde escribirlo
    /// en un worker; así el formato también se puede reutilizar desde pruebas/automatización.
    pub fn basket_naygolist_json(&self, root: Option<PathBuf>) -> Result<String, String> {
        naygo_core::naygolist::NaygoList::from_paths(self.basket.items().iter().cloned(), root)
            .to_json()
            .map_err(|error| error.to_string())
    }

    /// Importa referencias incluso si ya no existen. No ejecuta metadata/exists y por tanto
    /// conserva el carácter portable de .naygolist; la bandeja las muestra como rutas ausentes.
    #[cfg(test)]
    pub fn basket_import_naygolist_json(&mut self, text: &str) -> Result<usize, String> {
        let list = naygo_core::naygolist::NaygoList::from_json(text)?;
        Ok(self.basket.add(list.resolve_paths()?))
    }

    pub fn start_basket_import(&mut self, path: PathBuf) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| {
                let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
                let list = naygo_core::naygolist::NaygoList::from_json(&text)?;
                let paths = list.resolve_paths()?;
                let missing = paths
                    .iter()
                    .filter(|path| !path.exists())
                    .cloned()
                    .collect();
                Ok(BasketImport { paths, missing })
            })();
            let _ = tx.send(result);
        });
        self.basket_import_rx = Some(rx);
    }

    pub fn pump_basket_import(&mut self) -> bool {
        let Some(rx) = self.basket_import_rx.as_ref() else {
            return true;
        };
        let Ok(result) = rx.try_recv() else {
            return false;
        };
        self.basket_import_rx = None;
        match result {
            Ok(import) => {
                self.basket.add(import.paths);
                self.basket_missing.extend(import.missing);
            }
            Err(error) => self.pending_shell_error = Some(error),
        }
        true
    }

    pub fn basket_add_selected(&mut self) -> usize {
        let paths = self.selected_paths();
        self.basket.add(paths)
    }

    pub fn basket_remove(&mut self, index: usize) -> bool {
        let removed_path = self.basket.items().get(index).cloned();
        let removed = self.basket.remove_indices(&[index]) > 0;
        if let Some(path) = removed_path {
            self.basket_missing.remove(&path);
        }
        if self.basket.is_empty() {
            self.basket_staging.clear();
            self.basket_missing.clear();
        }
        removed
    }

    pub fn basket_clear(&mut self) {
        self.basket.clear();
        self.basket_staging.clear();
        self.basket_missing.clear();
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
                    missing: self.basket_missing.contains(&path),
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
            self.basket_missing.clear();
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
            self.basket_missing.clear();
            return had_items;
        }
        self.ensure_ops_pane();
        let label = self.config.t("ops.file_kind_delete");
        self.ops
            .start_op(naygo_core::ops::delete(paths, true), label, true);
        self.basket.clear();
        self.basket_staging.clear();
        self.basket_missing.clear();
        true
    }
}
