# ops-A — Operaciones de archivo — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Set completo de operaciones de archivo (copiar, mover, eliminar a papelera/permanente, renombrar, crear), cancelables y no-bloqueantes, con panel de operaciones (velocidad/bytes/progreso animado/cola), diálogos de conflicto y confirmación, resumen exportable, y disparadores por teclado/toolbar/menú contextual/entre-paneles.

**Architecture:** `naygo-core::ops` define el modelo + la planificación PURA (testeable sin disco) + el motor (worker que copia por buffers, cancelable, comunica por canal). `naygo-platform::trash` hace la papelera (Win32 IFileOperation). `naygo-ui` dispara, confirma con modales, y pinta el panel de operaciones; nunca bloquea (worker + mpsc, patrón `listing`).

**Tech Stack:** Rust, `naygo-core`/`naygo-platform`/`naygo-ui`, `eframe`/`egui` 0.34.3, crate `windows` 0.62 (COM + Shell). `tempfile` (dev) para tests del motor. Sin dependencias de terceros nuevas.

**Estado de partida (rama base `feat/ops-a`, desde `main`):**
- `naygo_core::cancel::CancellationToken` (Clone, `cancel()`, `is_cancelled()`) — el token a usar en los workers.
- `naygo_core::listing` PATRÓN DEL WORKER A IMITAR: `spawn_listing_filtered(dir, token, filter) -> (Receiver<ListingMsg>, JoinHandle<()>)` hace `let (tx, rx) = mpsc::channel(); let handle = thread::spawn(move || { ... loop con token.is_cancelled() ... tx.send(...) }); (rx, handle)`.
- `naygo_core::fs_model::Entry { name, path: PathBuf, kind: EntryKind, size: Option<u64>, modified, created, hidden }`. `EntryKind { Directory, File, Other }`.
- `naygo_core::workspace::file_pane::FilePaneState { current_dir: PathBuf, entries: Vec<Entry>, selected: Vec<usize>, focused: Option<usize>, table, sort, ... }`. `view_indices() -> Vec<usize>` (vista filtrada), `focused_view_entry() -> Option<&Entry>`.
- `naygo_core::config::Settings { version, bar_position, icon_only, icon_set, show_parent_entry, language, theme }`. Campos aditivos usan `#[serde(default = "fn")]`. `config::{portable_dir() -> PathBuf, load_settings, save_settings}`. `CONFIG_VERSION` gate en load.
- `naygo-ui::app::NaygoApp`: tiene `workspace`, `listings: HashMap<PaneId, PaneListing>`, `settings`, `i18n`, `icons`, `active_theme`, `config_dir`. `ui()` (eframe::App) construye toolbar + status bar (`egui::Panel::bottom("status_bar")`) + DockArea con `NaygoTabViewer`, procesa `pending`/`tree_actions`/`table_actions`. `logic()` hace `pump_all()`/`pump_tree()`/`pump_ops` (a agregar) + `handle_input(ctx)` + repaint. `tr(key)->String`. `eframe::App::save` llama `config::save_settings(&self.config_dir, &self.settings)`.
- `naygo-ui::input::Action` enum (MoveUp/Down/Activate/GoUp/GoBack/GoForward/SwitchPane/CancelListing) + `map_key`/`handle_input` en app.rs que lee `ctx.input` y aplica acciones al panel activo. `workspace.active_files()/active_files_mut()/active_id()`.
- `naygo-ui::docking::{NaygoTabViewer, PaneRequest{NavigateTo,Activate}}`. Files arm llama `file_panel::show(...)`.
- `naygo-ui::toolbar::{show, buttons}` pinta botones (back/forward/up/refresh + ➕ + plantillas + ⚙). `naygo-ui::panes::file_panel::show(...)` usa `egui_extras::TableBuilder`, fila completa clicable (`row.response()`), selección `row.set_selected`.
- `crates/core/Cargo.toml`: `[dev-dependencies] tempfile = "3"`.
- `crates/platform/Cargo.toml`: `windows` (cfg windows) con features `Win32_Globalization`, `Win32_Storage_FileSystem`, `Win32_System_WindowsProgramming`. `naygo-platform` ya es dep de `naygo-ui`.

**Prerequisito:** toolchain Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo en PowerShell. Binario `--bin naygo`. Verificar `$LASTEXITCODE`.

**Convenciones (CLAUDE.md):** código en inglés; comentarios/commits en español OK. Header de 2 líneas en archivos NUEVOS. `core` NUNCA importa egui/windows. UI nunca hace I/O. Build limpio + tests + clippy antes de cada commit. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/ops-a`. NO cambiar de rama.

**Alcance:** ENTRA: core::ops (modelo+plan puro+motor worker), platform::trash, Settings de ops, ui (panel, diálogos, disparadores, clipboard interno), i18n. NO ENTRA: journal/retomar-tras-crash (ops-B), portapapeles del SO/paste inteligente, menú nativo/ShellExecute (platform/shell), drag&drop COM.

---

## Estructura de archivos

```
crates/core/src/ops/
├── mod.rs     # OpKind, ConflictPolicy, OpRequest, OpStep, OpPlan, OpMsg, OpProgress, OpSummary, OpOutcome
├── names.rs   # validación de nombres + resolución de conflicto (archivo (2).ext) — puro
├── plan.rs    # plan(): expandir a pasos+bytes, detectar carpeta-dentro-de-sí, mismo-vol vs cruzado
└── engine.rs  # spawn(): motor worker (copia por buffers, cancelación, conflict_rx, summary)

crates/platform/src/trash.rs   # move_to_trash (Win32 IFileOperation) + stub no-win
crates/ui/src/ops_actions.rs   # OpRequest desde disparadores (puro, testeable)
crates/ui/src/ops_panel.rs     # panel de operaciones
crates/ui/src/ops_dialogs.rs   # modales confirmación + conflicto
```

---

## Task 1: `core::ops` — modelo de tipos (mod.rs)

**Files:**
- Create: `crates/core/src/ops/mod.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear `ops/mod.rs` con los tipos**

Create `crates/core/src/ops/mod.rs`:

```rust
// Naygo — operaciones de archivo: modelo de tipos (puro, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Tipos que describen una operación de archivo (copiar/mover/eliminar/renombrar/
//! crear), su plan (pasos + bytes), los mensajes del motor (progreso/conflicto/fin)
//! y el resumen. La planificación (`plan`) y la ejecución (`engine`) viven en sus
//! propios submódulos. Todo el modelo es serializable (útil para el journal de ops-B).

pub mod engine;
pub mod names;
pub mod plan;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Qué operación se pide.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpKind {
    Copy,
    Move,
    Delete { to_trash: bool },
    Rename { new_name: String },
    CreateDir { name: String },
    CreateFile { name: String },
}

/// Qué hacer ante un nombre que ya existe en el destino.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictPolicy {
    /// Preguntar a la UI (emite `OpMsg::Conflict` y espera la decisión).
    Ask,
    Overwrite,
    Skip,
    Rename,
}

/// Solicitud de operación, armada por la UI desde la selección.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpRequest {
    pub kind: OpKind,
    pub sources: Vec<PathBuf>,
    /// Carpeta destino (Copy/Move). `None` para Delete/Rename/Create.
    pub dest_dir: Option<PathBuf>,
    pub conflict: ConflictPolicy,
}

/// Un paso concreto del plan: copiar/mover un archivo, o crear una carpeta.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpStep {
    /// Origen (None para crear carpeta vacía en el destino).
    pub from: Option<PathBuf>,
    pub to: PathBuf,
    pub bytes: u64,
    pub is_dir: bool,
}

/// Plan completo de una operación: pasos + totales.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpPlan {
    pub steps: Vec<OpStep>,
    pub total_bytes: u64,
    pub total_files: usize,
}

/// Progreso emitido por el motor mientras trabaja.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpProgress {
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: usize,
    pub files_total: usize,
    pub current: PathBuf,
}

/// Petición de decisión de conflicto que el motor manda a la UI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConflictPrompt {
    pub existing: PathBuf,
    pub incoming: PathBuf,
}

/// Decisión que la UI devuelve al motor ante un conflicto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictDecision {
    pub action: ConflictAction,
    /// Aplicar esta decisión a todos los conflictos siguientes de la op.
    pub apply_all: bool,
}

/// Acción concreta de un conflicto resuelto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictAction {
    Overwrite,
    Skip,
    Rename,
}

/// Resultado por archivo, para el resumen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OpOutcome {
    Done,
    Skipped,
    Failed(String),
}

/// Resumen de una operación terminada (o cancelada).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpSummary {
    /// (ruta destino, resultado) por archivo procesado.
    pub items: Vec<(PathBuf, OpOutcome)>,
    pub bytes_done: u64,
    pub elapsed_secs: f64,
}

impl OpSummary {
    pub fn count_done(&self) -> usize {
        self.items.iter().filter(|(_, o)| matches!(o, OpOutcome::Done)).count()
    }
    pub fn count_skipped(&self) -> usize {
        self.items.iter().filter(|(_, o)| matches!(o, OpOutcome::Skipped)).count()
    }
    pub fn count_failed(&self) -> usize {
        self.items.iter().filter(|(_, o)| matches!(o, OpOutcome::Failed(_))).count()
    }
}

/// Mensajes que el motor emite hacia la UI por el canal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OpMsg {
    Progress(OpProgress),
    Conflict(ConflictPrompt),
    Done(OpSummary),
    Cancelled(OpSummary),
    /// Error fatal que impide siquiera empezar (p. ej. plan inválido).
    Failed(String),
}
```

