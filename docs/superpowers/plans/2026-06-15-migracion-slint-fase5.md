# Fase 5 (Slint): integraciones con el SO — Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cablear en la capa Slint las integraciones con Windows que ya viven en
`naygo-platform` (watcher de carpeta y de dispositivos, drag&drop OLE en ambas direcciones,
autostart) y portar tray/splash desde la capa egui, sin pérdida de funcionalidad.

**Architecture:** La lógica pesada está en `naygo-platform`/`naygo-core`; F5 la conecta a la UI
Slint reusando el patrón worker-en-hilo + canal + tick (el hilo de UI nunca hace I/O). Los
watchers están dormidos salvo un evento real, que despierta la UI con un *waker*
(`slint::Weak<AppWindow>` + `slint::invoke_from_event_loop`). Bajo consumo siempre.

**Tech Stack:** Rust, Slint 1.16 (winit software), `naygo-core::listing` (apply_dir_events),
`naygo-platform::{dir_watch, device_watch, dnd, autostart}`, crate `tray-icon = "0.24.1"`.

**Convenciones (OBLIGATORIAS):**
- Antes de leer/grepear: `graphify query "<pregunta>"`. Tras cambios: `graphify update .`.
- Gate antes de CADA commit: `cargo test --workspace` + `cargo clippy --workspace --all-targets
  -- -D warnings` + `cargo fmt --all -- --check`.
- Stage EXPLÍCITO (nunca `git add -A`): `CLAUDE.md` y `graphify-out/` NO se commitean.
- Commits en español (heredoc), terminando con
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Header en archivos nuevos: `// Naygo — <desc>.` / `// Copyright (c) 2026 Nicolás Groth /
  ISGroth. MIT License.`
- i18n: todo texto nuevo a claves (`Tr` + es.json/en.json en paridad). Colores vía `Theme`.
- Probar la GUI en ESTA máquina (binario real, Win32 `Start-Process` + foreground por PID con
  `SetForegroundWindow`, screenshots/computer-use). Lanzar UNA instancia (matar las previas:
  el `HideWindow`/proceso puede quedar vivo). Nicolás mide RENDIMIENTO en la VM.
- Trabajar en `main` (como F2–F4). Push al cierre de cada bloque grande, autorizado por Nicolás.

**APIs reusadas (firmas verificadas):**
- `core::listing::apply_dir_events(entries: &mut Vec<Entry>, events: &[DirEvent], read_entry:
  &dyn Fn(&Path)->Option<Entry>) -> Vec<PathBuf>` (devuelve rutas NUEVAS para resaltar);
  `core::listing::entry_from_path(&Path)->Option<Entry>` (el `read_entry` de producción);
  `DirEvent::{Created,Removed,Modified,Renamed{from,to}}`.
- `platform::dir_watch::watch(dir:&Path, tx:Sender<Vec<DirEvent>>, waker:Waker)->WatchHandle`
  (`Waker = Arc<dyn Fn()+Send+Sync>`; Drop detiene; handle inerte si no se puede vigilar).
- `platform::device_watch::watch(tx:Sender<DeviceEvent>, waker:Waker)->DeviceWatchHandle`;
  `DeviceEvent::DrivesChanged`.
- `platform::dnd::start_drag(paths:&[PathBuf])->Result<DragOutcome,DndError>` (BLOQUEANTE: corre
  el bucle OLE hasta soltar); `DragOutcome::{Copied,Moved,Cancelled}`.
- `platform::autostart::{is_enabled()->bool, set_enabled(on:bool)->Result<(),String>}`.
- Tray a portar desde `crates/ui/src/tray.rs` (crate `tray-icon`): `TrayMsg::{Open,Exit}`,
  ícono embebido `assets/icons/naygo_icon.ico`, handlers globales `TrayIconEvent`/`MenuEvent`.

**Estructura de archivos (qué se crea/toca):**
- `crates/ui-slint/src/watch.rs` (nuevo): estado de watchers de carpeta por panel + canal +
  helper de resaltado (set de rutas recientes con instante).
- `crates/ui-slint/src/devices.rs` (nuevo): watcher de dispositivos + reubicar paneles.
- `crates/ui-slint/src/tray.rs` (nuevo): port del tray a Slint (waker = `slint::Weak`).
- `crates/ui-slint/src/workspace_ctrl.rs` (mod): integra watch/devices, `should_quit_on_close`.
- `crates/ui-slint/src/main.rs` (mod): arranca watchers, drena en el tick, cierre correcto,
  drag sacar/recibir, tray, splash.
