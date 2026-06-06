# Naygo — Fase 2A: Layout dinámico (plan de implementación)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convertir Naygo de un layout fijo de un panel a **paneles independientes componibles** (Files/Tree/Inspector) con historial de navegación atrás/adelante por panel (teclado + botones del mouse), plantillas de layout (built-in + propias, con recientes y favoritos), barra de íconos con posición configurable, y persistencia tolerante del workspace.

**Architecture:** Se añade a `naygo-core` un módulo `workspace` (lógica pura: `NavHistory`, `FilePaneState`, `Workspace`, `LayoutTemplate`/`TemplateStore`, y un `SerializableDockLayout` que **desacopla `core` de egui_dock**) y un módulo `config` (3 JSON portables, tolerantes a corrupción). La capa `ui` reemplaza el único `PaneState` por un `Workspace` de N paneles, traduce `SerializableDockLayout`↔`DockState<PaneId>`, mantiene **un worker de listing por panel `Files`** (N listados en paralelo, UI nunca bloquea), y añade `toolbar`/`templates_menu`. Todo lo pesado y testeable vive en `core`; la UI solo pinta y despacha.

**Tech Stack:** Rust, `naygo-core` (serde/serde_json), `eframe`/`egui` 0.34.3, `egui_dock` 0.19.1. Sin dependencias nuevas. Botones del mouse vía `egui::PointerButton::Extra1/Extra2`.

**Estado de partida (Fase 1, ya en `main`/rama base):**
- `naygo-core`: `cancel` (`CancellationToken`), `fs_model` (`Entry`, `EntryKind`, `SortSpec`, `SortKey`, `ViewMode`, `PaneState`), `sort` (`sort_entries`), `listing` (`spawn_listing`, `ListingMsg`).
- `naygo-ui`: `app.rs` con `UiState { pane: PaneState, listing_rx, listing_token, status, typeahead_buf }` y `NaygoApp { dock_state: DockState<PaneTab>, ui_state }`; `docking.rs` (`NaygoTabViewer`); `input.rs` (`Key`, `Action`, `map_key`); `panes/{file_panel,tree_panel,inspector_panel}`; `typeahead.rs`; `logging.rs`; `main.rs`.
- egui 0.34.3: `App::ui(&mut self, ui, frame)` (requerido) + `App::logic(&mut self, ctx, frame)` (no pinta); paneles/DockArea con `.show_inside(ui, ...)`; `ui.ctx()` para el contexto.

**Prerequisito:** toolchain Rust en PATH. En cada comando PowerShell, prepend `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. Nunca usar `2>&1` con cargo. Verificar `$LASTEXITCODE`.

**Alcance (qué entra / qué NO):**
- ENTRA: `core::workspace` (`NavHistory`, `FilePaneState`, `PaneNode`, `Workspace`, `PaneId`, `PanePurpose`, `SerializableDockLayout`, `LayoutTemplate`, `TemplateStore`), `core::config` (load/save tolerante de workspace/templates/settings), UI multi-panel (un worker por panel `Files`), `toolbar` (posición configurable, solo-ícono), `templates_menu` (combobox recientes/favoritos/built-in + guardar), navegación atrás/adelante por panel + `Alt+←/→` + botones del mouse, filtro `show_dirs` por panel, breadcrumb por panel, marca de panel activo, inspector sigue al activo.
- NO ENTRA (fases siguientes): íconos reales (siguen glifos `[D]`), i18n (texto hardcoded ES), temas/color sets, drag&drop SO, ops de archivo, tamaño de carpeta, menú contextual nativo, árbol expandible real, **filtro de texto por panel** (campo `text_filter` reservado, sin UI), UI completa de Configuración (sólo posición de barra en 2A), botones de toolbar con texto.

---

## Estructura de archivos

```
crates/core/src/
├── lib.rs                      # MODIFICAR: + re-exports de workspace y config
├── fs_model.rs                 # (sin cambios; FilePaneState lo reutiliza/extiende)
├── workspace/
│   ├── mod.rs                  # PaneId, PanePurpose, PaneNode, Workspace
│   ├── nav_history.rs          # NavHistory (puro, muy testeado)
│   ├── file_pane.rs            # FilePaneState (+ show_dirs, text_filter reservado, nav)
│   ├── layout.rs               # SerializableDockLayout (desacople de egui_dock)
│   └── template.rs             # LayoutTemplate, built-ins, TemplateStore (recientes/fav)
└── config/
    └── mod.rs                  # Settings, BarPosition; load/save de los 3 JSON

crates/ui/src/
├── app.rs                      # MODIFICAR: Workspace + DockState<PaneId>, N workers, config
├── docking.rs                  # MODIFICAR: TabViewer despacha por PaneId/PanePurpose
├── toolbar.rs                  # NUEVO: barra de íconos (posición configurable, solo-ícono)
├── templates_menu.rs           # NUEVO: combobox de plantillas
├── input.rs                    # MODIFICAR: + Alt+←/→, botones del mouse
├── main.rs                     # MODIFICAR: declarar mod toolbar; mod templates_menu;
└── panes/
    ├── file_panel.rs           # MODIFICAR: show_dirs, breadcrumb, marca activo, opera sobre FilePaneState
    ├── tree_panel.rs           # MODIFICAR: opera sobre el panel activo del workspace
    └── inspector_panel.rs      # MODIFICAR: refleja el panel Files activo
```

**Por qué así:** cada pieza de `core::workspace` es una unidad con una responsabilidad y test propio. `SerializableDockLayout` aísla `core` de egui_dock. La UI se parte en `toolbar`/`templates_menu`/`docking`/paneles, cada archivo enfocado. El plan construye primero todo `core` (puro, testeable) y luego conecta la UI, como en Fase 1.

---

## Task 1: `PaneId` y `PanePurpose`

**Files:**
- Create: `crates/core/src/workspace/mod.rs`
- Modify: `crates/core/src/lib.rs`
- Test: módulo `#[cfg(test)]` en `mod.rs`

- [ ] **Step 1: Crear el módulo con los tipos base y su test**

Create `crates/core/src/workspace/mod.rs`:

```rust
// Naygo — workspace: paneles independientes componibles (lógica pura).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Modelo del espacio de trabajo: una colección de paneles independientes
//! (archivos / árbol / inspector), cuál está activo, y la disposición. No depende
//! de egui ni de Windows: la UI traduce esto a egui_dock.

pub mod file_pane;
pub mod layout;
pub mod nav_history;
pub mod template;

use serde::{Deserialize, Serialize};

/// Identificador único y estable de un panel dentro del workspace.
/// Estable: no cambia aunque el panel se reordene en la UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u64);

/// Qué tipo de panel es.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanePurpose {
    /// Lista de archivos navegable.
    Files,
    /// Árbol de carpetas (esqueleto en Fase 2A).
    Tree,
    /// Inspector de metadatos del elemento enfocado en el panel activo.
    Inspector,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_id_es_comparable_y_ordenable() {
        assert_eq!(PaneId(1), PaneId(1));
        assert!(PaneId(1) < PaneId(2));
    }

    #[test]
    fn pane_purpose_round_trip_serde() {
        let json = serde_json::to_string(&PanePurpose::Files).unwrap();
        let back: PanePurpose = serde_json::from_str(&json).unwrap();
        assert_eq!(back, PanePurpose::Files);
    }
}
```

Modify `crates/core/src/lib.rs` — añadir tras la línea `pub mod sort;`:

```rust
pub mod config;
pub mod workspace;
```

NOTA: `config` se crea en la Tarea 8; hasta entonces, para que compile, NO añadas `pub mod config;` todavía — añádelo solo cuando crees `config/mod.rs` (Tarea 8). En esta tarea añade SOLO `pub mod workspace;`. El re-export de tipos concretos se hace a medida que existen.

Por ahora añade SOLO:
```rust
pub mod workspace;
```

- [ ] **Step 2: Correr los tests**

Run: `cargo test -p naygo-core workspace`
Expected: PASS (2 tests: `pane_id_es_comparable_y_ordenable`, `pane_purpose_round_trip_serde`).

NOTA: `mod.rs` declara `pub mod file_pane; pub mod layout; pub mod nav_history; pub mod template;` que aún no existen → NO compilará todavía. Para que ESTA tarea compile aislada, comenta esas 4 líneas `pub mod ...;` temporalmente y descoméntalas a medida que cada submódulo se cree en las tareas siguientes. Alternativa más limpia: crea los 4 archivos vacíos con solo su header en esta tarea. ELIGE la segunda: crea los 4 submódulos como stubs con header (contenido real en sus tareas):

Create `crates/core/src/workspace/nav_history.rs`:
```rust
// Naygo — historial de navegación atrás/adelante de un panel (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```
Create `crates/core/src/workspace/file_pane.rs`:
```rust
// Naygo — estado de un panel de archivos (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```
Create `crates/core/src/workspace/layout.rs`:
```rust
// Naygo — disposición serializable, desacoplada de egui_dock.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```
Create `crates/core/src/workspace/template.rs`:
```rust
// Naygo — plantillas de layout y store de recientes/favoritos.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```

Run de nuevo: `cargo test -p naygo-core workspace` → PASS (2 tests).

- [ ] **Step 3: Clippy y commit**

Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

```bash
git add crates/core/src/workspace/ crates/core/src/lib.rs
git commit -m "feat(core): tipos base del workspace (PaneId, PanePurpose)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `NavHistory` (historial atrás/adelante puro)

**Files:**
- Modify: `crates/core/src/workspace/nav_history.rs`
- Test: módulo `#[cfg(test)]` en el mismo archivo

- [ ] **Step 1: Escribir el módulo con la firma stub + tests (TDD)**

Replace `crates/core/src/workspace/nav_history.rs`:

```rust
// Naygo — historial de navegación atrás/adelante de un panel (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `NavHistory` es la pila de rutas visitadas de un panel, con un cursor. Modela
//! el atrás/adelante de un navegador: `push` a una ruta nueva trunca la rama de
//! "adelante". Puro y testeable; no toca disco.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Tope de profundidad: más allá, se descartan las entradas más viejas.
const MAX_DEPTH: usize = 256;

/// Historial de navegación de un panel: rutas visitadas + cursor a la actual.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NavHistory {
    /// Rutas visitadas, de la más vieja a la más nueva.
    stack: Vec<PathBuf>,
    /// Índice de la ruta "actual" dentro de `stack`. `None` si está vacío.
    cursor: Option<usize>,
}

impl NavHistory {
    /// Historial vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ruta actual (donde está parado el cursor), si hay alguna.
    pub fn current(&self) -> Option<&Path> {
        self.cursor.map(|i| self.stack[i].as_path())
    }

    /// Navega a una ruta nueva: la agrega tras la actual y trunca la rama de
    /// "adelante" (todo lo que estaba después del cursor). El cursor pasa a la nueva.
    pub fn push(&mut self, path: PathBuf) {
        // Truncar la rama de adelante.
        if let Some(i) = self.cursor {
            self.stack.truncate(i + 1);
        } else {
            self.stack.clear();
        }
        self.stack.push(path);
        self.cursor = Some(self.stack.len() - 1);

        // Respetar el tope de profundidad descartando las más viejas.
        if self.stack.len() > MAX_DEPTH {
            let overflow = self.stack.len() - MAX_DEPTH;
            self.stack.drain(0..overflow);
            self.cursor = Some(self.stack.len() - 1);
        }
    }

    /// `true` si hay a dónde ir atrás.
    pub fn can_back(&self) -> bool {
        matches!(self.cursor, Some(i) if i > 0)
    }

    /// `true` si hay a dónde ir adelante.
    pub fn can_forward(&self) -> bool {
        matches!(self.cursor, Some(i) if i + 1 < self.stack.len())
    }

    /// Mueve el cursor un paso atrás y devuelve la ruta nueva, o `None` si no se puede.
    pub fn back(&mut self) -> Option<&Path> {
        if self.can_back() {
            let i = self.cursor.unwrap() - 1;
            self.cursor = Some(i);
            Some(self.stack[i].as_path())
        } else {
            None
        }
    }

    /// Mueve el cursor un paso adelante y devuelve la ruta nueva, o `None`.
    pub fn forward(&mut self) -> Option<&Path> {
        if self.can_forward() {
            let i = self.cursor.unwrap() + 1;
            self.cursor = Some(i);
            Some(self.stack[i].as_path())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn historial_vacio_no_tiene_actual_ni_movimiento() {
        let mut h = NavHistory::new();
        assert!(h.current().is_none());
        assert!(!h.can_back());
        assert!(!h.can_forward());
        assert!(h.back().is_none());
        assert!(h.forward().is_none());
    }

    #[test]
    fn push_avanza_la_actual() {
        let mut h = NavHistory::new();
        h.push(p("C:/a"));
        h.push(p("C:/a/b"));
        assert_eq!(h.current(), Some(p("C:/a/b").as_path()));
        assert!(h.can_back());
        assert!(!h.can_forward());
    }

    #[test]
    fn back_y_forward_mueven_el_cursor() {
        let mut h = NavHistory::new();
        h.push(p("C:/a"));
        h.push(p("C:/a/b"));
        h.push(p("C:/a/b/c"));
        assert_eq!(h.back(), Some(p("C:/a/b").as_path()));
        assert_eq!(h.back(), Some(p("C:/a").as_path()));
        assert!(!h.can_back());
        assert_eq!(h.forward(), Some(p("C:/a/b").as_path()));
        assert_eq!(h.current(), Some(p("C:/a/b").as_path()));
    }

    #[test]
    fn push_trunca_la_rama_de_adelante() {
        let mut h = NavHistory::new();
        h.push(p("C:/a"));
        h.push(p("C:/a/b"));
        h.push(p("C:/a/b/c"));
        h.back(); // estamos en C:/a/b
        h.push(p("C:/a/b/x")); // navegar a algo nuevo trunca "c"
        assert_eq!(h.current(), Some(p("C:/a/b/x").as_path()));
        assert!(!h.can_forward(), "la rama de adelante (c) se truncó");
    }
}
```

- [ ] **Step 2: Correr los tests (la impl ya está completa — TDD con impl dada)**

Run: `cargo test -p naygo-core nav_history`
Expected: PASS (4 tests).

- [ ] **Step 3: Re-export en workspace/mod.rs**

