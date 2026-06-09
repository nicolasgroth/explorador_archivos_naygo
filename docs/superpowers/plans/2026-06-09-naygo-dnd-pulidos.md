# Drag & Drop + pulidos + autoría (Entrega 1) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Drag & drop completo (interno entre paneles + Explorer↔Naygo vía OLE), más pulidos chicos (persistencia del dock, minors de multi-selección, fila "..") y autoría explícita en los scripts.

**Architecture:** La decisión mover/copiar es PURA en `core` (testeable). El DnD interno usa el `dnd_drag_source`/`dnd_release_payload` de egui (ya usado para columnas) y dispara `transfer` (motor de ops existente). El DnD con el SO vive en `platform::dnd` (OLE: IDropTarget para recibir, IDataObject+DoDragDrop para sacar), siguiendo el patrón COM de `trash.rs`, reusando `build_hdrop_global` de clipboard. Los pulidos tocan `dock_translate` (from_dock_state), `file_panel` (M2), `input` (M5). Una rama, sub-faseada, merge al final.

**Tech Stack:** Rust, eframe/egui 0.34 + egui_dock 0.19, crate `windows` 0.62 (Win32 OLE/Shell/COM), `naygo-core`/`platform`/`ui`.

**Estado de partida (rama `feat/dnd-pulidos`, desde `main` cf52590):**
- `crates/ui/src/ops_actions.rs`: `pub fn transfer(kind_move: bool, sources: Vec<PathBuf>, dest_dir: PathBuf) -> OpRequest`. El DnD interno solo lo invoca; el motor de ops (core::ops) ya cancela/journaliza/resuelve conflictos.
- `crates/ui/src/panes/file_panel.rs`: usa `dnd_drag_source(id, payload, |ui|)` + `dnd_release_payload::<usize>()` + `dnd_hover_payload::<usize>()` para reordenar columnas (headers). El render de fila `DisplayRow::Entry(i)`: celda `ci == 0` = nombre (`icon_row`), captura `name_cell_rect: Cell<Option<Rect>>`. `clicked: Option<(usize,bool,bool)>` (pos,ctrl,shift). Multi-selección: `f.selected` (pos vista) pintado, `f.is_selected(i)`. La fn `show(id, workspace: &mut Workspace, ..., pending: &mut Vec<PaneRequest>, ...)` consume clic y rubber-band internamente (workspace.pane_mut(id)). Hay un `selected_set = f.selected.clone()` antes de la tabla. El rubber-band arranca en celdas NO-nombre/vacío.
- `crates/ui/src/app.rs`: `selected_paths() -> Vec<PathBuf>` (selección o foco). `NaygoApp.hwnd: Option<isize>` (de shell-B). `refresh_pane(id, dir)`. Los `pump_*` se drenan en `logic`. `self.active_dir()`, `self.workspace.active_id()`.
- `crates/platform/src/clipboard.rs`: dentro de `mod windows_impl` (línea 39), `unsafe fn build_hdrop_global(paths: &[PathBuf]) -> Result<HGLOBAL, ClipboardError>` construye DROPFILES+rutas UTF-16 doble-NUL (CF_HDROP). PRIVADO — hay que exponer una versión reusable para el IDataObject del DnD (o factorizar a un helper compartido).
- `crates/platform/src/trash.rs` / `context_menu.rs`: patrón COM (CoInitializeEx APARTMENTTHREADED, needs_uninit=hr.is_ok(), CoUninitialize solo si needs_uninit, unsafe acotado, errores tipados, nunca panic). `context_menu.rs` ya hizo IShellFolder/IContextMenu.
- `crates/platform/Cargo.toml`: `windows` 0.62 con `Win32_System_Ole`, `Win32_System_Com`, `Win32_UI_Shell`, `Win32_UI_Shell_Common`, `Win32_System_Memory`, `Win32_System_DataExchange`, `Win32_Foundation`, `Win32_System_LibraryLoader`, etc. Quizá falte alguna sub-feature de OLE drag (Win32_System_Ole cubre RegisterDragDrop/DoDragDrop/IDropTarget/IDropSource/IDataObject — verificar; añadir si el build lo pide).
- `crates/ui/src/dock_translate.rs`: `pub fn to_dock_state(layout: &SerializableDockLayout) -> DockState<PaneId>` existe; NO hay `from_dock_state`. `naygo_core::workspace::layout::{DockNode, SerializableDockLayout, SplitDir}`. egui_dock 0.19 `DockState<PaneId>`, `Tree`, `NodeIndex`. `Workspace.layout: SerializableDockLayout` se persiste (config::save_workspace) pero NO se actualiza desde el DockState vivo.
- `crates/ui/src/input.rs`: `chord_text(...)` con "Espacio" hardcoded (M5).
- `scripts/build-release.ps1`, `crates/ui/src/bin/gen_icons.rs`: tienen header MIT; falta autor explícito (C).
- Multi-selección minors: M2 (clic-derecho fuera de selección no reduce), M3/M4 (rubber-band scroll / temp-leak).

