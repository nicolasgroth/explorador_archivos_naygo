# sizing — Tamaño de carpeta bajo demanda (F3) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** F3 calcula el tamaño total de las carpetas seleccionadas (o la enfocada), de forma recursiva, async y cancelable, mostrando el acumulado en vivo en la columna Tamaño.

**Architecture:** `core::sizing` hace el recorrido recursivo en un worker cancelable (con una pila propia, sin seguir symlinks, marcando "parcial" ante accesos denegados), con la suma pura (`dir_size_walk`) testeable inyectando el lister. La UI lanza un job por carpeta, drena los mensajes por frame y escribe el resultado en `Entry.size` (la columna existente lo muestra). Esc cancela; F5 re-lista y recalcula lo que tenía. F3→`Action::ComputeSize` ya está en el keymap (fase atajos); este plan implementa su arm (hoy no-op).

**Tech Stack:** Rust, `naygo-core`/`naygo-ui`, `eframe`/`egui` 0.34.3, `std::thread`/`mpsc`, `CancellationToken`. Sin chrono, sin dependencias nuevas.

**Estado de partida (rama `feat/sizing`, desde `main`):**
- `naygo_core::fs_model::Entry { name, path: PathBuf, kind: EntryKind, size: Option<u64>, modified, created, hidden }`. `EntryKind { Directory, File, Other }`. Carpetas tienen `size: None`.
- `naygo_core::cancel::CancellationToken`: `new()`, `cancel(&self)`, `is_cancelled(&self) -> bool` (clonable, compartible entre hilos).
- `naygo_core::format::human_size(u64) -> String`.
- `naygo_core::listing`: patrón worker (`spawn_listing`). `read_dir` se usa allí.
- `crates/ui/src/app.rs`:
  - `apply_action` (~1264): `Action::ComputeSize => {}` (no-op, línea ~1306 — A REEMPLAZAR). `Action::CancelListing` (~1287): `if let Some(id)=active_id { if let Some(l)=self.listings.get(&id) { l.token.cancel(); } }` — A AMPLIAR para cancelar size_jobs.
  - `selected_paths(&self) -> Vec<PathBuf>` (~1313): rutas seleccionadas del panel activo (o la enfocada si no hay selección). Devuelve rutas reales (no "..").
  - `refresh_pane(&mut self, id: PaneId, dir: PathBuf)` (~687): limpia entries/foco y re-lista.
  - `self.workspace.active_id() -> Option<PaneId>`; `active_files() -> Option<&FilePaneState>`; `pane_mut(id).and_then(|p| p.files.as_mut())`. `FilePaneState { entries: Vec<Entry>, focused, selected, ... }`, `focused_view_entry() -> Option<&Entry>`.
  - `self.settings`. `self.i18n`. Pump loop (~2070): `pump_all/pump_tree/pump_ops/pump_paste_write/pump_disk_usage/pump_watchers/pump_devices`; repaint guard con varias condiciones.
  - El dock viewer `NaygoTabViewer` (docking.rs) se construye con campos `&self.disk_usage`, `new_items_at_end`, etc. — patrón para pasar datos al file panel.
- `crates/ui/src/panes/file_panel.rs`: `cell_text(entry, kind)` (~509) → `ColumnKind::Size => format_size(entry)`; `format_size(entry)` (~531) → `human_size(entry.size)` o "". `file_panel::show(ui, workspace, id, pending, icons, show_parent_entry, i18n, table_actions, theme, ops_actions, new_items_at_end)` (la firma ganó params en watcher/shell-A — verificar la actual). Llamado desde `docking.rs`.
- `crates/core/src/config/mod.rs`: `Settings` con patrón aditivo `#[serde(default = "fn")]` + manual `impl Default`. `read_json`/`write_json`. `CONFIG_VERSION=1`.
- `crates/ui/src/settings_window/advanced.rs`: secciones de Settings (ops, paste, watcher) — patrón `app.tr(key)` + widgets + `app.settings.*`.

