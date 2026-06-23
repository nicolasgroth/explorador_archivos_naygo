# Copiar/mover + panel de progreso — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Arreglar los 5 bugs del subsistema de copiar/mover de Naygo y construir un panel de operaciones rico (progreso por bytes, velocidad, ETA, pausa, cola, historial) más drag&drop entre paneles.

**Architecture:** El motor `core::ops` está sano (test lo prueba) y se mantiene. Se le agrega progreso por bytes durante la copia y un estado de pausa en el `CancellationToken`. La velocidad/ETA y todo el panel viven en la UI (`ops_ctrl` + un panel acoplado nuevo). El drag intra-app usa el drag nativo de Slint y reusa `core::dnd` + `ops::transfer`.

**Tech Stack:** Rust workspace (naygo-core / naygo-platform / naygo-ui-slint), Slint 1.16 render software, mpsc channels. Build con `CARGO_BUILD_JOBS=2`.

**Gate (correr SIEMPRE uno mismo tras cada subagente):**
```
$env:CARGO_BUILD_JOBS = "2"
cargo fmt
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**i18n:** triple (es + en en `crates/core/src/i18n/{es,en}.json` + props en `crates/ui-slint/ui/i18n.slint` + setters en `crates/ui-slint/src/i18n_keys.rs`), español neutral SIN voseo. NO reutilizar claves.

**REGLA para subagentes:** graphify antes de grep (`graphify query "<x>"` en bash); incluirla en cada prompt.

---

## Estructura de archivos

**Modificar (core):**
- `crates/core/src/cancel.rs` — `CancellationToken` gana estado `paused` + `pause()`/`resume()`/`is_paused()`/`wait_if_paused()`.
- `crates/core/src/ops/engine.rs` — `copy_buffered` emite progreso por bytes con throttle + respeta pausa.
- `crates/core/src/ops/mod.rs` — (si hace falta) helpers; `OpProgress` ya tiene los campos.
- `crates/core/src/format.rs` — helpers `format_speed(bytes_per_sec)` y `format_duration(secs)`.
- `crates/core/src/workspace/mod.rs` — `PanePurpose::Operations`.

**Modificar (ui-slint):**
- `crates/ui-slint/src/workspace_ctrl.rs` — `on_key_release` (reset modificadores); registrar panel Operations.
- `crates/ui-slint/src/ops_ctrl.rs` — VM ampliado, muestras de velocidad/ETA, `pause_op`/`resume_op`/`skip_op`, historial.
- `crates/ui-slint/src/main.rs` — cableado de teclas, panel, drag&drop.
- `crates/ui-slint/ui/types.slint` — `OpRowVm` ampliado.
- `crates/ui-slint/ui/ops-panel.slint` — panel rico de 3 zonas.
- `crates/ui-slint/ui/app-window.slint` / `file-panel.slint` — key-released, drag&drop, panel.
- i18n (es/en json + i18n.slint + i18n_keys.rs).

---

# FASE 1 — Bug 1: reset de modificadores

### Task 1: Resetear ctrl_down/shift_down en keyup

**Files:**
- Modify: `crates/ui-slint/ui/app-window.slint` (FocusScope key-released)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (on_key_release)
- Modify: `crates/ui-slint/src/main.rs` (cablear callback)

- [ ] **Step 1: Localizar el FocusScope del teclado y el callback on_key**

Corre: `graphify query "FocusScope key-pressed on_key ctrl_down teclado app-window"`.
Lee cómo `on_key` se declara en app-window.slint (callback `key(...)` o similar) y cómo lo recibe
main.rs (`ui.on_key(...)`). Hay un `key-pressed` en un FocusScope; necesitas agregar `key-released`.

- [ ] **Step 2: Agregar el callback key-released en Slint**

En `app-window.slint`, en el MISMO FocusScope que tiene `key-pressed`, agregar:
```slint
            key-released(ev) => {
                root.key-release(ev.text);
                return accept;
            }
```
Y declarar el callback en el AppWindow junto al `key` existente:
```slint
    callback key-release(string);