**Prerequisito de entorno:** Rust en PATH (`export PATH="$HOME/.cargo/bin:$PATH"`). NUNCA `2>&1` con cargo. `cargo fmt --all` antes de cada commit. Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt verdes. Header 2 líneas en archivos nuevos.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. COM/OLE SOLO en `platform`. Tolerante, nunca panic. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/dnd-pulidos`. NO cambiar de rama.

**Reparto de verificación:** el agente: tests puros (decide_drop_action, from_dock_state round-trip, build/clippy/fmt). La prueba VISUAL/interactiva (arrastres reales entre paneles y con Explorer, layout del dock tras reiniciar) la hace Nicolás. El OLE (DoDragDrop/IDropTarget) NO se puede probar headless.

**SUB-FASEO (orden):** A1 interno (Tasks 1-2) → A2-recibir (Task 3) → A2-sacar (Task 4) → pulidos (Tasks 5-7) → autoría (Task 8) → cierre (Task 9). El OLE de "sacar" (Task 4, DoDragDrop modal vs egui) es lo más incierto: si una vía falla, el implementer reporta y prueba la alternativa (documentada en la tarea).

---

## Estructura de archivos

```
crates/core/src/dnd.rs                    # NUEVO: decide_drop_action + same_drive (puro, testeado)
crates/core/src/lib.rs                    # + pub mod dnd
crates/platform/src/dnd.rs                # NUEVO: OLE — IDropTarget (recibir) + IDataObject/DoDragDrop (sacar)
crates/platform/src/lib.rs                # + pub mod dnd
crates/platform/src/clipboard.rs          # exponer build_hdrop_global reusable (o factorizar)
crates/ui/src/panes/file_panel.rs         # drag source en celda nombre + drop target del panel + banner/badge + M2
crates/ui/src/app.rs                       # disparar transfer al soltar; registrar/revocar IDropTarget; drenar drops del SO
crates/ui/src/dock_translate.rs            # from_dock_state + round-trip
crates/ui/src/input.rs                     # M5 ("Espacio")
scripts/build-release.ps1                  # autoría
crates/ui/src/bin/gen_icons.rs             # autoría
```

---

## Task 1: core::dnd — decide_drop_action + same_drive (puro, TDD)

**Files:**
- Create: `crates/core/src/dnd.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear dnd.rs con tests**
```rust
// Naygo — lógica pura de drag & drop: decidir mover vs copiar.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Decide si un drop mueve o copia, con las reglas del Explorador de Windows:
//! Shift = mover, Ctrl = copiar, sin tecla = mover en el mismo disco / copiar entre
//! discos distintos. Puro y testeable; la capa UI lee los modificadores y los discos.

use std::path::Path;

/// Acción resultante de un drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropAction {
    Move,
    Copy,
}

/// Decide la acción según modificadores y si origen/destino están en el mismo disco.
/// Prioridad: Shift→Move, Ctrl→Copy, si no: same_drive→Move, distinto→Copy.
/// (Si ambos modificadores, Shift gana — coincide con el comportamiento de Windows.)
pub fn decide_drop_action(ctrl: bool, shift: bool, same_drive: bool) -> DropAction {
    if shift {
        DropAction::Move
    } else if ctrl {
        DropAction::Copy
    } else if same_drive {
        DropAction::Move
    } else {
        DropAction::Copy
    }
}

/// ¿`a` y `b` están en el mismo disco/volumen? En Windows compara la letra de unidad
/// (case-insensitive). Si alguna no tiene prefijo de unidad reconocible, devuelve
/// `false` (conservador: trata como discos distintos → copiar, más seguro que mover).
pub fn same_drive(a: &Path, b: &Path) -> bool {
    fn drive_letter(p: &Path) -> Option<char> {
        let s = p.to_string_lossy();
        let mut chars = s.chars();
        let c = chars.next()?;
        if c.is_ascii_alphabetic() && chars.next() == Some(':') {
            Some(c.to_ascii_uppercase())
        } else {
            None
        }
    }
    match (drive_letter(a), drive_letter(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn shift_siempre_mueve() {
        assert_eq!(decide_drop_action(false, true, false), DropAction::Move);
        assert_eq!(decide_drop_action(true, true, false), DropAction::Move); // shift gana
    }
    #[test]
    fn ctrl_copia() {
        assert_eq!(decide_drop_action(true, false, true), DropAction::Copy);
    }
    #[test]
    fn sin_tecla_mismo_disco_mueve() {
        assert_eq!(decide_drop_action(false, false, true), DropAction::Move);
    }
    #[test]
    fn sin_tecla_distinto_disco_copia() {
        assert_eq!(decide_drop_action(false, false, false), DropAction::Copy);
    }
    #[test]
    fn same_drive_misma_letra() {
        assert!(same_drive(&PathBuf::from("C:\\a"), &PathBuf::from("C:\\b\\c")));
        assert!(same_drive(&PathBuf::from("c:\\a"), &PathBuf::from("C:\\b"))); // case-insensitive
    }
    #[test]
    fn same_drive_distinta_letra() {
        assert!(!same_drive(&PathBuf::from("C:\\a"), &PathBuf::from("D:\\b")));
    }
    #[test]
    fn same_drive_sin_letra_es_false() {
        assert!(!same_drive(&PathBuf::from("\\\\red\\share"), &PathBuf::from("C:\\a")));
    }
}
```

