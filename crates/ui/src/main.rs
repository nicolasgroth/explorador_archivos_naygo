// Naygo — explorador de archivos rápido para Windows. Entrypoint.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Naygo — un explorador de archivos estilo Commander.
// Autor: Nicolás Groth / ISGroth (Chile). Licencia MIT.

// En release, no abrir consola en Windows — EXCEPTO si se compila con la feature
// `console` (build de diagnóstico): ahí se deja la consola para ver errores de wgpu/
// abort que cierran la app sin panic de Rust.
#![cfg_attr(
    all(not(debug_assertions), not(feature = "console")),
    windows_subsystem = "windows"
)]

mod app;
mod batch_rename_dialog;
mod column_menu;
mod dock_translate;
mod docking;
mod icons;
mod input;
mod logging;
mod ops_actions;
mod ops_dialogs;
mod ops_panel;
mod panes;
mod settings_window;
mod sort_ui;
mod splash;
mod table_actions;
mod templates_menu;
mod theme_apply;
mod toolbar;
mod tray;
mod tree_actions;
mod typeahead;

use app::NaygoApp;

fn main() -> eframe::Result<()> {
    let _log_guard = logging::init();
    install_panic_handler();

    // Carpeta inicial opcional: naygo.exe <ruta> abre esa carpeta.
    let args: Vec<String> = std::env::args().collect();
    let initial_dir = naygo_core::cli::parse_initial_dir(args.get(1..).unwrap_or(&[]));
    if let Some(d) = &initial_dir {
        tracing::info!("carpeta inicial por CLI: {}", d.display());
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Naygo")
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size([640.0, 400.0]);
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Naygo",
        native_options,
        Box::new(move |cc| Ok(Box::new(NaygoApp::new(cc, initial_dir.clone())))),
    )
}

/// Carga el ícono de la ventana desde el PNG del logo embebido. Si falla la
/// decodificación, devuelve None (la app arranca igual, sin ícono de ventana).
fn load_window_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../../../assets/icons/logo_naygo.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = (img.width(), img.height());
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

/// Captura panics: los loguea en vez de morir en silencio. Sigue el principio del
/// spec: la app comunica el fallo, no desaparece sin rastro.
fn install_panic_handler() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // El log normal usa un appender en hilo aparte (non_blocking): si el proceso
        // muere por el panic, el último mensaje se puede perder antes de vaciarse el
        // buffer. Por eso ESCRIBIMOS EL PANIC TAMBIÉN de forma SÍNCRONA a un archivo
        // dedicado (flush inmediato), para que un crash de arranque quede registrado.
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "?".to_string());
        let _ = std::fs::create_dir_all("logs");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("logs/naygo-panic.log")
        {
            use std::io::Write;
            let _ = writeln!(f, "PANIC en {loc}: {info}");
            let _ = f.flush();
        }
        tracing::error!("PANIC: {info}");
        default_hook(info);
    }));
}
