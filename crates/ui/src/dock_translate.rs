// Naygo — traducción entre el layout puro de core y el DockState de egui_dock.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `core` describe la disposición con `SerializableDockLayout` (sin egui_dock).
//! Aquí la traducimos a un `DockState<PaneId>` de egui_dock para pintarla. La
//! estrategia: arrancar el dock con la primera hoja y aplicar los splits del árbol
//! en pre-orden. `core` permanece desacoplado; este puente vive en la UI.

use egui_dock::{DockState, Node, NodeIndex, Tree};
use naygo_core::workspace::layout::{DockNode, SerializableDockLayout, SplitDir};
use naygo_core::workspace::PaneId;

/// Construye un `DockState<PaneId>` a partir del layout puro.
/// Precondición de 2A: el layout tiene al menos un panel (nunca vacío en runtime).
pub fn to_dock_state(layout: &SerializableDockLayout) -> DockState<PaneId> {
    let ids = layout.pane_ids();
    let first = ids.first().copied().unwrap_or(PaneId(0));
    let mut state = DockState::new(vec![first]);

    if let Some(root) = &layout.root {
        let tree = state.main_surface_mut();
        build_into(tree, NodeIndex::root(), root);
    }
    state
}

/// Inserta recursivamente el árbol `node` en el nodo `at`. El nodo `at` ya
/// contiene la primera hoja del subárbol `node` (puesta por el split previo o por
/// el DockState inicial), así que un `Leaf` no hace nada; un `Split` divide.
fn build_into(tree: &mut Tree<PaneId>, at: NodeIndex, node: &DockNode) {
    match node {
        DockNode::Leaf(_id) => {
            // El nodo `at` ya tiene esta hoja; nada que hacer.
        }
        DockNode::Tabs { .. } => {
            // Los grupos de pestañas son una feature de la capa Slint. En la capa egui
            // (que se retira en F6) se degrada al miembro activo: el nodo `at` ya quedó
            // sembrado con ese id por `first_leaf_id`, así que no hay nada más que insertar.
        }
        DockNode::Split {
            dir,
            fraction,
            first,
            second,
        } => {
            // El lado nuevo arranca con UNA sola hoja (la primera del subárbol
            // `second`), igual que el DockState inicial arranca con una. Si
            // pusiéramos todas las hojas de `second` aquí, la recursión posterior
            // las volvería a insertar y quedarían duplicadas.
            let second_first = first_leaf_id(second);
            let [a, b] = match dir {
                SplitDir::Horizontal => tree.split_right(at, *fraction, vec![second_first]),
                SplitDir::Vertical => tree.split_below(at, *fraction, vec![second_first]),
            };
            // `a` (existente) aloja el subárbol `first`; `b` (nuevo) el `second`.
            build_into(tree, a, first);
            build_into(tree, b, second);
        }
    }
}

/// La primera hoja de un subárbol (la que más a la izquierda/arriba queda).
/// Es la hoja con la que se siembra el lado nuevo de un split antes de recursar.
fn first_leaf_id(node: &DockNode) -> PaneId {
    match node {
        DockNode::Leaf(id) => *id,
        // Un grupo se degrada a su pestaña activa en la capa egui (ver build_into).
        DockNode::Tabs { members, active } => {
            members.get(*active).copied().unwrap_or(members[0])
        }
        DockNode::Split { first, .. } => first_leaf_id(first),
    }
}

/// Lee el `DockState` vivo de egui_dock de vuelta a un layout puro persistible.
/// Inverso de `to_dock_state`. Recorre el árbol de la superficie principal: un nodo
/// hoja → `DockNode::Leaf` (su primera/única tab); un split → `DockNode::Split` con la
/// dirección, la fracción y los subárboles. Tolerante: nodos vacíos/raros se omiten.
///
/// Captura los cambios hechos sobre el dock en vivo (paneles añadidos con ➕ y
/// reacomodos por arrastre), para que sobrevivan al reinicio. La correspondencia
/// espeja `to_dock_state`:
/// - `Node::Horizontal` (alta por `split_right`) ↔ `SplitDir::Horizontal`.
/// - `Node::Vertical`   (alta por `split_below`) ↔ `SplitDir::Vertical`.
/// - el hijo izquierdo (`idx.left()`) aloja el subárbol `first`; el derecho
///   (`idx.right()`) el `second` (mismo orden que devuelve `split_*`).
pub fn from_dock_state(state: &DockState<PaneId>) -> SerializableDockLayout {
    let tree = state.main_surface();
    let root = node_to_dock(tree, NodeIndex::root());
    SerializableDockLayout { root }
}

