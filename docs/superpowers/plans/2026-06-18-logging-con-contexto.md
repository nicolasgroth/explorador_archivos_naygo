# Logging con contexto (diagnóstico de caídas) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hacer que el `naygo.log` permita reconstruir qué hacía el usuario y en qué estado estaba la app cuando ocurre una caída: migas de pan de acciones clave + snapshot de estado + entorno, con timestamp en hora local legible, volcados al log ante un panic.

**Architecture:** Dos globales thread-safe en `logging.rs` (un buffer circular de migas y un snapshot de diagnóstico), más una línea de entorno fijada al arrancar. El controlador llama `breadcrumb()` en acciones clave y refresca el snapshot. El panic hook lee las globales con `try_lock` (infalible) y vuelca un bloque de contexto antes del backtrace. El timestamp legible reusa `core::format` (sin crates nuevos).

**Tech Stack:** Rust workspace (naygo-core / naygo-platform / naygo-ui-slint), Slint 1.16 (render software). Build SIEMPRE con `CARGO_BUILD_JOBS=2`. Gate de cada tarea de código: `cargo fmt --all` + `cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform` + `cargo clippy --workspace --all-targets -- -D warnings`.

**Spec:** `docs/superpowers/specs/2026-06-18-logging-con-contexto-design.md`

**Convenciones del repo (recordatorio):**
- Español neutral SIN voseo (código, comentarios, docs). PowerShell 5.1: nunca `&&`/`||`.
- Header en archivos nuevos: `// Naygo — <descr>` + `// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.`
- NO commitear `CLAUDE.md`. graphify-out/ y assets/icons/otros/ ya en .gitignore.
- NO `git push`. Commits locales por tarea. Mensajes terminan con `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Tras tocar código: `graphify update .`. Regla graphify: `graphify query` antes de grepear/leer fuentes.
- Sin telemetría ni red. El log sigue siendo un archivo de texto local junto al .exe.

**Datos reales ya explorados:**
- `crates/core/src/format.rs`: `fn civil_from_epoch(secs: i64) -> (i64, u32, u32, u32, u32)` (año, mes, día, hora, minuto — SIN segundos, privada). `pub fn format_time(local_epoch_secs: Option<i64>, fmt: DateFormat) -> String`. El llamador ajusta el offset local antes de pasar los segundos.
- `crates/ui-slint/src/logging.rs`: `init()`, `log_line(msg)` (anexa al archivo, infalible), `timestamp()` (epoch — a reemplazar), `log_path()`, `install_panic_hook()` (escribe mensaje+ubicación+backtrace, muestra diálogo rfd). Usa `std::fmt::Write` y `std::io::Write`.
- Crate `windows` ya es dependencia.

---

## File Structure

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `crates/core/src/format.rs` | `format_log_time(epoch_ms, tz_offset_min) -> String` (puro, "YYYY-MM-DD HH:MM:SS.mmm"). + tests. | Modificar |
| `crates/ui-slint/src/logging.rs` | Globales `BREADCRUMBS`/`DIAG`/`ENV_INFO`; `breadcrumb`, `DiagSnapshot`+`set_diag_snapshot`, `set_env_info`, `local_time_str`, `build_context_block` (puro); panic hook vuelca el bloque con `try_lock`. + tests. | Modificar |
| `crates/ui-slint/src/workspace_ctrl.rs` | `breadcrumb(...)` en acciones clave + `set_diag_snapshot(...)` en el punto de re-sync. | Modificar |
| `crates/ui-slint/src/main.rs` | Capturar el offset de zona una vez; `set_env_info(...)` cuando se conoce la ventana. | Modificar |

**Orden:** Task 1 (formato en core, testeable). Task 2 (globales + helpers + tests en logging). Task 3 (panic hook vuelca contexto). Task 4 (instrumentar controlador). Task 5 (entorno + offset en main.rs).

---

## Task 1: `format_log_time` en core::format

**Files:**
- Modify: `crates/core/src/format.rs`

- [ ] **Step 1: Escribir el test (falla porque la función no existe)**

> ANTES de escribir los asserts, calcula el epoch real de una fecha redonda para no inventar
> valores. En Git Bash: `date -u -d "2026-06-18 00:00:00" +%s` → multiplícalo por 1000 para ms.
> Usa ESE valor en los tests y ajusta las cadenas esperadas a la fecha/hora que de verdad
> corresponde con cada offset. Los tests de abajo asumen que ese epoch_ms es `1_781_568_000_000`;
> si tu cálculo da otro número, sustitúyelo en los tres tests y recalcula las cadenas esperadas
> (la del offset -180 resta 3 h; la de +60 suma 1 h).

Agregar al `mod tests` de `format.rs`:
```rust
    #[test]
    fn format_log_time_basico() {
        // 2026-06-18 00:00:00.000 UTC = 1781568000000 ms (epoch). Offset 0.
        // (epoch de 2026-06-18T00:00:00Z; verificamos formato, no el valor exacto del día.)
        let s = format_log_time(1_781_568_000_000, 0);
        assert_eq!(s, "2026-06-18 00:00:00.000");
    }

    #[test]
    fn format_log_time_con_offset_y_milisegundos() {
        // 1781568000000 ms + 123 ms, offset -180 min (UTC-3): la hora local resta 3 h.
        let s = format_log_time(1_781_568_000_123, -180);
        assert_eq!(s, "2026-06-17 21:00:00.123");
    }

    #[test]
    fn format_log_time_offset_positivo() {
        // offset +60 min: suma 1 hora.
        let s = format_log_time(1_781_568_000_000, 60);
        assert_eq!(s, "2026-06-18 01:00:00.000");
    }