Modify `crates/core/src/workspace/mod.rs` — tras los `pub mod`, añadir:
```rust
pub use nav_history::NavHistory;
```

Run: `cargo test -p naygo-core workspace` → PASS. `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/workspace/nav_history.rs crates/core/src/workspace/mod.rs
git commit -m "feat(core): NavHistory atrás/adelante puro y testeado

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `FilePaneState` (estado de un panel de archivos)

**Files:**
- Modify: `crates/core/src/workspace/file_pane.rs`
- Test: módulo `#[cfg(test)]` en el mismo archivo

- [ ] **Step 1: Escribir el módulo con tests (TDD, impl dada)**

Replace `crates/core/src/workspace/file_pane.rs`:

```rust
// Naygo — estado de un panel de archivos (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `FilePaneState` es el estado de un panel de archivos: dónde está parado, qué
//! lista, su historial de navegación, su filtro de carpetas. No toca disco: la UI
//! le inyecta las entradas (vía el motor de `listing`) y le pide navegar.

use crate::fs_model::{Entry, SortSpec, ViewMode};
use crate::workspace::nav_history::NavHistory;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Estado de un panel de archivos. Lo serializable se persiste; `entries` no
/// (se re-lista al abrir) y `history` tampoco (arranca limpio cada sesión).
#[derive(Clone, Debug)]
pub struct FilePaneState {
    pub current_dir: PathBuf,
    pub entries: Vec<Entry>,
    pub sort: SortSpec,
    pub view: ViewMode,
    pub focused: Option<usize>,
    pub selected: Vec<usize>,
    pub history: NavHistory,
    /// Si es `false`, el panel oculta las carpetas (muestra solo archivos).
    pub show_dirs: bool,
    /// RESERVADO para una fase futura (filtro de texto). Siempre `None` en 2A.
    pub text_filter: Option<String>,
}

/// Lo que se persiste de un panel de archivos (sin entries ni history).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilePanePersist {
    pub current_dir: PathBuf,
    pub sort: SortSpec,
    pub view: ViewMode,
    pub show_dirs: bool,
    pub text_filter: Option<String>,
}

impl FilePaneState {
    /// Crea un panel parado en `dir`, con su historial ya apuntando a `dir`.
    pub fn new(dir: PathBuf) -> Self {
        let mut history = NavHistory::new();
        history.push(dir.clone());
        FilePaneState {
            current_dir: dir,
            entries: Vec::new(),
            sort: SortSpec::default(),
            view: ViewMode::default(),
            focused: None,
            selected: Vec::new(),
            history,
            show_dirs: true,
            text_filter: None,
        }
    }

    /// Entrada con foco, si existe.
    pub fn focused_entry(&self) -> Option<&Entry> {
        self.focused.and_then(|i| self.entries.get(i))
    }

    /// Navega a una carpeta nueva: registra en el historial y limpia entries/foco.
    /// (La UI lanzará el listado de `dir` tras llamar esto.)
    pub fn navigate_to(&mut self, dir: PathBuf) {
        self.history.push(dir.clone());
        self.enter(dir);
    }

    /// Va atrás en el historial. Devuelve la nueva carpeta si se movió.
    pub fn go_back(&mut self) -> Option<PathBuf> {
        let path = self.history.back().map(Path::to_path_buf)?;
        self.enter(path.clone());
        Some(path)
    }

    /// Va adelante en el historial. Devuelve la nueva carpeta si se movió.
    pub fn go_forward(&mut self) -> Option<PathBuf> {
        let path = self.history.forward().map(Path::to_path_buf)?;
        self.enter(path.clone());
        Some(path)
    }

    /// Sube al directorio padre (entra al historial). Devuelve el padre si existe.
    pub fn go_up(&mut self) -> Option<PathBuf> {
        let parent = self.current_dir.parent()?.to_path_buf();
        self.navigate_to(parent.clone());
        Some(parent)
    }

    /// Reemplaza la carpeta actual sin tocar el historial (uso interno).
    fn enter(&mut self, dir: PathBuf) {
        self.current_dir = dir;
        self.entries.clear();
        self.focused = None;
        self.selected.clear();
    }

    /// Estado persistible (sin entries ni history).
    pub fn to_persist(&self) -> FilePanePersist {
        FilePanePersist {
            current_dir: self.current_dir.clone(),
            sort: self.sort,
            view: self.view,
            show_dirs: self.show_dirs,
            text_filter: self.text_filter.clone(),
        }
    }

    /// Reconstruye desde lo persistido (historial nuevo apuntando a la carpeta).
    pub fn from_persist(p: FilePanePersist) -> Self {
        let mut s = FilePaneState::new(p.current_dir);
        s.sort = p.sort;
        s.view = p.view;
        s.show_dirs = p.show_dirs;
        s.text_filter = p.text_filter;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn nuevo_apunta_su_historial_a_la_carpeta() {
        let s = FilePaneState::new(p("C:/a"));
        assert_eq!(s.current_dir, p("C:/a"));
        assert_eq!(s.history.current(), Some(p("C:/a").as_path()));
        assert!(s.show_dirs);
        assert!(s.text_filter.is_none());
    }

    #[test]
    fn navigate_y_back_actualizan_carpeta_e_historial() {
        let mut s = FilePaneState::new(p("C:/a"));
        s.navigate_to(p("C:/a/b"));
        assert_eq!(s.current_dir, p("C:/a/b"));
        let back = s.go_back();
        assert_eq!(back, Some(p("C:/a")));
        assert_eq!(s.current_dir, p("C:/a"));
        let fwd = s.go_forward();
        assert_eq!(fwd, Some(p("C:/a/b")));
    }

    #[test]
    fn navegar_limpia_entries_y_foco() {
        let mut s = FilePaneState::new(p("C:/a"));
        s.focused = Some(3);
        s.selected = vec![1, 2];
        s.navigate_to(p("C:/a/b"));
        assert!(s.entries.is_empty());
        assert!(s.focused.is_none());
        assert!(s.selected.is_empty());
    }

    #[test]
    fn persist_round_trip_conserva_lo_serializable() {
        let mut s = FilePaneState::new(p("C:/a"));
        s.show_dirs = false;
        let restored = FilePaneState::from_persist(s.to_persist());
        assert_eq!(restored.current_dir, p("C:/a"));
        assert!(!restored.show_dirs);
        // El historial se reinicia apuntando a la carpeta.
        assert_eq!(restored.history.current(), Some(p("C:/a").as_path()));
    }
}
```

- [ ] **Step 2: Correr los tests**

Run: `cargo test -p naygo-core file_pane`
Expected: PASS (4 tests).

- [ ] **Step 3: Re-export y verificación**

Modify `crates/core/src/workspace/mod.rs` — añadir:
```rust
pub use file_pane::{FilePanePersist, FilePaneState};
```

Run: `cargo test -p naygo-core workspace` → PASS. `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/workspace/file_pane.rs crates/core/src/workspace/mod.rs
git commit -m "feat(core): FilePaneState con navegación, show_dirs y persistencia

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `SerializableDockLayout` (desacople de egui_dock)

**Files:**
- Modify: `crates/core/src/workspace/layout.rs`
- Test: módulo `#[cfg(test)]` en el mismo archivo

- [ ] **Step 1: Escribir el módulo con tests**

Replace `crates/core/src/workspace/layout.rs`:

```rust
// Naygo — disposición serializable, desacoplada de egui_dock.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `SerializableDockLayout` describe la disposición de paneles (un árbol binario
//! de splits) sin depender de egui_dock. La capa `ui` traduce esto a/desde el
//! `DockState` de egui_dock. Así `core` permanece testeable y la persistencia es
//! independiente del formato interno de la librería de docking.

use crate::workspace::PaneId;
use serde::{Deserialize, Serialize};

/// Orientación de un split.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDir {
    /// Hijos uno al lado del otro (izquierda | derecha).
    Horizontal,
    /// Hijos uno sobre otro (arriba / abajo).
    Vertical,
}

/// Un nodo del árbol de disposición: o una hoja (un panel) o un split de dos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DockNode {
    /// Una hoja: el panel con este id ocupa el espacio.
    Leaf(PaneId),
    /// Un split: `fraction` es la proporción [0,1] que toma el primer hijo.
    Split {
        dir: SplitDir,
        fraction: f32,
        first: Box<DockNode>,
        second: Box<DockNode>,
    },
}

/// La disposición completa: el árbol raíz (o vacío si no hay paneles).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct SerializableDockLayout {
    pub root: Option<DockNode>,
}

impl SerializableDockLayout {
    /// Disposición vacía (sin paneles).
    pub fn empty() -> Self {
        Self { root: None }
    }

    /// Disposición de un solo panel.
    pub fn single(id: PaneId) -> Self {
        Self {
            root: Some(DockNode::Leaf(id)),
        }
    }

    /// Recolecta todos los `PaneId` presentes en la disposición (orden de árbol).
    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        if let Some(node) = &self.root {
            collect(node, &mut out);
        }
        out
    }
}

fn collect(node: &DockNode, out: &mut Vec<PaneId>) {
    match node {
        DockNode::Leaf(id) => out.push(*id),
        DockNode::Split { first, second, .. } => {
            collect(first, out);
            collect(second, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vacio_no_tiene_paneles() {
        assert!(SerializableDockLayout::empty().pane_ids().is_empty());
    }

    #[test]
    fn single_tiene_un_panel() {
        let l = SerializableDockLayout::single(PaneId(7));
        assert_eq!(l.pane_ids(), vec![PaneId(7)]);
    }

    #[test]
    fn split_recolecta_en_orden() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.3,
                first: Box::new(DockNode::Leaf(PaneId(1))),
                second: Box::new(DockNode::Split {
                    dir: SplitDir::Horizontal,
                    fraction: 0.5,
                    first: Box::new(DockNode::Leaf(PaneId(2))),
                    second: Box::new(DockNode::Leaf(PaneId(3))),
                }),
            }),
        };
        assert_eq!(l.pane_ids(), vec![PaneId(1), PaneId(2), PaneId(3)]);
    }

    #[test]
    fn round_trip_serde() {
        let l = SerializableDockLayout::single(PaneId(42));
        let json = serde_json::to_string(&l).unwrap();
        let back: SerializableDockLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }
}
```

- [ ] **Step 2: Correr los tests**

Run: `cargo test -p naygo-core layout`
Expected: PASS (4 tests).

- [ ] **Step 3: Re-export y verificación**

Modify `crates/core/src/workspace/mod.rs` — añadir:
```rust
pub use layout::{DockNode, SerializableDockLayout, SplitDir};
```

Run: `cargo test -p naygo-core workspace` → PASS. `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/workspace/layout.rs crates/core/src/workspace/mod.rs
git commit -m "feat(core): SerializableDockLayout desacopla la disposición de egui_dock

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: `Workspace` y `PaneNode`

**Files:**
- Modify: `crates/core/src/workspace/mod.rs`
- Test: ampliar el módulo `#[cfg(test)]` de `mod.rs`

- [ ] **Step 1: Añadir `PaneNode`, `Workspace` y sus tests**

Modify `crates/core/src/workspace/mod.rs` — tras los `pub use`, añadir el cuerpo (antes del `#[cfg(test)]`):

```rust
use crate::workspace::file_pane::FilePaneState;
use crate::workspace::layout::SerializableDockLayout;

/// Un panel concreto del workspace. Solo los `Files` llevan `FilePaneState`.
#[derive(Clone, Debug)]
pub struct PaneNode {
    pub id: PaneId,
    pub purpose: PanePurpose,
    /// Estado del panel de archivos; `None` para Tree/Inspector.
    pub files: Option<FilePaneState>,
}

/// El espacio de trabajo: paneles + cuál está activo + la disposición.
#[derive(Clone, Debug)]
pub struct Workspace {
    panes: Vec<PaneNode>,
    active: Option<PaneId>,
    next_id: u64,
    /// Disposición visual (traducida a/desde egui_dock por la capa ui).
    pub layout: SerializableDockLayout,
}

impl Workspace {
    /// Workspace vacío.
    pub fn new() -> Self {
        Workspace {
            panes: Vec::new(),
            active: None,
            next_id: 0,
            layout: SerializableDockLayout::empty(),
        }
    }

    /// Agrega un panel del tipo dado y devuelve su id. Si es el primer panel,
    /// queda activo. Para `Files`, crea su `FilePaneState` parado en `dir`
    /// (ignorado para Tree/Inspector).
    pub fn add_pane(&mut self, purpose: PanePurpose, dir: std::path::PathBuf) -> PaneId {
        let id = PaneId(self.next_id);
        self.next_id += 1;
        let files = match purpose {
            PanePurpose::Files => Some(FilePaneState::new(dir)),
            _ => None,
        };
        self.panes.push(PaneNode { id, purpose, files });
        if self.active.is_none() {
            self.active = Some(id);
        }
        id
    }

    /// Quita el panel `id`. Si era el activo, reasigna el activo al primer panel
    /// `Files` restante (o a cualquier panel, o `None` si no queda ninguno).
    pub fn remove_pane(&mut self, id: PaneId) {
        self.panes.retain(|p| p.id != id);
        if self.active == Some(id) {
            self.active = self
                .panes
                .iter()
                .find(|p| p.purpose == PanePurpose::Files)
                .or_else(|| self.panes.first())
                .map(|p| p.id);
        }
    }

    /// El id del panel activo, si hay alguno.
    pub fn active_id(&self) -> Option<PaneId> {
        self.active
    }

    /// Marca `id` como activo si existe.
    pub fn set_active(&mut self, id: PaneId) {
        if self.panes.iter().any(|p| p.id == id) {
            self.active = Some(id);
        }
    }

    /// Referencia a un panel por id.
    pub fn pane(&self, id: PaneId) -> Option<&PaneNode> {
        self.panes.iter().find(|p| p.id == id)
    }

    /// Referencia mutable a un panel por id.
    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut PaneNode> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    /// El `FilePaneState` del panel `Files` activo (lo que refleja el inspector).
    /// Si el activo no es `Files`, devuelve el primer `Files` que haya.
    pub fn active_files(&self) -> Option<&FilePaneState> {
        self.active
            .and_then(|id| self.pane(id))
            .filter(|p| p.purpose == PanePurpose::Files)
            .and_then(|p| p.files.as_ref())
            .or_else(|| {
                self.panes
                    .iter()
                    .find(|p| p.purpose == PanePurpose::Files)
                    .and_then(|p| p.files.as_ref())
            })
    }

    /// Versión mutable de `active_files`.
    pub fn active_files_mut(&mut self) -> Option<&mut FilePaneState> {
        let target = self
            .active
            .filter(|id| {
                self.pane(*id)
                    .map(|p| p.purpose == PanePurpose::Files)
                    .unwrap_or(false)
            })
            .or_else(|| {
                self.panes
                    .iter()
                    .find(|p| p.purpose == PanePurpose::Files)
                    .map(|p| p.id)
            })?;
        self.pane_mut(target).and_then(|p| p.files.as_mut())
    }

    /// Itera los paneles (orden de inserción).
    pub fn panes(&self) -> &[PaneNode] {
        &self.panes
    }

    /// Itera los paneles mutables.
    pub fn panes_mut(&mut self) -> &mut [PaneNode] {
        &mut self.panes
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}
```