/// Traduce el nodo `idx` del árbol de egui_dock a un `DockNode` puro (recursivo).
/// Devuelve `None` para nodos vacíos, fuera de rango, hojas sin tabs, o splits cuyos
/// dos hijos resultaron vacíos (degeneración: se descarta el split).
fn node_to_dock(tree: &Tree<PaneId>, idx: NodeIndex) -> Option<DockNode> {
    // Fuera del Vec de nodos (el árbol es disperso: índices binarios 2i+1/2i+2).
    if idx.0 >= tree.len() {
        return None;
    }
    match &tree[idx] {
        Node::Empty => None,
        Node::Leaf(_) => tree[idx].tabs()?.first().copied().map(DockNode::Leaf),
        Node::Horizontal(split) => split_to_dock(tree, idx, SplitDir::Horizontal, split.fraction),
        Node::Vertical(split) => split_to_dock(tree, idx, SplitDir::Vertical, split.fraction),
    }
}

/// Arma un `DockNode::Split` a partir de un nodo split de egui_dock. Si solo uno de
/// los dos hijos sobrevive (el otro quedó vacío), colapsa al hijo presente en vez de
/// fabricar un split degenerado.
fn split_to_dock(
    tree: &Tree<PaneId>,
    idx: NodeIndex,
    dir: SplitDir,
    fraction: f32,
) -> Option<DockNode> {
    let first = node_to_dock(tree, idx.left());
    let second = node_to_dock(tree, idx.right());
    match (first, second) {
        (Some(first), Some(second)) => Some(DockNode::Split {
            dir,
            fraction,
            first: Box::new(first),
            second: Box::new(second),
        }),
        // Split degenerado (un solo hijo real): colapsar a ese hijo.
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

/// Recolecta los `PaneId` presentes en el dock (para verificación/persistencia).
pub fn dock_pane_ids(state: &DockState<PaneId>) -> Vec<PaneId> {
    state.iter_all_tabs().map(|(_, id)| *id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::workspace::layout::SerializableDockLayout;

    #[test]
    fn single_pane_produce_un_dock_con_ese_tab() {
        let layout = SerializableDockLayout::single(PaneId(5));
        let state = to_dock_state(&layout);
        assert_eq!(dock_pane_ids(&state), vec![PaneId(5)]);
    }

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
        assert_eq!(
            got, want,
            "el dock contiene exactamente los paneles del layout"
        );
    }

    /// Compara dos layouts permitiendo deriva de coma flotante en las fracciones.
    fn dock_nodes_equivalentes(a: &DockNode, b: &DockNode) -> bool {
        match (a, b) {
            (DockNode::Leaf(x), DockNode::Leaf(y)) => x == y,
            (
                DockNode::Split {
                    dir: da,
                    fraction: fa,
                    first: f1,
                    second: s1,
                },
                DockNode::Split {
                    dir: db,
                    fraction: fb,
                    first: f2,
                    second: s2,
                },
            ) => {
                da == db
                    && (fa - fb).abs() < 1e-4
                    && dock_nodes_equivalentes(f1, f2)
                    && dock_nodes_equivalentes(s1, s2)
            }
            _ => false,
        }
    }

    /// Recorre to_dock_state → from_dock_state y verifica equivalencia estructural.
    fn assert_round_trips(layout: &SerializableDockLayout) {
        let state = to_dock_state(layout);
        let back = from_dock_state(&state);
        match (&layout.root, &back.root) {
            (Some(a), Some(b)) => assert!(
                dock_nodes_equivalentes(a, b),
                "round-trip difiere:\norig = {a:?}\nback = {b:?}"
            ),
            (None, None) => {}
            other => panic!("round-trip cambió la presencia de raíz: {other:?}"),
        }
    }

    #[test]
    fn single_leaf_round_trips() {
        assert_round_trips(&SerializableDockLayout::single(PaneId(9)));
    }

    #[test]
    fn horizontal_split_de_dos_hojas_round_trips() {
        let layout = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.4,
                first: Box::new(DockNode::Leaf(PaneId(1))),
                second: Box::new(DockNode::Leaf(PaneId(2))),
            }),
        };
        assert_round_trips(&layout);
    }

    #[test]
    fn vertical_split_de_dos_hojas_round_trips() {
        let layout = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Vertical,
                fraction: 0.65,
                first: Box::new(DockNode::Leaf(PaneId(3))),
                second: Box::new(DockNode::Leaf(PaneId(4))),
            }),
        };
        assert_round_trips(&layout);
    }

    #[test]
    fn nested_split_round_trips() {
        let layout = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                fraction: 0.3,
                first: Box::new(DockNode::Leaf(PaneId(1))),
                second: Box::new(DockNode::Split {
                    dir: SplitDir::Vertical,
                    fraction: 0.5,
                    first: Box::new(DockNode::Leaf(PaneId(2))),
                    second: Box::new(DockNode::Leaf(PaneId(3))),
                }),
            }),
        };
        assert_round_trips(&layout);
    }
}