**Prerequisito:** Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo. `cargo fmt --all -- --check`. Bash: NO `cd /d`.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. Header de 2 líneas en NUEVOS. `core` NUNCA importa egui/windows. UI no bloquea (worker). Cancelable siempre. FS hostil (sin panics). Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt antes de cada commit. SIEMPRE `cargo fmt --all`. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/sizing`. NO cambiar de rama.

**Alcance:** ENTRA: core::sizing (dir_size_walk puro + spawn_dir_size worker), Settings size_no_subdirs, Action::ComputeSize real + pump_sizing + Esc + F5-recalcula, sufijo parcial en la celda Size, opción en Configuración, i18n. NO ENTRA: caché de tamaños, calcular todo el panel, seguir symlinks, shell-B.

---

## Estructura de archivos

```
crates/core/src/
├── sizing.rs        # NUEVO: SizeMsg, dir_size_walk (puro), spawn_dir_size (worker) + tests
├── config/mod.rs    # + size_no_subdirs
├── lib.rs           # + pub mod sizing;
└── i18n/{es,en}.json # + size.partial_suffix + settings.size_no_subdirs

crates/ui/src/
├── app.rs           # Action::ComputeSize real; size_jobs/sized_paths/size_partial; pump_sizing; Esc cancela; F5 recalcula
├── panes/file_panel.rs  # sufijo "(parcial)" en celda Size para paths en size_partial
├── docking.rs       # pasar size_partial al file panel (como disk_usage)
└── settings_window/advanced.rs  # opción size_no_subdirs
```

---

## Task 1: `core::sizing` — dir_size_walk (puro) + SizeMsg

**Files:**
- Create: `crates/core/src/sizing.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear sizing.rs con tipos + dir_size_walk + tests (TDD)**