```
> Ajusta `ev.text` / el nombre del callback existente (`key`) a lo REAL. Mira cómo `key-pressed`
> pasa la tecla a Rust y replica para released.

- [ ] **Step 3: Implementar on_key_release en el controlador**

En `workspace_ctrl.rs`, agregar:
```rust
    /// Una tecla se soltó: limpiar los modificadores para que no queden pegados. Sin esto,
    /// `ctrl_down` se quedaba en true tras un Ctrl+C y el siguiente doble clic entraba por la
    /// rama "abrir en otro panel".
    pub fn on_key_release(&mut self, text: &str) {
        // Slint manda el "text" de la tecla; los modificadores no traen texto imprimible.
        // Estrategia robusta: en CUALQUIER key-released, si no hay teclas de modificador
        // sostenidas, limpiar ambos. Como Slint no nos da el estado real de los modificadores
        // aquí, el camino simple y seguro es: al soltar Control o Shift, limpiar el flag
        // correspondiente; y como red de seguridad, limpiar ambos si el text está vacío
        // (las teclas modificadoras no producen text).
        if text.is_empty() {
            self.ctrl_down = false;
            self.shift_down = false;
        }
    }
```
> VERIFICA cómo Slint reporta el texto de Control/Shift en key-released. Si Slint expone
> `ev.modifiers` en el evento, MEJOR: pasa los modificadores reales y setea `ctrl_down =
> ev.modifiers.control`. Investiga (`graphify query "KeyEvent modifiers slint"` o la doc de Slint
> 1.16) y usa el camino más fiable. El objetivo: tras soltar Ctrl, `ctrl_down` vuelve a false.

- [ ] **Step 4: Cablear en main.rs + defensa al abrir modales**

En main.rs, junto a `ui.on_key(...)`:
```rust
    {
        let ctrl = ctrl.clone();
        ui.on_key_release(move |text| {
            ctrl.borrow_mut().on_key_release(text.as_str());
        });
    }
```
Defensa extra: donde se abre un modal/overlay (config, paleta, diálogo de ops), resetear los
modificadores (`ctrl.borrow_mut().clear_modifiers()` — agrega ese método trivial que pone ambos
en false, o llama on_key_release con ""). Busca el patrón de apertura de modales y aplícalo.

- [ ] **Step 5: Gate + commit**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint; cargo clippy --workspace --all-targets -- -D warnings`
Expected: compila, clippy limpio. (Verificación visual en VM: Ctrl+C → doble clic en carpeta → navega.)
```bash
git add crates/ui-slint/ui/app-window.slint crates/ui-slint/src/workspace_ctrl.rs crates/ui-slint/src/main.rs
git commit -m "fix(ui): resetear ctrl_down/shift_down al soltar tecla (bug del doble clic)"
```
Termina el mensaje con la línea literal:
Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>

---

# FASE 2 — Bug 4+3: progreso por bytes + pausa + robustez (core)

### Task 2: Estado de pausa en CancellationToken

**Files:**
- Modify: `crates/core/src/cancel.rs`

> VERIFICADO: `CancellationToken { cancelled: Arc<AtomicBool> }` con `new()`/`cancel()`/`is_cancelled()`. Clone comparte estado.

- [ ] **Step 1: Tests de pausa (fallan)**

En el `#[cfg(test)]` de cancel.rs:
```rust
    #[test]
    fn pausar_y_reanudar() {
        let t = CancellationToken::new();
        assert!(!t.is_paused());
        t.pause();
        assert!(t.is_paused());
        t.resume();
        assert!(!t.is_paused());
    }

    #[test]
    fn pausa_se_comparte_entre_clones() {
        let t = CancellationToken::new();
        let c = t.clone();
        t.pause();
        assert!(c.is_paused(), "el clon comparte el estado de pausa");
    }

    #[test]
    fn wait_if_paused_retorna_si_cancelado_estando_pausado() {
        let t = CancellationToken::new();
        t.pause();
        t.cancel();
        // No debe colgar: si está pausado pero cancelado, wait_if_paused retorna en seguida.
        t.wait_if_paused();
        assert!(t.is_cancelled());
    }
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core pausar`
Expected: FAIL — métodos no existen.

- [ ] **Step 3: Implementar pausa + espera**

