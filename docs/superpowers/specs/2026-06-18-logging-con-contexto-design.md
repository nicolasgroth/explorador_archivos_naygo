# Logging con contexto (diagnóstico de caídas) — Diseño

> Mejora pedida por Nicolás 2026-06-18 tras un panic en la VM cuyo log no decía qué se
> estaba haciendo ni en qué estado estaba la app. Objetivo: que el log permita reconstruir
> el momento y el motivo de una falla, para resolver caídas futuras más rápido.

**Objetivo:** registrar "migas de pan" de las acciones clave del usuario y un snapshot del
estado, de modo que cuando ocurra un panic (u otro error), el log muestre la secuencia que
llevó ahí y el contexto (carpetas abiertas, tema, idioma, entorno) — sin afectar la velocidad
de navegación ni agregar telemetría/red.

**Autoría:** Nicolás Groth / ISGroth, 2026, MIT.

**Decisiones tomadas con el usuario:**
- Registrar **migas de acciones clave** (navegar, paneles, split/swap, operaciones, abrir
  config, expulsar USB, vista profunda, errores no fatales). NO eventos por-frame ni de bajo
  nivel (resize, selección) — ruido y costo.
- **Timestamp en hora local legible** (reusar `core::format`, sin crates nuevos).
- **Buffer circular en memoria (~200 eventos)** que NO escribe a disco por acción; se vuelca
  al archivo SOLO ante panic/error → cero impacto en la velocidad.
- Contexto del panic: **carpetas por panel + última acción + versión/OS/tamaño de ventana +
  tema/idioma**.
- **Buffer y snapshot GLOBALES thread-safe** (el panic hook no recibe parámetros, debe leerlos
  directo). El snapshot lo refresca el controlador en cada acción relevante.

---

## Contexto actual (lo que se reutiliza — no rehacer)

- **`crates/ui-slint/src/logging.rs`**: `init()` (trunca el log >2MB, escribe línea de arranque,
  instala el panic hook), `log_line(msg)` (anexa al archivo, infalible), `timestamp()`
  (segundos epoch — ilegible, se reemplaza), `log_path()` (junto al .exe), `install_panic_hook()`
  (escribe mensaje+ubicación+backtrace y muestra diálogo nativo). Sin red ni telemetría.
- **`crates/core/src/format.rs`**: `civil_from_epoch()` (epoch → año/mes/día/h/m/s SIN crates
  externos), `format_time()`, `DateFormat`. Base para el timestamp legible.
- Crate `windows` ya es dependencia (para `GetTimeZoneInformation` / `GetVersion`).
- `WorkspaceCtrl` es el controlador; tiene el estado de paneles, tema e idioma.

---

## Sección 1 — Dos estructuras globales en `logging.rs`

Thread-safe, `std` only (`LazyLock`/`OnceLock` + `Mutex`), sin ensuciar firmas del resto.

### 1.1 Buffer de migas de pan
```rust
/// Buffer circular en memoria de las últimas ~200 migas (eventos significativos). NO se
/// escribe a disco salvo en un panic/error. Cada miga lleva su hora local.
static BREADCRUMBS: LazyLock<Mutex<VecDeque<String>>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(BREADCRUMB_CAP)));
const BREADCRUMB_CAP: usize = 200;

/// Registra un evento significativo. Barato (un lock + push). Llamar desde el controlador
/// en acciones clave. Si el buffer está lleno, descarta el más viejo.
pub fn breadcrumb(msg: &str) {
    if let Ok(mut buf) = BREADCRUMBS.lock() {
        if buf.len() >= BREADCRUMB_CAP {
            buf.pop_front();
        }
        buf.push_back(format!("[{}] {}", local_time_str(), msg));
    }
}
```