- `crates/ui-slint/ui/file-panel.slint` (mod): gesto de arrastre OLE + resaltado de filas nuevas.
- `crates/ui-slint/ui/splash.slint` (nuevo) + `crates/ui-slint/ui/config-window.slint` (mod:
  toggle autostart).
- `crates/core/src/config/mod.rs` (mod): campo `autostart` en Settings (si no existe).

---

## Sub-fase 5A — Watcher de carpeta (refresh + resaltado)

### Task 1: módulo `watch.rs` — estado de watchers + resaltado

**Files:**
- Create: `crates/ui-slint/src/watch.rs`
- Modify: `crates/ui-slint/src/main.rs` (`mod watch;`)

- [ ] **Step 1: Orientar**

Run: `graphify query "apply_dir_events entry_from_path DirEvent FilePaneState entries sort"`.

- [ ] **Step 2: Crear `watch.rs`**

```rust
// Naygo — watchers de carpeta por panel (Fase 5A): vigilan la carpeta abierta de cada panel
// Files y aplican los cambios sin re-listar, resaltando los archivos nuevos un tiempo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
use naygo_core::listing::DirEvent;
use naygo_platform::dir_watch::{self, WatchHandle, Waker};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;

/// Vigila las carpetas abiertas (una por panel) y junta los eventos en un canal común.
/// Cada panel guarda su `WatchHandle` (Drop = deja de vigilar). Las rutas recién aparecidas
/// se recuerdan con su instante para pintarlas resaltadas un tiempo.
pub struct Watchers {
    handles: HashMap<u64, WatchHandle>, // clave: PaneId.0
    tx: Sender<(u64, Vec<DirEvent>)>,
    rx: Receiver<(u64, Vec<DirEvent>)>,
    /// Rutas resaltadas (recién aparecidas) con el instante de aparición, por panel.
    pub fresh: HashMap<u64, Vec<(PathBuf, Instant)>>,
}

impl Watchers {
    pub fn new() -> Watchers {
        let (tx, rx) = channel();
        Watchers { handles: HashMap::new(), tx, rx, fresh: HashMap::new() }
    }

    /// (Re)empieza a vigilar `dir` para el panel `pane`. Reemplaza el watcher anterior (su
    /// Drop lo detiene). `waker` despierta la UI al llegar un evento.
    pub fn watch(&mut self, pane: u64, dir: PathBuf, waker: Waker) {
        // Adaptar el canal de dir_watch (Vec<DirEvent>) a nuestro canal etiquetado por panel.
        let (pane_tx, pane_rx) = channel::<Vec<DirEvent>>();
        let h = dir_watch::watch(&dir, pane_tx, waker);
        self.handles.insert(pane, h);
        // Reenviar lo que emita pane_rx a nuestro tx etiquetado, en un hilo liviano.
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            while let Ok(batch) = pane_rx.recv() {
                if tx.send((pane, batch)).is_err() {
                    break;
                }
            }
        });
    }

    /// Deja de vigilar el panel `pane`.
    pub fn unwatch(&mut self, pane: u64) {
        self.handles.remove(&pane);
        self.fresh.remove(&pane);
    }

    /// Drena los eventos pendientes: devuelve (pane, eventos) acumulados.
    pub fn drain(&mut self) -> Vec<(u64, Vec<DirEvent>)> {
        let mut out = Vec::new();
        while let Ok(item) = self.rx.try_recv() {
            out.push(item);
        }
        out
    }

    /// Registra rutas nuevas para resaltar en el panel `pane`.
    pub fn mark_fresh(&mut self, pane: u64, paths: Vec<PathBuf>, now: Instant) {
        let v = self.fresh.entry(pane).or_default();
        for p in paths {
            v.push((p, now));
        }
    }

    /// ¿La ruta está resaltada (apareció hace menos de `secs`)? Limpia las vencidas.
    pub fn is_fresh(&mut self, pane: u64, path: &std::path::Path, secs: u64, now: Instant) -> bool {
        let Some(v) = self.fresh.get_mut(&pane) else {
            return false;
        };
        v.retain(|(_, t)| now.duration_since(*t).as_secs() < secs);
        v.iter().any(|(p, _)| p == path)
    }
}

impl Default for Watchers {
    fn default() -> Self {
        Watchers::new()
    }
}
```

- [ ] **Step 3: Registrar el módulo**

