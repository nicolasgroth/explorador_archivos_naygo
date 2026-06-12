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
    /// Confirmar un rename inline (R1): renombrar `source` a `new_name`. `NaygoApp`
    /// lo convierte en un OpRequest de Rename (mismo camino que el diálogo viejo).
    CommitRename { source: PathBuf, new_name: String },
}

pub struct NaygoTabViewer<'a> {
    pub workspace: &'a mut Workspace,
    /// Estado del rename inline (F2): vive en `NaygoApp`, lo pinta/maneja file_panel.
    pub inline_rename: &'a mut Option<crate::app::InlineRename>,
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
    /// Historial de deshacer (lo pinta el panel History; NaygoApp lo posee).
    pub undo_history: &'a [naygo_core::ops::undo::UndoEntry],
    /// Ids de entradas del historial a deshacer, EN ORDEN (diferido a NaygoApp).
    pub undo_clicks: &'a mut Vec<u64>,
    /// Epoch (s) de este frame, para el "hace cuánto" del historial.
    pub now_epoch: u64,
    /// Favoritos como (etiqueta, ruta), en orden de usuario. Los pintan la
    /// path-bar (☆/★), el panel Favoritos y la sección anclada del árbol.
    pub favorites: &'a [(String, std::path::PathBuf)],
    /// Carpetas recientes (ya podadas de rutas inexistentes por NaygoApp).
    pub recent_dirs: &'a [std::path::PathBuf],
    /// Navegaciones pedidas desde el panel Favoritos (diferidas a NaygoApp).
    pub fav_navigate: &'a mut Vec<std::path::PathBuf>,
    /// Favoritos a quitar (clic derecho en el panel Favoritos; diferido).
    pub fav_remove: &'a mut Vec<std::path::PathBuf>,
    /// Edición de la ruta de un panel (vive en NaygoApp, como `inline_rename`).
    pub path_edit: &'a mut Option<crate::app::PathEdit>,
    /// Acciones diferidas de la path-bar (copiar/favorito/ruta inválida).
    pub pathbar_actions: &'a mut Vec<crate::pathbar::PathBarAction>,
    /// Carpeta padre cuyos nombres tiene cargados el worker de autocompletado.
    pub path_ac_parent: &'a str,
    /// Nombres de subcarpetas de `path_ac_parent` (candidatos sin filtrar).
    pub path_ac_names: &'a [String],
    /// El panel en edición de ruta SÍ se pintó este frame. Si al final del frame
    /// hay `path_edit` pero nadie lo pintó (tab oculto en un stack o cerrado),
    /// `NaygoApp` descarta la edición — si no, el input global quedaría suspendido
    /// apuntando a un panel invisible.
    pub path_edit_seen: &'a mut bool,
    /// Filas visibles medidas por cada file panel ESTE frame (salida). `NaygoApp` lo
    /// guarda para el tamaño de página de AvPag/RePag.
    pub visible_rows: &'a mut std::collections::HashMap<naygo_core::workspace::PaneId, usize>,
    /// Posición de vista a la que cada file panel debe hacer scroll este frame (foco
    /// movido por teclado). Lo consume el panel; `NaygoApp` lo limpia tras pintar.
    pub scroll_to_focus: &'a std::collections::HashMap<naygo_core::workspace::PaneId, usize>,
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
            Some(PanePurpose::History) => self.i18n.t("pane.history.title").to_string(),
            Some(PanePurpose::Favorites) => self.i18n.t("pane.favorites.title").to_string(),
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
        // CRÍTICO: aislar el espacio de IDs de egui de CADA panel. egui_dock entrega a
        // todos los tabs `ui`s con el mismo id base, así que los auto-ids internos de dos
        // paneles del mismo tipo COLISIONAN (mismo widget ID para las celdas/ScrollArea de
        // ambas tablas — visible con las advertencias rojas "First/Second use of widget ID"
        // en builds debug). Con IDs duplicados la interacción de egui queda indefinida: los
        // clics en las filas se pierden aunque el hover se pinte. `push_id` con el PaneId
        // hace único todo lo que el panel cree adentro.
        ui.push_id(("naygo_tab_id_scope", id.0), |ui| {
            self.tab_contents(ui, id);
        });
    }
}

impl NaygoTabViewer<'_> {
    /// Contenido real de un tab (separado de `ui()` para poder envolverlo en `push_id`).
    fn tab_contents(&mut self, ui: &mut egui::Ui, id: PaneId) {
        let purpose = self.workspace.pane(id).map(|p| p.purpose);
        match purpose {
            Some(PanePurpose::Files) => {
                let mut local: Vec<crate::table_actions::TableAction> = Vec::new();
                // ¿La carpeta actual de ESTE panel es favorita? (decide ☆ vs ★ en
                // su path-bar). Comparación en memoria sobre la lista de favoritos.
                let is_favorite = self
                    .workspace
                    .pane(id)
                    .and_then(|p| p.files.as_ref())
                    .map(|f| self.favorites.iter().any(|(_, fp)| fp == &f.current_dir))
                    .unwrap_or(false);
                let pathbar = crate::pathbar::PathBarParams {
                    is_favorite,
                    path_edit: &mut *self.path_edit,
                    actions: &mut *self.pathbar_actions,
                    ac_parent: self.path_ac_parent,
                    ac_names: self.path_ac_names,
                    recents: self.recent_dirs,
                };
                let mut visible_rows_out: Option<usize> = None;
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
                    self.inline_rename,
                    pathbar,
                    self.scroll_to_focus.get(&id).copied(),
                    &mut visible_rows_out,
                );
                if let Some(rows) = visible_rows_out {
                    self.visible_rows.insert(id, rows);
                }
                for a in local {
                    self.table_actions.push((id, a));
                }
                // Marcar la edición de ruta como "pintada" DESPUÉS del show: cubre
                // tanto la edición ya abierta como la que el clic en la barra abrió
                // recién durante este mismo pintado. Si el panel la cerró (Esc),
                // path_edit ya es None y la guarda final de NaygoApp no hace nada.
                if self.path_edit.as_ref().is_some_and(|e| e.pane == id) {
                    *self.path_edit_seen = true;
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
                        self.favorites,
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
            Some(PanePurpose::History) => {
                crate::panes::history_panel::show(
                    ui,
                    self.undo_history,
                    self.i18n,
                    self.theme,
                    self.now_epoch,
                    self.undo_clicks,
                );
            }
            Some(PanePurpose::Favorites) => {
                crate::panes::favorites_panel::show(
                    ui,
                    self.favorites,
                    self.recent_dirs,
                    self.i18n,
                    self.theme,
                    self.fav_navigate,
                    self.fav_remove,
                );
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