NOTA: `pub mod engine; pub mod names; pub mod plan;` se declaran ahora pero esos archivos se crean en Tareas 2-4. Para que ESTA tarea compile sola, crea los 3 archivos como stubs mínimos: `names.rs` y `plan.rs` con solo el header de 2 líneas + un `//!` doc (vacíos de código), y `engine.rs` igual. Se llenan en sus tareas. (Sin los stubs, `pub mod` falla.) Crea los 3 stubs en este paso.

- [ ] **Step 2: Declarar el módulo + re-exports**

Modify `crates/core/src/lib.rs`:
- Añadir `pub mod ops;` en orden alfabético (entre `pub mod listing;` y `pub mod sort;` — `listing < ops < sort`; READ para ubicar).
- Re-export: `pub use ops::{OpKind, OpRequest, ConflictPolicy, OpMsg, OpSummary};`

- [ ] **Step 3: Verificar**

Run: `cargo build -p naygo-core` → compila (los stubs vacíos compilan).
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/ops/ crates/core/src/lib.rs
git commit -m "feat(core): modelo de tipos de operaciones (ops::mod)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `core::ops::names` — validación + resolución de conflicto (puro)

**Files:**
- Modify: `crates/core/src/ops/names.rs`

- [ ] **Step 1: Escribir los tests (TDD)**

Replace `crates/core/src/ops/names.rs` (era stub) con header + un `#[cfg(test)] mod tests` primero:

```rust
// Naygo — nombres de archivo: validación y resolución de conflictos (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Funciones puras sobre nombres: validar caracteres prohibidos en Windows y generar
//! el siguiente nombre libre ante un conflicto (`archivo (2).ext`).

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn nombre_valido() {
        assert!(is_valid_name("informe.pdf"));
        assert!(is_valid_name("Carpeta nueva"));
    }

    #[test]
    fn nombre_invalido_caracteres_prohibidos() {
        for bad in ["a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b"] {
            assert!(!is_valid_name(bad), "{bad} debería ser inválido");
        }
        assert!(!is_valid_name(""), "vacío inválido");
    }

    #[test]
    fn dedup_sin_conflicto_devuelve_igual() {
        let exists = |_p: &std::path::Path| false;
        let out = dedup_name(&PathBuf::from("C:/x/a.txt"), &exists);
        assert_eq!(out, PathBuf::from("C:/x/a.txt"));
    }

    #[test]
    fn dedup_con_conflicto_agrega_sufijo() {
        // a.txt existe → a (2).txt
        let taken: HashSet<PathBuf> = [PathBuf::from("C:/x/a.txt")].into_iter().collect();
        let exists = |p: &std::path::Path| taken.contains(p);
        let out = dedup_name(&PathBuf::from("C:/x/a.txt"), &exists);
        assert_eq!(out, PathBuf::from("C:/x/a (2).txt"));
    }

    #[test]
    fn dedup_incrementa_si_2_tambien_existe() {
        let taken: HashSet<PathBuf> = [
            PathBuf::from("C:/x/a.txt"),
            PathBuf::from("C:/x/a (2).txt"),
        ].into_iter().collect();
        let exists = |p: &std::path::Path| taken.contains(p);
        let out = dedup_name(&PathBuf::from("C:/x/a.txt"), &exists);
        assert_eq!(out, PathBuf::from("C:/x/a (3).txt"));
    }

    #[test]
    fn dedup_sin_extension() {
        let taken: HashSet<PathBuf> = [PathBuf::from("C:/x/LEEME")].into_iter().collect();
        let exists = |p: &std::path::Path| taken.contains(p);
        let out = dedup_name(&PathBuf::from("C:/x/LEEME"), &exists);
        assert_eq!(out, PathBuf::from("C:/x/LEEME (2)"));
    }
}
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core ops::names`
Expected: ERROR de compilación — `is_valid_name`/`dedup_name` no existen.

- [ ] **Step 3: Implementar (antes del mod tests)**

```rust
use std::path::{Path, PathBuf};

/// Caracteres prohibidos en nombres de archivo de Windows.
const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// `true` si `name` es un nombre de archivo/carpeta válido (no vacío, sin caracteres
/// prohibidos en Windows, sin ser solo espacios/puntos).
pub fn is_valid_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.trim().is_empty() || name.chars().all(|c| c == '.') {
        return false;
    }
    !name.chars().any(|c| FORBIDDEN.contains(&c) || (c as u32) < 0x20)
}

/// Dada una ruta destino candidata y un predicado `exists`, devuelve la primera ruta
/// libre añadiendo " (N)" antes de la extensión si hace falta. Pura: el caller provee
/// `exists` (en tests es un set; en el motor es `Path::exists`).
pub fn dedup_name(candidate: &Path, exists: &dyn Fn(&Path) -> bool) -> PathBuf {
    if !exists(candidate) {
        return candidate.to_path_buf();
    }
    let dir = candidate.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let stem = candidate
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let ext = candidate.extension().and_then(|s| s.to_str()).map(|s| s.to_string());
    // Empezar en (2).
    let mut n = 2u32;
    loop {
        let name = match &ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        let cand = dir.join(name);
        if !exists(&cand) {
            return cand;
        }
        n += 1;
    }
}
```

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core ops::names` → 6 tests PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 5: Re-export + commit**

Modify `crates/core/src/ops/mod.rs`: añadir `pub use names::{dedup_name, is_valid_name};` tras los `pub mod`.

```bash
git add crates/core/src/ops/names.rs crates/core/src/ops/mod.rs
git commit -m "feat(core): validación de nombres + dedup de conflictos (ops::names)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `core::ops::plan` — planificación pura

**Files:**
- Modify: `crates/core/src/ops/plan.rs`

- [ ] **Step 1: Escribir los tests (TDD)**

Replace `crates/core/src/ops/plan.rs` (stub) con header + tests primero. La planificación recorre el filesystem (para expandir carpetas y leer tamaños), así que se testea con `tempfile` (dev-dep ya presente):

```rust
// Naygo — planificación de operaciones: expandir a pasos + validar (recorre FS).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `plan` toma una `OpRequest` y produce un `OpPlan` (lista de pasos + totales),
//! validando precondiciones (nombres, carpeta-dentro-de-sí-misma). Para Copy/Move
//! recorre el árbol de orígenes leyendo tamaños. Devuelve `Result<OpPlan, PlanError>`.

use super::{OpKind, OpPlan, OpRequest, OpStep};
use std::path::{Path, PathBuf};

/// Error de planificación (antes de empezar a ejecutar).
#[derive(Debug, PartialEq)]
pub enum PlanError {
    /// El destino está dentro de uno de los orígenes (copia recursiva infinita).
    DestInsideSource,
    /// Nombre inválido (al renombrar/crear).
    InvalidName(String),
    /// Falta el destino para una op que lo requiere.
    MissingDest,
    /// Un origen no existe / no se pudo leer.
    SourceUnreadable(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{ConflictPolicy, OpKind, OpRequest};
    use std::fs;

    fn req(kind: OpKind, sources: Vec<PathBuf>, dest: Option<PathBuf>) -> OpRequest {
        OpRequest { kind, sources, dest_dir: dest, conflict: ConflictPolicy::Overwrite }
    }

    #[test]
    fn copy_archivo_simple_un_paso_con_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"hola").unwrap(); // 4 bytes
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let plan = plan(&req(OpKind::Copy, vec![src.clone()], Some(dest.clone()))).unwrap();
        assert_eq!(plan.total_files, 1);
        assert_eq!(plan.total_bytes, 4);
        assert_eq!(plan.steps[0].to, dest.join("a.txt"));
        assert_eq!(plan.steps[0].bytes, 4);
        assert!(!plan.steps[0].is_dir);
    }

    #[test]
    fn copy_carpeta_recursiva_expande_pasos() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"aa").unwrap();   // 2
        fs::create_dir(src.join("sub")).unwrap();
        fs::write(src.join("sub/b.txt"), b"bbb").unwrap(); // 3
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let plan = plan(&req(OpKind::Copy, vec![src], Some(dest))).unwrap();
        // Pasos: crear carpeta + crear sub + a.txt + b.txt. total_files cuenta archivos (2).
        assert_eq!(plan.total_bytes, 5);
        assert_eq!(plan.total_files, 2);
        assert!(plan.steps.iter().any(|s| s.is_dir));
    }

    #[test]
    fn copy_carpeta_dentro_de_si_misma_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        let dest = src.join("sub"); // dest dentro de src
        fs::create_dir(&dest).unwrap();
        let e = plan(&req(OpKind::Copy, vec![src], Some(dest))).unwrap_err();
        assert_eq!(e, PlanError::DestInsideSource);
    }

    #[test]
    fn rename_nombre_invalido_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let r = req(OpKind::Rename { new_name: "a/b.txt".into() }, vec![src], None);
        let e = plan(&r).unwrap_err();
        assert!(matches!(e, PlanError::InvalidName(_)));
    }

    #[test]
    fn copy_sin_dest_es_error() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let e = plan(&req(OpKind::Copy, vec![src], None)).unwrap_err();
        assert_eq!(e, PlanError::MissingDest);
    }
}
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core ops::plan`
Expected: ERROR — `plan` no existe.

