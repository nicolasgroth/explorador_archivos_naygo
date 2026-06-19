// Naygo — logging básico a archivo + panic handler (diagnóstico de caídas).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Sin telemetría ni red: solo un archivo de texto JUNTO al ejecutable (el mismo
// `portable_dir()` donde vive settings.json), para poder diagnosticar caídas en VMs y
// equipos limpios. El panic handler captura el mensaje, la ubicación y el backtrace, los
// escribe al log y los muestra en un diálogo nativo, en vez de cerrarse en silencio.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, OnceLock};

/// Nombre del archivo de log (en el directorio portable, junto a naygo.exe).
const LOG_FILE: &str = "naygo.log";
/// Si el log supera este tamaño, se trunca al arrancar (evita que crezca sin límite).
const LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Capacidad del buffer circular de migas.
const BREADCRUMB_CAP: usize = 200;

/// Migas de pan: últimos eventos significativos, en memoria. Se vuelcan al log SOLO en un
/// panic/error (no se escribe a disco por acción → no afecta la velocidad).
static BREADCRUMBS: LazyLock<Mutex<VecDeque<String>>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(BREADCRUMB_CAP)));

/// Snapshot del estado para diagnóstico (Strings ya resumidos; el panic hook NO toca el
/// estado vivo). Lo refresca el controlador en cada acción relevante.
static DIAG: LazyLock<Mutex<DiagSnapshot>> = LazyLock::new(|| Mutex::new(DiagSnapshot::default()));

/// Línea de entorno (versión/OS/ventana), fijada una vez al arrancar.
static ENV_INFO: OnceLock<String> = OnceLock::new();

/// Offset del huso local en minutos (lo fija main.rs al arrancar; 0 = UTC si no se pudo).
static TZ_OFFSET_MIN: OnceLock<i32> = OnceLock::new();

/// Resumen barato del estado, para volcar en un panic.
#[derive(Clone, Default)]
pub struct DiagSnapshot {
    /// Paneles abiertos, p. ej. "[1] C:\\  [2] C:\\Users\\ngrot  [3] F:\\logs (no existe)".
    pub panes: String,
    pub theme: String,
    pub lang: String,
    /// Última acción del usuario, p. ej. "navegar panel 3 → F:\\logs\\siebel".
    pub last_action: String,
}

/// El controlador refresca el snapshot cuando cambia algo relevante. Barato.
pub fn set_diag_snapshot(snap: DiagSnapshot) {
    if let Ok(mut d) = DIAG.lock() {
        *d = snap;
    }
}

/// Fija la línea de entorno (una vez). `window` = (ancho, alto) en px; `scale` = factor.
pub fn set_env_info(window: (u32, u32), scale: f32, os: &str) {
    let info = format!(
        "Naygo v{} · {} · ventana {}x{} @{:.1}",
        env!("CARGO_PKG_VERSION"),
        os,
        window.0,
        window.1,
        scale
    );
    let _ = ENV_INFO.set(info);
}

/// Fija el offset del huso local en minutos (una vez, desde main.rs).
pub fn set_tz_offset(minutes: i32) {
    let _ = TZ_OFFSET_MIN.set(minutes);
}

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

/// Anexa una línea al log (con marca de tiempo local legible). Nunca falla hacia
/// afuera: si no se puede escribir (disco lleno, permiso), se ignora en silencio.
pub fn log_line(msg: &str) {
    let line = format!("[{}] {msg}\n", local_time_str());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Hora local legible para el log ("YYYY-MM-DD HH:MM:SS.mmm"). Usa el offset fijado por
/// main.rs (0 = UTC si no se fijó). Sin crates externos.
fn local_time_str() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let offset = TZ_OFFSET_MIN.get().copied().unwrap_or(0);
    naygo_core::format::format_log_time(epoch_ms, offset)
}

/// Registra un evento significativo en el buffer circular (en memoria, con su hora local).
/// NO escribe a disco. Llamar desde el controlador en acciones clave.
pub fn breadcrumb(msg: &str) {
    let line = format!("[{}] {msg}", local_time_str());
    if let Ok(mut buf) = BREADCRUMBS.lock() {
        if buf.len() >= BREADCRUMB_CAP {
            buf.pop_front();
        }
        buf.push_back(line);
    }
}

