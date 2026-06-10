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
            .exact_size(176.0)
            .show_inside(ui, |ui| {
                ui.add_space(8.0);
                section_item(
                    ui,
                    app,
                    SettingsSection::Appearance,
                    "🎨",
                    "settings.appearance",
                );
                section_item(ui, app, SettingsSection::Panes, "▦", "settings.panes");
                section_item(
                    ui,
                    app,
                    SettingsSection::Shortcuts,
                    "⌨",
                    "settings.shortcuts",
                );
                section_item(
                    ui,
                    app,
                    SettingsSection::Language,
                    "🌐",
                    "settings.language",
                );
                section_item(ui, app, SettingsSection::Advanced, "⚙", "settings.advanced");
                section_item(ui, app, SettingsSection::About, "ℹ", "settings.about");
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

/// Un ítem de la lista de secciones (rediseño A): ícono + texto en una fila de 32 px.
/// El activo lleva barra de acento de 3 px a la izquierda + fondo tintado con el
/// acento del tema + ícono en acento. Hover sutil en los inactivos. Los colores salen
/// SIEMPRE del tema activo (los 4 temas se ven bien).
fn section_item(
    ui: &mut egui::Ui,
    app: &mut NaygoApp,
    section: SettingsSection,
    icon: &str,
    key: &str,
) {
    let selected = app.settings_section == section;
    let label = app.tr(key).to_string();
    let accent = app.active_theme.accent();

    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 32.0), egui::Sense::click());
    if resp.clicked() {
        app.settings_section = section;
    }

    let painter = ui.painter();
    if selected {
        // Fondo tintado: el acento muy atenuado, detrás de toda la fila.
        let tint = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 26);
        painter.rect_filled(rect, 0.0, tint);
        let bar = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + 3.0, rect.bottom()),
        );
        painter.rect_filled(bar, 0.0, accent);
    } else if resp.hovered() {
        painter.rect_filled(rect, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }

    let icon_color = if selected {
        accent
    } else {
        ui.visuals().weak_text_color()
    };
    let text_color = if selected {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    painter.text(
        egui::pos2(rect.left() + 16.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        icon,
        egui::FontId::proportional(15.0),
        icon_color,
    );
    painter.text(
        egui::pos2(rect.left() + 42.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.5),
        text_color,
    );
}

/// Encabezado de una sección (rediseño A): título fuerte + subtítulo débil + aire.
pub(crate) fn section_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(title)
            .size(17.0)
            .color(ui.visuals().strong_text_color()),
    );
    ui.label(egui::RichText::new(subtitle).size(12.0).weak());
    ui.add_space(14.0);
}

/// Etiqueta de grupo (rediseño A): pequeña, débil y en mayúsculas.
pub(crate) fn group_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text.to_uppercase()).size(11.0).weak());
    ui.add_space(6.0);
}

/// Separador entre grupos, con aire arriba y abajo.
pub(crate) fn group_sep(ui: &mut egui::Ui) {
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);
}