- [ ] **Step 2: Declarar el módulo**

In `crates/core/src/lib.rs` add `pub mod dnd;`.

- [ ] **Step 3: Verificar + commit**

Run: `cargo test -p naygo-core dnd` → 7 PASS. `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean. `cargo fmt --all`.
```
git add crates/core/src/dnd.rs crates/core/src/lib.rs
git commit -m "feat(core): dnd::decide_drop_action + same_drive (puro)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `DropAction{Move,Copy}`, `decide_drop_action(ctrl,shift,same_drive)`, `same_drive(a,b)` EXACTOS.

---

## Task 2: DnD interno entre paneles (egui)

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`

- [ ] **Step 1: Drag source en la celda del nombre**

In `file_panel.rs` `DisplayRow::Entry(i)`, in the `ci == 0` (name) cell, make the name a drag source carrying the view position. egui's `dnd_drag_source(id, payload, add_contents)`:
```rust
                                if ci == 0 {
                                    let key = icon_key_for(entry);
                                    let drag_id = egui::Id::new(("file_drag", id.0, i));
                                    let resp = ui
                                        .dnd_drag_source(drag_id, DragPayload { pane: id, pos: i }, |ui| {
                                            let _ = icon_row(ui, icons, key, &entry.name, name_color);
                                        })
                                        .response;
                                    name_cell_rect.set(Some(resp.rect));
                                }
```
Define a small payload type (in file_panel.rs or a shared module): `#[derive(Clone)] struct DragPayload { pane: PaneId, pos: usize }` — but the actual files to move come from the SOURCE pane's `selected_paths()` at drop time (multi-selection). Simpler: payload carries the source `PaneId` (the source pane's current selection is resolved at drop). VERIFY egui 0.34 `dnd_drag_source` signature and that the payload type is `Clone + Send + Sync + 'static` (PaneId is Copy). NOTE: this must NOT break the name-cell click (selection). egui's dnd_drag_source only initiates a drag on actual drag motion; a click still registers. VERIFY the click-vs-drag coexistence on the name cell (the row's `Sense::click` for selection + the name cell's drag source). If they conflict (drag source eats the click), report and adjust.

- [ ] **Step 2: Drop target = el panel; banner + badge**

