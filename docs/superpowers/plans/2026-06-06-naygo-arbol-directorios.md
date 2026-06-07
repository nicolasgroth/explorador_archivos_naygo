# Árbol de directorios real — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convertir el panel "Carpetas" (Tree) de Naygo de un esqueleto a un árbol de directorios real: raíces = unidades, ramas colapsables con lazy-load async por streaming (solo carpetas), clic en flecha expande / clic en nombre navega el panel activo, resaltado de la carpeta activa con auto-reveal en cascada.

**Architecture:** El estado del árbol vive en `naygo-core` (módulo `tree`, puro y testeable, sin egui). La enumeración de unidades vive en `naygo-platform` (`drives()` con la API Win32). El listado solo-directorios reutiliza el worker de `core::listing` con un filtro. La UI (`tree_panel.rs`) pinta el árbol recursivamente y acumula acciones; `app.rs` mantiene un `DirTree` por panel Tree + sus workers, los drena cada frame (`pump_tree`) y hace auto-sync con la carpeta del panel activo.

**Tech Stack:** Rust, `naygo-core` / `naygo-platform` / `naygo-ui`, `eframe`/`egui` 0.34.3, crate `windows` 0.62 (Win32). Sin dependencias de terceros nuevas.

**Estado de partida (en `main`, tras fase 2D):**
- `naygo-core`: `fs_model::EntryKind { Directory, File, Other }`; `Entry { name, path, kind, size, modified, created, hidden }`; `icon_kind::{IconKey::{Folder, ParentDir, File(_), Drive(DriveKind), Unknown}, DriveKind::{Fixed, Removable, Network, Optical, Unknown}}`.
- `listing::spawn_listing(dir: PathBuf, token: CancellationToken) -> (Receiver<ListingMsg>, JoinHandle<()>)`; cuerpo en `fn list_into(dir, token, tx)` que llama `fn entry_from_dirent(&DirEntry) -> Entry`. `ListingMsg { Entry(Entry), Error(String), Done, Cancelled }`.
- `workspace`: `Workspace::{active_id() -> Option<PaneId>, set_active(id), pane(id) -> Option<&PaneNode>, pane_mut(id) -> Option<&mut PaneNode>, active_files() -> Option<&FilePaneState>, panes() -> &[PaneNode]}`. `PaneNode { id: PaneId, purpose: PanePurpose, files: Option<FilePaneState>, .. }`. `PanePurpose::{Files, Tree, Inspector}`. `FilePaneState.current_dir: PathBuf`.
- `naygo-platform`: solo `locale.rs` + `hello()`. `Cargo.toml`: `windows = { workspace = true, features = ["Win32_Globalization"] }` bajo `[target.'cfg(windows)'.dependencies]`. Root `[workspace.dependencies] windows = "0.62"`.
- `ui/app.rs`: `NaygoApp { workspace, dock_state, listings: HashMap<PaneId, PaneListing>, settings, .. }`. `PaneListing { rx: Option<Receiver<ListingMsg>>, token: CancellationToken }`. Loop: `logic()` llama `pump_all()` + `handle_input()` + repaint si `any_listing_active()`. `ui()` pinta toolbar + status bar + `DockArea` con `NaygoTabViewer`, luego procesa `pending: Vec<PaneRequest>`.
- `ui/docking.rs`: `PaneRequest { NavigateTo { id, dir }, Activate { id } }`. `NaygoTabViewer { workspace, status, pending, icons, show_parent_entry, i18n }`. En `ui()` despacha `PanePurpose::Tree` a `tree_panel::show(ui, workspace, pending, icons, i18n)`.
- `ui/panes/tree_panel.rs`: esqueleto actual (ubicación + botón subir).
- i18n: `crates/core/src/i18n/{es,en}.json`, objeto JSON plano clave→texto. Ya tiene `pane.tree.title`, `tree.location`, `tree.go_up`. `I18n::t(&self, key) -> &str`.

**Prerequisito:** toolchain Rust en PATH. En PowerShell anteponer `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA usar `2>&1` con cargo en PowerShell. Compilar el binario con `--bin naygo`. Verificar `$LASTEXITCODE`.

**Convenciones (CLAUDE.md):** código en inglés; comentarios/commits en español OK. Cada archivo NUEVO lleva header de 2 líneas:
```
// Naygo — <descripción breve del archivo>
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```
Privilegiar legibilidad. `core` NUNCA importa egui/windows/image. Build limpio + tests + clippy antes de cada commit. Footer de commit obligatorio:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** crear `feat/arbol-directorios` desde `main` antes de la Tarea 1.

**Alcance:** ENTRA: `core::tree` (modelo puro); `platform::drives()`; variante solo-directorios en `core::listing`; reescritura de `tree_panel.rs` (render recursivo, spinner, resaltado modo B, scroll); `HashMap<PaneId, DirTree>` + workers por `(PaneId, PathBuf)` + `pump_tree` + auto-sync en `app.rs`; i18n nuevas. NO ENTRA: refresco manual / watcher; menú contextual; drag&drop; archivos en el árbol; favoritos/breadcrumbs.

---

## Estructura de archivos

```
crates/core/src/
├── tree.rs                # NUEVO: DirTree, TreeNode, NodeState, NodeOutcome, reveal_chain… (puro)
├── listing.rs             # + ListingFilter + spawn_listing_filtered + filtro en list_into
├── lib.rs                 # + pub mod tree; + re-exports
└── i18n/{es,en}.json      # + tree.loading, tree.empty, tree.access_denied

crates/platform/src/
├── drives.rs              # NUEVO: DriveInfo, drives() (Win32 GetLogicalDriveStringsW; stub no-win)
├── lib.rs                 # + pub mod drives;
└── (Cargo.toml)           # + feature Win32_Storage_FileSystem (y System_SystemInformation)

crates/ui/src/
├── tree_actions.rs        # NUEVO: TreeAction { Expand, Collapse, Navigate } (enum simple)
├── panes/tree_panel.rs    # REESCRITO: render recursivo, acciones, spinner, resaltado B, scroll
├── docking.rs             # pasa &mut DirTree + &mut Vec<TreeAction> al tree_panel
├── app.rs                 # + trees: HashMap<PaneId,DirTree> + tree_listings + pump_tree + auto-sync
└── main.rs                # + mod tree_actions;
```

---

## Task 1: `core::tree` — tipos base (TreeNode, NodeState, DirTree) + from_drives

**Files:**
- Create: `crates/core/src/tree.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear el archivo con los tipos y `from_drives`, con tests**

Create `crates/core/src/tree.rs`:

