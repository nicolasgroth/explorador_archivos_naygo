// Naygo — panel de árbol (esqueleto de Fase 2A/2B): ubicación + ícono de unidad.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Esqueleto: muestra la carpeta del panel `Files` activo (con un ícono de unidad)
//! y permite subir. El árbol expandible real es trabajo posterior.

use crate::docking::PaneRequest;
use crate::icons::IconProvider;
use naygo_core::icon_kind::{DriveKind, IconKey};
use naygo_core::workspace::Workspace;

pub fn show(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    pending: &mut Vec<PaneRequest>,
    icons: &IconProvider,
    i18n: &naygo_core::i18n::I18n,
) {
    let active = workspace.active_id();
    let dir = workspace.active_files().map(|f| f.current_dir.clone());

    ui.horizontal(|ui| {
        let tex = icons.texture(IconKey::Drive(DriveKind::Unknown));
        ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(16.0, 16.0)));
        ui.label(i18n.t("tree.location"));
    });
    if let Some(d) = &dir {
        ui.monospace(d.display().to_string());
    } else {
        ui.label("—");
    }
    ui.separator();
    if ui.button(format!("⬆ {}", i18n.t("tree.go_up"))).clicked() {
        if let (Some(id), Some(d)) = (active, dir) {
            if let Some(parent) = d.parent() {
                pending.push(PaneRequest::NavigateTo {
                    id,
                    dir: parent.to_path_buf(),
                });
            }
        }
    }
}