### 1.2 Snapshot de diagnóstico
```rust
/// Resumen barato del estado actual, para volcar en un panic. Son Strings YA resumidos (no
/// referencias al estado vivo), así el panic hook no toca el WorkspaceCtrl (que podría estar
/// a medio mutar).
#[derive(Clone, Default)]
pub struct DiagSnapshot {
    pub panes: String,        // "[1] C:\\  [2] C:\\Users\\ngrot  [3] F:\\logs (no existe)"
    pub theme: String,        // id del tema activo
    pub lang: String,         // idioma activo
    pub last_action: String,  // "navegar panel 3 → F:\\logs\\siebel"
}
static DIAG: LazyLock<Mutex<DiagSnapshot>> = LazyLock::new(|| Mutex::new(DiagSnapshot::default()));

/// El controlador refresca el snapshot cuando cambia algo relevante. Barato.
pub fn set_diag_snapshot(snap: DiagSnapshot) {
    if let Ok(mut d) = DIAG.lock() {
        *d = snap;
    }
}
```

### 1.3 Entorno (capturado una vez al arrancar)
```rust
/// Línea de entorno: versión de Naygo + versión de Windows + tamaño/escala de ventana.
/// Se fija una vez (en init() y/o cuando se conoce la ventana). El panic hook la lee.
static ENV_INFO: OnceLock<String> = OnceLock::new();
pub fn set_env_info(window_size: (u32, u32), scale: f32);  // "Naygo v0.1.0 · Windows 10.0.19045 · ventana 1002x712 @1.0"
```
La versión de Windows se obtiene de `windows::Win32::System::SystemInformation` (o
`RtlGetVersion`); si falla, "Windows (desconocido)".

## Sección 2 — Timestamp legible + eventos

### 2.1 Timestamp legible (sin crates nuevos)
- Nueva función en `core::format`: `pub fn format_log_time(epoch_ms: u64, tz_offset_min: i32) -> String`
  → `"2026-06-18 14:32:05.123"` (hora local = epoch + offset). Reusa `civil_from_epoch`.
  Testeable con valores fijos.
- En `logging.rs`, `local_time_str()` toma la hora actual (`SystemTime`), el offset de zona
  (capturado una vez con `GetTimeZoneInformation` al arrancar; default 0 = UTC si falla, con
  marca " UTC") y llama `format_log_time`. Reemplaza el `timestamp()` epoch actual en TODO el
  log (arranque, migas, panic).

### 2.2 Eventos instrumentados (llamadas a `breadcrumb`)
En el controlador (`workspace_ctrl.rs`) y donde corresponda, una miga corta por acción clave:
- **Navegación:** `breadcrumb(&format!("navegar panel {id} → {dir}"))`, "subir un nivel",
  "atrás", "adelante".
- **Paneles/layout:** "abrir panel <tipo>", "cerrar panel", "split <dir>", "swap paneles",
  "aplicar layout <nombre>".
- **Operaciones:** "copiar N ítems", "mover N ítems", "eliminar N (papelera)", "renombrar",
  "batch-rename".
- **UI/toolbar:** "abrir configuración", "expulsar USB <letra>", "vista profunda on/off",
  "buscar '<q>'".
- **Errores no fatales:** "carpeta no encontrada: <ruta>", "permiso denegado: <ruta>", "fallo
  al expulsar (en uso)". Estos van al buffer **y** a `log_line` (al archivo) de inmediato —
  un error siempre se ve, no solo si hay panic después.

Criterio: lo que un humano llamaría "lo que estaba haciendo". Nada por-frame.

### 2.3 Snapshot tras cada acción
El controlador, en su punto central donde "algo cambió" (la rutina que ya re-sincroniza la
UI), llama `set_diag_snapshot(...)` con: rutas por panel, tema, idioma, última acción. Es una
foto barata; siempre lista para el panic. (La "última acción" puede ser la última miga.)

## Sección 3 — Volcado en el panic + robustez

