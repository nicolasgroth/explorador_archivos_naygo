// Naygo — estado raíz de la aplicación y loop de egui.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use eframe::CreationContext;

/// Estado raíz de Naygo. En la Tarea 8 se le agregan los paneles y el docking.
pub struct NaygoApp {}

impl NaygoApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        NaygoApp {}
    }
}

impl eframe::App for NaygoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("Naygo");
        ui.label("Esqueleto en construcción…");
    }
}
