// Naygo — inicialización de logging a archivo (sin telemetría).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Logging básico a archivo desde el día uno. Escribe a `logs/naygo.log` junto
//! al ejecutable usando un appender con rotación diaria. Sin telemetría, sin red.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Inicializa el subscriber global. Devuelve un guard que debe vivir tanto como
/// el programa (si se dropea, se pierden logs pendientes en el buffer).
pub fn init() -> WorkerGuard {
    let file_appender = tracing_appender::rolling::daily("logs", "naygo.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Por defecto: info general + debug de nuestros crates + WARN de las capas gráficas
    // (wgpu/winit/eframe), para capturar errores de dispositivo/superficie que podrían
    // cerrar la app sin un panic de Rust. Override con la variable RUST_LOG si hace falta.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,naygo_core=debug,naygo_ui=debug,wgpu=warn,wgpu_core=warn,wgpu_hal=warn,\
             winit=warn,eframe=debug,egui_wgpu=debug",
        )
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    tracing::info!("Naygo iniciado");
    guard
}