```

- [ ] **Step 2: Correr para ver que falla**
```
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core format_log_time
```
Esperado: FAIL (función no existe).

- [ ] **Step 3: Implementar `format_log_time`**

Agregar en `format.rs` (después de `format_time`):
```rust
/// Formatea un instante para el log: `"YYYY-MM-DD HH:MM:SS.mmm"` en hora LOCAL.
/// `epoch_ms` son milisegundos desde epoch UTC; `tz_offset_min` es el desplazamiento del
/// huso local en minutos (p. ej. -180 para UTC-3, +60 para UTC+1). Puro, sin dependencias.
pub fn format_log_time(epoch_ms: u64, tz_offset_min: i32) -> String {
    let total_secs = (epoch_ms / 1000) as i64 + (tz_offset_min as i64) * 60;
    let millis = (epoch_ms % 1000) as u32;
    let (y, mo, d, h, mi) = civil_from_epoch(total_secs);
    let sec = total_secs.rem_euclid(60) as u32;
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{sec:02}.{millis:03}")
}
```
> `civil_from_epoch` ya da y/mo/d/h/mi; los segundos se sacan con `total_secs.rem_euclid(60)`
> (coherente con cómo `civil_from_epoch` calcula hora/minuto del resto del día). NO hace falta
> modificar `civil_from_epoch`.

- [ ] **Step 4: Tests pasan**
```
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core format_log_time
```
Esperado: 3 PASS. (Si el valor epoch de los tests no cae exactamente en 2026-06-18, ajusta el
epoch del test al que sí — lo importante es verificar formato, offset y milisegundos; usa un
epoch redondo conocido y su fecha real. Puedes calcular el epoch correcto con
`date -u -d "2026-06-18 00:00:00" +%s` en Git Bash y multiplicar por 1000.)

- [ ] **Step 5: Gate + commit**
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt -p naygo-core
$env:CARGO_BUILD_JOBS = "2"; cargo clippy -p naygo-core --all-targets -- -D warnings
```
```bash
git add crates/core/src/format.rs
git commit -F - <<'EOF'
feat(core): format_log_time para timestamps legibles del log

"YYYY-MM-DD HH:MM:SS.mmm" en hora local (epoch_ms + offset de huso en minutos).
Reusa civil_from_epoch, sin dependencias. Con tests.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 2: Globales + helpers en `logging.rs` (buffer, snapshot, entorno, helper puro)

**Files:**
- Modify: `crates/ui-slint/src/logging.rs`

- [ ] **Step 1: Agregar imports y globales**

En la cabecera de `logging.rs`, junto a los `use` existentes, agregar:
```rust
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex, OnceLock};
```
Y las globales (tras las constantes `LOG_FILE`/`LOG_MAX_BYTES`):
```rust
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
```

- [ ] **Step 2: `DiagSnapshot` + `set_diag_snapshot` + `set_env_info` + `set_tz_offset`**
```rust
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
```

- [ ] **Step 3: `local_time_str` (reemplaza `timestamp`) + `breadcrumb`**

Reemplaza la función `timestamp()` actual por `local_time_str()`:
```rust
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
```
Actualiza `log_line` para usar `local_time_str()` en vez de `timestamp()`:
```rust
pub fn log_line(msg: &str) {
    let line = format!("[{}] {msg}\n", local_time_str());
    // ... resto igual ...
}
```
Agrega `breadcrumb`:
```rust
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
```

- [ ] **Step 4: `build_context_block` (helper PURO, testeable)**
```rust
/// Arma el bloque de contexto que precede al panic en el log. PURO: recibe los datos ya
/// extraídos (no toca las globales) para poder testearlo. `crumbs` es del más viejo al más
/// nuevo.
fn build_context_block(env: &str, snap: &DiagSnapshot, crumbs: &[String]) -> String {
    let mut b = String::new();
    let _ = writeln!(b, "-- Contexto --");
    let _ = writeln!(b, "{}", if env.is_empty() { "(entorno no disponible)" } else { env });
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
```

- [ ] **Step 5: Tests del buffer y del helper**

Agregar un `mod tests` al final de `logging.rs`:
```rust
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
        let block = build_context_block("Naygo v0.1.0 · Windows · ventana 800x600 @1.0", &snap, &crumbs);
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
        // Empuja más que la capacidad y verifica que el buffer no la excede.
        for i in 0..(BREADCRUMB_CAP + 50) {
            breadcrumb(&format!("evento {i}"));
        }
        let buf = BREADCRUMBS.lock().unwrap();
        assert!(buf.len() <= BREADCRUMB_CAP);
        // El más nuevo está al final.
        assert!(buf.back().unwrap().contains(&format!("evento {}", BREADCRUMB_CAP + 49)));
    }
}
```
> El test `breadcrumb_respeta_capacidad` usa el `static` real; está bien porque solo verifica el
> tope. Si hubiera interferencia entre tests por el estado global compartido, no la habrá aquí
> porque es el único test que escribe en BREADCRUMBS.

- [ ] **Step 6: Build + tests**
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-ui-slint logging
```
Esperado: compila; 3 tests PASS. (Si `naygo_core::format::format_log_time` no es visible, confirma
que Task 1 lo dejó `pub` y que `naygo_core` reexporta `format` — ya es `pub mod format`.)

