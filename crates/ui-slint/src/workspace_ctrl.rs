// Naygo — controlador multi-panel de la UI Slint (Fase 2a). Posee el Workspace (varios
// FilePaneState + layout) y traduce gestos a llamadas del core.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::bridge::{rows_from_view, PlainRow};
use crate::listing::Listing;
use naygo_core::fs_model::{EntryKind, SortKey};
use naygo_core::keymap::{Action, KeyMap};
use naygo_core::workspace::layout::{
    Rect, SerializableDockLayout, SplitDir, SplitHandle, SplitStep,
};
use naygo_core::workspace::{FilePaneState, PaneId, PanePurpose, Workspace};
use std::collections::HashMap;

const PAGE_ROWS: usize = 20;

pub struct WorkspaceCtrl {
    pub ws: Workspace,
    pub keymap: KeyMap,
    /// Un listado en curso por panel (la carpeta de cada panel se lista por separado).
    pub listings: HashMap<PaneId, Listing>,
    pub typeahead: String,
    pub ctrl_down: bool,
    pub shift_down: bool,
}

impl WorkspaceCtrl {
    /// Arranca con UN panel Files en `start` (el usuario agrega más con el botón). Lanza
    /// su listado inicial.
    pub fn new(start: std::path::PathBuf) -> WorkspaceCtrl {
        let mut ws = Workspace::new();
        let id = ws.add_pane(PanePurpose::Files, start.clone());
        ws.layout = SerializableDockLayout::single(id);
        ws.set_active(id);
        let mut c = WorkspaceCtrl {
            ws,
            keymap: KeyMap::defaults(),
            listings: HashMap::new(),
            typeahead: String::new(),
            ctrl_down: false,
            shift_down: false,
        };
        c.start_listing(id, start);
        c
    }

    /// Arranca el listado del panel `id` en `dir` (cancela el suyo anterior).
    pub fn start_listing(&mut self, id: PaneId, dir: std::path::PathBuf) {
        if let Some(l) = self.listings.get(&id) {
            l.cancel();
        }
        self.listings.insert(id, Listing::start(dir));
    }

