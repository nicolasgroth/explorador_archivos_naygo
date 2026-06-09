// Naygo — splash de arranque: muestra el logo brevemente al iniciar (solo release).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Estado liviano que pinta el logo de Naygo centrado durante el arranque. Se cierra
//! solo tras `MAX_VISIBLE` o cuando el usuario interactúa (clic/tecla), lo que pase
//! primero. No frena el arranque: como Naygo arranca rápido, en la práctica es un
//! destello corto. Tolerante: si el logo no carga, no hay splash.

use std::time::{Duration, Instant};

/// Tiempo máximo visible del splash.
const MAX_VISIBLE: Duration = Duration::from_millis(1200);

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

    /// Pinta el splash y devuelve `true` si debe seguir visible. Devuelve `false`
    /// cuando expira el tiempo o hubo input (clic/tecla).
    pub fn show(&self, ui: &mut egui::Ui) -> bool {
        let ctx = ui.ctx().clone();
        let input_dismiss = ctx.input(|i| {
            i.pointer.any_click()
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { .. }))
        });
        ui.centered_and_justified(|ui| {
            let max = egui::vec2(ui.available_width() * 0.6, ui.available_height() * 0.6);
            ui.add(egui::Image::new(&self.texture).max_size(max));
        });
        // Repaint para que el tiempo avance aunque no haya input.
        ctx.request_repaint();
        let expired = self.started.elapsed() >= MAX_VISIBLE;
        !(expired || input_dismiss)
    }
}