- [ ] **Step 3: Implementar `plan` (antes del mod tests)**

```rust
use super::names::is_valid_name;

/// Planifica una `OpRequest`: produce los pasos + totales, o un `PlanError`.
pub fn plan(req: &OpRequest) -> Result<OpPlan, PlanError> {
    match &req.kind {
        OpKind::Copy | OpKind::Move => plan_transfer(req),
        OpKind::Delete { .. } => plan_delete(req),
        OpKind::Rename { new_name } => {
            if !is_valid_name(new_name) {
                return Err(PlanError::InvalidName(new_name.clone()));
            }
            // Renombrar es un paso lógico; sin recorrido de bytes.
            let from = req.sources.first().cloned();
            let to = from
                .as_ref()
                .and_then(|p| p.parent())
                .map(|parent| parent.join(new_name))
                .ok_or(PlanError::MissingDest)?;
            Ok(OpPlan {
                steps: vec![OpStep { from, to, bytes: 0, is_dir: false }],
                total_bytes: 0,
                total_files: 1,
            })
        }
        OpKind::CreateDir { name } | OpKind::CreateFile { name } => {
            if !is_valid_name(name) {
                return Err(PlanError::InvalidName(name.clone()));
            }
            let dest = req.dest_dir.clone().ok_or(PlanError::MissingDest)?;
            let is_dir = matches!(req.kind, OpKind::CreateDir { .. });
            Ok(OpPlan {
                steps: vec![OpStep { from: None, to: dest.join(name), bytes: 0, is_dir }],
                total_bytes: 0,
                total_files: if is_dir { 0 } else { 1 },
            })
        }
    }
}

/// Plan de copiar/mover: expande cada origen (recursivo) a pasos.
fn plan_transfer(req: &OpRequest) -> Result<OpPlan, PlanError> {
    let dest = req.dest_dir.clone().ok_or(PlanError::MissingDest)?;
    // Validar dest-dentro-de-source para cada origen que sea carpeta.
    for src in &req.sources {
        if src.is_dir() && is_inside(&dest, src) {
            return Err(PlanError::DestInsideSource);
        }
    }
    let mut steps = Vec::new();
    let mut total_bytes = 0u64;
    let mut total_files = 0usize;
    for src in &req.sources {
        let base_to = dest.join(src.file_name().unwrap_or_default());
        expand(src, &base_to, &mut steps, &mut total_bytes, &mut total_files)?;
    }
    Ok(OpPlan { steps, total_bytes, total_files })
}

/// Plan de eliminar: un paso por origen (el motor recorre al borrar). Bytes 0 (no
/// se calculan para borrar; el progreso de delete es por archivo).
fn plan_delete(req: &OpRequest) -> Result<OpPlan, PlanError> {
    let mut steps = Vec::new();
    for src in &req.sources {
        if !src.exists() {
            return Err(PlanError::SourceUnreadable(src.clone()));
        }
        let is_dir = src.is_dir();
        steps.push(OpStep { from: Some(src.clone()), to: src.clone(), bytes: 0, is_dir });
    }
    let n = steps.iter().filter(|s| !s.is_dir).count();
    Ok(OpPlan { steps, total_bytes: 0, total_files: n })
}

/// Recorre `src` (archivo o carpeta) agregando pasos hacia `to`.
fn expand(
    src: &Path,
    to: &Path,
    steps: &mut Vec<OpStep>,
    total_bytes: &mut u64,
    total_files: &mut usize,
) -> Result<(), PlanError> {
    let meta = std::fs::metadata(src).map_err(|_| PlanError::SourceUnreadable(src.to_path_buf()))?;
    if meta.is_dir() {
        // Paso para crear la carpeta destino.
        steps.push(OpStep { from: Some(src.to_path_buf()), to: to.to_path_buf(), bytes: 0, is_dir: true });
        let entries = std::fs::read_dir(src).map_err(|_| PlanError::SourceUnreadable(src.to_path_buf()))?;
        for entry in entries.flatten() {
            let child = entry.path();
            let child_to = to.join(entry.file_name());
            expand(&child, &child_to, steps, total_bytes, total_files)?;
        }
    } else {
        let bytes = meta.len();
        steps.push(OpStep { from: Some(src.to_path_buf()), to: to.to_path_buf(), bytes, is_dir: false });
        *total_bytes += bytes;
        *total_files += 1;
    }
    Ok(())
}

/// `true` si `inner` está dentro de (o es igual a) `outer`.
fn is_inside(inner: &Path, outer: &Path) -> bool {
    inner.starts_with(outer)
}
```

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core ops::plan` → 5 tests PASS.
Run: `cargo test -p naygo-core` → verde.
Run: `cargo clippy -p naygo-core --lib --all-targets -- -D warnings` → limpio.

- [ ] **Step 5: Re-export + commit**

Modify `crates/core/src/ops/mod.rs`: añadir `pub use plan::{plan, PlanError};`.

```bash
git add crates/core/src/ops/plan.rs crates/core/src/ops/mod.rs
git commit -m "feat(core): planificación de operaciones (ops::plan, recorrido + validación)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `core::ops::engine` — motor worker (copia por buffers, cancelable)

**Files:**
- Modify: `crates/core/src/ops/engine.rs`

- [ ] **Step 1: Escribir los tests (TDD, con tempfile)**

Replace `crates/core/src/ops/engine.rs` (stub) con header + tests. El motor se testea de forma SÍNCRONA con un helper `run_plan` (sin spawnear hilo), igual que `listing` testea `list_into_filtered`:

