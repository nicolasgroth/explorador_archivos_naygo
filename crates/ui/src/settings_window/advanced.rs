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

    ui.add_space(12.0);
    ui.heading(app.tr("settings.ops.section"));
    ui.add_space(6.0);

    // Modo de ejecución.
    let (l_mode, l_queue, l_parallel) = (
        app.tr("settings.ops.mode"),
        app.tr("settings.ops.mode.queue"),
        app.tr("settings.ops.mode.parallel"),
    );
    ui.label(l_mode);
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.settings.ops_mode,
            naygo_core::config::OpsMode::Queue,
            l_queue,
        );
        ui.selectable_value(
            &mut app.settings.ops_mode,
            naygo_core::config::OpsMode::Parallel,
            l_parallel,
        );
    });
    ui.add_space(6.0);

    // Cómo se muestra el progreso.
    let (l_display, l_panel, l_modal, l_always) = (
        app.tr("settings.ops.display"),
        app.tr("settings.ops.display.panel"),
        app.tr("settings.ops.display.modal"),
        app.tr("settings.ops.display.always"),
    );
    ui.label(l_display);
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.settings.ops_display,
            naygo_core::config::OpsDisplay::Panel,
            l_panel,
        );
        ui.selectable_value(
            &mut app.settings.ops_display,
            naygo_core::config::OpsDisplay::Modal,
            l_modal,
        );
        ui.selectable_value(
            &mut app.settings.ops_display,
            naygo_core::config::OpsDisplay::AlwaysVisible,
            l_always,
        );
    });
    ui.add_space(6.0);

    // Checkboxes.
    let l_confirm_trash = app.tr("settings.ops.confirm_trash");
    let mut confirm_trash = app.settings.confirm_trash;
    if ui.checkbox(&mut confirm_trash, l_confirm_trash).changed() {
        app.settings.confirm_trash = confirm_trash;
    }
    let l_show_summary = app.tr("settings.ops.show_summary");
    let mut show_summary = app.settings.show_op_summary;
    if ui.checkbox(&mut show_summary, l_show_summary).changed() {
        app.settings.show_op_summary = show_summary;
    }
}
