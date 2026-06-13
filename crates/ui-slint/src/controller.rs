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

    /// Doble clic: carpeta navega; archivo abre con la app por defecto. Devuelve la
    /// carpeta a la que navegar (para que el caller arranque el listado), si aplica.
    pub fn on_row_double_clicked(&mut self, pos: usize) -> Option<std::path::PathBuf> {
        let view = self.pane.view_indices();
        let real = *view.get(pos)?;
        let e = self.pane.entries.get(real)?.clone();
        if e.kind == EntryKind::Directory {
            self.pane.navigate_to(e.path.clone());
            Some(e.path)
        } else {
            let _ = naygo_platform::open::open_default(&e.path);
            None
        }
    }

    /// Subir al padre. Devuelve la carpeta a listar, si hay padre.
    pub fn on_go_up(&mut self) -> Option<std::path::PathBuf> {
        self.pane.go_up()
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

    /// Tecla: resuelve via keymap y aplica la accion navegable de Fase 1. Devuelve la
    /// carpeta a listar si la accion navegó (GoUp/Activate sobre carpeta).
    pub fn on_key(
        &mut self,
        text: &str,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> Option<std::path::PathBuf> {
        // Recordar los modificadores para el Ctrl/Shift+clic.
        self.ctrl_down = ctrl;
        self.shift_down = shift;

        let Some(chord) = crate::keys::chord_from(text, ctrl, shift, alt) else {
            return self.typeahead(text);
        };
        let Some(action) = self.keymap.action_for(&chord) else {
            return self.typeahead(text);
        };
        self.typeahead.clear();
        match action {
            Action::MoveUp => {
                self.pane.move_focus_extend(-1, false);
                None
            }
            Action::MoveDown => {
                self.pane.move_focus_extend(1, false);
                None
            }
            Action::ExtendUp => {
                self.pane.move_focus_extend(-1, true);
                None
            }
            Action::ExtendDown => {
                self.pane.move_focus_extend(1, true);
                None
            }
            Action::FocusPageUp => {
                self.pane.focus_page(-1, PAGE_ROWS, false);
                None
            }
            Action::FocusPageDown => {
                self.pane.focus_page(1, PAGE_ROWS, false);
                None
            }
            Action::ExtendPageUp => {
                self.pane.focus_page(-1, PAGE_ROWS, true);
                None
            }
            Action::ExtendPageDown => {
                self.pane.focus_page(1, PAGE_ROWS, true);
                None
            }
            Action::FocusHome => {
                self.pane.focus_home(false);
                None
            }
            Action::FocusEnd => {
                self.pane.focus_end(false);
                None
            }
            Action::ExtendHome => {
                self.pane.focus_home(true);
                None
            }
            Action::ExtendEnd => {
                self.pane.focus_end(true);
                None
            }
            Action::FocusUpKeep => {
                self.pane.move_focus_keep(-1);
                None
            }
            Action::FocusDownKeep => {
                self.pane.move_focus_keep(1);
                None
            }
            Action::ToggleSelect | Action::ToggleFocused => {
                if let Some(p) = self.pane.focused {
                    self.pane.select_toggle(p);
                }
                None
            }
            Action::SelectAll => {
                self.pane.select_all();
                None
            }
            Action::GoUp => self.on_go_up(),
            Action::Activate => {
                let pos = self.pane.focused?;
                self.on_row_double_clicked(pos)
            }
            // Resto de acciones (Copy/Cut/Paste/Delete/Rename/…): NO-OP en F1; se conectan
            // en F3. No se pierden del keymap, solo aún no tienen efecto.
            _ => None,
        }
    }

    /// Salto por tipeo: primer item de la vista cuyo nombre empieza con lo tecleado
    /// (case-insensitive). El buffer se limpia ante una accion del keymap.
    fn typeahead(&mut self, text: &str) -> Option<std::path::PathBuf> {
        let ch = text.chars().next().filter(|c| !c.is_control())?;
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
        None
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
