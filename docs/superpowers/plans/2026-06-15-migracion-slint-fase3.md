# Fase 3 (Slint): operaciones + diálogos + progreso — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Llevar las operaciones de archivo (copiar/cortar/pegar, eliminar, nuevo, renombrar)
con sus diálogos, progreso, deshacer, journal, clipboard del SO con corte visual y menú
contextual híbrido, de la capa egui a la capa Slint (`crates/ui-slint`).

**Architecture:** Todo el motor de ops vive en `naygo-core`/`naygo-platform` (reusable). Se
escribe la orquestación en un módulo nuevo `ops_ctrl.rs`. El único cambio a core es ops-B
(conflicto interactivo por-ítem en `engine.rs`). Los modales y el panel de progreso son
componentes Slint nuevos con modelos estables; el motor corre en su hilo y un `slint::Timer`
drena el progreso.

**Tech Stack:** Rust, Slint 1.16 (backend winit software), `naygo-core::ops`,
`naygo-platform::{trash,clipboard,open,context_menu}`, raw-window-handle.

**Convenciones del proyecto (OBLIGATORIAS):**
- Gates antes de CADA commit: `cargo test --workspace` + `cargo clippy --workspace
  --all-targets -- -D warnings` + `cargo fmt --all -- --check`.
- Commits en español (heredoc `git commit -F - <<'EOF' … EOF`), terminando con
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Stagear rutas EXPLÍCITAS (NUNCA `git add -A`): hay cambios pendientes no relacionados en
  `CLAUDE.md` y el directorio `graphify-out/` que NO deben commitearse.
- `graphify update .` tras cambios de código.
- Header en archivos nuevos:
  `// Naygo — <descripción>.` / `// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.`
- Probar el binario en ESTA máquina con Win32 (PostMessage + PrintWindow), NO con los clics de
  computer-use. Nicolás solo mide rendimiento en la VM.
- NO mergear a main hasta el visto bueno de Nicolás (el trabajo va directo a `main` en commits;
  ver nota: trabajamos en `main` en esta serie, sin rama aparte).

---

## Fase A — Core: mover ops_actions + conflicto por-ítem (ops-B)

### Task 1: Mover `ops_actions` a `core::ops::actions`

`ops_actions.rs` vive en `crates/ui/` (egui). Para reusarlo desde ui-slint sin depender de
egui, se mueve a `core::ops::actions` (es puro: arma `OpRequest`s).

**Files:**
- Create: `crates/core/src/ops/actions.rs`
- Modify: `crates/core/src/ops/mod.rs` (agregar `pub mod actions;` + re-export)
- Modify: `crates/ui/src/ops_actions.rs` (reemplazar por re-export desde core) o eliminar y
  ajustar imports en `crates/ui/src/app.rs`.

- [ ] **Step 1: Orientar y leer el actual `ops_actions.rs`**

Run: `graphify query "ops_actions transfer delete rename create OpRequest"` luego leer
`crates/ui/src/ops_actions.rs` completo.

- [ ] **Step 2: Crear `crates/core/src/ops/actions.rs` con las 4 funciones puras**

Copiar las funciones (las firmas exactas son `transfer(kind_move: bool, sources:
Vec<PathBuf>, dest_dir: PathBuf) -> OpRequest`, `delete(sources: Vec<PathBuf>, to_trash: bool)
-> OpRequest`, `rename(source: PathBuf, new_name: String) -> OpRequest`, `create(dir: PathBuf,
name: String, is_dir: bool) -> OpRequest`). Usar los tipos de `super::{OpKind, OpRequest,
ConflictPolicy}`. Agregar header de copyright. Incluir los tests existentes (si los hay) o
añadir uno por función verificando el `OpKind`/`conflict` resultante.

- [ ] **Step 3: Exportar desde `mod.rs`**

En `crates/core/src/ops/mod.rs` agregar `pub mod actions;` y
`pub use actions::{transfer, delete, rename, create};`.

- [ ] **Step 4: Ajustar la capa egui para importar de core**

En `crates/ui/src/app.rs` reemplazar `use crate::ops_actions::...` por
`use naygo_core::ops::{transfer, delete, rename, create};`. Eliminar
`crates/ui/src/ops_actions.rs` y su `mod ops_actions;` en `lib.rs`/`main.rs`.

- [ ] **Step 5: Gate + commit**

Run: `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings` y
`cargo fmt --all -- --check`. Esperado: todo verde (egui sigue compilando).