```rust
// Naygo — modelo del árbol de directorios (puro, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Estado de un árbol de carpetas: raíces (unidades) + nodos lazy. Las
//! operaciones son puras y testeables; el I/O (listar hijos, enumerar unidades)
//! ocurre fuera (workers de `listing`, `platform::drives`). El árbol SOLO modela
//! carpetas: nunca contiene archivos.

use crate::icon_kind::DriveKind;
use std::path::{Path, PathBuf};

/// Estado de carga de un nodo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeState {
    /// Aún no expandido (no se han pedido sus hijos).
    Collapsed,
    /// Expandido y listando sus hijos (worker en vuelo).
    Loading,
    /// Expandido con hijos cargados.
    Loaded,
    /// Expandido pero sin subcarpetas.
    Empty,
    /// Falló al listar (permiso, disco caído).
    Error,
}

/// Resultado de un intento de listado de un nodo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeOutcome {
    /// Terminó bien; el nodo queda Loaded o Empty según tenga hijos.
    Done,
    /// Falló.
    Error,
}

/// Un nodo del árbol = una carpeta o una unidad.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeNode {
    pub path: PathBuf,
    /// Nombre visible (último componente, o etiqueta de unidad si es raíz).
    pub name: String,
    /// `Some(..)` si el nodo es una raíz (unidad de disco).
    pub drive_kind: Option<DriveKind>,
    pub expanded: bool,
    pub state: NodeState,
    /// `None` = nunca expandido (lazy). `Some(vec)` = hijos ya cargados.
    pub children: Option<Vec<TreeNode>>,
}

impl TreeNode {
    /// Crea un nodo de carpeta colapsado (lazy).
    pub fn folder(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        TreeNode {
            path,
            name,
            drive_kind: None,
            expanded: false,
            state: NodeState::Collapsed,
            children: None,
        }
    }

    /// Crea un nodo raíz (unidad) colapsado.
    pub fn drive(path: PathBuf, name: String, kind: DriveKind) -> Self {
        TreeNode {
            path,
            name,
            drive_kind: Some(kind),
            expanded: false,
            state: NodeState::Collapsed,
            children: None,
        }
    }
}

/// El árbol completo: raíces (unidades) + carpeta activa + reveal pendiente.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DirTree {
    pub roots: Vec<TreeNode>,
    /// Carpeta del panel activo, a resaltar.
    pub active_path: Option<PathBuf>,
    /// Si `Some(path)`, la UI debe hacer scroll a ese nodo y luego limpiarlo.
    pub reveal_to: Option<PathBuf>,
}

impl DirTree {
    /// Crea el árbol con una raíz por unidad. `(path, label, kind)` por unidad.
    pub fn from_drives(drives: &[(PathBuf, String, DriveKind)]) -> Self {
        let roots = drives
            .iter()
            .map(|(path, label, kind)| TreeNode::drive(path.clone(), label.clone(), *kind))
            .collect();
        DirTree {
            roots,
            active_path: None,
            reveal_to: None,
        }
    }

    /// `true` si el árbol no tiene raíces todavía (aún no inicializado).
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Recorre todas las raíces buscando el primer nodo cuyo `path == path`.
    pub fn node_at(&self, path: &Path) -> Option<&TreeNode> {
        self.roots.iter().find_map(|r| find_node(r, path))
    }

    /// Versión mutable de `node_at`.
    pub fn node_at_mut(&mut self, path: &Path) -> Option<&mut TreeNode> {
        self.roots.iter_mut().find_map(|r| find_node_mut(r, path))
    }
}

/// Busca recursivamente un nodo por path dentro de `node`.
fn find_node<'a>(node: &'a TreeNode, path: &Path) -> Option<&'a TreeNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .as_ref()?
        .iter()
        .find_map(|c| find_node(c, path))
}

/// Versión mutable de `find_node`.
fn find_node_mut<'a>(node: &'a mut TreeNode, path: &Path) -> Option<&'a mut TreeNode> {
    if node.path == path {
        return Some(node);
    }
    node.children
        .as_mut()?
        .iter_mut()
        .find_map(|c| find_node_mut(c, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive_list() -> Vec<(PathBuf, String, DriveKind)> {
        vec![
            (PathBuf::from("C:\\"), "C:\\".into(), DriveKind::Fixed),
            (PathBuf::from("D:\\"), "D:\\".into(), DriveKind::Removable),
        ]
    }

    #[test]
    fn from_drives_crea_una_raiz_por_unidad() {
        let t = DirTree::from_drives(&drive_list());
        assert_eq!(t.roots.len(), 2);
        assert_eq!(t.roots[0].path, PathBuf::from("C:\\"));
        assert_eq!(t.roots[0].drive_kind, Some(DriveKind::Fixed));
        assert!(!t.roots[0].expanded);
        assert_eq!(t.roots[0].state, NodeState::Collapsed);
        assert!(t.roots[0].children.is_none());
    }

    #[test]
    fn folder_toma_el_ultimo_componente_como_nombre() {
        let n = TreeNode::folder(PathBuf::from("C:\\Users\\ngroth"));
        assert_eq!(n.name, "ngroth");
        assert_eq!(n.drive_kind, None);
    }

    #[test]
    fn node_at_encuentra_la_raiz() {
        let t = DirTree::from_drives(&drive_list());
        assert!(t.node_at(Path::new("C:\\")).is_some());
        assert!(t.node_at(Path::new("Z:\\")).is_none());
    }
}
```

- [ ] **Step 2: Declarar el módulo y re-exportar**

Modify `crates/core/src/lib.rs`:
- Tras `pub mod sort;` añadir `pub mod tree;`.
- Tras `pub use sort::sort_entries;` añadir:
```rust
pub use tree::{DirTree, NodeOutcome, NodeState, TreeNode};
```

- [ ] **Step 3: Correr los tests**

Run: `cargo test -p naygo-core tree`
Expected: 3 tests PASS (`from_drives_crea_una_raiz_por_unidad`, `folder_toma_el_ultimo_componente_como_nombre`, `node_at_encuentra_la_raiz`).
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/tree.rs crates/core/src/lib.rs
git commit -m "feat(core): modelo base del árbol (TreeNode, NodeState, DirTree, from_drives)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `core::tree` — transiciones de estado (expandir, push_child, finish, collapse)

**Files:**
- Modify: `crates/core/src/tree.rs`

- [ ] **Step 1: Escribir los tests de las transiciones (primero, TDD)**

En el `#[cfg(test)] mod tests` de `tree.rs`, añadir:

```rust
    #[test]
    fn begin_loading_marca_loading_y_expandido() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        let n = t.node_at(Path::new("C:\\")).unwrap();
        assert_eq!(n.state, NodeState::Loading);
        assert!(n.expanded);
        // Al empezar a cargar, children pasa a Some(vec vacío) para recibir hijos.
        assert_eq!(n.children.as_ref().map(|c| c.len()), Some(0));
    }

    #[test]
    fn push_child_inserta_ordenado_case_insensitive() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        t.push_child(Path::new("C:\\"), PathBuf::from("C:\\Windows"));
        t.push_child(Path::new("C:\\"), PathBuf::from("C:\\apps"));
        t.push_child(Path::new("C:\\"), PathBuf::from("C:\\Users"));
        let n = t.node_at(Path::new("C:\\")).unwrap();
        let names: Vec<&str> = n
            .children
            .as_ref()
            .unwrap()
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        // Orden case-insensitive: apps, Users, Windows.
        assert_eq!(names, vec!["apps", "Users", "Windows"]);
    }

    #[test]
    fn finish_loading_con_hijos_queda_loaded() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        t.push_child(Path::new("C:\\"), PathBuf::from("C:\\Users"));
        t.finish_loading(Path::new("C:\\"), NodeOutcome::Done);
        assert_eq!(t.node_at(Path::new("C:\\")).unwrap().state, NodeState::Loaded);
    }

    #[test]
    fn finish_loading_sin_hijos_queda_empty() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        t.finish_loading(Path::new("C:\\"), NodeOutcome::Done);
        assert_eq!(t.node_at(Path::new("C:\\")).unwrap().state, NodeState::Empty);
    }

    #[test]
    fn finish_loading_error_queda_error() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        t.finish_loading(Path::new("C:\\"), NodeOutcome::Error);
        assert_eq!(t.node_at(Path::new("C:\\")).unwrap().state, NodeState::Error);
    }

    #[test]
    fn collapse_conserva_hijos() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        t.push_child(Path::new("C:\\"), PathBuf::from("C:\\Users"));
        t.finish_loading(Path::new("C:\\"), NodeOutcome::Done);
        t.collapse(Path::new("C:\\"));
        let n = t.node_at(Path::new("C:\\")).unwrap();
        assert!(!n.expanded);
        // Conserva los hijos: no se re-lista al reabrir.
        assert_eq!(n.children.as_ref().map(|c| c.len()), Some(1));
        assert_eq!(n.state, NodeState::Loaded);
    }
```

