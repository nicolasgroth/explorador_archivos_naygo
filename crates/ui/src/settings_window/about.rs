// Naygo — sección "Acerca de" de la Configuración (autoría + easter egg).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Acerca de: logo, versión, autoría (Nicolás Groth / ISGroth) y licencia MIT.
//! Esconde un easter egg DINÁMICO: 5 clics seguidos sobre el logo disparan ~8 s de
//! "lluvia de Naygo" (íconos de carpeta cayendo, del set activo) con un mensaje que
//! se escribe letra a letra. Bajo consumo: solo repinta mientras el egg está activo.

use crate::app::NaygoApp;
use naygo_core::icon_kind::IconKey;
use std::time::{Duration, Instant};

/// Ventana de clics para encadenar el contador del egg.
const EGG_CLICK_WINDOW: Duration = Duration::from_secs(2);
/// Clics seguidos sobre el logo que disparan el egg.
const EGG_CLICKS: u8 = 5;
/// Duración del egg una vez disparado.
const EGG_DURATION: Duration = Duration::from_secs(8);

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    // Logo embebido (el mismo del splash), cargado una sola vez.
    if app.about_logo.is_none() {
        app.about_logo = load_logo(ui.ctx());
    }

    ui.vertical_centered(|ui| {
        ui.add_space(16.0);
        if let Some(tex) = &app.about_logo {
            let img = ui.add(
                egui::Image::new(tex)
                    .max_width(220.0)
                    .sense(egui::Sense::click()),
            );
            if img.clicked() {
                register_egg_click(app);
            }
        } else {
            ui.heading("Naygo");
        }
        ui.add_space(8.0);
        ui.label(
            app.tr("about.version")
                .replace("{v}", env!("CARGO_PKG_VERSION")),
        );
        ui.add_space(12.0);
        ui.label(egui::RichText::new(app.tr("about.author")).strong());
        ui.label(app.tr("about.company"));
        ui.add_space(8.0);
        ui.label(app.tr("about.license"));
        ui.label(egui::RichText::new(app.tr("about.stack")).weak());
        ui.add_space(8.0);
        ui.hyperlink_to(
            app.tr("about.repo"),
            "https://github.com/nicolasgroth/explorador_archivos_naygo",
        );
        ui.add_space(16.0);
    });

    paint_egg(ui, app);
}

/// Cuenta un clic del logo; 5 seguidos (ventana de 2 s) disparan el egg.
fn register_egg_click(app: &mut NaygoApp) {
    let now = Instant::now();
    let chained = app
        .egg_last_click
        .is_some_and(|t| now.duration_since(t) < EGG_CLICK_WINDOW);
    app.egg_clicks = if chained { app.egg_clicks + 1 } else { 1 };
    app.egg_last_click = Some(now);
    if app.egg_clicks >= EGG_CLICKS {
        app.egg_clicks = 0;
        app.egg_until = Some(now + EGG_DURATION);
        tracing::debug!("easter egg activado");
    }
}

/// Pseudoaleatorio determinista en [0,1) a partir de un índice (sin crate `rand`).
fn frac(seed: f32) -> f32 {
    let h = (seed * 12.9898).sin() * 43758.547;
    h - h.floor()
}

/// Pinta la "lluvia de Naygo" + el mensaje tipeado mientras el egg esté activo.
fn paint_egg(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let Some(until) = app.egg_until else { return };
    let now = Instant::now();
    if now >= until {
        app.egg_until = None;
        return;
    }

    let area = ui.clip_rect();
    let t = ui.input(|i| i.time) as f32;
    let tex_id = app.icons.texture(IconKey::Folder).id();
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    let painter = ui.painter();
    for i in 0..28 {
        let fi = i as f32;
        let x = area.left() + frac(fi + 0.17) * area.width();
        let speed = 60.0 + frac(fi + 0.43) * 140.0;
        let drop = (t * speed + frac(fi + 0.71) * 900.0) % (area.height() + 48.0);
        let y = area.top() + drop - 24.0;
        let size = 14.0 + frac(fi + 0.29) * 14.0;
        // Tenue balanceo horizontal para que la caída se sienta viva.
        let sway = (t * 2.0 + fi).sin() * 6.0;
        let rect = egui::Rect::from_center_size(egui::pos2(x + sway, y), egui::vec2(size, size));
        painter.image(tex_id, rect, uv, egui::Color32::WHITE);
    }

    // Mensaje que se escribe letra a letra.
    let msg = app.tr("about.egg_message");
    let elapsed = EGG_DURATION
        .saturating_sub(until.duration_since(now))
        .as_secs_f32();
    let shown_n = ((elapsed * 14.0) as usize).min(msg.chars().count());
    let shown: String = msg.chars().take(shown_n).collect();
    painter.text(
        egui::pos2(area.center().x, area.bottom() - 28.0),
        egui::Align2::CENTER_CENTER,
        shown,
        egui::FontId::proportional(16.0),
        ui.visuals().strong_text_color(),
    );

    // El egg anima: repintar mientras viva (y solo mientras viva).
    ui.ctx().request_repaint();
}

/// Decodifica el logo embebido a una textura. Tolerante: `None` si falla (la sección
/// muestra el nombre en texto y la app sigue normal).
fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let bytes = include_bytes!("../../../../assets/icons/logo_naygo.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture("naygo_about_logo", color, egui::TextureOptions::LINEAR))
}
