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
        if let Some(node) = self.root.take() {
            self.root = remove_in(node, id);
        }
    }
}

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
                Rect {
                    x: area.x,
                    y: area.y,
                    w: first_w,
                    h: area.h,
                },
                Rect {
                    x: second_x,
                    y: area.y,
                    w: second_w,
                    h: area.h,
                },
            )
        }
        SplitDir::Vertical => {
            let first_h = (area.h * f - half).max(0.0);
            let second_y = area.y + area.h * f + half;
            let second_h = (area.y + area.h - second_y).max(0.0);
            (
                Rect {
                    x: area.x,
                    y: area.y,
                    w: area.w,
                    h: first_h,
                },
                Rect {
                    x: area.x,
                    y: second_y,
                    w: area.w,
                    h: second_h,
                },
            )
        }
    }
}

/// Recorre el árbol acumulando el handle (barra) de cada split. La barra ocupa el hueco
/// `SPLIT_BAR` entre los dos hijos.
fn handles(node: &DockNode, area: Rect, path: &mut Vec<SplitStep>, out: &mut Vec<SplitHandle>) {
    if let DockNode::Split {
        dir,
        fraction,
        first,
        second,
    } = node
    {
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
        out.push(SplitHandle {
            path: path.clone(),
            rect: bar,
            dir: *dir,
        });
        let (a, b) = split_area(area, *dir, *fraction);
        path.push(SplitStep::First);
        handles(first, a, path, out);
        path.pop();
        path.push(SplitStep::Second);
        handles(second, b, path, out);
        path.pop();
    }
}

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

    fn rect_eq(a: Rect, x: f32, y: f32, w: f32, h: f32) -> bool {
        (a.x - x).abs() < 0.01
            && (a.y - y).abs() < 0.01
            && (a.w - w).abs() < 0.01
            && (a.h - h).abs() < 0.01
    }

    #[test]
    fn pane_rects_un_panel_ocupa_todo() {
        let l = SerializableDockLayout::single(PaneId(1));
        let rects = l.pane_rects(Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        });
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
        let rects = l.pane_rects(Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        });
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
        let rects = l.pane_rects(Rect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 400.0,
        });
        let r1 = rects.iter().find(|(id, _)| *id == PaneId(1)).unwrap().1;
        let r2 = rects.iter().find(|(id, _)| *id == PaneId(2)).unwrap().1;
        assert!((r1.w - 400.0).abs() < 0.01, "vertical: mismo ancho");
        assert!(r2.y > r1.y + r1.h - 0.1, "2º debajo del 1º");
    }

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
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        let handles = l.split_handles(area);
        assert_eq!(handles.len(), 1, "un split, un handle");
        let h = &handles[0];
        assert_eq!(h.dir, SplitDir::Horizontal);
        assert!((h.rect.x - 398.0).abs() < 3.0);
        assert!((h.rect.h - 600.0).abs() < 0.01);
        let path = h.path.clone();
        l.set_fraction(&path, 0.25);
        let r1 = l
            .pane_rects(area)
            .iter()
            .find(|(id, _)| *id == PaneId(1))
            .unwrap()
            .1;
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
        l.set_fraction(&[], 2.0);
        if let Some(DockNode::Split { fraction, .. }) = &l.root {
            assert!(*fraction <= 0.95 && *fraction >= 0.05);
        } else {
            panic!("raíz debe seguir siendo split");
        }
    }

    #[test]
    fn split_leaf_divide_la_hoja() {
        let mut l = SerializableDockLayout::single(PaneId(1));
        l.split_leaf(PaneId(1), SplitDir::Horizontal, PaneId(2));
        assert_eq!(l.pane_ids(), vec![PaneId(1), PaneId(2)]);
        if let Some(DockNode::Split {
            dir, first, second, ..
        }) = &l.root
        {
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
        assert_eq!(l.root, Some(DockNode::Leaf(PaneId(2))));
    }

    #[test]
    fn remove_leaf_unico_deja_vacio() {
        let mut l = SerializableDockLayout::single(PaneId(1));
        l.remove_leaf(PaneId(1));
        assert_eq!(l.root, None);
    }
}