After the table, detect a file-drag payload hovering over THIS panel and paint the banner (style B) + decide the action for the badge. On release, resolve and apply:
```rust
    // ¿Hay un arrastre de archivos sobre este panel? (payload de otro —o este— panel)
    let hover_payload = ui.ctx().drag_stopped_id(); // placeholder — ver mecánica real
```
The real mechanic (VERIFY against egui 0.34): use `egui::DragAndDrop::has_any_payload(ctx)` / `ctx.drag_stopped()` and `Response::dnd_hover_payload::<DragPayload>()` / `dnd_release_payload::<DragPayload>()` on a response covering the panel area. Concretely:
- Get a panel-area response (e.g. `ui.interact(panel_rect, id_for_panel, Sense::hover())` or reuse the rubber-band area response) to call `dnd_hover_payload::<DragPayload>()`.
- If hovering with a `DragPayload` whose `pane != id` (or even same pane — but same-pane drop is a no-op; skip if `payload.pane == id`), paint a thin banner at the top of the panel: "Soltar para {mover|copiar} aquí" (i18n) + compute the action via `decide_drop_action`. The badge following the cursor is optional/nice — at minimum the banner. (egui draws the dragged payload's `add_contents` near the cursor already, which serves as the "ghost".)
- On `dnd_release_payload::<DragPayload>()`: resolve sources = the SOURCE pane's `selected_paths`-equivalent (the source pane's selection, or just the dragged entry if no multi-selection — get the source pane via `workspace.pane(payload.pane)`), dest = this panel's `current_dir`, read modifiers (`ctx.input(|i| (i.modifiers.ctrl||command, i.modifiers.shift))`), `same = naygo_core::dnd::same_drive(first_source, dest)`, `action = decide_drop_action(...)`. Then push a transfer request (via the existing ops trigger path — same as how a paste/move is started). For LEFT button = direct. For RIGHT button drag → defer a small menu "Mover/Copiar/Cancelar" (egui 0.34: detect the pressed button via input; if right-drag, show a popup at release). 
NOTE: this is the trickiest UI part. The plan's mechanic names (`dnd_hover_payload`/`dnd_release_payload`) are the egui 0.34 API used elsewhere in this file for columns — REUSE that exact pattern (it's proven for `usize` payloads here; now with a `DragPayload` struct). VERIFY the right-button-drag detection; if egui doesn't expose which button started the drag cleanly, do LEFT=direct for this task and note right-button-menu as a follow-up within this task or a small deferral (report).

- [ ] **Step 3: Disparar el transfer**

The actual move/copy: build the source paths from the source pane's selection. Find how the file panel currently triggers an op (it defers to NaygoApp via `pending: &mut Vec<PaneRequest>` or calls into ops). Likely add a new `PaneRequest::DropTransfer { sources, dest, kind_move }` (or reuse an existing transfer request) that app.rs handles by calling `ops_actions::transfer(...)` + spawning the op (same path as copy/move today). VERIFY how moves are started today (Ctrl+X/V or F5/F6) and route the drop through the same engine. Add the `PaneRequest` variant + its handling in app.rs.

- [ ] **Step 4: i18n del banner**

Add to both json: `dnd.drop_move` ("Soltar para mover aquí"/"Drop to move here"), `dnd.drop_copy` ("Soltar para copiar aquí"/"Drop to copy here"), and if right-menu: `dnd.move_here`/`dnd.copy_here`/`dnd.cancel`. Identical keys both files.

- [ ] **Step 5: Verify**

Run: `cargo build --workspace` → compiles. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.
(Visual: Nicolás verifies drag between panes, banner, move/copy by key/drive, right-button menu.)

- [ ] **Step 6: Commit**
```
git add crates/ui/src/panes/file_panel.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): drag & drop interno entre paneles (mover/copiar con feedback)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
Report which egui dnd APIs you used, whether name-cell drag broke click-selection, whether right-button-drag menu landed or was deferred.

---

## Task 3: A2-recibir — IDropTarget (Explorer → Naygo)

**Files:**
- Create: `crates/platform/src/dnd.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: platform::dnd con IDropTarget**

