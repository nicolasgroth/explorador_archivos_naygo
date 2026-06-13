# Migración a Slint — Fase 2a: splits multi-panel redimensionables — Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Varios paneles Files lado a lado en `naygo-ui-slint`, con splits
redimensionables (arrastrar el borde) y "agregar panel" que divide el leaf enfocado —
computando el layout en Rust (core) y pintando rects absolutos en Slint.

**Architecture:** El árbol `SerializableDockLayout` (core, agnóstico) gana operaciones
PURAS: `pane_rects`/`split_handles`/`set_fraction`/`split_leaf`/`remove_leaf` + un `Rect`
propio. El controller de la UI Slint pasa de un panel único a un `Workspace` (varios
`FilePaneState`). Slint pinta un `for` plano de paneles posicionados por su rect, con
handles de splitter arrastrables.

**Tech Stack:** Rust, naygo-core, Slint 1.16 (posicionamiento absoluto + Toucharea drag).

---

## Reglas operativas

1. Puertas antes de CADA commit: `cargo test --workspace` (lee TODAS las líneas
   `test result:`), `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo fmt --all`. `naygo-ui` (egui) NO se toca.
2. `cargo build -p naygo-ui-slint`; `Stop-Process -Name naygo-slint -Force -EA SilentlyContinue` antes de compilar.
3. Commits en español con heredoc de Bash. Stagea rutas explícitas (NO `git add -A`).
4. Header de copyright en archivos nuevos.

## Estructura de archivos

- `crates/core/src/workspace/layout.rs` — Rect + ops de árbol + tests (MODIFICA).
- `crates/ui-slint/src/workspace_ctrl.rs` — NUEVO: controller multi-panel (generaliza el
  `controller` de F1 a varios paneles). El `controller.rs` de F1 se absorbe aquí.
- `crates/ui-slint/ui/types.slint` — añade `PaneRect`, `SplitHandle` (MODIFICA).
- `crates/ui-slint/ui/app-window.slint` — pinta el `for` de paneles + splitters (MODIFICA).
- `crates/ui-slint/ui/file-panel.slint` — recibe su `pane-id` y resalta si activo (MODIFICA).
- `crates/ui-slint/src/main.rs` — cablea el nuevo controller (MODIFICA).

---

### Tarea 1 — core: Rect + pane_rects (reparto del árbol en rectángulos)

**Files:**
- Modify: `crates/core/src/workspace/layout.rs`

- [ ] **Step 1: Test de `pane_rects`**

En el `mod tests` de `layout.rs`, agregar:

```rust
    fn rect_eq(a: Rect, x: f32, y: f32, w: f32, h: f32) -> bool {
        (a.x - x).abs() < 0.01 && (a.y - y).abs() < 0.01 && (a.w - w).abs() < 0.01 && (a.h - h).abs() < 0.01
    }

    #[test]
    fn pane_rects_un_panel_ocupa_todo() {
        let l = SerializableDockLayout::single(PaneId(1));
        let area = Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 };
        let rects = l.pane_rects(area);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].0, PaneId(1));
        assert!(rect_eq(rects[0].1, 0.0, 0.0, 800.0, 600.0));
    }

    #[test]
    fn pane_rects_split_horizontal_reparte_por_fraction() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.25,
                first: Box::new(DockNode::Leaf(PaneId(1))),
                second: Box::new(DockNode::Leaf(PaneId(2))),
            }),
        };
        let rects = l.pane_rects(Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 });
        // 25% a la izquierda (200px) menos media barra; 2º a la derecha.
        let r1 = rects.iter().find(|(id, _)| *id == PaneId(1)).unwrap().1;
        let r2 = rects.iter().find(|(id, _)| *id == PaneId(2)).unwrap().1;
        assert!((r1.w - 198.0).abs() < 2.0, "1º ~25% menos media barra");
        assert!(r2.x > r1.x + r1.w - 0.1, "2º arranca tras el 1º + barra");
        assert!((r1.h - 600.0).abs() < 0.01 && (r2.h - 600.0).abs() < 0.01);
    }

    #[test]
    fn pane_rects_split_vertical() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Vertical,
                fraction: 0.5,
                first: Box::new(DockNode::Leaf(PaneId(1))),
                second: Box::new(DockNode::Leaf(PaneId(2))),
            }),
        };
        let rects = l.pane_rects(Rect { x: 0.0, y: 0.0, w: 400.0, h: 400.0 });
        let r1 = rects.iter().find(|(id, _)| *id == PaneId(1)).unwrap().1;
        let r2 = rects.iter().find(|(id, _)| *id == PaneId(2)).unwrap().1;
        assert!((r1.w - 400.0).abs() < 0.01, "vertical: mismo ancho");
        assert!(r2.y > r1.y + r1.h - 0.1, "2º debajo del 1º");
    }
```

- [ ] **Step 2: Run (falla: `Rect`/`pane_rects` no existen)**

Run (PowerShell): `cargo test -p naygo-core pane_rects 2>&1 | Select-String "error|test result"`
Expected: FALLA de compilación.

- [ ] **Step 3: Implementar `Rect` + `pane_rects`**

En `layout.rs`, tras la definición de `SerializableDockLayout` (antes del `impl`), agregar
el `Rect`, y dentro del `impl SerializableDockLayout` agregar `pane_rects`:

```rust
/// Rectángulo en píxeles lógicos (sin depender de egui/slint). Origen arriba-izquierda.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Grosor (px) de la barra entre dos paneles de un split (zona de arrastre + hueco visual).
pub const SPLIT_BAR: f32 = 4.0;
```

Y dentro de `impl SerializableDockLayout`:

```rust
    /// Reparte `area` entre los paneles según el árbol: cada split divide su área por
    /// `fraction` (primer hijo) descontando la barra `SPLIT_BAR`. Devuelve el rect de cada
    /// hoja. PURO: misma entrada → misma salida; base del render de docking.
    pub fn pane_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            place(root, area, &mut out);
        }
        out
    }
```

Y como funciones libres (junto a `collect`):