Ampliar el bloque `#[cfg(test)]` de `mod.rs` con:

```rust
    use std::path::PathBuf;

    #[test]
    fn primer_panel_queda_activo() {
        let mut w = Workspace::new();
        let id = w.add_pane(PanePurpose::Files, PathBuf::from("C:/"));
        assert_eq!(w.active_id(), Some(id));
    }

    #[test]
    fn quitar_el_activo_reasigna_a_otro_files() {
        let mut w = Workspace::new();
        let a = w.add_pane(PanePurpose::Files, PathBuf::from("C:/a"));
        let b = w.add_pane(PanePurpose::Files, PathBuf::from("C:/b"));
        w.set_active(a);
        w.remove_pane(a);
        assert_eq!(w.active_id(), Some(b));
    }

    #[test]
    fn active_files_apunta_al_panel_files_activo() {
        let mut w = Workspace::new();
        let _tree = w.add_pane(PanePurpose::Tree, PathBuf::new());
        let files = w.add_pane(PanePurpose::Files, PathBuf::from("C:/x"));
        w.set_active(files);
        assert_eq!(
            w.active_files().map(|f| f.current_dir.clone()),
            Some(PathBuf::from("C:/x"))
        );
    }

    #[test]
    fn tree_no_tiene_file_pane_state() {
        let mut w = Workspace::new();
        let t = w.add_pane(PanePurpose::Tree, PathBuf::new());
        assert!(w.pane(t).unwrap().files.is_none());
    }
```

- [ ] **Step 2: Correr los tests + re-export**

Modify `crates/core/src/workspace/mod.rs` — asegúrate de que arriba haya:
```rust
pub use file_pane::{FilePanePersist, FilePaneState};
pub use layout::{DockNode, SerializableDockLayout, SplitDir};
pub use nav_history::NavHistory;
```
(`PaneNode` y `Workspace` ya son públicos por estar definidos en `mod.rs`.)

Run: `cargo test -p naygo-core workspace`
Expected: PASS (los 2 base + 4 nuevos = 6).

Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/workspace/mod.rs
git commit -m "feat(core): Workspace de N paneles (activo, add/remove, active_files)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: `LayoutTemplate` y built-ins

**Files:**
- Modify: `crates/core/src/workspace/template.rs`
- Test: módulo `#[cfg(test)]` en el mismo archivo

- [ ] **Step 1: Escribir las plantillas con tests**

Replace `crates/core/src/workspace/template.rs`:

```rust
// Naygo — plantillas de layout y store de recientes/favoritos.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Una `LayoutTemplate` describe una disposición nombrada (qué paneles y cómo se
//! reparten). Hay built-ins (código) y plantillas del usuario (persistidas). El
//! `TemplateStore` agrega los favoritos y la lista de recientes.

use crate::workspace::layout::{DockNode, SerializableDockLayout, SplitDir};
use crate::workspace::PanePurpose;
use serde::{Deserialize, Serialize};

/// De dónde arranca un panel `Files` de una plantilla.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateDir {
    /// El home del usuario.
    Home,
    /// Una ruta fija.
    Fixed(String),
}

/// Un panel descrito por una plantilla (tipo + carpeta inicial si es Files).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplatePane {
    pub purpose: PanePurpose,
    /// Solo relevante para `Files`.
    pub dir: TemplateDir,
}

/// Una disposición nombrada. Los built-in se construyen en código; los del usuario
/// se serializan en `templates.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutTemplate {
    pub name: String,
    pub builtin: bool,
    pub favorite: bool,
    /// Paneles que crea la plantilla, en orden.
    pub panes: Vec<TemplatePane>,
    /// Cómo se reparten visualmente (los índices de hoja referencian `panes`).
    pub layout: LayoutShape,
}

/// La forma del layout descrita por índices a `panes` (no por PaneId, porque la
/// plantilla es previa a la creación de los paneles). La UI/Workspace la
/// materializa creando los paneles y mapeando índice→PaneId.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LayoutShape {
    /// Hoja: el panel `panes[idx]`.
    Leaf(usize),
    Split {
        dir: SplitDir,
        fraction: f32,
        first: Box<LayoutShape>,
        second: Box<LayoutShape>,
    },
}

impl LayoutTemplate {
    /// Minimalista: un solo panel de archivos.
    pub fn minimalista() -> Self {
        LayoutTemplate {
            name: "Minimalista".into(),
            builtin: true,
            favorite: false,
            panes: vec![TemplatePane {
                purpose: PanePurpose::Files,
                dir: TemplateDir::Home,
            }],
            layout: LayoutShape::Leaf(0),
        }
    }

    /// Clásico: árbol | archivos | inspector.
    pub fn clasico() -> Self {
        LayoutTemplate {
            name: "Clásico".into(),
            builtin: true,
            favorite: false,
            panes: vec![
                TemplatePane { purpose: PanePurpose::Tree, dir: TemplateDir::Home },
                TemplatePane { purpose: PanePurpose::Files, dir: TemplateDir::Home },
                TemplatePane { purpose: PanePurpose::Inspector, dir: TemplateDir::Home },
            ],
            layout: LayoutShape::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.22,
                first: Box::new(LayoutShape::Leaf(0)),
                second: Box::new(LayoutShape::Split {
                    dir: SplitDir::Horizontal,
                    fraction: 0.74,
                    first: Box::new(LayoutShape::Leaf(1)),
                    second: Box::new(LayoutShape::Leaf(2)),
                }),
            },
        }
    }

    /// Dual-pane: árbol | archivos A | archivos B | inspector. (Default de la app.)
    pub fn dual_pane() -> Self {
        LayoutTemplate {
            name: "Dual-pane".into(),
            builtin: true,
            favorite: false,
            panes: vec![
                TemplatePane { purpose: PanePurpose::Tree, dir: TemplateDir::Home },
                TemplatePane { purpose: PanePurpose::Files, dir: TemplateDir::Home },
                TemplatePane { purpose: PanePurpose::Files, dir: TemplateDir::Home },
                TemplatePane { purpose: PanePurpose::Inspector, dir: TemplateDir::Home },
            ],
            layout: LayoutShape::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.18,
                first: Box::new(LayoutShape::Leaf(0)),
                second: Box::new(LayoutShape::Split {
                    dir: SplitDir::Horizontal,
                    fraction: 0.4,
                    first: Box::new(LayoutShape::Leaf(1)),
                    second: Box::new(LayoutShape::Split {
                        dir: SplitDir::Horizontal,
                        fraction: 0.66,
                        first: Box::new(LayoutShape::Leaf(2)),
                        second: Box::new(LayoutShape::Leaf(3)),
                    }),
                }),
            },
        }
    }

    /// Power-user: tres paneles de archivos lado a lado + inspector.
    pub fn power_user() -> Self {
        LayoutTemplate {
            name: "Power-user".into(),
            builtin: true,
            favorite: false,
            panes: vec![
                TemplatePane { purpose: PanePurpose::Files, dir: TemplateDir::Home },
                TemplatePane { purpose: PanePurpose::Files, dir: TemplateDir::Home },
                TemplatePane { purpose: PanePurpose::Files, dir: TemplateDir::Home },
                TemplatePane { purpose: PanePurpose::Inspector, dir: TemplateDir::Home },
            ],
            layout: LayoutShape::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.3,
                first: Box::new(LayoutShape::Leaf(0)),
                second: Box::new(LayoutShape::Split {
                    dir: SplitDir::Horizontal,
                    fraction: 0.43,
                    first: Box::new(LayoutShape::Leaf(1)),
                    second: Box::new(LayoutShape::Split {
                        dir: SplitDir::Horizontal,
                        fraction: 0.6,
                        first: Box::new(LayoutShape::Leaf(2)),
                        second: Box::new(LayoutShape::Leaf(3)),
                    }),
                }),
            },
        }
    }

    /// Todas las plantillas built-in, en el orden en que se muestran.
    pub fn builtins() -> Vec<LayoutTemplate> {
        vec![
            Self::minimalista(),
            Self::clasico(),
            Self::dual_pane(),
            Self::power_user(),
        ]
    }
}

/// Un uso reciente de una plantilla (nombre + timestamp inyectado por la UI).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecentUse {
    pub name: String,
    /// Segundos epoch; lo inyecta la capa ui (core no llama a `SystemTime::now`).
    pub at: u64,
}

/// Tope de entradas en la lista de recientes.
const MAX_RECENTS: usize = 8;

/// Plantillas del usuario + recientes. Lo que se persiste en `templates.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TemplateStore {
    /// Plantillas creadas por el usuario (builtin = false).
    pub user: Vec<LayoutTemplate>,
    /// Usos recientes, del más nuevo al más viejo.
    pub recents: Vec<RecentUse>,
}

impl TemplateStore {
    /// Registra el uso de una plantilla: la pone al frente de recientes (sin
    /// duplicar) y respeta el tope. `at` es el timestamp inyectado por la ui.
    pub fn record_use(&mut self, name: &str, at: u64) {
        self.recents.retain(|r| r.name != name);
        self.recents.insert(0, RecentUse { name: name.to_string(), at });
        self.recents.truncate(MAX_RECENTS);
    }

    /// Marca/desmarca como favorita una plantilla del usuario por nombre.
    pub fn set_favorite(&mut self, name: &str, favorite: bool) {
        if let Some(t) = self.user.iter_mut().find(|t| t.name == name) {
            t.favorite = favorite;
        }
    }

    /// Agrega una plantilla del usuario (la fuerza a builtin=false).
    pub fn add_user(&mut self, mut t: LayoutTemplate) {
        t.builtin = false;
        // Si ya existe una con el mismo nombre, la reemplaza.
        self.user.retain(|x| x.name != t.name);
        self.user.push(t);
    }