```rust
// Naygo — motor de operaciones: ejecuta un OpPlan en un worker, cancelable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Ejecuta los pasos de un `OpPlan` copiando archivos POR BUFFERS (cancelable a
//! media copia), emitiendo `OpMsg` por canal. Un error de un paso no aborta la op
//! (se registra en el summary). Cancelar borra el parcial del archivo en curso.

use super::{ConflictDecision, OpKind, OpMsg, OpOutcome, OpPlan, OpProgress, OpSummary};
use crate::cancel::CancellationToken;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};

/// Tamaño de buffer para copiar (y granularidad de cancelación dentro de un archivo).
const BUF_SIZE: usize = 256 * 1024;

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::plan::plan;
    use super::super::{ConflictPolicy, OpKind, OpRequest};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;

    fn run(req: OpRequest) -> (Vec<OpMsg>, OpSummary) {
        let p = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let summary = run_plan(&p, &req.kind, &token, &tx, &crx);
        drop(tx);
        let msgs: Vec<OpMsg> = rx.into_iter().collect();
        (msgs, summary)
    }

    #[test]
    fn copy_archivo_crea_destino() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"contenido").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = OpRequest { kind: OpKind::Copy, sources: vec![src], dest_dir: Some(dest.clone()), conflict: ConflictPolicy::Overwrite };
        let (_msgs, summary) = run(req);
        assert!(dest.join("a.txt").exists());
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"contenido");
        assert_eq!(summary.count_done(), 1);
    }

    #[test]
    fn copy_conflicto_skip_no_sobrescribe() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"nuevo").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("a.txt"), b"viejo").unwrap();
        let req = OpRequest { kind: OpKind::Copy, sources: vec![src], dest_dir: Some(dest.clone()), conflict: ConflictPolicy::Skip };
        let (_m, summary) = run(req);
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"viejo"); // no tocó
        assert_eq!(summary.count_skipped(), 1);
    }

    #[test]
    fn copy_conflicto_overwrite_reemplaza() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"nuevo").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("a.txt"), b"viejo").unwrap();
        let req = OpRequest { kind: OpKind::Copy, sources: vec![src], dest_dir: Some(dest.clone()), conflict: ConflictPolicy::Overwrite };
        let (_m, _s) = run(req);
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"nuevo");
    }

    #[test]
    fn move_borra_el_origen() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = OpRequest { kind: OpKind::Move, sources: vec![src.clone()], dest_dir: Some(dest.clone()), conflict: ConflictPolicy::Overwrite };
        let (_m, _s) = run(req);
        assert!(!src.exists());
        assert!(dest.join("a.txt").exists());
    }

    #[test]
    fn cancelar_antes_de_empezar_no_copia() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let p = plan(&OpRequest { kind: OpKind::Copy, sources: vec![src], dest_dir: Some(dest.clone()), conflict: ConflictPolicy::Overwrite }).unwrap();
        let token = CancellationToken::new();
        token.cancel();
        let (tx, _rx) = mpsc::channel();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let _summary = run_plan(&p, &OpKind::Copy, &token, &tx, &crx);
        assert!(!dest.join("a.txt").exists());
    }

    #[test]
    fn delete_permanente_borra() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let req = OpRequest { kind: OpKind::Delete { to_trash: false }, sources: vec![src.clone()], dest_dir: None, conflict: ConflictPolicy::Overwrite };
        let (_m, summary) = run(req);
        assert!(!src.exists());
        assert_eq!(summary.count_done(), 1);
    }
}
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core ops::engine`
Expected: ERROR — `run_plan`/`spawn` no existen.

- [ ] **Step 3: Implementar el motor (antes del mod tests)**

```rust
/// Lanza el motor en un worker. Devuelve el receptor de `OpMsg` y el handle. El
/// `conflict_rx` recibe las decisiones de la UI cuando la política es `Ask`.
pub fn spawn(
    plan: OpPlan,
    kind: OpKind,
    token: CancellationToken,
    conflict_rx: Receiver<ConflictDecision>,
) -> (Receiver<OpMsg>, std::thread::JoinHandle<()>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let summary = run_plan(&plan, &kind, &token, &tx, &conflict_rx);
        if token.is_cancelled() {
            let _ = tx.send(OpMsg::Cancelled(summary));
        } else {
            let _ = tx.send(OpMsg::Done(summary));
        }
    });
    (rx, handle)
}

/// Cuerpo del motor (síncrono, testeable). Ejecuta cada paso, emite progreso y
/// arma el summary. NO envía Done/Cancelled (eso lo hace `spawn` según el token).
pub fn run_plan(
    plan: &OpPlan,
    kind: &OpKind,
    token: &CancellationToken,
    tx: &Sender<OpMsg>,
    _conflict_rx: &Receiver<ConflictDecision>,
) -> OpSummary {
    let start = std::time::Instant::now();
    let mut summary = OpSummary::default();
    let mut files_done = 0usize;

    for step in &plan.steps {
        if token.is_cancelled() {
            break;
        }
        let outcome = match kind {
            OpKind::Delete { to_trash } => exec_delete(step, *to_trash),
            OpKind::Rename { .. } => exec_rename(step),
            OpKind::CreateDir { .. } => exec_create_dir(step),
            OpKind::CreateFile { .. } => exec_create_file(step),
            OpKind::Copy | OpKind::Move => {
                // Conflicto: si destino existe, decidir según la política embebida en
                // los pasos. En ops-A el conflicto Ask se resuelve en la UI ANTES de
                // spawnear (la UI re-planifica con la decisión); el motor recibe pasos
                // ya resueltos (Overwrite/Skip/Rename aplicados al `to`). Por eso aquí
                // solo distinguimos "destino existe + no debemos sobrescribir" → skip.
                exec_copy_step(step, kind, token, tx, &mut summary.bytes_done)
            }
        };
        if !step.is_dir {
            files_done += 1;
        }
        // Emitir progreso tras cada archivo.
        let _ = tx.send(OpMsg::Progress(OpProgress {
            bytes_done: summary.bytes_done,
            bytes_total: plan.total_bytes,
            files_done,
            files_total: plan.total_files,
            current: step.to.clone(),
        }));
        // Registrar resultado (no registrar carpetas creadas como "archivos").
        if !step.is_dir {
            summary.items.push((step.to.clone(), outcome));
        } else if let OpOutcome::Failed(_) = outcome {
            summary.items.push((step.to.clone(), outcome));
        }
    }

    // Si se mueve, borrar los orígenes ya copiados (cuando Copy+Move). En este diseño
    // simple, Move de mismo-volumen lo hace exec_copy_step con rename; el cruzado copia
    // y aquí no re-borra (exec_copy_step de Move ya borró el origen del archivo). Ver
    // exec_copy_step.
    summary.elapsed_secs = start.elapsed().as_secs_f64();
    summary
}

/// Copia (o mueve) UN archivo por buffers, cancelable. Devuelve el outcome.
fn exec_copy_step(
    step: &super::OpStep,
    kind: &OpKind,
    token: &CancellationToken,
    tx: &Sender<OpMsg>,
    bytes_done: &mut u64,
) -> OpOutcome {
    if step.is_dir {
        // Crear la carpeta destino.
        if let Err(e) = std::fs::create_dir_all(&step.to) {
            return OpOutcome::Failed(e.to_string());
        }
        return OpOutcome::Done;
    }
    let Some(from) = &step.from else {
        return OpOutcome::Failed("paso sin origen".into());
    };
    // Skip si el destino ya existe (la UI ya resolvió conflicto re-escribiendo `to`
    // cuando era Rename; Overwrite deja el `to` original y aquí truncamos).
    let is_move = matches!(kind, OpKind::Move);
    // Move mismo-volumen: intentar rename (rápido). Si falla (cruzado), copiar+borrar.
    if is_move {
        if std::fs::rename(from, &step.to).is_ok() {
            *bytes_done += step.bytes;
            return OpOutcome::Done;
        }
    }
    match copy_buffered(from, &step.to, token, tx, bytes_done, step.bytes) {
        Ok(true) => {
            if is_move {
                let _ = std::fs::remove_file(from);
            }
            OpOutcome::Done
        }
        Ok(false) => {
            // Cancelado a mitad: borrar el parcial.
            let _ = std::fs::remove_file(&step.to);
            OpOutcome::Skipped
        }
        Err(e) => {
            let _ = std::fs::remove_file(&step.to);
            OpOutcome::Failed(e.to_string())
        }
    }
}

/// Copia byte a byte por buffers; chequea el token entre bloques. `Ok(true)` =
/// completo, `Ok(false)` = cancelado a mitad.
fn copy_buffered(
    from: &Path,
    to: &Path,
    token: &CancellationToken,
    tx: &Sender<OpMsg>,
    bytes_done: &mut u64,
    total: u64,
) -> std::io::Result<bool> {
    let mut reader = std::fs::File::open(from)?;
    let mut writer = std::fs::File::create(to)?;
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        if token.is_cancelled() {
            return Ok(false);
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        *bytes_done += n as u64;
        // Progreso intra-archivo (barra animada ligada a bytes).
        let _ = tx.send(OpMsg::Progress(OpProgress {
            bytes_done: *bytes_done,
            bytes_total: total.max(*bytes_done),
            files_done: 0,
            files_total: 0,
            current: to.to_path_buf(),
        }));
    }
    writer.flush()?;
    Ok(true)
}

fn exec_delete(step: &super::OpStep, to_trash: bool) -> OpOutcome {
    let Some(target) = &step.from else { return OpOutcome::Failed("sin objetivo".into()); };
    if to_trash {
        // La papelera la hace platform; en core, si se pide to_trash, lo marcamos como
        // no soportado a nivel core (la UI llama platform::move_to_trash directamente
        // para el caso papelera; el motor core solo borra permanente). Ver NOTA.
        return OpOutcome::Failed("papelera se maneja en platform".into());
    }
    let res = if step.is_dir { std::fs::remove_dir_all(target) } else { std::fs::remove_file(target) };
    match res {
        Ok(()) => OpOutcome::Done,
        Err(e) => OpOutcome::Failed(e.to_string()),
    }
}

fn exec_rename(step: &super::OpStep) -> OpOutcome {
    let Some(from) = &step.from else { return OpOutcome::Failed("sin origen".into()); };
    match std::fs::rename(from, &step.to) {
        Ok(()) => OpOutcome::Done,
        Err(e) => OpOutcome::Failed(e.to_string()),
    }
}

fn exec_create_dir(step: &super::OpStep) -> OpOutcome {
    match std::fs::create_dir(&step.to) {
        Ok(()) => OpOutcome::Done,
        Err(e) => OpOutcome::Failed(e.to_string()),
    }
}

fn exec_create_file(step: &super::OpStep) -> OpOutcome {
    match std::fs::File::create(&step.to) {
        Ok(_) => OpOutcome::Done,
        Err(e) => OpOutcome::Failed(e.to_string()),
    }
}
```

