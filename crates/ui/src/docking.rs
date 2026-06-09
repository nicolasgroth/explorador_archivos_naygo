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
    /// Iniciar un arrastre OLE hacia el SO (Naygo → Explorer/escritorio/correo). Lo emite
    /// la celda Nombre de un file panel cuando el usuario empieza a arrastrar. `NaygoApp`
    /// lo despacha **fuera** del closure de egui (vía `platform::dnd::start_drag`), porque
    /// `DoDragDrop` corre un bucle modal que toma el control del mouse. Mismo patrón que el
    /// menú contextual nativo (`native_menu_request`).
    StartOsDrag { paths: Vec<PathBuf> },
}

pub struct NaygoTabViewer<'a> {
    pub workspace: &'a mut Workspace,
    pub status: &'a mut String,
    pub pending: &'a mut Vec<PaneRequest>,
    pub icons: &'a crate::icons::IconProvider,
    pub theme: &'a crate::theme_apply::ActiveTheme,
    pub show_parent_entry: bool,
    pub i18n: &'a naygo_core::i18n::I18n,
    pub trees:
        &'a std::collections::HashMap<naygo_core::workspace::PaneId, naygo_core::tree::DirTree>,
    pub tree_actions: &'a mut Vec<(
        naygo_core::workspace::PaneId,
        crate::tree_actions::TreeAction,
    )>,
    /// Panes de árbol cuyo nodo objetivo de `reveal_to` se pintó (y se hizo scroll)
    /// en este frame. `NaygoApp` limpia `reveal_to` SOLO para estos panes.
    pub tree_revealed: &'a mut std::collections::HashSet<naygo_core::workspace::PaneId>,
    /// Acciones de tabla (menú de columna) acumuladas al pintar los file panels.
    pub table_actions: &'a mut Vec<(
        naygo_core::workspace::PaneId,
        crate::table_actions::TableAction,
    )>,
    /// Disparadores de operaciones del menú contextual de un file panel, a aplicar
    /// (vía `apply_action`) tras pintar. Patrón de acción diferida del file panel.
    pub ops_actions: &'a mut Vec<crate::input::Action>,
    /// Petición de menú contextual nativo (shell-B): coords de PANTALLA del clic del
    /// ítem "Más opciones de Windows…". La procesa NaygoApp tras pintar el dock.
    pub native_menu_request: &'a mut Option<(f32, f32)>,
    /// Espacio por unidad (root → uso) para pintar la barra de uso en el árbol.
    pub disk_usage: &'a std::collections::HashMap<std::path::PathBuf, naygo_core::disk::DiskUsage>,
    /// Si los archivos recién aparecidos (resaltados) se agrupan al final de la vista.
    pub new_items_at_end: bool,
    /// Carpetas cuyo tamaño calculado es parcial (p. ej. subcarpetas sin acceso).
    pub size_partial: &'a std::collections::HashSet<std::path::PathBuf>,
}

impl egui_dock::TabViewer for NaygoTabViewer<'_> {
    type Tab = PaneId;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        let name: String = match self.workspace.pane(*tab).map(|p| p.purpose) {
            Some(PanePurpose::Files) => self
                .workspace
                .pane(*tab)
                .and_then(|p| p.files.as_ref())
                .map(|f| {
                    f.current_dir
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| f.current_dir.display().to_string())
                })
                .unwrap_or_default(),
            Some(PanePurpose::Tree) => self.i18n.t("pane.tree.title").to_string(),
            Some(PanePurpose::Inspector) => self.i18n.t("pane.inspector.title").to_string(),
            None => "—".to_string(),
        };
        // Resaltar el panel activo: título en color de acento + negrita.
        if self.workspace.active_id() == Some(*tab) {
            egui::RichText::new(name)
                .color(self.theme.accent())
                .strong()
                .into()
        } else {
            name.into()
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let id = *tab;
        let purpose = self.workspace.pane(id).map(|p| p.purpose);
        match purpose {
            Some(PanePurpose::Files) => {
                let mut local: Vec<crate::table_actions::TableAction> = Vec::new();
                crate::panes::file_panel::show(
                    ui,
                    self.workspace,
                    id,
                    self.pending,
                    self.icons,
                    self.show_parent_entry,
                    self.i18n,
                    &mut local,
                    self.theme,
                    self.ops_actions,
                    self.native_menu_request,
                    self.new_items_at_end,
                    self.size_partial,
                );
                for a in local {
                    self.table_actions.push((id, a));
                }
            }
            Some(PanePurpose::Tree) => {
                if let Some(tree) = self.trees.get(&id) {
                    let mut local: Vec<crate::tree_actions::TreeAction> = Vec::new();
                    let revealed = crate::panes::tree_panel::show(
                        ui,
                        tree,
                        &mut local,
                        self.icons,
                        self.i18n,
                        self.theme,
                        self.disk_usage,
                    );
                    if revealed {
                        self.tree_revealed.insert(id);
                    }
                    for a in local {
                        self.tree_actions.push((id, a));
                    }
                } else {
                    ui.label(self.i18n.t("tree.loading"));
                }
            }
            Some(PanePurpose::Inspector) => {
                crate::panes::inspector_panel::show(ui, self.workspace, self.i18n)
            }
            None => {
                ui.label(self.i18n.t("pane.unknown"));
            }
        }
        let _ = &mut self.status;
    }
}