Create `crates/core/src/sizing.rs`:
```rust
// Naygo — tamaño de carpeta bajo demanda: recorrido recursivo cancelable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Calcula el tamaño total de una carpeta sumando sus archivos descendientes. El
//! recorrido es CANCELABLE (token) y usa una pila propia (no recursión de stack, para
//! árboles profundos). NO sigue symlinks/junctions (evita loops y doble conteo). Una
//! entrada ilegible se salta marcando el resultado como "parcial". El worker
//! (`spawn_dir_size`) corre fuera del hilo de UI; `dir_size_walk` es la lógica pura.

use crate::cancel::CancellationToken;
use std::path::PathBuf;

/// Mensaje del worker de cálculo de tamaño.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SizeMsg {
    /// Acumulado parcial (bytes) mientras avanza (throttled por el worker).
    Progress { bytes: u64 },
    /// Terminó. `total` bytes; `partial` = hubo entradas saltadas (permiso/error).
    Done { total: u64, partial: bool },
    /// Cancelado; `bytes` = lo acumulado hasta el corte.
    Cancelled { bytes: u64 },
}

/// Una entrada lista por el "lister": (ruta, es_dir, es_symlink, tamaño_si_archivo).
/// `size` es `Some` para archivos (bytes) y `None` para carpetas/inaccesibles.
pub struct WalkEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: Option<u64>,
}

/// Resultado de listar un directorio: las entradas, o `None` si no se pudo leer
/// (permiso denegado, desapareció) → cuenta como "parcial".
pub type ListResult = Option<Vec<WalkEntry>>;

/// Suma recursiva PURA del tamaño de `root`. `lister(&Path) -> ListResult` produce las
/// entradas de un directorio (en producción lee el FS; en tests, un closure). Si
/// `recursive`, baja a subcarpetas (salvo symlinks). Usa una pila propia. Chequea
/// `token` entre directorios. `on_progress(bytes)` se llama con el acumulado (el worker
/// le pone throttle; en tests puede no hacer nada). Devuelve `(total, partial, cancelled)`.
pub fn dir_size_walk(
    root: &std::path::Path,
    recursive: bool,
    lister: &dyn Fn(&std::path::Path) -> ListResult,
    token: &CancellationToken,
    on_progress: &mut dyn FnMut(u64),
) -> (u64, bool, bool) {
    let mut total: u64 = 0;
    let mut partial = false;
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if token.is_cancelled() {
            return (total, partial, true);
        }
        match lister(&dir) {
            None => {
                // No se pudo listar este directorio → parcial.
                partial = true;
            }
            Some(entries) => {
                for e in entries {
                    if e.is_symlink {
                        continue; // no seguir symlinks/junctions
                    }
                    if e.is_dir {
                        if recursive {
                            stack.push(e.path);
                        }
                        // (carpeta en sí no aporta bytes)
                    } else {
                        match e.size {
                            Some(b) => total = total.saturating_add(b),
                            None => partial = true, // archivo ilegible
                        }
                    }
                }
                on_progress(total);
            }
        }
    }
    (total, partial, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Construye un lister desde un mapa ruta→entradas para testear sin FS.
    fn lister_from<'a>(map: &'a HashMap<PathBuf, Vec<WalkEntry>>) -> impl Fn(&std::path::Path) -> ListResult + 'a {
        move |p: &std::path::Path| map.get(p).map(|v| {
            v.iter()
                .map(|e| WalkEntry { path: e.path.clone(), is_dir: e.is_dir, is_symlink: e.is_symlink, size: e.size })
                .collect()
        })
    }

    fn file(p: &str, size: u64) -> WalkEntry {
        WalkEntry { path: PathBuf::from(p), is_dir: false, is_symlink: false, size: Some(size) }
    }
    fn dir(p: &str) -> WalkEntry {
        WalkEntry { path: PathBuf::from(p), is_dir: true, is_symlink: false, size: None }
    }

    #[test]
    fn suma_recursiva() {
        // root: a.txt(10), sub/  ; sub: b.txt(20), c.txt(5)
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![file("root/a.txt", 10), dir("root/sub")]);
        m.insert(PathBuf::from("root/sub"), vec![file("root/sub/b.txt", 20), file("root/sub/c.txt", 5)]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, partial, cancelled) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert_eq!(total, 35);
        assert!(!partial && !cancelled);
    }

    #[test]
    fn no_recursivo_solo_primer_nivel() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![file("root/a.txt", 10), dir("root/sub")]);
        m.insert(PathBuf::from("root/sub"), vec![file("root/sub/b.txt", 20)]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, _, _) = dir_size_walk(std::path::Path::new("root"), false, &lister, &token, &mut prog);
        assert_eq!(total, 10, "no baja a sub");
    }

    #[test]
    fn no_sigue_symlink() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        let link = WalkEntry { path: PathBuf::from("root/link"), is_dir: true, is_symlink: true, size: None };
        m.insert(PathBuf::from("root"), vec![file("root/a.txt", 10), link]);
        m.insert(PathBuf::from("root/link"), vec![file("root/link/big.bin", 9999)]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, _, _) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert_eq!(total, 10, "no entra al symlink");
    }

    #[test]
    fn subdir_ilegible_marca_parcial() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![file("root/a.txt", 10), dir("root/secret")]);
        // "root/secret" NO está en el mapa → lister devuelve None → parcial.
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, partial, _) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert_eq!(total, 10);
        assert!(partial, "secret ilegible → parcial");
    }

    #[test]
    fn carpeta_vacia_es_cero() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        let mut prog = |_| {};
        let (total, partial, _) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert_eq!(total, 0);
        assert!(!partial);
    }

    #[test]
    fn token_cancelado_corta() {
        let mut m: HashMap<PathBuf, Vec<WalkEntry>> = HashMap::new();
        m.insert(PathBuf::from("root"), vec![dir("root/sub")]);
        m.insert(PathBuf::from("root/sub"), vec![file("root/sub/x", 100)]);
        let lister = lister_from(&m);
        let token = CancellationToken::new();
        token.cancel(); // cancelado de entrada
        let mut prog = |_| {};
        let (_, _, cancelled) = dir_size_walk(std::path::Path::new("root"), true, &lister, &token, &mut prog);
        assert!(cancelled);
    }
}
```

- [ ] **Step 2: Declarar el módulo**