- [ ] **Step 2: Correr y ver fallar (no compila: métodos inexistentes)**

Run: `cargo test -p naygo-core tree`
Expected: ERROR de compilación "no method named `begin_loading`/`push_child`/`finish_loading`/`collapse`".

- [ ] **Step 3: Implementar los métodos en `impl DirTree`**

En `impl DirTree` (en `tree.rs`), añadir:

```rust
    /// Marca el nodo como `Loading`, expandido, y prepara su lista de hijos vacía
    /// (lista para recibir los que lleguen del worker). No-op si el path no existe.
    pub fn begin_loading(&mut self, path: &Path) {
        if let Some(node) = self.node_at_mut(path) {
            node.expanded = true;
            node.state = NodeState::Loading;
            node.children = Some(Vec::new());
        }
    }

    /// Inserta una subcarpeta `child_dir` en el nodo `parent`, manteniendo el orden
    /// por nombre (case-insensitive). No-op si el padre no existe o aún no tiene
    /// `children` inicializado.
    pub fn push_child(&mut self, parent: &Path, child_dir: PathBuf) {
        if let Some(node) = self.node_at_mut(parent) {
            if let Some(children) = node.children.as_mut() {
                let child = TreeNode::folder(child_dir);
                let key = child.name.to_lowercase();
                let pos = children
                    .iter()
                    .position(|c| c.name.to_lowercase() > key)
                    .unwrap_or(children.len());
                children.insert(pos, child);
            }
        }
    }

    /// Cierra el listado de un nodo: `Loaded`/`Empty` según tenga hijos, o `Error`.
    pub fn finish_loading(&mut self, path: &Path, outcome: NodeOutcome) {
        if let Some(node) = self.node_at_mut(path) {
            node.state = match outcome {
                NodeOutcome::Error => NodeState::Error,
                NodeOutcome::Done => {
                    let has = node.children.as_ref().map(|c| !c.is_empty()).unwrap_or(false);
                    if has {
                        NodeState::Loaded
                    } else {
                        NodeState::Empty
                    }
                }
            };
        }
    }

    /// Colapsa un nodo conservando sus hijos ya cargados.
    pub fn collapse(&mut self, path: &Path) {
        if let Some(node) = self.node_at_mut(path) {
            node.expanded = false;
        }
    }
```

- [ ] **Step 4: Correr los tests — pasan**

Run: `cargo test -p naygo-core tree` → todos PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/tree.rs
git commit -m "feat(core): transiciones del árbol (begin_loading, push_child, finish, collapse)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `core::tree` — auto-reveal (set_active + reveal_chain)

**Files:**
- Modify: `crates/core/src/tree.rs`

- [ ] **Step 1: Escribir los tests (primero)**

En el `mod tests` de `tree.rs`, añadir:

```rust
    #[test]
    fn set_active_fija_path_y_reveal() {
        let mut t = DirTree::from_drives(&drive_list());
        t.set_active(PathBuf::from("C:\\Users\\ngroth"));
        assert_eq!(t.active_path, Some(PathBuf::from("C:\\Users\\ngroth")));
        assert_eq!(t.reveal_to, Some(PathBuf::from("C:\\Users\\ngroth")));
    }

    #[test]
    fn reveal_chain_devuelve_ancestros_desde_la_raiz() {
        let t = DirTree::from_drives(&drive_list());
        let chain = t.reveal_chain(Path::new("C:\\Users\\ngroth\\Documents"));
        // De la unidad hacia abajo, SIN incluir el destino final (solo lo que hay
        // que EXPANDIR para revelarlo): C:\, C:\Users, C:\Users\ngroth.
        assert_eq!(
            chain,
            vec![
                PathBuf::from("C:\\"),
                PathBuf::from("C:\\Users"),
                PathBuf::from("C:\\Users\\ngroth"),
            ]
        );
    }

    #[test]
    fn reveal_chain_de_una_raiz_es_vacia() {
        let t = DirTree::from_drives(&drive_list());
        // La carpeta activa es la unidad misma: no hay nada que expandir antes.
        let chain = t.reveal_chain(Path::new("C:\\"));
        assert!(chain.is_empty());
    }

    #[test]
    fn reveal_chain_sin_raiz_conocida_es_vacia() {
        let t = DirTree::from_drives(&drive_list());
        // Z:\ no es una raíz del árbol → no se puede revelar nada.
        let chain = t.reveal_chain(Path::new("Z:\\algo"));
        assert!(chain.is_empty());
    }

    #[test]
    fn clear_reveal_borra_el_pendiente() {
        let mut t = DirTree::from_drives(&drive_list());
        t.set_active(PathBuf::from("C:\\Users"));
        t.clear_reveal();
        assert_eq!(t.reveal_to, None);
        // active_path se conserva (solo se limpia el scroll pendiente).
        assert_eq!(t.active_path, Some(PathBuf::from("C:\\Users")));
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core tree`
Expected: ERROR de compilación "no method named `set_active`/`reveal_chain`/`clear_reveal`".

- [ ] **Step 3: Implementar**

En `impl DirTree` añadir:

```rust
    /// Fija la carpeta activa (a resaltar) y marca que hay que revelarla (scroll).
    pub fn set_active(&mut self, path: PathBuf) {
        self.active_path = Some(path.clone());
        self.reveal_to = Some(path);
    }

    /// Limpia el scroll pendiente (tras hacerlo). Conserva `active_path`.
    pub fn clear_reveal(&mut self) {
        self.reveal_to = None;
    }

    /// Dada una carpeta destino, devuelve la cadena de ancestros que hay que
    /// EXPANDIR para revelarla, desde la raíz/unidad hacia abajo (sin incluir el
    /// destino final). Vacía si ninguna raíz del árbol es prefijo del destino.
    pub fn reveal_chain(&self, target: &Path) -> Vec<PathBuf> {
        // Buscar la raíz (unidad) que es prefijo del destino.
        let root = self
            .roots
            .iter()
            .find(|r| target.starts_with(&r.path))
            .map(|r| r.path.clone());
        let Some(root) = root else {
            return Vec::new();
        };
        // Construir desde la raíz: root, root/a, root/a/b, ... sin incluir target.
        let mut chain = Vec::new();
        let mut acc = root.clone();
        chain.push(acc.clone());
        // Componentes relativos de root → target.
        if let Ok(rel) = target.strip_prefix(&root) {
            let comps: Vec<_> = rel.components().collect();
            // Todos menos el último componente (ese es el destino, no se expande).
            for comp in comps.iter().take(comps.len().saturating_sub(1)) {
                acc = acc.join(comp.as_os_str());
                chain.push(acc.clone());
            }
        }
        // Si el destino ES la raíz, no hay nada que expandir.
        if target == root {
            return Vec::new();
        }
        chain
    }
```

NOTA de diseño: `reveal_chain` devuelve los nodos a EXPANDIR (ancestros del destino, incluida la unidad). El consumidor (ui, Tarea 9) lanza un worker por cada uno que aún no esté cargado, en orden; cuando cada nivel termina, el siguiente ya puede expandir.

- [ ] **Step 4: Correr los tests — pasan**

Run: `cargo test -p naygo-core tree` → todos PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/tree.rs
git commit -m "feat(core): auto-reveal del árbol (set_active, reveal_chain, clear_reveal)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `core::listing` — variante solo-directorios

**Files:**
- Modify: `crates/core/src/listing.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Escribir el test (primero)**

En el `#[cfg(test)] mod tests` de `listing.rs`, añadir:

