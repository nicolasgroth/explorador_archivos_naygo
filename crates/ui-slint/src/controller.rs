// Naygo — controlador de la UI Slint (Fase 1): estado del panel + handlers de gestos.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Es el equivalente ACOTADO de NaygoApp para la Fase 1: posee el FilePaneState (un solo
// panel), traduce los gestos de Slint (clic, doble clic, orden, teclado) a llamadas del
// core, y reconstruye el modelo de filas. CERO logica de negocio nueva.

use crate::bridge::{rows_from_view, PlainRow};
use crate::listing::Listing;
use naygo_core::fs_model::{EntryKind, SortKey};
use naygo_core::keymap::{Action, KeyMap};
use naygo_core::workspace::FilePaneState;

/// Alto de fila (px) — fijo, igual que la tabla egui; se usa para el scroll a foco.
pub const ROW_HEIGHT: f32 = 22.0;
/// Tamaño de página por defecto para AvPag/RePag (fallback; F1 no mide el alto real aún).
const PAGE_ROWS: usize = 20;

/// Estado de la Fase 1: un panel + su keymap + el listado en curso.
pub struct Controller {
    pub pane: FilePaneState,
    pub keymap: KeyMap,
    pub listing: Option<Listing>,
    pub typeahead: String,
    /// Últimos modificadores de teclado vistos (para Ctrl/Shift+clic, que la TouchArea de
    /// Slint no expone directamente en el evento de clic).
    pub ctrl_down: bool,
    pub shift_down: bool,
}

impl Controller {
    pub fn new(start: std::path::PathBuf) -> Controller {
        let pane = FilePaneState::new(start.clone());
        let mut c = Controller {
            pane,
            keymap: KeyMap::defaults(),
            listing: None,
            typeahead: String::new(),
            ctrl_down: false,
            shift_down: false,
        };
        c.start_listing(start);
        c
    }

    /// Arranca (o reinicia) el listado de `dir`. Cancela el anterior.
    pub fn start_listing(&mut self, dir: std::path::PathBuf) {
        if let Some(l) = &self.listing {
            l.cancel();
        }
        self.listing = Some(Listing::start(dir));
    }

    /// Drena el lote actual del listado. Devuelve `true` si el listado TERMINO (para que
    /// el caller apague el timer). Aplica las entries y reordena al terminar.
    pub fn pump_listing(&mut self) -> bool {
        let Some(l) = &self.listing else {
            return true;
        };
        let (batch, done) = l.poll();
        if !batch.is_empty() {
            self.pane.entries.extend(batch);
        }
        if done {
            let spec = self.pane.sort;
            naygo_core::sort::sort_entries(&mut self.pane.entries, &spec);
            if self.pane.focused.is_none() && !self.pane.entries.is_empty() {
                self.pane.focused = Some(0);
            }
            self.listing = None;
        }
        done
    }

    /// Filas actuales para el modelo de Slint.
    pub fn rows(&self) -> Vec<PlainRow> {
        rows_from_view(&self.pane)
    }

    pub fn current_path(&self) -> String {
        self.pane.current_dir.display().to_string()
    }

    /// Clic en una fila (pos de vista). Usa los modificadores de teclado vistos por última
    /// vez (Ctrl=toggle, Shift=rango), igual que un explorador.
    pub fn on_row_clicked(&mut self, pos: usize) {
        if self.shift_down {
            self.pane.select_range_to(pos);
        } else if self.ctrl_down {
            self.pane.select_toggle(pos);
        } else {
            self.pane.select_single(pos);
        }
    }

    /// Doble clic: carpeta navega (y ARRANCA su listado); archivo abre con la app por
    /// defecto. Devuelve `true` si se navegó (para que el caller encienda el timer del
    /// listado). El listado se arranca aquí mismo —junto al cambio de carpeta— para que
    /// `current_dir` y el `Listing` activo NUNCA se desincronicen.
    pub fn on_row_double_clicked(&mut self, pos: usize) -> bool {
        let view = self.pane.view_indices();
        let Some(&real) = view.get(pos) else {
            return false;
        };
        let Some(e) = self.pane.entries.get(real).cloned() else {
            return false;
        };
        if e.kind == EntryKind::Directory {
            self.pane.navigate_to(e.path.clone());
            self.start_listing(e.path);
            true
        } else {
            let _ = naygo_platform::open::open_default(&e.path);
            false
        }
    }

    /// Subir al padre (y ARRANCA su listado). Devuelve `true` si se movió.
    pub fn on_go_up(&mut self) -> bool {
        match self.pane.go_up() {
            Some(dir) => {
                self.start_listing(dir);
                true
            }
            None => false,
        }
    }

    /// Clic en encabezado: ordenar por esa columna (alterna asc/desc si ya era esa).
    pub fn on_sort_by(&mut self, column: &str) {
        let key = match column {
            "name" => SortKey::Name,
            "ext" => SortKey::Extension,
            "size" => SortKey::Size,
            "modified" => SortKey::Modified,
            _ => return,
        };
        if self.pane.sort.key == key {
            self.pane.sort.ascending = !self.pane.sort.ascending;
        } else {
            self.pane.sort.key = key;
            self.pane.sort.ascending = true;
        }
        let spec = self.pane.sort;
        naygo_core::sort::sort_entries(&mut self.pane.entries, &spec);
    }

