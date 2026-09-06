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
        self.basket_selection.reconcile(self.basket.items());
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
        self.basket_selection = Default::default();
    }

    /// Selecciona una fila de la bandeja. No consulta el disco: Preview y metadata harán su
    /// trabajo en sus workers habituales cuando el tick sincronice el nuevo objetivo.
    pub fn basket_select(&mut self, index: usize) -> bool {
        self.basket_select_modified(index, false, false)
    }

    pub fn basket_select_modified(&mut self, index: usize, control: bool, shift: bool) -> bool {
        self.basket_context = true;
        self.basket_selection
            .select(self.basket.items(), index, control, shift)
    }

    pub fn basket_select_all(&mut self) -> bool {
        self.basket_context = true;
        self.basket_selection.select_all(self.basket.items());
        !self.basket.is_empty()
    }

    /// Sin selección no se opera implícitamente sobre toda la bandeja. Ctrl+A hace explícito
    /// ese alcance y los botones muestran cuántas referencias están marcadas.
    pub fn basket_action_paths(&self) -> Vec<PathBuf> {
        self.basket_selection.paths(self.basket.items())
    }

    pub fn basket_remove_selected(&mut self) -> bool {
        let paths = self.basket_action_paths();
        for path in &paths {
            self.basket.remove(path);
            self.basket_missing.remove(path);
        }
        self.basket_selection.reconcile(self.basket.items());
        if self.basket.is_empty() {
            self.basket_staging.clear();
        }
        !paths.is_empty()
    }

    pub fn basket_move_selection(&mut self, delta: isize) -> bool {
        self.basket_move_modified(delta, false, false)
    }

    pub fn basket_move_modified(&mut self, delta: isize, keep: bool, shift: bool) -> bool {
        let len = self.basket.items().len();
        if len == 0 {
            return false;
        }
        let current = self
            .basket_selection
            .focused
            .clone()
            .and_then(|p| self.basket.items().iter().position(|v| v == &p))
            .map(|i| i as isize);
        let next = current
            .map_or(if delta < 0 { len as isize - 1 } else { 0 }, |i| i + delta)
            .clamp(0, len as isize - 1) as usize;
        if keep && !shift {
            self.basket_selection.focused = self.basket.items().get(next).cloned();
            true
        } else {
            self.basket_select_modified(next, keep, shift)
        }
    }

    /// Ruta elegida en la bandeja, siempre que siga perteneciendo a ella. Centralizar esta
    /// validación evita previews colgados si una operación vacía la bandeja entre dos ticks.
    pub fn basket_selected_path(&self) -> Option<PathBuf> {
        if !self.basket_context {
            return None;
        }
        let path = self.basket_selection.focused.as_ref()?;
        self.basket.items().contains(path).then(|| path.clone())
    }

    /// Propiedades inmediatas del ítem elegido, sin metadata ni consultas de disco. El worker de
    /// metadata completará los campos específicos (imagen, ejecutable, procedencia) en el tick.
    pub fn basket_inspector_info(&self) -> Option<crate::bridge::InspectorInfo> {
        let path = self.basket_selected_path()?;
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let basic = self
            .meta_job
            .as_ref()
            .filter(|job| job.path == path)
            .and_then(|job| job.basic.as_ref());
        let metadata = basic.and_then(|result| result.as_ref().ok());
        let is_dir = metadata.is_some_and(|m| m.is_dir());
        let date_format = self.config.settings.date_format;
        let offset = naygo_platform::time::local_utc_offset_secs();
        Some(crate::bridge::InspectorInfo {
            present: true,
            name,
            kind: if let Some(Err(error)) = basic {
                error.clone()
            } else if metadata.is_none() {
                self.config.t("meta.loading")
            } else {
                self.config
                    .t(if is_dir { "kind.folder" } else { "kind.file" })
            },
            path: path.display().to_string(),
            size: metadata
                .filter(|m| !m.is_dir())
                .map(|m| naygo_core::format::human_size(m.len()))
                .unwrap_or_default(),
            modified: crate::bridge::fmt_time(
                metadata.and_then(|m| m.modified().ok()),
                date_format,
                offset,
            ),
            created: crate::bridge::fmt_time(
                metadata.and_then(|m| m.created().ok()),
                date_format,
                offset,
            ),
            is_dir,
            size_calc: String::new(),
        })
    }

    /// Abre el radar de destinos para la bandeja. Los paneles Files abiertos se ofrecen antes
    /// que cualquier otra vía porque son el destino inmediato que el usuario ya tiene a la
    /// vista. El radar también deja disponible «Otra carpeta…» cuando no haya paneles Files.
    pub fn basket_request_transfer(&mut self, move_files: bool) -> bool {
        let sources = self.basket_action_paths();
        if sources.is_empty() {
            return false;
        }
        use naygo_core::destination_radar::{
            rank, DestinationCandidate as Candidate, DestinationSource,
        };
        let open: Vec<Candidate> = self
            .ws
            .layout
            .pane_ids()
            .into_iter()
            .filter_map(|id| {
                let files = self.ws.pane(id)?.files.as_ref()?;
                Some(Candidate {
                    path: files.current_dir.clone(),
                    label: self.pane_label(id),
                    source: DestinationSource::OpenPanel,
                })
            })
            .collect();
        self.destination_radar = Some(DestinationRadar {
            move_files,
            sources,
            candidates: rank([open], usize::MAX),
            origin: DestinationRadarOrigin::Basket,
        });
        true
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
                    selected: self.basket_selection.contains(&path),
                    icon,
                }
            })
            .collect()
    }

    #[cfg(test)]
    pub fn basket_transfer_to(&mut self, dest: PathBuf, move_files: bool) -> bool {
        let paths = self.basket.items().to_vec();
        self.basket_transfer_paths(paths, dest, move_files)
    }

    pub fn basket_transfer_paths(
        &mut self,
        paths: Vec<PathBuf>,
        dest: PathBuf,
        move_files: bool,
    ) -> bool {
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
        // Conservar referencias y staging hasta que el usuario los retire: cancelar o fallar
        // una transferencia no debe vaciar el conjunto de trabajo.
        true
    }

    pub fn basket_delete(&mut self) -> bool {
        self.basket_delete_kind(false)
    }

    pub fn basket_delete_kind(&mut self, permanent: bool) -> bool {
        // Los ítems provenientes de un .zip/WinRAR viven en staging: «eliminarlos» significa
        // soltarlos de la bandeja y dejar que el guard limpie el temporal, no mandar basura
        // interna de Naygo a la Papelera. Solo las rutas reales llegan a la operación Shell.
        let virtual_roots: Vec<&std::path::Path> = self
            .basket_staging
            .iter()
            .map(|guard| guard.root())
            .collect();
        let paths: Vec<PathBuf> = self
            .basket_action_paths()
            .into_iter()
            .filter(|path| !virtual_roots.iter().any(|root| path.starts_with(root)))
            .collect();
        if paths.is_empty() {
            return false;
        }
        self.ensure_ops_pane();
        // La acción destructiva es explícita y confirma los originales concretos. Mantener
        // las referencias si se cancela o falla; «quitar» y «vaciar» son acciones distintas.
        self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
            sources: paths,
            permanent,
        });
        true
    }
}
