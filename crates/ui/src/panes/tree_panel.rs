// Naygo — panel de árbol de carpetas (esqueleto de Fase 1).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! En la Fase 1 el árbol es un esqueleto: muestra la carpeta actual y permite
//! subir al padre con un botón. El árbol expandible real (con lazy-load por
//! streaming) se construye en una fase posterior; este panel reserva el espacio
//! arquitectónico y el lugar en el dock.

use crate::app::UiState;
use crate::input::Action;

pub fn show(ui: &mut egui::Ui, state: &mut UiState) {
    ui.label("Ubicación actual:");
    ui.monospace(state.pane.current_dir.display().to_string());
    ui.separator();
    if ui.button("⬆ Subir un nivel").clicked() {
        state.apply_action(Action::GoUp);
    }
}
