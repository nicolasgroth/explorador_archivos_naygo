// Naygo — splash de arranque: el logo a pantalla completa (solo release).
// Copyright (c) 2026 Nicolás Groth / ISGroth.
// Autor: Nicolás Groth <ngroth@gmail.com> — ISGroth. MIT License.

//! Splash de arranque pintado DENTRO de la ventana principal: un panel a pantalla
//! completa con el fondo del logo (claro) y el logo centrado. No usa un viewport
//! aparte (eso impedía que la ventana principal se presentara). Evita el "cuadrado
//! flotante" y el borde oscuro: TODO el panel se pinta con el color de fondo del logo,
//! así el logo se funde con el fondo en vez de verse como una tarjeta sobre el tema
//! oscuro. Se cierra solo tras `MAX_VISIBLE` o al primer input (clic/tecla), lo que
//! pase primero. Tolerante: si el logo no carga, no hay splash y la app arranca igual.

use std::time::{Duration, Instant};

/// Tiempo máximo visible del splash.
const MAX_VISIBLE: Duration = Duration::from_millis(1200);

/// Color de fondo del logo (esquina del PNG, ~blanco). El panel entero se pinta de
/// este color para que el logo no se vea como un cuadro flotando sobre el tema oscuro.
const LOGO_BG: egui::Color32 = egui::Color32::from_rgb(249, 249, 249);

/// Estado del splash mientras está activo.
pub struct Splash {
    texture: egui::TextureHandle,
    started: Instant,
}

impl Splash {
    /// Crea el splash si el logo decodifica; si no, `None` (no hay splash).
    // Solo se invoca en release (en debug no hay splash), así que en build debug
    // queda sin usar: silenciamos el dead_code SOLO en debug.
    #[cfg_attr(debug_assertions, allow(dead_code))]
    pub fn new(ctx: &egui::Context) -> Option<Self> {
        let bytes = include_bytes!("../../../assets/icons/logo_naygo.png");
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        let texture = ctx.load_texture("naygo_splash", color, egui::TextureOptions::LINEAR);
        Some(Splash {
            texture,
            started: Instant::now(),
        })
    }

    /// Pinta el splash a pantalla completa en la ventana principal. Devuelve `true` si
    /// debe seguir visible; `false` cuando expira el tiempo o hubo input (clic/tecla),
    /// y ahí el llamador limpia el estado y deja pasar a la UI real.
    pub fn show(&self, ui: &mut egui::Ui) -> bool {
        let ctx = ui.ctx().clone();
        let dismissed = ctx.input(|i| {
            i.pointer.any_click()
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { .. }))
        });

        // Panel a pantalla completa con el fondo del logo (sin borde oscuro / sin
        // cuadrado flotante): el logo centrado se funde con el fondo claro.
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(LOGO_BG))
            .show_inside(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    // El logo a ~60% del lado menor, centrado y nítido.
                    let side = ui.available_width().min(ui.available_height()) * 0.6;
                    ui.add(egui::Image::new(&self.texture).fit_to_exact_size(egui::vec2(side, side)));
                });
            });

        // Repaint para que el tiempo avance aunque no haya input.
        ctx.request_repaint();
        let expired = self.started.elapsed() >= MAX_VISIBLE;
        !(expired || dismissed)
    }
}