Reemplazar el struct y agregar los métodos:
```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Token de cancelación + pausa compartido. Barato de clonar (Arc).
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    // Para despertar al worker cuando se reanuda o se cancela (sin sondear con sleep).
    waker: Arc<(Mutex<()>, Condvar)>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        // Despertar a quien esté esperando en wait_if_paused.
        self.waker.1.notify_all();
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    /// Marca el token como pausado. El worker se suspenderá en su próximo wait_if_paused.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }
    /// Reanuda: el worker pausado continúa.
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.waker.1.notify_all();
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
    /// Si está pausado, BLOQUEA hasta que se reanude o se cancele. No quema CPU (condvar).
    /// Retorna de inmediato si no está pausado o si ya está cancelado.
    pub fn wait_if_paused(&self) {
        if !self.is_paused() || self.is_cancelled() {
            return;
        }
        let mut guard = self.waker.0.lock().unwrap();
        while self.is_paused() && !self.is_cancelled() {
            // Espera con timeout de seguridad por si una notificación se pierde.
            let (g, _timeout) = self
                .waker
                .1
                .wait_timeout(guard, std::time::Duration::from_millis(200))
                .unwrap();
            guard = g;
        }
    }
}
```

- [ ] **Step 4: Correr y ver que pasan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core pausar pausa_se wait_if`
Expected: PASS. Corre `cargo test -p naygo-core --lib` para confirmar que no rompiste otros usos
del token, y `cargo clippy -p naygo-core --all-targets -- -D warnings`.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/cancel.rs
git commit -m "feat(core): CancellationToken con pausa real (pause/resume/wait_if_paused)"
```
(con la línea Co-Authored-By literal).

---

### Task 3: copy_buffered emite progreso por bytes + respeta pausa

**Files:**
- Modify: `crates/core/src/ops/engine.rs`

> VERIFICADO: `copy_buffered(from, to, token)` (engine.rs:318) loop de 256KB hasta EOF. `OpMsg::Progress(OpProgress{bytes_done,bytes_total,files_done,files_total,current})`. El `tx` (Sender<OpMsg>) y los acumulados del plan están disponibles en el llamador `exec_copy_step`.

- [ ] **Step 1: Test de que emite progreso multi-chunk**

En el `#[cfg(test)]` de engine.rs, agregar (usa el patrón de `run`/`run_ask` existente para tener un `rx`):
```rust
    #[test]
    fn copia_grande_emite_progreso_durante_la_copia() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("big.bin");
        // 2 MiB → varios chunks de 256K → debe emitir más de un Progress.
        fs::write(&src, vec![7u8; 2 * 1024 * 1024]).unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = super::super::transfer(false, vec![src], dest.clone());
        let p = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        run_plan(&p, &OpKind::Copy, ConflictPolicy::Overwrite, &token, &tx, &crx, None);
        drop(tx);
        let progresos = rx.iter().filter(|m| matches!(m, OpMsg::Progress(_))).count();
        assert!(progresos >= 2, "esperaba >=2 Progress durante la copia, hubo {progresos}");
        assert_eq!(fs::read(dest.join("big.bin")).unwrap().len(), 2 * 1024 * 1024);
    }
```
> AJUSTA la firma de `run_plan` a la real (mira cómo la llama `run`/`run_ask`). Si `run_plan` no
> es accesible desde el test, usa el mismo helper que usan los otros tests (`run(req)`), pero ese
> drena el rx internamente; en ese caso, escribe el test usando el helper que SÍ expone el rx
> (como `run_ask`). Lo esencial: contar los `OpMsg::Progress` emitidos.