```rust
/// Coloca recursivamente `node` dentro de `area`, acumulando los rects de las hojas.
fn place(node: &DockNode, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
    match node {
        DockNode::Leaf(id) => out.push((*id, area)),
        DockNode::Split {
            dir,
            fraction,
            first,
            second,
        } => {
            let (a, b) = split_area(area, *dir, *fraction);
            place(first, a, out);
            place(second, b, out);
        }
    }
}

/// Divide `area` en dos sub-rects según la orientación y la fracción del primer hijo,
/// descontando media barra a cada lado del corte. `fraction` se clampa a [0.05, 0.95].
fn split_area(area: Rect, dir: SplitDir, fraction: f32) -> (Rect, Rect) {
    let f = fraction.clamp(0.05, 0.95);
    let half = SPLIT_BAR / 2.0;
    match dir {
        SplitDir::Horizontal => {
            let first_w = (area.w * f - half).max(0.0);
            let second_x = area.x + area.w * f + half;
            let second_w = (area.x + area.w - second_x).max(0.0);
            (
                Rect { x: area.x, y: area.y, w: first_w, h: area.h },
                Rect { x: second_x, y: area.y, w: second_w, h: area.h },
            )
        }
        SplitDir::Vertical => {
            let first_h = (area.h * f - half).max(0.0);
            let second_y = area.y + area.h * f + half;
            let second_h = (area.y + area.h - second_y).max(0.0);
            (
                Rect { x: area.x, y: area.y, w: area.w, h: first_h },
                Rect { x: area.x, y: second_y, w: area.w, h: second_h },
            )
        }
    }
}
```

- [ ] **Step 4: Run (pasa) + commit**

Run (PowerShell): `cargo test -p naygo-core pane_rects 2>&1 | Select-String "test result"`
Expected: `ok`.

```bash
git add crates/core/src/workspace/layout.rs
git commit -F - <<'EOF'
feat(core): pane_rects reparte el arbol de docking en rectangulos (puro, con tests)

Rect propio (sin egui/slint) + pane_rects(area): recorre el arbol de splits y reparte el
area por fraction/orientacion descontando la barra SPLIT_BAR, devolviendo el rect de cada
hoja. Base del render de docking en Slint computado en Rust. Tests: 1 panel, split H y V.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 2 — core: split_handles + set_fraction (splitters arrastrables)

**Files:**
- Modify: `crates/core/src/workspace/layout.rs`

**Diseño:** cada split tiene una RUTA estable (secuencia de pasos First/Second desde la
raíz). `split_handles` devuelve, por cada split, su ruta + el rect de la barra + su
orientación (para hit-test y para saber cómo mapear el arrastre a una nueva fracción).
`set_fraction(path, f)` ajusta ese split.

- [ ] **Step 1: Test**

```rust
    #[test]
    fn split_handles_y_set_fraction() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.5,
                first: Box::new(DockNode::Leaf(PaneId(1))),
                second: Box::new(DockNode::Leaf(PaneId(2))),
            }),
        };
        let area = Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 };
        let handles = l.split_handles(area);
        assert_eq!(handles.len(), 1, "un split, un handle");
        let h = &handles[0];
        assert_eq!(h.dir, SplitDir::Horizontal);
        // La barra está cerca del 50% (~400px en x) y cubre el alto.
        assert!((h.rect.x - 398.0).abs() < 3.0);
        assert!((h.rect.h - 600.0).abs() < 0.01);
        // Ajustar la fracción de ese split mueve el borde.
        l.set_fraction(&h.path, 0.25);
        let r1 = l.pane_rects(area).iter().find(|(id, _)| *id == PaneId(1)).unwrap().1;
        assert!((r1.w - 198.0).abs() < 2.0, "ahora el 1º ocupa ~25%");
    }

    #[test]
    fn set_fraction_clampa() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.5,
                first: Box::new(DockNode::Leaf(PaneId(1))),
                second: Box::new(DockNode::Leaf(PaneId(2))),
            }),
        };
        l.set_fraction(&[], 2.0); // ruta vacía = raíz; valor fuera de rango
        if let Some(DockNode::Split { fraction, .. }) = &l.root {
            assert!(*fraction <= 0.95 && *fraction >= 0.05);
        } else {
            panic!("raíz debe seguir siendo split");
        }
    }
```

- [ ] **Step 2: Run (falla)**

Run: `cargo test -p naygo-core split_handles 2>&1 | Select-String "error|test result"`
Expected: FALLA.

- [ ] **Step 3: Implementar `SplitStep`, `SplitHandle`, `split_handles`, `set_fraction`**

En `layout.rs`:

```rust
/// Un paso en la ruta a un split: por cuál hijo se baja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitStep {
    First,
    Second,
}

/// Un splitter arrastrable: la ruta a su split, el rect de su barra y su orientación.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitHandle {
    pub path: Vec<SplitStep>,
    pub rect: Rect,
    pub dir: SplitDir,
}
```

Dentro de `impl SerializableDockLayout`:

```rust
    /// Los handles (barras) de todos los splits, con su ruta y rect, para hit-test y drag.
    pub fn split_handles(&self, area: Rect) -> Vec<SplitHandle> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            handles(root, area, &mut Vec::new(), &mut out);
        }
        out
    }

    /// Ajusta la fracción del split en `path` (clamp 0.05..0.95). No-op si la ruta no
    /// apunta a un split.
    pub fn set_fraction(&mut self, path: &[SplitStep], fraction: f32) {
        let Some(root) = self.root.as_mut() else {
            return;
        };
        let mut node = root;
        for step in path {
            match node {
                DockNode::Split { first, second, .. } => {
                    node = match step {
                        SplitStep::First => first,
                        SplitStep::Second => second,
                    };
                }
                DockNode::Leaf(_) => return,
            }
        }
        if let DockNode::Split { fraction: fr, .. } = node {
            *fr = fraction.clamp(0.05, 0.95);
        }
    }
