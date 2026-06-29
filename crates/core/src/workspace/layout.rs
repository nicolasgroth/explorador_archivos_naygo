// Naygo — disposición serializable, desacoplada de egui_dock.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

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

/// Un nodo del árbol de disposición: una hoja (un panel), un grupo de pestañas
/// (varios paneles apilados en el mismo rect) o un split de dos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DockNode {
    /// Una hoja: el panel con este id ocupa el espacio.
    Leaf(PaneId),
    /// Un grupo de pestañas: varios paneles comparten el MISMO rect; `active` es el índice
    /// (en `0`) de la pestaña visible. Invariante: `members` no vacío y `active < len`.
    /// Un grupo que queda con un solo miembro se colapsa a `Leaf` (ver `remove_in`).
    Tabs { members: Vec<PaneId>, active: usize },
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

/// Una zona de drop al reacomodar por arrastre: dónde, sobre qué panel, cae lo arrastrado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropZone {
    /// Apilar como pestaña en el panel destino (centro).
    Center,
    /// Dividir: el panel arrastrado queda a la izquierda/derecha/arriba/abajo del destino.
    Left,
    Right,
    Top,
    Bottom,
}

/// Las cinco zonas de drop de un panel (centro + 4 bordes), con su rect, dado el rect del
/// panel. El centro ocupa la franja media; los bordes, una banda de `BORDE` de ancho.
pub fn drop_zones(pane: Rect) -> Vec<(DropZone, Rect)> {
    // Banda de borde: 28% del lado (acotada) para que sea fácil de apuntar sin GPU.
    let bw = (pane.w * 0.28).min(pane.w / 2.0);
    let bh = (pane.h * 0.28).min(pane.h / 2.0);
    vec![
        (
            DropZone::Left,
            Rect {
                x: pane.x,
                y: pane.y,
                w: bw,
                h: pane.h,
            },
        ),
        (
            DropZone::Right,
            Rect {
                x: pane.x + pane.w - bw,
                y: pane.y,
                w: bw,
                h: pane.h,
            },
        ),
        (
            DropZone::Top,
            Rect {
                x: pane.x,
                y: pane.y,
                w: pane.w,
                h: bh,
            },
        ),
        (
            DropZone::Bottom,
            Rect {
                x: pane.x,
                y: pane.y + pane.h - bh,
                w: pane.w,
                h: bh,
            },
        ),
        // El centro va al final: tiene prioridad al hacer hit-test desde adentro.
        (
            DropZone::Center,
            Rect {
                x: pane.x + bw,
                y: pane.y + bh,
                w: (pane.w - 2.0 * bw).max(0.0),
                h: (pane.h - 2.0 * bh).max(0.0),
            },
        ),
    ]
}

