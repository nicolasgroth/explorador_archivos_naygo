// Naygo — estado raíz de la aplicación y loop de egui.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `NaygoApp` mantiene el estado de un panel de archivos, drena el canal del
//! worker de listing cada frame (sin bloquear), y traduce el teclado a acciones.
//! El hilo de UI nunca lee el disco: solo dispara `spawn_listing` y consume mpsc.

use crate::input::{map_key, Action, Key as NaygoKey};
use eframe::CreationContext;
use egui_dock::{DockArea, DockState, NodeIndex, Style};
use naygo_core::cancel::CancellationToken;
use naygo_core::fs_model::PaneState;
use naygo_core::listing::{spawn_listing, ListingMsg};
use naygo_core::sort::sort_entries;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// Qué panel ocupa cada tab del dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneTab {
    Tree,
    Files,
    Inspector,
}

/// Estado compartido que los paneles del dock leen y modifican.
pub struct UiState {
    pub pane: PaneState,
    /// Recibe entradas del worker activo; `None` si no hay listado en curso.
    pub listing_rx: Option<Receiver<ListingMsg>>,
    /// Token del listado en curso, para cancelarlo.
    pub listing_token: CancellationToken,
    /// Texto de estado en la barra inferior.
    pub status: String,
    /// Buffer del type-ahead acumulado entre teclas seguidas.
    pub typeahead_buf: String,
}

impl UiState {
    /// Empieza a listar `dir`: cancela el listado anterior y lanza uno nuevo.
    pub fn navigate_to(&mut self, dir: PathBuf) {
        self.listing_token.cancel(); // corta el worker anterior si lo hay
        let token = CancellationToken::new();
        let (rx, _handle) = spawn_listing(dir.clone(), token.clone());
        self.pane = PaneState::new(dir);
        self.listing_rx = Some(rx);
        self.listing_token = token;
        self.status = "Listando…".to_string();
    }

    /// Drena lo que el worker haya emitido hasta ahora, sin bloquear.
    pub fn pump_listing(&mut self) {
        let Some(rx) = &self.listing_rx else { return };
        let mut finished = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ListingMsg::Entry(e) => self.pane.entries.push(e),
                ListingMsg::Done => {
                    finished = true;
                    self.status = format!("{} elementos", self.pane.entries.len());
                }
                ListingMsg::Cancelled => {
                    finished = true;
                    self.status = "Cancelado".to_string();
                }
                ListingMsg::Error(err) => {
                    finished = true;
                    self.status = format!("Error: {err}");
                }
            }
        }
        if finished {
            let spec = self.pane.sort;
            sort_entries(&mut self.pane.entries, &spec);
            if self.pane.focused.is_none() && !self.pane.entries.is_empty() {
                self.pane.focused = Some(0);
            }
            self.listing_rx = None;
        }
    }

    /// Aplica una acción de navegación al estado del panel.
    pub fn apply_action(&mut self, action: Action) {
        match action {
            Action::MoveUp => self.move_focus(-1),
            Action::MoveDown => self.move_focus(1),
            Action::Activate => self.activate_focused(),
            Action::GoUp => self.go_up(),
            Action::CancelListing => {
                self.listing_token.cancel();
            }
            Action::SwitchPane => { /* multi-panel llega en fase posterior */ }
        }
    }

    fn move_focus(&mut self, delta: isize) {
        if self.pane.entries.is_empty() {
            return;
        }
        let len = self.pane.entries.len() as isize;
        let cur = self.pane.focused.unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, len - 1);
        self.pane.focused = Some(next as usize);
    }

    fn activate_focused(&mut self) {
        let Some(entry) = self.pane.focused_entry().cloned() else {
            return;
        };
        if entry.is_dir() {
            self.navigate_to(entry.path);
        } else {
            self.status = format!("Abrir: {} (pendiente platform::shell)", entry.name);
        }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.pane.current_dir.parent() {
            self.navigate_to(parent.to_path_buf());
        }
    }

    /// Procesa caracteres tipeados para el type-ahead: acumula el prefijo y
    /// mueve el foco a la primera entrada que empieza así.
    pub fn typeahead(&mut self, typed: &str) {
        if typed.is_empty() {
            return;
        }
        self.typeahead_buf.push_str(typed);
        let names: Vec<String> = self.pane.entries.iter().map(|e| e.name.clone()).collect();
        let start = self.pane.focused.unwrap_or(0);
        if let Some(i) = crate::typeahead::find_match(&names, &self.typeahead_buf, start) {
            self.pane.focused = Some(i);
        }
    }
}

/// Estado raíz: el dock y el estado compartido de los paneles.
pub struct NaygoApp {
    dock_state: DockState<PaneTab>,
    ui_state: UiState,
}

impl NaygoApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        let mut dock_state = DockState::new(vec![PaneTab::Files]);
        let surface = dock_state.main_surface_mut();
        let [files, _tree] = surface.split_left(NodeIndex::root(), 0.22, vec![PaneTab::Tree]);
        surface.split_right(files, 0.78, vec![PaneTab::Inspector]);

        let start_dir = default_start_dir();

        let mut ui_state = UiState {
            pane: PaneState::new(start_dir.clone()),
            listing_rx: None,
            listing_token: CancellationToken::new(),
            status: String::new(),
            typeahead_buf: String::new(),
        };
        ui_state.navigate_to(start_dir);

        NaygoApp {
            dock_state,
            ui_state,
        }
    }

    /// Lee el teclado y aplica acciones. Se llama desde `logic` (no pinta).
    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let keys = [
            (egui::Key::ArrowUp, NaygoKey::ArrowUp),
            (egui::Key::ArrowDown, NaygoKey::ArrowDown),
            (egui::Key::ArrowLeft, NaygoKey::ArrowLeft),
            (egui::Key::Enter, NaygoKey::Enter),
            (egui::Key::Backspace, NaygoKey::Backspace),
            (egui::Key::Tab, NaygoKey::Tab),
            (egui::Key::Escape, NaygoKey::Escape),
        ];
        let mut actions = Vec::new();
        let mut typed = String::new();
        ctx.input(|i| {
            for (egui_key, naygo_key) in keys {
                if i.key_pressed(egui_key) {
                    if let Some(action) = map_key(naygo_key) {
                        actions.push(action);
                    }
                }
            }
            for event in &i.events {
                if let egui::Event::Text(t) = event {
                    typed.push_str(t);
                }
            }
        });

        // Las acciones de navegación reinician el buffer de type-ahead.
        if !actions.is_empty() {
            self.ui_state.typeahead_buf.clear();
        }
        for action in actions {
            self.ui_state.apply_action(action);
        }
        if !typed.is_empty() {
            self.ui_state.typeahead(&typed);
        }
    }
}

impl eframe::App for NaygoApp {
    /// Lógica que NO pinta: drenar el canal y leer teclado. Corre antes de `ui`.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui_state.pump_listing();
        self.handle_keyboard(ctx);
        if self.ui_state.listing_rx.is_some() {
            ctx.request_repaint();
        }
    }

    /// Pintado. egui 0.34: el método requerido recibe un `Ui` raíz.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.ui_state.pane.current_dir.display().to_string());
                ui.separator();
                ui.label(&self.ui_state.status);
            });
        });

        let mut viewer = crate::docking::NaygoTabViewer {
            state: &mut self.ui_state,
        };
        DockArea::new(&mut self.dock_state)
            .style(Style::from_egui(ui.style().as_ref()))
            .show_inside(ui, &mut viewer);
    }
}

/// Carpeta inicial razonable: el home del usuario, o `C:\` como fallback.
fn default_start_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("C:\\"))
}