```rust
    #[test]
    fn solo_directorios_filtra_los_archivos() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"x").unwrap();
        fs::write(dir.path().join("b.log"), b"y").unwrap();
        fs::create_dir(dir.path().join("sub1")).unwrap();
        fs::create_dir(dir.path().join("sub2")).unwrap();

        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        list_into_filtered(dir.path(), &token, &tx, ListingFilter::DirsOnly);
        drop(tx);

        let mut nombres = Vec::new();
        for msg in rx {
            if let ListingMsg::Entry(e) = msg {
                nombres.push(e.name);
            }
        }
        nombres.sort();
        // Solo las carpetas, ningún archivo.
        assert_eq!(nombres, vec!["sub1", "sub2"]);
    }

    #[test]
    fn filtro_all_sigue_emitiendo_todo() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"x").unwrap();
        fs::create_dir(dir.path().join("sub1")).unwrap();

        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        list_into_filtered(dir.path(), &token, &tx, ListingFilter::All);
        drop(tx);

        let mut nombres = Vec::new();
        for msg in rx {
            if let ListingMsg::Entry(e) = msg {
                nombres.push(e.name);
            }
        }
        nombres.sort();
        assert_eq!(nombres, vec!["a.txt", "sub1"]);
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core listing`
Expected: ERROR de compilación "cannot find function `list_into_filtered`" / "cannot find `ListingFilter`".

- [ ] **Step 3: Implementar el filtro sin duplicar el cuerpo**

Modify `crates/core/src/listing.rs`:

a) Añadir el enum del filtro (tras la def de `ListingMsg`):
```rust
/// Qué entradas emite un listado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListingFilter {
    /// Todas las entradas (comportamiento del file panel).
    All,
    /// Solo directorios (para el árbol de carpetas).
    DirsOnly,
}
```

b) Añadir una función pública que lanza con filtro, y hacer que `spawn_listing` delegue:
```rust
/// Lanza el listado de `dir` con un filtro. Igual que `spawn_listing` pero pudiendo
/// emitir solo directorios (para el árbol).
pub fn spawn_listing_filtered(
    dir: PathBuf,
    token: CancellationToken,
    filter: ListingFilter,
) -> (Receiver<ListingMsg>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        list_into_filtered(&dir, &token, &tx, filter);
    });
    (rx, handle)
}
```

c) Cambiar `spawn_listing` para delegar:
```rust
pub fn spawn_listing(
    dir: PathBuf,
    token: CancellationToken,
) -> (Receiver<ListingMsg>, JoinHandle<()>) {
    spawn_listing_filtered(dir, token, ListingFilter::All)
}
```

d) Renombrar el cuerpo `list_into` → `list_into_filtered` con el parámetro `filter`, y saltar no-directorios cuando corresponda. Reemplazar la firma y el bucle:
```rust
/// Cuerpo del worker: recorre el directorio emitiendo por `tx`, aplicando `filter`.
/// Extraído para testearlo síncrono sin spawnear un hilo.
fn list_into_filtered(
    dir: &Path,
    token: &CancellationToken,
    tx: &mpsc::Sender<ListingMsg>,
    filter: ListingFilter,
) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            let _ = tx.send(ListingMsg::Error(e.to_string()));
            return;
        }
    };

    for dirent in read_dir {
        if token.is_cancelled() {
            let _ = tx.send(ListingMsg::Cancelled);
            return;
        }

        let dirent = match dirent {
            Ok(d) => d,
            Err(_) => continue,
        };

        let entry = entry_from_dirent(&dirent);
        if filter == ListingFilter::DirsOnly && entry.kind != EntryKind::Directory {
            continue;
        }
        if tx.send(ListingMsg::Entry(entry)).is_err() {
            return;
        }
    }

    if token.is_cancelled() {
        let _ = tx.send(ListingMsg::Cancelled);
    } else {
        let _ = tx.send(ListingMsg::Done);
    }
}
```

e) Los tests existentes llaman `list_into(...)`. Para no romperlos, añadir un wrapper fino justo encima del nuevo cuerpo:
```rust
/// Wrapper de compatibilidad: lista todo (lo que esperan los tests existentes).
#[cfg(test)]
fn list_into(dir: &Path, token: &CancellationToken, tx: &mpsc::Sender<ListingMsg>) {
    list_into_filtered(dir, token, tx, ListingFilter::All);
}
```
(El `#[cfg(test)]` evita un warning de función sin usar en release. Si algún test fuera de `listing.rs` lo usa, quitar el `#[cfg(test)]`; al cierre de la tarea, `cargo test` lo confirmará.)

- [ ] **Step 4: Re-exportar y verificar**

Modify `crates/core/src/lib.rs` — cambiar la línea de re-export de listing a:
```rust
pub use listing::{spawn_listing, spawn_listing_filtered, ListingFilter, ListingMsg};
```

Run: `cargo test -p naygo-core listing` → los 2 nuevos + los 3 existentes (`lista_archivos_de_un_directorio`, `token_cancelado_antes_de_empezar_no_emite_entradas`, `directorio_inexistente_emite_error`) PASS.
Run: `cargo test -p naygo-core` → todo verde.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/listing.rs crates/core/src/lib.rs
git commit -m "feat(core): listado solo-directorios (ListingFilter::DirsOnly) para el árbol

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: `platform::drives()` — enumeración de unidades (Win32)

**Files:**
- Create: `crates/platform/src/drives.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/platform/Cargo.toml`

- [ ] **Step 1: Añadir las features de la API a Cargo.toml**

Modify `crates/platform/Cargo.toml` — la línea de `windows` bajo `[target.'cfg(windows)'.dependencies]`:
```toml
windows = { workspace = true, features = ["Win32_Globalization", "Win32_Storage_FileSystem", "Win32_System_WindowsProgramming"] }
```
(`GetLogicalDriveStringsW` y `GetDriveTypeW` están en `Win32_Storage_FileSystem`. Si el build reclama por un símbolo, ajustar la feature según el error del compilador.)

- [ ] **Step 2: Crear `drives.rs` con `DriveInfo`, `drives()` y un test smoke**

Create `crates/platform/src/drives.rs`:

```rust
// Naygo — enumeración de unidades de disco del sistema (Win32, aislado).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `drives()` lista las unidades lógicas del equipo para las raíces del árbol.
//! Tolerante: una unidad que no responde se incluye igual (su expansión dará
//! error en el listado), no aborta la enumeración. El hilo de UI no llama a esto
//! directamente sobre disco caliente: se invoca una vez al crear el árbol.

use naygo_core::icon_kind::DriveKind;
use std::path::PathBuf;

/// Una unidad de disco descubierta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriveInfo {
    /// Raíz de la unidad, p. ej. `C:\`.
    pub path: PathBuf,
    /// Etiqueta a mostrar (por ahora la raíz misma, p. ej. `C:\`).
    pub label: String,
    pub kind: DriveKind,
}

#[cfg(windows)]
pub fn drives() -> Vec<DriveInfo> {
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDriveStringsW};
    use windows::Win32::Storage::FileSystem::{
        DRIVE_CDROM, DRIVE_FIXED, DRIVE_RAMDISK, DRIVE_REMOTE, DRIVE_REMOVABLE,
    };

    // Primera llamada con buffer 0 devuelve el largo necesario (en chars).
    let needed = unsafe { GetLogicalDriveStringsW(None) };
    if needed == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u16; needed as usize + 1];
    let written = unsafe { GetLogicalDriveStringsW(Some(&mut buf)) };
    if written == 0 {
        return Vec::new();
    }

    // El buffer es una lista de cadenas terminadas en NUL, con un NUL final extra.
    let mut out = Vec::new();
    for chunk in buf[..written as usize].split(|&c| c == 0) {
        if chunk.is_empty() {
            continue;
        }
        let root = String::from_utf16_lossy(chunk); // p. ej. "C:\\"
        let kind = drive_kind_of(&root);
        out.push(DriveInfo {
            path: PathBuf::from(&root),
            label: root.clone(),
            kind,
        });
        let _ = GetDriveTypeW; // referencia para el helper de abajo
        let _ = (DRIVE_FIXED, DRIVE_REMOVABLE, DRIVE_REMOTE, DRIVE_CDROM, DRIVE_RAMDISK);
    }
    out
}

/// Clasifica el tipo de una unidad por su raíz (p. ej. "C:\\").
#[cfg(windows)]
fn drive_kind_of(root: &str) -> DriveKind {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetDriveTypeW, DRIVE_CDROM, DRIVE_FIXED, DRIVE_RAMDISK, DRIVE_REMOTE, DRIVE_REMOVABLE,
    };
    let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
    let t = unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) };
    match t {
        x if x == DRIVE_FIXED => DriveKind::Fixed,
        x if x == DRIVE_REMOVABLE => DriveKind::Removable,
        x if x == DRIVE_REMOTE => DriveKind::Network,
        x if x == DRIVE_CDROM => DriveKind::Optical,
        x if x == DRIVE_RAMDISK => DriveKind::Fixed,
        _ => DriveKind::Unknown,
    }
}

/// Stub para plataformas no-Windows: devuelve la raíz `/` como única "unidad".
#[cfg(not(windows))]
pub fn drives() -> Vec<DriveInfo> {
    vec![DriveInfo {
        path: PathBuf::from("/"),
        label: "/".into(),
        kind: DriveKind::Fixed,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drives_devuelve_al_menos_una_unidad() {
        let d = drives();
        assert!(!d.is_empty(), "debe haber al menos una unidad");
        // Cada unidad tiene un path no vacío.
        assert!(d.iter().all(|x| !x.path.as_os_str().is_empty()));
    }
}
```