/// Resuelve sobre qué panel y zona cae el punto `(px, py)`, dados los rects de los paneles.
/// Devuelve (PaneId destino, zona). Prioriza el CENTRO si el punto cae en él; si no, el
/// borde correspondiente. `None` si el punto no está sobre ningún panel.
pub fn drop_hit(panes: &[(PaneId, Rect)], px: f32, py: f32) -> Option<(PaneId, DropZone)> {
    for (id, r) in panes {
        if px < r.x || px >= r.x + r.w || py < r.y || py >= r.y + r.h {
            continue;
        }
        // Distancia a cada borde; el menor define la zona, salvo que esté en el centro.
        let zones = drop_zones(*r);
        // El centro es la última; si el punto cae en su rect, gana.
        if let Some((_, center)) = zones.iter().find(|(z, _)| *z == DropZone::Center) {
            if px >= center.x
                && px < center.x + center.w
                && py >= center.y
                && py < center.y + center.h
            {
                return Some((*id, DropZone::Center));
            }
        }
        // Si no, el borde más cercano.
        let dl = px - r.x;
        let dr = (r.x + r.w) - px;
        let dt = py - r.y;
        let db = (r.y + r.h) - py;
        let min = dl.min(dr).min(dt).min(db);
        let zone = if min == dl {
            DropZone::Left
        } else if min == dr {
            DropZone::Right
        } else if min == dt {
            DropZone::Top
        } else {
            DropZone::Bottom
        };
        return Some((*id, zone));
    }
    None
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
                // Una hoja o un grupo de pestañas no tiene sub-splits que recorrer.
                DockNode::Leaf(_) | DockNode::Tabs { .. } => return,
            }
        }
        if let DockNode::Split { fraction: fr, .. } = node {
            *fr = fraction.clamp(0.05, 0.95);
        }
    }

    /// Dado el split en `path`, el `area` total y la posición del puntero `(px, py)` en coords de
    /// contenido, calcula (fraction, bar_rect): la fracción CLAMP [0.05,0.95] que pondría el corte
    /// bajo el puntero, y el rect de la barra resultante. La fracción se mide DENTRO del sub-rect
    /// del split (no sobre el área total), que es lo correcto para splits anidados. Devuelve
    /// `None` si la ruta no apunta a un split. Lo usan tanto la barra-fantasma del arrastre (vista
    /// previa en vivo) como el commit al soltar, para que ambos coincidan.
    pub fn fraction_at(
        &self,
        path: &[SplitStep],
        area: Rect,
        px: f32,
        py: f32,
    ) -> Option<(f32, Rect)> {
        let root = self.root.as_ref()?;
        // Bajar al split objetivo, recortando el sub-rect en cada paso.
        let mut node = root;
        let mut sub = area;
        for step in path {
            if let DockNode::Split {
                dir,
                fraction,
                first,
                second,
            } = node
            {
                let (a, b) = split_area(sub, *dir, *fraction);
                match step {
                    SplitStep::First => {
                        node = first;
                        sub = a;
                    }
                    SplitStep::Second => {
                        node = second;
                        sub = b;
                    }
                }
            } else {
                return None;
            }
        }
        let DockNode::Split { dir, .. } = node else {
            return None;
        };
        let half = SPLIT_BAR / 2.0;
        let f = match dir {
            SplitDir::Horizontal => {
                if sub.w <= 0.0 {
                    0.5
                } else {
                    ((px - sub.x) / sub.w).clamp(0.05, 0.95)
                }
            }
            SplitDir::Vertical => {
                if sub.h <= 0.0 {
                    0.5
                } else {
                    ((py - sub.y) / sub.h).clamp(0.05, 0.95)
                }
            }
        };
        let bar = match dir {
            SplitDir::Horizontal => Rect {
                x: sub.x + sub.w * f - half,
                y: sub.y,
                w: SPLIT_BAR,
                h: sub.h,
            },
            SplitDir::Vertical => Rect {
                x: sub.x,
                y: sub.y + sub.h * f - half,
                w: sub.w,
                h: SPLIT_BAR,
            },
        };
        Some((f, bar))
    }

    /// Divide la hoja `leaf` en un split: el lado nuevo lleva `new_id`. Si `leaf` no
    /// existe, no-op. El split nuevo arranca al 50%.
    pub fn split_leaf(&mut self, leaf: PaneId, dir: SplitDir, new_id: PaneId) {
        if let Some(root) = self.root.as_mut() {
            split_in(root, leaf, dir, new_id);
        }
    }

    /// Quita la hoja `id` y colapsa el split que la contenía (el hermano sube a su lugar).
    /// Si era la única hoja, el layout queda vacío. Si `id` era miembro de un grupo de
    /// pestañas, se quita del grupo (que se colapsa a hoja si queda uno solo).
    pub fn remove_leaf(&mut self, id: PaneId) {
        if let Some(node) = self.root.take() {
            self.root = remove_in(node, id);
        }
    }

    /// Apila `new_id` como una pestaña más en el nodo que contiene a `onto`: si `onto` es
    /// una `Leaf`, se convierte en un `Tabs` de dos; si ya es un `Tabs`, se agrega al final.
    /// La pestaña recién agregada queda activa. No-op si `onto` no está en el layout.
    pub fn stack_onto(&mut self, onto: PaneId, new_id: PaneId) {
        if let Some(root) = self.root.as_mut() {
            stack_in(root, onto, new_id);
        }
    }

    /// Marca la pestaña `member` como activa dentro de su grupo. No-op si `member` no
    /// pertenece a ningún grupo de pestañas.
    pub fn set_active_tab(&mut self, member: PaneId) {
        if let Some(root) = self.root.as_mut() {
            activate_in(root, member);
        }
    }

    /// Devuelve, para cada grupo de pestañas del layout, sus miembros y el índice activo.
    /// Las hojas simples no aparecen (no son grupos). Útil para que la UI pinte las barras
    /// de pestañas sabiendo qué paneles se apilan en un mismo rect y cuál se ve.
    pub fn tab_groups(&self) -> Vec<(Vec<PaneId>, usize)> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            collect_groups(root, &mut out);
        }
        out
    }

    /// Intercambia los dos hijos del split cuyo primer subárbol contiene a `a` y el segundo
    /// a `b` (o viceversa). Sirve para colocar un panel recién dividido del lado opuesto
    /// (drop a la izquierda/arriba). No-op si no hay un split que separe a `a` de `b`.
    pub fn swap_split_children(&mut self, a: PaneId, b: PaneId) {
        if let Some(root) = self.root.as_mut() {
            swap_children_in(root, a, b);
        }
    }
}