En `main.rs` agregar `mod watch;` junto a los otros `mod`.

- [ ] **Step 4: Test del resaltado por tiempo**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn fresh_caduca_por_tiempo() {
        let mut w = Watchers::new();
        let t0 = Instant::now();
        w.mark_fresh(1, vec![PathBuf::from("C:/a.txt")], t0);
        assert!(w.is_fresh(1, std::path::Path::new("C:/a.txt"), 3, t0));
        let later = t0 + Duration::from_secs(5);
        assert!(!w.is_fresh(1, std::path::Path::new("C:/a.txt"), 3, later));
    }
}
```

- [ ] **Step 5: Gate + commit**

```bash
cargo test -p naygo-ui-slint watch:: && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/src/watch.rs crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(slint): módulo de watchers de carpeta por panel (Fase 5A)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 2: WorkspaceCtrl posee Watchers + arranca/aplica eventos

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs`

- [ ] **Step 1: Orientar**

Run: `graphify query "WorkspaceCtrl start_listing navigate_active_to pump_listings sort_entries entries"`.

- [ ] **Step 2: Campo `watchers` + arrancar al listar**

En `WorkspaceCtrl` agregar `pub watchers: crate::watch::Watchers` (init `Watchers::new()` en
`new_in`). El waker se pasa desde `main.rs` (ver Task 3), así que `start_listing` recibe un
waker opcional o se arranca el watcher desde `main`. Decisión: **arrancar el watcher en
`main.rs`** tras cada navegación (main tiene el `slint::Weak`); `WorkspaceCtrl` solo expone
`apply_watch_events`.

```rust
/// Aplica un lote de eventos del watcher al panel `pane`: muta sus entries sin re-listar,
/// re-ordena, y devuelve las rutas nuevas (para resaltar). No-op si el panel no es Files.
pub fn apply_watch_events(&mut self, pane: PaneId, events: &[naygo_core::listing::DirEvent]) -> Vec<std::path::PathBuf> {
    let Some(f) = self.ws.pane_mut(pane).and_then(|p| p.files.as_mut()) else {
        return Vec::new();
    };
    let nuevas = naygo_core::listing::apply_dir_events(
        &mut f.entries,
        events,
        &|p| naygo_core::listing::entry_from_path(p),
    );
    let spec = f.sort;
    naygo_core::sort::sort_entries(&mut f.entries, &spec);
    nuevas
}
```

- [ ] **Step 3: Test de integración (aplicar evento)**

```rust
#[test]
fn watch_events_agregan_entry() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("viejo.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    let nuevo = tmp.path().join("nuevo.txt");
    std::fs::write(&nuevo, b"y").unwrap();
    let nuevas = c.apply_watch_events(id, &[naygo_core::listing::DirEvent::Created(nuevo.clone())]);
    assert_eq!(nuevas, vec![nuevo.clone()]);
    assert!(c.rows_of(id).iter().any(|r| r.name == "nuevo.txt"));
}
```

Run: `cargo test -p naygo-ui-slint watch_events_agregan_entry`. Esperado: PASS.

- [ ] **Step 4: Gate + commit**

```bash
git add crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(slint): WorkspaceCtrl aplica eventos del watcher al panel (Fase 5A)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 3: cablear el watcher en main.rs (waker Slint + tick + resaltado)

**Files:**
- Modify: `crates/ui-slint/src/main.rs`
- Modify: `crates/ui-slint/ui/file-panel.slint` (campo `fresh` en RowData ya existe como
  `highlight`? si no, agregar `highlight: bool` y pintarlo con `Theme.highlight`)
- Modify: `crates/ui-slint/ui/types.slint` (RowData gana `highlight: bool` si falta)

- [ ] **Step 1: Orientar**

Run: `graphify query "main timer pump_listings sync_rows to_row_data RowData start_listing navigate slint Weak invoke_from_event_loop"`.

- [ ] **Step 2: Waker Slint**

En `main.rs`, construir el waker una vez:

```rust
let waker: naygo_platform::dir_watch::Waker = {
    let ui_weak = ui.as_weak();
    let start_timer = start_timer.clone();
    std::sync::Arc::new(move || {
        let ui_weak = ui_weak.clone();
        let start_timer = start_timer.clone();
        // Despertar la UI dormida desde el hilo del watcher.
        let _ = slint::invoke_from_event_loop(move || {
            let _ = ui_weak;
            start_timer();
        });
    })
};
```