- [ ] **Step 2: Correr y ver que falla**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core copia_grande_emite_progreso`
Expected: FAIL — hoy se emite a lo sumo 1 Progress por archivo (antes de copiar).

- [ ] **Step 3: Emitir progreso por bytes dentro de copy_buffered + respetar pausa**

`copy_buffered` necesita acceso al `tx`, a los acumulados (bytes ya hechos de archivos previos,
total del plan, files_done/total, current path) para emitir un `OpProgress` correcto. Refactor:
pasar esos datos. La forma mínima: cambiar la firma de `copy_buffered` para recibir un closure o
los datos de progreso, y emitir con throttle. Implementación sugerida:

```rust
/// Copia `from` → `to` por bloques, chequeando cancelación y PAUSA entre bloques, y emitiendo
/// progreso por bytes con throttle (no en cada bloque). `on_bytes` recibe los bytes copiados de
/// ESTE archivo hasta ahora; el llamador los suma a los de archivos previos para el total.
fn copy_buffered(
    from: &Path,
    to: &Path,
    token: &CancellationToken,
    mut on_bytes: impl FnMut(u64),
) -> std::io::Result<bool> {
    let mut reader = std::fs::File::open(from)?;
    let mut writer = std::fs::File::create(to)?;
    let mut buf = vec![0u8; BUF_SIZE];
    let mut copied: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    loop {
        // Pausa: suspende sin cerrar el archivo. Cancelar despierta y sale.
        token.wait_if_paused();
        if token.is_cancelled() {
            return Ok(false);
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        copied += n as u64;
        // Throttle: emite a lo sumo ~cada 100 ms.
        if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
            on_bytes(copied);
            last_emit = std::time::Instant::now();
        }
    }
    writer.flush()?;
    on_bytes(copied); // emisión final con el total del archivo
    Ok(true)
}
```

Y en `exec_copy_step` (el llamador, engine.rs:287), pasar el closure que arma y envía el
`OpProgress` con los acumulados. Necesitas los acumulados de progreso del plan en ese punto: si
`exec_copy_step`/`exec_step` no los tiene, propágalos desde `run_plan` (que itera los pasos y ya
lleva la cuenta de bytes/archivos hechos — mira cómo arma el `OpMsg::Progress` de antes-de-copiar
en engine.rs:107 y reusa esa info). El closure:
```rust
        let prev_bytes = /* bytes de archivos ya terminados */;
        let total_bytes = /* plan.total_bytes */;
        let files_done = /* índice del archivo actual */;
        let files_total = /* plan.total_files */;
        let cur = from.clone();
        let on_bytes = |this_file: u64| {
            let _ = tx.send(OpMsg::Progress(OpProgress {
                bytes_done: prev_bytes + this_file,
                bytes_total: total_bytes,
                files_done,
                files_total,
                current: cur.clone(),
            }));
        };
```
> AJUSTA los nombres reales (`plan.total_bytes`, cómo `run_plan` lleva la cuenta). Lo esencial:
> el `bytes_done` del progreso = bytes de archivos previos + bytes de este archivo hasta ahora.
> Mantén la emisión "antes de copiar" (0% de este archivo) si ya existe, o quítala si el throttle
> ya cubre el primer tick.

- [ ] **Step 4: Correr y ver que pasa**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core copia_grande_emite_progreso`
Expected: PASS. Corre `cargo test -p naygo-core --lib` (no romper los tests de copia/conflicto
existentes) y clippy.

- [ ] **Step 5: Test de pausa funcional (opcional pero recomendado)**

Test: arrancar una copia en un hilo con un archivo grande, pausar, verificar que el tamaño del
destino deja de crecer un momento, reanudar, verificar que completa. Es time-dependent; si resulta
flaky, déjalo como `#[ignore]` con comentario. Lo crítico (emisión de progreso) ya está en Step 1.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/ops/engine.rs
git commit -m "feat(core): copy_buffered emite progreso por bytes (throttle) y respeta la pausa"
```
(con la línea Co-Authored-By literal).

---

### Task 4: Helpers de formato (velocidad + duración)

**Files:**
- Modify: `crates/core/src/format.rs`

> VERIFICADO: `format_size(bytes, fmt)` y `human_size(bytes)` ya existen aquí.

- [ ] **Step 1: Tests (fallan)**

En el `#[cfg(test)]` de format.rs:
```rust
    #[test]
    fn velocidad_legible() {
        assert!(format_speed(0).contains("0"));
        assert!(format_speed(125_000_000).contains("MB/s"));
    }

    #[test]
    fn duracion_legible() {
        assert_eq!(format_duration(0), "00:00");
        assert_eq!(format_duration(108), "01:48");      // 1 min 48 s
        assert_eq!(format_duration(3661), "1:01:01");   // 1 h 1 min 1 s
    }
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core velocidad_legible duracion_legible`
Expected: FAIL.

- [ ] **Step 3: Implementar**