Create `crates/platform/src/dnd.rs`. Implement a COM `IDropTarget` that, on `Drop`, extracts CF_HDROP paths from the `IDataObject` and sends them (with the drop screen position) through an `mpsc::Sender<DroppedFiles>` to the app (like `device_watch` passes a Sender). API:
```rust
pub struct DroppedFiles { pub paths: Vec<PathBuf>, pub screen_x: i32, pub screen_y: i32, pub effect_copy: bool }

/// Registra el drop target en la ventana `hwnd`. Devuelve un guard que al Drop hace
/// RevokeDragDrop. Requiere OLE inicializado (ver register). Tolerante: si falla,
/// devuelve None (la app sigue sin recibir drops externos).
#[cfg(windows)]
pub fn register_drop_target(hwnd: isize, tx: std::sync::mpsc::Sender<DroppedFiles>) -> Option<DropTargetGuard>;
```
Mechanics (VERIFY windows 0.62 signatures): `OleInitialize(None)` on the UI thread (NOT just CoInitialize — RegisterDragDrop needs OLE; balance with OleUninitialize, but careful: eframe/winit may already have initialized — follow the trash.rs needs_uninit pattern adapted to Ole). Implement `IDropTarget` (windows crate `#[implement(IDropTarget)]`) with DragEnter/DragOver (return DROPEFFECT_COPY or MOVE based on key state — for received files default Copy, or honor the source effect), DragLeave, Drop (read CF_HDROP via `DragQueryFileW` from the IDataObject's HGLOBAL → Vec<PathBuf>, send through tx, set *pdwEffect). `RegisterDragDrop(HWND, &target)`; the guard holds the HWND and calls `RevokeDragDrop` on Drop. Doc-comment the COM flow richly. Stub `#[cfg(not(windows))]` returns None.
(VERIFY: `windows` 0.62 `#[implement]` macro for IDropTarget, `RegisterDragDrop`/`RevokeDragDrop`/`OleInitialize` in Win32_System_Ole, `DragQueryFileW` in Win32_UI_Shell, `IDataObject::GetData` + FORMATETC for CF_HDROP. Add Cargo sub-features if the build asks. This is the delicate COM part — implement carefully, never panic, map failures to None/Err.)

- [ ] **Step 2: lib.rs + app.rs wiring**

`crates/platform/src/lib.rs`: `pub mod dnd;`. In `crates/ui/src/app.rs` `NaygoApp::new`: if `hwnd` is Some, `let (tx, rx) = mpsc::channel(); let drop_guard = naygo_platform::dnd::register_drop_target(hwnd, tx); store drop_guard + rx on NaygoApp`. Add a `pump_dropped_files` drained in `logic` (like pump_devices): for each `DroppedFiles`, find the panel under (screen_x, screen_y) — map screen→which pane (or default to active pane), resolve dest = that pane's dir, and start a transfer (copy by default, or move if effect_copy false) via the same ops path as Task 2. VERIFY where to map screen coords to a pane; if mapping is hard, drop into the ACTIVE pane (simpler, acceptable) and report.

- [ ] **Step 3: Verify + commit**

Run: `cargo build --workspace`; `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all`.
(Visual: Nicolás drags a file from Explorer into Naygo.)
```
git add crates/platform/src/dnd.rs crates/platform/src/lib.rs crates/platform/Cargo.toml crates/ui/src/app.rs
git commit -m "feat(platform): recibir drag&drop del SO (IDropTarget, Explorer→Naygo)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
Report the COM signatures used, OLE init balance, how screen→pane mapping resolved.

---

## Task 4: A2-sacar — IDataObject + DoDragDrop (Naygo → Explorer)

**Files:**
- Modify: `crates/platform/src/dnd.rs`
- Modify: `crates/platform/src/clipboard.rs` (exponer build_hdrop_global)
- Modify: `crates/ui/src/panes/file_panel.rs` (disparar el drag OLE al sacar)

THE MOST UNCERTAIN TASK. `DoDragDrop` runs its own modal loop. Integrating it with egui/winit is delicate.

- [ ] **Step 1: Exponer build_hdrop_global reusable**

In `crates/platform/src/clipboard.rs`, factor `build_hdrop_global` so `platform::dnd` can reuse it (make it `pub(crate)` and move to a shared place, or expose a `pub(crate) fn hdrop_global(paths) -> Result<HGLOBAL,...>`). Don't duplicate the DROPFILES construction.

- [ ] **Step 2: IDataObject + IDropSource + DoDragDrop**

In `platform::dnd`, add:
```rust
/// Inicia un arrastre OLE de `paths` hacia el SO (Explorer, correo, etc.). Bloqueante
/// (DoDragDrop corre su propio bucle modal). Devuelve el efecto resultante (¿movió?).
#[cfg(windows)]
pub fn start_drag(paths: &[PathBuf]) -> Result<DragOutcome, DndError>;

pub enum DragOutcome { Copied, Moved, Cancelled }
```
Implement a COM `IDataObject` exposing CF_HDROP (built from `hdrop_global`) + a minimal `IDropSource` (QueryContinueDrag/GiveFeedback standard impls) and call `DoDragDrop(data_object, drop_source, DROPEFFECT_COPY|DROPEFFECT_MOVE, &mut effect)`. Map `effect` → DragOutcome. OLE init as in Task 3. Doc-comment richly. Stub non-Windows → Err(NotSupported).
(VERIFY windows 0.62: `#[implement(IDataObject)]` + `#[implement(IDropSource)]`, `DoDragDrop`, FORMATETC/STGMEDIUM for CF_HDROP, the many IDataObject methods — GetData/QueryGetData/EnumFormatEtc etc.; implementing IDataObject fully is verbose. There may be a shell helper `SHCreateDataObject` / `SHCreateStdEnumFmtEtc` that simplifies it — PREFER a shell helper if available in 0.62 to avoid hand-implementing every IDataObject method. Report what you used.)

- [ ] **Step 3: Disparar start_drag desde el panel**

In `file_panel.rs`: when a file drag from the name cell is detected to be leaving toward the OS (or — simpler, per the spec's fallback — when a drag from the name starts and the egui-internal drop target is NOT another Naygo pane), call `platform::dnd::start_drag(&source_paths)`. DECISION (resolve here, report): the cleanest integration is probably — the egui internal drag handles intra-Naygo; for OS-out, detect the pointer leaving the window during a name-drag and hand off to `start_drag`. BUT DoDragDrop is modal and will conflict with egui's drag state. ALTERNATIVE (likely more robust): on a name-cell drag START, immediately call `start_drag` (DoDragDrop handles BOTH dropping inside our window — via the IDropTarget from Task 3 — and outside). I.e. unify: dragging a file ALWAYS goes through OLE DoDragDrop; our own IDropTarget receives it if dropped on our window, Explorer receives it if dropped outside. This removes the egui-internal drag for files entirely and is how real shell apps work. EVALUATE this unified approach: if Task 3's IDropTarget can receive our own DoDragDrop source (it should — it's a normal OLE drop), then Task 2's egui-internal drag could be REPLACED by OLE for files. If that works, it's cleaner; if egui/winit's event loop fights DoDragDrop's modal loop, fall back to egui-internal for intra-app + DoDragDrop only when leaving. The implementer must TRY and report which works. Document the chosen integration thoroughly.
NOTE: this task may reveal that Task 2's internal drag and Task 4's OLE drag should be ONE mechanism (OLE for everything). If so, that's a better design — report it; we may simplify Task 2's egui-internal path to defer to OLE. Do NOT force a broken dual-path.

- [ ] **Step 4: Verify + commit**

Run: `cargo build --workspace`; `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all`.
(Visual: Nicolás drags a file from Naygo to Explorer/desktop/email.)
```
git add crates/platform/src/dnd.rs crates/platform/src/clipboard.rs crates/ui/src/panes/file_panel.rs
git commit -m "feat(platform): sacar drag&drop al SO (IDataObject + DoDragDrop, Naygo→Explorer)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
Report the integration approach chosen (unified OLE vs dual-path), whether a shell helper simplified IDataObject, COM signatures, any BLOCKED concern.

---

## Task 5: Pulido — persistencia del dock (from_dock_state)

**Files:**
- Modify: `crates/ui/src/dock_translate.rs`
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: from_dock_state + round-trip test**

In `dock_translate.rs`, add `pub fn from_dock_state(state: &DockState<PaneId>) -> SerializableDockLayout` (the inverse of `to_dock_state`). READ `to_dock_state` and `SerializableDockLayout`/`DockNode`/`SplitDir` to mirror the structure (walk the egui_dock Tree → build DockNode Split/Leaf). Add a round-trip test: `to_dock_state(layout)` → `from_dock_state(...)` → equivalent layout (same pane ids + structure). VERIFY the egui_dock 0.19 Tree/Node API for walking (`state.main_surface()`, iterate nodes, splits with ratio/direction).

- [ ] **Step 2: Persistir el DockState vivo**

In `app.rs`, where the workspace is saved (on close / on change), update `workspace.layout = from_dock_state(&self.dock_state)` before persisting, so ➕-added panes and drag-rearrangements survive restart. VERIFY where save_workspace is called; update the layout from the live dock_state there.

- [ ] **Step 3: Verify + commit**

Run: `cargo build --workspace`; `cargo test --workspace` (round-trip passes); `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all`.
(Visual: Nicolás adds a pane with ➕, rearranges, restarts → layout survives.)
```
git add crates/ui/src/dock_translate.rs crates/ui/src/app.rs
git commit -m "fix(ui): persistir el layout del dock (➕ y reacomodos sobreviven al reinicio)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
If `from_dock_state` is infeasible with egui_dock 0.19's API (no way to read splits), report it — fall back to at least persisting the set of open panes (so ➕ panes survive even if exact split ratios don't).