    /// Borra una plantilla del usuario por nombre.
    pub fn remove_user(&mut self, name: &str) {
        self.user.retain(|t| t.name != name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_tienen_las_cuatro() {
        let names: Vec<_> = LayoutTemplate::builtins()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, vec!["Minimalista", "Clásico", "Dual-pane", "Power-user"]);
    }

    #[test]
    fn minimalista_es_un_solo_files() {
        let t = LayoutTemplate::minimalista();
        assert_eq!(t.panes.len(), 1);
        assert_eq!(t.panes[0].purpose, PanePurpose::Files);
        assert_eq!(t.layout, LayoutShape::Leaf(0));
    }

    #[test]
    fn record_use_pone_al_frente_sin_duplicar() {
        let mut s = TemplateStore::default();
        s.record_use("Dual-pane", 100);
        s.record_use("Power-user", 200);
        s.record_use("Dual-pane", 300); // re-uso: sube al frente, no duplica
        assert_eq!(s.recents.len(), 2);
        assert_eq!(s.recents[0].name, "Dual-pane");
        assert_eq!(s.recents[0].at, 300);
    }

    #[test]
    fn recientes_respeta_el_tope() {
        let mut s = TemplateStore::default();
        for i in 0..20 {
            s.record_use(&format!("t{i}"), i as u64);
        }
        assert_eq!(s.recents.len(), MAX_RECENTS);
        assert_eq!(s.recents[0].name, "t19");
    }

    #[test]
    fn add_user_fuerza_no_builtin_y_reemplaza_por_nombre() {
        let mut s = TemplateStore::default();
        let mut t = LayoutTemplate::minimalista();
        t.name = "Mía".into();
        t.builtin = true; // debe forzarse a false
        s.add_user(t.clone());
        s.add_user(t); // mismo nombre: reemplaza, no duplica
        assert_eq!(s.user.len(), 1);
        assert!(!s.user[0].builtin);
    }

    #[test]
    fn set_favorite_marca_la_del_usuario() {
        let mut s = TemplateStore::default();
        let mut t = LayoutTemplate::minimalista();
        t.name = "Mía".into();
        s.add_user(t);
        s.set_favorite("Mía", true);
        assert!(s.user[0].favorite);
    }
}
```

- [ ] **Step 2: Correr los tests**

Run: `cargo test -p naygo-core template`
Expected: PASS (6 tests).

- [ ] **Step 3: Re-export y verificación**

Modify `crates/core/src/workspace/mod.rs` — añadir:
```rust
pub use template::{LayoutShape, LayoutTemplate, RecentUse, TemplateDir, TemplatePane, TemplateStore};
```

Run: `cargo test -p naygo-core workspace` → PASS. `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/workspace/template.rs crates/core/src/workspace/mod.rs
git commit -m "feat(core): LayoutTemplate (4 built-ins) + TemplateStore (recientes/favoritos)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Materializar una plantilla en un `Workspace`

Convierte una `LayoutTemplate` (índices) en paneles reales de un `Workspace` (con `PaneId`) + su `SerializableDockLayout`. Es el puente entre "preset" y "estado vivo".

**Files:**
- Modify: `crates/core/src/workspace/mod.rs` (método `from_template`)
- Test: ampliar `#[cfg(test)]` de `mod.rs`

- [ ] **Step 1: Añadir `Workspace::from_template` y su test**

Modify `crates/core/src/workspace/mod.rs` — dentro de `impl Workspace`, añadir:

```rust
    /// Construye un workspace desde una plantilla: crea los paneles (mapeando
    /// índice de la plantilla → PaneId real) y arma la disposición. `home` es la
    /// carpeta para los `TemplateDir::Home`. El primer panel `Files` queda activo.
    pub fn from_template(
        tpl: &crate::workspace::template::LayoutTemplate,
        home: &std::path::Path,
    ) -> Self {
        use crate::workspace::template::{LayoutShape, TemplateDir};

        let mut w = Workspace::new();
        // Crear paneles, guardando el PaneId de cada índice.
        let mut ids: Vec<PaneId> = Vec::with_capacity(tpl.panes.len());
        for tp in &tpl.panes {
            let dir = match &tp.dir {
                TemplateDir::Home => home.to_path_buf(),
                TemplateDir::Fixed(s) => std::path::PathBuf::from(s),
            };
            ids.push(w.add_pane(tp.purpose, dir));
        }
        // Activar el primer Files.
        if let Some(first_files) = tpl
            .panes
            .iter()
            .position(|p| p.purpose == PanePurpose::Files)
        {
            w.active = Some(ids[first_files]);
        }
        // Traducir LayoutShape (índices) → SerializableDockLayout (PaneId).
        fn shape_to_node(shape: &LayoutShape, ids: &[PaneId]) -> crate::workspace::layout::DockNode {
            use crate::workspace::layout::DockNode;
            match shape {
                LayoutShape::Leaf(i) => DockNode::Leaf(ids[*i]),
                LayoutShape::Split { dir, fraction, first, second } => DockNode::Split {
                    dir: *dir,
                    fraction: *fraction,
                    first: Box::new(shape_to_node(first, ids)),
                    second: Box::new(shape_to_node(second, ids)),
                },
            }
        }
        w.layout = SerializableDockLayout {
            root: if tpl.panes.is_empty() {
                None
            } else {
                Some(shape_to_node(&tpl.layout, &ids))
            },
        };
        w
    }
```

Ampliar el `#[cfg(test)]` de `mod.rs`:

```rust
    #[test]
    fn from_template_minimalista_crea_un_files_activo() {
        let tpl = crate::workspace::template::LayoutTemplate::minimalista();
        let w = Workspace::from_template(&tpl, std::path::Path::new("C:/home"));
        assert_eq!(w.panes().len(), 1);
        assert_eq!(w.panes()[0].purpose, PanePurpose::Files);
        assert!(w.active_id().is_some());
        assert_eq!(
            w.active_files().map(|f| f.current_dir.clone()),
            Some(PathBuf::from("C:/home"))
        );
        // El layout tiene exactamente ese panel.
        assert_eq!(w.layout.pane_ids().len(), 1);
    }

    #[test]
    fn from_template_dual_pane_crea_cuatro_paneles() {
        let tpl = crate::workspace::template::LayoutTemplate::dual_pane();
        let w = Workspace::from_template(&tpl, std::path::Path::new("C:/home"));
        assert_eq!(w.panes().len(), 4);
        // 2 Files, 1 Tree, 1 Inspector.
        let files = w.panes().iter().filter(|p| p.purpose == PanePurpose::Files).count();
        assert_eq!(files, 2);
        // El layout referencia los 4 paneles.
        assert_eq!(w.layout.pane_ids().len(), 4);
        // El activo es un Files.
        let active = w.active_id().unwrap();
        assert_eq!(w.pane(active).unwrap().purpose, PanePurpose::Files);
    }
```

- [ ] **Step 2: Correr los tests**

Run: `cargo test -p naygo-core workspace`
Expected: PASS (6 previos + 2 nuevos = 8).

Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/workspace/mod.rs
git commit -m "feat(core): Workspace::from_template materializa un preset en paneles reales

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Módulo `config` (persistencia tolerante)

**Files:**
- Create: `crates/core/src/config/mod.rs`
- Modify: `crates/core/src/lib.rs`
- Test: módulo `#[cfg(test)]` en `config/mod.rs` (usa `tempfile`, ya es dev-dep)

- [ ] **Step 1: Escribir el módulo con tests**

Create `crates/core/src/config/mod.rs`:

```rust
// Naygo — persistencia portable del workspace/plantillas/ajustes (JSON).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Carga y guarda tres archivos JSON independientes junto al ejecutable
//! (portable). Tolerante: un archivo ausente, corrupto o de versión incompatible
//! NO crashea — se cae al default y se loguea. Cada archivo es independiente.

use crate::workspace::template::TemplateStore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Versión del formato de los archivos de config; permite migrar/descartar.
const CONFIG_VERSION: u32 = 1;

/// Dónde se ancla la barra de íconos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BarPosition {
    Top,
    Side,
}

/// Ajustes de la app (settings.json).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    pub bar_position: BarPosition,
    /// Botones de la barra solo con ícono (sin texto).
    pub icon_only: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: CONFIG_VERSION,
            bar_position: BarPosition::Top,
            icon_only: true,
        }
    }
}

/// Estado persistible de un panel dentro del workspace (un panel del layout).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspacePersist {
    pub version: u32,
    /// La disposición (árbol de splits con PaneId).
    pub layout: crate::workspace::layout::SerializableDockLayout,
    /// El id del panel activo.
    pub active: Option<crate::workspace::PaneId>,
    /// Estado persistible de cada panel Files, indexado por PaneId.
    pub files: Vec<(crate::workspace::PaneId, crate::workspace::file_pane::FilePanePersist)>,
    /// Tipo de cada panel del layout (para reconstruir Tree/Inspector también).
    pub purposes: Vec<(crate::workspace::PaneId, crate::workspace::PanePurpose)>,
}

/// Lee un archivo JSON y lo deserializa, devolviendo `None` si no existe o falla.
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<T>(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("config ilegible en {}: {e}", path.display());
            None
        }
    }
}

/// Escribe un valor como JSON (pretty). Loguea y traga el error (nunca crashea).
fn write_json<T: Serialize>(path: &Path, value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => {
            if let Err(e) = std::fs::write(path, text) {
                tracing::warn!("no se pudo guardar {}: {e}", path.display());
            }
        }
        Err(e) => tracing::warn!("no se pudo serializar {}: {e}", path.display()),
    }
}

/// Carga settings; si falta/corrupto/versión incompatible → default.
pub fn load_settings(dir: &Path) -> Settings {
    match read_json::<Settings>(&dir.join("settings.json")) {
        Some(s) if s.version == CONFIG_VERSION => s,
        Some(_) => {
            tracing::warn!("settings.json de versión incompatible; usando default");
            Settings::default()
        }
        None => Settings::default(),
    }
}

/// Guarda settings.
pub fn save_settings(dir: &Path, s: &Settings) {
    write_json(&dir.join("settings.json"), s);
}

/// Carga el store de plantillas; si falta/corrupto → vacío.
pub fn load_templates(dir: &Path) -> TemplateStore {
    read_json::<TemplateStore>(&dir.join("templates.json")).unwrap_or_default()
}

/// Guarda el store de plantillas.
pub fn save_templates(dir: &Path, store: &TemplateStore) {
    write_json(&dir.join("templates.json"), store);
}

/// Carga el workspace persistido; `None` si falta/corrupto/versión incompatible
/// (el llamador cae a la plantilla default).
pub fn load_workspace(dir: &Path) -> Option<WorkspacePersist> {
    match read_json::<WorkspacePersist>(&dir.join("workspace.json")) {
        Some(w) if w.version == CONFIG_VERSION => Some(w),
        Some(_) => {
            tracing::warn!("workspace.json de versión incompatible; ignorando");
            None
        }
        None => None,
    }
}

/// Guarda el workspace persistido.
pub fn save_workspace(dir: &Path, w: &WorkspacePersist) {
    write_json(&dir.join("workspace.json"), w);
}

/// Directorio de config portable: junto al ejecutable, o el cwd como fallback.
pub fn portable_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings {
            version: CONFIG_VERSION,
            bar_position: BarPosition::Side,
            icon_only: false,
        };
        save_settings(dir.path(), &s);
        assert_eq!(load_settings(dir.path()), s);
    }

    #[test]
    fn settings_ausente_da_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
    }

    #[test]
    fn settings_corrupto_da_default_sin_panic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.json"), b"{ no es json valido").unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
    }

    #[test]
    fn settings_version_incompatible_da_default() {
        let dir = tempfile::tempdir().unwrap();
        // version 999 ≠ CONFIG_VERSION.
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":999,"bar_position":"Top","icon_only":true}"#,
        )
        .unwrap();
        assert_eq!(load_settings(dir.path()), Settings::default());
    }

    #[test]
    fn templates_ausente_da_vacio() {
        let dir = tempfile::tempdir().unwrap();
        let store = load_templates(dir.path());
        assert!(store.user.is_empty());
        assert!(store.recents.is_empty());
    }

    #[test]
    fn workspace_ausente_da_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_workspace(dir.path()).is_none());
    }
}
```

Modify `crates/core/src/lib.rs` — añadir `pub mod config;` (junto a `pub mod workspace;`).

- [ ] **Step 2: Correr los tests**

Run: `cargo test -p naygo-core config`
Expected: PASS (6 tests).

Run: `cargo test -p naygo-core` → toda la suite de core verde.
Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/config/mod.rs crates/core/src/lib.rs
git commit -m "feat(core): módulo config (settings/templates/workspace) tolerante a corrupción

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Re-exports de `core` y cierre del núcleo

**Files:**
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Añadir re-exports de conveniencia**

Modify `crates/core/src/lib.rs` — tras los `pub use` existentes, añadir:

```rust
pub use config::{BarPosition, Settings};
pub use workspace::{
    FilePaneState, LayoutTemplate, NavHistory, PaneId, PaneNode, PanePurpose, TemplateStore,
    Workspace,
};
```

- [ ] **Step 2: Verificar todo el núcleo**

Run: `cargo test -p naygo-core`
Expected: PASS (todos: cancel, fs_model, sort, listing, nav_history, file_pane, layout, workspace, template, config).

Run: `cargo build --workspace` → compila (la UI sigue usando la API vieja; aún no la tocamos).
Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/lib.rs
git commit -m "feat(core): re-exports del workspace y config; núcleo de Fase 2A completo

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Traducción `SerializableDockLayout` ↔ `DockState<PaneId>` en la UI

Puente entre el layout puro de `core` y egui_dock. Vive en la UI (puede depender de egui_dock).

**Files:**
- Create: `crates/ui/src/dock_translate.rs`
- Modify: `crates/ui/src/main.rs` (declarar `mod dock_translate;`)
- Test: módulo `#[cfg(test)]` en `dock_translate.rs`

- [ ] **Step 1: Escribir la traducción con un test de ida**

Create `crates/ui/src/dock_translate.rs`:

```rust
// Naygo — traducción entre el layout puro de core y el DockState de egui_dock.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `core` describe la disposición con `SerializableDockLayout` (sin egui_dock).
//! Aquí la traducimos a un `DockState<PaneId>` de egui_dock para pintarla, y de
//! vuelta para persistir lo que el usuario reacomodó.

use egui_dock::{DockState, NodeIndex};
use naygo_core::workspace::layout::{DockNode, SerializableDockLayout, SplitDir};
use naygo_core::workspace::PaneId;

/// Construye un `DockState<PaneId>` a partir del layout puro. Si el layout está
/// vacío, devuelve un dock con un único tab "placeholder" inexistente evitado:
/// en ese caso usamos el primer pane del layout. Para 2A, el layout nunca llega
/// vacío (siempre hay al menos un panel), así que tomamos esa precondición.
pub fn to_dock_state(layout: &SerializableDockLayout) -> DockState<PaneId> {
    let ids = layout.pane_ids();
    // Precondición de 2A: hay al menos un panel.
    let first = ids.first().copied().unwrap_or(PaneId(0));
    let mut state = DockState::new(vec![first]);

    if let Some(DockNode::Split { .. }) = &layout.root {
        // Reconstruir recursivamente a partir del primer nodo, dividiendo.
        let surface = state.main_surface_mut();
        // Reemplazamos el contenido del root con el árbol real.
        if let Some(root) = &layout.root {
            // El root inicial tiene 'first' como única hoja; reconstruimos.
            build_into(surface, NodeIndex::root(), root);
        }
    }
    state
}

/// Inserta recursivamente el árbol `node` en el nodo `at` del surface. El nodo
/// `at` inicialmente contiene un tab placeholder que se sobrescribe.
fn build_into(
    surface: &mut egui_dock::Surface<PaneId>,
    _at: NodeIndex,
    _node: &DockNode,
) {
    // Implementación detallada: ver Step 3 (esta función se completa allí).
    let _ = surface;
}

/// Recolecta el orden actual de paneles del DockState (para persistir el layout
/// tras un reacomodo del usuario). En 2A persistimos el `SerializableDockLayout`
/// que ya tenemos en el Workspace; la lectura inversa fina del DockState se
/// puede afinar luego. Aquí devolvemos los ids en orden de iteración.
pub fn dock_pane_ids(state: &DockState<PaneId>) -> Vec<PaneId> {
    let mut out = Vec::new();
    for (_surf, node) in state.iter_all_tabs() {
        out.push(*node);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::workspace::layout::SerializableDockLayout;

    #[test]
    fn single_pane_produce_un_dock_con_ese_tab() {
        let layout = SerializableDockLayout::single(PaneId(5));
        let state = to_dock_state(&layout);
        let ids = dock_pane_ids(&state);
        assert_eq!(ids, vec![PaneId(5)]);
    }
}
```

> **NOTA AL IMPLEMENTADOR:** la API exacta de `egui_dock` 0.19.1 para construir un
> árbol arbitrario (split recursivo) puede requerir usar
> `surface.split(parent, fraction, vec![tab])` con la orientación adecuada, o
> `Tree::split_left/right/above/below`. ANTES de implementar `build_into`, lee la
> fuente en
> `C:\Users\ngrot\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\egui_dock-0.19.1\src\dock_state\tree\mod.rs`
> y usa los métodos reales. `SplitDir::Horizontal` → split izquierda/derecha;
> `SplitDir::Vertical` → split arriba/abajo. Si la API no permite reconstruir el
> árbol exacto fácilmente, una estrategia válida para 2A: construir el dock
> aplicando los splits del árbol en pre-orden (primer hijo a un lado, segundo al
> otro), aceptando que las fracciones se respeten lo mejor posible. Reporta la
> estrategia elegida.

- [ ] **Step 2: Declarar el módulo y ver el test mínimo pasar**

Modify `crates/ui/src/main.rs` — añadir `mod dock_translate;` junto a los otros `mod`.

Run: `cargo test -p naygo-ui dock_translate`
Expected: el test `single_pane_produce_un_dock_con_ese_tab` PASA (con la versión mínima; el split real se implementa en Step 3).

- [ ] **Step 3: Implementar `build_into` contra la API real de egui_dock 0.19.1**

Lee la fuente de egui_dock indicada y reemplaza `build_into` por una implementación que reconstruya el árbol. Enfoque recomendado (pre-orden con splits):

```rust
fn build_into(
    surface: &mut egui_dock::Surface<PaneId>,
    at: NodeIndex,
    node: &DockNode,
) {
    match node {
        DockNode::Leaf(_id) => {
            // El nodo `at` ya contiene este tab (lo pusimos como placeholder o
            // por el split previo); no hay que hacer nada.
        }
        DockNode::Split { dir, fraction, first, second } => {
            // El segundo hijo va al lado/abajo; el primero queda en `at`.
            let tree = surface; // Surface deref a Tree en egui_dock 0.19
            let second_ids = layout_leaf_ids(second);
            let [a, b] = match dir {
                SplitDir::Horizontal => tree.split_right(at, *fraction, second_ids),
                SplitDir::Vertical => tree.split_below(at, *fraction, second_ids),
            };
            // Recurse: el subárbol `first` queda en `a`, `second` en `b`.
            build_into(tree, a, first);
            build_into(tree, b, second);
        }
    }
}

/// Ids de las hojas de un subárbol, en orden.
fn layout_leaf_ids(node: &DockNode) -> Vec<PaneId> {
    let mut out = Vec::new();
    fn go(n: &DockNode, out: &mut Vec<PaneId>) {
        match n {
            DockNode::Leaf(id) => out.push(*id),
            DockNode::Split { first, second, .. } => {
                go(first, out);
                go(second, out);
            }
        }
    }
    go(node, &mut out);
    out
}
```

> Ajusta `split_right`/`split_below` a los nombres reales de egui_dock 0.19.1
> (`split_left/right/above/below`) y al tipo que devuelven (`[NodeIndex; 2]`). El
> `to_dock_state` inicial debe arrancar el `DockState` con SOLO la primera hoja y
> dejar que `build_into` agregue el resto, para que los ids no se dupliquen.
> Verifica que `dock_pane_ids` tras `to_dock_state` devuelva exactamente
> `layout.pane_ids()` (mismo conjunto).

Añade un test que lo verifique para `dual_pane`:

```rust
    #[test]
    fn dual_pane_layout_round_trips_los_ids() {
        use naygo_core::workspace::Workspace;
        let tpl = naygo_core::workspace::template::LayoutTemplate::dual_pane();
        let w = Workspace::from_template(&tpl, std::path::Path::new("C:/home"));
        let state = to_dock_state(&w.layout);
        let mut got = dock_pane_ids(&state);
        let mut want = w.layout.pane_ids();
        got.sort();
        want.sort();
        assert_eq!(got, want, "el dock contiene exactamente los paneles del layout");
    }
```

- [ ] **Step 4: Verificar**

Run: `cargo test -p naygo-ui dock_translate`
Expected: PASS (2 tests). Si `iter_all_tabs`/`Surface`/`split_*` difieren en nombre, ajustar contra la fuente y reportar.

Run: `cargo clippy -p naygo-ui -- -D warnings` → limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/dock_translate.rs crates/ui/src/main.rs
git commit -m "feat(ui): traducción SerializableDockLayout -> DockState<PaneId>

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: Ampliar `input` (atrás/adelante por teclado y mouse)

**Files:**
- Modify: `crates/ui/src/input.rs`
- Test: ampliar `#[cfg(test)]` de `input.rs`

- [ ] **Step 1: Añadir las acciones nuevas y su mapeo (TDD)**

Modify `crates/ui/src/input.rs`:

(a) Añadir a `enum Action` las variantes `GoBack`, `GoForward` (tras `GoUp`):
```rust
    /// Ir atrás en el historial del panel activo.
    GoBack,
    /// Ir adelante en el historial del panel activo.
    GoForward,
```

(b) `map_key` NO cambia para `Key` (atrás/adelante en teclado usan modificador Alt,
que se maneja en app.rs, no en el `Key` simple). Añadir un mapeo de botón de mouse:

```rust
/// Botones extra del mouse (laterales). Espejo de `egui::PointerButton`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseExtra {
    /// Botón lateral 1 (típicamente "atrás").
    Back,
    /// Botón lateral 2 (típicamente "adelante").
    Forward,
}

/// Mapea un botón lateral del mouse a su acción de navegación.
pub fn map_mouse_extra(button: MouseExtra) -> Action {
    match button {
        MouseExtra::Back => Action::GoBack,
        MouseExtra::Forward => Action::GoForward,
    }
}
```

Ampliar los tests:
```rust
    #[test]
    fn botones_laterales_del_mouse_navegan() {
        assert_eq!(map_mouse_extra(MouseExtra::Back), Action::GoBack);
        assert_eq!(map_mouse_extra(MouseExtra::Forward), Action::GoForward);
    }
```

- [ ] **Step 2: Correr los tests**

Run: `cargo test -p naygo-ui input`
Expected: PASS (los 3 previos + 1 nuevo = 4).

Run: `cargo clippy -p naygo-ui -- -D warnings` → limpio (puede haber dead-code de `GoBack`/`GoForward` hasta que app.rs los use en la Tarea 12; si clippy se queja con `-D warnings`, está bien que NO compiles clippy con `-D warnings` en ESTA tarea aislada o que esperes a la Tarea 12. Recomendado: implementar Tareas 11 y 12 juntas antes de correr clippy estricto. Reporta si hay warning de variante no construida.)

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src/input.rs
git commit -m "feat(ui): acciones GoBack/GoForward + mapeo de botones laterales del mouse

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 12: Reescribir `app.rs` — `Workspace` multi-panel con N workers

La tarea grande de la UI: reemplaza el `UiState` de un panel por un `Workspace` de N paneles, cada `Files` con su propio worker de listing, carga/guarda config, y mapea teclado+mouse a acciones sobre el panel activo.

**Files:**
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/main.rs` (declarar `mod toolbar; mod templates_menu;` — stubs hasta sus tareas)

- [ ] **Step 1: Crear stubs de toolbar y templates_menu para que compile**

Create `crates/ui/src/toolbar.rs`:
```rust
// Naygo — barra de íconos (posición configurable). Contenido real en Tarea 14.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```
Create `crates/ui/src/templates_menu.rs`:
```rust
// Naygo — combobox de plantillas. Contenido real en Tarea 15.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```
Modify `crates/ui/src/main.rs` — añadir `mod toolbar;` y `mod templates_menu;`.

- [ ] **Step 2: Reescribir `app.rs`**

Replace `crates/ui/src/app.rs` con la estructura multi-panel. Puntos clave:
- `NaygoApp` tiene: `workspace: Workspace`, `dock_state: DockState<PaneId>`,
  `listings: HashMap<PaneId, PaneListing>`, `settings: Settings`,
  `templates: TemplateStore`, `config_dir: PathBuf`, `status: String`,
  `typeahead_buf: String`.
- `PaneListing { rx: Option<Receiver<ListingMsg>>, token: CancellationToken }` — el
  worker activo de un panel `Files`.

```rust
// Naygo — estado raíz de la aplicación y loop de egui (multi-panel).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `NaygoApp` mantiene un `Workspace` de N paneles independientes. Cada panel
//! `Files` lista en su propio worker (un `PaneListing`); el hilo de UI drena
//! todos los canales sin bloquear. El teclado y los botones del mouse actúan
//! sobre el panel activo. El layout y las carpetas se persisten vía `config`.

use crate::input::{map_key, map_mouse_extra, Action, Key as NaygoKey, MouseExtra};
use eframe::CreationContext;
use egui_dock::{DockArea, DockState, Style};
use naygo_core::cancel::CancellationToken;
use naygo_core::config::{self, Settings};
use naygo_core::listing::{spawn_listing, ListingMsg};
use naygo_core::sort::sort_entries;
use naygo_core::workspace::template::LayoutTemplate;
use naygo_core::workspace::{PaneId, PanePurpose, Workspace};
use naygo_core::TemplateStore;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// El worker de listing activo de un panel `Files`.
pub struct PaneListing {
    pub rx: Option<Receiver<ListingMsg>>,
    pub token: CancellationToken,
}

/// Estado raíz de la app.
pub struct NaygoApp {
    pub workspace: Workspace,
    dock_state: DockState<PaneId>,
    listings: HashMap<PaneId, PaneListing>,
    pub settings: Settings,
    pub templates: TemplateStore,
    config_dir: PathBuf,
    pub status: String,
    typeahead_buf: String,
}

impl NaygoApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        let config_dir = config::portable_dir();
        let settings = config::load_settings(&config_dir);
        let templates = config::load_templates(&config_dir);
        let home = default_start_dir();

        // Cargar workspace persistido, o caer al Dual-pane default.
        let workspace = load_or_default_workspace(&config_dir, &home);
        let dock_state = crate::dock_translate::to_dock_state(&workspace.layout);

        let mut app = NaygoApp {
            workspace,
            dock_state,
            listings: HashMap::new(),
            settings,
            templates,
            config_dir,
            status: String::new(),
            typeahead_buf: String::new(),
        };
        app.start_all_listings();
        app
    }

    /// Lanza un worker de listing para CADA panel `Files`, en su carpeta.
    fn start_all_listings(&mut self) {
        let files: Vec<(PaneId, PathBuf)> = self
            .workspace
            .panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Files)
            .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.current_dir.clone())))
            .collect();
        for (id, dir) in files {
            self.start_listing(id, dir);
        }
    }

    /// (Re)lanza el listado de un panel: cancela el anterior y arranca otro.
    fn start_listing(&mut self, id: PaneId, dir: PathBuf) {
        if let Some(prev) = self.listings.get(&id) {
            prev.token.cancel();
        }
        let token = CancellationToken::new();
        let (rx, _handle) = spawn_listing(dir, token.clone());
        self.listings.insert(id, PaneListing { rx: Some(rx), token });
    }

    /// Drena los canales de TODOS los paneles, sin bloquear.
    fn pump_all(&mut self) {
        let ids: Vec<PaneId> = self.listings.keys().copied().collect();
        for id in ids {
            self.pump_one(id);
        }
    }

    fn pump_one(&mut self, id: PaneId) {
        // Tomar los mensajes disponibles del canal de este panel.
        let mut finished = false;
        let mut new_entries = Vec::new();
        let mut err = None;
        if let Some(listing) = self.listings.get(&id) {
            if let Some(rx) = &listing.rx {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        ListingMsg::Entry(e) => new_entries.push(e),
                        ListingMsg::Done | ListingMsg::Cancelled => finished = true,
                        ListingMsg::Error(e) => {
                            err = Some(e);
                            finished = true;
                        }
                    }
                }
            }
        }
        // Aplicar al FilePaneState del panel.
        if let Some(pane) = self.workspace.pane_mut(id) {
            if let Some(f) = pane.files.as_mut() {
                f.entries.extend(new_entries);
                if finished {
                    let spec = f.sort;
                    sort_entries(&mut f.entries, &spec);
                    if f.focused.is_none() && !f.entries.is_empty() {
                        f.focused = Some(0);
                    }
                }
            }
        }
        if finished {
            if let Some(listing) = self.listings.get_mut(&id) {
                listing.rx = None;
            }
            if let Some(e) = err {
                self.status = format!("Error: {e}");
            }
        }
    }

    /// ¿Hay algún listado en curso? (para pedir repaint).
    fn any_listing_active(&self) -> bool {
        self.listings.values().any(|l| l.rx.is_some())
    }

    /// Aplica una acción al panel activo.
    pub fn apply_action(&mut self, action: Action) {
        let Some(active) = self.workspace.active_id() else { return };
        match action {
            Action::MoveUp => self.move_focus(-1),
            Action::MoveDown => self.move_focus(1),
            Action::Activate => self.activate_focused(),
            Action::GoUp => self.nav(|f| f.go_up()),
            Action::GoBack => self.nav(|f| f.go_back()),
            Action::GoForward => self.nav(|f| f.go_forward()),
            Action::CancelListing => {
                if let Some(l) = self.listings.get(&active) {
                    l.token.cancel();
                }
            }
            Action::SwitchPane => self.cycle_active_files(),
        }
    }

    /// Ejecuta una navegación (go_up/back/forward) sobre el panel activo y, si
    /// cambió de carpeta, lanza el listado nuevo.
    fn nav(&mut self, f: impl FnOnce(&mut naygo_core::workspace::FilePaneState) -> Option<PathBuf>) {
        let Some(active) = self.workspace.active_id() else { return };
        let moved = self
            .workspace
            .pane_mut(active)
            .and_then(|p| p.files.as_mut())
            .and_then(f);
        if let Some(dir) = moved {
            self.start_listing(active, dir);
        }
    }

    fn move_focus(&mut self, delta: isize) {
        if let Some(f) = self.workspace.active_files_mut() {
            if f.entries.is_empty() {
                return;
            }
            let len = f.entries.len() as isize;
            let cur = f.focused.unwrap_or(0) as isize;
            f.focused = Some((cur + delta).clamp(0, len - 1) as usize);
        }
    }

    fn activate_focused(&mut self) {
        let Some(active) = self.workspace.active_id() else { return };
        let entry = self
            .workspace
            .pane(active)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.focused_entry().cloned());
        let Some(entry) = entry else { return };
        if entry.is_dir() {
            if let Some(f) = self
                .workspace
                .pane_mut(active)
                .and_then(|p| p.files.as_mut())
            {
                f.navigate_to(entry.path.clone());
            }
            self.start_listing(active, entry.path);
        } else {
            self.status = format!("Abrir: {} (pendiente platform::shell)", entry.name);
        }
    }

    /// Cicla el panel activo entre los paneles `Files`.
    fn cycle_active_files(&mut self) {
        let files: Vec<PaneId> = self
            .workspace
            .panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Files)
            .map(|p| p.id)
            .collect();
        if files.is_empty() {
            return;
        }
        let cur = self.workspace.active_id();
        let idx = files.iter().position(|id| Some(*id) == cur).unwrap_or(0);
        let next = files[(idx + 1) % files.len()];
        self.workspace.set_active(next);
    }

    /// type-ahead sobre el panel activo.
    fn typeahead(&mut self, typed: &str) {
        if typed.is_empty() {
            return;
        }
        self.typeahead_buf.push_str(typed);
        let buf = self.typeahead_buf.clone();
        if let Some(f) = self.workspace.active_files_mut() {
            let names: Vec<String> = f.entries.iter().map(|e| e.name.clone()).collect();
            let start = f.focused.unwrap_or(0);
            if let Some(i) = crate::typeahead::find_match(&names, &buf, start) {
                f.focused = Some(i);
            }
        }
    }

    /// Lee teclado + botones del mouse y aplica acciones al panel activo.
    fn handle_input(&mut self, ctx: &egui::Context) {
        let keys = [
            (egui::Key::ArrowUp, NaygoKey::ArrowUp),
            (egui::Key::ArrowDown, NaygoKey::ArrowDown),
            (egui::Key::ArrowLeft, NaygoKey::ArrowLeft),
            (egui::Key::Enter, NaygoKey::Enter),
            (egui::Key::Backspace, NaygoKey::Backspace),
            (egui::Key::Tab, NaygoKey::Tab),
            (egui::Key::Escape, NaygoKey::Escape),
        ];
        let mut actions = Vec::new();
        let mut typed = String::new();
        ctx.input(|i| {
            let alt = i.modifiers.alt;
            // Alt+←/→ = atrás/adelante (antes que el ArrowLeft simple=GoUp).
            if alt && i.key_pressed(egui::Key::ArrowLeft) {
                actions.push(Action::GoBack);
            } else if alt && i.key_pressed(egui::Key::ArrowRight) {
                actions.push(Action::GoForward);
            } else {
                for (egui_key, naygo_key) in keys {
                    if i.key_pressed(egui_key) {
                        if let Some(a) = map_key(naygo_key) {
                            actions.push(a);
                        }
                    }
                }
            }
            // Botones laterales del mouse.
            if i.pointer.button_pressed(egui::PointerButton::Extra1) {
                actions.push(map_mouse_extra(MouseExtra::Back));
            }
            if i.pointer.button_pressed(egui::PointerButton::Extra2) {
                actions.push(map_mouse_extra(MouseExtra::Forward));
            }
            for event in &i.events {
                if let egui::Event::Text(t) = event {
                    typed.push_str(t);
                }
            }
        });

        if !actions.is_empty() {
            self.typeahead_buf.clear();
        }
        for a in actions {
            self.apply_action(a);
        }
        if !typed.is_empty() {
            self.typeahead(&typed);
        }
    }

    /// Guarda el workspace persistible.
    fn save_workspace(&self) {
        let files = self
            .workspace
            .panes()
            .iter()
            .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.to_persist())))
            .collect();
        let purposes = self
            .workspace
            .panes()
            .iter()
            .map(|p| (p.id, p.purpose))
            .collect();
        let persist = config::WorkspacePersist {
            version: 1,
            layout: self.workspace.layout.clone(),
            active: self.workspace.active_id(),
            files,
            purposes,
        };
        config::save_workspace(&self.config_dir, &persist);
    }
}

impl eframe::App for NaygoApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump_all();
        self.handle_input(ctx);
        if self.any_listing_active() {
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Barra (Tarea 14) — por ahora placeholder hasta que toolbar exista.
        crate::toolbar::show(ui, self);

        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let dir = self
                    .workspace
                    .active_files()
                    .map(|f| f.current_dir.display().to_string())
                    .unwrap_or_default();
                ui.label(dir);
                ui.separator();
                ui.label(&self.status);
            });
        });

        let mut viewer = crate::docking::NaygoTabViewer { app: self_ptr_workaround(self) };
        let style = Style::from_egui(ui.style().as_ref());
        // NOTA: el viewer necesita acceso mutable a self.workspace + start_listing.
        // Ver Tarea 13 para la firma real del TabViewer (toma &mut NaygoApp o un
        // contexto). Implementar la conexión allí.
        let _ = (viewer, style);
        DockArea::new(&mut self.dock_state)
            .style(Style::from_egui(ui.style().as_ref()))
            .show_inside(ui, &mut DummyViewer);
        let _ = &mut self.dock_state;
    }
}

/// Carpeta inicial: home del usuario o C:\ como fallback.
fn default_start_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("C:\\"))
}

/// Carga el workspace persistido y lo reconstruye, o cae al Dual-pane default.
fn load_or_default_workspace(dir: &std::path::Path, home: &std::path::Path) -> Workspace {
    if let Some(persist) = config::load_workspace(dir) {
        if let Some(w) = rebuild_workspace(persist, home) {
            return w;
        }
    }
    Workspace::from_template(&LayoutTemplate::dual_pane(), home)
}
```

> **NOTA AL IMPLEMENTADOR (importante):** El bloque `ui()` de arriba contiene
> placeholders (`self_ptr_workaround`, `DummyViewer`) PORQUE la conexión real del
> `TabViewer` con `NaygoApp` se define en la **Tarea 13** (el viewer necesita
> `&mut` al workspace y poder llamar `start_listing`). Para que la Tarea 12
> compile y sea testeable de forma aislada, implementa `ui()` SIN el DockArea real
> todavía: pinta solo la barra + status + un `CentralPanel` placeholder, y deja el
> `dock_state` sin usar con `let _ = &self.dock_state;`. El DockArea real se
> conecta en la Tarea 13. Reporta esta decisión. La función `rebuild_workspace` se
> implementa en el Step 3.

- [ ] **Step 3: Implementar `rebuild_workspace` (restaurar del JSON)**

Añadir en `app.rs`:

```rust
/// Reconstruye un `Workspace` desde lo persistido. Devuelve `None` si el layout es
/// inconsistente (entonces el llamador cae al default). Tolera carpetas que ya no
/// existen: el panel se queda con su ruta y el listado mostrará el error.
fn rebuild_workspace(
    persist: config::WorkspacePersist,
    _home: &std::path::Path,
) -> Option<Workspace> {
    use naygo_core::workspace::FilePaneState;
    let mut w = Workspace::new();
    // Mapa de purposes y de files persistidos.
    let files_map: HashMap<PaneId, _> = persist.files.into_iter().collect();
    // Recrear cada panel respetando su PaneId original es complejo con la API
    // pública (add_pane asigna ids nuevos). Para 2A: reconstruir creando paneles
    // en el orden del layout y re-mapear los PaneId del layout a los nuevos.
    let layout_ids = persist.layout.pane_ids();
    if layout_ids.is_empty() {
        return None;
    }
    let mut remap: HashMap<PaneId, PaneId> = HashMap::new();
    for old_id in &layout_ids {
        let purpose = persist
            .purposes
            .iter()
            .find(|(pid, _)| pid == old_id)
            .map(|(_, p)| *p)?;
        let new_id = match purpose {
            PanePurpose::Files => {
                let fp = files_map.get(old_id)?;
                let state = FilePaneState::from_persist(fp.clone());
                let dir = state.current_dir.clone();
                let id = w.add_pane(PanePurpose::Files, dir);
                // Restaurar sort/view/show_dirs en el panel recién creado.
                if let Some(p) = w.pane_mut(id) {
                    if let Some(f) = p.files.as_mut() {
                        f.sort = state.sort;
                        f.view = state.view;
                        f.show_dirs = state.show_dirs;
                    }
                }
                id
            }
            other => w.add_pane(other, std::path::PathBuf::new()),
        };
        remap.insert(*old_id, new_id);
    }
    // Reconstruir el layout con los ids nuevos.
    w.layout = remap_layout(&persist.layout, &remap);
    // Activo.
    if let Some(old_active) = persist.active {
        if let Some(new_active) = remap.get(&old_active) {
            w.set_active(*new_active);
        }
    }
    Some(w)
}

/// Reescribe los PaneId de un layout según el mapa old→new.
fn remap_layout(
    layout: &naygo_core::workspace::layout::SerializableDockLayout,
    remap: &HashMap<PaneId, PaneId>,
) -> naygo_core::workspace::layout::SerializableDockLayout {
    use naygo_core::workspace::layout::{DockNode, SerializableDockLayout};
    fn go(node: &DockNode, remap: &HashMap<PaneId, PaneId>) -> DockNode {
        match node {
            DockNode::Leaf(id) => DockNode::Leaf(*remap.get(id).unwrap_or(id)),
            DockNode::Split { dir, fraction, first, second } => DockNode::Split {
                dir: *dir,
                fraction: *fraction,
                first: Box::new(go(first, remap)),
                second: Box::new(go(second, remap)),
            },
        }
    }
    SerializableDockLayout {
        root: layout.root.as_ref().map(|n| go(n, remap)),
    }
}
```

- [ ] **Step 4: Hacer que compile sin el DockArea real (placeholder ui)**

Ajusta el `ui()` de `app.rs` a esta versión mínima (el DockArea real llega en Tarea 13):

```rust
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::toolbar::show(ui, self);
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let dir = self
                    .workspace
                    .active_files()
                    .map(|f| f.current_dir.display().to_string())
                    .unwrap_or_default();
                ui.label(dir);
                ui.separator();
                ui.label(&self.status);
            });
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.label("(dock en construcción — Tarea 13)");
        });
        let _ = &self.dock_state; // se usará en la Tarea 13
    }
```

Y `crate::toolbar::show` debe existir como stub que no pinta nada todavía. Modify `crates/ui/src/toolbar.rs`:
```rust
// Naygo — barra de íconos (posición configurable). Contenido real en Tarea 14.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;

/// Stub: la barra real se implementa en la Tarea 14.
pub fn show(_ui: &mut egui::Ui, _app: &mut NaygoApp) {}
```

Elimina del `app.rs` el bloque con `self_ptr_workaround`/`DummyViewer`/`viewer` (era ilustrativo); deja solo la versión mínima de `ui()` de este step.

- [ ] **Step 5: Compilar y verificar**

Run: `cargo build -p naygo-ui`
Expected: compila (la app abre con un placeholder central; el dock real llega en Tarea 13). Reporta warnings verbatim.

Run: `cargo test --workspace` → core completo + ui (input, typeahead, dock_translate) verdes.

Run: `cargo clippy -p naygo-ui -- -D warnings` → reporta y resuelve mínimamente (puede haber dead-code de campos/métodos que usa la Tarea 13: `pump_one` privado se usa; `save_workspace` aún no se llama → si clippy se queja de método sin usar, llámalo en `on_exit` o márcalo para Tarea 13; ver Step 6).

- [ ] **Step 6: Guardar al cerrar**

Implementa `eframe::App::on_exit` o guarda en `save_workspace` al detectar cierre. egui 0.34: añade al `impl eframe::App for NaygoApp`:
```rust
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save_workspace();
        config::save_settings(&self.config_dir, &self.settings);
        config::save_templates(&self.config_dir, &self.templates);
    }
```
> Verifica en la fuente de eframe 0.34.3 (`epi.rs`) la firma exacta de `save`
> (`fn save(&mut self, _storage: &mut dyn Storage)`). eframe llama `save`
> periódicamente y al cerrar. Si la firma difiere, ajústala. Esto también usa
> `save_workspace`, eliminando el posible warning de método sin usar.

Run: `cargo clippy -p naygo-ui -- -D warnings` → limpio.

- [ ] **Step 7: Commit**

```bash
git add crates/ui/src/app.rs crates/ui/src/main.rs crates/ui/src/toolbar.rs crates/ui/src/templates_menu.rs
git commit -m "feat(ui): app multi-panel con Workspace, N workers de listing, carga/guarda config

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 13: Conectar el DockArea real + paneles por `PanePurpose`

**Files:**
- Modify: `crates/ui/src/docking.rs`
- Modify: `crates/ui/src/app.rs` (usar el viewer real en `ui()`)
- Modify: `crates/ui/src/panes/{file_panel,tree_panel,inspector_panel}.rs`

- [ ] **Step 1: Reescribir el `TabViewer` para despachar por `PaneId`**

El reto de borrow: el `TabViewer` necesita leer/mutar el `Workspace` y poder lanzar
listados. Patrón: el viewer guarda referencias mutables a lo que necesita +
acumula "navegaciones pendientes" que `app` ejecuta tras pintar (para no llamar
`start_listing` con `self` prestado).

Replace `crates/ui/src/docking.rs`:

```rust
// Naygo — TabViewer de egui_dock: despacha cada panel por su PaneId/PanePurpose.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! El `TabViewer` recibe un `PaneId` por tab, busca el panel en el `Workspace` y
//! lo pinta según su `PanePurpose`. Las navegaciones que el usuario dispara con el
//! mouse (doble clic en una carpeta, botón "subir") se acumulan en `pending` para
//! que `NaygoApp` las ejecute tras pintar, evitando préstamos conflictivos.

use naygo_core::workspace::{PaneId, PanePurpose, Workspace};
use std::path::PathBuf;

/// Una navegación pedida desde un panel durante el pintado, a ejecutar después.
pub enum PaneRequest {
    /// El panel `id` debe navegar a `dir` (entra al historial).
    NavigateTo { id: PaneId, dir: PathBuf },
    /// El panel `id` pasa a ser el activo.
    Activate { id: PaneId },
}

pub struct NaygoTabViewer<'a> {
    pub workspace: &'a mut Workspace,
    pub status: &'a mut String,
    pub pending: &'a mut Vec<PaneRequest>,
}