(Si `start_timer` no es `Send`, capturar solo `ui_weak` y, en el closure del event loop,
re-arrancar el timer vía un callback; alternativamente exponer un callback `on_wake` en la UI
que el waker invoque. Implementar la variante que compile: el event-loop closure corre en el
hilo de UI, así que ahí sí se puede tocar `start_timer`.)

- [ ] **Step 3: Arrancar el watcher tras navegar/listar**

Donde se llama `start_listing`/`navigate`, además: `ctrl.borrow_mut().watchers.watch(pane.0,
dir.clone(), waker.clone())`. Al cerrar un panel: `watchers.unwatch(pane.0)`.

- [ ] **Step 4: Drenar en el tick + aplicar + marcar fresh**

En el closure del timer, antes de `sync_rows()`:

```rust
let now = std::time::Instant::now();
let batches = ctrl.borrow_mut().watchers.drain();
for (pane, events) in batches {
    let nuevas = ctrl.borrow_mut().apply_watch_events(PaneId(pane), &events);
    ctrl.borrow_mut().watchers.mark_fresh(pane, nuevas, now);
}
```

- [ ] **Step 5: Resaltado en `to_row_data`**

`RowData` ya tiene/gana `highlight: bool`. En `sync_rows`, al construir las filas de un panel
Files, setear `highlight` consultando `watchers.is_fresh(pane, &entry.path,
settings.highlight_duration_secs, now)`. En el `.slint`, el fondo de la fila usa
`row.highlight ? Theme.highlight.with-alpha(...) : <fondo normal>`.

- [ ] **Step 6: Build + verificación viva**

Build debug, lanzar, navegar a una carpeta, crear un archivo desde otra ventana/PowerShell →
el panel debe mostrarlo solo y resaltado unos segundos. Capturar.

- [ ] **Step 7: Gate + commit**

```bash
git add crates/ui-slint/src/main.rs crates/ui-slint/ui/file-panel.slint crates/ui-slint/ui/types.slint crates/ui-slint/src/bridge.rs
git commit -F - <<'EOF'
feat(slint): watcher de carpeta en vivo — refresco automático + resaltado de nuevos (5A)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 5B — Watcher de dispositivos (USB)

### Task 4: módulo `devices.rs` + reubicar paneles

**Files:**
- Create: `crates/ui-slint/src/devices.rs`
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (reubicar paneles si su raíz desapareció)
- Modify: `crates/ui-slint/src/main.rs` (`mod devices;`, arrancar, drenar en tick)

- [ ] **Step 1: Orientar**

Run: `graphify query "device_watch DeviceEvent DrivesChanged drives build_tree DirTree from_drives panes current_dir"`.

- [ ] **Step 2: Crear `devices.rs`**

```rust
// Naygo — watcher de dispositivos (Fase 5B): detecta enchufar/quitar unidades (USB) y avisa
// para re-escanear unidades y reubicar paneles cuya raíz desapareció.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
use naygo_platform::device_watch::{self, DeviceEvent, DeviceWatchHandle, Waker};
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct Devices {
    _handle: DeviceWatchHandle,
    rx: Receiver<DeviceEvent>,
}

impl Devices {
    /// Arranca la vigilancia de dispositivos. `waker` despierta la UI dormida.
    pub fn start(waker: Waker) -> Devices {
        let (tx, rx): (Sender<DeviceEvent>, Receiver<DeviceEvent>) = channel();
        let handle = device_watch::watch(tx, waker);
        Devices { _handle: handle, rx }
    }

