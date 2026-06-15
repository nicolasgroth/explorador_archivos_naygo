// Naygo — puente i18n: vuelca los textos del idioma activo al global `Tr` de Slint. Una sola
// función (apply) mantiene sincronizados los catálogos (es.json/en.json) con las propiedades
// del global. Se llama al arrancar y cada vez que cambia el idioma.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::config_ctrl::ConfigCtrl;
use crate::{AppWindow, Tr};
use slint::ComponentHandle;

/// Aplica todos los textos del idioma activo al global `Tr` de la ventana `ui`.
pub fn apply(ui: &AppWindow, c: &ConfigCtrl) {
    let tr = ui.global::<Tr>();
    // Barra de herramientas.
    tr.set_toolbar_up(c.t("slint.toolbar.up").into());
    tr.set_toolbar_up_tip(c.t("toolbar.up").into());
    tr.set_toolbar_add_tip(c.t("slint.toolbar.add").into());
    tr.set_toolbar_panel(c.t("slint.toolbar.panel").into());
    tr.set_toolbar_panel_tip(c.t("toolbar.add_other").into());
    tr.set_toolbar_swap(c.t("slint.toolbar.swap").into());
    tr.set_toolbar_swap_tip(c.t("toolbar.swap_panes").into());
    tr.set_toolbar_clone(c.t("slint.toolbar.clone").into());
    tr.set_toolbar_clone_tip(c.t("toolbar.clone_path").into());
    tr.set_toolbar_tabs(c.t("slint.toolbar.tabs").into());
    tr.set_toolbar_tabs_tip(c.t("slint.toolbar.tabs_tip").into());
    // Menú desplegable "agregar panel".
    tr.set_add_files(c.t("slint.add.files").into());
    tr.set_add_tree(c.t("pane.tree.title").into());
    tr.set_add_inspector(c.t("pane.inspector.title").into());
    tr.set_add_history(c.t("pane.history.title").into());
    tr.set_add_favorites(c.t("pane.favorites.title").into());
    tr.set_add_preview(c.t("pane.preview.title").into());
}