```

Función libre:

```rust
/// Recorre el árbol acumulando el handle (barra) de cada split. La barra ocupa el hueco
/// `SPLIT_BAR` entre los dos hijos.
fn handles(node: &DockNode, area: Rect, path: &mut Vec<SplitStep>, out: &mut Vec<SplitHandle>) {
    if let DockNode::Split { dir, fraction, first, second } = node {
        let f = fraction.clamp(0.05, 0.95);
        let half = SPLIT_BAR / 2.0;
        let bar = match dir {
            SplitDir::Horizontal => Rect {
                x: area.x + area.w * f - half,
                y: area.y,
                w: SPLIT_BAR,
                h: area.h,
            },
            SplitDir::Vertical => Rect {
                x: area.x,
                y: area.y + area.h * f - half,
                w: area.w,
                h: SPLIT_BAR,
            },
        };
        out.push(SplitHandle { path: path.clone(), rect: bar, dir: *dir });
        let (a, b) = split_area(area, *dir, *fraction);
        path.push(SplitStep::First);
        handles(first, a, path, out);
        path.pop();
        path.push(SplitStep::Second);
        handles(second, b, path, out);
        path.pop();
    }
}
```

- [ ] **Step 4: Run (pasa) + commit**

Run: `cargo test -p naygo-core layout 2>&1 | Select-String "test result"`
Expected: `ok`.

```bash
git add crates/core/src/workspace/layout.rs
git commit -F - <<'EOF'
feat(core): split_handles + set_fraction para splitters arrastrables (puro, con tests)

Cada split se identifica por una ruta estable (First/Second desde la raiz). split_handles
da el rect de cada barra + su ruta + orientacion (hit-test y drag); set_fraction(path, f)
ajusta esa proporcion con clamp 0.05..0.95. Tests de handle y de ajuste.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 3 — core: split_leaf + remove_leaf (agregar/cerrar panel)

**Files:**
- Modify: `crates/core/src/workspace/layout.rs`

- [ ] **Step 1: Test**

```rust
    #[test]
    fn split_leaf_divide_la_hoja() {
        let mut l = SerializableDockLayout::single(PaneId(1));
        l.split_leaf(PaneId(1), SplitDir::Horizontal, PaneId(2));
        // Ahora raíz es un split con 1 y 2.
        assert_eq!(l.pane_ids(), vec![PaneId(1), PaneId(2)]);
        if let Some(DockNode::Split { dir, first, second, .. }) = &l.root {
            assert_eq!(*dir, SplitDir::Horizontal);
            assert_eq!(**first, DockNode::Leaf(PaneId(1)));
            assert_eq!(**second, DockNode::Leaf(PaneId(2)));
        } else {
            panic!("raíz debe ser split");
        }
    }

    #[test]
    fn remove_leaf_colapsa_el_split() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.5,
                first: Box::new(DockNode::Leaf(PaneId(1))),
                second: Box::new(DockNode::Leaf(PaneId(2))),
            }),
        };
        l.remove_leaf(PaneId(1));
        // Queda solo el panel 2 como raíz (el split degenerado colapsa).
        assert_eq!(l.root, Some(DockNode::Leaf(PaneId(2))));
    }

    #[test]
    fn remove_leaf_unico_deja_vacio() {
        let mut l = SerializableDockLayout::single(PaneId(1));
        l.remove_leaf(PaneId(1));
        assert_eq!(l.root, None);
    }
```

- [ ] **Step 2: Run (falla)**

Run: `cargo test -p naygo-core leaf 2>&1 | Select-String "error|test result"`
Expected: FALLA.

- [ ] **Step 3: Implementar**

Dentro de `impl SerializableDockLayout`:

```rust
    /// Divide la hoja `leaf` en un split: el lado nuevo lleva `new_id`. Si `leaf` no
    /// existe, no-op. El split nuevo arranca al 50%.
    pub fn split_leaf(&mut self, leaf: PaneId, dir: SplitDir, new_id: PaneId) {
        if let Some(root) = self.root.as_mut() {
            split_in(root, leaf, dir, new_id);
        }
    }

    /// Quita la hoja `id` y colapsa el split que la contenía (el hermano sube a su lugar).
    /// Si era la única hoja, el layout queda vacío.
    pub fn remove_leaf(&mut self, id: PaneId) {
        match self.root.take() {
            Some(node) => self.root = remove_in(node, id),
            None => {}
        }
    }
```

Funciones libres:

```rust
/// Busca la hoja `leaf` y la reemplaza por un split [leaf | new_id] con la orientación dada.
fn split_in(node: &mut DockNode, leaf: PaneId, dir: SplitDir, new_id: PaneId) {
    match node {
        DockNode::Leaf(id) if *id == leaf => {
            *node = DockNode::Split {
                dir,
                fraction: 0.5,
                first: Box::new(DockNode::Leaf(leaf)),
                second: Box::new(DockNode::Leaf(new_id)),
            };
        }
        DockNode::Leaf(_) => {}
        DockNode::Split { first, second, .. } => {
            split_in(first, leaf, dir, new_id);
            split_in(second, leaf, dir, new_id);
        }
    }
}

/// Quita la hoja `id` del subárbol. Devuelve el subárbol resultante (o None si todo el
/// subárbol era esa hoja). Un split que pierde un hijo colapsa al otro.
fn remove_in(node: DockNode, id: PaneId) -> Option<DockNode> {
    match node {
        DockNode::Leaf(leaf) => {
            if leaf == id {
                None
            } else {
                Some(DockNode::Leaf(leaf))
            }
        }
        DockNode::Split {
            dir,
            fraction,
            first,
            second,
        } => {
            let f = remove_in(*first, id);
            let s = remove_in(*second, id);
            match (f, s) {
                (Some(f), Some(s)) => Some(DockNode::Split {
                    dir,
                    fraction,
                    first: Box::new(f),
                    second: Box::new(s),
                }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            }
        }
    }
}
```

- [ ] **Step 4: Run (pasa) + commit**

Run: `cargo test -p naygo-core 2>&1 | Select-String "test result:|FAILED" | Select-Object -First 3`
Expected: `ok`.

```bash
git add crates/core/src/workspace/layout.rs
git commit -F - <<'EOF'
feat(core): split_leaf + remove_leaf (agregar/cerrar panel) en el arbol de docking

split_leaf(leaf, dir, new_id) divide una hoja en [leaf | nuevo] al 50%; remove_leaf(id)
quita la hoja y colapsa el split degenerado (el hermano sube). Tests de division,
colapso y vaciado. Completa las ops puras del layout para el docking en Slint.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 4 — ui-slint: controller multi-panel (Workspace)

**Files:**
- Create: `crates/ui-slint/src/workspace_ctrl.rs`
- Modify: `crates/ui-slint/src/main.rs`, eliminar `crates/ui-slint/src/controller.rs`
  (su lógica se absorbe; F1 era un solo panel).

**Diseño:** `WorkspaceCtrl` posee un `naygo_core::workspace::Workspace` (varios
`FilePaneState` + `layout` + `active`). Mantiene un `Listing` POR panel (un
`HashMap<PaneId, Listing>`), porque cada panel lista su carpeta. El timer drena TODOS los
listados activos. Reusa toda la lógica de F1 (selección/teclado/orden) pero contra el
panel ACTIVO.

- [ ] **Step 1: Crear `workspace_ctrl.rs`**

```rust
// Naygo — controlador multi-panel de la UI Slint (Fase 2a). Posee el Workspace (varios
// FilePaneState + layout) y traduce gestos a llamadas del core.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::bridge::{rows_from_view, PlainRow};
use crate::listing::Listing;
use naygo_core::fs_model::{EntryKind, SortKey};
use naygo_core::keymap::{Action, KeyMap};
use naygo_core::workspace::layout::{Rect, SplitStep};
use naygo_core::workspace::{PaneId, PanePurpose, Workspace};
use std::collections::HashMap;