En format.rs:
```rust
/// Velocidad legible a partir de bytes/segundo: reusa `human_size` y añade "/s".
pub fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", human_size(bytes_per_sec))
}

/// Duración legible: mm:ss si < 1 h, h:mm:ss si >= 1 h.
pub fn format_duration(total_secs: u64) -> String {
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}
```

- [ ] **Step 4: Correr y ver que pasan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core velocidad_legible duracion_legible`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/format.rs
git commit -m "feat(core): format_speed y format_duration para el panel de operaciones"
```
(con la línea Co-Authored-By literal).

---

# FASE 3 — Bug 4/5: VM ampliado + panel de operaciones rico (UI)

### Task 5: PanePurpose::Operations

**Files:**
- Modify: `crates/core/src/workspace/mod.rs` (PanePurpose)

- [ ] **Step 1: Agregar la variante**

Corre `graphify query "PanePurpose variantes Tree Preview Inspector panel"` y lee el enum.
Agregar `Operations` al enum `PanePurpose` (junto a Tree/Preview/Inspector/History/Favorites).
Si hay un `match` exhaustivo sobre PanePurpose (label, serialización, ícono), agregar el brazo
`Operations` en TODOS (busca `PanePurpose::Preview` para encontrarlos). Para serde, mantener
retro-compat (los layouts viejos no tienen Operations; serde default debe tolerarlo).

- [ ] **Step 2: Gate**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core; cargo build -p naygo-ui-slint`
Expected: compila (si la UI hace match sobre PanePurpose, habrá que agregar el brazo allí también
en la Task 7 — por ahora, si rompe el build de UI con "non-exhaustive", agrega un brazo mínimo
temporal y anótalo; o hazlo en esta task si es directo).

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/workspace/mod.rs
git commit -m "feat(core): PanePurpose::Operations para el panel de operaciones"
```
(con la línea Co-Authored-By literal).

---

### Task 6: VM ampliado + cálculo de velocidad/ETA + métodos pause/resume/skip

**Files:**
- Modify: `crates/ui-slint/ui/types.slint` (OpRowVm)
- Modify: `crates/ui-slint/src/ops_ctrl.rs` (OpRowData, muestras, métodos)

- [ ] **Step 1: Ampliar OpRowVm en types.slint**

Corre `graphify query "OpRowVm campos types.slint panel operaciones"`. El struct hoy tiene
index/label/percent/status/running. Ampliar a:
```slint
struct OpRowVm {
    index: int,
    label: string,
    percent: float,
    status: string,        // "en cola" / "en curso" / "pausada" / "esperando decisión" / "hecho: …"
    running: bool,
    paused: bool,
    bytes-done: string,    // ya formateado ("12,4 GB")
    bytes-total: string,   // ("94,9 GB")
    files-done: int,
    files-total: int,
    current-file: string,
    speed: string,         // "123 MB/s"
    speed-peak: string,    // "126 MB/s"
    eta: string,           // "~11:08" o ""
    elapsed: string,       // "01:48"
    kind: int,             // 0=en curso 1=en cola 2=historial (para decidir qué zona)
}
```

- [ ] **Step 2: Ampliar OpRowData + acumular muestras de velocidad/ETA**

En ops_ctrl.rs:
- `OpRowData` (el struct Rust que mapea a OpRowVm) gana los mismos campos (como String los ya
  formateados; ints donde corresponde).
- `ActiveOp` gana un historial de muestras para velocidad: un `Vec<(Instant, u64)>` (timestamp,
  bytes_done) o, más simple, `started_at: Instant`, `last_sample: Option<(Instant, u64)>`, y
  `peak_speed: u64`. Al recibir un `OpProgress` en `poll`, calcular velocidad instantánea
  (Δbytes/Δt), actualizar `peak_speed`, y velocidad media = bytes_done / elapsed. ETA =
  (bytes_total - bytes_done) / velocidad_media (si > 0).
> NOTA: `Instant::now()` no está disponible en core (que prohíbe el reloj), pero ESTO es la UI
> (ui-slint), donde sí se puede usar `std::time::Instant`. Confírmalo.
- En `op_rows()` (que hoy descarta los datos), llenar TODOS los campos: formatear bytes con
  `naygo_core::format::format_size`, velocidad con `format_speed`, tiempos con `format_duration`.