impl egui_dock::TabViewer for NaygoTabViewer<'_> {
    type Tab = PaneId;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match self.workspace.pane(*tab).map(|p| p.purpose) {
            Some(PanePurpose::Files) => {
                let name = self
                    .workspace
                    .pane(*tab)
                    .and_then(|p| p.files.as_ref())
                    .map(|f| {
                        f.current_dir
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| f.current_dir.display().to_string())
                    })
                    .unwrap_or_default();
                name.into()
            }
            Some(PanePurpose::Tree) => "Carpetas".into(),
            Some(PanePurpose::Inspector) => "Propiedades".into(),
            None => "—".into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let id = *tab;
        let purpose = self.workspace.pane(id).map(|p| p.purpose);
        match purpose {
            Some(PanePurpose::Files) => {
                crate::panes::file_panel::show(ui, self.workspace, id, self.pending)
            }
            Some(PanePurpose::Tree) => {
                crate::panes::tree_panel::show(ui, self.workspace, self.pending)
            }
            Some(PanePurpose::Inspector) => {
                crate::panes::inspector_panel::show(ui, self.workspace)
            }
            None => {
                ui.label("Panel desconocido");
            }
        }
        let _ = self.status;
    }
}
```

- [ ] **Step 2: Reescribir `file_panel.rs` para operar sobre el Workspace + emitir requests**

Replace `crates/ui/src/panes/file_panel.rs`:

```rust
// Naygo — panel de archivos: vista Detalle (columnas) sobre un FilePaneState.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta las entradas del panel `id` del workspace en columnas. Respeta
//! `show_dirs` (oculta carpetas si está off). Clic selecciona; doble clic sobre
//! una carpeta emite una `NavigateTo`; clic en el panel lo activa. No hace I/O.