Modify `crates/core/src/lib.rs`: add `pub mod sizing;`.

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core sizing` → 6 tests PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → clean. (Watch: `spawn_dir_size` no existe aún; `WalkEntry`/`SizeMsg`/`ListResult` pueden marcarse parcialmente sin usar fuera de tests — `pub` evita dead_code; si clippy se queja de `SizeMsg` sin uso, es `pub` así que no debería.)

- [ ] **Step 4: Commit**
```
git add crates/core/src/sizing.rs crates/core/src/lib.rs
git commit -m "feat(core): dir_size_walk (suma recursiva pura, cancelable) + SizeMsg

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `SizeMsg`/`WalkEntry`/`ListResult`/`dir_size_walk` EXACTOS (Tasks 2,4 depend).

---

## Task 2: `spawn_dir_size` — worker que envuelve dir_size_walk

**Files:**
- Modify: `crates/core/src/sizing.rs`

- [ ] **Step 1: Test smoke con tempfile (TDD)**

Añadir al `mod tests` de sizing.rs:
```rust
    #[test]
    fn spawn_dir_size_suma_un_arbol_real() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"hola").unwrap(); // 4
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("b.txt"), b"mundo!").unwrap(); // 6
        let token = CancellationToken::new();
        let rx = spawn_dir_size(dir.path().to_path_buf(), true, token);
        // Drenar hasta el mensaje final.
        let mut total = None;
        while let Ok(msg) = rx.recv() {
            if let SizeMsg::Done { total: t, .. } = msg {
                total = Some(t);
                break;
            }
            if let SizeMsg::Cancelled { .. } = msg {
                break;
            }
        }
        assert_eq!(total, Some(10));
    }

    #[test]
    fn spawn_dir_size_no_recursivo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"hola").unwrap(); // 4
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("b.txt"), b"mundo!").unwrap(); // 6 (no debe contarse)
        let token = CancellationToken::new();
        let rx = spawn_dir_size(dir.path().to_path_buf(), false, token);
        let mut total = None;
        while let Ok(msg) = rx.recv() {
            if let SizeMsg::Done { total: t, .. } = msg { total = Some(t); break; }
        }
        assert_eq!(total, Some(4));
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core sizing::tests::spawn` → ERROR: `spawn_dir_size` no existe.

- [ ] **Step 3: Implementar spawn_dir_size + el lister real (en sizing.rs)**
```rust
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// Throttle de emisión de `Progress`.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(150);

/// Lista un directorio del FS real para `dir_size_walk`. `None` si no se puede leer.
/// Usa `symlink_metadata` (NO sigue symlinks). Los archivos aportan su `len()`.
fn fs_lister(dir: &std::path::Path) -> ListResult {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut out = Vec::new();
    for ent in rd.flatten() {
        let path = ent.path();
        // symlink_metadata NO sigue el enlace: detectamos symlinks/junctions.
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => {
                // entrada ilegible: representarla como archivo sin tamaño → parcial.
                out.push(WalkEntry { path, is_dir: false, is_symlink: false, size: None });
                continue;
            }
        };
        let is_symlink = meta.file_type().is_symlink();
        let is_dir = meta.is_dir();
        let size = if !is_dir && !is_symlink { Some(meta.len()) } else { None };
        out.push(WalkEntry { path, is_dir, is_symlink, size });
    }
    Some(out)
}

/// Lanza el cálculo del tamaño de `dir` en un worker. Emite `Progress` (throttle ~150ms)
/// y un mensaje final (`Done`/`Cancelled`) por el canal devuelto. Cancelable vía `token`.
pub fn spawn_dir_size(
    dir: PathBuf,
    recursive: bool,
    token: CancellationToken,
) -> Receiver<SizeMsg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut last = Instant::now();
        let mut on_progress = |bytes: u64| {
            if last.elapsed() >= PROGRESS_THROTTLE {
                let _ = tx.send(SizeMsg::Progress { bytes });
                last = Instant::now();
            }
        };
        let (total, partial, cancelled) =
            dir_size_walk(&dir, recursive, &fs_lister, &token, &mut on_progress);
        let final_msg = if cancelled {
            SizeMsg::Cancelled { bytes: total }
        } else {
            SizeMsg::Done { total, partial }
        };
        let _ = tx.send(final_msg);
    });
    rx
}
```
NOTE: el `on_progress` captura `tx` por referencia — pero `tx` también se usa al final para el mensaje final. Como `on_progress` es un closure que vive dentro del hilo y se dropea antes del `tx.send(final_msg)`... NO: el closure `on_progress` toma `tx` por captura. Para evitar mover `tx` al closure, capturar `tx` por referencia en el closure: el closure es `&mut dyn FnMut`, vive en `dir_size_walk`; al volver, `tx` sigue disponible. Asegurar que `on_progress` use `&tx` (no mueva tx): declarar `let tx_ref = &tx;` y que el closure capture `tx_ref`. Ajustar para que compile (el closure debe NO mover `tx`; usar `let _ = tx.send(...)` dentro tomando `&tx`). Verificar el borrow: `on_progress` se pasa como `&mut on_progress` a `dir_size_walk`, y luego se usa `tx` — el closure debe capturar `tx` por referencia compartida. Si el borrow checker se queja, clonar `tx` (mpsc Sender es Clone): `let tx2 = tx.clone();` para el closure y `tx` para el final. SIMPLE: clonar.

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core sizing` → 8 PASS. `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/sizing.rs
git commit -m "feat(core): spawn_dir_size (worker cancelable + throttle)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `spawn_dir_size(PathBuf, bool, CancellationToken) -> Receiver<SizeMsg>` EXACTO (Task 4 depende).