    /// Tecla: resuelve via keymap y aplica la accion navegable de Fase 1. Devuelve `true`
    /// si la accion NAVEGÓ (GoUp/Activate sobre carpeta) y arrancó un listado, para que el
    /// caller encienda el timer.
    pub fn on_key(&mut self, text: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        // Recordar los modificadores para el Ctrl/Shift+clic.
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
        match action {
            Action::MoveUp => self.pane.move_focus_extend(-1, false),
            Action::MoveDown => self.pane.move_focus_extend(1, false),
            Action::ExtendUp => self.pane.move_focus_extend(-1, true),
            Action::ExtendDown => self.pane.move_focus_extend(1, true),
            Action::FocusPageUp => self.pane.focus_page(-1, PAGE_ROWS, false),
            Action::FocusPageDown => self.pane.focus_page(1, PAGE_ROWS, false),
            Action::ExtendPageUp => self.pane.focus_page(-1, PAGE_ROWS, true),
            Action::ExtendPageDown => self.pane.focus_page(1, PAGE_ROWS, true),
            Action::FocusHome => self.pane.focus_home(false),
            Action::FocusEnd => self.pane.focus_end(false),
            Action::ExtendHome => self.pane.focus_home(true),
            Action::ExtendEnd => self.pane.focus_end(true),
            Action::FocusUpKeep => self.pane.move_focus_keep(-1),
            Action::FocusDownKeep => self.pane.move_focus_keep(1),
            Action::ToggleSelect | Action::ToggleFocused => {
                if let Some(p) = self.pane.focused {
                    self.pane.select_toggle(p);
                }
            }
            Action::SelectAll => self.pane.select_all(),
            Action::GoUp => return self.on_go_up(),
            Action::Activate => {
                if let Some(pos) = self.pane.focused {
                    return self.on_row_double_clicked(pos);
                }
            }
            // Resto de acciones (Copy/Cut/Paste/Delete/Rename/…): NO-OP en F1; se conectan
            // en F3. No se pierden del keymap, solo aún no tienen efecto.
            _ => {}
        }
        false
    }

    /// Salto por tipeo: primer item de la vista cuyo nombre empieza con lo tecleado
    /// (case-insensitive). El buffer se limpia ante una accion del keymap.
    fn typeahead(&mut self, text: &str) {
        let Some(ch) = text.chars().next().filter(|c| !c.is_control()) else {
            return;
        };
        self.typeahead.push(ch.to_ascii_lowercase());
        let view = self.pane.view_indices();
        let needle = self.typeahead.clone();
        for (pos, &real) in view.iter().enumerate() {
            if let Some(e) = self.pane.entries.get(real) {
                if e.name.to_lowercase().starts_with(needle.as_str()) {
                    self.pane.select_single(pos);
                    break;
                }
            }
        }
    }

    /// Offset de scroll (px) para que la fila enfocada quede visible. El caller lo aplica a
    /// `panel-scroll-y`. Simplificacion F1: alinea la fila enfocada al tope (negativo = el
    /// viewport sube). El pulido fino (scroll minimo) se evalua en verificacion viva.
    pub fn focus_scroll_y(&self) -> f32 {
        match self.pane.focused {
            Some(p) => -(p as f32) * ROW_HEIGHT,
            None => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drena el listado en curso hasta que termina (con timeout duro), simulando los ticks
    /// del Timer. Devuelve true si terminó.
    fn drain_until_done(c: &mut Controller) -> bool {
        for _ in 0..2000 {
            if c.pump_listing() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        false
    }

    /// REGRESIÓN: al navegar a otra carpeta, el listado de ESA carpeta debe arrancar y
    /// poblar la vista (el bug era que navigate_to limpiaba entries pero nadie relanzaba
    /// el listado → quedaba vacío). Usa un tempdir con un archivo conocido.
    #[test]
    fn navegar_a_carpeta_repuebla_la_vista() {
        let tmp = tempfile::tempdir().unwrap();
        // Subcarpeta "sub" con un archivo dentro.
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("dentro.txt"), b"hola").unwrap();

        // Arrancar el controller en el tempdir y completar su listado inicial.
        let mut c = Controller::new(tmp.path().to_path_buf());
        assert!(drain_until_done(&mut c), "el listado inicial debe terminar");
        // La vista del tempdir contiene "sub".
        let pos_sub = c
            .pane
            .view_indices()
            .iter()
            .position(|&real| c.pane.entries[real].name == "sub")
            .expect("'sub' debe aparecer en el tempdir");

        // Doble clic en "sub" → debe navegar Y arrancar el listado de sub.
        assert!(
            c.on_row_double_clicked(pos_sub),
            "doble clic en carpeta navega"
        );
        assert!(c.listing.is_some(), "navegar arranca un listado nuevo");
        assert!(drain_until_done(&mut c), "el listado de sub debe terminar");

        // La vista AHORA refleja el contenido de sub (no quedó vacía: ESE era el bug).
        let rows = c.rows();
        assert!(
            rows.iter().any(|r| r.name == "dentro.txt"),
            "tras navegar, la vista muestra el contenido de la carpeta (no vacía)"
        );
    }

    /// Subir al padre también repuebla la vista.
    #[test]
    fn subir_al_padre_repuebla_la_vista() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(tmp.path().join("raiz.txt"), b"x").unwrap();

        let mut c = Controller::new(sub.clone());
        assert!(drain_until_done(&mut c));
        assert!(c.on_go_up(), "hay padre, debe subir");
        assert!(drain_until_done(&mut c));
        let rows = c.rows();
        assert!(rows.iter().any(|r| r.name == "raiz.txt"));
        assert!(rows.iter().any(|r| r.name == "sub"));
    }
}
