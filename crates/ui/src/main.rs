// Naygo — explorador de archivos rápido para Windows. Entrypoint.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Naygo — un explorador de archivos estilo Commander.
// Autor: Nicolás Groth / ISGroth (Chile). Licencia MIT.

// En release, no abrir consola en Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod docking;
mod input;
mod logging;
mod panes;
mod typeahead;

use app::NaygoApp;

fn main() -> eframe::Result<()> {
    let _log_guard = logging::init();
    install_panic_handler();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Naygo")
            .with_inner_size([1100.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Naygo",
        native_options,
        Box::new(|cc| Ok(Box::new(NaygoApp::new(cc)))),
    )
}

/// Captura panics: los loguea en vez de morir en silencio. Sigue el principio del
/// spec: la app comunica el fallo, no desaparece sin rastro.
fn install_panic_handler() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("PANIC: {info}");
        default_hook(info);
    }));
}
