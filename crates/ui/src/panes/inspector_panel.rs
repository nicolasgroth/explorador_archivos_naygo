// Naygo — panel inspector: metadatos básicos del elemento enfocado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Muestra los metadatos que ya tenemos en el `Entry` (nombre, tipo, tamaño,
//! fecha). Las propiedades extendidas del Shell (atributos, propietario, etc.)
//! llegan con `platform::shell` en una fase posterior.

use crate::app::UiState;
use naygo_core::fs_model::EntryKind;

pub fn show(ui: &mut egui::Ui, state: &mut UiState) {
    let Some(entry) = state.pane.focused_entry() else {
        ui.label("Nada seleccionado.");
        return;
    };

    egui::Grid::new("inspector_grid").num_columns(2).show(ui, |ui| {
        ui.strong("Nombre");
        ui.label(&entry.name);
        ui.end_row();

        ui.strong("Tipo");
        ui.label(match entry.kind {
            EntryKind::Directory => "Carpeta",
            EntryKind::File => "Archivo",
            EntryKind::Other => "Otro",
        });
        ui.end_row();

        ui.strong("Ruta");
        ui.label(entry.path.display().to_string());
        ui.end_row();

        if let Some(size) = entry.size {
            ui.strong("Tamaño");
            ui.label(format!("{size} bytes"));
            ui.end_row();
        }
    });
}
