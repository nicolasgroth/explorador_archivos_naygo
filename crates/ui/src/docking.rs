// Naygo — TabViewer de egui_dock: despacha cada panel por su PaneId/PanePurpose.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! El `TabViewer` recibe un `PaneId` por tab, busca el panel en el `Workspace` y
//! lo pinta según su `PanePurpose`. Las navegaciones que el usuario dispara con el
//! mouse (doble clic en una carpeta, botón "subir") se acumulan en `pending` para
//! que `NaygoApp` las ejecute tras pintar, evitando préstamos conflictivos.

use naygo_core::workspace::{PaneId, PanePurpose, Workspace};
use std::path::PathBuf;

/// Una navegación pedida desde un panel durante el pintado, a ejecutar después.
pub enum PaneRequest {
    /// El panel `id` debe navegar a `dir` (entra al historial).
    NavigateTo { id: PaneId, dir: PathBuf },
    /// El panel `id` pasa a ser el activo.
    Activate { id: PaneId },
}

pub struct NaygoTabViewer<'a> {
    pub workspace: &'a mut Workspace,
    pub status: &'a mut String,
    pub pending: &'a mut Vec<PaneRequest>,
    pub icons: &'a crate::icons::IconProvider,
    pub show_parent_entry: bool,
}

impl egui_dock::TabViewer for NaygoTabViewer<'_> {
    type Tab = PaneId;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match self.workspace.pane(*tab).map(|p| p.purpose) {
            Some(PanePurpose::Files) => {
                let name = self
                    .workspace
                    .pane(*tab)
                    .and_then(|p| p.files.as_ref())
                    .map(|f| {
                        f.current_dir
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| f.current_dir.display().to_string())
                    })
                    .unwrap_or_default();
                name.into()
            }
            Some(PanePurpose::Tree) => "Carpetas".into(),
            Some(PanePurpose::Inspector) => "Propiedades".into(),
            None => "—".into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let id = *tab;
        let purpose = self.workspace.pane(id).map(|p| p.purpose);
        match purpose {
            Some(PanePurpose::Files) => crate::panes::file_panel::show(
                ui,
                self.workspace,
                id,
                self.pending,
                self.icons,
                self.show_parent_entry,
            ),
            Some(PanePurpose::Tree) => {
                crate::panes::tree_panel::show(ui, self.workspace, self.pending, self.icons)
            }
            Some(PanePurpose::Inspector) => crate::panes::inspector_panel::show(ui, self.workspace),
            None => {
                ui.label("Panel desconocido");
            }
        }
        let _ = &mut self.status;
    }
}