NOTA IMPORTANTE sobre papelera y conflicto Ask (decisión de diseño para mantener el motor simple y testeable):
- **Papelera**: el motor core solo hace borrado PERMANENTE (`to_trash=false`). Cuando el usuario pide borrar a papelera, la UI llama `naygo_platform::trash::move_to_trash(paths)` DIRECTAMENTE (Tarea 6), sin pasar por el motor (la papelera es atómica vía Win32 y no necesita el motor por-buffers). El motor maneja permanente.
- **Conflicto Ask**: para ops-A, la resolución de conflictos `Ask` la hace la UI ANTES de spawnear el worker: la UI detecta colisiones (con `plan` + chequeo de existencia), muestra el modal, y re-arma los pasos con la decisión (Overwrite deja `to`; Skip omite el paso; Rename cambia `to` vía `dedup_name`). El motor recibe pasos ya resueltos y NO emite `OpMsg::Conflict` en esta fase. (El `conflict_rx` queda en la firma para ops-B/futuro, sin usar — marcar `_conflict_rx`.) Esto evita el ida-y-vuelta worker↔UI a media operación, más simple y robusto para ops-A. Si el reviewer prefiere el modelo con el worker preguntando en vivo, es un cambio acotado; para ops-A vamos con resolución previa.

Esta NOTA simplifica el flujo: la UI resuelve conflictos al planificar; el motor ejecuta pasos limpios. Implementar así.

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core ops::engine` → 6 tests PASS.
Run: `cargo test -p naygo-core` → verde.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → limpio.

- [ ] **Step 5: Re-export + commit**

Modify `crates/core/src/ops/mod.rs`: añadir `pub use engine::{run_plan, spawn};` y a los re-exports de mod.rs: `ConflictDecision, ConflictAction, OpOutcome, OpProgress, OpStep, OpPlan`.

```bash
git add crates/core/src/ops/engine.rs crates/core/src/ops/mod.rs
git commit -m "feat(core): motor de operaciones (ops::engine, copia por buffers cancelable)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: `Settings` de ops (serde default)

**Files:**
- Modify: `crates/core/src/config/mod.rs`

- [ ] **Step 1: Tests (TDD)**

En el `#[cfg(test)] mod tests` de `config/mod.rs`:
```rust
    #[test]
    fn settings_default_ops() {
        let s = Settings::default();
        assert_eq!(s.ops_mode, OpsMode::Queue);
        assert_eq!(s.ops_display, OpsDisplay::Panel);
        assert!(!s.confirm_trash);
        assert!(s.show_op_summary);
    }

    #[test]
    fn settings_viejo_sin_ops_cae_a_defaults() {
        let json = r#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"Flat","show_parent_entry":true,"language":"es","theme":"dark-blue"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.ops_mode, OpsMode::Queue);
        assert!(s.show_op_summary);
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core config` → ERROR (falta `ops_mode`, `OpsMode`, etc.).

- [ ] **Step 3: Implementar**

Modify `crates/core/src/config/mod.rs`:
a) Añadir los enums (cerca de `BarPosition`/`IconSet`):
```rust
/// Modo de ejecución de operaciones múltiples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpsMode {
    /// Una operación a la vez (las demás esperan en cola).
    Queue,
    /// Varias en paralelo.
    Parallel,
}

/// Cómo se muestra el progreso de operaciones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpsDisplay {
    /// Panel acoplado abajo (oculto si no hay ops).
    Panel,
    /// Diálogo modal.
    Modal,
    /// Panel siempre visible.
    AlwaysVisible,
}
```
b) En `struct Settings`, tras `theme`:
```rust
    #[serde(default = "default_ops_mode")]
    pub ops_mode: OpsMode,
    #[serde(default = "default_ops_display")]
    pub ops_display: OpsDisplay,
    /// Confirmar también el borrado a papelera (el permanente siempre confirma).
    #[serde(default)]
    pub confirm_trash: bool,
    #[serde(default = "default_show_op_summary")]
    pub show_op_summary: bool,
```
c) Helpers:
```rust
fn default_ops_mode() -> OpsMode { OpsMode::Queue }
fn default_ops_display() -> OpsDisplay { OpsDisplay::Panel }
fn default_show_op_summary() -> bool { true }
```
d) En `impl Default for Settings`, añadir: `ops_mode: OpsMode::Queue, ops_display: OpsDisplay::Panel, confirm_trash: false, show_op_summary: true,`.
e) Si hay un test de round-trip con struct literal completo, añadir los 4 campos ahí.

- [ ] **Step 4: Verificar + commit**

Run: `cargo test -p naygo-core config` → PASS. `cargo test -p naygo-core` → verde. `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

```bash
git add crates/core/src/config/mod.rs
git commit -m "feat(core): Settings de operaciones (modo cola/paralelo, display, confirmación)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: `platform::trash` — papelera (Win32 IFileOperation)

**Files:**
- Create: `crates/platform/src/trash.rs`
- Modify: `crates/platform/src/lib.rs`, `crates/platform/Cargo.toml`

- [ ] **Step 1: Features Win32 en Cargo.toml**

Modify `crates/platform/Cargo.toml` — añadir a las features de `windows` (cfg windows): `"Win32_System_Com"`, `"Win32_UI_Shell"`, `"Win32_UI_Shell_Common"`, `"Win32_Foundation"`. (Ajustar al implementar según los símbolos que pida el compilador para `IFileOperation`/`SHCreateItemFromParsingName`.)

- [ ] **Step 2: Crear `trash.rs`**

Create `crates/platform/src/trash.rs`. La implementación usa COM `IFileOperation` con flag `FOF_ALLOWUNDO` (papelera). VERIFICAR las firmas exactas en `windows` 0.62 contra el registry al implementar (CoInitializeEx, CoCreateInstance de `FileOperation`, `SHCreateItemFromParsingName`, `IFileOperation::DeleteItem`, `PerformOperations`).