### 3.1 Bloque de contexto en el panic hook
Antes del mensaje/backtrace que ya escribe, `install_panic_hook` vuelca:
```
*** PANIC ***
-- Contexto --
Naygo v0.1.0 · Windows 10.0.19045 · ventana 1002x712 @1.0
Tema: midnight · Idioma: es
Última acción: navegar panel 3 → F:\logs\siebel
Paneles: [1] C:\  [2] C:\Users\ngrot  [3] F:\logs\siebel (no existe)
-- Últimos eventos --
[14:32:01.110] abrir configuración
[14:32:05.118] navegar panel 3 → F:\logs\siebel
[14:32:05.121] carpeta no encontrada: F:\logs\siebel
-- Fin contexto --
mensaje: ...
en: ...
backtrace: ...
```

### 3.2 Robustez (corre DURANTE un panic — no debe paniquear ni colgarse)
- Leer las globales con **`try_lock`**, no `lock`: si el mutex está envenenado (panic mientras
  alguien lo tenía) o tomado, escribir "(contexto no disponible)" y seguir. NUNCA `unwrap`.
- Armado con `write!` a `String` (infalible, ya es el patrón).
- El snapshot guarda Strings ya resumidos → el hook NO toca el `WorkspaceCtrl`.
- Si `BREADCRUMBS.try_lock()` falla, omitir las migas pero igual escribir mensaje+backtrace.

### 3.3 Privacidad
Sin cambios: archivo de texto local junto al `.exe`, sin red ni telemetría. El snapshot guarda
rutas de carpetas (que el usuario ya ve), NO contenido de archivos. Truncado por tamaño (2MB)
se mantiene.

---

## Archivos tocados

| Archivo | Acción |
|---|---|
| `crates/core/src/format.rs` | + `format_log_time(epoch_ms, tz_offset_min) -> String` + tests. |
| `crates/ui-slint/src/logging.rs` | Buffer `BREADCRUMBS` + `breadcrumb`; `DiagSnapshot` + `set_diag_snapshot`; `ENV_INFO` + `set_env_info`; `local_time_str` (reemplaza `timestamp`); helper PURO `build_context_block(env, snap, crumbs) -> String` (testeable, sin tocar los `static`); el panic hook lo invoca leyendo las globales con `try_lock`. |
| `crates/ui-slint/src/workspace_ctrl.rs` | Llamadas a `breadcrumb(...)` en las acciones clave; `set_diag_snapshot(...)` en el punto de "algo cambió". |
| `crates/ui-slint/src/main.rs` | `set_env_info(...)` cuando se conoce la ventana (tamaño/escala) al arrancar; capturar el offset de zona una vez. |

## Testing

- **core** (`format.rs`): `format_log_time(epoch_ms, offset)` con valores fijos → cadena
  esperada; offset 0 (UTC) y offset negativo/positivo; subsegundos formateados a 3 dígitos.
- **logging**: buffer circular (push > cap descarta el viejo; `breadcrumb` antepone hora);
  armado del bloque de contexto desde un `DiagSnapshot` fijo + migas fijas (verifica el formato
  exacto, SIN disparar un panic real). Como las globales son `static`, los tests usan funciones
  puras auxiliares (p. ej. `build_context_block(env, snap, crumbs) -> String`) que NO dependen
  de los `static` para ser testeables.
- El volcado real en un panic se verifica a mano (provocar un panic de prueba y mirar el log).

## Fuera de alcance (YAGNI)

- Nivel de log configurable (mínimo/normal/detallado) — se decidió un solo nivel de migas.
- Escribir cada miga al archivo al instante — se decidió buffer en memoria + volcado en panic.
- Telemetría / envío por red — explícitamente NO.
- Rotación de logs por fecha — el truncado por tamaño (2MB) basta.

## Riesgos y mitigaciones

- **Doble-panic / deadlock en el hook:** mitigado con `try_lock` y datos pre-resumidos.
- **Costo de instrumentar:** `breadcrumb` es lock+push por evento puntual (no por frame) →
  imperceptible; el snapshot es una foto barata en el punto que ya re-sincroniza la UI.
- **Offset de zona indisponible:** cae a UTC con marca clara, nunca rompe.
- **Rutas en el log:** son carpetas (ya visibles al usuario), no contenido; sin red, queda local.