NOTA para el implementador: las dos líneas `let _ = GetDriveTypeW;` y `let _ = (DRIVE_FIXED, ...)` dentro de `drives()` son un parche provisional para no dejar imports sin usar SI decides no llamar a `drive_kind_of`. PREFERIBLE: borra esas dos líneas y los imports `GetDriveTypeW`/`DRIVE_*` del cuerpo de `drives()` (quedan solo en `drive_kind_of`), y asegúrate de llamar `drive_kind_of(&root)` (que ya se hace). El objetivo es CERO warnings con `-D warnings`. Verifica con clippy y ajusta los `use` hasta que quede limpio.

- [ ] **Step 3: Declarar el módulo**

Modify `crates/platform/src/lib.rs` — tras `pub mod locale;` añadir:
```rust
pub mod drives;
```

- [ ] **Step 4: Compilar y testear**

Run: `cargo test -p naygo-platform` → `drives_devuelve_al_menos_una_unidad` PASS (en Windows lista las unidades reales; en otro SO, el stub da `/`).
Run: `cargo clippy -p naygo-platform -- -D warnings` → limpio (ajustar imports `use` si reclama imports sin usar).

NOTA: si `GetLogicalDriveStringsW`/`GetDriveTypeW` no se encuentran con la feature elegida, el error del compilador dirá el módulo correcto; ajustar la ruta `use windows::Win32::Storage::FileSystem::...` y/o la feature en Cargo.toml. La API existe en `windows` 0.62 bajo `Win32_Storage_FileSystem`.

- [ ] **Step 5: Commit**

```bash
git add crates/platform/src/drives.rs crates/platform/src/lib.rs crates/platform/Cargo.toml
git commit -m "feat(platform): enumeración de unidades de disco (drives() vía Win32)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: i18n — claves nuevas del árbol (ES + EN)

**Files:**
- Modify: `crates/core/src/i18n/es.json`
- Modify: `crates/core/src/i18n/en.json`

- [ ] **Step 1: Añadir las claves en español**

Modify `crates/core/src/i18n/es.json` — tras la línea `"tree.go_up": "Subir un nivel",` añadir:
```json
  "tree.loading": "cargando…",
  "tree.empty": "(sin subcarpetas)",
  "tree.access_denied": "acceso denegado",
```

- [ ] **Step 2: Añadir las mismas claves en inglés**

Modify `crates/core/src/i18n/en.json` — tras la línea equivalente `"tree.go_up": ...,` añadir:
```json
  "tree.loading": "loading…",
  "tree.empty": "(no subfolders)",
  "tree.access_denied": "access denied",
```

(Verifica que la coma del JSON quede válida: la última clave del objeto NO lleva coma. Inserta ANTES de claves existentes para no tener que tocar la última.)

- [ ] **Step 3: Verificar que el JSON es válido y los tests de i18n pasan**

Run: `cargo test -p naygo-core i18n` → PASS (los catálogos embebidos siguen cargando; ES y EN tienen las mismas claves).
Run: `cargo test -p naygo-core` → todo verde.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "i18n: claves del árbol (cargando, sin subcarpetas, acceso denegado)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: `ui::tree_actions` — enum de acciones del árbol

**Files:**
- Create: `crates/ui/src/tree_actions.rs`
- Modify: `crates/ui/src/main.rs`

- [ ] **Step 1: Crear el enum con un test trivial**

Create `crates/ui/src/tree_actions.rs`:

```rust
// Naygo — acciones que el árbol acumula durante el pintado y se ejecutan después.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Igual que `PaneRequest` en el file panel: el render del árbol no muta el estado
//! ni hace I/O directamente; acumula `TreeAction`s que `NaygoApp` procesa tras
//! pintar, evitando préstamos conflictivos.

use std::path::PathBuf;

/// Una acción pedida desde el árbol durante el pintado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeAction {
    /// Expandir (y listar) el nodo en `path`.
    Expand(PathBuf),
    /// Colapsar el nodo en `path`.
    Collapse(PathBuf),
    /// Navegar el panel activo a `path` (clic en el nombre de una carpeta).
    Navigate(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_action_es_comparable() {
        let a = TreeAction::Expand(PathBuf::from("C:\\"));
        let b = TreeAction::Expand(PathBuf::from("C:\\"));
        assert_eq!(a, b);
        assert_ne!(a, TreeAction::Collapse(PathBuf::from("C:\\")));
    }
}
```

- [ ] **Step 2: Declarar el módulo**

Modify `crates/ui/src/main.rs` — añadir `mod tree_actions;` junto a los otros `mod` (orden alfabético: tras `mod toolbar;`).

- [ ] **Step 3: Correr el test**

Run: `cargo test -p naygo-ui tree_action` → PASS.
Run: `cargo clippy -p naygo-ui --all-targets -- -D warnings` → limpio (puede haber `dead_code` en variantes aún no usadas; se consumen en la Tarea 8/9. Si clippy bloquea, añadir `#[allow(dead_code)]` temporal al enum con comentario "consumido en Tarea 8-9" y quitarlo en la Tarea 9).

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/tree_actions.rs crates/ui/src/main.rs
git commit -m "feat(ui): enum TreeAction (acciones diferidas del árbol)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: `app.rs` — estado del árbol por panel + pump_tree + creación perezosa

**Files:**
- Modify: `crates/ui/src/app.rs`

Esta tarea agrega el ESTADO y el DRENADO del árbol, sin render todavía (eso es Tarea 9). Tras esta tarea, los workers del árbol existen y se drenan, pero nada los dispara aún (lo hará el render + auto-sync). Para validar que compila y no rompe nada, se conecta el bucle.

- [ ] **Step 1: Añadir los campos de estado y el tipo de worker**

Modify `crates/ui/src/app.rs`:

a) En los `use`, añadir:
```rust
use naygo_core::tree::DirTree;
use naygo_core::listing::{spawn_listing_filtered, ListingFilter};
use naygo_core::NodeOutcome;
```
(ya hay `use naygo_core::listing::{spawn_listing, ListingMsg};` — déjalo; o consolida en una sola línea `use naygo_core::listing::{spawn_listing, spawn_listing_filtered, ListingFilter, ListingMsg};`.)

