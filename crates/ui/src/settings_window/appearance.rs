// Naygo — sección Apariencia de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    ui.heading(app.tr("settings.appearance"));
    ui.add_space(8.0);

    // Sets disponibles del catálogo: los 3 embebidos + packs sueltos del usuario.
    let catalog = naygo_core::icon_set::IconSetCatalog::load(app.config_dir());
    let items: Vec<(String, String)> = catalog
        .available()
        .iter()
        .map(|info| {
            // Embebidos: etiqueta i18n. Sueltos: el nombre de la carpeta.
            let label = match info.id.as_str() {
                "flat" => app.tr("settings.icons.flat"),
                "fluent" => app.tr("settings.icons.fluent"),
                "mono" => app.tr("settings.icons.mono"),
                _ => info.label.clone(),
            };
            (info.id.clone(), label)
        })
        .collect();
    ui.label(app.tr("settings.icon_set"));
    ui.horizontal(|ui| {
        for (id, label) in &items {
            ui.selectable_value(&mut app.settings.icon_set, id.clone(), label);
        }
    });
    ui.add_space(8.0);

    // Sección Barra de herramientas: estilo de íconos + color de los glifos.
    ui.separator();
    let sec = app.tr("settings.toolbar.section");
    let lbl_style = app.tr("settings.toolbar.style");
    let lbl_glyphs = app.tr("settings.toolbar.glyphs");
    let lbl_pack = app.tr("settings.toolbar.pack");
    let lbl_color = app.tr("settings.toolbar.glyph_color");
    let lbl_use_theme = app.tr("settings.toolbar.use_theme_color");
    ui.label(sec);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(lbl_style);
        ui.selectable_value(
            &mut app.settings.toolbar_icon_style,
            naygo_core::config::ToolbarIconStyle::Glyphs,
            lbl_glyphs,
        );
        ui.selectable_value(
            &mut app.settings.toolbar_icon_style,
            naygo_core::config::ToolbarIconStyle::Pack,
            lbl_pack,
        );
    });
    ui.horizontal(|ui| {
        ui.label(lbl_color);
        let mut use_theme = app.settings.toolbar_glyph_color.is_none();
        if ui.checkbox(&mut use_theme, lbl_use_theme).changed() {
            app.settings.toolbar_glyph_color = if use_theme {
                None
            } else {
                Some(naygo_core::theme::ThemeColor::new(0x2f, 0x81, 0xf7))
            };
        }
        if let Some(tc) = app.settings.toolbar_glyph_color {
            let mut c = egui::Color32::from_rgb(tc.r, tc.g, tc.b);
            if ui.color_edit_button_srgba(&mut c).changed() {
                app.settings.toolbar_glyph_color =
                    Some(naygo_core::theme::ThemeColor::new(c.r(), c.g(), c.b()));
            }
        }
    });
    ui.add_space(8.0);

    ui.label(app.tr("settings.theme.section"));
    ui.add_space(4.0);
    let current = app.settings.theme.clone();
    let cards = app.theme_cards();
    let mut chosen: Option<naygo_core::theme::ThemeId> = None;
    ui.horizontal_wrapped(|ui| {
        for (id, theme) in &cards {
            let is_active = *id == current;
            if theme_card(ui, theme, is_active).clicked() {
                chosen = Some(id.clone());
            }
        }
    });
    if let Some(id) = chosen {
        app.settings.theme = id; // el hot-swap del próximo frame lo aplica
    }
    ui.add_space(10.0);

    ui.label(app.tr("settings.packs.section"));
    ui.label(egui::RichText::new(app.tr("settings.packs.hint")).weak());
    ui.add_space(4.0);
    let packs = app.packs();
    let mut chosen_pack: Option<naygo_core::theme::pack::Pack> = None;
    ui.horizontal_wrapped(|ui| {
        for pack in &packs {
            if ui.button(pack.name.as_str()).clicked() {
                chosen_pack = Some(pack.clone());
            }
        }
    });
    if let Some(p) = chosen_pack {
        app.apply_pack(&p);
    }
    ui.add_space(10.0);

    let mut icon_only = app.settings.icon_only;
    let lbl = app.tr("settings.icon_only");
    if ui.checkbox(&mut icon_only, lbl).changed() {
        app.settings.icon_only = icon_only;
    }
}

/// Pinta una tarjeta de tema: 3 swatches (panel/selección/acento) + nombre. Borde de
/// acento si es el activo. Devuelve el Response clicable.
fn theme_card(ui: &mut egui::Ui, theme: &naygo_core::theme::Theme, active: bool) -> egui::Response {
    use crate::theme_apply::to_color32;
    let desired = egui::vec2(92.0, 56.0);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, to_color32(theme.panel_bg));
    let sw_h = 18.0;
    let third = rect.width() / 3.0;
    for (i, c) in [theme.panel_bg, theme.selection_bg, theme.accent]
        .iter()
        .enumerate()
    {
        let x0 = rect.left() + third * i as f32;
        let sr = egui::Rect::from_min_size(egui::pos2(x0, rect.top()), egui::vec2(third, sw_h));
        painter.rect_filled(sr, 0.0, to_color32(*c));
    }
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 12.0),
        egui::Align2::CENTER_CENTER,
        &theme.name,
        egui::FontId::proportional(12.0),
        to_color32(theme.text),
    );
    let stroke = if active {
        egui::Stroke::new(2.0, to_color32(theme.accent))
    } else {
        egui::Stroke::new(1.0, to_color32(theme.border))
    };
    painter.rect_stroke(rect, 6.0, stroke, egui::StrokeKind::Inside);
    resp
}
