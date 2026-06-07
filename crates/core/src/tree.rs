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
