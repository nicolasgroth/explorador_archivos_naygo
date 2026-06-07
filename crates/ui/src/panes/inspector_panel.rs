// Naygo — inspector: metadatos del elemento enfocado en el panel Files activo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Refleja el panel `Files` ACTIVO: muestra los metadatos del elemento enfocado.
//! Las propiedades extendidas del Shell llegan con `platform::shell` (fase futura).

use naygo_core::fs_model::EntryKind;
use naygo_core::workspace::Workspace;

pub fn show(ui: &mut egui::Ui, workspace: &mut Workspace, i18n: &naygo_core::i18n::I18n) {
    let Some(entry) = workspace
        .active_files()
        .and_then(|f| f.focused_view_entry())
    else {
        ui.label(i18n.t("inspector.nothing"));
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
            ui.strong(i18n.t("col.name"));
            ui.label(&name);
            ui.end_row();
            ui.strong(i18n.t("inspector.type"));
            ui.label(match kind {
                EntryKind::Directory => i18n.t("kind.folder"),
                EntryKind::File => i18n.t("kind.file"),
                EntryKind::Other => i18n.t("kind.other"),
            });
            ui.end_row();
            ui.strong(i18n.t("inspector.path"));
            ui.label(path.display().to_string());
            ui.end_row();
            if let Some(s) = size {
                ui.strong(i18n.t("col.size"));
                ui.label(format!("{s} bytes"));
                ui.end_row();
            }
        });
}