use crate::docking::PaneRequest;
use naygo_core::fs_model::{Entry, EntryKind};
use naygo_core::workspace::{PaneId, Workspace};

pub fn show(ui: &mut egui::Ui, workspace: &mut Workspace, id: PaneId, pending: &mut Vec<PaneRequest>) {
    // Marcar este panel como activo al interactuar.
    let is_active = workspace.active_id() == Some(id);
    if !is_active && ui.rect_contains_pointer(ui.max_rect()) && ui.input(|i| i.pointer.any_pressed()) {
        pending.push(PaneRequest::Activate { id });
    }

    let Some(pane) = workspace.pane(id) else { return };
    let Some(f) = pane.files.as_ref() else { return };
    let focused = f.focused;
    let show_dirs = f.show_dirs;

    // Breadcrumb simple (clicable llega en pulido; aquí muestra la ruta).
    ui.horizontal(|ui| {
        ui.monospace(f.current_dir.display().to_string());
    });
    ui.separator();

    let mut clicked: Option<usize> = None;
    let mut activated: Option<usize> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new(("file_grid", id.0))
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Nombre");
                ui.strong("Tamaño");
                ui.strong("Modificado");
                ui.end_row();

                for (i, entry) in f.entries.iter().enumerate() {
                    // Filtro show_dirs.
                    if !show_dirs && entry.is_dir() {
                        continue;
                    }
                    let selected = focused == Some(i);
                    let label = format!("{} {}", kind_glyph(entry.kind), entry.name);
                    let resp = ui.selectable_label(selected, label);
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                    if resp.double_clicked() {
                        activated = Some(i);
                    }
                    ui.label(format_size(entry));
                    ui.label(format_modified(entry));
                    ui.end_row();
                }
            });
    });

    // Aplicar selección/activación.
    if let Some(i) = clicked {
        if let Some(f) = workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.focused = Some(i);
        }
        pending.push(PaneRequest::Activate { id });
    }
    if let Some(i) = activated {
        let entry = workspace
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.entries.get(i).cloned());
        if let Some(entry) = entry {
            if entry.is_dir() {
                pending.push(PaneRequest::Activate { id });
                pending.push(PaneRequest::NavigateTo { id, dir: entry.path });
            }
        }
    }
}