- [ ] **Step 7: Gate + commit**
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all
$env:CARGO_BUILD_JOBS = "2"; cargo clippy -p naygo-ui-slint --all-targets -- -D warnings
```
> Nota: `set_diag_snapshot`/`set_env_info`/`set_tz_offset`/`breadcrumb`/`DiagSnapshot` aún no
> tienen llamadores (Tasks 4-5) → clippy podría marcar dead_code. Si lo hace, añade
> `#[allow(dead_code)]` con comentario "lo usan Task 4/5" y quítalo cuando se cableen. Reporta.

```bash
git add crates/ui-slint/src/logging.rs
git commit -F - <<'EOF'
feat(logging): buffer de migas + snapshot + entorno + timestamp legible

Globales thread-safe (BREADCRUMBS circular, DIAG, ENV_INFO, TZ_OFFSET); breadcrumb(),
set_diag_snapshot(), set_env_info(), set_tz_offset(); local_time_str reemplaza el
timestamp epoch por hora local legible (reusa core::format). build_context_block puro
y testeado.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 3: El panic hook vuelca el bloque de contexto (con `try_lock`)

**Files:**
- Modify: `crates/ui-slint/src/logging.rs` (función `install_panic_hook`)

- [ ] **Step 1: Insertar el volcado de contexto antes del mensaje del panic**

En `install_panic_hook`, dentro de la closure, DESPUÉS de `writeln!(report, "*** PANIC ***")`
y ANTES del `writeln!(report, "mensaje: {payload}")`, agregar la lectura con `try_lock`:
```rust
        // Bloque de contexto: entorno + estado + migas. Se lee con try_lock para NO colgar ni
        // re-paniquear si un mutex quedó tomado/envenenado durante el panic.
        let env = ENV_INFO.get().cloned().unwrap_or_default();
        let snap = DIAG.try_lock().map(|d| d.clone()).unwrap_or_default();
        let crumbs: Vec<String> = BREADCRUMBS
            .try_lock()
            .map(|b| b.iter().cloned().collect())
            .unwrap_or_else(|_| vec!["(migas no disponibles)".to_string()]);
        let _ = write!(report, "{}", build_context_block(&env, &snap, &crumbs));
