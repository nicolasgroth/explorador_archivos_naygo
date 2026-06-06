// Naygo — panel de árbol (esqueleto de Fase 2A): ubicación del panel activo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Esqueleto: muestra la carpeta del panel `Files` activo y permite subir. El
//! árbol expandible real es trabajo posterior. Emite un request de navegación
//! sobre el panel activo.

use crate::docking::PaneRequest;
use crate::icons::IconProvider;
use naygo_core::workspace::Workspace;

pub fn show(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    pending: &mut Vec<PaneRequest>,
    _icons: &IconProvider,
) {
    let active = workspace.active_id();
    let dir = workspace.active_files().map(|f| f.current_dir.clone());

    ui.label("Panel activo en:");
    if let Some(d) = &dir {
        ui.monospace(d.display().to_string());
    } else {
        ui.label("—");
    }
    ui.separator();
    if ui.button("⬆ Subir un nivel").clicked() {
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
