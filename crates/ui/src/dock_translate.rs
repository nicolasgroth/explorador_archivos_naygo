// Naygo — traducción entre el layout puro de core y el DockState de egui_dock.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `core` describe la disposición con `SerializableDockLayout` (sin egui_dock).
//! Aquí la traducimos a un `DockState<PaneId>` de egui_dock para pintarla. La
//! estrategia: arrancar el dock con la primera hoja y aplicar los splits del árbol
//! en pre-orden. `core` permanece desacoplado; este puente vive en la UI.

use egui_dock::{DockState, NodeIndex, Tree};
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
        DockNode::Split { first, .. } => first_leaf_id(first),
    }
}

/// Recolecta los `PaneId` presentes en el dock (para verificación/persistencia).
// Por ahora solo lo usan los tests; lo consumirá la persistencia del reacomodo
// manual de tabs (leer el DockState de vuelta) en un pulido posterior.
#[allow(dead_code)]
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
}