    /// ¿Llegó al menos un evento de cambio de unidades desde el último drenado?
    pub fn drivesChanged(&self) -> bool {
        let mut changed = false;
        while self.rx.try_recv().is_ok() {
            changed = true;
        }
        changed
    }
}
```

(Renombrar `drivesChanged` a `drives_changed` para respetar snake_case de Rust.)

- [ ] **Step 3: `reubicar_paneles_invalidos` en WorkspaceCtrl**

```rust
/// Tras un cambio de unidades, reubica a HOME los paneles Files cuya carpeta ya no existe
/// (p. ej. el USB se sacó). Devuelve los panes reubicados (para re-listar).
pub fn relocate_orphans(&mut self, home: &std::path::Path) -> Vec<PaneId> {
    let mut moved = Vec::new();
    let ids: Vec<PaneId> = self.ws.files_panes();
    for id in ids {
        let gone = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| !f.current_dir.exists())
            .unwrap_or(false);
        if gone {
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.navigate_to(home.to_path_buf());
            }
            moved.push(id);
        }
    }
    moved
}
```

- [ ] **Step 4: Test de reubicación**

```rust
#[test]
fn relocate_reubica_panel_huerfano() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("usb");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new_in(sub.clone(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    std::fs::remove_dir_all(&sub).unwrap(); // "sacar el USB"
    let moved = c.relocate_orphans(tmp.path());
    assert_eq!(moved.len(), 1);
    assert_eq!(c.ws.active_files().unwrap().current_dir, tmp.path());
}
```

Run: `cargo test -p naygo-ui-slint relocate_reubica_panel_huerfano`. Esperado: PASS.

- [ ] **Step 5: Cablear en main.rs**

`mod devices;`. Arrancar `let devices = devices::Devices::start(waker.clone());` (guardar en
una variable viva por toda la sesión). En el tick: si `devices.drives_changed()` → re-`build_tree`
de cada panel Tree, `relocate_orphans(home)` y re-listar los reubicados; `sync_layout()`.

- [ ] **Step 6: Gate + commit**

```bash
git add crates/ui-slint/src/devices.rs crates/ui-slint/src/workspace_ctrl.rs crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(slint): watcher de dispositivos (USB) — re-escanea unidades y reubica paneles huérfanos (5B)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 5C — Drag&drop OLE (sacar)

### Task 5: gesto de arrastre → start_drag

**Files:**
- Modify: `crates/ui-slint/ui/file-panel.slint` (detectar inicio de arrastre sobre fila)
- Modify: `crates/ui-slint/ui/app-window.slint` (callback `row-drag-out(int)`)
- Modify: `crates/ui-slint/src/main.rs` (wirear → `dnd::start_drag`)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (helper `selected_paths_for_drag`)

- [ ] **Step 1: Orientar**

Run: `graphify query "file-panel pointer-event row TouchArea selected_paths dnd start_drag DragOutcome"`.

- [ ] **Step 2: Detectar inicio de arrastre en la fila**

En `file-panel.slint`, en el `pointer-event` de la fila: tras un `down` sobre una fila YA
seleccionada, si llega un `move` con desplazamiento > umbral (p. ej. 6px) mientras el botón
sigue abajo, emitir `root.row-drag-out(root.pane-id)` UNA vez (flag para no repetir). NO romper
el clic/doble-clic ya existente (solo dispara si hubo movimiento real con el botón presionado).

- [ ] **Step 3: Helper de rutas en WorkspaceCtrl**

```rust
/// Las rutas seleccionadas del panel `pane` (o la enfocada si no hay selección), para arrastrar.
pub fn drag_paths(&self, pane: PaneId) -> Vec<std::path::PathBuf> {
    self.ws
        .pane(pane)
        .and_then(|p| p.files.as_ref())
        .map(|f| f.selected_paths_or_focused())
        .unwrap_or_default()
}
```

(Si `selected_paths_or_focused` no existe en core, usar la API que ya consume el copiar/cortar
de F3 — `selected_paths` del WorkspaceCtrl — para no duplicar lógica.)

- [ ] **Step 4: Wirear en main.rs**

```rust
{
    let ctrl = ctrl.clone();
    ui.on_row_drag_out(move |pane| {
        let paths = ctrl.borrow().drag_paths(PaneId(pane as u64));
        if paths.is_empty() {
            return;
        }
        // start_drag es bloqueante (bucle OLE nativo hasta soltar). Devuelve el resultado.
        let _ = naygo_platform::dnd::start_drag(&paths);
    });
}
```

- [ ] **Step 5: Build + verificación viva**

Build, lanzar, seleccionar archivos, arrastrarlos al Explorer/escritorio → deben copiarse allá.
(Verificación con computer-use: press sostenido + move; si el arrastre sintético es frágil, dejar
constancia y confiar en que `start_drag` está unit-probado en platform.)

- [ ] **Step 6: Gate + commit**

```bash
git add crates/ui-slint/ui/file-panel.slint crates/ui-slint/ui/app-window.slint crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(slint): drag&drop OLE — sacar archivos de Naygo hacia el Explorer/escritorio (5C)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 5D — Drag&drop OLE (recibir)

### Task 6: registrar drop-target + soltar en panel → copiar/mover

**Files:**
- Create: `crates/ui-slint/src/drop_target.rs` (registro OLE del HWND + lectura CF_HDROP)
- Modify: `crates/ui-slint/src/main.rs` (registrar tras crear la ventana; wirear el drop)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (lanzar op de copia/mover a un panel)

- [ ] **Step 1: Orientar**

Run: `graphify query "dnd FilesDataObject hdrop RegisterDragDrop IDropTarget naygo_hwnd ops transfer copy move into pane"`.
Leer `crates/platform/src/dnd.rs` (FilesDataObject/CF_HDROP) para reusar la lectura de rutas.

- [ ] **Step 2: Módulo `drop_target.rs`**

Implementar un `IDropTarget` COM que, al `Drop`, lea las rutas CF_HDROP y las envíe por un canal
`Sender<(Vec<PathBuf>, bool /*shift=mover*/, (f32,f32) /*punto*/)>`. Registrar con
`RegisterDragDrop(hwnd, &target)` usando el HWND de `naygo_hwnd(&ui)` (helper de F3). Exponer
`register(hwnd, tx) -> DropTargetGuard` (revoca en Drop con `RevokeDragDrop`).
NOTA DE RIESGO (del spec): si el registro choca con winit, FALLBACK acotado — drop a nivel de
ventana → panel ACTIVO. Documentar en el header del módulo cuál se tomó.

- [ ] **Step 3: Lanzar la op al soltar**

En `WorkspaceCtrl`:

```rust
/// Recibe rutas externas soltadas sobre el panel `dest`: copia (o mueve si `move_`) a su
/// carpeta, reusando el engine de operaciones (diálogos/progreso/cancelación de F3).
pub fn drop_external(&mut self, dest: PaneId, sources: Vec<std::path::PathBuf>, move_: bool) {
    let Some(dir) = self.ws.pane(dest).and_then(|p| p.files.as_ref()).map(|f| f.current_dir.clone()) else {
        return;
    };
    let req = if move_ {
        naygo_core::ops::actions::transfer(sources, dir, naygo_core::ops::OpKind::Move)
    } else {
        naygo_core::ops::actions::transfer(sources, dir, naygo_core::ops::OpKind::Copy)
    };
    self.ops.start_op(req);
}
```

(Ajustar a la firma real de `ops::actions::transfer` / `start_op` que ya usa F3; confirmarla con
graphify antes de escribir.)

- [ ] **Step 4: Wirear en main.rs**

Registrar el drop-target tras crear la ventana; en el tick, drenar el canal del drop-target y
llamar `drop_external(panel_bajo_el_punto_o_activo, sources, move_)`; `start_timer()` para
mostrar el progreso.

- [ ] **Step 5: Build + verificación viva**

Build, lanzar, arrastrar un archivo desde el Explorer y soltarlo en un panel → debe copiarse a
esa carpeta (con su diálogo de conflicto si ya existe). Con Shift → mover.

- [ ] **Step 6: Gate + commit**

```bash
git add crates/ui-slint/src/drop_target.rs crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(slint): drag&drop OLE — recibir archivos soltados en un panel (copia/mueve, reusa ops) (5D)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 5E — Tray + cerrar-a-bandeja + autostart

### Task 7: campo `autostart` en Settings + toggle en config

**Files:**
- Modify: `crates/core/src/config/mod.rs` (campo `autostart`)
- Modify: `crates/ui-slint/src/config_ctrl.rs` (setter que aplica + persiste)
- Modify: `crates/ui-slint/ui/config-window.slint` (toggle en General) + types/i18n

- [ ] **Step 1: Orientar**

Run: `graphify query "Settings autostart serde default config_ctrl set_ setting General config-window SettingsVm"`.

- [ ] **Step 2: Campo en Settings**

En `core::config::Settings`, agregar:

```rust
/// Iniciar Naygo con Windows (entrada Run del registro). `#[serde(default)]` retro-compat.
#[serde(default)]
pub autostart: bool,
```

(Y en el `Default for Settings`, `autostart: false`.)

- [ ] **Step 3: Setter en ConfigCtrl que aplica al SO**

```rust
/// Activa/desactiva el inicio con Windows: escribe la entrada Run y persiste el ajuste.
pub fn set_autostart(&mut self, on: bool) {
    if naygo_platform::autostart::set_enabled(on).is_ok() {
        self.settings.autostart = on;
        self.save();
    }
}
```

- [ ] **Step 4: Toggle en la UI**

`SettingsVm` gana `autostart: bool`; `build_settings_vm` lo llena con `s.autostart`. En
config-window.slint, categoría General, agregar un `Field { label: Tr.cfg-autostart; CheckBox {
checked: root.vm.autostart; toggled => { root.set-autostart(self.checked); } } }`. Agregar
callback `set-autostart(bool)`, su clave i18n `slint.cfg.autostart` (es: "Iniciar con Windows",
en: "Start with Windows"), y wirear `on_cfg_set_autostart` en main.rs.

- [ ] **Step 5: Test del setter (con guard)**

```rust
#[test]
fn set_autostart_persiste_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    {
        let mut c = ConfigCtrl::new(dir.clone());
        c.settings.autostart = true; // probamos solo la persistencia del flag
        c.save();
    }
    let c2 = ConfigCtrl::new(dir);
    assert!(c2.settings.autostart);
}
```

(No se testea el efecto en el registro real; `set_enabled` tiene su propio guard en platform.)

- [ ] **Step 6: Gate + commit**

```bash
git add crates/core/src/config/mod.rs crates/ui-slint/src/config_ctrl.rs crates/ui-slint/ui/config-window.slint crates/ui-slint/ui/types.slint crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/ui-slint/src/main.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -F - <<'EOF'
feat(slint): autostart (iniciar con Windows) — ajuste en Configuración + entrada Run (5E)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 8: tray + cierre correcto (quit real)