```bash
git add crates/core/src/ops/actions.rs crates/core/src/ops/mod.rs crates/ui/src/app.rs crates/ui/src/lib.rs
git rm crates/ui/src/ops_actions.rs
git commit -F - <<'EOF'
refactor(core): mover ops_actions a core::ops::actions (reuso desde Slint)

Las funciones que arman OpRequest (transfer/delete/rename/create) eran puras pero vivían en
la capa egui. Se mueven a core::ops::actions para reusarlas desde la capa Slint sin depender
de egui. La capa egui pasa a importarlas de core.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Task 2: Core ops-B — conflicto interactivo por-ítem en `engine.rs`

Hoy `exec_copy_step` trata `Ask` como `Overwrite`. Para ops-B: cuando la política es `Ask` y
el destino existe, emitir `OpMsg::Conflict` y BLOQUEAR en `conflict_rx` hasta recibir una
`ConflictDecision`. Con `apply_all`, memorizar la acción y no volver a preguntar.

**Files:**
- Modify: `crates/core/src/ops/engine.rs` (firmar `exec_step`/`exec_copy_step` con `tx` y
  `conflict_rx`; lógica de Ask interactivo; estado `applied_all` en `run_plan`).
- Test: `crates/core/src/ops/engine.rs` (`#[cfg(test)]`).

- [ ] **Step 1: Escribir el test del conflicto interactivo (Overwrite)**

En el módulo `tests` de `engine.rs`, agregar (usa `tempfile`):

```rust
#[test]
fn ask_emite_conflict_y_aplica_overwrite() {
    use crate::ops::{ConflictAction, ConflictDecision, ConflictPolicy, OpKind, OpMsg};
    use crate::cancel::CancellationToken;
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("a.txt");
    let dstdir = tmp.path().join("dst");
    std::fs::create_dir(&dstdir).unwrap();
    std::fs::write(&src, b"NUEVO").unwrap();
    std::fs::write(dstdir.join("a.txt"), b"VIEJO").unwrap(); // ya existe → choque
    let req = crate::ops::transfer(false, vec![src.clone()], dstdir.clone());
    let plan = crate::ops::plan(&req).unwrap();
    let (ctx, crx) = std::sync::mpsc::channel::<ConflictDecision>();
    let token = CancellationToken::new();
    let (tx, rx) = std::sync::mpsc::channel::<OpMsg>();
    // Responder Overwrite cuando llegue el Conflict (en otro hilo).
    let resp = std::thread::spawn(move || {
        loop {
            if let Ok(OpMsg::Conflict(_)) = rx.recv() {
                ctx.send(ConflictDecision { action: ConflictAction::Overwrite, apply_all: false }).unwrap();
            } else { break; }
        }
    });
    let summary = crate::ops::engine::run_plan(&plan, &OpKind::Copy, ConflictPolicy::Ask, &token, &tx, &crx, None);
    drop(tx); // cierra el canal para que el hilo `resp` termine
    let _ = resp.join();
    assert_eq!(summary.count_done(), 1);
    assert_eq!(std::fs::read_to_string(dstdir.join("a.txt")).unwrap(), "NUEVO");
}
```

- [ ] **Step 2: Correr el test → debe fallar**

Run: `cargo test -p naygo-core ask_emite_conflict_y_aplica_overwrite -- --nocapture`
Esperado: FALLA — hoy `Ask` sobrescribe sin emitir `Conflict`, así que el archivo queda
"NUEVO" pero NUNCA se emite `OpMsg::Conflict` (el hilo `resp` no responde) — el test puede
colgar o pasar por la razón equivocada. Para que falle de verdad, el assert clave es que
DEBE emitirse un Conflict: cambiar el hilo `resp` para `panic!("no llegó Conflict")` si recibe
`Done` antes que `Conflict`. Ajustar el test para que falle si no hay `Conflict`.

- [ ] **Step 3: Threading de `tx` y `conflict_rx` a `exec_copy_step`**

Modificar las firmas:
`fn exec_step(step, kind, conflict, token, tx: &Sender<OpMsg>, conflict_rx: &Receiver<ConflictDecision>, applied_all: &mut Option<ConflictAction>) -> (...)`
y `exec_copy_step(step, conflict, token, is_move, tx, conflict_rx, applied_all) -> (...)`.
En `run_plan`, declarar `let mut applied_all: Option<ConflictAction> = None;` y pasarlo a
`exec_step`. Pasar `tx` y `conflict_rx` (ya disponibles en `run_plan`).

- [ ] **Step 4: Lógica de Ask interactivo en `exec_copy_step`**

Reemplazar el branch de conflicto:

