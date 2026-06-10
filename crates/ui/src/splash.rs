// Naygo — splash de arranque: ventana propia sin bordes con el logo (solo release).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Splash de arranque como VENTANA propia del SO sin bordes (deferred viewport),
//! del tamaño del logo y centrada. Evita el "cuadrado flotante": el logo (con su
//! fondo claro horneado) llena toda la ventana, sin borde oscuro del tema. Se cierra
//! solo tras `MAX_VISIBLE` o al primer input (clic/tecla), lo que pase primero. No
//! frena el arranque: la app principal sigue su lógica normal por detrás. Tolerante:
//! si el logo no carga, no hay splash y la app arranca igual.

use std::time::{Duration, Instant};

/// Tiempo máximo visible del splash.
const MAX_VISIBLE: Duration = Duration::from_millis(1200);

/// Lado (px lógicos) de la ventana del splash. El logo es cuadrado (1254²); se
/// escala a este tamaño para una ventana compacta y nítida.
const SPLASH_SIZE: f32 = 360.0;

/// Id estable del viewport del splash (compartido con `app.rs` para cerrarlo).
pub fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("naygo_splash")
}

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

    /// Pinta el splash como una VENTANA propia sin bordes (deferred viewport), del
    /// tamaño del logo. Devuelve `true` si debe seguir visible; `false` cuando expira
    /// el tiempo o hubo input (clic/tecla) → el llamador limpia el estado y cierra el
    /// viewport. Recibe el `ctx` de la app principal (no se puede pintar en `logic`,
    /// pero `show_viewport_deferred` solo registra el viewport: el pintado real ocurre
    /// dentro del closure, en el contexto del viewport hijo).
    pub fn show(&self, ctx: &egui::Context) -> bool {
        // Input que descarta el splash: clic o cualquier tecla, en CUALQUIER viewport.
        let expired = self.started.elapsed() >= MAX_VISIBLE;
        let dismissed = ctx.input(|i| {
            i.pointer.any_click()
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { .. }))
        });
        if expired || dismissed {
            return false;
        }

        let builder = egui::ViewportBuilder::default()
            .with_title("Naygo")
            .with_inner_size([SPLASH_SIZE, SPLASH_SIZE])
            .with_decorations(false) // sin barra de título ni borde del SO
            .with_resizable(false)
            .with_taskbar(false) // no aparece en la barra de tareas
            .with_always_on_top();

        let tex = self.texture.clone();
        ctx.show_viewport_deferred(viewport_id(), builder, move |ui, _class| {
            // El closure recibe el `Ui` raíz del viewport hijo (egui 0.34). Pintamos un
            // CentralPanel SIN margen ni fondo del tema (`Frame::NONE`): el logo cubre
            // toda la ventana, así no hay borde oscuro ni "cuadrado flotante".
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show_inside(ui, |ui| {
                    let size = ui.available_size();
                    ui.add(egui::Image::new(&tex).fit_to_exact_size(size));
                });
            // Mantener el viewport repintando para que su timer (en el ctx padre) avance.
            ui.ctx().request_repaint();
        });

        // Que el ctx PADRE siga corriendo: así el timer del splash avanza aunque no
        // haya input y la app principal sigue cargando por detrás.
        ctx.request_repaint();
        true
    }
}