**Files:**
- Create: `crates/ui-slint/src/tray.rs` (port desde `crates/ui/src/tray.rs`)
- Modify: `crates/ui-slint/Cargo.toml` (dep `tray-icon = "0.24.1"`)
- Modify: `crates/ui-slint/src/main.rs` (crear tray si tray_enabled; drenar; cierre correcto)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (helper `should_quit_on_close`)

- [ ] **Step 1: Orientar**

Run: `graphify query "tray TrayMsg create load_icon TrayIconBuilder MenuEvent close_to_tray tray_enabled on_close_requested quit_event_loop"`.
Leer `crates/ui/src/tray.rs` (port). El ícono embebido: `assets/icons/naygo_icon.ico`.

- [ ] **Step 2: dep tray-icon**

En `crates/ui-slint/Cargo.toml`: `tray-icon = "0.24.1"`.

- [ ] **Step 3: `tray.rs` (port, waker Slint)**

Portar `create`/`load_icon`/`TrayMsg` de la versión egui, reemplazando `egui::Context` +
`request_repaint()` por un `waker` Slint: el handler empuja `TrayMsg` al canal y hace
`slint::invoke_from_event_loop(...)` (o usa el waker `Arc<dyn Fn()>` ya construido). Firma:
`create(open_label: &str, exit_label: &str, waker: Waker) -> Option<Tray>` con
`Tray { _icon, rx: Receiver<TrayMsg> }`.