---

## Task 6: Pulido — M2 (clic-derecho reduce la selección)

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`

- [ ] **Step 1: Si la fila clickeada-con-derecho no está en selección, seleccionarla**

In the row context-menu handler: before showing the menu, if the right-clicked row `i` is NOT in `f.selected` (i.e. `!selected_set.contains(&i)`), reduce the selection to just that row (`select_single(i)` on the pane) — like Windows. If it IS in the selection, leave the multi-selection (operate on all). VERIFY the context_menu/secondary_clicked path: `context_focus = Some(i)` is set; add the select-single when the row isn't already selected. This must mutate the pane (`workspace.pane_mut(id)`) at the right point (when the menu opens / on secondary_clicked), consistent with how clicked is consumed.

- [ ] **Step 2: Verify + commit**

Run: `cargo build -p naygo-ui`; `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all`.
```
git add crates/ui/src/panes/file_panel.rs
git commit -m "fix(ui): clic-derecho sobre fila fuera de la selección la reduce a ese ítem (como Windows)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Pulido — M5 ("Espacio") + M3/M4 + fila ".."

**Files:**
- Modify: `crates/ui/src/input.rs`
- Modify: `crates/ui/src/panes/file_panel.rs` (M4 si trivial)

- [ ] **Step 1: M5 — "Espacio" neutro**