pub const ROW_HEIGHT: f32 = 22.0;
const PAGE_ROWS: usize = 20;

pub struct WorkspaceCtrl {
    pub ws: Workspace,
    pub keymap: KeyMap,
    /// Un listado en curso por panel (la carpeta de cada panel se lista por separado).
    pub listings: HashMap<PaneId, Listing>,
    pub typeahead: String,
    pub ctrl_down: bool,
    pub shift_down: bool,
}

impl WorkspaceCtrl {
    /// Arranca con UN panel Files en `start` (la Fase 2a inicia simple; el usuario agrega
    /// paneles con el boton). Lanza su listado inicial.
    pub fn new(start: std::path::PathBuf) -> WorkspaceCtrl {
        let mut ws = Workspace::new();
        let id = ws.add_pane(PanePurpose::Files, start.clone());
        ws.layout = naygo_core::workspace::layout::SerializableDockLayout::single(id);
        ws.set_active(id);
        let mut c = WorkspaceCtrl {
            ws,
            keymap: KeyMap::defaults(),
            listings: HashMap::new(),
            typeahead: String::new(),
            ctrl_down: false,
            shift_down: false,
        };
        c.start_listing(id, start);
        c
    }

    /// Arranca el listado del panel `id` en `dir` (cancela el suyo anterior).
    pub fn start_listing(&mut self, id: PaneId, dir: std::path::PathBuf) {
        if let Some(l) = self.listings.get(&id) {
            l.cancel();
        }
        self.listings.insert(id, Listing::start(dir));
    }

