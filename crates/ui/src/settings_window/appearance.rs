// Naygo — sección Apariencia de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::config::IconSet;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    ui.heading(app.tr("settings.appearance"));
    ui.add_space(8.0);

    ui.label(app.tr("settings.icon_set"));
    let (l_flat, l_fluent, l_mono) = (
        app.tr("settings.icons.flat"),
        app.tr("settings.icons.fluent"),
        app.tr("settings.icons.mono"),
    );
    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.settings.icon_set, IconSet::Flat, l_flat);
        ui.selectable_value(&mut app.settings.icon_set, IconSet::Fluent, l_fluent);
        ui.selectable_value(&mut app.settings.icon_set, IconSet::Mono, l_mono);
    });
    ui.add_space(8.0);

    ui.label(app.tr("settings.theme"));
    let placeholder = app.tr("settings.theme.placeholder");
    ui.label(egui::RichText::new(placeholder).weak());
    ui.add_space(8.0);

    let mut icon_only = app.settings.icon_only;
    let lbl = app.tr("settings.icon_only");
    if ui.checkbox(&mut icon_only, lbl).changed() {
        app.settings.icon_only = icon_only;
    }
}