- [ ] **Step 3: Métodos pause_op/resume_op/skip_op**

En ops_ctrl.rs, junto a `cancel_op`:
```rust
    pub fn pause_op(&mut self, op_index: usize) {
        if let Some(op) = self.active_ops.get(op_index) {
            op.token.pause();
        }
    }
    pub fn resume_op(&mut self, op_index: usize) {
        if let Some(op) = self.active_ops.get(op_index) {
            op.token.resume();
        }
    }
```
Para `skip_op` (saltar el archivo actual y seguir): el motor no tiene "saltar archivo en curso"
hoy. Opción mínima viable: `skip_op` = cancelar SOLO si es un archivo (poco útil con 1 archivo) —
mejor, DIFERIR el "saltar archivo en vivo" y dejar el botón Saltar para cuando hay varios archivos
(salta al siguiente). Si el motor no lo soporta, implementa skip como "marcar el archivo actual
para saltar" vía un flag en el token o un canal, o DOCUMENTA que Saltar por ahora solo aplica
entre archivos de un lote (no corta un archivo único a la mitad). Reporta cómo lo resolviste.
> Para no bloquear: si "saltar archivo en vivo" es complejo, implementa Pausar/Reanudar/Cancelar
> completos y deja Saltar como no-op documentado o solo-entre-archivos; márcalo en el reporte.

- [ ] **Step 4: Historial reciente**

`OpsCtrl` mantiene las ops terminadas con su resumen para el historial (tope ~20). Hoy
`prune_finished` las quita; en su lugar, al terminar una op, mover su resumen a un `Vec`
`recent_history` (tope 20, descartar las más viejas). `op_rows()` incluye las del historial con
`kind=2`. Ajusta `prune_finished` para no perder el historial.

- [ ] **Step 5: Gate**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio.

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/ui/types.slint crates/ui-slint/src/ops_ctrl.rs
git commit -m "feat(ui): VM de ops ampliado + velocidad/ETA + pause/resume + historial"
```
(con la línea Co-Authored-By literal).

---

### Task 7: Panel de operaciones rico (ops-panel.slint) + registrar el panel + auto-aparecer

**Files:**
- Modify: `crates/ui-slint/ui/ops-panel.slint` (3 zonas)
- Modify: `crates/ui-slint/src/main.rs` (cablear callbacks pause/resume/skip/cancel + auto-aparecer)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (registrar PanePurpose::Operations, auto-add)
- Modify: `crates/ui-slint/ui/app-window.slint` (menú Panel ▾ + instanciar el panel)
- i18n.

- [ ] **Step 1: Panel rico de 3 zonas**

Reescribir `ops-panel.slint`. Recibe `in property <[OpRowVm]> rows;` y callbacks
`pause(int)`/`resume(int)`/`skip(int)`/`cancel(int)`. Estructura:
```slint
// Encabezado: "OPERACIONES — N en curso · M en cola"
// Zona EN CURSO (rows con kind==0): por cada op → ítem actual, barra con %, "copiado {bytes-done}
//   de {bytes-total}", "Velocidad {speed} (pico {speed-peak})", "Transcurrido {elapsed}",
//   "Restante {eta}", botones según estado: Pausar/Reanudar + Saltar + Cancelar.
// Zona EN COLA (kind==1): fila compacta con label + tamaño + "en espera" + cancelar.
// Zona HISTORIAL (kind==2): fila con ✓/⚠ + label + resumen ("N copiados, M saltados, K fallidos").
```
Usa piezas reutilizables (un `component OpRow`, una barra, un bloque de datos) para que agregar un
campo futuro sea trivial. Íconos por Path (no glifos). Colores del Theme. Botón Pausar muestra
"Reanudar" si `row.paused`.

- [ ] **Step 2: Registrar el panel Operations + auto-aparecer**

En `workspace_ctrl.rs`: el panel `PanePurpose::Operations` se puede agregar al layout (como Tree).
Cuando `start_op` arranca una operación, si no hay un panel Operations en el layout, AGREGARLO
(auto-aparecer). Mira cómo se agrega un panel de un purpose dado (el menú "Panel ▾" usa
`add_pane_of` o similar) y reúsalo. Respeta que si el usuario lo cerró, no ser intrusivo en exceso
(criterio: aparece al iniciar una op; el usuario puede cerrarlo).
> Agrega `Operations` al menú "Panel ▾" (que lista los purposes que se pueden añadir).

- [ ] **Step 3: Cablear en main.rs**

- Pasar `ops_ctrl.op_rows()` mapeadas a `[OpRowVm]` al panel (en el refresco / poll).
- Callbacks `pause/resume/skip/cancel` → `ctrl.pause_op(i)` etc. + refrescar.
- Auto-aparecer: tras `start_op`, llamar la lógica de agregar el panel si no está.

- [ ] **Step 4: i18n**

Claves nuevas (es/en + i18n.slint + i18n_keys.rs): `ops.title` ("Operaciones"), `ops.in_progress`
("En curso"), `ops.queued` ("En cola"), `ops.history` ("Historial reciente"), `ops.pause`
("Pausar"), `ops.resume` ("Reanudar"), `ops.skip` ("Saltar"), `ops.cancel` ("Cancelar"),
`ops.copied_of` ("{done} de {total}"), `ops.speed` ("Velocidad"), `ops.peak` ("pico"),
`ops.elapsed` ("Transcurrido"), `ops.remaining` ("Restante"), `ops.waiting` ("en espera"),
`ops.summary` ("{done} copiados, {skipped} saltados, {failed} fallidos"). en: traducir todas.
NO reutilizar claves. (Algunas pueden ya existir del panel viejo — reúsalas si encajan, NO las dupliques.)

- [ ] **Step 5: Gate**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio.

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/ui/ops-panel.slint crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl.rs crates/ui-slint/ui/app-window.slint crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): panel de operaciones rico (en curso/cola/historial) + auto-aparecer"
```
(con la línea Co-Authored-By literal).