fn kind_glyph(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Directory => "[D]",
        EntryKind::File => "   ",
        EntryKind::Other => "[?]",
    }
}

fn format_size(entry: &Entry) -> String {
    match entry.size {
        Some(bytes) => human_size(bytes),
        None => String::new(),
    }
}

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

/// PROVISIONAL: segundos epoch hasta tener i18n (fase 2C).
fn format_modified(entry: &Entry) -> String {
    use std::time::UNIX_EPOCH;
    match entry.modified.and_then(|t| t.duration_since(UNIX_EPOCH).ok()) {
        Some(d) => format!("{}", d.as_secs()),
        None => String::new(),
    }
}
```

- [ ] **Step 3: Reescribir `tree_panel.rs` e `inspector_panel.rs`**

Replace `crates/ui/src/panes/tree_panel.rs`:

```rust
// Naygo — panel de árbol (esqueleto de Fase 2A): ubicación del panel activo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Esqueleto: muestra la carpeta del panel `Files` activo y permite subir. El
//! árbol expandible real es trabajo posterior. Emite un request de navegación
//! sobre el panel activo.

use crate::docking::PaneRequest;
use naygo_core::workspace::Workspace;

pub fn show(ui: &mut egui::Ui, workspace: &mut Workspace, pending: &mut Vec<PaneRequest>) {
    let active = workspace.active_id();
    let dir = workspace
        .active_files()
        .map(|f| f.current_dir.clone());

    ui.label("Panel activo en:");
    if let Some(d) = &dir {
        ui.monospace(d.display().to_string());
    } else {
        ui.label("—");
    }
    ui.separator();
    if ui.button("⬆ Subir un nivel").clicked() {
        if let (Some(id), Some(d)) = (active, dir) {
            if let Some(parent) = d.parent() {
                pending.push(PaneRequest::NavigateTo {
                    id,
                    dir: parent.to_path_buf(),
                });
            }
        }
    }
}
```

Replace `crates/ui/src/panes/inspector_panel.rs`:

```rust
// Naygo — inspector: metadatos del elemento enfocado en el panel Files activo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Refleja el panel `Files` ACTIVO: muestra los metadatos del elemento enfocado.
//! Las propiedades extendidas del Shell llegan con `platform::shell` (fase futura).

use naygo_core::fs_model::EntryKind;
use naygo_core::workspace::Workspace;

pub fn show(ui: &mut egui::Ui, workspace: &mut Workspace) {
    let Some(entry) = workspace.active_files().and_then(|f| f.focused_entry()) else {
        ui.label("Nada seleccionado.");
        return;
    };
    let (name, kind, path, size) = (
        entry.name.clone(),
        entry.kind,
        entry.path.clone(),
        entry.size,
    );

    egui::Grid::new("inspector_grid").num_columns(2).show(ui, |ui| {
        ui.strong("Nombre");
        ui.label(&name);
        ui.end_row();
        ui.strong("Tipo");
        ui.label(match kind {
            EntryKind::Directory => "Carpeta",
            EntryKind::File => "Archivo",
            EntryKind::Other => "Otro",
        });
        ui.end_row();
        ui.strong("Ruta");
        ui.label(path.display().to_string());
        ui.end_row();
        if let Some(s) = size {
            ui.strong("Tamaño");
            ui.label(format!("{s} bytes"));
            ui.end_row();
        }
    });
}
```

- [ ] **Step 4: Conectar el DockArea real en `app.rs::ui` + drenar `pending`**

Modify `crates/ui/src/app.rs` — reemplazar el `ui()` placeholder por el real:

```rust
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::toolbar::show(ui, self);

        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let dir = self
                    .workspace
                    .active_files()
                    .map(|f| f.current_dir.display().to_string())
                    .unwrap_or_default();
                ui.label(dir);
                ui.separator();
                ui.label(&self.status);
            });
        });

        // Pintar el dock; las navegaciones pedidas se acumulan en `pending`.
        let mut pending: Vec<crate::docking::PaneRequest> = Vec::new();
        {
            let mut viewer = crate::docking::NaygoTabViewer {
                workspace: &mut self.workspace,
                status: &mut self.status,
                pending: &mut pending,
            };
            DockArea::new(&mut self.dock_state)
                .style(Style::from_egui(ui.style().as_ref()))
                .show_inside(ui, &mut viewer);
        }
        // Ejecutar las navegaciones/activaciones acumuladas (ya sin viewer prestado).
        for req in pending {
            match req {
                crate::docking::PaneRequest::Activate { id } => {
                    self.workspace.set_active(id);
                }
                crate::docking::PaneRequest::NavigateTo { id, dir } => {
                    if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                        f.navigate_to(dir.clone());
                    }
                    self.start_listing(id, dir);
                }
            }
        }
    }
```

Asegúrate de que los `use` de `app.rs` incluyan `DockArea` y `Style` (de `egui_dock`).

- [ ] **Step 5: Compilar y verificar a fondo**

Run: `cargo build -p naygo-ui` → compila. Reporta warnings verbatim.
Run: `cargo clippy --workspace -- -D warnings` → limpio (resuelve mínimamente).
Run: `cargo test --workspace` → todo verde.
Run: `cargo fmt` → formatear.

App-start + verificación manual de comportamiento:
`$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; $p = Start-Process -FilePath "cargo" -ArgumentList "run","-p","naygo-ui" -PassThru -WindowStyle Hidden; Start-Sleep -Seconds 30; if (-not $p.HasExited) { "STILL RUNNING (good)"; $p.Kill() } else { "EXITED code $($p.ExitCode)" }`

Verifica (manual, si hay display): arranca con Dual-pane (árbol + 2 paneles + inspector), cada panel lista su carpeta por streaming, doble clic entra a carpeta en ese panel, Tab cambia el panel activo, el inspector refleja el activo, `Alt+←/→` y botones del mouse hacen atrás/adelante.

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/docking.rs crates/ui/src/app.rs crates/ui/src/panes/
git commit -m "feat(ui): DockArea real multi-panel; paneles operan sobre el Workspace

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 14: Barra de íconos (posición configurable, solo-ícono)

**Files:**
- Modify: `crates/ui/src/toolbar.rs`

- [ ] **Step 1: Implementar la barra real**

Replace `crates/ui/src/toolbar.rs`:

```rust
// Naygo — barra de íconos: navegación + layouts + agregar panel + ajustes.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Barra de acciones con botones solo-ícono (tooltips). Posición configurable
//! (arriba o al costado), según `Settings.bar_position`. Atrás/adelante se
//! habilitan según el historial del panel activo.

use crate::app::NaygoApp;
use crate::input::Action;
use naygo_core::config::BarPosition;

/// Pinta la barra en la posición configurada. Debe llamarse al inicio de `ui()`.
pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    match app.settings.bar_position {
        BarPosition::Top => {
            egui::Panel::top("toolbar").show_inside(ui, |ui| {
                ui.horizontal(|ui| buttons(ui, app));
            });
        }
        BarPosition::Side => {
            egui::SidePanel::left("toolbar")
                .resizable(false)
                .exact_width(40.0)
                .show_inside(ui, |ui| {
                    ui.vertical(|ui| buttons(ui, app));
                });
        }
    }
}

fn buttons(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (can_back, can_forward) = app
        .workspace
        .active_files()
        .map(|f| (f.history.can_back(), f.history.can_forward()))
        .unwrap_or((false, false));

    if icon_button(ui, "◀", "Atrás (Alt+←)", can_back) {
        app.apply_action(Action::GoBack);
    }
    if icon_button(ui, "▶", "Adelante (Alt+→)", can_forward) {
        app.apply_action(Action::GoForward);
    }
    if icon_button(ui, "▲", "Subir un nivel (Backspace)", true) {
        app.apply_action(Action::GoUp);
    }
    if icon_button(ui, "⟳", "Refrescar", true) {
        // Re-listar el panel activo: navegar a la misma carpeta.
        if let Some(f) = app.workspace.active_files() {
            let dir = f.current_dir.clone();
            if let Some(id) = app.workspace.active_id() {
                app.refresh_pane(id, dir);
            }
        }
    }
    ui.separator();
    // Layouts y agregar panel se conectan en la Tarea 15 (templates_menu).
    crate::templates_menu::layouts_button(ui, app);
    if icon_button(ui, "➕", "Agregar panel de archivos", true) {
        app.add_files_pane();
    }
}

/// Un botón solo-ícono con tooltip; deshabilitado si `enabled` es false.
fn icon_button(ui: &mut egui::Ui, icon: &str, tip: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(icon))
        .on_hover_text(tip)
        .clicked()
}
```

- [ ] **Step 2: Añadir los métodos `refresh_pane` y `add_files_pane` a `NaygoApp`**

Modify `crates/ui/src/app.rs` — dentro de `impl NaygoApp`, añadir:

```rust
    /// Re-lista un panel sin tocar su historial (refrescar).
    pub fn refresh_pane(&mut self, id: PaneId, dir: PathBuf) {
        // Limpiar entries y relanzar el worker.
        if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.entries.clear();
            f.focused = None;
        }
        self.start_listing(id, dir);
    }

    /// Agrega un panel de archivos nuevo en la carpeta del activo (o home) y lo
    /// inserta en el dock.
    pub fn add_files_pane(&mut self) {
        let dir = self
            .workspace
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(default_start_dir);
        let id = self.workspace.add_pane(PanePurpose::Files, dir.clone());
        // Insertar el tab en el dock (egui_dock): se añade como nuevo tab en el
        // surface principal. Ver la API real de push_to_focused_leaf / add_window.
        self.dock_state.main_surface_mut().push_to_focused_leaf(id);
        self.start_listing(id, dir);
    }
```

> Verifica el nombre real para insertar un tab en egui_dock 0.19.1
> (`push_to_focused_leaf` existe en `Tree`). Si difiere, ajusta contra la fuente.

- [ ] **Step 3: Conectar `templates_menu::layouts_button` como stub temporal**

Modify `crates/ui/src/templates_menu.rs`:
```rust
// Naygo — combobox de plantillas. Contenido real en Tarea 15.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;