- [ ] **Step 4: helper de cierre (puro, testeable)**

En `workspace_ctrl.rs` (o `tray.rs`):

```rust
/// ¿La app debe SALIR al cerrar la ventana? Sale salvo que se haya pedido "cerrar a bandeja" y
/// el tray esté activo (en cuyo caso se oculta a la bandeja).
pub fn should_quit_on_close(close_to_tray: bool, tray_active: bool) -> bool {
    !(close_to_tray && tray_active)
}
```

```rust
#[test]
fn cierre_sale_salvo_close_to_tray_con_tray() {
    assert!(should_quit_on_close(false, true));
    assert!(should_quit_on_close(true, false));
    assert!(!should_quit_on_close(true, true));
}
```

- [ ] **Step 5: arreglar `on_close_requested` en main.rs**

```rust
ui.window().on_close_requested({
    let ctrl = ctrl.clone();
    let tray_active = /* se setea al crear el tray */ ;
    move || {
        ctrl.borrow().save_session();
        let close_to_tray = ctrl.borrow().config.settings.close_to_tray;
        if workspace_ctrl::should_quit_on_close(close_to_tray, tray_active) {
            slint::quit_event_loop().ok();
            slint::CloseRequestResponse::HideWindow // tras quit, da igual; HideWindow evita doble-cierre
        } else {
            slint::CloseRequestResponse::HideWindow // a la bandeja
        }
    }
});
```

(Ajustar: `tray_active` debe ser un `bool`/`Rc<Cell<bool>>` capturado, true si el tray se creó.)

- [ ] **Step 6: crear tray + drenar en tick**