---

## Task 3: `core::config` — size_no_subdirs

**Files:**
- Modify: `crates/core/src/config/mod.rs`

- [ ] **Step 1: Test (round-trip)**

En el `mod tests` de config, extender el round-trip existente para incluir `size_no_subdirs: true` y verificar que sobrevive serde.

- [ ] **Step 2: Implementar**

Modify `crates/core/src/config/mod.rs`: en `struct Settings`, añadir:
```rust
    /// Al calcular el tamaño de una carpeta (F3), NO bajar a subdirectorios (solo el
    /// primer nivel). Más barato. `false` (default) = recursivo.
    #[serde(default = "default_size_no_subdirs")]
    pub size_no_subdirs: bool,
```
Default fn:
```rust
fn default_size_no_subdirs() -> bool {
    false
}
```
Y en el manual `impl Default for Settings`, añadir `size_no_subdirs: false,`. Si el round-trip test construye Settings con todos los campos, añadirlo. CONFIG_VERSION sigue 1.

- [ ] **Step 3: Verificar + commit**

Run: `cargo test -p naygo-core config` → PASS. `cargo test -p naygo-core` → green. `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.
```
git add crates/core/src/config/mod.rs
git commit -m "feat(core): Settings size_no_subdirs (sizing no recursivo)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `size_no_subdirs` EXACTO (Task 4 depende).

---

## Task 4: UI — Action::ComputeSize + pump_sizing + Esc + F5

**Files:**
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: Estado en NaygoApp**

Add to `struct NaygoApp`:
```rust
    /// Cálculos de tamaño en curso, por (panel, carpeta).
    size_jobs: std::collections::HashMap<(PaneId, std::path::PathBuf), SizeJob>,
    /// Paths que tienen un tamaño calculado (para recalcular en F5), por panel.
    sized_paths: std::collections::HashMap<PaneId, std::collections::HashSet<std::path::PathBuf>>,
    /// Paths cuyo tamaño calculado es PARCIAL (hubo accesos denegados) — para el render.
    size_partial: std::collections::HashSet<std::path::PathBuf>,
```
And a struct near `ActiveOp`/`PaneListing`:
```rust
/// Un cálculo de tamaño de carpeta en curso.
struct SizeJob {
    rx: std::sync::mpsc::Receiver<naygo_core::sizing::SizeMsg>,
    token: CancellationToken,
}
```
Init in `new`: all three `HashMap::new()`/`HashSet::new()`. (`CancellationToken` is already imported in app.rs.)

- [ ] **Step 2: Action::ComputeSize (reemplaza el no-op)**