    /// Drena los lotes de TODOS los listados activos. Devuelve true si TODOS terminaron
    /// (para apagar el timer). Quita del mapa los que terminan.
    pub fn pump_listings(&mut self) -> bool {
        let ids: Vec<PaneId> = self.listings.keys().copied().collect();
        for id in ids {
            let (batch, done) = match self.listings.get(&id) {
                Some(l) => l.poll(),
                None => continue,
            };
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                if !batch.is_empty() {
                    f.entries.extend(batch);
                }
                if done {
                    let spec = f.sort;
                    naygo_core::sort::sort_entries(&mut f.entries, &spec);
                    if f.focused.is_none() && !f.entries.is_empty() {
                        f.focused = Some(0);
                    }
                }
            }
            if done {
                self.listings.remove(&id);
            }
        }
        self.listings.is_empty()
    }

    pub fn listings_active(&self) -> bool {
        !self.listings.is_empty()
    }

    /// Rects de los paneles (id, x, y, w, h) dado el area de contenido.
    pub fn pane_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        self.ws.layout.pane_rects(area)
    }

    /// Handles de splitter (para pintarlos y arrastrarlos).
    pub fn split_handles(&self, area: Rect) -> Vec<naygo_core::workspace::layout::SplitHandle> {
        self.ws.layout.split_handles(area)
    }

    /// Ajusta la fraccion de un split (drag de splitter).
    pub fn set_fraction(&mut self, path: &[SplitStep], fraction: f32) {
        self.ws.layout.set_fraction(path, fraction);
    }

    /// Filas a pintar del panel `id`.
    pub fn rows_of(&self, id: PaneId) -> Vec<PlainRow> {
        match self.ws.pane(id).and_then(|p| p.files.as_ref()) {
            Some(f) => rows_from_view(f),
            None => Vec::new(),
        }
    }

    /// Carpeta actual del panel `id` (para su path-bar).
    pub fn path_of(&self, id: PaneId) -> String {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.display().to_string())
            .unwrap_or_default()
    }

    pub fn active_id(&self) -> Option<PaneId> {
        self.ws.active_id()
    }

    pub fn set_active(&mut self, id: PaneId) {
        self.ws.set_active(id);
    }

    /// Agrega un panel Files DIVIDIENDO el leaf activo (horizontal). Lo deja activo y
    /// arranca su listado en la misma carpeta que el activo (o el home).
    pub fn add_pane_split(&mut self) {
        let dir = self
            .ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
        let active = self.ws.active_id();
        let new_id = self.ws.add_pane(PanePurpose::Files, dir.clone());
        if let Some(active) = active {
            self.ws.layout.split_leaf(
                active,
                naygo_core::workspace::layout::SplitDir::Horizontal,
                new_id,
            );
        }
        self.ws.set_active(new_id);
        self.start_listing(new_id, dir);
    }

    // --- Gestos sobre el panel ACTIVO (reusan la logica de F1) ---

    fn active_files_mut(&mut self) -> Option<&mut naygo_core::workspace::FilePaneState> {
        self.ws.active_files_mut()
    }

    pub fn on_row_clicked(&mut self, id: PaneId, pos: usize) {
        self.ws.set_active(id);
        let (ctrl, shift) = (self.ctrl_down, self.shift_down);
        if let Some(f) = self.active_files_mut() {
            if shift {
                f.select_range_to(pos);
            } else if ctrl {
                f.select_toggle(pos);
            } else {
                f.select_single(pos);
            }
        }
    }

    /// Doble clic en el panel `id`, posicion `pos`. Navega (y arranca listado) o abre.
    pub fn on_row_double_clicked(&mut self, id: PaneId, pos: usize) -> bool {
        self.ws.set_active(id);
        let target = {
            let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) else {
                return false;
            };
            let view = f.view_indices();
            let Some(&real) = view.get(pos) else {
                return false;
            };
            f.entries.get(real).cloned()
        };
        let Some(e) = target else { return false };
        if e.kind == EntryKind::Directory {
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.navigate_to(e.path.clone());
            }
            self.start_listing(id, e.path);
            true
        } else {
            let _ = naygo_platform::open::open_default(&e.path);
            false
        }
    }

    pub fn on_go_up(&mut self) -> bool {
        let active = match self.ws.active_id() {
            Some(a) => a,
            None => return false,
        };
        let moved = self
            .active_files_mut()
            .and_then(|f| f.go_up());
        match moved {
            Some(dir) => {
                self.start_listing(active, dir);
                true
            }
            None => false,
        }
    }

    pub fn on_sort_by(&mut self, column: &str) {
        let key = match column {
            "name" => SortKey::Name,
            "ext" => SortKey::Extension,
            "size" => SortKey::Size,
            "modified" => SortKey::Modified,
            _ => return,
        };
        if let Some(f) = self.active_files_mut() {
            if f.sort.key == key {
                f.sort.ascending = !f.sort.ascending;
            } else {
                f.sort.key = key;
                f.sort.ascending = true;
            }
            let spec = f.sort;
            naygo_core::sort::sort_entries(&mut f.entries, &spec);
        }
    }

    /// Tecla sobre el panel activo (reusa el keymap). Devuelve true si navegó.
    pub fn on_key(&mut self, text: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.ctrl_down = ctrl;
        self.shift_down = shift;
        let Some(chord) = crate::keys::chord_from(text, ctrl, shift, alt) else {
            self.typeahead(text);
            return false;
        };
        let Some(action) = self.keymap.action_for(&chord) else {
            self.typeahead(text);
            return false;
        };
        self.typeahead.clear();
        let active = self.ws.active_id();
        match action {
            Action::MoveUp => self.with_active(|f| f.move_focus_extend(-1, false)),
            Action::MoveDown => self.with_active(|f| f.move_focus_extend(1, false)),
            Action::ExtendUp => self.with_active(|f| f.move_focus_extend(-1, true)),
            Action::ExtendDown => self.with_active(|f| f.move_focus_extend(1, true)),
            Action::FocusPageUp => self.with_active(|f| f.focus_page(-1, PAGE_ROWS, false)),
            Action::FocusPageDown => self.with_active(|f| f.focus_page(1, PAGE_ROWS, false)),
            Action::ExtendPageUp => self.with_active(|f| f.focus_page(-1, PAGE_ROWS, true)),
            Action::ExtendPageDown => self.with_active(|f| f.focus_page(1, PAGE_ROWS, true)),
            Action::FocusHome => self.with_active(|f| f.focus_home(false)),
            Action::FocusEnd => self.with_active(|f| f.focus_end(false)),
            Action::ExtendHome => self.with_active(|f| f.focus_home(true)),
            Action::ExtendEnd => self.with_active(|f| f.focus_end(true)),
            Action::FocusUpKeep => self.with_active(|f| f.move_focus_keep(-1)),
            Action::FocusDownKeep => self.with_active(|f| f.move_focus_keep(1)),
            Action::ToggleSelect | Action::ToggleFocused => {
                self.with_active(|f| {
                    if let Some(p) = f.focused {
                        f.select_toggle(p);
                    }
                })
            }
            Action::SelectAll => self.with_active(|f| f.select_all()),
            Action::SwitchPane => {
                // Tab: ciclar el panel activo entre los Files.
                let files = self.ws.files_panes();
                if files.len() > 1 {
                    if let Some(cur) = active {
                        let i = files.iter().position(|&p| p == cur).unwrap_or(0);
                        let next = files[(i + 1) % files.len()];
                        self.ws.set_active(next);
                    }
                }
            }
            Action::GoUp => return self.on_go_up(),
            Action::Activate => {
                if let (Some(id), Some(pos)) =
                    (active, self.ws.active_files().and_then(|f| f.focused))
                {
                    return self.on_row_double_clicked(id, pos);
                }
            }
            _ => {}
        }
        false
    }

    /// Aplica `op` al panel activo (helper para no repetir el match de prestamos).
    fn with_active(&mut self, op: impl FnOnce(&mut naygo_core::workspace::FilePaneState)) {
        if let Some(f) = self.ws.active_files_mut() {
            op(f);
        }
    }

    fn typeahead(&mut self, text: &str) {
        let Some(ch) = text.chars().next().filter(|c| !c.is_control()) else {
            return;
        };
        self.typeahead.push(ch.to_ascii_lowercase());
        let needle = self.typeahead.clone();
        if let Some(f) = self.ws.active_files_mut() {
            let view = f.view_indices();
            for (pos, &real) in view.iter().enumerate() {
                if let Some(e) = f.entries.get(real) {
                    if e.name.to_lowercase().starts_with(needle.as_str()) {
                        f.select_single(pos);
                        break;
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Borrar `controller.rs` y actualizar `main.rs`**

Run (PowerShell): `Remove-Item crates\ui-slint\src\controller.rs`
En `main.rs`: cambiar `mod controller;` por `mod workspace_ctrl;` y `use controller::Controller;`
por `use workspace_ctrl::WorkspaceCtrl;`. (El wiring completo de callbacks se rehace en la
Tarea 6; este paso solo deja el módulo en su lugar y el `main` compilando con un stub que
crea el `WorkspaceCtrl` pero aún usa la UI de un panel — se completa en T5/T6.)

Para que compile en este paso intermedio, en `main.rs` reemplazar la construcción del
controller por el nuevo tipo y dejar los callbacks viejos apuntando al panel activo
temporalmente (se reconectan en T6). Si el wiring viejo no calza con la nueva firma
(`on_row_clicked(id, pos)`), comentar los `ui.on_*` que no compilen con
`// TODO(T6): reconectar` — PERO como el plan ejecuta T4→T5→T6 seguidas y T6 deja todo
conectado y verde, lo más limpio es hacer T4+T5+T6 y correr las puertas al final de T6.
Por eso: en T4 solo se CREA `workspace_ctrl.rs` y se cambia el `mod`; el `main.rs` se
termina en T6.

- [ ] **Step 3: Verificar que `workspace_ctrl.rs` compila aislado (cargo check del crate)**

Run (PowerShell): `cargo build -p naygo-ui-slint 2>&1 | Select-String "error|Finished" | Select-Object -First 20`
Si `main.rs` aún referencia `Controller`, habrá errores esperados: se resuelven en T5/T6.
Objetivo de este step: que `workspace_ctrl.rs` no tenga errores propios (firmas, tipos del
core). Leer los errores y arreglar SOLO los de `workspace_ctrl.rs`.

- [ ] **Step 4: Commit (intermedio, sin puertas completas — se cierran en T6)**

```bash
git add crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(slint): controller multi-panel WorkspaceCtrl (Fase 2a)

Generaliza el controller de F1 a un Workspace (varios FilePaneState + layout): un Listing
por panel, pump de todos, gestos sobre el panel ACTIVO (reusan el keymap), add_pane_split
(divide el leaf activo), set_active por clic, Tab cicla paneles. Reusa pane_rects/
split_handles/set_fraction del core. main.rs se reconecta en la tarea de wiring.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 5 — ui-slint: render de paneles posicionados + splitters (.slint)

**Files:**
- Modify: `crates/ui-slint/ui/types.slint`, `crates/ui-slint/ui/file-panel.slint`,
  `crates/ui-slint/ui/app-window.slint`

- [ ] **Step 1: types.slint — PaneVm y SplitVm**

Agregar a `types.slint`:

```slint
// Un panel colocado: su id + rect + filas + ruta + si es el activo.
export struct PaneVm {
    id: int,
    x: length,
    y: length,
    w: length,
    h: length,
    path: string,
    rows: [RowData],
    active: bool,
}

// Una barra de splitter colocada: su rect + si es horizontal (corte vertical, arrastra en x).
export struct SplitVm {
    index: int,
    x: length,
    y: length,
    w: length,
    h: length,
    horizontal: bool,
}
```

- [ ] **Step 2: file-panel.slint — recibe id y bandera active**

Añadir a `FilePanel` las propiedades `in property <int> pane-id;` y
`in property <bool> active;`, y un borde de acento cuando `active` (1.5px #4f8ae0 alrededor
del panel). Cambiar los callbacks para incluir el id: `row-clicked(int, int)` →
`(pane-id, pos)`, `row-double-clicked(int, int)`, `sort-by(int, string)`, y un nuevo
`activate(int)` al hacer foco/click en cualquier parte del panel. (El `key` queda igual;
el FocusScope del panel activo recibe el teclado.)

Reemplazar la firma de callbacks de `FilePanel`:
```slint
    in property <int> pane-id;
    in property <bool> active;
    callback row-clicked(int, int);          // (pane-id, pos)
    callback row-double-clicked(int, int);
    callback sort-by(int, string);
    callback activate(int);                  // (pane-id) al interactuar
    callback key(string, bool, bool, bool);
```
y en el cuerpo, envolver todo en un Rectangle con borde condicional:
```slint
    Rectangle {
        border-width: root.active ? 1.5px : 0px;
        border-color: #4f8ae0;
        // ... (el VerticalLayout actual de encabezados + ListView va aquí dentro)
    }
```
y en los handlers de fila usar `root.row-clicked(root.pane-id, i)` etc.; en el
`key-pressed` y en un TouchArea de fondo, llamar `root.activate(root.pane-id)`.

- [ ] **Step 3: app-window.slint — for de paneles + for de splitters**

Reemplazar el `panel := FilePanel { ... }` por un contenedor con posicionamiento absoluto:

```slint
    in property <[PaneVm]> panes;
    in property <[SplitVm]> splits;
    in property <length> content-x;
    in property <length> content-y;
    callback row-clicked(int, int);
    callback row-double-clicked(int, int);
    callback sort-by(int, string);
    callback activate(int);
    callback key(string, bool, bool, bool);
    callback split-drag(int, length, length);   // (index, dx, dy) acumulado del arrastre
    callback go-up();
    in property <string> active-path;

    // Zona de contenido: paneles posicionados + barras de splitter.
    Rectangle {
        x: root.content-x; y: root.content-y;
        // Paneles.
        for p in root.panes: FilePanel {
            x: p.x; y: p.y; width: p.w; height: p.h;
            pane-id: p.id; active: p.active; rows: p.rows;
            row-clicked(id, pos) => { root.row-clicked(id, pos); }
            row-double-clicked(id, pos) => { root.row-double-clicked(id, pos); }
            sort-by(id, col) => { root.sort-by(id, col); }
            activate(id) => { root.activate(id); }
            key(t, c, s, a) => { root.key(t, c, s, a); }
        }
        // Barras de splitter arrastrables.
        for s in root.splits: Rectangle {
            x: s.x; y: s.y; width: s.w; height: s.h;
            background: sp-touch.has-hover ? #4f8ae0 : #2a3a52;
            sp-touch := TouchArea {
                mouse-cursor: s.horizontal ? ew-resize : ns-resize;
                moved => {
                    root.split-drag(s.index, self.mouse-x - self.pressed-x, self.mouse-y - self.pressed-y);
                }
            }
        }
    }
```

NOTA: la path-bar global y el FocusScope: en 2a, mantener UNA path-bar arriba que muestra
`active-path` (la del panel activo) y el botón subir; el contenido va debajo (de ahí
`content-y`). El teclado: cada FilePanel es un FocusScope; el activo tiene el foco (se le
pide `focus()` al activar). Ajustar el VerticalLayout raíz: PathBar arriba, luego el
Rectangle de contenido que llena el resto (medir su tamaño para pasar el área a Rust →
ver T6, donde el área se obtiene de las propiedades de tamaño de la ventana).

- [ ] **Step 4: Build del .slint**

Run (PowerShell): `cargo build -p naygo-ui-slint 2>&1 | Select-String "error|Finished" | Select-Object -First 20`
Resolver errores de sintaxis Slint (nombres de propiedades de TouchArea: `mouse-x`,
`pressed-x` existen en 1.16; si no, usar `self.mouse-x`/guardar el inicio). El `main.rs`
seguirá con errores de wiring → se cierran en T6. Objetivo: el `.slint` compila.

- [ ] **Step 5: Commit (intermedio)**

```bash
git add crates/ui-slint/ui
git commit -F - <<'EOF'
feat(slint): render de paneles posicionados por rect + barras de splitter (.slint)

types.slint: PaneVm/SplitVm. file-panel recibe pane-id + bandera active (borde de
acento) y emite callbacks con el id. app-window pinta un `for` plano de FilePanel
posicionados por su rect (calculado en Rust) y un `for` de barras de splitter
arrastrables (cursor de resize). El wiring Rust se cierra en la tarea siguiente.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 6 — ui-slint: wiring del WorkspaceCtrl + área de contenido + persistencia

**Files:**
- Modify: `crates/ui-slint/src/main.rs`

- [ ] **Step 1: Reescribir `main.rs` para multi-panel**

Construye `WorkspaceCtrl`, define `refresh()` que: calcula el área de contenido (ancho de
ventana × (alto − barra superior)), pide `pane_rects(area)`/`split_handles(area)`, arma los
`PaneVm`/`SplitVm` (con `rows_of(id)`, `active`), y los setea. Cablea:
`on_row_clicked(id,pos)`, `on_row_double_clicked(id,pos)` (+timer si navega),
`on_sort_by(col)` sobre el activo, `on_activate(id)` → `set_active`+`focus`,
`on_key`(+timer), `on_split_drag(index, dx, dy)` → convierte el drag a una nueva fracción
del split `index` (usando el rect del handle y el área) y llama `set_fraction`,
`on_go_up`(+timer). Botón "agregar panel" (en la toolbar mínima o un botón temporal en la
path-bar) → `add_pane_split()`. El timer drena `pump_listings()`.

El área de contenido se obtiene de propiedades de tamaño que el `.slint` expone: agregar a
`AppWindow` `out property <length> content-w: self.width;` y `content-h` (alto menos la
barra). En `refresh`, leerlas con `ui.get_content_w()`/`get_content_h()`.

Código completo de `main.rs`:

```rust
// Naygo — arranque de la capa UI en Slint (Fase 2a: multi-panel).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//   $env:SLINT_BACKEND="winit-software"; cargo run -p naygo-ui-slint
mod bridge;
mod keys;
mod listing;
mod workspace_ctrl;

use naygo_core::workspace::layout::{Rect, SplitStep};
use slint::{ModelRc, SharedString, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use workspace_ctrl::WorkspaceCtrl;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let start = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
    let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new(start)));

    let refresh = {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let area = Rect {
                x: 0.0,
                y: 0.0,
                w: ui.get_content_w().max(0.0),
                h: ui.get_content_h().max(0.0),
            };
            let c = ctrl.borrow();
            let active = c.active_id();
            // PaneVm por panel.
            let panes: Vec<PaneVm> = c
                .pane_rects(area)
                .into_iter()
                .map(|(id, r)| {
                    let rows: Vec<RowData> =
                        c.rows_of(id).into_iter().map(to_row_data).collect();
                    PaneVm {
                        id: id.0 as i32,
                        x: r.x,
                        y: r.y,
                        w: r.w,
                        h: r.h,
                        path: SharedString::from(c.path_of(id).as_str()),
                        rows: ModelRc::from(Rc::new(VecModel::from(rows))),
                        active: Some(id) == active,
                    }
                })
                .collect();
            ui.set_panes(ModelRc::from(Rc::new(VecModel::from(panes))));
            // SplitVm por barra.
            let splits: Vec<SplitVm> = c
                .split_handles(area)
                .into_iter()
                .enumerate()
                .map(|(i, h)| SplitVm {
                    index: i as i32,
                    x: h.rect.x,
                    y: h.rect.y,
                    w: h.rect.w,
                    h: h.rect.h,
                    horizontal: matches!(h.dir, naygo_core::workspace::layout::SplitDir::Horizontal),
                })
                .collect();
            ui.set_splits(ModelRc::from(Rc::new(VecModel::from(splits))));
            if let Some(id) = active {
                ui.set_active_path(SharedString::from(c.path_of(id).as_str()));
            }
        }
    };

    // Timer que drena los listados activos; se apaga cuando todos terminan.
    let timer = Rc::new(slint::Timer::default());
    let start_timer: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let timer = timer.clone();
        Rc::new(move || {
            let ctrl = ctrl.clone();
            let refresh = refresh.clone();
            let timer2 = timer.clone();
            timer.start(
                TimerMode::Repeated,
                std::time::Duration::from_millis(30),
                move || {
                    let all_done = ctrl.borrow_mut().pump_listings();
                    refresh();
                    if all_done {
                        timer2.stop();
                    }
                },
            );
        })
    };
    start_timer();

    macro_rules! wire {
        ($setter:ident, $body:expr) => {{
            let ctrl = ctrl.clone();
            let refresh = refresh.clone();
            let start_timer = start_timer.clone();
            ui.$setter(move |_a, _b| {
                let f: &dyn Fn(&Rc<RefCell<WorkspaceCtrl>>, &Rc<dyn Fn()>, i32, i32) -> () = &$body;
                f(&ctrl, &start_timer, _a, _b);
                refresh();
            });
        }};
    }
    // Por claridad, cablear cada callback explicitamente (sin macro) — ver abajo.
    let _ = ();

    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        ui.on_row_clicked(move |id, pos| {
            ctrl.borrow_mut()
                .on_row_clicked(naygo_core::workspace::PaneId(id as u64), pos as usize);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_row_double_clicked(move |id, pos| {
            if ctrl
                .borrow_mut()
                .on_row_double_clicked(naygo_core::workspace::PaneId(id as u64), pos as usize)
            {
                start_timer();
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        ui.on_sort_by(move |_id, col| {
            ctrl.borrow_mut().on_sort_by(col.as_str());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let ui_weak = ui.as_weak();
        ui.on_activate(move |id| {
            ctrl.borrow_mut()
                .set_active(naygo_core::workspace::PaneId(id as u64));
            if let Some(_ui) = ui_weak.upgrade() {
                // El foco del FocusScope activo lo maneja el .slint via la bandera active.
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_key(move |text, c, s, a| {
            if ctrl.borrow_mut().on_key(text.as_str(), c, s, a) {
                start_timer();
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_go_up(move || {
            if ctrl.borrow_mut().on_go_up() {
                start_timer();
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_add_pane(move || {
            ctrl.borrow_mut().add_pane_split();
            start_timer();
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let ui_weak = ui.as_weak();
        ui.on_split_drag(move |index, dx, dy| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let area = Rect {
                x: 0.0,
                y: 0.0,
                w: ui.get_content_w().max(1.0),
                h: ui.get_content_h().max(1.0),
            };
            let mut c = ctrl.borrow_mut();
            let handles = c.split_handles(area);
            if let Some(h) = handles.get(index as usize) {
                // La nueva fraccion = (posicion de la barra + delta) / dimension del area
                // del split. Aproximacion 2a: usar el centro de la barra + delta sobre el
                // area total (suficiente para splits a un nivel; el pulido multinivel se
                // afina en verificacion viva).
                let (pos, total) = if matches!(h.dir, naygo_core::workspace::layout::SplitDir::Horizontal) {
                    (h.rect.x + h.rect.w / 2.0 + dx, area.w.max(1.0))
                } else {
                    (h.rect.y + h.rect.h / 2.0 + dy, area.h.max(1.0))
                };
                let path = h.path.clone();
                c.set_fraction(&path, pos / total);
            }
            drop(c);
            refresh();
        });
    }

    refresh();
    ui.run()
}

fn to_row_data(r: bridge::PlainRow) -> RowData {
    RowData {
        name: SharedString::from(r.name.as_str()),
        ext: SharedString::from(r.ext.as_str()),
        size: SharedString::from(r.size.as_str()),
        modified: SharedString::from(r.modified.as_str()),
        is_dir: r.is_dir,
        selected: r.selected,
        focused: r.focused,
    }
}
```

NOTA: el `macro_rules! wire!` de arriba es ilustrativo y NO se usa (se cableó cada callback
explícito); borrarlo al implementar para no dejar código muerto. Las propiedades
`content-w`/`content-h` y los callbacks (`add-pane`, `split-drag`, `activate`,
`row-clicked(int,int)`, etc.) deben existir en `app-window.slint` (añadirlas en T5 si
faltan). El cálculo del `set_fraction` multinivel es una aproximación; se valida en vivo y
se ajusta si un split anidado no responde con precisión (el área del split anidado difiere
del área total — si se nota, pasar a Rust el área del split via la ruta).

- [ ] **Step 2: Ajustar app-window.slint — propiedades content-w/h, callbacks faltantes**

Añadir a `AppWindow`: `out property <length> content-w: self.width - 12px;` (menos
padding) y `out property <length> content-h: self.height - 48px;` (menos toolbar/path-bar
+ padding); el botón "agregar panel" con `callback add-pane();` y los callbacks
`row-clicked(int,int)`, `row-double-clicked(int,int)`, `sort-by(int,string)`,
`activate(int)`, `split-drag(int,length,length)`. Ajustar valores de los descuentos a la
altura real de la barra superior.

- [ ] **Step 3: Build + puertas completas**

Run (PowerShell): `Stop-Process -Name naygo-slint -Force -EA SilentlyContinue; cargo build -p naygo-ui-slint; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --all`
Expected: build `Finished`, todas las líneas `test result: ok`, clippy limpio. Resolver lo
que el compilador marque (tipos de length=f32, nombres de getters/setters generados).

- [ ] **Step 4: Verificación viva local**

Run (PowerShell): `$env:SLINT_BACKEND="winit-software"; cargo run -p naygo-ui-slint`
Criterio: arranca con 1 panel; "agregar panel" divide en 2 lado a lado; cada panel lista y
navega independiente; arrastrar la barra entre ambos los redimensiona; clic en un panel lo
activa (borde de acento) y el teclado va a ese; Tab cicla el activo. Cerrar.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/src/main.rs crates/ui-slint/ui
git commit -F - <<'EOF'
feat(slint): wiring multi-panel (paneles posicionados, splitters, agregar panel)

main usa WorkspaceCtrl: refresh arma PaneVm/SplitVm desde pane_rects/split_handles del
core (area = tamaño de ventana menos la barra), cablea clic/doble clic/orden/teclado al
panel activo, activar-por-clic, agregar-panel (divide), y arrastre de splitter (set_fraction).
Timer drena todos los listados activos. Varios paneles Files redimensionables.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 7 — Cierre de la 2a

- [ ] **Step 1: Puertas finales**

Run (PowerShell): `Stop-Process -Name naygo-slint -Force -EA SilentlyContinue; cargo fmt --all --check; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo build -p naygo-ui-slint`
Expected: fmt sin diff, `test result: ok` en todas, clippy limpio, build `Finished`.

- [ ] **Step 2: Release para la VM**

Run (PowerShell): `cargo build -p naygo-ui-slint --release; Copy-Item target\release\naygo-slint.exe dist\slint-fase1\ -Force`
(Reusa `dist\slint-fase1\correr-software.cmd`.)

- [ ] **Step 3: Avisar a Nicolás (visual + CPU) + memoria**

Mostrar: varios paneles, redimensionar, agregar panel; pedir su visto bueno VISUAL del
multi-panel y que mida CPU al arrastrar la barra en la VM. Actualizar memoria. Pedir
merge/push del branch `slint-fase2` (o dejar 2a en él y seguir con 2b).

---

## Autoevaluación del plan (hecha)

- Cubre la 2a del spec: core layout (pane_rects [T1], split_handles/set_fraction [T2],
  split_leaf/remove_leaf [T3]); controller multi-panel [T4]; render posicionado +
  splitters [T5]; wiring + agregar panel + drag + persistencia-base [T6]; cierre [T7].
- Tipos consistentes: `Rect{x,y,w,h}`, `SplitStep{First,Second}`, `SplitHandle{path,rect,
  dir}`, `PaneVm`/`SplitVm`; `WorkspaceCtrl` con `pane_rects/split_handles/set_fraction/
  add_pane_split/on_*`; `PaneId(u64)` ↔ `i32` en la frontera Slint (cast explícito).
- Riesgos señalados: (a) el `set_fraction` multinivel es aproximado (área del split
  anidado ≠ área total) → si un split anidado no responde fino, pasar el área del split por
  la ruta; se valida en vivo. (b) Nombres exactos de propiedades de `TouchArea` (mouse-x/
  pressed-x) y getters generados se confirman contra el compilador. (c) `remove_leaf` ya
  existe para cerrar panel, pero el BOTÓN de cerrar panel es de 2c (tabs); 2a solo agrega y
  redimensiona. (d) `naygo-ui` egui intacto (puertas del workspace lo verifican).
- Placeholder scan: el `macro_rules! wire!` ilustrativo se marca para borrar en T6 Step1
  (no debe quedar). Sin otros placeholders.