```rust
// Naygo — papelera de Windows (Win32 IFileOperation, COM, aislado).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `move_to_trash` envía rutas a la Papelera de reciclaje vía la API COM moderna
//! `IFileOperation` (con `FOFX_RECYCLEONDELETE`/`FOF_ALLOWUNDO`). El borrado
//! permanente NO vive aquí (lo hace `core::ops`). Tolerante: una ruta que falle no
//! tumba el proceso; se reporta en el `Result`.

use std::path::Path;

/// Error al enviar a papelera.
#[derive(Debug)]
pub enum TrashError {
    NotSupported,
    Failed(String),
}

#[cfg(windows)]
pub fn move_to_trash(paths: &[std::path::PathBuf]) -> Result<(), TrashError> {
    // Implementación COM (esqueleto; AJUSTAR a windows 0.62):
    // 1. CoInitializeEx(None, COINIT_APARTMENTTHREADED)
    // 2. op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?
    // 3. op.SetOperationFlags(FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT)
    // 4. para cada path: item = SHCreateItemFromParsingName(path_wide, None)?; op.DeleteItem(&item, None)?
    // 5. op.PerformOperations()
    // 6. CoUninitialize()
    // Devolver Ok(()) o TrashError::Failed con el HRESULT.
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOperation, IFileOperation, SHCreateItemFromParsingName, IShellItem,
        FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT,
    };

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| -> Result<(), TrashError> {
            let op: IFileOperation =
                CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|e| TrashError::Failed(e.to_string()))?;
            op.SetOperationFlags(FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT)
                .map_err(|e| TrashError::Failed(e.to_string()))?;
            for path in paths {
                let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
                let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)
                    .map_err(|e| TrashError::Failed(e.to_string()))?;
                op.DeleteItem(&item, None).map_err(|e| TrashError::Failed(e.to_string()))?;
            }
            op.PerformOperations().map_err(|e| TrashError::Failed(e.to_string()))?;
            Ok(())
        })();
        CoUninitialize();
        result
    }
}

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Stub no-Windows: la papelera no está disponible.
#[cfg(not(windows))]
pub fn move_to_trash(_paths: &[std::path::PathBuf]) -> Result<(), TrashError> {
    Err(TrashError::NotSupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn enviar_un_archivo_a_papelera() {
        let dir = std::env::temp_dir().join(format!("naygo_trash_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("borrame.txt");
        std::fs::write(&f, b"x").unwrap();
        assert!(f.exists());
        let res = move_to_trash(&[f.clone()]);
        assert!(res.is_ok(), "move_to_trash falló: {res:?}");
        assert!(!f.exists(), "el archivo debería haber ido a la papelera");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

VERIFICAR contra `windows` 0.62: nombres exactos de `FileOperation` (CLSID), `IFileOperation`, sus métodos (`SetOperationFlags`, `DeleteItem`, `PerformOperations`), `SHCreateItemFromParsingName`, los flags `FOF_*` (pueden estar en `Win32::UI::Shell` con tipo `FILEOPERATION_FLAGS`). Ajustar imports y tipos hasta compilar limpio. `CoInitializeEx` en 0.62 devuelve `Result<()>`/`HRESULT` — adaptar.

- [ ] **Step 3: Declarar módulo + verificar**

Modify `crates/platform/src/lib.rs`: `pub mod trash;` tras `pub mod drives;`.

Run: `cargo build -p naygo-platform` → compila (resolver la API COM real).
Run: `cargo test -p naygo-platform trash` → el test de papelera pasa en Windows.
Run: `cargo clippy -p naygo-platform -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/platform/src/trash.rs crates/platform/src/lib.rs crates/platform/Cargo.toml
git commit -m "feat(platform): enviar a la papelera (Win32 IFileOperation)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: i18n — claves de ops (ES + EN)

**Files:**
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: Añadir claves (ES)**

Modify `crates/core/src/i18n/es.json` — insertar (en un punto con coma válida; READ primero):
```json
  "op.copy": "Copiar",
  "op.cut": "Cortar",
  "op.paste": "Pegar",
  "op.delete": "Eliminar",
  "op.delete_permanent": "Eliminar permanentemente",
  "op.rename": "Renombrar",
  "op.new_file": "Nuevo archivo",
  "op.new_folder": "Nueva carpeta",
  "op.confirm_delete_title": "Eliminar",
  "op.confirm_delete_body": "¿Eliminar {n} elemento(s)?",
  "op.confirm_permanent_body": "Esto NO se puede deshacer. ¿Eliminar permanentemente {n} elemento(s)?",
  "op.conflict_title": "El destino ya existe",
  "op.conflict_body": "Ya existe «{name}» en el destino.",
  "op.overwrite": "Sobrescribir",
  "op.skip": "Saltar",
  "op.apply_all": "Aplicar a todos",
  "op.cancel": "Cancelar",
  "ops.panel_title": "Operaciones",
  "ops.expand": "Expandir",
  "ops.collapse": "Compactar",
  "ops.queued": "en espera",
  "ops.summary_done": "{done} hechos · {skipped} omitidos · {failed} con error",
  "ops.view_detail": "Ver detalle",
  "ops.export": "Exportar…"
```

- [ ] **Step 2: Mismas claves (EN)**

Modify `crates/core/src/i18n/en.json`:
```json
  "op.copy": "Copy",
  "op.cut": "Cut",
  "op.paste": "Paste",
  "op.delete": "Delete",
  "op.delete_permanent": "Delete permanently",
  "op.rename": "Rename",
  "op.new_file": "New file",
  "op.new_folder": "New folder",
  "op.confirm_delete_title": "Delete",
  "op.confirm_delete_body": "Delete {n} item(s)?",
  "op.confirm_permanent_body": "This CANNOT be undone. Permanently delete {n} item(s)?",
  "op.conflict_title": "Destination already exists",
  "op.conflict_body": "\"{name}\" already exists in the destination.",
  "op.overwrite": "Overwrite",
  "op.skip": "Skip",
  "op.apply_all": "Apply to all",
  "op.cancel": "Cancel",
  "ops.panel_title": "Operations",
  "ops.expand": "Expand",
  "ops.collapse": "Collapse",
  "ops.queued": "queued",
  "ops.summary_done": "{done} done · {skipped} skipped · {failed} failed",
  "ops.view_detail": "View detail",
  "ops.export": "Export…"
```

Ambos catálogos = mismo set de claves. JSON válido.

- [ ] **Step 3: Verificar + commit**

Run: `cargo test -p naygo-core i18n` → PASS (parity). `cargo test -p naygo-core` → verde.

```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "i18n: claves de operaciones (ES/EN)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: `ui::ops_actions` — armar OpRequest desde la selección (puro)

**Files:**
- Create: `crates/ui/src/ops_actions.rs`
- Modify: `crates/ui/src/main.rs`

- [ ] **Step 1: Crear con función pura + tests**

Create `crates/ui/src/ops_actions.rs`:
```rust
// Naygo — construcción de OpRequest desde la selección/disparadores (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Funciones puras que arman una `OpRequest` a partir de las rutas seleccionadas y el
//! destino. Separadas del render para testearlas sin egui.

use naygo_core::ops::{ConflictPolicy, OpKind, OpRequest};
use std::path::PathBuf;

/// Copiar/mover `sources` a `dest_dir`.
pub fn transfer(kind_move: bool, sources: Vec<PathBuf>, dest_dir: PathBuf) -> OpRequest {
    OpRequest {
        kind: if kind_move { OpKind::Move } else { OpKind::Copy },
        sources,
        dest_dir: Some(dest_dir),
        conflict: ConflictPolicy::Ask,
    }
}

/// Eliminar `sources` (a papelera o permanente).
pub fn delete(sources: Vec<PathBuf>, to_trash: bool) -> OpRequest {
    OpRequest {
        kind: OpKind::Delete { to_trash },
        sources,
        dest_dir: None,
        conflict: ConflictPolicy::Overwrite,
    }
}

/// Renombrar un archivo.
pub fn rename(source: PathBuf, new_name: String) -> OpRequest {
    OpRequest {
        kind: OpKind::Rename { new_name },
        sources: vec![source],
        dest_dir: None,
        conflict: ConflictPolicy::Ask,
    }
}