Replace the `Action::ComputeSize => {}` arm (~1306) with `Action::ComputeSize => self.compute_size()`, and add the method:
```rust
    /// Lanza el cálculo de tamaño de las carpetas seleccionadas (o la enfocada si es dir).
    fn compute_size(&mut self) {
        let Some(pane) = self.workspace.active_id() else { return };
        // Carpetas objetivo: las seleccionadas que sean dir; si la selección no aporta
        // carpetas, la entry enfocada si es dir.
        let dirs: Vec<std::path::PathBuf> = {
            let from_sel: Vec<std::path::PathBuf> = self
                .selected_paths()
                .into_iter()
                .filter(|p| p.is_dir())
                .collect();
            if !from_sel.is_empty() {
                from_sel
            } else if let Some(e) = self
                .workspace
                .active_files()
                .and_then(|f| f.focused_view_entry())
            {
                if e.is_dir() { vec![e.path.clone()] } else { Vec::new() }
            } else {
                Vec::new()
            }
        };
        let recursive = !self.settings.size_no_subdirs;
        for dir in dirs {
            // Si ya hay un job sobre esa carpeta, cancelarlo y relanzar.
            if let Some(old) = self.size_jobs.remove(&(pane, dir.clone())) {
                old.token.cancel();
            }
            let token = CancellationToken::new();
            let rx = naygo_core::sizing::spawn_dir_size(dir.clone(), recursive, token.clone());
            self.size_jobs.insert((pane, dir), SizeJob { rx, token });
        }
    }
```

- [ ] **Step 3: pump_sizing**

Add:
```rust
    /// Drena los cálculos de tamaño y escribe el resultado en el Entry de cada carpeta.
    fn pump_sizing(&mut self) {
        if self.size_jobs.is_empty() {
            return;
        }
        // Recolectar updates sin prestar size_jobs mientras mutamos el workspace.
        let mut updates: Vec<(PaneId, std::path::PathBuf, u64, Option<bool>)> = Vec::new();
        // (pane, dir, bytes, finished_partial) — finished_partial: Some(partial) si terminó.
        let mut finished: Vec<(PaneId, std::path::PathBuf)> = Vec::new();
        for ((pane, dir), job) in &self.size_jobs {
            while let Ok(msg) = job.rx.try_recv() {
                match msg {
                    naygo_core::sizing::SizeMsg::Progress { bytes } => {
                        updates.push((*pane, dir.clone(), bytes, None));
                    }
                    naygo_core::sizing::SizeMsg::Done { total, partial } => {
                        updates.push((*pane, dir.clone(), total, Some(partial)));
                        finished.push((*pane, dir.clone()));
                    }
                    naygo_core::sizing::SizeMsg::Cancelled { bytes } => {
                        updates.push((*pane, dir.clone(), bytes, Some(false)));
                        finished.push((*pane, dir.clone()));
                    }
                }
            }
        }
        for (pane, dir, bytes, fin) in updates {
            if let Some(f) = self.workspace.pane_mut(pane).and_then(|p| p.files.as_mut()) {
                if let Some(e) = f.entries.iter_mut().find(|e| e.path == dir) {
                    e.size = Some(bytes);
                }
            }
            if let Some(partial) = fin {
                if partial {
                    self.size_partial.insert(dir.clone());
                }
                self.sized_paths.entry(pane).or_default().insert(dir.clone());
            }
        }
        for key in finished {
            self.size_jobs.remove(&key);
        }
    }
```

- [ ] **Step 4: Llamar pump_sizing + repaint**

Junto a `self.pump_disk_usage();` añadir `self.pump_sizing();`. En la guarda de `request_repaint`, añadir `|| !self.size_jobs.is_empty()`.

- [ ] **Step 5: Esc cancela sizing**

En `apply_action` `Action::CancelListing` (~1287), tras cancelar el listing, cancelar los size_jobs:
```rust
            Action::CancelListing => {
                if let Some(id) = self.workspace.active_id() {
                    if let Some(l) = self.listings.get(&id) {
                        l.token.cancel();
                    }
                }
                // Cancelar también los cálculos de tamaño en curso.
                for job in self.size_jobs.values() {
                    job.token.cancel();
                }
            }
```
(Los jobs se drenan/quitan en pump_sizing cuando llega su `Cancelled`.)

- [ ] **Step 6: F5 re-lista + recalcula lo que tenía**

