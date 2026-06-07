// Naygo — panel de árbol de directorios: render recursivo de un DirTree.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta un `DirTree` de forma recursiva: las raíces son las unidades de disco y
//! cada nodo es SOLO carpetas (nunca archivos). El triángulo (▶/▼) expande o
//! colapsa; clicar el nombre navega el panel `Files` activo. El render NO hace
//! I/O ni muta el árbol: acumula `TreeAction`s que `NaygoApp` procesa después.
//!
//! La carpeta activa se resalta (fondo gris suave + barra azul a la izquierda) y,
//! si el árbol pide `reveal_to`, se hace scroll hasta ese nodo.

use crate::icons::IconProvider;
use crate::tree_actions::TreeAction;
use naygo_core::icon_kind::IconKey;
use naygo_core::tree::{DirTree, NodeState, TreeNode};

/// Lado del ícono de carpeta/unidad, en px.
const ICON_SIZE: f32 = 16.0;
/// Sangría por nivel de profundidad, en px.
const INDENT: f32 = 14.0;

/// Devuelve `true` si el nodo objetivo de `reveal_to` se encontró y se pintó (y se
/// llamó a `scroll_to_me`) en este frame. `NaygoApp` usa esto para limpiar
/// `reveal_to` SOLO cuando el objetivo ya se reveló, en vez de cada frame (lo que
/// perdía el scroll a carpetas profundas cargadas en cascada).
pub fn show(
    ui: &mut egui::Ui,
    tree: &DirTree,
    actions: &mut Vec<TreeAction>,
    icons: &IconProvider,
    i18n: &naygo_core::i18n::I18n,
) -> bool {
    if tree.is_empty() {
        ui.label(i18n.t("tree.empty"));
        return false;
    }
    egui::ScrollArea::both()
        .show(ui, |ui| {
            let mut revealed = false;
            for root in &tree.roots {
                revealed |= show_node(ui, root, 0, tree, actions, icons, i18n);
            }
            revealed
        })
        .inner
}

/// Pinta un nodo (y, si está expandido, sus hijos) recursivamente. Devuelve `true`
/// si este nodo o alguno de sus descendientes era el objetivo de `reveal_to` y se
/// hizo scroll hacia él en este frame.
#[allow(clippy::too_many_arguments)]
fn show_node(
    ui: &mut egui::Ui,
    node: &TreeNode,
    depth: usize,
    tree: &DirTree,
    actions: &mut Vec<TreeAction>,
    icons: &IconProvider,
    i18n: &naygo_core::i18n::I18n,
) -> bool {
    let is_active = tree.active_path.as_deref() == Some(node.path.as_path());

    // El contenido de la fila se pinta dentro de este closure; lo envolvemos en un
    // `Frame` cuando el nodo es el activo, para que el fondo gris quede DETRÁS del
    // texto (pintar con rect_filled después taparía el contenido).
    let mut row_content = |ui: &mut egui::Ui| {
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * INDENT);

            // Triángulo expandir/colapsar. Solo si el nodo PUEDE tener hijos: los
            // estados Empty/Error no muestran toggle (no hay nada que abrir).
            if !matches!(node.state, NodeState::Empty | NodeState::Error) {
                let glyph = if node.expanded { "▼" } else { "▶" };
                let tri = ui.add(egui::Label::new(glyph).sense(egui::Sense::click()));
                if tri.clicked() {
                    if node.expanded {
                        actions.push(TreeAction::Collapse(node.path.clone()));
                    } else {
                        actions.push(TreeAction::Expand(node.path.clone()));
                    }
                }
            } else {
                ui.add_space(INDENT);
            }

            // Ícono: unidad si el nodo es una raíz, carpeta en otro caso.
            let key = match node.drive_kind {
                Some(kind) => IconKey::Drive(kind),
                None => IconKey::Folder,
            };
            let tex = icons.texture(key);
            // Ícono + nombre como UNA zona clicable: clic en cualquiera de los dos
            // (no en el triángulo) navega el panel activo a esta carpeta.
            let img = ui.add(
                egui::Image::new(tex)
                    .fit_to_exact_size(egui::vec2(ICON_SIZE, ICON_SIZE))
                    .sense(egui::Sense::click()),
            );
            let label = ui.selectable_label(is_active, &node.name);
            if img.union(label).clicked() {
                actions.push(TreeAction::Navigate(node.path.clone()));
            }

            // Spinner mientras lista sus hijos.
            if node.state == NodeState::Loading {
                ui.spinner();
            }
        })
        .response
    };

    let row = if is_active {
        // Fondo gris suave detrás de toda la fila activa.
        let inner = egui::Frame::NONE
            .fill(egui::Color32::from_rgb(0x37, 0x37, 0x3d))
            .show(ui, row_content);
        // Barra azul vertical de 3px en el borde izquierdo de la fila.
        let rect = inner.response.rect;
        let bar = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + 3.0, rect.bottom()),
        );
        ui.painter()
            .rect_filled(bar, 0.0, egui::Color32::from_rgb(0x3b, 0x82, 0xf6));
        inner.response
    } else {
        row_content(ui)
    };

    // Reveal: si el árbol pide revelar esta carpeta, hacer scroll hasta ella.
    let mut revealed = false;
    if tree.reveal_to.as_deref() == Some(node.path.as_path()) {
        row.scroll_to_me(Some(egui::Align::Center));
        revealed = true;
    }

    // Hijos: solo si el nodo está expandido.
    if node.expanded {
        let child_depth = depth + 1;
        // Sub-fila de estado (cargando / vacío / error) indentada un nivel más.
        match node.state {
            NodeState::Loading => {
                status_row(ui, child_depth, |ui| {
                    ui.weak(i18n.t("tree.loading"));
                });
            }
            NodeState::Empty => {
                status_row(ui, child_depth, |ui| {
                    ui.weak(i18n.t("tree.empty"));
                });
            }
            NodeState::Error => {
                status_row(ui, child_depth, |ui| {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xe0, 0x4b, 0x4b),
                        format!("⚠ {}", i18n.t("tree.access_denied")),
                    );
                });
            }
            _ => {}
        }
        if let Some(children) = &node.children {
            for child in children {
                revealed |= show_node(ui, child, child_depth, tree, actions, icons, i18n);
            }
        }
    }

    revealed
}

/// Pinta una sub-fila de estado (texto débil/coloreado) indentada a `depth`.
fn status_row(ui: &mut egui::Ui, depth: usize, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * INDENT);
        add(ui);
    });
}
