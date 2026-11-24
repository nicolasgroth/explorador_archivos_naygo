// Naygo — selección por identidad en la vista profunda.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
    pub(super) fn deep_select_move(
        &mut self,
        id: PaneId,
        delta: isize,
        extend: bool,
        keep: bool,
    ) -> bool {
        if extend {
            if let Some(job) = self
                .deep_job
                .as_mut()
                .filter(|j| j.pane == id && j.selection_anchor.is_none())
            {
                job.selection_anchor = job.focused_path.clone();
            }
        }
        let moved = self.deep_move_focus(id, delta);
        if keep {
            if moved {
                self.mark_explicit_creation_selection(id);
            }
            return moved;
        }
        if !extend {
            // El movimiento ya resolvió la ruta visible: evita buscar dos veces su índice al
            // seleccionar una sola fila. Shift conserva el cálculo de rango por posición.
            if !moved && self.deep_focused_position(id).is_none() {
                return false;
            }
            let Some(job) = self.deep_job.as_mut().filter(|job| job.pane == id) else {
                return false;
            };
            let Some(target) = job.focused_path.clone() else {
                return false;
            };
            job.selected_paths.clear();
            job.selected_paths.insert(target.clone());
            job.selection_anchor = Some(target);
            job.selection_revision = job.selection_revision.wrapping_add(1);
            self.mark_explicit_creation_selection(id);
            return true;
        }
        self.deep_focused_position(id)
            .is_some_and(|pos| self.deep_select_at(id, pos, false, extend))
    }
    fn deep_visible_paths_range(&self, id: PaneId, lo: usize, hi: usize) -> Vec<PathBuf> {
        let Some(view) = self.deep_view_indices(id) else {
            return Vec::new();
        };
        let job = self.deep_job.as_ref().expect("vista profunda activa");
        view.iter()
            .skip(lo)
            .take(hi.saturating_sub(lo).saturating_add(1))
            .map(|&index| job.items[index].0.path.clone())
            .collect()
    }

    /// Ctrl conserva el lote completo aunque un filtro oculte parte de él; las acciones
    /// rechazan ese estado hasta que el usuario lo resuelva explícitamente.
    pub(super) fn deep_select_at(
        &mut self,
        id: PaneId,
        pos: usize,
        ctrl: bool,
        shift: bool,
    ) -> bool {
        let Some(target) = self
            .deep_visible_entry(id, pos)
            .map(|entry| entry.path.clone())
        else {
            return false;
        };
        let anchor_position = if shift {
            self.deep_job
                .as_ref()
                .filter(|job| job.pane == id)
                .and_then(|job| job.selection_anchor.as_ref())
                .and_then(|anchor| self.deep_position_of(id, anchor))
        } else {
            None
        };
        let range = if shift {
            let anchor = anchor_position.unwrap_or(pos);
            self.deep_visible_paths_range(id, anchor.min(pos), anchor.max(pos))
        } else {
            Vec::new()
        };
        let Some(job) = self.deep_job.as_mut().filter(|j| j.pane == id) else {
            return false;
        };
        if shift {
            job.selected_paths = range.into_iter().collect();
            if anchor_position.is_none() {
                job.selection_anchor = Some(target.clone());
            }
        } else if ctrl {
            if !job.selected_paths.remove(&target) {
                job.selected_paths.insert(target.clone());
            }
            job.selection_anchor = Some(target.clone());
        } else {
            job.selected_paths.clear();
            job.selected_paths.insert(target.clone());
            job.selection_anchor = Some(target.clone());
        }
        job.focused_path = Some(target);
        job.selection_revision = job.selection_revision.wrapping_add(1);
        self.mark_explicit_creation_selection(id);
        true
    }

    pub(super) fn deep_select_all(&mut self, id: PaneId) -> bool {
        let visible = self.deep_visible_paths_range(id, 0, usize::MAX);
        let last = visible.last().cloned();
        let Some(job) = self.deep_job.as_mut().filter(|j| j.pane == id) else {
            return false;
        };
        job.selected_paths = visible.into_iter().collect();
        if let Some(last) = last {
            job.focused_path = Some(last);
        }
        job.selection_revision = job.selection_revision.wrapping_add(1);
        self.mark_explicit_creation_selection(id);
        true
    }

    pub(super) fn deep_select_rect(&mut self, id: PaneId, from: i32, to: i32, additive: bool) {
        let count = self.deep_visible_count(id);
        if count == 0 {
            return;
        }
        let lo = from.min(to).clamp(0, count as i32 - 1) as usize;
        let hi = from.max(to).clamp(0, count as i32 - 1) as usize;
        let visible = self.deep_visible_paths_range(id, lo, hi);
        let Some(last) = visible.last().cloned() else {
            return;
        };
        let Some(job) = self.deep_job.as_mut().filter(|j| j.pane == id) else {
            return;
        };
        if !additive {
            job.selected_paths.clear();
        }
        job.selected_paths.extend(visible);
        job.focused_path = Some(last.clone());
        job.selection_anchor = Some(last);
        job.selection_revision = job.selection_revision.wrapping_add(1);
        self.mark_explicit_creation_selection(id);
    }

    /// Nunca entregar un subconjunto cuando parte de la selección está filtrada o ausente.
    pub(crate) fn deep_complete_selection(&self, id: PaneId) -> Option<Vec<PathBuf>> {
        let job = self.deep_job.as_ref().filter(|j| j.pane == id)?;
        // Las filas anteriores siguen visibles durante una reconciliación, pero todavía
        // no representan un conjunto verificado para lanzar otra operación sobre ellas.
        if job.replacement_items.is_some() {
            return None;
        }
        if job.selected_paths.is_empty() {
            return self
                .deep_focused_position(id)
                .and_then(|pos| self.deep_visible_entry(id, pos))
                .map(|entry| vec![entry.path.clone()]);
        }
        let selected: Vec<_> = self
            .deep_view_indices(id)?
            .iter()
            .map(|&index| &job.items[index].0)
            .filter(|entry| job.selected_paths.contains(&entry.path))
            .map(|entry| entry.path.clone())
            .collect();
        if selected.len() != job.selected_paths.len() {
            return None;
        }
        // No entregar carpeta y descendiente como fuentes separadas: algunas operaciones
        // recorrerían el mismo árbol dos veces. Rechazar el lote completo, nunca reducirlo.
        let identities: std::collections::HashSet<_> = selected
            .iter()
            .map(|path| naygo_core::path_identity::key(path))
            .collect();
        if selected.iter().any(|path| {
            path.ancestors()
                .skip(1)
                .any(|parent| identities.contains(&naygo_core::path_identity::key(parent)))
        }) {
            return None;
        }
        Some(selected)
    }

    pub(super) fn operation_sources(&mut self) -> Vec<PathBuf> {
        if let Some(id) = self.ws.active_id().filter(|id| self.is_deep_active(*id)) {
            if let Some(menu) = self
                .context_menu
                .as_ref()
                .filter(|menu| !menu.folder_mode && menu.pane == id)
            {
                return menu.targets.clone();
            }
            return match self.deep_complete_selection(id) {
                Some(paths) => paths,
                None => {
                    self.deep_selection_notice = true;
                    Vec::new()
                }
            };
        }
        self.selected_paths()
    }
}
