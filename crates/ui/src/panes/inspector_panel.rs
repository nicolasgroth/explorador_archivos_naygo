// Naygo — inspector: metadatos del elemento enfocado en el panel Files activo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Refleja el panel `Files` ACTIVO: muestra los metadatos del elemento enfocado.
//! Las propiedades extendidas del Shell llegan con `platform::shell` (fase futura).

use naygo_core::fs_model::EntryKind;
use naygo_core::workspace::Workspace;

pub fn show(ui: &mut egui::Ui, workspace: &mut Workspace) {
    let Some(entry) = workspace.active_files().and_then(|f| f.focused_entry()) else {
        ui.label("Nada seleccionado.");
        return;
    };
    let (name, kind, path, size) = (
        entry.name.clone(),
        entry.kind,
        entry.path.clone(),
        entry.size,
    );

    egui::Grid::new("inspector_grid")
        .num_columns(2)
        .show(ui, |ui| {
            ui.strong("Nombre");
            ui.label(&name);
            ui.end_row();
            ui.strong("Tipo");
            ui.label(match kind {
                EntryKind::Directory => "Carpeta",
                EntryKind::File => "Archivo",
                EntryKind::Other => "Otro",
            });
            ui.end_row();
            ui.strong("Ruta");
            ui.label(path.display().to_string());
            ui.end_row();
            if let Some(s) = size {
                ui.strong("Tamaño");
                ui.label(format!("{s} bytes"));
                ui.end_row();
            }
        });
}