En `refresh_pane` (~687), tras re-listar, recalcular las carpetas que tenían tamaño:
```rust
    pub fn refresh_pane(&mut self, id: PaneId, dir: PathBuf) {
        if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.entries.clear();
            f.focused = None;
        }
        // Limpiar el estado de parcial de este panel (se recalculará).
        // (No se puede saber fácil qué paths son de este panel en size_partial; se limpia
        //  por path al re-registrar abajo. Para simplicidad: dejar size_partial; el re-listado
        //  pone size=None y el sufijo solo se pinta si el path está en entries Y en size_partial,
        //  pero como entries se limpió, no se pinta hasta que el recálculo vuelva a marcarlo.)
        let to_recompute: Vec<PathBuf> =
            self.sized_paths.get(&id).map(|s| s.iter().cloned().collect()).unwrap_or_default();
        // El panel ya no tiene esos tamaños (entries limpio); olvidar el registro previo.
        self.sized_paths.remove(&id);
        self.start_listing(id, dir);
        // Relanzar el cálculo de los que tenían tamaño y siguen siendo carpetas.
        let recursive = !self.settings.size_no_subdirs;
        for p in to_recompute {
            if p.is_dir() {
                self.size_partial.remove(&p);
                let token = CancellationToken::new();
                let rx = naygo_core::sizing::spawn_dir_size(p.clone(), recursive, token.clone());
                self.size_jobs.insert((id, p), SizeJob { rx, token });
            }
        }
    }
```
NOTE: there may be a race — `start_listing` re-lists asynchronously, so when `pump_sizing` later writes `e.size` into `entries`, the entry must exist. Since the listing worker populates entries over time and `pump_sizing` finds the entry by path (skipping if not yet present), the size will land once both the listing entry and the size result are in. The `find(|e| e.path == dir)` is a no-op if the entry isn't listed yet; the FINAL Done writes it, and if the entry arrives after, the size is lost for that path until next F3. ACCEPTABLE for F5-recompute (the common case: entries list fast, size takes longer). If you want robustness, pump_sizing could buffer pending sizes and apply when the entry appears — but YAGNI; the listing of one dir's immediate children is near-instant. Document it.

- [ ] **Step 7: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings` → clean; `cargo fmt --all` + `--check`.
MANUAL: F3 on a folder → its Size column fills (partial climbing → final). Esc mid-calc stops it. F5 re-lists and recomputes the ones that had a size. Multi-select several folders → each gets its size. Toggle `size_no_subdirs` → F3 sums only first level.

- [ ] **Step 8: Commit**
```
git add crates/ui/src/app.rs
git commit -m "feat(ui): F3 calcula tamaño de carpeta (worker, parcial en vivo, Esc, F5)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: UI — sufijo "(parcial)" en la celda Size

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/ui/src/docking.rs`
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: i18n**

Add to es.json/en.json (identical keys): ES `"size.partial_suffix": " (parcial)"`, EN `"size.partial_suffix": " (partial)"`.

- [ ] **Step 2: Pasar size_partial al file panel**

`file_panel::show` decides the Size cell via `cell_text(entry, kind)` which has no access to the partial set. Thread `size_partial: &HashSet<PathBuf>` into `show` (like `disk_usage`/`new_items_at_end` were threaded in shell-A/watcher). Steps:
- `crates/ui/src/docking.rs`: add a field to `NaygoTabViewer` `pub size_partial: &'a std::collections::HashSet<std::path::PathBuf>,` and pass it to the `file_panel::show(...)` call.
- `crates/ui/src/app.rs` (NaygoTabViewer construction): add `size_partial: &self.size_partial,`.
- `crates/ui/src/panes/file_panel.rs`: add param `size_partial: &std::collections::HashSet<std::path::PathBuf>` to `show`, and where the Size cell text is produced for a row, if the entry is a dir AND `entry.size.is_some()` AND `size_partial.contains(&entry.path)`, append the i18n suffix. Since `cell_text` is a free fn without i18n/partial, the cleanest: at the row-render site, special-case Size: compute `format_size(entry)` then, if partial, append `i18n.t("size.partial_suffix")`. Find where `cell_text`/`ColumnKind::Size` is rendered per row and inject the suffix there (the row has access to `entry`, `i18n`, and now `size_partial`).

