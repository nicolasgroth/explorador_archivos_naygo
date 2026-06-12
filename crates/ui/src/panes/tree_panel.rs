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
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    tree: &DirTree,
    actions: &mut Vec<TreeAction>,
    icons: &IconProvider,
    i18n: &naygo_core::i18n::I18n,
    theme: &crate::theme_apply::ActiveTheme,
    disk_usage: &std::collections::HashMap<std::path::PathBuf, naygo_core::disk::DiskUsage>,
    favorites: &[(String, std::path::PathBuf)],
) -> bool {
    if tree.is_empty() {
        ui.label(i18n.t("tree.empty"));
        return false;
    }
    egui::ScrollArea::both()
        .show(ui, |ui| {
            // Favoritos anclados ARRIBA de las unidades: nodos hoja simples (sin
            // triángulo ni hijos), clic = navegar como cualquier nodo del árbol.
            // Si no hay favoritos, no se pinta nada extra (ni el separador).
            if !favorites.is_empty() {
                for (label, path) in favorites {
                    show_favorite_row(ui, label, path, tree, actions, icons);
                }
                ui.separator();
            }
            let mut revealed = false;
            for root in &tree.roots {
                revealed |= show_node(ui, root, 0, tree, actions, icons, i18n, theme, disk_usage);
            }
            revealed
        })
        .inner
}

/// Pinta un favorito anclado: ícono de carpeta + nombre, resaltado si es la
/// carpeta activa. El clic en ícono o nombre emite `TreeAction::Navigate` (mismo
/// camino diferido que el resto del árbol; no muta nada al pintar).
fn show_favorite_row(
    ui: &mut egui::Ui,
    label: &str,
    path: &std::path::Path,
    tree: &DirTree,
    actions: &mut Vec<TreeAction>,
    icons: &IconProvider,
) {
    let is_active = tree.active_path.as_deref() == Some(path);
    ui.horizontal(|ui| {
        // Misma sangría que el hueco del triángulo de los nodos sin hijos.
        ui.add_space(INDENT);
        let tex = icons.texture(IconKey::Folder);
        let img = ui.add(
            egui::Image::new(tex)
                .fit_to_exact_size(egui::vec2(ICON_SIZE, ICON_SIZE))
                .sense(egui::Sense::click()),
        );
        let lbl = ui
            .selectable_label(is_active, label)
            .on_hover_text(path.display().to_string());
        if img.union(lbl).clicked() {
            actions.push(TreeAction::Navigate(path.to_path_buf()));
        }
    });
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
    theme: &crate::theme_apply::ActiveTheme,
    disk_usage: &std::collections::HashMap<std::path::PathBuf, naygo_core::disk::DiskUsage>,
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
            .fill(theme.selection_bg())
            .show(ui, row_content);
        // Barra azul vertical de 3px en el borde izquierdo de la fila.
        let rect = inner.response.rect;
        let bar = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + 3.0, rect.bottom()),
        );
        ui.painter().rect_filled(bar, 0.0, theme.accent());
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

    // Barra de uso de disco: solo bajo las raíces (unidades), justo debajo del
    // nombre y sobre las carpetas hijas. La rellena el worker async de espacio.
    if depth == 0 && node.drive_kind.is_some() {
        // Rect del bloque de uso (barra + texto), para hacerlo clicable abajo.
        let mut usage_rect: Option<egui::Rect> = None;
        if let Some(usage) = disk_usage.get(&node.path) {
            let pct = usage.percent_used();
            let frac = pct as f32 / 100.0;
            let color = if usage.is_critical() {
                egui::Color32::from_rgb(0xE0, 0x55, 0x55)
            } else if usage.is_high() {
                egui::Color32::from_rgb(0xE0, 0xA0, 0x30)
            } else {
                theme.accent()
            };
            let bar_row = ui
                .horizontal(|ui| {
                    ui.add_space(INDENT);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(120.0, 5.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 2.0, egui::Color32::from_gray(60));
                    let mut fill = rect;
                    fill.set_width(rect.width() * frac);
                    ui.painter().rect_filled(fill, 2.0, color);
                })
                .response;
            let label = i18n
                .t("disk.usage")
                .replace("{free}", &naygo_core::format::human_size(usage.free))
                .replace("{total}", &naygo_core::format::human_size(usage.total))
                .replace("{pct}", &pct.to_string());
            let text_row = ui
                .horizontal(|ui| {
                    ui.add_space(INDENT);
                    ui.label(egui::RichText::new(label).weak().small());
                })
                .response;
            usage_rect = Some(bar_row.rect.union(text_row.rect));
        }

        // La unidad es un BLOQUE clicable completo (pedido de Nicolás): además del
        // ícono+nombre, también navegan (a) el espacio a la DERECHA del nombre en su
        // fila y (b) la barra de uso + el texto de espacio. Son DOS interacts sobre
        // zonas sin widgets clicables — no se solapan con el triángulo ni el nombre,
        // así que no les roban el clic (lección del hit-test de egui).
        let panel_right = ui.max_rect().right();
        let strip = egui::Rect::from_min_max(
            egui::pos2(row.rect.right(), row.rect.top()),
            egui::pos2(panel_right, row.rect.bottom()),
        );
        let mut navigate = ui
            .interact(
                strip,
                egui::Id::new(("naygo_drive_strip", &node.path)),
                egui::Sense::click(),
            )
            .clicked();
        if let Some(u) = usage_rect {
            let below = egui::Rect::from_min_max(
                egui::pos2(row.rect.left(), u.top()),
                egui::pos2(panel_right, u.bottom()),
            );
            navigate |= ui
                .interact(
                    below,
                    egui::Id::new(("naygo_drive_usage", &node.path)),
                    egui::Sense::click(),
                )
                .clicked();
        }
        if navigate {
            actions.push(TreeAction::Navigate(node.path.clone()));
        }
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
                    ui.colored_label(theme.error(), format!("⚠ {}", i18n.t("tree.access_denied")));
                });
            }
            _ => {}
        }
        if let Some(children) = &node.children {
            for child in children {
                revealed |= show_node(
                    ui,
                    child,
                    child_depth,
                    tree,
                    actions,
                    icons,
                    i18n,
                    theme,
                    disk_usage,
                );
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
