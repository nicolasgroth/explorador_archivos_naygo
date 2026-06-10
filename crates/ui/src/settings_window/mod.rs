// Naygo — ventana de Configuración (viewport separado del SO) con secciones.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! La Configuración es una segunda ventana real del SO (egui multi-viewport).
//! `show_settings_viewport` se llama cada frame de la app principal cuando
//! `settings_open`; usa `show_viewport_immediate` (el closure captura `&mut app`),
//! con un panel lateral (`Panel::left`) de secciones y un `CentralPanel` que
//! despacha a cada una.

mod about;
mod advanced;
mod appearance;
mod language;
mod panes;
mod shortcuts;

use crate::app::NaygoApp;

/// Secciones de la ventana de Configuración.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    Appearance,
    Panes,
    Shortcuts,
    Language,
    Advanced,
    About,
}

/// Abre/pinta el viewport de Configuración. Debe llamarse cada frame mientras
/// `app.settings_open` sea true.
pub fn show_settings_viewport(app: &mut NaygoApp, ctx: &egui::Context) {
    let viewport_id = egui::ViewportId::from_hash_of("naygo_settings");
    let builder = egui::ViewportBuilder::default()
        .with_title(app.tr("settings.title"))
        .with_inner_size([560.0, 420.0])
        .with_min_inner_size([460.0, 360.0])
        .with_close_button(true);

    ctx.show_viewport_immediate(viewport_id, builder, |ui, _class| {
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            app.settings_open = false;
        }

        egui::Panel::left("settings_sections")
            .resizable(false)
            .exact_size(160.0)
            .show_inside(ui, |ui| {
                ui.add_space(6.0);
                section_item(ui, app, SettingsSection::Appearance, "settings.appearance");
                section_item(ui, app, SettingsSection::Panes, "settings.panes");
                section_item(ui, app, SettingsSection::Shortcuts, "settings.shortcuts");
                section_item(ui, app, SettingsSection::Language, "settings.language");
                section_item(ui, app, SettingsSection::Advanced, "settings.advanced");
                section_item(ui, app, SettingsSection::About, "settings.about");
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match app.settings_section {
                SettingsSection::Appearance => appearance::show(ui, app),
                SettingsSection::Panes => panes::show(ui, app),
                SettingsSection::Shortcuts => shortcuts::show(ui, app),
                SettingsSection::Language => language::show(ui, app),
                SettingsSection::Advanced => advanced::show(ui, app),
                SettingsSection::About => about::show(ui, app),
            });
        });
    });
}

/// Un ítem clicable de la lista de secciones (resaltado si es el activo).
fn section_item(ui: &mut egui::Ui, app: &mut NaygoApp, section: SettingsSection, key: &str) {
    let selected = app.settings_section == section;
    let label = app.tr(key);
    if ui.selectable_label(selected, label).clicked() {
        app.settings_section = section;
    }
}