/// Crear carpeta/archivo en `dir`.
pub fn create(dir: PathBuf, name: String, is_dir: bool) -> OpRequest {
    OpRequest {
        kind: if is_dir { OpKind::CreateDir { name } } else { OpKind::CreateFile { name } },
        sources: vec![],
        dest_dir: Some(dir),
        conflict: ConflictPolicy::Ask,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_move_arma_kind_move() {
        let r = transfer(true, vec![PathBuf::from("a")], PathBuf::from("dst"));
        assert_eq!(r.kind, OpKind::Move);
        assert_eq!(r.dest_dir, Some(PathBuf::from("dst")));
    }

    #[test]
    fn delete_papelera_flag() {
        let r = delete(vec![PathBuf::from("a")], true);
        assert_eq!(r.kind, OpKind::Delete { to_trash: true });
    }

    #[test]
    fn create_dir_vs_file() {
        assert_eq!(create(PathBuf::from("d"), "x".into(), true).kind, OpKind::CreateDir { name: "x".into() });
        assert_eq!(create(PathBuf::from("d"), "x".into(), false).kind, OpKind::CreateFile { name: "x".into() });
    }
}
```

- [ ] **Step 2: Declarar módulo**

Modify `crates/ui/src/main.rs`: `mod ops_actions;` (orden alfabético: tras `mod logging;`/`mod main`... ubicar entre `mod logging;` y `mod panes;` → `ops_actions` va ahí; READ).

- [ ] **Step 3: Verificar + commit**

Run: `cargo test -p naygo-ui ops_actions` → 3 PASS.
Run: `cargo clippy -p naygo-ui --all-targets -- -D warnings`. NOTA: estas fns no se usan hasta Task 10 → `dead_code`. Si bloquea, `#[allow(dead_code)]` con comentario "consumido en Task 10", quitar luego.

```bash
git add crates/ui/src/ops_actions.rs crates/ui/src/main.rs
git commit -m "feat(ui): armar OpRequest desde disparadores (ops_actions, puro)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: `app.rs` — estado de ops + pump_ops + clipboard interno (sin UI de panel aún)

**Files:**
- Modify: `crates/ui/src/app.rs`

Esta tarea agrega el ESTADO y el DRENADO de operaciones, más el clipboard interno, sin el panel ni los disparadores (esos en Tasks 10-11). Conecta `pump_ops` al loop.

- [ ] **Step 1: Tipos de estado + campos**

Modify `crates/ui/src/app.rs`:
a) `use`: `use naygo_core::ops::{self, OpMsg, OpProgress, OpRequest, OpSummary}; use naygo_core::cancel::CancellationToken; use std::sync::mpsc::Receiver;` (consolidar; `Receiver` quizá ya está).
b) Tras `PaneListing`, añadir:
```rust
/// Una operación de archivo en curso (o terminada, mostrándose en el panel).
pub struct ActiveOp {
    pub rx: Option<Receiver<OpMsg>>,
    pub token: CancellationToken,
    pub label: String,            // p. ej. "Copiar → D:\backup"
    pub progress: Option<OpProgress>,
    pub summary: Option<OpSummary>,
    pub started: bool,            // false = en cola, true = corriendo
}

/// Clipboard interno de Naygo (para Ctrl+C/X/V entre paneles).
#[derive(Default)]
pub struct InternalClipboard {
    pub paths: Vec<std::path::PathBuf>,
    pub cut: bool,
}
```
c) En `struct NaygoApp`: `active_ops: Vec<ActiveOp>,` y `clipboard: InternalClipboard,`.
d) En `new`, inicializar: `active_ops: Vec::new(), clipboard: InternalClipboard::default(),`.

- [ ] **Step 2: `pump_ops` + lanzar una op**

En `impl NaygoApp`:
```rust
    /// Lanza una operación: planifica, spawnea el worker, agrega al panel. En modo
    /// cola, si ya hay una corriendo, queda en espera (started=false).
    pub fn start_op(&mut self, req: OpRequest, label: String) {
        // Papelera: caso especial atómico vía platform (no pasa por el motor core).
        if let naygo_core::ops::OpKind::Delete { to_trash: true } = &req.kind {
            let _ = naygo_platform::trash::move_to_trash(&req.sources);
            // Refrescar el panel activo tras borrar.
            if let Some(id) = self.workspace.active_id() {
                if let Some(dir) = self.workspace.active_files().map(|f| f.current_dir.clone()) {
                    self.refresh_pane(id, dir);
                }
            }
            return;
        }
        let plan = match naygo_core::ops::plan(&req) {
            Ok(p) => p,
            Err(_e) => { return; } // plan inválido: en una versión futura mostrar error
        };
        let token = CancellationToken::new();
        let (_ctx, crx) = std::sync::mpsc::channel();
        let queue_mode = self.settings.ops_mode == naygo_core::config::OpsMode::Queue;
        let any_running = self.active_ops.iter().any(|o| o.started && o.rx.is_some());
        let start_now = !queue_mode || !any_running;
        let rx = if start_now {
            let (rx, _h) = naygo_core::ops::spawn(plan, req.kind.clone(), token.clone(), crx);
            Some(rx)
        } else {
            // En cola: guardamos el request para lanzarlo cuando toque (simplificación:
            // re-planificar al iniciar). Para ops-A, encolar = guardar y lanzar en
            // pump_ops cuando no haya ninguna corriendo. Guardar el plan+kind:
            // (implementación: ver NOTA — usar un Vec de pendientes).
            None
        };
        self.active_ops.push(ActiveOp {
            rx, token, label, progress: None, summary: None, started: start_now,
        });
        // NOTA: el encolado real (lanzar el siguiente al terminar uno) se maneja en
        // pump_ops re-spawneando los pendientes. Para mantener este Task acotado, si
        // start_now=false guardamos también el (plan, kind) — añadir esos campos a
        // ActiveOp (pending_plan: Option<(OpPlan, OpKind)>) y lanzarlos en pump_ops.
    }

    /// Drena los canales de las ops activas; gestiona la cola.
    pub fn pump_ops(&mut self) {
        for op in &mut self.active_ops {
            if let Some(rx) = &op.rx {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        OpMsg::Progress(p) => op.progress = Some(p),
                        OpMsg::Done(s) | OpMsg::Cancelled(s) => {
                            op.summary = Some(s);
                            op.rx = None;
                        }
                        OpMsg::Failed(_) => { op.rx = None; }
                        OpMsg::Conflict(_) => {} // ops-A resuelve conflicto antes de spawn
                    }
                }
            }
        }
        // Cola: si no hay ninguna corriendo, arrancar la primera pendiente.
        // (Ver NOTA de start_op sobre pending_plan.)
        // Limpiar ops terminadas que ya no tienen summary que mostrar es decisión del
        // panel (Task 10): aquí solo drenamos.
    }

    pub fn any_op_active(&self) -> bool {
        self.active_ops.iter().any(|o| o.rx.is_some())
    }
```

NOTA al implementador: el encolado completo (lanzar el siguiente pendiente al terminar el actual) requiere guardar el `(OpPlan, OpKind)` de los pendientes en `ActiveOp` (campo `pending: Option<(OpPlan, OpKind)>`) y, en `pump_ops`, cuando `!any running`, tomar el primer pendiente, spawnear y marcar `started=true`. Implementar ese campo y la lógica. Es la pieza que falta arriba (marcada). Mantenerlo simple: un solo worker a la vez en modo Queue.

- [ ] **Step 3: Conectar al loop**

En `impl eframe::App for NaygoApp`, `logic()`: añadir `self.pump_ops();` tras `self.pump_tree();` y en el repaint: `if self.any_listing_active() || self.any_tree_listing_active() || self.any_op_active() { ctx.request_repaint(); }`.

- [ ] **Step 4: Verificar**

Run: `cargo build -p naygo-ui` → compila.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio. (`start_op`/`clipboard` sin uso aún → allow temporal si hace falta; Task 10-11 los consumen.)
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/app.rs
git commit -m "feat(ui): estado de operaciones + pump_ops + clipboard interno

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: `ui::ops_panel` + `ops_dialogs` + disparadores (integración)

**Files:**
- Create: `crates/ui/src/ops_panel.rs`, `crates/ui/src/ops_dialogs.rs`
- Modify: `crates/ui/src/app.rs`, `crates/ui/src/input.rs`, `crates/ui/src/toolbar.rs`, `crates/ui/src/panes/file_panel.rs`, `crates/ui/src/main.rs`

Tarea de integración grande. Produce ops-A funcional. Verificar egui 0.34.3 (modales `egui::Window`/`Modal`, popup de menú contextual, progress bar) contra fuente.

- [ ] **Step 1: `ops_panel.rs` — pintar el panel**

Create `crates/ui/src/ops_panel.rs` con `pub fn show(ui: &mut egui::Ui, active_ops: &[crate::app::ActiveOp], i18n, expanded: &mut bool) -> Vec<usize>` que pinta el panel (compacto/expandido), barra de progreso por op (`egui::ProgressBar::new(fraction)`), velocidad/bytes formateados (reusar `human_size`), botón ✕ por op (devuelve índices a cancelar), y el resumen. VERIFICAR `egui::ProgressBar` y `egui::Panel::bottom`. El panel se pinta en `app.rs::ui()` como un `egui::Panel::bottom("ops_panel")` cuando `!active_ops.is_empty()` o display=AlwaysVisible.

- [ ] **Step 2: `ops_dialogs.rs` — modales**

Create `crates/ui/src/ops_dialogs.rs`: `confirm_delete(ctx, i18n, count, permanent) -> Option<bool>` (modal con Eliminar/Cancelar; `Some(true)` confirmado) y `conflict(ctx, i18n, name) -> Option<ConflictChoice>` (Sobrescribir/Saltar/Renombrar + aplicar-a-todos). Usar `egui::Window::new(...).collapsible(false).resizable(false)` centrada, o `egui::Modal` si existe en 0.34 (VERIFICAR — 0.34 tiene `egui::Modal`). El estado del diálogo (abierto/respuesta) vive en `NaygoApp` (un enum `PendingDialog`).

- [ ] **Step 3: Disparadores — input.rs + app.rs**

- `input.rs`: añadir a `Action` las variantes de ops y mapear teclas en `handle_input` (Ctrl+C/X/V, Delete, Shift+Delete, F2, Ctrl+N, Ctrl+Shift+N, F5, F6). Verificar `i.modifiers.ctrl/shift` y `i.key_pressed(Key::...)`.
- `app.rs`: en `apply_action`, manejar las nuevas acciones: armar `OpRequest` vía `ops_actions`, resolver confirmación de borrado / conflicto (abriendo el diálogo y, al confirmar, llamar `start_op`). Copy/Cut → `self.clipboard`. Paste → `start_op(transfer(...))` desde el clipboard a la carpeta activa. F5/F6 → transfer al otro panel.
- Resolución de conflicto ANTES de spawn (decisión de diseño del spec): al pegar/copiar, chequear colisiones con `plan` + existencia; si hay y policy=Ask, abrir modal de conflicto; aplicar la decisión re-armando los pasos (Overwrite/Skip/Rename vía `dedup_name`) y entonces `start_op`.

- [ ] **Step 4: Toolbar + menú contextual**

- `toolbar.rs`: añadir botones (copiar/cortar/pegar/eliminar/renombrar/nuevo) que disparan las mismas acciones.
- `file_panel.rs`: clic derecho sobre una fila de archivo (`row.response().secondary_clicked()` o un `Popup`/`context_menu`) abre un menú con las ops (usar `response.context_menu(|ui|{...})` de egui — VERIFICAR en 0.34). Las ops elegidas se acumulan como una acción a procesar en app.rs.

- [ ] **Step 5: Pintar el panel + diálogos en app.rs::ui()**

Tras el DockArea, pintar `egui::Panel::bottom("ops_panel")` si corresponde (llamando `ops_panel::show`), procesar los índices a cancelar (`token.cancel()`), y pintar el diálogo pendiente (`ops_dialogs`).

- [ ] **Step 6: Quitar allows + verificar**

Quitar `#[allow(dead_code)]` de ops_actions/start_op/clipboard.
Run: `cargo build -p naygo-ui` → compila. `cargo clippy --workspace --all-targets -- -D warnings` → limpio. `cargo test --workspace` → verde. `cargo fmt`.

