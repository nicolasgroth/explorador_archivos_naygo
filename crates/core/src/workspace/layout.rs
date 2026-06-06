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
