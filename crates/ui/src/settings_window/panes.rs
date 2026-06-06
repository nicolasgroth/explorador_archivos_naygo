// Naygo — sección Paneles de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::config::BarPosition;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let title = app.tr("settings.panes");
    ui.heading(title);
    ui.add_space(8.0);

    let mut show_parent = app.settings.show_parent_entry;
    let lbl = app.tr("settings.show_parent");
    if ui.checkbox(&mut show_parent, lbl).changed() {
        app.settings.show_parent_entry = show_parent;
    }
    ui.add_space(8.0);

    ui.label(app.tr("settings.bar_position"));
    let (l_top, l_side) = (app.tr("settings.bar.top"), app.tr("settings.bar.side"));
    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.settings.bar_position, BarPosition::Top, l_top);
        ui.selectable_value(&mut app.settings.bar_position, BarPosition::Side, l_side);
    });
}
