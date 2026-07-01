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
/// (varios paneles apilados en el mismo rect) o un split de N hijos con pesos.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum DockNode {
    /// Una hoja: el panel con este id ocupa el espacio.
    Leaf(PaneId),
    /// Un grupo de pestañas: varios paneles comparten el MISMO rect; `active` es el índice
    /// (en `0`) de la pestaña visible. Invariante: `members` no vacío y `active < len`.
    /// Un grupo que queda con un solo miembro se colapsa a `Leaf` (ver `remove_in`).
    Tabs { members: Vec<PaneId>, active: usize },
    /// Un split: una fila (Horizontal) o columna (Vertical) de N≥2 hijos, cada uno con
    /// un peso relativo (`weights[i]`). El ancho/alto de cada hijo se reparte por
    /// `weights[i] / Σweights`. Los divisores viven entre hijos consecutivos: hay
    /// `children.len() - 1` divisores por split. Invariantes: `children.len() ==
    /// weights.len() >= 2`; `weights[i] > 0`. Un split que quedaría con un solo hijo se
    /// colapsa a ese hijo (ver `remove_in`).
    Split {
        dir: SplitDir,
        children: Vec<DockNode>,
        weights: Vec<f32>,
    },
}

/// Formato "de cable" para deserializar `DockNode`: acepta tanto el formato N-ario actual
/// (`children`/`weights`) como el formato binario viejo (`fraction`/`first`/`second`), para
/// que las disposiciones guardadas antes de la migración a pesos sigan cargando.
#[derive(Deserialize)]
enum DockNodeWire {
    Leaf(PaneId),
    Tabs {
        members: Vec<PaneId>,
        active: usize,
    },
    Split {
        dir: SplitDir,
        #[serde(default)]
        children: Vec<DockNodeWire>,
        #[serde(default)]
        weights: Vec<f32>,
        #[serde(default)]
        fraction: Option<f32>,
        #[serde(default)]
        first: Option<Box<DockNodeWire>>,
        #[serde(default)]
        second: Option<Box<DockNodeWire>>,
    },
}

/// Colapsa un split recién deserializado a su forma canónica: 1 hijo → ese hijo; ≥2 → Split.
/// Solo se llama con `children` no vacío (el caso de 0 hijos se resuelve antes, en
/// `dock_from_wire`, porque un split sin contenido no puede representarse como `DockNode`).
fn make_split(dir: SplitDir, mut children: Vec<DockNode>, mut weights: Vec<f32>) -> DockNode {
    // Defensa: pesos de largo distinto (p. ej. `workspace.json` editado a mano) → uniformes.
    if weights.len() != children.len() {
        weights = vec![1.0; children.len()];
    }
    if children.len() == 1 {
        return children.pop().unwrap();
    }
    DockNode::Split {
        dir,
        children,
        weights,
    }
}