b) Tras la struct `PaneListing`, añadir el worker del árbol:
```rust
/// Un worker de listado solo-directorios para una rama del árbol.
struct TreeListing {
    rx: Option<Receiver<ListingMsg>>,
    token: CancellationToken,
}
```

c) En `struct NaygoApp`, tras `listings: HashMap<PaneId, PaneListing>,` añadir:
```rust
    /// Estado del árbol por cada panel Tree (creado perezosamente).
    trees: HashMap<PaneId, DirTree>,
    /// Workers solo-directorios del árbol, por (panel, carpeta) expandida.
    tree_listings: HashMap<(PaneId, PathBuf), TreeListing>,
```

d) En `NaygoApp::new`, en el literal `let mut app = NaygoApp { ... }`, tras `listings: HashMap::new(),` añadir:
```rust
            trees: HashMap::new(),
            tree_listings: HashMap::new(),
```

- [ ] **Step 2: Implementar la creación perezosa y la expansión de una rama**

En `impl NaygoApp`, añadir estos métodos (cerca de `start_listing`):

```rust
    /// Devuelve el `DirTree` del panel `id`, creándolo (con las unidades) la primera
    /// vez. Útil antes de pintar un panel Tree.
    fn ensure_tree(&mut self, id: PaneId) -> &mut DirTree {
        self.trees.entry(id).or_insert_with(|| {
            let drives = naygo_platform::drives::drives()
                .into_iter()
                .map(|d| (d.path, d.label, d.kind))
                .collect::<Vec<_>>();
            DirTree::from_drives(&drives)
        })
    }

    /// Expande una rama del árbol del panel `id`: marca Loading y lanza el worker
    /// solo-directorios. No-op si ya hay un worker para esa (id, path).
    fn tree_expand(&mut self, id: PaneId, path: PathBuf) {
        if self.tree_listings.contains_key(&(id, path.clone())) {
            return;
        }
        if let Some(tree) = self.trees.get_mut(&id) {
            tree.begin_loading(&path);
        }
        let token = CancellationToken::new();
        let (rx, _h) = spawn_listing_filtered(path.clone(), token.clone(), ListingFilter::DirsOnly);
        self.tree_listings.insert(
            (id, path),
            TreeListing { rx: Some(rx), token },
        );
    }

    /// Colapsa una rama (conserva hijos). Cancela su worker si seguía cargando.
    fn tree_collapse(&mut self, id: PaneId, path: PathBuf) {
        if let Some(l) = self.tree_listings.get(&(id, path.clone())) {
            l.token.cancel();
        }
        self.tree_listings.remove(&(id, path.clone()));
        if let Some(tree) = self.trees.get_mut(&id) {
            tree.collapse(&path);
        }
    }

    /// Drena los canales de TODOS los workers del árbol, sin bloquear.
    fn pump_tree(&mut self) {
        let keys: Vec<(PaneId, PathBuf)> = self.tree_listings.keys().cloned().collect();
        for key in keys {
            self.pump_tree_one(key);
        }
    }

    fn pump_tree_one(&mut self, key: (PaneId, PathBuf)) {
        let (id, ref path) = key;
        let mut finished = false;
        let mut err = false;
        let mut new_dirs: Vec<PathBuf> = Vec::new();
        if let Some(listing) = self.tree_listings.get(&key) {
            if let Some(rx) = &listing.rx {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        ListingMsg::Entry(e) => new_dirs.push(e.path),
                        ListingMsg::Done => finished = true,
                        ListingMsg::Cancelled => finished = true,
                        ListingMsg::Error(_) => {
                            err = true;
                            finished = true;
                        }
                    }
                }
            }
        }
        if let Some(tree) = self.trees.get_mut(&id) {
            for d in new_dirs {
                tree.push_child(path, d);
            }
            if finished {
                let outcome = if err { NodeOutcome::Error } else { NodeOutcome::Done };
                tree.finish_loading(path, outcome);
            }
        }
        if finished {
            if let Some(listing) = self.tree_listings.get_mut(&key) {
                listing.rx = None;
            }
            // Soltar el worker terminado del mapa (su rx ya está vacío).
            self.tree_listings.remove(&key);
        }
    }

    fn any_tree_listing_active(&self) -> bool {
        self.tree_listings.values().any(|l| l.rx.is_some())
    }
```

- [ ] **Step 3: Conectar `pump_tree` y el repaint en el loop**

En `impl eframe::App for NaygoApp`, método `logic`, cambiar:
```rust
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump_all();
        self.pump_tree();
        self.handle_input(ctx);
        if self.any_listing_active() || self.any_tree_listing_active() {
            ctx.request_repaint();
        }
    }
```

- [ ] **Step 4: Compilar (aún sin uso del render → puede haber dead_code)**

Run: `cargo build -p naygo-ui` → compila.
Run: `cargo clippy -p naygo-ui --all-targets -- -D warnings`.
NOTA: `ensure_tree`, `tree_expand`, `tree_collapse` aún no se llaman (se conectan en la Tarea 9) → clippy dará `dead_code`. Añadir `#[allow(dead_code)]` a esos tres métodos con un comentario `// conectado en la Tarea 9 (render + auto-sync)`. En la Tarea 9 se quitan los allow al usarlos. `pump_tree`/`pump_tree_one`/`any_tree_listing_active` SÍ se usan (en `logic`), no necesitan allow.

Run: `cargo test --workspace` → verde (no se rompió nada).

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/app.rs
git commit -m "feat(ui): estado del árbol por panel + workers solo-dir + pump_tree

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: `tree_panel.rs` reescrito + docking + auto-sync (render, spinner, resaltado B, scroll)

**Files:**
- Modify: `crates/ui/src/panes/tree_panel.rs` (reescritura)
- Modify: `crates/ui/src/docking.rs`
- Modify: `crates/ui/src/app.rs` (pasar tree+acciones; auto-sync; procesar acciones; quitar allows)

Esta es la tarea de integración grande. Produce el árbol funcional.

- [ ] **Step 1: Reescribir `tree_panel.rs` con render recursivo y acumulación de acciones**

Replace `crates/ui/src/panes/tree_panel.rs` con:

