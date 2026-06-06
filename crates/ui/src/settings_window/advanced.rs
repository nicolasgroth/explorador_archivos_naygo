// Naygo — sección Avanzado de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let title = app.tr("settings.advanced");
    ui.heading(title);
    ui.add_space(8.0);
    let (l_dir, dir) = (app.tr("settings.config_dir"), app.config_dir_display());
    ui.horizontal(|ui| {
        ui.label(l_dir);
        ui.monospace(dir);
    });
    let l_ver = app.tr("settings.version");
    ui.horizontal(|ui| {
        ui.label(l_ver);
        ui.monospace(env!("CARGO_PKG_VERSION"));
    });
}