/// La última miga registrada (sin la marca de tiempo), o vacío si no hay ninguna. La usa el
/// snapshot para el campo "última acción". Lee con try_lock para no colgar.
pub fn last_breadcrumb() -> String {
    BREADCRUMBS
        .try_lock()
        .ok()
        .and_then(|b| b.back().cloned())
        .unwrap_or_default()
}

/// Arma el bloque de contexto que precede al panic en el log. PURO: recibe los datos ya
/// extraídos (no toca las globales) para poder testearlo. `crumbs` del más viejo al más nuevo.
fn build_context_block(env: &str, snap: &DiagSnapshot, crumbs: &[String]) -> String {
    let mut b = String::new();
    let _ = writeln!(b, "-- Contexto --");
    let _ = writeln!(
        b,
        "{}",
        if env.is_empty() {
            "(entorno no disponible)"
        } else {
            env
        }
    );
    let _ = writeln!(b, "Tema: {} · Idioma: {}", snap.theme, snap.lang);
    let _ = writeln!(b, "Última acción: {}", snap.last_action);
    let _ = writeln!(b, "Paneles: {}", snap.panes);
    let _ = writeln!(b, "-- Últimos eventos --");
    if crumbs.is_empty() {
        let _ = writeln!(b, "(sin eventos registrados)");
    } else {
        for c in crumbs {
            let _ = writeln!(b, "{c}");
        }
    }
    let _ = writeln!(b, "-- Fin contexto --");
    b
}

/// Instala un panic hook que escribe el panic (mensaje + ubicación + backtrace) al log y
/// muestra un diálogo nativo, para que una caída NO sea un cierre silencioso. Conserva el
/// hook anterior (lo llama después) por si el entorno imprime a stderr.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut report = String::new();
        let _ = writeln!(report, "*** PANIC ***");
        // Bloque de contexto: entorno + estado + migas. Se lee con try_lock para NO colgar ni
        // re-paniquear si un mutex quedó tomado/envenenado durante el panic.
        let env = ENV_INFO.get().cloned().unwrap_or_default();
        let snap = DIAG.try_lock().map(|d| d.clone()).unwrap_or_default();
        let crumbs: Vec<String> = BREADCRUMBS
            .try_lock()
            .map(|b| b.iter().cloned().collect())
            .unwrap_or_else(|_e| vec!["(migas no disponibles)".to_string()]);
        let _ = write!(report, "{}", build_context_block(&env, &snap, &crumbs));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_context_block_formato() {
        let snap = DiagSnapshot {
            panes: "[1] C:\\".to_string(),
            theme: "midnight".to_string(),
            lang: "es".to_string(),
            last_action: "navegar panel 1 → C:\\".to_string(),
        };
        let crumbs = vec!["[10:00:00.000] abrir configuración".to_string()];
        let block = build_context_block(
            "Naygo v0.1.0 · Windows · ventana 800x600 @1.0",
            &snap,
            &crumbs,
        );
        assert!(block.contains("-- Contexto --"));
        assert!(block.contains("Tema: midnight · Idioma: es"));
        assert!(block.contains("Última acción: navegar panel 1 → C:\\"));
        assert!(block.contains("Paneles: [1] C:\\"));
        assert!(block.contains("abrir configuración"));
        assert!(block.contains("-- Fin contexto --"));
    }

    #[test]
    fn build_context_block_sin_datos() {
        let block = build_context_block("", &DiagSnapshot::default(), &[]);
        assert!(block.contains("(entorno no disponible)"));
        assert!(block.contains("(sin eventos registrados)"));
    }

    #[test]
    fn breadcrumb_respeta_capacidad() {
        for i in 0..(BREADCRUMB_CAP + 50) {
            breadcrumb(&format!("evento {i}"));
        }
        let buf = BREADCRUMBS.lock().unwrap();
        assert!(buf.len() <= BREADCRUMB_CAP);
        assert!(buf
            .back()
            .unwrap()
            .contains(&format!("evento {}", BREADCRUMB_CAP + 49)));
    }
}