```rust
let to = if step.to.exists() {
    // Acción efectiva: si ya se eligió "aplicar a todos", usarla sin preguntar.
    let effective: ConflictAction = match conflict {
        ConflictPolicy::Skip => ConflictAction::Skip,
        ConflictPolicy::Overwrite => ConflictAction::Overwrite,
        ConflictPolicy::Rename => ConflictAction::Rename,
        ConflictPolicy::Ask => {
            if let Some(prev) = *applied_all {
                prev
            } else {
                // Preguntar a la UI: emitir Conflict y BLOQUEAR esperando la decisión.
                let _ = tx.send(OpMsg::Conflict(ConflictPrompt {
                    existing: step.to.clone(),
                    incoming: from.clone(),
                }));
                // Esperar la decisión sin colgar si se cancela: poll con timeout.
                let decision = loop {
                    if token.is_cancelled() {
                        return (step.to.clone(), OpOutcome::Skipped, 0, true);
                    }
                    match conflict_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                        Ok(d) => break d,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            return (step.to.clone(), OpOutcome::Skipped, 0, true);
                        }
                    }
                };
                if decision.apply_all {
                    *applied_all = Some(decision.action);
                }
                decision.action
            }
        }
    };
    match effective {
        ConflictAction::Skip => return (step.to.clone(), OpOutcome::Skipped, 0, true),
        ConflictAction::Rename => super::dedup_name(&step.to, &|p: &Path| p.exists()),
        ConflictAction::Overwrite => step.to.clone(),
    }
} else {
    step.to.clone()
};
```

Agregar imports: `use super::{ConflictAction, ConflictPrompt};` en la cabecera.

- [ ] **Step 5: Correr el test → debe pasar**

Run: `cargo test -p naygo-core ask_emite_conflict_y_aplica_overwrite`
Esperado: PASS.

- [ ] **Step 6: Tests de Skip, Rename, apply_all y cancelación**

Agregar 3 tests más, análogos:
- `ask_skip_deja_el_existente`: responder `Skip` → el archivo destino conserva "VIEJO",
  `count_skipped()==1`.
- `ask_rename_crea_copia`: responder `Rename` → existe `a (2).txt` con "NUEVO", el original
  "VIEJO" intacto.
- `apply_all_no_vuelve_a_preguntar`: dos fuentes que chocan; responder Overwrite con
  `apply_all:true` en el PRIMER Conflict; verificar que solo se emitió UN `OpMsg::Conflict`
  (contar) y ambos se sobrescribieron.
- `cancelar_durante_espera_aborta`: emitir Conflict, no responder, cancelar el token →
  `run_plan` retorna y el item queda `Skipped`.

- [ ] **Step 7: Verificar que egui sigue OK**

La capa egui pre-resuelve el conflicto antes de spawn (pasa `Overwrite`/`Skip`/`Rename`, nunca
`Ask` interactivo), así que su comportamiento no cambia. Run: `cargo test -p naygo-ui`.
Esperado: PASS.

- [ ] **Step 8: Gate + commit**