/// Convierte el formato de cable a `DockNode`, colapsando/descartando los casos degenerados
/// en vez de producir un `Split` que viole la invariante `children.len() >= 2`:
/// - `Tabs` sin miembros → `None` (un grupo de pestañas vacío no representa nada).
/// - `Split` binario viejo (`first`/`second`): convierte cada lado presente; si un lado falta
///   o es a su vez degenerado, el otro sobrevive como hijo único (no se pierde el panel); si
///   ambos faltan, `None`.
/// - `Split` N-ario: convierte cada hijo, descarta los `None` (y su peso correspondiente); si
///   no queda ninguno, `None`.
///
/// Devuelve `None` cuando el nodo no puede representar ningún panel; el `Deserialize` de
/// `DockNode` traduce ese caso a un error (no puede devolver `Option` al no ser fallible).
fn dock_from_wire(w: DockNodeWire) -> Option<DockNode> {
    match w {
        DockNodeWire::Leaf(id) => Some(DockNode::Leaf(id)),
        DockNodeWire::Tabs { members, active } => {
            if members.is_empty() {
                None
            } else {
                Some(DockNode::Tabs { members, active })
            }
        }
        DockNodeWire::Split {
            dir,
            children,
            weights,
            fraction,
            first,
            second,
        } => {
            if first.is_some() || second.is_some() {
                // Formato binario viejo: recoge el/los lado(s) presentes y convertibles.
                // Si solo hay uno (el otro faltaba o era degenerado), sobrevive como hijo
                // único en vez de descartarse en silencio (FIX I-2).
                let f = first.and_then(|b| dock_from_wire(*b));
                let s = second.and_then(|b| dock_from_wire(*b));
                return match (f, s) {
                    (Some(f), Some(s)) => {
                        let frac = fraction.unwrap_or(0.5).clamp(0.05, 0.95);
                        Some(make_split(dir, vec![f, s], vec![frac, 1.0 - frac]))
                    }
                    (Some(only), None) | (None, Some(only)) => Some(only),
                    (None, None) => None,
                };
            }
            // Formato N-ario: convierte cada hijo, descarta los degenerados y su peso.
            let mut kept: Vec<DockNode> = Vec::new();
            let mut kw: Vec<f32> = Vec::new();
            let weights_or_default = if weights.len() == children.len() {
                weights
            } else {
                vec![1.0; children.len()]
            };
            for (c, w) in children.into_iter().zip(weights_or_default) {
                if let Some(c) = dock_from_wire(c) {
                    kept.push(c);
                    kw.push(w);
                }
            }
            if kept.is_empty() {
                None
            } else {
                Some(make_split(dir, kept, kw))
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for DockNode {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let wire = DockNodeWire::deserialize(d)?;
        dock_from_wire(wire).ok_or_else(|| serde::de::Error::custom("split degenerado sin hijos"))
    }
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

/// Un paso en la ruta a un split anidado: por cuál hijo (índice) se baja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitStep(pub usize);

/// Un splitter arrastrable: la ruta al split, cuál divisor interno es (`divider`: 0..N-2),
/// el rect de su barra y su orientación.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitHandle {
    pub path: Vec<SplitStep>,
    pub divider: usize,
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

    /// Mueve el divisor `divider` (0..N-2) del split en `path`: `frac_local` ∈ [0.05, 0.95]
    /// es la proporción que toma el hijo `divider` DENTRO del par (`divider`, `divider+1`).
    /// Transfiere peso solo entre esos dos hijos (su suma se conserva) → el resto no se mueve.
    /// No-op si la ruta no apunta a un split o el índice de divisor está fuera de rango.
    pub fn set_divider(&mut self, path: &[SplitStep], divider: usize, frac_local: f32) {
        let Some(root) = self.root.as_mut() else {
            return;
        };
        let mut node = root;
        for SplitStep(i) in path {
            match node {
                DockNode::Split { children, .. } => {
                    let Some(child) = children.get_mut(*i) else {
                        return;
                    };
                    node = child;
                }
                // Una hoja o un grupo de pestañas no tiene sub-splits que recorrer.
                DockNode::Leaf(_) | DockNode::Tabs { .. } => return,
            }
        }
        if let DockNode::Split { weights, .. } = node {
            if divider + 1 >= weights.len() {
                return;
            }
            let f = frac_local.clamp(0.05, 0.95);
            let pair = weights[divider] + weights[divider + 1];
            weights[divider] = pair * f;
            weights[divider + 1] = pair * (1.0 - f);
        }
    }

    /// Dado el split en `path`, el divisor `divider` (entre hijos `divider` y `divider+1`), el
    /// `area` total y el puntero `(px,py)`, calcula (frac_local, bar_rect): la fracción CLAMP
    /// [0.05,0.95] que el corte pondría DENTRO del par de hijos adyacentes, y el rect de la
    /// barra-fantasma resultante. Se mide sobre el sub-rect del PAR. `None` si la ruta no es un
    /// split o el divisor está fuera de rango. Lo usan el preview del arrastre y el commit.
    pub fn divider_at(
        &self,
        path: &[SplitStep],
        divider: usize,
        area: Rect,
        px: f32,
        py: f32,
    ) -> Option<(f32, Rect)> {
        let root = self.root.as_ref()?;
        let mut node = root;
        let mut sub = area;
        for SplitStep(i) in path {
            if let DockNode::Split {
                dir,
                children,
                weights,
            } = node
            {
                let rects = child_rects(sub, *dir, weights);
                node = children.get(*i)?;
                sub = *rects.get(*i)?;
            } else {
                return None;
            }
        }
        let DockNode::Split {
            dir,
            children,
            weights,
        } = node
        else {
            return None;
        };
        if divider + 1 >= children.len() {
            return None;
        }
        let rects = child_rects(sub, *dir, weights);
        let a = rects[divider];
        let b = rects[divider + 1];
        let half = SPLIT_BAR / 2.0;
        let (f, bar) = match dir {
            SplitDir::Horizontal => {
                let pair_x = a.x;
                let pair_w = (b.x + b.w) - a.x;
                let f = if pair_w <= 0.0 {
                    0.5
                } else {
                    ((px - pair_x) / pair_w).clamp(0.05, 0.95)
                };
                let bx = pair_x + pair_w * f - half;
                (
                    f,
                    Rect {
                        x: bx,
                        y: a.y,
                        w: SPLIT_BAR,
                        h: a.h,
                    },
                )
            }
            SplitDir::Vertical => {
                let pair_y = a.y;
                let pair_h = (b.y + b.h) - a.y;
                let f = if pair_h <= 0.0 {
                    0.5
                } else {
                    ((py - pair_y) / pair_h).clamp(0.05, 0.95)
                };
                let by = pair_y + pair_h * f - half;
                (
                    f,
                    Rect {
                        x: a.x,
                        y: by,
                        w: a.w,
                        h: SPLIT_BAR,
                    },
                )
            }
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

/// Busca el split cuyo hijo `ia` contiene a `a` y cuyo hijo `ib` contiene a `b` (ia != ib) e
/// intercambia esos dos hijos (posición y peso). Si no hay tal split en este nivel, baja a los
/// hijos que sean splits.
fn swap_children_in(node: &mut DockNode, a: PaneId, b: PaneId) -> bool {
    if let DockNode::Split {
        children, weights, ..
    } = node
    {
        let ia = children.iter().position(|c| subtree_contains(c, a));
        let ib = children.iter().position(|c| subtree_contains(c, b));
        if let (Some(ia), Some(ib)) = (ia, ib) {
            if ia != ib {
                children.swap(ia, ib);
                weights.swap(ia, ib);
                return true;
            }
        }
        return children.iter_mut().any(|c| swap_children_in(c, a, b));
    }
    false
}

/// `true` si el subárbol `node` contiene la hoja/miembro `id`.
fn subtree_contains(node: &DockNode, id: PaneId) -> bool {
    match node {
        DockNode::Leaf(leaf) => *leaf == id,
        DockNode::Tabs { members, .. } => members.contains(&id),
        DockNode::Split { children, .. } => children.iter().any(|c| subtree_contains(c, id)),
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
            children,
            weights,
        } => {
            let rects = child_rects(area, *dir, weights);
            for (child, r) in children.iter().zip(rects) {
                place(child, r, out);
            }
        }
    }
}

/// Reparte `area` entre N hijos según sus pesos y la orientación, descontando la barra
/// `SPLIT_BAR` entre cada par de hijos. Devuelve un rect por hijo, en orden. Los pesos se
/// normalizan (Σ); un hijo nunca recibe menos de 1.0 px de lado útil (el render por software
/// castea f32→i32 y paniquea con geometrías degeneradas). Pura.
fn child_rects(area: Rect, dir: SplitDir, weights: &[f32]) -> Vec<Rect> {
    let n = weights.len();
    debug_assert!(n >= 2, "un split tiene ≥2 hijos");
    let sum: f32 = weights.iter().copied().filter(|w| *w > 0.0).sum();
    let sum = if sum > 0.0 { sum } else { n as f32 };
    let bars = SPLIT_BAR * (n as f32 - 1.0);
    let mut out = Vec::with_capacity(n);
    match dir {
        SplitDir::Horizontal => {
            let usable = (area.w - bars).max(n as f32); // ≥1px por hijo
            let mut x = area.x;
            for (i, w) in weights.iter().enumerate() {
                let cw = (usable * (w.max(0.0) / sum)).max(1.0);
                out.push(Rect {
                    x,
                    y: area.y,
                    w: cw,
                    h: area.h,
                });
                x += cw;
                if i + 1 < n {
                    x += SPLIT_BAR;
                }
            }
        }
        SplitDir::Vertical => {
            let usable = (area.h - bars).max(n as f32);
            let mut y = area.y;
            for (i, w) in weights.iter().enumerate() {
                let ch = (usable * (w.max(0.0) / sum)).max(1.0);
                out.push(Rect {
                    x: area.x,
                    y,
                    w: area.w,
                    h: ch,
                });
                y += ch;
                if i + 1 < n {
                    y += SPLIT_BAR;
                }
            }
        }
    }
    out
}

/// Recorre el árbol acumulando el handle (barra) de cada divisor de cada split. La barra
/// ocupa el hueco `SPLIT_BAR` entre dos hijos consecutivos.
fn handles(node: &DockNode, area: Rect, path: &mut Vec<SplitStep>, out: &mut Vec<SplitHandle>) {
    if let DockNode::Split {
        dir,
        children,
        weights,
    } = node
    {
        let rects = child_rects(area, *dir, weights);
        let divider_count = children.len().saturating_sub(1);
        for (i, a) in rects.iter().copied().enumerate().take(divider_count) {
            let bar = match dir {
                SplitDir::Horizontal => Rect {
                    x: a.x + a.w,
                    y: a.y,
                    w: SPLIT_BAR,
                    h: a.h,
                },
                SplitDir::Vertical => Rect {
                    x: a.x,
                    y: a.y + a.h,
                    w: a.w,
                    h: SPLIT_BAR,
                },
            };
            out.push(SplitHandle {
                path: path.clone(),
                divider: i,
                rect: bar,
                dir: *dir,
            });
        }
        for (i, child) in children.iter().enumerate() {
            path.push(SplitStep(i));
            handles(child, rects[i], path, out);
            path.pop();
        }
    }
}

/// Busca la hoja `leaf` y la divide con la orientación `dir`, aplanando cuando es posible.
///
/// - Si `leaf` es hijo directo de ESTE `Split` y el `dir` de ese split coincide con `dir`
///   pedido, inserta `Leaf(new_id)` como hermano adyacente (justo después de `leaf`) EN ESE
///   MISMO split: `[.., leaf, new, ..]`, partiendo en dos el peso que tenía `leaf` (así los
///   demás hijos conservan su tamaño exacto). No se crea un sub-split.
/// - Si el `dir` difiere, o `leaf` no es hijo directo de un split (está más anidado, o es la
///   raíz sola, o dentro de un grupo `Tabs`), se comporta como antes: la hoja (o el grupo
///   entero, si `leaf` es miembro de un `Tabs`) se reemplaza por un sub-split de 2 hijos con
///   pesos [1,1].
///
/// La detección "¿este split tiene a `leaf` como hijo directo y su dir coincide?" se hace en
/// CADA nivel de la recursión, así un leaf anidado dentro de un split interno del mismo dir
/// también se aplana ahí (y no en la raíz).
fn split_in(node: &mut DockNode, leaf: PaneId, dir: SplitDir, new_id: PaneId) {
    match node {
        DockNode::Leaf(id) if *id == leaf => {
            *node = DockNode::Split {
                dir,
                children: vec![DockNode::Leaf(leaf), DockNode::Leaf(new_id)],
                weights: vec![1.0, 1.0],
            };
        }
        DockNode::Leaf(_) => {}
        // Si `leaf` es uno de los miembros del grupo, el grupo entero se divide: el grupo
        // queda en el primer lado y el panel nuevo en el segundo. No aplanar tabs.
        DockNode::Tabs { members, .. } if members.contains(&leaf) => {
            let group = node.clone();
            *node = DockNode::Split {
                dir,
                children: vec![group, DockNode::Leaf(new_id)],
                weights: vec![1.0, 1.0],
            };
        }
        DockNode::Tabs { .. } => {}
        DockNode::Split {
            dir: split_dir,
            children,
            weights,
        } => {
            // ¿`leaf` es hijo directo de ESTE split, con el mismo dir pedido? Si sí, aplana
            // aquí mismo en vez de descender.
            if *split_dir == dir {
                if let Some(idx) = children
                    .iter()
                    .position(|c| matches!(c, DockNode::Leaf(id) if *id == leaf))
                {
                    let half = weights[idx] / 2.0;
                    weights[idx] = half;
                    children.insert(idx + 1, DockNode::Leaf(new_id));
                    weights.insert(idx + 1, half);
                    return;
                }
            }
            for c in children.iter_mut() {
                split_in(c, leaf, dir, new_id);
            }
        }
    }
}

/// Quita la hoja `id` del subárbol. Devuelve el subárbol resultante (o None si todo el
/// subárbol era esa hoja). Un split que pierde hijos hasta quedar con uno solo colapsa a ese
/// hijo; si quedan cero, desaparece.
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
            children,
            weights,
        } => {
            let mut kept: Vec<DockNode> = Vec::new();
            let mut kw: Vec<f32> = Vec::new();
            for (c, w) in children.into_iter().zip(weights) {
                if let Some(c) = remove_in(c, id) {
                    kept.push(c);
                    kw.push(w);
                }
            }
            match kept.len() {
                0 => None,
                1 => Some(kept.into_iter().next().unwrap()),
                _ => Some(DockNode::Split {
                    dir,
                    children: kept,
                    weights: kw,
                }),
            }
        }
    }
}

fn collect(node: &DockNode, out: &mut Vec<PaneId>) {
    match node {
        DockNode::Leaf(id) => out.push(*id),
        DockNode::Tabs { members, .. } => out.extend_from_slice(members),
        DockNode::Split { children, .. } => {
            for c in children {
                collect(c, out);
            }
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
        DockNode::Split { children, .. } => {
            for c in children.iter_mut() {
                stack_in(c, onto, new_id);
            }
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
        DockNode::Split { children, .. } => {
            for c in children.iter_mut() {
                activate_in(c, member);
            }
        }
    }
}

/// Acumula (miembros, activo) de cada grupo de pestañas del árbol.
fn collect_groups(node: &DockNode, out: &mut Vec<(Vec<PaneId>, usize)>) {
    match node {
        DockNode::Leaf(_) => {}
        DockNode::Tabs { members, active } => out.push((members.clone(), *active)),
        DockNode::Split { children, .. } => {
            for c in children {
                collect_groups(c, out);
            }
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
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Split {
                        dir: SplitDir::Horizontal,
                        children: vec![DockNode::Leaf(PaneId(2)), DockNode::Leaf(PaneId(3))],
                        weights: vec![1.0, 1.0],
                    },
                ],
                weights: vec![0.3, 0.7],
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
    fn child_rects_reparte_por_pesos_horizontal() {
        let rects = child_rects(
            Rect {
                x: 0.0,
                y: 0.0,
                w: 800.0,
                h: 600.0,
            },
            SplitDir::Horizontal,
            &[1.0, 2.0, 1.0],
        );
        assert_eq!(rects.len(), 3);
        assert!((rects[0].w - 198.0).abs() < 1.0, "1º ~198: {}", rects[0].w);
        assert!((rects[1].w - 396.0).abs() < 1.0, "2º ~396: {}", rects[1].w);
        assert!((rects[2].w - 198.0).abs() < 1.0, "3º ~198: {}", rects[2].w);
        assert!(rects[1].x > rects[0].x + rects[0].w - 0.1);
        assert!(rects[2].x > rects[1].x + rects[1].w - 0.1);
        assert!(rects.iter().all(|r| (r.h - 600.0).abs() < 0.01));
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
                children: vec![DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))],
                weights: vec![0.25, 0.75],
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
                children: vec![DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))],
                weights: vec![1.0, 1.0],
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
    fn split_handles_y_set_divider() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))],
                weights: vec![1.0, 1.0],
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
        l.set_divider(&path, 0, 0.25);
        let r1 = l
            .pane_rects(area)
            .iter()
            .find(|(id, _)| *id == PaneId(1))
            .unwrap()
            .1;
        assert!((r1.w - 198.0).abs() < 2.0, "ahora el 1º ocupa ~25%");
    }

    #[test]
    fn split_handles_uno_por_divisor() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Leaf(PaneId(2)),
                    DockNode::Leaf(PaneId(3)),
                ],
                weights: vec![1.0, 1.0, 1.0],
            }),
        };
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 900.0,
            h: 600.0,
        };
        let hs = l.split_handles(area);
        assert_eq!(hs.len(), 2, "3 hijos → 2 divisores");
        assert_eq!(hs[0].divider, 0);
        assert_eq!(hs[1].divider, 1);
        assert!(hs[0].rect.x < hs[1].rect.x);
    }

    #[test]
    fn divider_at_mide_local_al_par() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Leaf(PaneId(2)),
                    DockNode::Leaf(PaneId(3)),
                ],
                weights: vec![1.0, 1.0, 1.0],
            }),
        };
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 900.0,
            h: 600.0,
        };
        let (f, bar) = l.divider_at(&[], 1, area, 600.0, 300.0).unwrap();
        assert!((f - 0.5).abs() < 0.05, "frac local ~0.5: {f}");
        assert!(
            (bar.x - 598.0).abs() < 6.0,
            "barra cerca de x~600: {}",
            bar.x
        );
        let (fmin, _) = l.divider_at(&[], 1, area, 0.0, 300.0).unwrap();
        assert!((fmin - 0.05).abs() < 0.001);
        assert!(l.divider_at(&[], 9, area, 100.0, 100.0).is_none());
    }

    #[test]
    fn set_divider_solo_afecta_a_los_dos_vecinos() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Leaf(PaneId(2)),
                    DockNode::Leaf(PaneId(3)),
                ],
                weights: vec![1.0, 1.0, 1.0],
            }),
        };
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 900.0,
            h: 600.0,
        };
        let w2_antes = l
            .pane_rects(area)
            .iter()
            .find(|(id, _)| *id == PaneId(3))
            .unwrap()
            .1
            .w;
        l.set_divider(&[], 0, 0.1);
        let rects = l.pane_rects(area);
        let w0 = rects.iter().find(|(id, _)| *id == PaneId(1)).unwrap().1.w;
        let w1 = rects.iter().find(|(id, _)| *id == PaneId(2)).unwrap().1.w;
        let w2 = rects.iter().find(|(id, _)| *id == PaneId(3)).unwrap().1.w;
        assert!(w0 < w1);
        assert!(
            (w2 - w2_antes).abs() < 1.0,
            "el hijo 2 (no vecino) no cambia: {w2} vs {w2_antes}"
        );
    }

    #[test]
    fn set_divider_clampa() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))],
                weights: vec![1.0, 1.0],
            }),
        };
        let pair_antes = 2.0; // 1.0 + 1.0
        l.set_divider(&[], 0, 2.0);
        if let Some(DockNode::Split { weights, .. }) = &l.root {
            assert_eq!(weights.len(), 2);
            assert!(weights[0] > 0.0 && weights[1] > 0.0, "ninguno domina 100%");
            assert!(
                (weights[0] + weights[1] - pair_antes).abs() < 0.001,
                "el par sigue sumando lo mismo: {:?}",
                weights
            );
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
            dir,
            children,
            weights,
        }) = &l.root
        {
            assert_eq!(*dir, SplitDir::Horizontal);
            assert_eq!(
                children.as_slice(),
                [DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))]
            );
            assert_eq!(weights.len(), 2);
        } else {
            panic!("raíz debe ser split");
        }
    }

    #[test]
    fn split_leaf_aplana_en_fila_del_mismo_dir() {
        // Split[Horizontal, Leaf(1), Leaf(2)] con pesos iguales; dividir Leaf(2) en el MISMO
        // eje (Horizontal) debe insertar Leaf(3) como hermano plano, no anidar un sub-split.
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))],
                weights: vec![1.0, 1.0],
            }),
        };
        l.split_leaf(PaneId(2), SplitDir::Horizontal, PaneId(3));

        let (dir, children, weights) = match &l.root {
            Some(DockNode::Split {
                dir,
                children,
                weights,
            }) => (*dir, children, weights),
            other => panic!("raíz debe seguir siendo un split plano: {other:?}"),
        };
        assert_eq!(dir, SplitDir::Horizontal);
        assert_eq!(
            children.as_slice(),
            [
                DockNode::Leaf(PaneId(1)),
                DockNode::Leaf(PaneId(2)),
                DockNode::Leaf(PaneId(3)),
            ],
            "3 hijos PLANOS, sin sub-split anidado"
        );
        assert_eq!(weights.len(), 3);
        // Leaf(1) conserva su peso original (1.0); Leaf(2) y Leaf(3) se reparten el 1.0 que
        // tenía Leaf(2) (0.5 cada uno).
        assert!(
            (weights[0] - 1.0).abs() < 0.001,
            "peso de 1 intacto: {weights:?}"
        );
        assert!(
            (weights[1] - 0.5).abs() < 0.001,
            "peso de 2 a la mitad: {weights:?}"
        );
        assert!(
            (weights[2] - 0.5).abs() < 0.001,
            "peso de 3 la otra mitad: {weights:?}"
        );

        // pane_rects sobre un área conocida: el panel 1 conserva su ancho anterior (a nivel de
        // PESOS es EXACTO — ver los `assert` de `weights` arriba — pero en PÍXELES hay un
        // corrimiento de hasta un `SPLIT_BAR` porque `child_rects` descuenta `SPLIT_BAR*(N-1)`
        // del área usable y N pasó de 2 a 3 hijos: ese es el costo fijo de la barra nueva entre
        // 2 y 3, no un efecto "elástico" sobre 1) y 2+3 juntos ocupan lo que ocupaba 2.
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        let antes = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))],
                weights: vec![1.0, 1.0],
            }),
        }
        .pane_rects(area);
        let w1_antes = antes.iter().find(|(id, _)| *id == PaneId(1)).unwrap().1.w;
        let w2_antes = antes.iter().find(|(id, _)| *id == PaneId(2)).unwrap().1.w;

        let despues = l.pane_rects(area);
        let w1 = despues.iter().find(|(id, _)| *id == PaneId(1)).unwrap().1.w;
        let w2 = despues.iter().find(|(id, _)| *id == PaneId(2)).unwrap().1.w;
        let w3 = despues.iter().find(|(id, _)| *id == PaneId(3)).unwrap().1.w;

        assert!(
            (w1 - w1_antes).abs() < SPLIT_BAR + 0.5,
            "panel 1 conserva su ancho anterior (± el costo fijo de la barra nueva): {w1} vs {w1_antes}"
        );
        // 2 y 3 juntos (más la nueva barra entre ambos, que antes no existía) ocupan lo que
        // ocupaba 2: comparamos contra w2_antes con una tolerancia que cubre la barra extra.
        assert!(
            (w2 + w3 - w2_antes).abs() < SPLIT_BAR + 1.0,
            "2+3 juntos ocupan ~lo que ocupaba 2: {w2}+{w3} vs {w2_antes}"
        );
    }

    #[test]
    fn split_leaf_perpendicular_anida() {
        // Mismo layout base, pero dividiendo en el eje PERPENDICULAR (Vertical): debe anidar
        // un sub-split, no aplanar.
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))],
                weights: vec![1.0, 1.0],
            }),
        };
        l.split_leaf(PaneId(2), SplitDir::Vertical, PaneId(3));

        match &l.root {
            Some(DockNode::Split {
                dir,
                children,
                weights,
            }) => {
                assert_eq!(
                    *dir,
                    SplitDir::Horizontal,
                    "el split externo no cambia de eje"
                );
                assert_eq!(children.len(), 2, "el split externo sigue con 2 hijos");
                assert_eq!(children[0], DockNode::Leaf(PaneId(1)));
                assert!(
                    (weights[0] - 1.0).abs() < 0.001,
                    "peso externo de 1 intacto"
                );
                match &children[1] {
                    DockNode::Split {
                        dir: inner_dir,
                        children: inner_children,
                        weights: inner_weights,
                    } => {
                        assert_eq!(*inner_dir, SplitDir::Vertical);
                        assert_eq!(
                            inner_children.as_slice(),
                            [DockNode::Leaf(PaneId(2)), DockNode::Leaf(PaneId(3))]
                        );
                        assert_eq!(inner_weights.len(), 2);
                    }
                    other => {
                        panic!("Leaf(2) debe reemplazarse por un sub-split anidado: {other:?}")
                    }
                }
            }
            other => panic!("raíz debe seguir siendo split: {other:?}"),
        }
    }

    #[test]
    fn split_leaf_root_unico_crea_split() {
        // Caso 1 explícito: root = Leaf único → split de 2 con pesos [1,1].
        let mut l = SerializableDockLayout::single(PaneId(1));
        l.split_leaf(PaneId(1), SplitDir::Horizontal, PaneId(2));
        assert_eq!(
            l.root,
            Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))],
                weights: vec![1.0, 1.0],
            })
        );
    }

    #[test]
    fn split_leaf_en_split_anidado_aplana_ahi() {
        // Split{ Vertical, [Leaf(1), Split{ Horizontal, [Leaf(2), Leaf(3)] }] }: dividir
        // Leaf(3) en Horizontal debe aplanar el split INTERNO (mismo dir), no la raíz.
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Vertical,
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Split {
                        dir: SplitDir::Horizontal,
                        children: vec![DockNode::Leaf(PaneId(2)), DockNode::Leaf(PaneId(3))],
                        weights: vec![1.0, 1.0],
                    },
                ],
                weights: vec![0.4, 0.6],
            }),
        };
        l.split_leaf(PaneId(3), SplitDir::Horizontal, PaneId(4));

        match &l.root {
            Some(DockNode::Split {
                dir,
                children,
                weights,
            }) => {
                assert_eq!(*dir, SplitDir::Vertical, "el split externo no cambia");
                assert_eq!(children.len(), 2, "el split externo sigue con 2 hijos");
                assert_eq!(children[0], DockNode::Leaf(PaneId(1)));
                assert!(
                    (weights[0] - 0.4).abs() < 0.001 && (weights[1] - 0.6).abs() < 0.001,
                    "pesos del split externo NO cambian: {weights:?}"
                );
                match &children[1] {
                    DockNode::Split {
                        dir: inner_dir,
                        children: inner_children,
                        weights: inner_weights,
                    } => {
                        assert_eq!(*inner_dir, SplitDir::Horizontal);
                        assert_eq!(
                            inner_children.as_slice(),
                            [
                                DockNode::Leaf(PaneId(2)),
                                DockNode::Leaf(PaneId(3)),
                                DockNode::Leaf(PaneId(4)),
                            ],
                            "el split interno horizontal pasa a 3 hijos PLANOS"
                        );
                        assert_eq!(inner_weights.len(), 3);
                        assert!((inner_weights[0] - 1.0).abs() < 0.001, "peso de 2 intacto");
                        assert!(
                            (inner_weights[1] - 0.5).abs() < 0.001,
                            "peso de 3 a la mitad"
                        );
                        assert!(
                            (inner_weights[2] - 0.5).abs() < 0.001,
                            "peso de 4 la otra mitad"
                        );
                    }
                    other => panic!("el hijo 1 debe seguir siendo el split interno: {other:?}"),
                }
            }
            other => panic!("raíz debe seguir siendo split: {other:?}"),
        }
    }

    #[test]
    fn remove_leaf_colapsa_el_split() {
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![DockNode::Leaf(PaneId(1)), DockNode::Leaf(PaneId(2))],
                weights: vec![1.0, 1.0],
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
        if let Some(DockNode::Split { children, .. }) = &l.root {
            assert_eq!(children.len(), 2);
            assert_eq!(
                children[0],
                DockNode::Tabs {
                    members: vec![PaneId(1), PaneId(2)],
                    active: 0,
                }
            );
            assert_eq!(children[1], DockNode::Leaf(PaneId(9)));
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

    #[test]
    fn round_trip_serde_con_split_nario() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Leaf(PaneId(2)),
                    DockNode::Leaf(PaneId(3)),
                ],
                weights: vec![1.0, 2.0, 1.0],
            }),
        };
        let json = serde_json::to_string(&l).unwrap();
        let back: SerializableDockLayout = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }

    #[test]
    fn migra_split_binario_viejo_a_pesos() {
        let viejo = r#"{"root":{"Split":{"dir":"Horizontal","fraction":0.25,"first":{"Leaf":1},"second":{"Leaf":2}}}}"#;
        let l: SerializableDockLayout = serde_json::from_str(viejo).unwrap();
        if let Some(DockNode::Split {
            children, weights, ..
        }) = &l.root
        {
            assert_eq!(children.len(), 2);
            assert!((weights[0] - 0.25).abs() < 0.001);
            assert!((weights[1] - 0.75).abs() < 0.001);
        } else {
            panic!("debe migrar a Split N-ario");
        }
    }

    // --- Blindaje contra splits degenerados (I-1/I-2) ---------------------------------

    #[test]
    fn deser_split_vacio_es_error_o_none() {
        // Un Split sin `children` ni `first`/`second`: no puede representar ningún panel.
        // Debe fallar la deserialización (no producir un Split degenerado ni paniquear).
        let corrupto = r#"{"root":{"Split":{"dir":"Horizontal"}}}"#;
        let r = serde_json::from_str::<SerializableDockLayout>(corrupto);
        assert!(r.is_err(), "un split sin hijos debe rechazarse: {r:?}");
    }

    #[test]
    fn deser_split_un_hijo_colapsa() {
        let json =
            r#"{"root":{"Split":{"dir":"Horizontal","children":[{"Leaf":1}],"weights":[1.0]}}}"#;
        let l: SerializableDockLayout = serde_json::from_str(json).unwrap();
        assert_eq!(l.root, Some(DockNode::Leaf(PaneId(1))));
    }

    #[test]
    fn migra_binario_solo_first_conserva_el_lado() {
        // `second` ausente (p. ej. archivo truncado): el lado presente no debe perderse.
        let json = r#"{"root":{"Split":{"dir":"Horizontal","fraction":0.3,"first":{"Leaf":7}}}}"#;
        let l: SerializableDockLayout = serde_json::from_str(json).unwrap();
        assert_eq!(l.root, Some(DockNode::Leaf(PaneId(7))));
    }

    #[test]
    fn migra_binario_anidado() {
        // Split viejo cuyo `second` es a su vez un split viejo: debe migrar a N-ario anidado.
        let json = r#"{"root":{"Split":{"dir":"Horizontal","fraction":0.5,"first":{"Leaf":1},
            "second":{"Split":{"dir":"Vertical","fraction":0.5,"first":{"Leaf":2},"second":{"Leaf":3}}}}}}"#;
        let l: SerializableDockLayout = serde_json::from_str(json).unwrap();
        assert_eq!(l.pane_ids(), vec![PaneId(1), PaneId(2), PaneId(3)]);
        match &l.root {
            Some(DockNode::Split {
                dir,
                children,
                weights,
            }) => {
                assert_eq!(*dir, SplitDir::Horizontal);
                assert_eq!(children.len(), 2);
                assert!((weights[0] - 0.5).abs() < 0.001);
                assert!((weights[1] - 0.5).abs() < 0.001);
                assert_eq!(children[0], DockNode::Leaf(PaneId(1)));
                match &children[1] {
                    DockNode::Split {
                        dir,
                        children,
                        weights,
                    } => {
                        assert_eq!(*dir, SplitDir::Vertical);
                        assert_eq!(
                            children.as_slice(),
                            [DockNode::Leaf(PaneId(2)), DockNode::Leaf(PaneId(3))]
                        );
                        assert!((weights[0] - 0.5).abs() < 0.001);
                        assert!((weights[1] - 0.5).abs() < 0.001);
                    }
                    other => panic!("el segundo hijo debe ser un split anidado: {other:?}"),
                }
            }
            other => panic!("la raíz debe ser un split: {other:?}"),
        }
    }

    #[test]
    fn deser_weights_largo_distinto_se_repara() {
        // 1 peso para 2 hijos: debe deserializar OK con pesos uniformes en vez de paniquear.
        let json = r#"{"root":{"Split":{"dir":"Horizontal","children":[{"Leaf":1},{"Leaf":2}],"weights":[5.0]}}}"#;
        let l: SerializableDockLayout = serde_json::from_str(json).unwrap();
        if let Some(DockNode::Split {
            children, weights, ..
        }) = &l.root
        {
            assert_eq!(children.len(), 2);
            assert_eq!(weights.len(), 2);
            assert!((weights[0] - weights[1]).abs() < 0.001, "pesos uniformes");
        } else {
            panic!("la raíz debe ser un split");
        }
        let rects = l.pane_rects(Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        });
        let r1 = rects.iter().find(|(id, _)| *id == PaneId(1)).unwrap().1;
        let r2 = rects.iter().find(|(id, _)| *id == PaneId(2)).unwrap().1;
        assert!(
            (r1.w - r2.w).abs() < 2.0,
            "se reparte ~50/50: {} vs {}",
            r1.w,
            r2.w
        );
    }
}