- [ ] **Step 3: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green (i18n parity — the new key in BOTH files); `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all` + `--check`.
MANUAL: F3 on a folder containing a permission-denied subfolder → its size shows "X (parcial)".

- [ ] **Step 4: Commit**
```
git add crates/ui/src/panes/file_panel.rs crates/ui/src/docking.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): sufijo (parcial) en el tamaño de carpeta con accesos denegados

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: UI — opción size_no_subdirs en Configuración + cierre

**Files:**
- Modify: `crates/ui/src/settings_window/advanced.rs`
- Modify: `crates/core/src/i18n/{es,en}.json`
- Modify: `README.md`

- [ ] **Step 1: i18n + opción en Settings**

i18n (both): ES `"settings.size_no_subdirs": "Calcular tamaño solo del primer nivel (no subcarpetas)"`, EN `"settings.size_no_subdirs": "Compute size of first level only (no subfolders)"`.
In `advanced.rs`, near the watcher/paste groups, add a checkbox:
```rust
    let l_size = app.tr("settings.size_no_subdirs");
    let mut v = app.settings.size_no_subdirs;
    if ui.checkbox(&mut v, l_size).changed() {
        app.settings.size_no_subdirs = v;
    }
```
(Place under an existing group heading or a small "Tamaño" sub-area; match the file's pattern.)

- [ ] **Step 2: README**

READ the status block and replace with:
```markdown
> **Estado:** Fase sizing (tamaño de carpeta bajo demanda, F3) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-08-naygo-sizing-design.md`](docs/superpowers/specs/2026-06-08-naygo-sizing-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-08-naygo-sizing.md`](docs/superpowers/plans/2026-06-08-naygo-sizing.md).
> Operaciones (ops-A/B), paste, shell-A, watcher, atajos configurables y bloque visual completos.
```

- [ ] **Step 3: Verificación final**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings` → clean; `cargo fmt --all -- --check`; `cargo build --release -p naygo-ui`.
MANUAL end-to-end: F3 (selección/enfocada), parcial en vivo, Esc, F5 recalcula, opción no-subdirs, sufijo parcial.

- [ ] **Step 4: Commit y push**
```
git add crates/ui/src/settings_window/advanced.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json README.md
git commit -m "feat(ui): opción de no analizar subdirectorios + README (fase sizing)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/sizing
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| dir_size_walk puro (recursivo/no, no-symlink, partial, vacía, cancel) | 1 |
| SizeMsg | 1 |
| spawn_dir_size worker + throttle + fs_lister (symlink_metadata) | 2 |
| size_no_subdirs Settings | 3 |
| Action::ComputeSize (selección dirs / enfocada) | 4 |
| pump_sizing (Entry.size, partial, sized_paths) | 4 |
| Esc cancela sizing | 4 |
| F5 re-lista + recalcula lo que tenía | 4 |
| sufijo (parcial) en celda Size | 5 |
| opción en Configuración | 6 |
| i18n | 5, 6 |
| caché/todo-el-panel/symlinks FUERA | (no se tocan) |

**Notas de riesgo:**
- **tx en spawn_dir_size** (Task 2): el closure on_progress no debe mover `tx` (se usa para el msg final). Clonar `tx` para el closure (`Sender: Clone`). Documentado.
- **F5 race** (Task 4): el size puede llegar antes de que el listing repueble la entry; el `find` es no-op y el size se pierde para ese path hasta el siguiente F3. Aceptable (listing de hijos inmediatos es instantáneo); documentado. No sobre-ingeniar un buffer.
- **size_partial threading** (Task 5): pasar al file panel como disk_usage en shell-A; verificar la firma real de file_panel::show + NaygoTabViewer.
- **compute_size targets** (Task 4): `selected_paths` filtradas a `is_dir()`; si no hay dirs seleccionados, la enfocada si es dir. F3 sobre archivos = no-op.
- **request_repaint** mientras size_jobs activo, para drenar parciales sin input.
```