---

# FASE 4 — Bug 2: drag&drop intra-app

### Task 8: Drag nativo Slint entre paneles

**Files:**
- Modify: `crates/ui-slint/ui/file-panel.slint` (zona de suelte + inicio de arrastre)
- Modify: `crates/ui-slint/src/main.rs` (callbacks drop) + `workspace_ctrl.rs` (enrutar a ops::transfer)

- [ ] **Step 1: Investigar el drag&drop nativo de Slint 1.16**

Corre `graphify query "row-drag-out file-panel drag arrastre OLE start_drag"` y mira cómo se inicia
hoy el arrastre (Sense drag → on_row_drag_out → start_drag OLE). Investiga el soporte de
drag&drop INTERNO de Slint 1.16 (¿hay `DropArea`/`drag`?). Si Slint NO tiene zona-de-suelte nativa
usable en render software, ve directo al FALLBACK de hit-testing (Step 3-alt). DOCUMENTA qué
encontraste antes de implementar.

- [ ] **Step 2: Payload interno + zona de suelte (camino preferido)**

Si Slint soporta drop interno: al arrastrar una fila, además del OLE, marcar en el controlador un
"drag interno activo" con `selected_paths()` del panel origen y su PaneId. Cada panel Files es
zona-de-suelte: al soltar, leer el payload, y llamar:
```rust
    // En workspace_ctrl: ejecutar un drop intra-app del panel `from_pane` al `to_pane`.
    pub fn drop_internal(&mut self, to_pane: PaneId, ctrl: bool, shift: bool) -> bool {
        let Some(paths) = self.internal_drag.take() else { return false; };
        let Some(dest) = self.pane_dir(to_pane) else { return false; };
        // Mismo disco mueve / otro copia; Ctrl/Shift fuerzan (lógica ya existente).
        let same = naygo_core::dnd::same_drive_of(&paths, &dest); // AJUSTA al helper real
        let action = naygo_core::dnd::decide_drop_action(ctrl, shift, same);
        let is_move = matches!(action, naygo_core::dnd::DropAction::Move);
        let req = naygo_core::ops::transfer(is_move, paths, dest);
        self.start_op_from_request(req); // arranca el motor + panel (reusa start_op)
        true
    }
```
> AJUSTA `decide_drop_action`/`same_drive`/`DropAction`/`transfer`/`start_op` a las firmas REALES
> (corre `graphify query "decide_drop_action DropAction same_drive transfer core::dnd"`). NO
> sueltes sobre el mismo panel origen (no-op). Resalta el panel destino al pasar el arrastre.