```rust
// Naygo — panel de árbol: carpetas expandibles con lazy-load, navegación y reveal.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta un `DirTree` recursivamente: raíces = unidades, ramas colapsables (solo
//! carpetas). Clic en el triángulo expande/colapsa; clic en el nombre navega el
//! panel activo. No hace I/O ni muta el árbol: acumula `TreeAction`s que `NaygoApp`
//! ejecuta tras pintar. La carpeta activa se resalta (barra azul + fondo tenue) y,
//! si hay `reveal_to`, se hace scroll a ella.

use crate::icons::IconProvider;
use crate::tree_actions::TreeAction;
use naygo_core::icon_kind::IconKey;
use naygo_core::tree::{DirTree, NodeState, TreeNode};

const ICON_SIZE: f32 = 16.0;
const INDENT: f32 = 14.0;

pub fn show(
    ui: &mut egui::Ui,
    tree: &DirTree,
    actions: &mut Vec<TreeAction>,
    icons: &IconProvider,
    i18n: &naygo_core::i18n::I18n,
) {
    if tree.is_empty() {
        ui.label(i18n.t("tree.empty"));
        return;
    }
    egui::ScrollArea::both().show(ui, |ui| {
        for root in &tree.roots {
            show_node(ui, root, 0, tree, actions, icons, i18n);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn show_node(
    ui: &mut egui::Ui,
    node: &TreeNode,
    depth: usize,
    tree: &DirTree,
    actions: &mut Vec<TreeAction>,
    icons: &IconProvider,
    i18n: &naygo_core::i18n::I18n,
) {
    let is_active = tree.active_path.as_deref() == Some(node.path.as_path());

    let row = ui.horizontal(|ui| {
        ui.add_space(depth as f32 * INDENT);

        // Triángulo: solo si el nodo puede tener hijos (no Empty, no Error).
        let has_toggle = !matches!(node.state, NodeState::Empty | NodeState::Error);
        if has_toggle {
            let arrow = if node.expanded { "▼" } else { "▶" };
            if ui
                .add(egui::Label::new(arrow).sense(egui::Sense::click()))
                .clicked()
            {
                if node.expanded {
                    actions.push(TreeAction::Collapse(node.path.clone()));
                } else {
                    actions.push(TreeAction::Expand(node.path.clone()));
                }
            }
        } else {
            ui.add_space(INDENT);
        }

        // Ícono: unidad si es raíz, carpeta si no.
        let key = match node.drive_kind {
            Some(kind) => IconKey::Drive(kind),
            None => IconKey::Folder,
        };
        let tex = icons.texture(key);
        ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(ICON_SIZE, ICON_SIZE)));

        // Nombre clicable: navega el panel activo.
        let label = egui::SelectableLabel::new(is_active, &node.name);
        if ui.add(label).clicked() {
            actions.push(TreeAction::Navigate(node.path.clone()));
        }

        // Spinner mientras carga.
        if node.state == NodeState::Loading {
            ui.spinner();
        }
    });

    // Resaltado modo B: barra azul a la izquierda + fondo tenue en la fila activa.
    if is_active {
        let rect = row.response.rect;
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(0x37, 0x37, 0x3d));
        let bar = egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height()));
        painter.rect_filled(bar, 0.0, egui::Color32::from_rgb(0x2f, 0x81, 0xf7));
    }

    // Reveal: si este nodo es el objetivo de scroll, llevarlo a la vista.
    if tree.reveal_to.as_deref() == Some(node.path.as_path()) {
        row.response.scroll_to_me(Some(egui::Align::Center));
    }

    // Hijos (si expandido y cargados).
    if node.expanded {
        match node.state {
            NodeState::Loading => {
                ui.horizontal(|ui| {
                    ui.add_space((depth + 1) as f32 * INDENT + INDENT);
                    ui.weak(i18n.t("tree.loading"));
                });
            }
            NodeState::Empty => {
                ui.horizontal(|ui| {
                    ui.add_space((depth + 1) as f32 * INDENT + INDENT);
                    ui.weak(i18n.t("tree.empty"));
                });
            }
            NodeState::Error => {
                ui.horizontal(|ui| {
                    ui.add_space((depth + 1) as f32 * INDENT + INDENT);
                    ui.colored_label(
                        egui::Color32::from_rgb(0xe0, 0x6c, 0x5b),
                        format!("⚠ {}", i18n.t("tree.access_denied")),
                    );
                });
            }
            _ => {}
        }
        if let Some(children) = &node.children {
            for child in children {
                show_node(ui, child, depth + 1, tree, actions, icons, i18n);
            }
        }
    }
}
```

NOTA egui 0.34.3: verificar contra `C:\Users\ngrot\.cargo\registry\src\index.crates.io-*\egui-0.34.3\` que existen: `egui::Label::new(..).sense(Sense::click())`, `egui::SelectableLabel::new(selected, text)` + `ui.add(..)`, `ui.spinner()`, `Response::scroll_to_me(Option<Align>)`, `ui.painter().rect_filled(rect, rounding, color)`, `Response::rect`. Si alguna firma cambió (p. ej. `scroll_to_me` quiere `Align` no `Option<Align>`, o `rect_filled` pide `CornerRadius`/`Rounding`), ajustar al de esta versión. El resaltado por `painter.rect_filled` se pinta DESPUÉS del contenido, lo que lo taparía; si se ve mal, alternativa: pintar el fondo ANTES con `ui.painter().rect_filled` usando un rect reservado, o usar `egui::Frame::none().fill(..)` envolviendo la fila. Elegir lo que se vea bien (validación manual al final).

- [ ] **Step 2: Actualizar el despacho en `docking.rs`**

Modify `crates/ui/src/docking.rs`:

a) En `struct NaygoTabViewer`, añadir dos campos (tras `pub i18n`):
```rust
    pub tree: Option<&'a naygo_core::tree::DirTree>,
    pub tree_actions: &'a mut Vec<crate::tree_actions::TreeAction>,
```
NOTA: como `ui()` pinta UN tab por llamada y el árbol es por panel, el `tree` que se pasa debe ser el del panel que se está pintando. Pero `NaygoTabViewer` se construye una vez por frame. Para resolverlo SIN reestructurar el viewer: en `app.rs` (Step 4) NO se usa este campo `tree` del viewer; en su lugar, el viewer obtiene el árbol del panel actual desde un mapa. Cambiar el enfoque: pasar el MAPA completo.

Reemplazar los dos campos anteriores por:
```rust
    pub trees: &'a std::collections::HashMap<naygo_core::workspace::PaneId, naygo_core::tree::DirTree>,
    pub tree_actions: &'a mut Vec<(naygo_core::workspace::PaneId, crate::tree_actions::TreeAction)>,
```

b) En `fn ui`, el brazo `Some(PanePurpose::Tree)` cambiar a:
```rust
            Some(PanePurpose::Tree) => {
                if let Some(tree) = self.trees.get(&id) {
                    let mut local: Vec<crate::tree_actions::TreeAction> = Vec::new();
                    crate::panes::tree_panel::show(ui, tree, &mut local, self.icons, self.i18n);
                    for a in local {
                        self.tree_actions.push((id, a));
                    }
                } else {
                    // Aún no creado (se crea en el próximo frame desde app); muestra vacío.
                    ui.label(self.i18n.t("tree.loading"));
                }
            }
```

- [ ] **Step 3: En `app.rs`, crear árboles para paneles Tree visibles, pasar el mapa, procesar acciones, auto-sync**

Modify `crates/ui/src/app.rs`, método `ui`:

a) ANTES de construir el `NaygoTabViewer`, asegurar que cada panel Tree tenga su `DirTree`, y hacer auto-sync con la carpeta activa:
```rust
        // Crear perezosamente el DirTree de cada panel Tree y sincronizar la carpeta
        // activa (auto-reveal en cascada).
        let active_dir = self
            .workspace
            .active_files()
            .map(|f| f.current_dir.clone());
        let tree_pane_ids: Vec<PaneId> = self
            .workspace
            .panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Tree)
            .map(|p| p.id)
            .collect();
        for id in &tree_pane_ids {
            self.ensure_tree(*id);
        }
        // Auto-sync: si la carpeta activa cambió respecto al árbol, revelarla.
        if let Some(dir) = active_dir.clone() {
            let ids = tree_pane_ids.clone();
            for id in ids {
                let changed = self
                    .trees
                    .get(&id)
                    .map(|t| t.active_path.as_deref() != Some(dir.as_path()))
                    .unwrap_or(false);
                if changed {
                    if let Some(t) = self.trees.get_mut(&id) {
                        t.set_active(dir.clone());
                    }
                    // Expandir en cascada los ancestros aún no cargados.
                    let chain = self
                        .trees
                        .get(&id)
                        .map(|t| t.reveal_chain(&dir))
                        .unwrap_or_default();
                    for ancestor in chain {
                        let needs = self
                            .trees
                            .get(&id)
                            .and_then(|t| t.node_at(&ancestor))
                            .map(|n| n.children.is_none())
                            .unwrap_or(false);
                        if needs {
                            self.tree_expand(id, ancestor);
                        }
                    }
                }
            }
        }
