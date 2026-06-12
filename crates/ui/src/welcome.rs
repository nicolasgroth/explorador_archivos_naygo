// Naygo — diálogo de bienvenida del primer arranque (elección del modo de consumo).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Se muestra UNA vez, en el primer arranque (no existía settings.json). Pregunta cómo
//! quiere el usuario que Naygo use los recursos y setea `low_power_mode`. Modal ligero
//! (no bloquea workers). Devuelve la elección, o `None` si el usuario aún no decidió.

use naygo_core::config::LowPowerMode;

/// Pinta el diálogo de bienvenida centrado, con un backdrop oscurecido. Devuelve
/// `Some(modo)` cuando el usuario elige (el caller baja `show_welcome` y persiste), o
/// `None` si sigue abierto.
pub fn show(
    ctx: &egui::Context,
    i18n: &naygo_core::i18n::I18n,
    theme: &crate::theme_apply::ActiveTheme,
) -> Option<LowPowerMode> {
    let mut chosen: Option<LowPowerMode> = None;
    // Backdrop semitransparente sobre toda la ventana (modal manual; egui 0.34 no garantiza
    // `Modal` en esta versión, así que se arma con Area + Window centrada).
    egui::Area::new(egui::Id::new("naygo_welcome_backdrop"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            let r = ctx.content_rect();
            ui.painter()
                .rect_filled(r, 0.0, egui::Color32::from_black_alpha(160));
        });
    egui::Window::new(i18n.t("welcome.title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(i18n.t("welcome.body"));
            ui.add_space(12.0);
            // Botón recomendado (Automático) en acento; los otros, normales.
            let auto = egui::Button::new(
                egui::RichText::new(i18n.t("welcome.auto"))
                    .color(theme.accent())
                    .strong(),
            );
            if ui.add(auto).clicked() {
                chosen = Some(LowPowerMode::Auto);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button(i18n.t("welcome.low")).clicked() {
                    chosen = Some(LowPowerMode::Always);
                }
                if ui.button(i18n.t("welcome.full")).clicked() {
                    chosen = Some(LowPowerMode::Never);
                }
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new(i18n.t("welcome.hint")).weak().small());
        });
    chosen
}
