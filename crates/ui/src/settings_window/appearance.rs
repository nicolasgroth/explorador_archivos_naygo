// Naygo — sección Apariencia de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;

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
        ui.selectable_value(&mut app.settings.icon_set, "flat".to_string(), l_flat);
        ui.selectable_value(&mut app.settings.icon_set, "fluent".to_string(), l_fluent);
        ui.selectable_value(&mut app.settings.icon_set, "mono".to_string(), l_mono);
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
