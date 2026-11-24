// Naygo — logging básico a archivo + panic handler (diagnóstico de caídas).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Sin telemetría ni red: XDG_STATE_HOME en Linux y junto al ejecutable portable en Windows.
// El hilo UI sólo encola; un worker crea el directorio y escribe el log. El panic handler captura
// mensaje, ubicación y backtrace y los muestra en un diálogo nativo.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

thread_local! {
    /// Profundidad de bloques que contienen panics recuperables de dependencias hostiles. El panic
    /// hook consulta este flag para no anunciar que Naygo se cerró cuando `catch_unwind` lo contiene.
    static RECOVERABLE_PANIC_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

struct RecoverablePanicGuard;

impl RecoverablePanicGuard {
    fn enter() -> Self {
        RECOVERABLE_PANIC_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
        Self
    }
}

impl Drop for RecoverablePanicGuard {
    fn drop(&mut self) {
        RECOVERABLE_PANIC_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Ejecuta una llamada de terceros cuyo contrato real puede incluir `panic` ante datos corruptos.
/// Solo silencia el hook en ESTE hilo y durante ESTE bloque; el resultado conserva el payload para
/// que el caller degrade a un error normal. No usar para esconder invariantes propias de Naygo.
pub fn catch_recoverable_panic<F, R>(f: F) -> std::thread::Result<R>
where
    F: FnOnce() -> R,
{
    let _guard = RecoverablePanicGuard::enter();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
}

/// Prefijo del archivo de log. El nombre real lleva la fecha local del día:
/// `naygo-YYYY-MM-DD.log` (junto a naygo.exe), para separar las corridas por día y no
/// mezclar diagnósticos de sesiones distintas.
const LOG_PREFIX: &str = "naygo";
/// Si el log supera este tamaño, se trunca al arrancar (evita que crezca sin límite).
const LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Directorio de estado ya resuelto al inicio, sin consultar el filesystem desde UI.
static LOG_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

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

/// Las líneas ordinarias van a un worker con cola acotada: una llamada desde un callback o tick
/// nunca abre/escribe el archivo ni espera a que un volumen lento responda.
static LOG_WRITER: OnceLock<Option<SyncSender<LogMessage>>> = OnceLock::new();
static DROPPED_LOG_LINES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

enum LogMessage {
    Prepare { path: PathBuf },
    Write { path: PathBuf, line: String },
    Flush(std::sync::mpsc::Sender<()>),
}

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

/// Offset del huso local en SEGUNDOS (0 si no se fijó). Lo usa el formateo de fechas fuera del log
/// (p. ej. el modal de conflicto) para mostrar la hora local, igual que las columnas del panel.
pub fn tz_offset_secs() -> i64 {
    TZ_OFFSET_MIN.get().copied().unwrap_or(0) as i64 * 60
}

/// Ruta del archivo de log del día: directorio de estado XDG en Linux y carpeta portable en
/// Windows, con la fecha local en el nombre.
pub fn log_path() -> PathBuf {
    let name = format!("{LOG_PREFIX}-{}.log", log_date_str());
    LOG_DIRECTORY
        .get()
        .cloned()
        .unwrap_or_else(naygo_core::config::portable_dir)
        .join(name)
}

/// Fecha local de hoy como "YYYY-MM-DD", reutilizando el mismo reloj y huso que las marcas
/// de tiempo del log (los primeros 10 caracteres de "YYYY-MM-DD HH:MM:SS.mmm").
fn log_date_str() -> String {
    let full = local_time_str();
    // `format_log_time` siempre devuelve "YYYY-MM-DD ..."; tomamos la parte de fecha.
    full.split(' ').next().unwrap_or("0000-00-00").to_string()
}

/// Inicializa el logging en el directorio de estado seleccionado por la plataforma. Preparar
/// carpeta y limitar el tamaño se delegan al worker; el hilo de UI sólo encola eventos.
/// Llamar UNA vez al inicio de `main`, antes de crear la ventana.
pub fn init(state_dir: PathBuf) {
    let _ = LOG_DIRECTORY.set(state_dir);
    let path = log_path();
    if let Some(sender) = log_writer() {
        let _ = sender.try_send(LogMessage::Prepare { path });
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
    let Some(sender) = log_writer() else {
        DROPPED_LOG_LINES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return;
    };
    let dropped = DROPPED_LOG_LINES.swap(0, std::sync::atomic::Ordering::Relaxed);
    let prefix = if dropped == 0 {
        String::new()
    } else {
        format!("[logger] se descartaron {dropped} líneas por cola llena\n")
    };
    let line = format!("{prefix}[{}] {msg}\n", local_time_str());
    if sender
        .try_send(LogMessage::Write {
            path: log_path(),
            line,
        })
        .is_err()
    {
        DROPPED_LOG_LINES.fetch_add(
            dropped.saturating_add(1),
            std::sync::atomic::Ordering::Relaxed,
        );
    }
}

fn log_writer() -> Option<&'static SyncSender<LogMessage>> {
    LOG_WRITER
        .get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<LogMessage>(512);
            std::thread::Builder::new()
                .name("naygo-log-writer".into())
                .spawn(move || {
                    while let Ok(message) = rx.recv() {
                        match message {
                            LogMessage::Prepare { path } => prepare_log_file(&path),
                            LogMessage::Write { path, line } => {
                                append_line(&path, &line);
                            }
                            LogMessage::Flush(done) => {
                                let _ = done.send(());
                            }
                        }
                    }
                })
                .ok()?;
            Some(tx)
        })
        .as_ref()
}

fn append_line(path: &std::path::Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Crea el directorio y aplica el límite una sola vez al inicio, siempre desde el worker.
fn prepare_log_file(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::metadata(path).is_ok_and(|meta| meta.len() > LOG_MAX_BYTES) {
        let _ = std::fs::remove_file(path);
    }
}

/// Vacía la cola antes de salir. Se llama desde el coordinador de cierre fuera del hilo de UI;
/// si el disco está bloqueado, el cierre continúa al vencer el plazo.
pub fn flush(timeout: Duration) -> bool {
    let Some(sender) = log_writer() else {
        return false;
    };
    let deadline = Instant::now() + timeout;
    let (tx, rx) = std::sync::mpsc::channel();
    loop {
        match sender.try_send(LogMessage::Flush(tx.clone())) {
            Ok(()) => break,
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => return false,
            Err(std::sync::mpsc::TrySendError::Full(_)) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(std::sync::mpsc::TrySendError::Full(_)) => return false,
        }
    }
    rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .is_ok()
}

/// Escritura de emergencia para el panic hook; no depende de la cola ordinaria que pudo llenarse.
fn log_line_sync(msg: &str) {
    append_line(&log_path(), &format!("[{}] {msg}\n", local_time_str()));
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
        if RECOVERABLE_PANIC_DEPTH.with(|depth| depth.get() > 0) {
            return;
        }
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

        log_line_sync(&report);

        // Diálogo nativo: el usuario VE que pasó algo y dónde está el log, en vez de un
        // cierre fantasma. (rfd ya es dependencia; el diálogo es síncrono y bloqueante.)
        let log = log_path();
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Naygo — error inesperado")
            .set_description(format!(
                "Naygo se cerró por un error inesperado.\n\nGuardamos un registro \
                 técnico en:\n{}\n\nSi el problema se repite, ese archivo ayuda a \
                 diagnosticarlo.",
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
    fn logger_worker_helpers_create_parent_and_enforce_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state").join("naygo.log");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, vec![b'x'; (LOG_MAX_BYTES + 1) as usize]).unwrap();

        prepare_log_file(&path);
        assert!(!path.exists(), "un log sobre el límite se trunca al inicio");
        append_line(&path, "línea de prueba\n");

        assert_eq!(std::fs::read_to_string(path).unwrap(), "línea de prueba\n");
    }

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

    #[test]
    fn panic_recuperable_se_convierte_en_resultado() {
        let result = catch_recoverable_panic(|| panic!("dependencia hostil"));
        assert!(result.is_err());
        assert_eq!(RECOVERABLE_PANIC_DEPTH.with(|depth| depth.get()), 0);
    }
}
