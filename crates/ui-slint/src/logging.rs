// Naygo — logging básico a archivo + panic handler (diagnóstico de caídas).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Sin telemetría ni red: solo un archivo de texto JUNTO al ejecutable (el mismo
// `portable_dir()` donde vive settings.json), para poder diagnosticar caídas en VMs y
// equipos limpios. El panic handler captura el mensaje, la ubicación y el backtrace, los
// escribe al log y los muestra en un diálogo nativo, en vez de cerrarse en silencio.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;

/// Nombre del archivo de log (en el directorio portable, junto a naygo.exe).
const LOG_FILE: &str = "naygo.log";
/// Si el log supera este tamaño, se trunca al arrancar (evita que crezca sin límite).
const LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Ruta del archivo de log: junto al ejecutable (mismo criterio que la config portable).
pub fn log_path() -> PathBuf {
    naygo_core::config::portable_dir().join(LOG_FILE)
}

/// Inicializa el logging: trunca el log si está muy grande, escribe una línea de arranque e
/// instala el panic handler. Llamar UNA vez al inicio de `main`, antes de crear la ventana.
pub fn init() {
    let path = log_path();
    // Truncado por tamaño: si el log viejo es enorme, empezar de cero.
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > LOG_MAX_BYTES {
            let _ = std::fs::remove_file(&path);
        }
    }
    log_line(&format!(
        "=== Naygo v{} arrancó ===",
        env!("CARGO_PKG_VERSION")
    ));
    install_panic_hook();
}

/// Anexa una línea al log (con marca de tiempo monotónica relativa). Nunca falla hacia
/// afuera: si no se puede escribir (disco lleno, permiso), se ignora en silencio.
pub fn log_line(msg: &str) {
    let line = format!("[{}] {msg}\n", timestamp());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Marca de tiempo legible para el log. Usa la hora del sistema (UTC); si no está
/// disponible, cae a un contador vacío. No usa crates externos.
fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => {
            // Formato simple: segundos desde epoch (suficiente para correlacionar eventos).
            let secs = d.as_secs();
            let ms = d.subsec_millis();
            format!("{secs}.{ms:03}")
        }
        Err(_) => "?".to_string(),
    }
}

/// Instala un panic hook que escribe el panic (mensaje + ubicación + backtrace) al log y
/// muestra un diálogo nativo, para que una caída NO sea un cierre silencioso. Conserva el
/// hook anterior (lo llama después) por si el entorno imprime a stderr.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut report = String::new();
        let _ = writeln!(report, "*** PANIC ***");
        // Mensaje del panic.
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(sin mensaje)".to_string());
        let _ = writeln!(report, "mensaje: {payload}");
        // Ubicación (archivo:línea).
        if let Some(loc) = info.location() {
            let _ = writeln!(report, "en: {}:{}:{}", loc.file(), loc.line(), loc.column());
        }
        // Backtrace (requiere RUST_BACKTRACE=1, pero lo forzamos a capturar igual).
        let bt = std::backtrace::Backtrace::force_capture();
        let _ = writeln!(report, "backtrace:\n{bt}");

        log_line(&report);

        // Diálogo nativo: el usuario VE que pasó algo y dónde está el log, en vez de un
        // cierre fantasma. (rfd ya es dependencia; el diálogo es síncrono y bloqueante.)
        let log = log_path();
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Naygo — error inesperado")
            .set_description(format!(
                "Naygo encontró un error y debe cerrarse.\n\n{payload}\n\nSe guardó un \
                 registro en:\n{}",
                log.display()
            ))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();

        // Llamar al hook previo (por si imprime a stderr en debug).
        previous(info);
    }));
}