```
NOTA: `node_at` es público (Tarea 1). El auto-sync corre cada frame pero solo actúa cuando `active_path` difiere → barato. Como cada `tree_expand` lista async, en frames siguientes llegan los hijos y el siguiente nivel de la cadena (que en ese frame todavía tenía `children.is_none()`) se expandirá cuando su padre exista; para garantizar la cascada completa, este bloque vuelve a calcular `reveal_chain` cada frame mientras `active_path` siga apuntando a `dir` (los niveles ya cargados se saltan por `needs=false`). Esto converge en pocos frames sin bloquear.

b) Cambiar la construcción del viewer para pasar el mapa de árboles y recoger acciones con su PaneId:
```rust
        let mut pending: Vec<crate::docking::PaneRequest> = Vec::new();
        let mut tree_actions: Vec<(PaneId, crate::tree_actions::TreeAction)> = Vec::new();
        {
            let mut viewer = crate::docking::NaygoTabViewer {
                workspace: &mut self.workspace,
                status: &mut self.status,
                pending: &mut pending,
                icons: &self.icons,
                show_parent_entry: self.settings.show_parent_entry,
                i18n: &self.i18n,
                trees: &self.trees,
                tree_actions: &mut tree_actions,
            };
            egui_dock::DockArea::new(&mut self.dock_state)
                .style(egui_dock::Style::from_egui(ui.style().as_ref()))
                .show_inside(ui, &mut viewer);
        }
```
PROBLEMA DE BORROW: `viewer` toma `&self.trees` (inmutable) y `&mut self.workspace`. Ambos son campos distintos de `self`, así que el borrow disjunto es válido en Rust 2021 dentro del mismo método. Pero `viewer.trees` no puede ser `&self.trees` mientras otras partes tomen `&mut self`. Como todo el bloque está acotado por `{ }` y no se toca `self.trees` mutablemente dentro, compila. Verificar; si el borrow falla, clonar la referencia necesaria no es trivial → alternativa: pasar `self.trees` por `std::mem::take` a un local antes del bloque y devolverlo después:
```rust
        let trees_snapshot = std::mem::take(&mut self.trees);
        // ... construir viewer con trees: &trees_snapshot ...
        // tras el bloque:
        self.trees = trees_snapshot;
```
Usar la variante `mem::take` si el borrow directo no compila (es segura: el árbol no se muta durante el pintado, las mutaciones van por `tree_actions` después).

c) DESPUÉS del bucle `for req in pending { ... }` existente, procesar las acciones del árbol:
```rust
        for (id, action) in tree_actions {
            match action {
                crate::tree_actions::TreeAction::Expand(path) => {
                    self.tree_expand(id, path);
                }
                crate::tree_actions::TreeAction::Collapse(path) => {
                    self.tree_collapse(id, path);
                }
                crate::tree_actions::TreeAction::Navigate(path) => {
                    // Navegar el panel ACTIVO a esa carpeta (mismo efecto que el
                    // file panel). Reutiliza el flujo NavigateTo.
                    if let Some(active) = self.workspace.active_id() {
                        if let Some(f) =
                            self.workspace.pane_mut(active).and_then(|p| p.files.as_mut())
                        {
                            f.navigate_to(path.clone());
                            self.start_listing(active, path);
                        }
                    }
                }
            }
        }
```

d) Quitar los `#[allow(dead_code)]` que se pusieron en la Tarea 8 sobre `ensure_tree`/`tree_expand`/`tree_collapse` (ahora se usan).

e) Limpiar el `reveal_to` tras pintar para no hacer scroll en bucle: al final del método `ui`, tras procesar acciones, añadir:
```rust
        // El scroll de reveal ya se aplicó durante el pintado; limpiarlo.
        for id in &tree_pane_ids {
            if let Some(t) = self.trees.get_mut(id) {
                if t.reveal_to.is_some() {
                    t.clear_reveal();
                }
            }
        }
```

- [ ] **Step 4: Compilar, verificar, formatear**

Run: `cargo build -p naygo-ui` → compila. Resolver borrows según las notas (usar `mem::take` si hace falta).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start (`--bin naygo`): el panel "Carpetas" muestra las unidades; clic en ▶ expande (spinner → subcarpetas, solo carpetas); clic en el nombre navega el panel activo; al navegar el panel activo (teclado/doble clic) el árbol revela y resalta la carpeta (barra azul + fondo tenue) y hace scroll a ella; una carpeta sin subcarpetas muestra "(sin subcarpetas)"; una sin permiso "⚠ acceso denegado".

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/panes/tree_panel.rs crates/ui/src/docking.rs crates/ui/src/app.rs
git commit -m "feat(ui): árbol de directorios funcional (render, lazy-load, navegación, auto-reveal)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: Actualizar el README**

Modify `README.md` — el bloque de estado, reemplazar el actual por:
```markdown
> **Estado:** Árbol de directorios (panel Carpetas) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-06-naygo-arbol-directorios-design.md`](docs/superpowers/specs/2026-06-06-naygo-arbol-directorios-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-06-naygo-arbol-directorios.md`](docs/superpowers/plans/2026-06-06-naygo-arbol-directorios.md).
> Fases 1, 2A (layout), 2B (íconos), 2C-i (configuración + i18n) y 2D (pulidos) completas.
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → todo verde (core: tree + listing solo-dir; platform: drives smoke; ui: tree_action).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo fmt --check` → limpio (si no, `cargo fmt` e incluir).
Run: `cargo build --release -p naygo-ui` → release compila.
App-start manual (`--bin naygo`): repasar el checklist de la Tarea 9 Step 4.

- [ ] **Step 3: Commit y push**

```bash
git add README.md
git commit -m "chore: actualizar estado del README (árbol de directorios)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/arbol-directorios
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| Modelo `core::tree` (TreeNode, NodeState, DirTree) | 1 |
| Transiciones (expandir, push_child streaming, finish, collapse conserva hijos) | 2 |
| Auto-reveal (set_active, reveal_chain, clear_reveal) | 3 |
| Listado solo-directorios | 4 |
| `platform::drives()` (Win32 + stub) | 5 |
| Raíces = unidades | 5 + 8 (ensure_tree) |
| i18n nuevas (cargando/vacía/error) | 6 |
| Acciones diferidas del árbol | 7 |
| Workers por (panel, path) + pump_tree + repaint | 8 |
| Creación perezosa del DirTree por panel Tree | 8 (ensure_tree) + 9 (llamado) |
| Render recursivo, triángulo, ícono, spinner | 9 |
| Clic flecha expande / clic nombre navega | 9 |
| Resaltado modo B (barra azul + fondo tenue) | 9 |
| Estados vacía/error visibles | 9 |
| Scroll al revelar | 9 |
| Auto-sync en cascada (sin límite) | 9 |
| Colapsar conserva hijos | 2 (modelo) + 9 (UI) |
| Multi-panel (DirTree por PaneId) | 8 + 9 |

**Notas de riesgo:**
- egui 0.34.3 API: verificar `Label::sense`, `SelectableLabel`, `ui.spinner`, `scroll_to_me`, `painter().rect_filled`, `Response::rect` contra la fuente del registry antes de implementar la Tarea 9 (patrón ya usado en fases previas).
- Borrow en `app.rs` Tarea 9: `&self.trees` + `&mut self.workspace` disjuntos; si el compilador se queja, usar `std::mem::take` del mapa de árboles durante el pintado (incluido en el plan).
- Resaltado modo B con `painter.rect_filled` se pinta sobre la fila ya dibujada → podría tapar el texto. Alternativa con `Frame`/fondo previo incluida en la nota de la Tarea 9; decidir por validación visual.
- Auto-reveal en cascada converge en varios frames (cada nivel lista async); el bloque recalcula `reveal_chain` mientras `active_path` no cambie. Sin bloqueo, sin recursión infinita (cadena finita).
- `windows` features: si `GetLogicalDriveStringsW`/`GetDriveTypeW` no aparecen con `Win32_Storage_FileSystem`, ajustar según el error (la API existe en 0.62).
