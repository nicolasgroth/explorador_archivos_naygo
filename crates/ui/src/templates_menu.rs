// Naygo — combobox de plantillas. Contenido real en Tarea 15.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;

/// Stub: botón "Layouts" sin menú todavía (Tarea 15 lo completa).
pub fn layouts_button(ui: &mut egui::Ui, _app: &mut NaygoApp) {
    let _ = ui.button("▦").on_hover_text("Layouts");
}