In `crates/ui/src/input.rs` `chord_text`, the `KeyCode::Space` arm returns "Espacio" (hardcoded Spanish). Change to a neutral symbol/label consistent with the others (e.g. `"Space"` is also English-only; the editor shows symbols like `↑`/`Alt+`). Use a neutral token like `"␣"` (space symbol) OR keep it but route through i18n if the editor supports it. SIMPLEST neutral: `"Espacio"` → `"Space"` is no better; use the unicode `␣` (U+2423 OPEN BOX) which is language-neutral, OR leave as-is and just note it. DECISION: use `"Espacio"` → a short neutral label; pick what reads OK in the shortcut editor and report. (This is cosmetic; minimal change.)

- [ ] **Step 2: M4 — limpiar el temp del rubber-band incondicionalmente**

In `file_panel.rs`, the rubber-band `drag_stopped` cleanup of the start-pos temp only runs when `interact_pointer_pos()` is Some. Move the `ui.memory_mut(|m| m.data.remove::<Pos2>(start_key))` to run unconditionally on `drag_stopped()` (so it never leaks if the pointer left the window on the stop frame). Small.

- [ ] **Step 3: M3 + fila ".." — verificar**

VERIFY (read only): (M3) does the rubber-band conflict with table scroll? If there's an obvious issue, note it; otherwise leave for Nicolás's visual check (documented). (Fila "..") confirm `DisplayRow::Parent` renders as a normal folder row (icon + alignment like others). If both are already fine, no code change — just report.

- [ ] **Step 4: Verify + commit**