- [ ] **Step 3 (FALLBACK): hit-testing por coordenadas**

Si el drop nativo de Slint no alcanza: trackear los rects de pantalla de cada panel (por frame) y,
al terminar el arrastre (drag_stopped), calcular sobre qué panel cayó el cursor y llamar
`drop_internal`. Documenta cuál camino usaste.

- [ ] **Step 4: Gate**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio. (La mecánica de drag se valida en VM.)

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/ui/file-panel.slint crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl.rs
git commit -m "feat(ui): drag&drop entre paneles (mismo disco mueve / otro copia)"
```
(con la línea Co-Authored-By literal).

---

# FASE 5 — Cierre

### Task 9: CHANGELOG + guía + gate final + dist

**Files:**
- Modify: `CHANGELOG.md`, `docs/GUIA-DE-USUARIO.md`

- [ ] **Step 1: CHANGELOG**

En `CHANGELOG.md`, bajo "### Corregido" y "### Añadido" de la sección en curso:
```
### Corregido
- Copiar/mover archivos grandes ahora muestra el avance real (antes parecía detenerse): barra de
  progreso por bytes, velocidad y tiempo restante. El doble clic en una carpeta vuelve a navegar
  aunque antes se haya usado un atajo con Ctrl.
### Añadido
- Panel de Operaciones: muestra la copia/movimiento en curso (archivo actual, copiado X de Y,
  velocidad media y pico, transcurrido y restante) con botones Pausar/Reanudar, Saltar y Cancelar,
  además de la cola pendiente y un historial reciente. Aparece solo al iniciar una operación.
- Pausar y reanudar una copia en curso.
- Arrastrar archivos de un panel a otro: dentro del mismo disco mueve, a otro disco copia
  (Ctrl fuerza copiar, Shift fuerza mover).
```

- [ ] **Step 2: Guía de usuario**

Agregar a `docs/GUIA-DE-USUARIO.md` (sección de operaciones/copiar) el panel de Operaciones, pausa,
y el drag&drop entre paneles. Español neutral sin voseo.

- [ ] **Step 3: Gate final**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings
```
Expected: PASS, clippy limpio.

- [ ] **Step 4: Grafo + commit docs**

```
graphify update .
git add CHANGELOG.md docs/GUIA-DE-USUARIO.md
git commit -m "docs: panel de operaciones, pausa y drag&drop entre paneles"
```
(con la línea Co-Authored-By literal).

- [ ] **Step 5: Dist**

Run: `$env:CARGO_BUILD_JOBS = "2"; powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1`
Expected: `dist/Naygo-0.1.0-portable.zip` + `dist/Naygo-0.1.0-setup.exe`.

(El push lo hace Nicolás. **Verificación crítica en la VM:** copiar la VM de 95 GB real con
conflicto+sobrescribir → el progreso AVANZA visible, copia COMPLETA, Pausar/Reanudar funciona.
Plus: Ctrl+C→doble clic navega (Bug 1); arrastrar panel→panel copia/mueve (Bug 2); cola e
historial se ven (Bug 5).)

---

## Notas para el implementador

- **SIEMPRE correr el gate uno mismo tras cada subagente** (no confiar en su reporte).
- **graphify antes de grep** (hook obligatorio); inclúyelo en prompts de subagentes.
- **El motor está sano** — no reescribir la lógica de copia; solo agregar progreso + pausa.
- **Velocidad/ETA en la UI, no en el core** (`Instant` no está en core; sí en ui-slint).
- **Slint 1.16**: drag&drop interno es lo más incierto (Task 8) — investigar primero, fallback de
  hit-testing si hace falta. ToolBtn tooltip = `tip`. Íconos por Path.
- **i18n triple, sin voseo, sin reutilizar claves.**
- `OpMsg` viaja por `mpsc::channel`; `ops_ctrl` drena con `try_recv` en el poll.
- Un solo dist al final (Task 9). Nicolás hace el push.