/// Busca el split que separa `a` de `b` (uno en cada subárbol) e intercambia sus hijos.
fn swap_children_in(node: &mut DockNode, a: PaneId, b: PaneId) -> bool {
    if let DockNode::Split { first, second, .. } = node {
        let fa = subtree_contains(first, a);
        let fb = subtree_contains(first, b);
        let sa = subtree_contains(second, a);
        let sb = subtree_contains(second, b);
        // Este es el split que los separa (a en uno, b en el otro).
        if (fa && sb) || (fb && sa) {
            std::mem::swap(first, second);
            return true;
        }
        // Si no, bajar al subárbol que contenga ambos.
        return swap_children_in(first, a, b) || swap_children_in(second, a, b);
    }
    false
}

/// `true` si el subárbol `node` contiene la hoja/miembro `id`.
fn subtree_contains(node: &DockNode, id: PaneId) -> bool {
    match node {
        DockNode::Leaf(leaf) => *leaf == id,
        DockNode::Tabs { members, .. } => members.contains(&id),
        DockNode::Split { first, second, .. } => {
            subtree_contains(first, id) || subtree_contains(second, id)
        }
    }
}

/// Coloca recursivamente `node` dentro de `area`, acumulando los rects de las hojas.
fn place(node: &DockNode, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
    match node {
        DockNode::Leaf(id) => out.push((*id, area)),
        // Las pestañas comparten el mismo rect (apiladas): cada miembro recibe `area`. La
        // UI muestra solo la activa y pinta la barra de pestañas; el resto queda detrás.
        DockNode::Tabs { members, .. } => {
            for id in members {
                out.push((*id, area));
            }
        }
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
            // Mínimo 1.0 px para que el render por software nunca reciba w=0
            // (Slint castea f32→i32 y paniquea con geometrías degeneradas).
            let first_w = (area.w * f - half).max(1.0);
            let second_x = area.x + area.w * f + half;
            let second_w = (area.x + area.w - second_x).max(1.0);
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
            // Mínimo 1.0 px por la misma razón (h=0 también paniquea).
            let first_h = (area.h * f - half).max(1.0);
            let second_y = area.y + area.h * f + half;
            let second_h = (area.y + area.h - second_y).max(1.0);
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
        // Si `leaf` es uno de los miembros del grupo, el grupo entero se divide: el grupo
        // queda en el primer lado y el panel nuevo en el segundo.
        DockNode::Tabs { members, .. } if members.contains(&leaf) => {
            let group = node.clone();
            *node = DockNode::Split {
                dir,
                fraction: 0.5,
                first: Box::new(group),
                second: Box::new(DockNode::Leaf(new_id)),
            };
        }
        DockNode::Tabs { .. } => {}
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
        // Quitar un miembro del grupo: si quedan ≥2 sigue siendo Tabs (ajustando `active`);
        // si queda 1 se colapsa a Leaf; si queda 0 desaparece.
        DockNode::Tabs {
            mut members,
            mut active,
        } => {
            members.retain(|m| *m != id);
            match members.len() {
                0 => None,
                1 => Some(DockNode::Leaf(members[0])),
                n => {
                    if active >= n {
                        active = n - 1;
                    }
                    Some(DockNode::Tabs { members, active })
                }
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
        DockNode::Tabs { members, .. } => out.extend_from_slice(members),
        DockNode::Split { first, second, .. } => {
            collect(first, out);
            collect(second, out);
        }
    }
}

/// Apila `new_id` en el nodo que contiene a `onto` (hoja → grupo de 2; grupo → +1 al final).
fn stack_in(node: &mut DockNode, onto: PaneId, new_id: PaneId) {
    match node {
        DockNode::Leaf(id) if *id == onto => {
            *node = DockNode::Tabs {
                members: vec![*id, new_id],
                active: 1,
            };
        }
        DockNode::Leaf(_) => {}
        DockNode::Tabs { members, active } if members.contains(&onto) => {
            members.push(new_id);
            *active = members.len() - 1;
        }
        DockNode::Tabs { .. } => {}
        DockNode::Split { first, second, .. } => {
            stack_in(first, onto, new_id);
            stack_in(second, onto, new_id);
        }
    }
}

/// Marca `member` como pestaña activa de su grupo (si pertenece a alguno).
fn activate_in(node: &mut DockNode, member: PaneId) {
    match node {
        DockNode::Leaf(_) => {}
        DockNode::Tabs { members, active } => {
            if let Some(pos) = members.iter().position(|m| *m == member) {
                *active = pos;
            }
        }
        DockNode::Split { first, second, .. } => {
            activate_in(first, member);
            activate_in(second, member);
        }
    }
}

/// Acumula (miembros, activo) de cada grupo de pestañas del árbol.
fn collect_groups(node: &DockNode, out: &mut Vec<(Vec<PaneId>, usize)>) {
    match node {
        DockNode::Leaf(_) => {}
        DockNode::Tabs { members, active } => out.push((members.clone(), *active)),
        DockNode::Split { first, second, .. } => {
            collect_groups(first, out);
            collect_groups(second, out);
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
    fn fraction_at_mide_dentro_del_subrect_del_split() {
        // Layout: split horizontal raíz al 50%; el segundo hijo es OTRO split horizontal.
        // El handle del split ANIDADO vive en la mitad derecha [400..800]. Poner el puntero en
        // x=600 (el centro de esa mitad) debe dar fraction ~0.5 del SUB-rect, no ~0.75 del total.
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.5,
                first: Box::new(DockNode::Leaf(PaneId(1))),
                second: Box::new(DockNode::Split {
                    dir: SplitDir::Horizontal,
                    fraction: 0.5,
                    first: Box::new(DockNode::Leaf(PaneId(2))),
                    second: Box::new(DockNode::Leaf(PaneId(3))),
                }),
            }),
        };
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        // El handle anidado es el segundo (path = [Second]).
        let handles = l.split_handles(area);
        let nested = handles
            .iter()
            .find(|h| h.path == vec![SplitStep::Second])
            .unwrap();
        let (f, bar) = l.fraction_at(&nested.path, area, 600.0, 300.0).unwrap();
        assert!(
            (f - 0.5).abs() < 0.02,
            "fraction relativa al sub-rect, no al total: {f}"
        );
        assert!(
            (bar.x - 598.0).abs() < 3.0,
            "la barra-fantasma cae en x~600: {}",
            bar.x
        );
        // Clamp en los extremos.
        let (fmin, _) = l.fraction_at(&nested.path, area, 0.0, 300.0).unwrap();
        assert!((fmin - 0.05).abs() < 0.001, "clamp mínimo");
        // Ruta a una hoja → None.
        assert!(l
            .fraction_at(&[SplitStep::First], area, 100.0, 100.0)
            .is_none());
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

    #[test]
    fn stack_onto_convierte_hoja_en_grupo() {
        let mut l = SerializableDockLayout::single(PaneId(1));
        l.stack_onto(PaneId(1), PaneId(2));
        assert_eq!(
            l.root,
            Some(DockNode::Tabs {
                members: vec![PaneId(1), PaneId(2)],
                active: 1,
            })
        );
        // Apilar otra más: se agrega al final y queda activa.
        l.stack_onto(PaneId(2), PaneId(3));
        assert_eq!(
            l.root,
            Some(DockNode::Tabs {
                members: vec![PaneId(1), PaneId(2), PaneId(3)],
                active: 2,
            })
        );
    }

    #[test]
    fn pane_ids_incluye_todos_los_miembros_del_grupo() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Tabs {
                members: vec![PaneId(5), PaneId(6), PaneId(7)],
                active: 0,
            }),
        };
        assert_eq!(l.pane_ids(), vec![PaneId(5), PaneId(6), PaneId(7)]);
    }

    #[test]
    fn pane_rects_de_un_grupo_da_el_mismo_rect_a_todos() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Tabs {
                members: vec![PaneId(1), PaneId(2)],
                active: 0,
            }),
        };
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        let rects = l.pane_rects(area);
        assert_eq!(rects.len(), 2);
        assert!(rect_eq(rects[0].1, 0.0, 0.0, 800.0, 600.0));
        assert!(rect_eq(rects[1].1, 0.0, 0.0, 800.0, 600.0));
    }

    #[test]
    fn set_active_tab_cambia_la_pestana_visible() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Tabs {
                members: vec![PaneId(1), PaneId(2), PaneId(3)],
                active: 0,
            }),
        };
        l.set_active_tab(PaneId(3));
        assert_eq!(
            l.tab_groups(),
            vec![(vec![PaneId(1), PaneId(2), PaneId(3)], 2)]
        );
    }

    #[test]
    fn remove_de_grupo_colapsa_a_hoja_cuando_queda_uno() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Tabs {
                members: vec![PaneId(1), PaneId(2)],
                active: 1,
            }),
        };
        l.remove_leaf(PaneId(2));
        assert_eq!(l.root, Some(DockNode::Leaf(PaneId(1))));
    }

    #[test]
    fn remove_de_grupo_ajusta_el_activo_fuera_de_rango() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Tabs {
                members: vec![PaneId(1), PaneId(2), PaneId(3)],
                active: 2,
            }),
        };
        l.remove_leaf(PaneId(3));
        assert_eq!(
            l.root,
            Some(DockNode::Tabs {
                members: vec![PaneId(1), PaneId(2)],
                active: 1,
            })
        );
    }

    #[test]
    fn split_leaf_dentro_de_un_grupo_divide_el_grupo() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Tabs {
                members: vec![PaneId(1), PaneId(2)],
                active: 0,
            }),
        };
        l.split_leaf(PaneId(1), SplitDir::Horizontal, PaneId(9));
        // El grupo entero queda en el primer lado; el panel nuevo en el segundo.
        if let Some(DockNode::Split { first, second, .. }) = &l.root {
            assert_eq!(
                **first,
                DockNode::Tabs {
                    members: vec![PaneId(1), PaneId(2)],
                    active: 0,
                }
            );
            assert_eq!(**second, DockNode::Leaf(PaneId(9)));
        } else {
            panic!("la raíz debe ser un split");
        }
    }

    #[test]
    fn drop_hit_centro_y_bordes() {
        let panes = vec![(
            PaneId(1),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
        )];
        // Centro (50,50) → Center.
        assert_eq!(
            drop_hit(&panes, 50.0, 50.0),
            Some((PaneId(1), DropZone::Center))
        );
        // Cerca del borde izquierdo (5,50) → Left.
        assert_eq!(
            drop_hit(&panes, 5.0, 50.0),
            Some((PaneId(1), DropZone::Left))
        );
        // Cerca del borde superior (50,5) → Top.
        assert_eq!(
            drop_hit(&panes, 50.0, 5.0),
            Some((PaneId(1), DropZone::Top))
        );
        // Cerca del borde derecho (95,50) → Right.
        assert_eq!(
            drop_hit(&panes, 95.0, 50.0),
            Some((PaneId(1), DropZone::Right))
        );
        // Fuera de todo panel → None.
        assert_eq!(drop_hit(&panes, 200.0, 200.0), None);
    }

    #[test]
    fn drop_hit_elige_el_panel_correcto() {
        let panes = vec![
            (
                PaneId(1),
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 100.0,
                },
            ),
            (
                PaneId(2),
                Rect {
                    x: 100.0,
                    y: 0.0,
                    w: 100.0,
                    h: 100.0,
                },
            ),
        ];
        // Punto en el segundo panel, al centro.
        assert_eq!(
            drop_hit(&panes, 150.0, 50.0),
            Some((PaneId(2), DropZone::Center))
        );
    }

    #[test]
    fn round_trip_serde_con_grupo() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Tabs {
                members: vec![PaneId(1), PaneId(2)],
                active: 1,
            }),
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: SerializableDockLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }
}