Run: `cargo build -p naygo-ui`; `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all`.
```
git add crates/ui/src/input.rs crates/ui/src/panes/file_panel.rs
git commit -m "fix(ui): label neutro de Espacio + limpieza incondicional del rubber-band temp

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
Report M3 + fila ".." findings (changed or already fine).

---

## Task 8: Autoría en scripts

**Files:**
- Modify: `scripts/build-release.ps1`
- Modify: `crates/ui/src/bin/gen_icons.rs`

- [ ] **Step 1: Header de autoría explícito**

In `scripts/build-release.ps1` (top comment block) and `crates/ui/src/bin/gen_icons.rs` (header), ensure the author is explicit:
- build-release.ps1: the header has the project + MIT line; add/confirm a line `# Autor: Nicolás Groth <ngroth@gmail.com> — ISGroth.`
- gen_icons.rs: the 2-line header — add the author email if not present: keep `// Naygo — ... // Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.` and add a third line `// Autor: Nicolás Groth <ngroth@gmail.com>.` if the convention allows (or fold into the copyright line).
Scan for any OTHER scripts/bins in the repo (`scripts/`, `crates/*/src/bin/`, `*.ps1`) and apply the same. Keep it to comments only.

- [ ] **Step 2: Verify + commit**

Run: `cargo build --workspace` (gen_icons.rs still compiles); `cargo fmt --all`.
```
git add scripts/ crates/ui/src/bin/gen_icons.rs
git commit -m "docs: autoría explícita (Nicolás Groth / ISGroth) en los scripts

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Cierre — README + verificación final + push

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README estado**

Replace the status block:
```markdown
> **Estado:** Drag & drop (interno + con el SO), persistencia del dock y pulidos
> agregados. Diseño en
> [`docs/superpowers/specs/2026-06-09-naygo-dnd-pulidos-design.md`](docs/superpowers/specs/2026-06-09-naygo-dnd-pulidos-design.md).
> Pendiente: "Acerca de…" (Entrega 2) y bandeja del sistema + autostart (Entrega 3).
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace`; `cargo build --release -p naygo-ui`; `cargo test --workspace` (green); `cargo clippy --workspace --all-targets -- -D warnings` (clean); `cargo fmt --all -- --check` (clean). `git status` limpio.

- [ ] **Step 3: Commit + push**
```
git add README.md
git commit -m "docs: README — DnD + pulidos (Entrega 1)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/dnd-pulidos
```

---

## Self-review (cobertura del spec)

| Requisito | Tarea(s) |
|---|---|
| decide_drop_action + same_drive (puro) | 1 |
| DnD interno entre paneles + feedback B + transfer | 2 |
| Mover/copiar por tecla/disco | 1, 2 |
| Botón derecho = menú | 2 (o deferral reportado) |
| Multi-selección en el drag | 2 |
| Recibir Explorer→Naygo (IDropTarget) | 3 |
| Sacar Naygo→Explorer (IDataObject+DoDragDrop) | 4 |
| Reusar build_hdrop_global | 4 |
| Persistencia del dock (from_dock_state) | 5 |
| M2 clic-derecho reduce selección | 6 |
| M5 "Espacio" + M4 temp + M3/fila ".." | 7 |
| Autoría en scripts | 8 |
| README | 9 |
| i18n parity | 2 |

**Notas de riesgo:**
- **DoDragDrop modal vs egui** (Task 4): el punto más incierto. El implementer debe PROBAR el enfoque unificado (OLE para todo) vs dual-path y reportar; puede que Task 2 (egui-internal) se simplifique a deferir a OLE. NO forzar un dual-path roto. Si bloquea, reportar.
- **OLE init** (Tasks 3,4): `OleInitialize` (no solo CoInitialize) para RegisterDragDrop/DoDragDrop; balance cuidado (eframe/winit ya inicializó algo) — patrón needs_uninit de trash.rs adaptado.
- **IDataObject verboso** (Task 4): preferir un shell helper (`SHCreateDataObject`?) si existe en windows 0.62 antes de implementar todos los métodos a mano. Reportar.
- **name-cell drag vs click-selección** (Task 2): verificar que el drag source no rompe el clic de selección de la celda nombre.
- **IDropTarget lifetime + refcount** (Task 3): guard con RevokeDragDrop al Drop; Sender dentro del COM object (patrón device_watch). El holistic final revisa el refcount.
- **from_dock_state factible** (Task 5): si egui_dock 0.19 no deja leer splits, degradar a persistir el set de paneles abiertos + reportar.
- **build_hdrop_global** (Task 4): reusar, NO duplicar el DROPFILES.
- **Cargo features OLE** (Tasks 3,4): añadir sub-features si el build las pide.
- **Documentación**: doc-comments ricos del flujo COM/OLE en platform::dnd (lo más nuevo/delicado).
```