```
(`write!`/`writeln!` ya están en scope vía `use std::fmt::Write as _;`.)

- [ ] **Step 2: Build**
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
```
Esperado: compila. (Ahora `build_context_block`, `DIAG`, `ENV_INFO`, `BREADCRUMBS` SÍ se usan
desde el hook → si tenías `#[allow(dead_code)]` en algunos, quítalos donde ya apliquen.)

- [ ] **Step 3: Gate**
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform
$env:CARGO_BUILD_JOBS = "2"; cargo clippy --workspace --all-targets -- -D warnings
```
Esperado: tests PASS, clippy limpio.

- [ ] **Step 4: Commit**
```bash
git add crates/ui-slint/src/logging.rs
git commit -F - <<'EOF'
feat(logging): el panic hook vuelca contexto (entorno + estado + migas)

Antes del mensaje/backtrace, el hook escribe el bloque de contexto leyendo las
globales con try_lock (infalible durante un panic: si un mutex esta tomado o
envenenado, escribe un marcador y sigue).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 4: Instrumentar el controlador (migas + snapshot)

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs`

- [ ] **Step 1: Migas en las acciones clave**

Orientación: `graphify query "navigation and panel operations in workspace controller"` para
ubicar los métodos. Agrega `crate::logging::breadcrumb(...)` al inicio (o donde la acción se
confirma) de los métodos clave. Mínimos a instrumentar (usa los nombres REALES de los métodos;
si un método no existe con ese nombre exacto, ubica el equivalente):
- Navegación: `navigate_active_to` / `navigate_pane_to` → `breadcrumb(&format!("navegar panel {} → {}", id.0, dir.display()))`. `on_go_up`/`on_go_back`/`on_go_forward` → "subir un nivel" / "atrás" / "adelante".
- Paneles/layout: agregar panel → "abrir panel <tipo>"; cerrar panel → "cerrar panel"; split → "split"; `swap_panes` → "intercambiar paneles"; aplicar plantilla → "aplicar layout <nombre>".
- Operaciones: donde se lanza copiar/mover/eliminar/renombrar/batch → "copiar N", "mover N", "eliminar N (papelera)", "renombrar", "batch-rename".
- UI: abrir config → "abrir configuración"; expulsar USB → "expulsar USB <letra>"; vista profunda → "vista profunda on"/"off"; búsqueda → `breadcrumb(&format!("buscar '{}'", q))`.
- Errores no fatales: donde se detecta carpeta no encontrada / permiso denegado / fallo de expulsión → además de `breadcrumb`, llamar `crate::logging::log_line(...)` con el mismo texto (estos sí van al archivo de inmediato).

Mantén los textos cortos y en español neutral. No instrumentes nada por-frame ni selección.

- [ ] **Step 2: Snapshot en el punto de re-sync**

Agrega un método al controlador que construya el `DiagSnapshot` desde el estado actual:
```rust
    /// Construye el snapshot de diagnóstico (rutas por panel + tema + idioma + última acción).
    /// Barato: solo strings. Lo consume el log ante un panic.
    pub fn diag_snapshot(&self) -> crate::logging::DiagSnapshot {
        let panes = self
            .ws
            .pane_ids_in_order()        // usa el método real que lista los paneles; si no existe, itera self.ws.panes()
            .iter()
            .enumerate()
            .map(|(i, id)| format!("[{}] {}", i + 1, self.path_of(*id)))  // path_of o el getter real de ruta
            .collect::<Vec<_>>()
            .join("  ");
        crate::logging::DiagSnapshot {
            panes,
            theme: self.config.settings.theme.to_string(),   // ajusta al getter real del tema
            lang: self.config.settings.language.clone(),     // ajusta al getter real del idioma
            last_action: String::new(),  // la última miga ya lo refleja; opcional dejar vacío
        }
    }