    /// Drena los lotes de TODOS los listados activos. Devuelve true si TODOS terminaron
    /// (para apagar el timer). Quita del mapa los que terminan.
    pub fn pump_listings(&mut self) -> bool {
        let ids: Vec<PaneId> = self.listings.keys().copied().collect();
        for id in ids {
            let (batch, done) = match self.listings.get(&id) {
                Some(l) => l.poll(),
                None => continue,
            };
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                if !batch.is_empty() {
                    f.entries.extend(batch);
                }
                if done {
                    let spec = f.sort;
                    naygo_core::sort::sort_entries(&mut f.entries, &spec);
                    if f.focused.is_none() && !f.entries.is_empty() {
                        f.focused = Some(0);
                    }
                }
            }
            if done {
                self.listings.remove(&id);
            }
        }
        self.listings.is_empty()
    }

    /// Rects de los paneles (id, rect) dado el área de contenido.
    pub fn pane_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        self.ws.layout.pane_rects(area)
    }

    /// Handles de splitter (para pintarlos y arrastrarlos).
    pub fn split_handles(&self, area: Rect) -> Vec<SplitHandle> {
        self.ws.layout.split_handles(area)
    }

    /// Ajusta la fracción de un split (drag de splitter).
    pub fn set_fraction(&mut self, path: &[SplitStep], fraction: f32) {
        self.ws.layout.set_fraction(path, fraction);
    }

    /// Filas a pintar del panel `id`.
    pub fn rows_of(&self, id: PaneId) -> Vec<PlainRow> {
        match self.ws.pane(id).and_then(|p| p.files.as_ref()) {
            Some(f) => rows_from_view(f),
            None => Vec::new(),
        }
    }

    /// Carpeta actual del panel `id` (para su path-bar).
    pub fn path_of(&self, id: PaneId) -> String {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.display().to_string())
            .unwrap_or_default()
    }

    pub fn active_id(&self) -> Option<PaneId> {
        self.ws.active_id()
    }

    pub fn set_active(&mut self, id: PaneId) {
        self.ws.set_active(id);
    }

    /// Agrega un panel Files DIVIDIENDO el leaf activo (horizontal). Lo deja activo y
    /// arranca su listado en la misma carpeta que el activo (o el home).
    pub fn add_pane_split(&mut self) {
        let dir = self
            .ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
        let active = self.ws.active_id();
        let new_id = self.ws.add_pane(PanePurpose::Files, dir.clone());
        if let Some(active) = active {
            self.ws
                .layout
                .split_leaf(active, SplitDir::Horizontal, new_id);
        }
        self.ws.set_active(new_id);
        self.start_listing(new_id, dir);
    }

    // --- Gestos sobre el panel ACTIVO (reusan la logica de F1) ---

    pub fn on_row_clicked(&mut self, id: PaneId, pos: usize) {
        self.ws.set_active(id);
        let (ctrl, shift) = (self.ctrl_down, self.shift_down);
        if let Some(f) = self.ws.active_files_mut() {
            if shift {
                f.select_range_to(pos);
            } else if ctrl {
                f.select_toggle(pos);
            } else {
                f.select_single(pos);
            }
        }
    }

    /// Doble clic en el panel `id`, posición `pos`. Navega (y arranca listado) o abre.
    pub fn on_row_double_clicked(&mut self, id: PaneId, pos: usize) -> bool {
        self.ws.set_active(id);
        let target = {
            let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) else {
                return false;
            };
            let view = f.view_indices();
            let Some(&real) = view.get(pos) else {
                return false;
            };
            f.entries.get(real).cloned()
        };
        let Some(e) = target else { return false };
        if e.kind == EntryKind::Directory {
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.navigate_to(e.path.clone());
            }
            self.start_listing(id, e.path);
            true
        } else {
            let _ = naygo_platform::open::open_default(&e.path);
            false
        }
    }

    /// Subir al padre en el panel activo (y arranca su listado).
    pub fn on_go_up(&mut self) -> bool {
        let active = match self.ws.active_id() {
            Some(a) => a,
            None => return false,
        };
        let moved = self.ws.active_files_mut().and_then(|f| f.go_up());
        match moved {
            Some(dir) => {
                self.start_listing(active, dir);
                true
            }
            None => false,
        }
    }

    pub fn on_sort_by(&mut self, column: &str) {
        let key = match column {
            "name" => SortKey::Name,
            "ext" => SortKey::Extension,
            "size" => SortKey::Size,
            "modified" => SortKey::Modified,
            _ => return,
        };
        if let Some(f) = self.ws.active_files_mut() {
            if f.sort.key == key {
                f.sort.ascending = !f.sort.ascending;
            } else {
                f.sort.key = key;
                f.sort.ascending = true;
            }
            let spec = f.sort;
            naygo_core::sort::sort_entries(&mut f.entries, &spec);
        }
    }

    /// Tecla sobre el panel activo (reusa el keymap). Devuelve true si navegó.
    pub fn on_key(&mut self, text: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.ctrl_down = ctrl;
        self.shift_down = shift;
        let Some(chord) = crate::keys::chord_from(text, ctrl, shift, alt) else {
            self.typeahead(text);
            return false;
        };
        let Some(action) = self.keymap.action_for(&chord) else {
            self.typeahead(text);
            return false;
        };
        self.typeahead.clear();
        let active = self.ws.active_id();
        match action {
            Action::MoveUp => self.with_active(|f| f.move_focus_extend(-1, false)),
            Action::MoveDown => self.with_active(|f| f.move_focus_extend(1, false)),
            Action::ExtendUp => self.with_active(|f| f.move_focus_extend(-1, true)),
            Action::ExtendDown => self.with_active(|f| f.move_focus_extend(1, true)),
            Action::FocusPageUp => self.with_active(|f| f.focus_page(-1, PAGE_ROWS, false)),
            Action::FocusPageDown => self.with_active(|f| f.focus_page(1, PAGE_ROWS, false)),
            Action::ExtendPageUp => self.with_active(|f| f.focus_page(-1, PAGE_ROWS, true)),
            Action::ExtendPageDown => self.with_active(|f| f.focus_page(1, PAGE_ROWS, true)),
            Action::FocusHome => self.with_active(|f| f.focus_home(false)),
            Action::FocusEnd => self.with_active(|f| f.focus_end(false)),
            Action::ExtendHome => self.with_active(|f| f.focus_home(true)),
            Action::ExtendEnd => self.with_active(|f| f.focus_end(true)),
            Action::FocusUpKeep => self.with_active(|f| f.move_focus_keep(-1)),
            Action::FocusDownKeep => self.with_active(|f| f.move_focus_keep(1)),
            Action::ToggleSelect | Action::ToggleFocused => self.with_active(|f| {
                if let Some(p) = f.focused {
                    f.select_toggle(p);
                }
            }),
            Action::SelectAll => self.with_active(|f| f.select_all()),
            Action::SwitchPane => {
                // Tab: ciclar el panel activo entre los Files.
                let files = self.ws.files_panes();
                if files.len() > 1 {
                    if let Some(cur) = active {
                        let i = files.iter().position(|&p| p == cur).unwrap_or(0);
                        let next = files[(i + 1) % files.len()];
                        self.ws.set_active(next);
                    }
                }
            }
            Action::GoUp => return self.on_go_up(),
            Action::Activate => {
                if let (Some(id), Some(pos)) =
                    (active, self.ws.active_files().and_then(|f| f.focused))
                {
                    return self.on_row_double_clicked(id, pos);
                }
            }
            _ => {}
        }
        false
    }

    /// Aplica `op` al panel activo (helper para no repetir el match de préstamos).
    fn with_active(&mut self, op: impl FnOnce(&mut FilePaneState)) {
        if let Some(f) = self.ws.active_files_mut() {
            op(f);
        }
    }

    fn typeahead(&mut self, text: &str) {
        let Some(ch) = text.chars().next().filter(|c| !c.is_control()) else {
            return;
        };
        self.typeahead.push(ch.to_ascii_lowercase());
        let needle = self.typeahead.clone();
        if let Some(f) = self.ws.active_files_mut() {
            let view = f.view_indices();
            for (pos, &real) in view.iter().enumerate() {
                if let Some(e) = f.entries.get(real) {
                    if e.name.to_lowercase().starts_with(needle.as_str()) {
                        f.select_single(pos);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drena los listados hasta que todos terminan (con timeout), simulando los ticks del
    /// Timer. Devuelve true si terminaron.
    fn drain(c: &mut WorkspaceCtrl) -> bool {
        for _ in 0..2000 {
            if c.pump_listings() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        false
    }

    fn active_pos_of(c: &WorkspaceCtrl, name: &str) -> Option<usize> {
        let f = c.ws.active_files()?;
        f.view_indices()
            .iter()
            .position(|&real| f.entries[real].name == name)
    }

    /// REGRESIÓN (heredada de F1): navegar a una carpeta repuebla la vista del panel
    /// activo (el listado de la carpeta nueva se arranca al navegar).
    #[test]
    fn navegar_repuebla_la_vista() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("dentro.txt"), b"x").unwrap();

        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c), "listado inicial termina");
        let id = c.active_id().unwrap();
        let pos = active_pos_of(&c, "sub").expect("'sub' visible");
        assert!(c.on_row_double_clicked(id, pos), "doble clic navega");
        assert!(drain(&mut c), "listado de sub termina");
        let rows = c.rows_of(c.active_id().unwrap());
        assert!(
            rows.iter().any(|r| r.name == "dentro.txt"),
            "la vista refleja la carpeta nueva (no vacía)"
        );
    }

    /// Agregar un panel divide el layout y deja DOS paneles Files; el nuevo queda activo.
    #[test]
    fn agregar_panel_divide_y_deja_dos() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let first = c.active_id().unwrap();
        c.add_pane_split();
        assert!(drain(&mut c));
        // Dos paneles Files en el layout, y el activo es el nuevo (distinto del primero).
        assert_eq!(c.ws.files_panes().len(), 2);
        assert_ne!(c.active_id(), Some(first), "el panel nuevo queda activo");
        // El área se reparte en dos rects no vacíos.
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        let rects = c.pane_rects(area);
        assert_eq!(rects.len(), 2);
        assert!(rects.iter().all(|(_, r)| r.w > 1.0 && r.h > 1.0));
        // Y hay un splitter entre ellos.
        assert_eq!(c.split_handles(area).len(), 1);
    }
}
