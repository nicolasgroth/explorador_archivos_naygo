// Naygo — sección Paneles de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::config::{BarPosition, ColumnWidthMode};

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (title, sub) = (app.tr("settings.panes"), app.tr("settings.panes.sub"));
    super::section_header(ui, &title, &sub);

    let mut show_parent = app.settings.show_parent_entry;
    let lbl = app.tr("settings.show_parent");
    if ui.checkbox(&mut show_parent, lbl).changed() {
        app.settings.show_parent_entry = show_parent;
    }
    ui.add_space(8.0);

    let accent = app.active_theme.accent();

    ui.label(app.tr("settings.bar_position"));
    let (l_top, l_side) = (app.tr("settings.bar.top"), app.tr("settings.bar.side"));
    crate::widgets::segmented(
        ui,
        &mut app.settings.bar_position,
        &[
            (BarPosition::Top, l_top.as_str()),
            (BarPosition::Side, l_side.as_str()),
        ],
        accent,
    );
    ui.add_space(8.0);

    // Ancho de columnas: fijo (resizable a mano) vs automático (la tabla reparte por
    // contenido). Afecta a todos los file panels en caliente.
    ui.label(app.tr("settings.column_width"));
    let (l_fixed, l_auto) = (
        app.tr("settings.column_width.fixed"),
        app.tr("settings.column_width.auto"),
    );
    crate::widgets::segmented(
        ui,
        &mut app.settings.column_width_mode,
        &[
            (ColumnWidthMode::Fixed, l_fixed.as_str()),
            (ColumnWidthMode::Auto, l_auto.as_str()),
        ],
        accent,
    );
    ui.add_space(8.0);

    // Guardar la tabla del panel activo (columnas visibles, orden y anchos) como plantilla
    // para los paneles NUEVOS. Sin panel activo el botón se deshabilita.
    let btn = app.tr("settings.save_default_table");
    let hint = app.tr("settings.save_default_table.hint");
    let active_table = app.workspace.active_files().map(|f| f.table.clone());
    let resp = ui.add_enabled(active_table.is_some(), egui::Button::new(btn));
    if resp.on_hover_text(&hint).clicked() {
        if let Some(table) = active_table {
            app.settings.default_table = Some(table);
            app.status = app.tr("status.default_table_saved");
        }
    }
    // Indicador de si ya hay una plantilla guardada (+ botón para limpiarla).
    if app.settings.default_table.is_some() {
        let set_lbl = app.tr("settings.default_table.set");
        let clear_lbl = app.tr("settings.default_table.clear");
        ui.horizontal(|ui| {
            ui.weak(set_lbl);
            if ui.small_button(clear_lbl).clicked() {
                app.settings.default_table = None;
                app.status = app.tr("status.default_table_cleared");
            }
        });
    }
}