```bash
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/core/src/ops/engine.rs
git commit -F - <<'EOF'
feat(core): conflicto interactivo por-ítem en el motor de ops (ops-B)

Cuando la política es Ask y el destino existe, el motor emite OpMsg::Conflict y bloquea
esperando una ConflictDecision por conflict_rx (Overwrite/Skip/Rename + apply_all). Con
apply_all memoriza la acción y no vuelve a preguntar. Cancelable durante la espera. La capa
egui no usa Ask interactivo (pre-resuelve antes de spawn), así que su flujo no cambia.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Fase B — ui-slint: estado y motor de ops (`ops_ctrl.rs`)

### Task 3: Esqueleto de `OpsCtrl` + clipboard interno (cut_set)

**Files:**
- Create: `crates/ui-slint/src/ops_ctrl.rs`
- Modify: `crates/ui-slint/src/main.rs` (agregar `mod ops_ctrl;`)

- [ ] **Step 1: Crear el módulo con la struct y el estado**

`OpsCtrl` con los campos del spec §1 (`active_ops`, `pending_dialog`, `undo_history`,
`next_undo_id`, `cut_set: HashSet<PathBuf>`, `pending_resume`, `config_dir`). Definir
`ActiveOp`, `OpDialog` (enum), `NamePurpose` (enum: NewFile/NewDir/Rename). `OpsCtrl::new(config_dir: PathBuf)`.

- [ ] **Step 2: Métodos de clipboard interno (cut_set) con test**

Agregar `set_cut(&mut self, paths: Vec<PathBuf>)` (llena cut_set + escribe al SO con
`platform::clipboard::write_files(&paths, true)`), `set_copy(&mut self, paths)` (limpia
cut_set + `write_files(.., false)`), `clear_cut(&mut self)`, `is_cut(&self, path: &Path) ->
bool`. Test puro (sin tocar el portapapeles real: extraer la lógica de cut_set a una función
testeable, o testear solo `is_cut`/`clear_cut` tras setear cut_set directamente).

```rust
#[test]
fn cut_set_marca_y_limpia() {
    let mut c = OpsCtrl::new(std::env::temp_dir());
    c.cut_set.insert(std::path::PathBuf::from("C:/x/a.txt"));
    assert!(c.is_cut(std::path::Path::new("C:/x/a.txt")));
    c.clear_cut();
    assert!(!c.is_cut(std::path::Path::new("C:/x/a.txt")));
}
```

- [ ] **Step 3: Registrar el módulo y compilar**

En `main.rs` agregar `mod ops_ctrl;`. Run: `cargo build -p naygo-ui-slint`. Esperado: compila
(con warnings de dead_code aceptables por ahora).

- [ ] **Step 4: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/src/ops_ctrl.rs crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(slint): esqueleto de OpsCtrl + clipboard interno con corte visual (Fase 3)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Task 4: Lanzar y drenar ops (start_op + pump_ops)

**Files:**
- Modify: `crates/ui-slint/src/ops_ctrl.rs`

- [ ] **Step 1: Test de `start_op` para una copia sin conflicto**

```rust
#[test]
fn start_op_copia_y_pump_la_completa() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("a.txt"); std::fs::write(&src, b"hola").unwrap();
    let dst = tmp.path().join("dst"); std::fs::create_dir(&dst).unwrap();
    let mut c = OpsCtrl::new(tmp.path().to_path_buf());
    let req = naygo_core::ops::transfer(false, vec![src.clone()], dst.clone());
    c.start_op(req, "Copiar".to_string(), true);
    // Drenar hasta terminar (simula el Timer).
    for _ in 0..2000 {
        c.pump_ops();
        if c.active_ops.iter().all(|o| o.summary.is_some() || o.rx.is_none()) && !c.active_ops.is_empty() { break; }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(dst.join("a.txt").exists());
}
```

- [ ] **Step 2: Implementar `start_op` (sin journal/cola por ahora)**

`pub fn start_op(&mut self, req: OpRequest, label: String, record_undo: bool)`:
- Si `Delete{to_trash:true}` → `platform::trash::move_to_trash(&req.sources)`, return (sin op
  en active_ops; el llamador refresca).
- `let plan = match naygo_core::ops::plan(&req) { Ok(p) => p, Err(_) => return; };`
- Pre-check de conflicto: `let conflict = if self.first_collision(&req) { Ask } else { Overwrite };`
- `let token = CancellationToken::new();`
- `let (conflict_tx, conflict_rx) = mpsc::channel::<ConflictDecision>();`
- `let (rx, _h) = naygo_core::ops::engine::spawn(plan, req.kind.clone(), conflict, token.clone(), conflict_rx, None);`
- push `ActiveOp { rx: Some(rx), conflict_tx, token, label, progress: None, summary: None, started: true, pending: None, journal_id: None, request: record_undo.then_some(req), awaiting_conflict: None }`.

Implementar `first_collision(&self, req: &OpRequest) -> bool` (replica de egui: para
Copy/Move, ¿`dest_dir/source.file_name()` ya existe para algún source?).

- [ ] **Step 3: Implementar `pump_ops`**

`pub fn pump_ops(&mut self) -> bool` (devuelve true si todo está en reposo): por cada op con
`rx`, `try_recv` en bucle:
- `Progress(p)` → `op.progress = Some(p)`.
- `Conflict(prompt)` → `op.awaiting_conflict = Some(prompt)` (el modal se abre en otro método;
  por ahora guardar).
- `Done(s)|Cancelled(s)` → tomar `op.request`, si `Some` y `build_undo` da `Some`, push
  `UndoEntry`; `op.summary = Some(s)`; `op.rx = None`.
- `Failed(e)` → `op.summary = Some(OpSummary::default())` con marca de error (o un campo).
Devuelve `self.active_ops.iter().all(|o| o.rx.is_none())`.

- [ ] **Step 4: Correr el test → pasa**

Run: `cargo test -p naygo-ui-slint start_op_copia_y_pump_la_completa`. Esperado: PASS.

- [ ] **Step 5: Test de registro de undo al terminar**

```rust
#[test]
fn copia_registra_undo() {
    // ... como arriba, pero tras drenar:
    assert_eq!(c.undo_history.len(), 1);
    assert!(c.undo_history[0].label.len() > 0);
}
```
Implementar el `next_undo_id` y la construcción de `UndoEntry { id, label, when_epoch_secs,
actions, undone:false }` en `pump_ops` (usar `build_undo`). Correr → PASS.

- [ ] **Step 6: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/src/ops_ctrl.rs
git commit -F - <<'EOF'
feat(slint): lanzar y drenar operaciones (start_op + pump_ops) con registro de undo

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Task 5: Resolver conflicto + cancelar + cola

**Files:**
- Modify: `crates/ui-slint/src/ops_ctrl.rs`

- [ ] **Step 1: Test de resolución de conflicto por-ítem**

```rust
#[test]
fn conflicto_se_resuelve_con_overwrite() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("a.txt"); std::fs::write(&src, b"NUEVO").unwrap();
    let dst = tmp.path().join("dst"); std::fs::create_dir(&dst).unwrap();
    std::fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
    let mut c = OpsCtrl::new(tmp.path().to_path_buf());
    c.start_op(naygo_core::ops::transfer(false, vec![src], dst.clone()), "Copiar".into(), true);
    // Drenar hasta que aparezca un conflicto pendiente, resolverlo, seguir drenando.
    let mut resolved = false;
    for _ in 0..3000 {
        c.pump_ops();
        if !resolved {
            if let Some(idx) = c.active_ops.iter().position(|o| o.awaiting_conflict.is_some()) {
                c.resolve_conflict(idx, ConflictAction::Overwrite, false);
                resolved = true;
            }
        }
        if c.active_ops.iter().all(|o| o.summary.is_some()) { break; }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "NUEVO");
}
```

- [ ] **Step 2: Implementar `resolve_conflict` y `cancel_op`**

`pub fn resolve_conflict(&mut self, op_index: usize, action: ConflictAction, apply_all: bool)`:
envía `ConflictDecision { action, apply_all }` por `op.conflict_tx`; limpia
`op.awaiting_conflict`. `pub fn cancel_op(&mut self, op_index: usize)`:
`op.token.cancel()`.

- [ ] **Step 3: Correr el test → pasa**

Run: `cargo test -p naygo-ui-slint conflicto_se_resuelve_con_overwrite`. Esperado: PASS.

- [ ] **Step 4: Cola de operaciones (modo Queue)**

Agregar `ops_mode: OpsMode` (enum Parallel/Queue, default Parallel por ahora) y, en `start_op`,
si Queue y hay otra `started && rx.is_some()`, push con `started:false, pending:Some((plan,
kind, conflict))` sin spawnear. En `pump_ops`, tras drenar, si nada corre y hay `pending`,
spawnearla. Test: dos ops en Queue → la 2ª arranca al terminar la 1ª.

- [ ] **Step 5: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/src/ops_ctrl.rs
git commit -F - <<'EOF'
feat(slint): resolver conflicto por-ítem, cancelar y cola de operaciones

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Fase C — ui-slint: gestos, diálogos y progreso (UI Slint)

### Task 6: Gestos de ops en el controlador + atajos

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (poseer `OpsCtrl`, enrutar Actions de ops)
- Modify: `crates/ui-slint/src/ops_ctrl.rs` (métodos de gesto: `copy/cut/paste/delete/new/rename`)

- [ ] **Step 1: WorkspaceCtrl posee OpsCtrl**

Agregar `pub ops: OpsCtrl` a `WorkspaceCtrl`, inicializado en `new` con un `config_dir`
(usar `naygo_core::config::config_dir()` si existe; si no, `USERPROFILE/.naygo`). Mover
`undo_history` de WorkspaceCtrl a OpsCtrl (los bridges del panel Historial leen de `ops`).

- [ ] **Step 2: Métodos de gesto en OpsCtrl**

`copy(&mut self, paths)`, `cut(&mut self, paths)`, `paste(&mut self, dest_dir)` (lee
clipboard, decide_paste, start_op o abre PastePreview), `delete(&mut self, paths, permanent)`
(abre `ConfirmDelete`), `new_file/new_dir(&mut self, dir)` (abre `NameInput`), `rename(&mut
self, source, new_name)` (start_op directo), `undo_last(&mut self)` (deshace la última
`UndoEntry` deshacible).

- [ ] **Step 3: Enrutar los Action de ops en `on_key`**

En `WorkspaceCtrl::on_key`, agregar brazos para `Action::{Copy, Cut, Paste, Delete,
DeletePermanent, NewFile, NewDir, Undo, CopyToOther, MoveToOther}` que llaman a `self.ops.*`
con `self.selected_paths()` y `self.active_dir()`. Para CopyToOther/MoveToOther, reusar el
selector 1..9 (PaneAction nuevo o resolver destino con `resolve_target`).

- [ ] **Step 4: Test de que el gesto arma el request correcto**

Test: `delete(paths, false)` deja `pending_dialog = Some(ConfirmDelete{permanent:false,..})`;
`new_dir(dir)` deja `NameInput{purpose:NewDir,..}`. (Sin tocar disco.)

- [ ] **Step 5: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/src/ops_ctrl.rs crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(slint): gestos de operaciones + atajos (copiar/cortar/pegar/eliminar/nuevo/deshacer)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Task 7: Corte visual (cut atenuado en las filas)

**Files:**
- Modify: `crates/ui-slint/src/bridge.rs` (campo `cut` en `PlainRow`)
- Modify: `crates/ui-slint/ui/types.slint` (campo `cut` en `RowData`)
- Modify: `crates/ui-slint/ui/file-panel.slint` (atenuar filas con `cut`)
- Modify: `crates/ui-slint/src/main.rs` (pasar `cut`), `workspace_ctrl.rs` (consultar cut_set)

- [ ] **Step 1: Agregar `cut: bool` a PlainRow y al bridge**

`rows_from_view` recibe un `&dyn Fn(&Path) -> bool` (o un `&HashSet<PathBuf>`) para marcar
`cut`. Test: una ruta en cut_set → su `PlainRow.cut == true`.

- [ ] **Step 2: `cut` en RowData (types.slint) + render atenuado**

En `file-panel.slint`, el `color` de los Text de la fila usa opacidad menor si `row.cut`
(ej. `opacity: row.cut ? 0.45 : 1.0` en el HorizontalLayout de la fila).

- [ ] **Step 3: Cablear en main.rs/workspace_ctrl**

`rows_of` consulta `self.ops.is_cut(path)` al construir cada fila. `to_row_data` copia `cut`.

- [ ] **Step 4: Build + verificación visual (Win32 PrintWindow)**

Run: `cargo build --release -p naygo-ui-slint`, lanzar, cortar una fila (Ctrl+X vía
PostMessage o tras seleccionar), capturar con PrintWindow, confirmar atenuación.

- [ ] **Step 5: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/src/bridge.rs crates/ui-slint/ui/types.slint crates/ui-slint/ui/file-panel.slint crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(slint): corte visual — filas cortadas se ven atenuadas hasta pegar

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Task 8: Diálogos modales (Slint)

**Files:**
- Create: `crates/ui-slint/ui/op-dialogs.slint` (los 5 modales como componentes)
- Modify: `crates/ui-slint/ui/types.slint` (VMs: `ConfirmDeleteVm`, `ConflictVm`, `NameVm`,
  `PastePreviewVm`, `ResumeVm`)
- Modify: `crates/ui-slint/ui/app-window.slint` (overlay del modal activo + callbacks)
- Modify: `crates/ui-slint/src/main.rs` (poblar el VM del modal activo + wirear decisiones)
- Modify: `crates/ui-slint/src/ops_ctrl.rs` (un getter del modal activo → VM; aplicar decisión)

- [ ] **Step 1: VMs en types.slint**

`ConfirmDeleteVm { active:bool, count:int, permanent:bool }`,
`ConflictVm { active:bool, name:string }` (decisión incluye apply_all),
`NameVm { active:bool, title:string, value:string, valid:bool }`,
`PastePreviewVm { active:bool, name:string, is_image:bool, ext:string }`,
`ResumeVm { active:bool, rows:[ResumeRowVm] }` con `ResumeRowVm { id:string, label:string }`.

- [ ] **Step 2: Componentes en op-dialogs.slint**

Cada uno: overlay con velo (TouchArea de fondo que cancela) + tarjeta centrada. ConfirmDelete:
texto + botones Eliminar/Cancelar. Conflict: nombre + 3 botones (Sobrescribir/Saltar/
Renombrar) + checkbox "Aplicar a todos". Name: TextInput + OK (deshabilitado si !valid) +
Cancelar; Enter confirma. PastePreview: nombre + Crear/Cancelar. Resume: lista + Retomar/
Descartar por fila + Todos.

- [ ] **Step 3: Integrar en app-window.slint**

`in property` por cada VM + `if vm.active: Dialog{...}`. Callbacks:
`confirm-delete(bool)`, `conflict-decide(int /*0 ow,1 skip,2 rename*/, bool /*all*/)`,
`name-confirm(string)`, `name-cancel()`, `paste-confirm(string)`, `resume-decide(string,int)`.

- [ ] **Step 4: Cablear en main.rs**

En `sync_*`, poblar el VM del modal activo desde `ops.pending_dialog`. Wirear cada callback a
`ops.*` (confirmar borrado → start_op delete; conflicto → resolve_conflict; name → start_op
create/rename; etc.). Tras decidir, `pending_dialog = None`.

- [ ] **Step 5: Build + verificación visual (Win32)**

Lanzar, disparar Eliminar (Del) → confirmar que aparece el modal; capturar con PrintWindow.
Conflicto: copiar sobre un existente → modal de conflicto. Nuevo → modal de nombre.

- [ ] **Step 6: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/ui/op-dialogs.slint crates/ui-slint/ui/types.slint crates/ui-slint/ui/app-window.slint crates/ui-slint/src/main.rs crates/ui-slint/src/ops_ctrl.rs
git commit -F - <<'EOF'
feat(slint): diálogos modales de operaciones (borrado, conflicto, nombre, pegado, retomar)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Task 9: Panel de progreso

**Files:**
- Create: `crates/ui-slint/ui/ops-panel.slint`
- Modify: `crates/ui-slint/ui/types.slint` (`OpRowVm { label, percent, status, index }`)
- Modify: `crates/ui-slint/ui/app-window.slint` (overlay inferior sobre la barra de estado)
- Modify: `crates/ui-slint/src/main.rs` (modelo estable de filas de ops + callback cancelar)

- [ ] **Step 1: `OpRowVm` + modelo estable**

En main.rs, un `VecModel<OpRowVm>` estable (como los de 2b). `sync` lo actualiza desde
`ops.active_ops` (label, percent = bytes_done/bytes_total*100, status string).

- [ ] **Step 2: Componente ops-panel.slint**

Lista de filas: etiqueta + barra de progreso (Rectangle con width proporcional) + estado +
botón ✕. Visible solo si hay filas. `cancel(int)` callback.

- [ ] **Step 3: Integrar + cablear cancelar**

En app-window, overlay inferior. `on_op_cancel(idx) => ops.cancel_op(idx)`. El Timer drena
`pump_ops` y refresca este modelo.

- [ ] **Step 4: Build + verificación visual**

Copiar una carpeta grande, capturar con PrintWindow mostrando la barra avanzando; cancelar.

- [ ] **Step 5: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/ui/ops-panel.slint crates/ui-slint/ui/types.slint crates/ui-slint/ui/app-window.slint crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(slint): panel de progreso de operaciones (barra %, estado, cancelar)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Fase D — Integraciones: journal, menú contextual híbrido

### Task 10: Journal de retomar-tras-crash

**Files:**
- Modify: `crates/ui-slint/src/ops_ctrl.rs` (crear JournalWriter en start_op; scan al arrancar;
  resume)
- Modify: `crates/ui-slint/src/main.rs` (al arrancar, `ops.scan_resume()` → abrir Resume si hay)

- [ ] **Step 1: JournalWriter en start_op para Copy/Move/Delete-permanente**

En `start_op`, antes de spawn, `let journal = make_journal(&req.kind, conflict, &plan)` que
devuelve `Some(JournalWriter::new(&config_dir, OpJournal::new(id, kind, conflict, plan)))` solo
para esos kinds. Pasar a `engine::spawn`. Guardar `journal_id` en la ActiveOp. Al terminar
(`pump_ops`), `journal::remove(&config_dir, &id)`.

- [ ] **Step 2: scan al arrancar + dialog Resume**

`pub fn scan_resume(&mut self)`: `let pend = journal::scan(&config_dir); if !pend.is_empty()
{ self.pending_dialog = Some(OpDialog::Resume{ items: pend }); }`. `resume(&mut self, id)`:
`resume_plan` → spawnear con writer nuevo reusando el id. `discard(&mut self, id)`:
`journal::remove`.

- [ ] **Step 3: Test del ciclo journal (crear → scan → resume)**

Test: crear un journal manualmente en un config_dir temporal, `scan_resume` lo detecta, abre
Resume. (No requiere crash real.)

- [ ] **Step 4: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/src/ops_ctrl.rs crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(slint): journal de operaciones — retomar copia/movida tras un cierre inesperado

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Task 11: Menú contextual híbrido (propio + nativo de Windows)

**Files:**
- Create: `crates/ui-slint/ui/context-menu.slint`
- Modify: `crates/ui-slint/ui/types.slint` (`ContextMenuVm { active, x, y, has_native }`)
- Modify: `crates/ui-slint/ui/app-window.slint` (right-click en filas → abrir; render del menú)
- Modify: `crates/ui-slint/src/main.rs` (callbacks de cada ítem + helper HWND + invocar nativo)
- Modify: `crates/ui-slint/Cargo.toml` (feature `raw-window-handle` de slint si hace falta)

- [ ] **Step 1: Right-click en una fila abre el menú propio**

En `file-panel.slint`, el `pointer-event` de la fila detecta botón derecho (`ev.button ==
PointerEventButton.right && ev.kind == down`) → callback `row-context(pane-id, pos, x, y)`.
En main.rs, abre `ContextMenuVm{ active:true, x, y, has_native: true }` y selecciona la fila.

- [ ] **Step 2: Menú propio con las acciones de Naygo**

`context-menu.slint`: overlay posicionado en (x,y) con ítems: Abrir, Abrir con…, Copiar,
Cortar, Pegar, Renombrar, Eliminar, Copiar ruta, ——, "Más opciones de Windows…". Velo que
cierra al clic fuera. Cada ítem un callback.

- [ ] **Step 3: Cablear acciones propias**

Cada ítem → `ops.*` o `platform::open::*`. "Abrir con…" → `platform::open::open_with_dialog`.

- [ ] **Step 4: Helper de HWND + invocar el menú nativo**

`fn naygo_hwnd(window: &AppWindow) -> Option<isize>`: obtener el raw window handle del backend
winit de Slint (`window.window().window_handle()` + raw-window-handle → `Win32WindowHandle.hwnd`).
"Más opciones de Windows…" → `platform::context_menu::show_native_context_menu(hwnd, &paths,
x_pantalla, y_pantalla)`; tras volver, refrescar. Si `naygo_hwnd` da None, ocultar ese ítem.

- [ ] **Step 5: Build + verificación visual (Win32)**

Clic derecho (PostMessage WM_RBUTTONDOWN/UP) en una fila → capturar el menú propio; click en
"Más opciones de Windows…" → aparece el menú del Shell.

- [ ] **Step 6: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/ui/context-menu.slint crates/ui-slint/ui/types.slint crates/ui-slint/ui/app-window.slint crates/ui-slint/src/main.rs crates/ui-slint/Cargo.toml
git commit -F - <<'EOF'
feat(slint): menú contextual híbrido — acciones propias + "Más opciones de Windows…" nativo

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Fase E — Verificación integral y cierre

### Task 12: Verificación viva + release + push

- [ ] **Step 1: Gate completo**

Run: `cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` +
`cargo fmt --all -- --check`. Todo verde.

- [ ] **Step 2: Build release + verificación con Win32**

`cargo build --release -p naygo-ui-slint`. Lanzar con `SLINT_BACKEND=winit-software` vía
`Start-Process`, obtener el HWND de la "Window Class", y con PostMessage + PrintWindow
verificar (capturando cada paso): copiar/mover entre dos paneles, eliminar a papelera y
permanente (con su modal), conflicto por-ítem con "aplicar a todos", nuevo archivo/carpeta,
renombrar inline, progreso + cancelar, deshacer desde el panel Historial, corte atenuado,
pegar, menú derecho propio + "Más opciones de Windows…".

- [ ] **Step 3: Copiar a dist + memoria + graphify**

`cp target/release/naygo-slint.exe dist/slint-fase1/naygo-slint.exe`. Actualizar
`memory/project-migracion-slint.md` con el estado de F3. `graphify update .`.

- [ ] **Step 4: Push a main (autorizado por Nicolás para esta serie)**

Stagear rutas explícitas, asegurar que `CLAUDE.md` y `graphify-out/` quedan fuera. `git push
origin main`.

- [ ] **Step 5: Pedir a Nicolás el rendimiento en la VM**

Avisar que F3 está funcional+verificada en esta máquina; pedir la medición de CPU en la VM al
operar (copiar/mover/borrar) y en reposo.

---

## Self-review (cobertura del spec)

- §1 Arquitectura ops_ctrl → Tasks 3-5. ✓
- §2 ops-B core → Task 2. ✓
- §3 Flujo de op → Tasks 4-5 (start_op/pump_ops/resolve/cola). ✓
- §4 Diálogos → Task 8. ✓
- §5 Panel de progreso → Task 9. ✓
- §6 Clipboard + corte visual → Tasks 3 (cut_set) + 7 (atenuado). ✓
- §7 Menú híbrido → Task 11. ✓
- §8 Atajos → Task 6. ✓
- §9 Integración Historial → Task 6 (mover undo_history) + 4 (registro). ✓
- §10 Testing → cada task tiene tests; Task 12 verificación viva. ✓
- §11 Puertas → en cada commit. ✓
- §12 Riesgos (mover ops_actions, HWND, batch-rename fuera) → Tasks 1, 11; batch-rename
  documentado como fuera de alcance. ✓

Sin placeholders: todos los steps tienen código o comandos concretos. Tipos consistentes:
`OpsCtrl`, `ActiveOp`, `OpDialog`, `ConflictAction`/`ConflictDecision`, `start_op`,
`pump_ops`, `resolve_conflict`, `cancel_op`, `first_collision`, `cut_set`/`is_cut` se usan con
los mismos nombres en todas las tasks.