Si `settings.tray_enabled`: `let tray = tray::create(&t("slint.tray.open"), &t("slint.tray.exit"),
waker.clone());`. En el tick, drenar `tray.rx`: `TrayMsg::Open` → `ui.show()` + foreground;
`TrayMsg::Exit` → `slint::quit_event_loop()`. Claves i18n `slint.tray.open`="Abrir"/"Open",
`slint.tray.exit`="Salir"/"Exit".

- [ ] **Step 7: Build + verificación viva**

Build release (el tray es comportamiento de release/usuario). Verificar: con `close_to_tray=false`
cerrar → la app TERMINA (no queda proceso colgado, a diferencia de F4). Con `tray_enabled=true` +
`close_to_tray=true` → cerrar oculta a bandeja; clic en el ícono reabre; menú Salir termina.

- [ ] **Step 8: Gate + commit**

```bash
git add crates/ui-slint/src/tray.rs crates/ui-slint/Cargo.toml Cargo.lock crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl.rs crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -F - <<'EOF'
feat(slint): tray (bandeja) + cerrar-a-bandeja + cierre correcto (sale de verdad) (5E)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 5F — Splash de arranque

### Task 9: splash en release + verificación integral + push

**Files:**
- Create: `crates/ui-slint/ui/splash.slint`
- Modify: `crates/ui-slint/src/main.rs` (mostrar splash en release, cerrar por timer)

- [ ] **Step 1: Orientar**

Run: `graphify query "splash main AppWindow show window timer release debug_assertions"`.

- [ ] **Step 2: `splash.slint`**

Ventana mínima (logo emoji 📁 + "Naygo" + "© 2026 Nicolás Groth / ISGroth"), fondo `Theme.panel-bg`,
texto `Theme.text`. Un `export component Splash inherits Window` sin decoración
(`no-frame: true`), ~360×200.

- [ ] **Step 3: Mostrar en release**

En `main.rs`, solo en release (`#[cfg(not(debug_assertions))]`): crear el Splash, mostrarlo, y un
`slint::Timer::single_shot(Duration::from_millis(1200), || splash.hide())`. La ventana principal
se construye y muestra en paralelo (el splash no la bloquea).

- [ ] **Step 4: Build release + verificación viva**

Build release, lanzar → debe verse el splash ~1.2s y luego la ventana principal. En debug NO
aparece.

- [ ] **Step 5: Verificación integral de F5**

Verificar en vivo (capturando): watcher de carpeta (crear archivo externo → aparece resaltado);
drag sacar (a Explorer); drag recibir (desde Explorer); tray (cerrar a bandeja + reabrir + salir);
autostart (toggle); splash. Lo de hardware (USB) lo prueba Nicolás.

- [ ] **Step 6: dist + memoria + graphify + push**

```bash
cp target/release/naygo-slint.exe dist/slint-fase1/
# actualizar memory/project-migracion-slint.md con el estado de F5
graphify update .
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/ui/splash.slint crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(slint): splash de arranque (release) + cierre de la Fase 5

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
git push origin main
```

- [ ] **Step 7: avisar a Nicolás** — F5 funcional+verificada; pedir prueba de rendimiento + USB
  real + tray/autostart en la VM. Mencionar el fallback de 5D si se tomó.

---

## Self-review (cobertura del spec)

- 5A watcher de carpeta (refresh + resaltado) → Tasks 1–3. ✓
- 5B watcher de dispositivos (USB) → Task 4. ✓
- 5C drag&drop OLE sacar → Task 5. ✓
- 5D drag&drop OLE recibir → Task 6 (con fallback documentado). ✓
- 5E tray + cerrar-a-bandeja + autostart → Tasks 7–8 (incluye el arreglo del cierre de F4 vía
  `should_quit_on_close` + `quit_event_loop`). ✓
- 5F splash → Task 9. ✓
- Manejo de errores transversal (handle inerte, no-op, reubicar) → presente en 5A/5B/5C/5E. ✓
- Testing (puro + vivo) → tests en cada sub-fase + verificación viva en Tasks 3,5,6,8,9. ✓
- Bajo consumo (watchers dormidos + waker Slint) → Tasks 3,4,8. ✓

Sin placeholders: cada step tiene código o comando. Tipos consistentes: `Watchers`
(watch/unwatch/drain/mark_fresh/is_fresh), `apply_watch_events`, `Devices`
(start/drives_changed), `relocate_orphans`, `drag_paths`, `drop_external`, `should_quit_on_close`,
`set_autostart`, `TrayMsg`, `Tray` — mismos nombres en plan y self-review. Riesgo de 5D marcado
con fallback explícito.
```
