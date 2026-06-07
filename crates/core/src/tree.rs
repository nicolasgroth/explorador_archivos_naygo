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
                    let has = node
                        .children
                        .as_ref()
                        .map(|c| !c.is_empty())
                        .unwrap_or(false);
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
        // Si el destino ES la raíz, no hay nada que expandir.
        if target == root {
            return Vec::new();
        }
        // Construir desde la raíz: root, root/a, root/a/b, ... sin incluir target.
        let mut chain = Vec::new();
        let mut acc = root.clone();
        chain.push(acc.clone());
        if let Ok(rel) = target.strip_prefix(&root) {
            let comps: Vec<_> = rel.components().collect();
            // Todos menos el último componente (ese es el destino, no se expande).
            for comp in comps.iter().take(comps.len().saturating_sub(1)) {
                acc = acc.join(comp.as_os_str());
                chain.push(acc.clone());
            }
        }
        chain
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

    #[test]
    fn begin_loading_marca_loading_y_expandido() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        let n = t.node_at(Path::new("C:\\")).unwrap();
        assert_eq!(n.state, NodeState::Loading);
        assert!(n.expanded);
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
        assert_eq!(names, vec!["apps", "Users", "Windows"]);
    }

    #[test]
    fn finish_loading_con_hijos_queda_loaded() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        t.push_child(Path::new("C:\\"), PathBuf::from("C:\\Users"));
        t.finish_loading(Path::new("C:\\"), NodeOutcome::Done);
        assert_eq!(
            t.node_at(Path::new("C:\\")).unwrap().state,
            NodeState::Loaded
        );
    }

    #[test]
    fn finish_loading_sin_hijos_queda_empty() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        t.finish_loading(Path::new("C:\\"), NodeOutcome::Done);
        assert_eq!(
            t.node_at(Path::new("C:\\")).unwrap().state,
            NodeState::Empty
        );
    }

    #[test]
    fn finish_loading_error_queda_error() {
        let mut t = DirTree::from_drives(&drive_list());
        t.begin_loading(Path::new("C:\\"));
        t.finish_loading(Path::new("C:\\"), NodeOutcome::Error);
        assert_eq!(
            t.node_at(Path::new("C:\\")).unwrap().state,
            NodeState::Error
        );
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
        assert_eq!(n.children.as_ref().map(|c| c.len()), Some(1));
        assert_eq!(n.state, NodeState::Loaded);
    }

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
        let chain = t.reveal_chain(Path::new("C:\\"));
        assert!(chain.is_empty());
    }

    #[test]
    fn reveal_chain_sin_raiz_conocida_es_vacia() {
        let t = DirTree::from_drives(&drive_list());
        let chain = t.reveal_chain(Path::new("Z:\\algo"));
        assert!(chain.is_empty());
    }

    #[test]
    fn clear_reveal_borra_el_pendiente() {
        let mut t = DirTree::from_drives(&drive_list());
        t.set_active(PathBuf::from("C:\\Users"));
        t.clear_reveal();
        assert_eq!(t.reveal_to, None);
        assert_eq!(t.active_path, Some(PathBuf::from("C:\\Users")));
    }
}