```
> Ajusta `pane_ids_in_order`/`path_of`/`theme`/`language` a los nombres REALES (búscalos: cómo
> arma main.rs las rutas de panel y cómo lee tema/idioma de settings). Si `theme`/`language` son
> enums/structs, usa su `to_string()`/`as_str()`/código real.

Luego, en el punto central donde la UI se re-sincroniza tras una acción (en `main.rs` la closure
`sync_rows`, o un método del controlador llamado tras cada acción), añade:
```rust
        crate::logging::set_diag_snapshot(ctrl.borrow().diag_snapshot());
```
> Colócalo donde no se llame por-frame en bucle ocioso (basta con que se actualice tras acciones;
> si `sync_rows` corre en el tick, está bien igual: construir el snapshot es barato — pero si
> prefieres, llámalo solo en los handlers de acción). Reporta dónde lo pusiste.

- [ ] **Step 3: Build + gate**
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform
$env:CARGO_BUILD_JOBS = "2"; cargo clippy --workspace --all-targets -- -D warnings
```
Esperado: compila, tests PASS, clippy limpio.

- [ ] **Step 4: Commit**
```bash
git add crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(logging): migas de pan en acciones clave + snapshot de estado

breadcrumb() en navegacion, paneles/layout, operaciones, UI (config/USB/vista
profunda/busqueda) y errores no fatales (estos tambien al archivo). diag_snapshot()
arma el resumen de estado (paneles/tema/idioma) y se publica en el re-sync.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 5: Entorno + offset de zona en main.rs

**Files:**
- Modify: `crates/ui-slint/src/main.rs`

- [ ] **Step 1: Fijar el offset de zona al arrancar**

Cerca del inicio de `main` (después de `logging::init()`), obtener el offset del huso local con
la API de Windows y fijarlo:
```rust
    // Offset del huso local (minutos) para los timestamps del log. Si falla, queda en UTC.
    {
        let offset_min = win_tz_offset_minutes().unwrap_or(0);
        crate::logging::set_tz_offset(offset_min);
    }