App-start (`--bin naygo`): seleccionar archivos → Ctrl+C, ir a otra carpeta → Ctrl+V copia (con panel de progreso); F5 copia al otro panel; Supr manda a papelera; Shift+Supr confirma y borra permanente; F2 renombra; Ctrl+N/Ctrl+Shift+N crean; clic derecho abre menú; el panel de ops muestra velocidad/bytes/barra animada/cola; conflicto abre modal; resumen al terminar.

- [ ] **Step 7: Commit**

```bash
git add crates/ui/src/ops_panel.rs crates/ui/src/ops_dialogs.rs crates/ui/src/app.rs crates/ui/src/input.rs crates/ui/src/toolbar.rs crates/ui/src/panes/file_panel.rs crates/ui/src/main.rs
git commit -m "feat(ui): panel de operaciones + diálogos + disparadores (ops-A funcional)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: Resumen final + exportar

**Files:**
- Modify: `crates/ui/src/ops_panel.rs`, `crates/ui/src/app.rs`

- [ ] **Step 1: Resumen + Ver detalle + Exportar**

En `ops_panel.rs`, cuando una op tiene `summary`, mostrar la línea de resumen (`ops.summary_done` con done/skipped/failed) + "Ver detalle" (expande la lista de items con su outcome) + "Exportar…". Exportar arma un texto (una línea por item: ruta + resultado) y lo escribe a un archivo — vía un worker corto (no en el hilo de UI) o, dado que es chico, aceptable síncrono con un file dialog simple (para ops-A, escribir a `<carpeta actual>/naygo-ops-<timestamp>.txt` sin file picker nativo, ya que el picker nativo es platform/shell). Mostrar al usuario la ruta exportada en el status.

VERIFICAR: si se quiere un file picker nativo, eso es platform/shell (fase posterior); para ops-A, exportar a una ruta fija/derivada está bien. Implementar así y dejar el picker para después.

- [ ] **Step 2: Mostrar resumen según config**

Respetar `settings.show_op_summary`: si está off, la op terminada se quita del panel sin mostrar resumen.

- [ ] **Step 3: Verificar + commit**

Run: `cargo build -p naygo-ui` → compila. `cargo clippy --workspace --all-targets -- -D warnings` → limpio. `cargo test --workspace` → verde. `cargo fmt`.

```bash
git add crates/ui/src/ops_panel.rs crates/ui/src/app.rs
git commit -m "feat(ui): resumen final de operaciones + exportar a archivo

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 12: Sección de config de ops en Apariencia/Avanzado

**Files:**
- Modify: `crates/ui/src/settings_window/advanced.rs` (o panes.rs — donde encaje), `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: Controles de config**

En la sección de Configuración apropiada (Avanzado o una nueva "Operaciones"), añadir: modo cola/paralelo (`selectable_value` OpsMode), display panel/modal/siempre-visible (OpsDisplay), checkbox confirmar-papelera, checkbox mostrar-resumen. Añadir claves i18n para las etiquetas (`settings.ops.*`) en ES/EN.

- [ ] **Step 2: Verificar + commit**

Run: `cargo build -p naygo-ui` → compila. `cargo test -p naygo-core i18n` → parity. `cargo clippy --workspace --all-targets -- -D warnings` → limpio. `cargo fmt`.

```bash
git add crates/ui/src/settings_window/ crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): configuración de operaciones (modo, display, confirmaciones)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 13: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: README**

Modify `README.md` — bloque de estado:
```markdown
> **Estado:** Fase ops-A (operaciones de archivo) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-07-naygo-ops-a-design.md`](docs/superpowers/specs/2026-06-07-naygo-ops-a-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-07-naygo-ops-a.md`](docs/superpowers/plans/2026-06-07-naygo-ops-a.md).
> Bloque visual completo (layout, íconos, config/i18n, árbol, columnas Excel, temas).
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → verde (core: ops::names/plan/engine + config; platform: trash; ui: ops_actions).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo fmt --check` → limpio.
Run: `cargo build --release -p naygo-ui` → release compila.
App-start manual: repasar el checklist de la Tarea 10 Step 6 + resumen/exportar + config.

- [ ] **Step 3: Commit y push**

```bash
git add README.md
git commit -m "chore: actualizar estado del README (fase ops-A)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/ops-a
```

---

## Self-review (cobertura del spec)

| Requisito del spec ops-A | Tarea(s) |
|---|---|
| Modelo de tipos (OpKind/Request/Plan/Msg/Summary) | 1 |
| Validación de nombres + dedup conflicto | 2 |
| Planificación pura (expandir, carpeta-dentro-de-sí, validar) | 3 |
| Motor worker (copia por buffers, cancelable, summary) | 4 |
| Settings de ops | 5 |
| Papelera (Win32 IFileOperation) | 6 |
| i18n | 7 + 12 |
| OpRequest desde disparadores (puro) | 8 |
| Estado de ops + pump_ops + clipboard interno | 9 |
| Panel de operaciones (compacto/expandir, cola, velocidad/bytes/animación) | 10 |
| Diálogos confirmación + conflicto | 10 |
| Disparadores (teclado/toolbar/menú/F5-F6) | 10 |
| Resumen final + exportar | 11 |
| Config de ops en Settings UI | 12 |
| Cancelación universal | 4 (motor) + 9/10 (token desde UI) |
| Recuperación nivel simple (reintentar/omitir) | 11 (resumen lista pendientes/fallados) — reintentar/omitir botón en resumen |

**Notas de riesgo:**
- **Win32 IFileOperation (Task 6):** verificar nombres/firmas COM en `windows` 0.62 (FileOperation CLSID, IFileOperation, SHCreateItemFromParsingName, flags FOF_*). Es el punto de mayor incertidumbre de API; ajustar contra el registry y el compilador.
- **Conflicto resuelto ANTES de spawn (Tasks 4, 10):** decisión de diseño para ops-A (la UI re-arma pasos con la decisión; el motor ejecuta limpio). El `conflict_rx` queda en la firma sin usar (para ops-B). Documentado.
- **Encolado (Task 9):** un solo worker a la vez en modo Queue; guardar (plan, kind) de pendientes en ActiveOp y lanzar el siguiente en pump_ops al terminar. Implementar ese campo.
- **Reintentar/omitir (recuperación nivel simple):** el resumen lista fallados/omitidos; el botón "reintentar pendientes" re-arma un OpRequest con esos paths. Si el reviewer lo ve fuera de Task 11, moverlo a una sub-tarea; está dentro del alcance ops-A.
- **egui 0.34 (Tasks 10-11):** verificar `egui::Modal` (existe en 0.34) o usar `Window` centrada; `ProgressBar`; `response.context_menu`; `Panel::bottom`. Contra fuente.
- **Mover mismo-volumen vs cruzado (Task 4):** `fs::rename` falla entre volúmenes → fallback copiar+borrar (incluido en exec_copy_step).
- **Papelera fuera del motor (Tasks 4, 9):** to_trash=true lo hace la UI vía platform directo; el motor core solo permanente. Documentado.
```