/// Stub: botón "Layouts" sin menú todavía (Tarea 15 lo completa).
pub fn layouts_button(ui: &mut egui::Ui, _app: &mut NaygoApp) {
    let _ = ui.button("▦").on_hover_text("Layouts");
}
```

- [ ] **Step 4: Compilar, verificar, formatear**

Run: `cargo build -p naygo-ui` → compila. Warnings verbatim.
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
App-start check (como Tarea 13 Step 5). Verifica manual: la barra aparece arriba con los íconos; atrás/adelante se ven deshabilitados sin historial; ➕ agrega un panel.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/toolbar.rs crates/ui/src/app.rs crates/ui/src/templates_menu.rs
git commit -m "feat(ui): barra de íconos (posición configurable) con navegación y agregar panel

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 15: Combobox de plantillas (recientes/favoritos/built-in + guardar)

**Files:**
- Modify: `crates/ui/src/templates_menu.rs`
- Modify: `crates/ui/src/app.rs` (métodos `apply_template`, `save_current_as_template`)

- [ ] **Step 1: Añadir a `NaygoApp` la aplicación y guardado de plantillas**

Modify `crates/ui/src/app.rs` — dentro de `impl NaygoApp`, añadir:

```rust
    /// Aplica una plantilla: recompone el workspace, reconstruye el dock, registra
    /// el uso en recientes y relanza los listados. `now` es el timestamp (epoch s)
    /// inyectado desde la UI (core no llama a SystemTime::now).
    pub fn apply_template(&mut self, tpl: &LayoutTemplate, now: u64) {
        let home = default_start_dir();
        self.workspace = Workspace::from_template(tpl, &home);
        self.dock_state = crate::dock_translate::to_dock_state(&self.workspace.layout);
        self.listings.clear();
        self.templates.record_use(&tpl.name, now);
        self.start_all_listings();
    }

    /// Guarda la disposición actual como una plantilla del usuario con `name`.
    /// (Para 2A, guardamos la forma del layout actual con carpetas = Home.)
    pub fn save_current_as_template(&mut self, name: &str) {
        use naygo_core::workspace::template::{LayoutShape, LayoutTemplate, TemplateDir, TemplatePane};
        // Construir panes + LayoutShape a partir del workspace actual.
        let ids = self.workspace.layout.pane_ids();
        let mut panes = Vec::new();
        let mut index_of = std::collections::HashMap::new();
        for (idx, id) in ids.iter().enumerate() {
            let purpose = self
                .workspace
                .pane(*id)
                .map(|p| p.purpose)
                .unwrap_or(PanePurpose::Files);
            panes.push(TemplatePane { purpose, dir: TemplateDir::Home });
            index_of.insert(*id, idx);
        }
        let shape = layout_to_shape(&self.workspace.layout, &index_of);
        let tpl = LayoutTemplate {
            name: name.to_string(),
            builtin: false,
            favorite: false,
            panes,
            layout: shape,
        };
        self.templates.add_user(tpl);
    }
```

Y añadir la función libre en `app.rs`:

```rust
/// Traduce el SerializableDockLayout actual a un LayoutShape (índices) para
/// guardar como plantilla.
fn layout_to_shape(
    layout: &naygo_core::workspace::layout::SerializableDockLayout,
    index_of: &std::collections::HashMap<PaneId, usize>,
) -> naygo_core::workspace::template::LayoutShape {
    use naygo_core::workspace::layout::DockNode;
    use naygo_core::workspace::template::LayoutShape;
    fn go(node: &DockNode, index_of: &std::collections::HashMap<PaneId, usize>) -> LayoutShape {
        match node {
            DockNode::Leaf(id) => LayoutShape::Leaf(*index_of.get(id).unwrap_or(&0)),
            DockNode::Split { dir, fraction, first, second } => LayoutShape::Split {
                dir: *dir,
                fraction: *fraction,
                first: Box::new(go(first, index_of)),
                second: Box::new(go(second, index_of)),
            },
        }
    }
    // Si no hay root (no debería en 2A), un solo leaf 0.
    layout
        .root
        .as_ref()
        .map(|n| go(n, index_of))
        .unwrap_or(LayoutShape::Leaf(0))
}
```

- [ ] **Step 2: Implementar el combobox**

Replace `crates/ui/src/templates_menu.rs`:

```rust
// Naygo — combobox de plantillas: recientes, favoritos, built-in, guardar.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! El botón "▦ Layouts" abre un menú con: Recientes (auto), Favoritos y Míos
//! (del usuario), Built-in, y "Guardar disposición actual…". La administración
//! fina (renombrar, limpiar) vive en Configuración (fase posterior).

use crate::app::NaygoApp;
use naygo_core::workspace::template::LayoutTemplate;

pub fn layouts_button(ui: &mut egui::Ui, app: &mut NaygoApp) {
    ui.menu_button("▦", |ui| {
        // Timestamp inyectado (epoch s) para registrar recientes.
        let now = now_epoch_secs();

        // Recientes.
        if !app.templates.recents.is_empty() {
            ui.label("🕘 Recientes");
            let recents: Vec<String> = app.templates.recents.iter().map(|r| r.name.clone()).collect();
            for name in recents {
                if ui.button(&name).clicked() {
                    if let Some(tpl) = find_template(app, &name) {
                        app.apply_template(&tpl, now);
                    }
                    ui.close_menu();
                }
            }
            ui.separator();
        }

        // Favoritos + Míos.
        let favs: Vec<LayoutTemplate> =
            app.templates.user.iter().filter(|t| t.favorite).cloned().collect();
        if !favs.is_empty() {
            ui.label("★ Favoritos");
            for t in favs {
                if ui.button(&t.name).clicked() {
                    app.apply_template(&t, now);
                    ui.close_menu();
                }
            }
            ui.separator();
        }

        let mine: Vec<LayoutTemplate> = app.templates.user.clone();
        if !mine.is_empty() {
            ui.label("👤 Míos");
            for t in mine {
                ui.horizontal(|ui| {
                    if ui.button(&t.name).clicked() {
                        app.apply_template(&t, now);
                        ui.close_menu();
                    }
                    let star = if t.favorite { "★" } else { "☆" };
                    if ui.small_button(star).on_hover_text("Favorito").clicked() {
                        app.templates.set_favorite(&t.name, !t.favorite);
                    }
                    if ui.small_button("🗑").on_hover_text("Borrar").clicked() {
                        app.templates.remove_user(&t.name);
                    }
                });
            }
            ui.separator();
        }

        // Built-in.
        ui.label("📋 Predefinidos");
        for t in LayoutTemplate::builtins() {
            if ui.button(&t.name).clicked() {
                app.apply_template(&t, now);
                ui.close_menu();
            }
        }
        ui.separator();

        // Guardar disposición actual.
        if ui.button("💾 Guardar disposición actual…").clicked() {
            // Nombre simple por defecto; un diálogo de nombre fino llega luego.
            let n = app.templates.user.len() + 1;
            app.save_current_as_template(&format!("Mi layout {n}"));
            ui.close_menu();
        }
    })
    .response
    .on_hover_text("Layouts");
}

/// Busca una plantilla por nombre entre las del usuario y las built-in.
fn find_template(app: &NaygoApp, name: &str) -> Option<LayoutTemplate> {
    app.templates
        .user
        .iter()
        .find(|t| t.name == name)
        .cloned()
        .or_else(|| LayoutTemplate::builtins().into_iter().find(|t| t.name == name))
}

/// Segundos epoch actuales (la UI puede llamar a SystemTime; core no).
fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
```

- [ ] **Step 3: Compilar, verificar, formatear**

Run: `cargo build -p naygo-ui` → compila. Warnings verbatim.
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
App-start check. Verifica manual: el botón ▦ abre el menú; aplicar "Minimalista" deja 1 panel; aplicar "Power-user" deja 3 paneles + inspector; "Guardar disposición actual" crea una entrada en Míos; marcar ★ y borrar 🗑 funcionan.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/templates_menu.rs crates/ui/src/app.rs
git commit -m "feat(ui): combobox de plantillas (recientes/favoritos/built-in + guardar)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 16: Ajuste de posición de barra + cierre de fase

**Files:**
- Modify: `crates/ui/src/toolbar.rs` (botón ⚙ que alterna la posición de la barra — config mínima de 2A)
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: Botón de ajustes mínimo (alternar posición de barra)**

Modify `crates/ui/src/toolbar.rs` — en `buttons`, tras el `➕`, añadir al final
(para `Top`) o donde corresponda un botón `⚙` que, por ahora, alterna la posición
de la barra (la UI completa de Configuración es fase posterior):

```rust
    // Empuja el botón de ajustes al extremo.
    if matches!(app.settings.bar_position, BarPosition::Top) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            settings_button(ui, app);
        });
    } else {
        settings_button(ui, app);
    }
```

Y añadir la función:

```rust
fn settings_button(ui: &mut egui::Ui, app: &mut NaygoApp) {
    ui.menu_button("⚙", |ui| {
        ui.label("Posición de la barra");
        if ui.button("Arriba").clicked() {
            app.settings.bar_position = BarPosition::Top;
            ui.close_menu();
        }
        if ui.button("Al costado").clicked() {
            app.settings.bar_position = BarPosition::Side;
            ui.close_menu();
        }
        ui.separator();
        let mut icon_only = app.settings.icon_only;
        if ui.checkbox(&mut icon_only, "Solo íconos").changed() {
            app.settings.icon_only = icon_only;
        }
    })
    .response
    .on_hover_text("Ajustes");
}
```

> NOTA: `icon_only` se persiste y queda disponible; mostrar texto junto al ícono
> cuando es `false` es un pulido posterior (en 2A los botones son solo-ícono
> siempre; el flag se guarda para 2B/2C). Si clippy marca `icon_only` como
> "leído pero sin efecto", añade un comentario `// efecto visual en fase posterior`.

- [ ] **Step 2: Actualizar el README**

Modify `README.md` — actualizar el bloque de estado para reflejar Fase 2A:

```markdown
> **Estado:** Fase 2A (layout dinámico) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-05-naygo-fase2a-layout-dinamico-design.md`](docs/superpowers/specs/2026-06-05-naygo-fase2a-layout-dinamico-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-05-naygo-fase2a-layout-dinamico.md`](docs/superpowers/plans/2026-06-05-naygo-fase2a-layout-dinamico.md).
> La Fase 1 (esqueleto navegable) está completa.
```

- [ ] **Step 3: Verificación final completa**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → todo verde (core: cancel, fs_model, sort, listing, nav_history, file_pane, layout, workspace, template, config; ui: input, typeahead, dock_translate).
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo fmt --check` → limpio (si no, `cargo fmt` y re-commit).
Run: `cargo build --release -p naygo-ui` → release compila (metadatos del .exe intactos).
App-start manual: la app abre en Dual-pane, paneles independientes listan en paralelo, navegación atrás/adelante (teclado/mouse) funciona, plantillas aplican, posición de barra se puede cambiar, y al cerrar/reabrir se restaura el workspace (probar moviendo un panel a otra carpeta, cerrar, reabrir).

- [ ] **Step 4: Commit y push**

```bash
git add crates/ui/src/toolbar.rs README.md
git commit -m "feat(ui): ajuste de posición de barra; cierre de Fase 2A

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/fase2a-layout-dinamico
```

---

## Self-review (cobertura del spec)

| Requisito del spec 2A | Tarea(s) |
|---|---|
| Paneles independientes componibles (Files/Tree/Inspector) | 1, 5, 13 |
| Estado por panel (carpeta, sort, vista, foco, selección, historial, show_dirs) | 3 |
| `NavHistory` atrás/adelante puro | 2 |
| Inspector sigue al panel activo | 13 (inspector usa `active_files`) |
| Navegación atrás/adelante por panel | 3, 12 (`nav`) |
| `Alt+←/→` + botones laterales del mouse | 11, 12 (`handle_input`) |
| Plantillas built-in (4) | 6 |
| Materializar plantilla → workspace | 7 |
| Plantillas propias + favoritos + recientes | 6 (`TemplateStore`), 15 |
| Combobox de plantillas + guardar | 15 |
| Barra de íconos solo-ícono, posición configurable | 14, 16 |
| Filtro `show_dirs` por panel | 3, 13 (file_panel respeta) |
| `text_filter` reservado (sin UI) | 3 (campo, siempre None) |
| Breadcrumb por panel | 13 (file_panel muestra ruta) |
| Persistencia (workspace/templates/settings, tolerante) | 8, 12 (`save`/`load`) |
| Errores: JSON corrupto→default; carpeta inexistente; purpose desconocido | 8, 12 (`rebuild_workspace`) |
| Desacople core↔egui_dock (`SerializableDockLayout`) | 4, 10 (traducción) |
| Un worker de listing por panel; UI no bloquea | 12 (`start_all_listings`, `pump_all`) |
| Marca de panel activo | 13 (file_panel) |
| Testing de core (nav, workspace, templates, config) | 2, 3, 4, 5, 6, 7, 8 |

**Diferido explícitamente (NO en 2A):** íconos reales (2B), i18n (2C), temas (2C),
drag&drop SO, ops de archivo, tamaño de carpeta, menú contextual, árbol expandible
real, filtro de texto por panel (campo reservado), UI completa de Configuración,
botones con texto, breadcrumb clicable fino (en 2A muestra la ruta).

**Notas de riesgo (API de egui_dock 0.19.1 / egui 0.34.3 — verificar contra fuente):**
- `to_dock_state` / `build_into` (Tarea 10): reconstruir un árbol arbitrario con
  `split_left/right/above/below`. La estrategia pre-orden está descrita; ajustar
  nombres reales y verificar que los ids no se dupliquen.
- `push_to_focused_leaf` (Tarea 14) para agregar un tab: confirmar nombre real.
- `iter_all_tabs` (Tarea 10): confirmar que existe para recolectar ids; si no, usar
  el `SerializableDockLayout` del workspace como fuente de verdad y no leer del
  DockState.
- `i.pointer.button_pressed(PointerButton::Extra1/Extra2)` (Tarea 12): confirmar el
  método real en `InputState`/`PointerState` de egui 0.34.3 (puede ser
  `i.pointer.button_pressed(btn)` o leer eventos `PointerButton`).
- `eframe::App::save` (Tarea 12): confirmar firma; eframe la llama al cerrar.
- `egui::Panel::top` / `SidePanel::left` con `show_inside` (Tarea 14): confirmar que
  `Panel::top` existe (como `Panel::bottom` en Fase 1) y que `SidePanel` tiene
  `show_inside`.
- `menu_button` / `close_menu` (Tarea 15): API de menús de egui 0.34.3.
- Persistir el layout tras un reacomodo manual del usuario (arrastrar tabs): en 2A
  se persiste el `SerializableDockLayout` del `Workspace`, que NO se actualiza
  automáticamente si el usuario arrastra tabs en egui_dock. Leer el `DockState` de
  vuelta a `SerializableDockLayout` (para capturar reacomodos manuales) se marca
  como mejora; si es viable con la API, hacerlo en la Tarea 13; si no, documentar
  que en 2A se persiste el layout de la última plantilla aplicada + cambios de
  carpeta, y el reacomodo manual fino se persistirá en un pulido posterior.