```
Y agrega la función auxiliar (en main.rs o donde haya helpers de plataforma):
```rust
/// Offset del huso local en minutos (negativo al oeste de UTC). Usa GetTimeZoneInformation:
/// `Bias` viene en minutos a RESTAR de la hora local para obtener UTC (UTC = local + Bias), o
/// sea el offset local = -Bias (más el ajuste de horario de verano si aplica).
#[cfg(windows)]
fn win_tz_offset_minutes() -> Option<i32> {
    use windows::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
    let mut tzi = TIME_ZONE_INFORMATION::default();
    // SAFETY: tzi vive durante la llamada; la función llena la estructura.
    let ret = unsafe { GetTimeZoneInformation(&mut tzi) };
    // ret: TIME_ZONE_ID_STANDARD(1)/DAYLIGHT(2) válidos; UNKNOWN(0) también da Bias.
    // El offset local = -(Bias + bias_estacional). En DST se suma DaylightBias.
    const TIME_ZONE_ID_DAYLIGHT: u32 = 2;
    let seasonal = if ret == TIME_ZONE_ID_DAYLIGHT { tzi.DaylightBias } else { tzi.StandardBias };
    Some(-(tzi.Bias + seasonal))
}
#[cfg(not(windows))]
fn win_tz_offset_minutes() -> Option<i32> { None }
```
> Verifica el path real de `GetTimeZoneInformation`/`TIME_ZONE_INFORMATION` en la versión de
> `windows` del proyecto (probablemente `windows::Win32::System::Time`). Si el feature no está
> habilitado en el Cargo.toml de ui-slint, agrégalo (p. ej. `Win32_System_Time`). El proyecto ya
> usa varios features de `windows`; añade el que falte.

- [ ] **Step 2: Fijar la línea de entorno cuando se conoce la ventana**

Donde se crea la `AppWindow` y se conoce su tamaño/escala (o justo tras mostrarla), llamar:
```rust
    // Línea de entorno para el log (versión/OS/ventana). Una vez.
    {
        let size = ui.window().size();             // PhysicalSize { width, height }
        let scale = ui.window().scale_factor();
        crate::logging::set_env_info((size.width, size.height), scale, &os_version_string());
    }
```
Y un helper para la versión de Windows:
```rust
/// Cadena corta de la versión del SO para el log. En Windows usa RtlGetVersion/GetVersionEx;
/// si falla, "Windows (desconocido)".
#[cfg(windows)]
fn os_version_string() -> String {
    // Forma simple y robusta: leer del registro CurrentVersion (sin API de versión, que miente
    // por manifest). Alternativa: dejar "Windows" + build si se obtiene fácil.
    "Windows".to_string()
}
#[cfg(not(windows))]
fn os_version_string() -> String { std::env::consts::OS.to_string() }
```
> El detalle exacto de la versión de Windows (10.0.19045) es deseable pero NO crítico: si
> obtenerlo limpio es complejo, deja `"Windows"` (o "Windows" + número de build si lo tienes a
> mano por otra vía ya usada en el proyecto). Lo esencial del entorno es versión de Naygo +
> tamaño de ventana, que sí tenemos. Reporta qué nivel de detalle lograste.

- [ ] **Step 3: Build + gate completo**
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform
$env:CARGO_BUILD_JOBS = "2"; cargo clippy --workspace --all-targets -- -D warnings
```
Esperado: compila, tests PASS, clippy sin warnings. Si quedó algún `#[allow(dead_code)]` de
Tasks anteriores que ahora SÍ se usa, quítalo.

- [ ] **Step 4: Verificación manual (opcional, recomendada)**

Compilar release, abrir la app, hacer algunas acciones (navegar, abrir config) y mirar
`naygo.log`: las líneas deben tener hora local legible. No hay panic que provocar de forma
fácil; el bloque de contexto se confirmará la próxima vez que ocurra un crash real. Documenta
en el reporte que el timestamp legible se ve bien.

- [ ] **Step 5: Commit**
```bash
git add crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(logging): fijar offset de huso y linea de entorno al arrancar

main.rs obtiene el offset del huso local (GetTimeZoneInformation) para los
timestamps legibles, y fija la linea de entorno (version/OS/ventana) cuando se
conoce la ventana. Asi el panic vuelca un contexto completo.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Verificación final del bloque

- [ ] Gate completo verde:
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform; cargo clippy --workspace --all-targets -- -D warnings
```
- [ ] Sin voseo en lo nuevo: `rg -ni "\b(arrastrá|escribí|elegí|podés|tenés|querés|hacé|volvé)\b"` en los archivos tocados → vacío.
- [ ] `graphify update .`.
- [ ] Una línea en «Sin publicar» del `CHANGELOG.md` (log más detallado: migas + contexto + hora legible).
- [ ] (Manual) confirmar timestamps legibles en `naygo.log` tras usar la app.

## Notas de cierre

- Actualizar memoria de proyecto al cerrar.
- Backlog que sigue pendiente: argumentos de CLI para `naygo.exe`.
