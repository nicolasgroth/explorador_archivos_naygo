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
    // Encabezados de columnas.
    tr.set_col_name(c.t("slint.col.name").into());
    tr.set_col_ext(c.t("slint.col.ext").into());
    tr.set_col_size(c.t("slint.col.size").into());
    tr.set_col_modified(c.t("slint.col.modified").into());
    // Diálogos de operaciones.
    tr.set_dlg_no_undo(c.t("slint.dialog.no_undo").into());
    tr.set_dlg_cancel(c.t("slint.dialog.cancel").into());
    tr.set_dlg_delete(c.t("slint.dialog.delete").into());
    tr.set_dlg_apply_all(c.t("slint.dialog.apply_all").into());
    tr.set_dlg_skip(c.t("slint.dialog.skip").into());
    tr.set_dlg_rename(c.t("slint.dialog.rename").into());
    tr.set_dlg_overwrite(c.t("slint.dialog.overwrite").into());
    tr.set_dlg_invalid_name(c.t("slint.dialog.invalid_name").into());
    tr.set_dlg_accept(c.t("slint.dialog.accept").into());
    tr.set_dlg_create(c.t("slint.dialog.create").into());
    tr.set_dlg_resume_q(c.t("slint.dialog.resume_q").into());
    tr.set_dlg_resume(c.t("slint.dialog.resume").into());
    tr.set_dlg_discard(c.t("slint.dialog.discard").into());
    // Menú contextual.
    tr.set_ctx_open(c.t("slint.ctx.open").into());
    tr.set_ctx_open_with(c.t("slint.ctx.open_with").into());
    tr.set_ctx_copy(c.t("slint.ctx.copy").into());
    tr.set_ctx_cut(c.t("slint.ctx.cut").into());
    tr.set_ctx_paste(c.t("slint.ctx.paste").into());
    tr.set_ctx_rename(c.t("slint.ctx.rename").into());
    tr.set_ctx_delete(c.t("slint.ctx.delete").into());
    tr.set_ctx_copy_names(c.t("slint.ctx.copy_names").into());
    tr.set_ctx_copy_path(c.t("slint.ctx.copy_path").into());
    tr.set_ctx_more_windows(c.t("slint.ctx.more_windows").into());
    // Paneles especiales.
    tr.set_tree_title(c.t("pane.tree.title").into());
    tr.set_inspector_title(c.t("pane.inspector.title").into());
    tr.set_inspector_no_selection(c.t("slint.inspector.no_selection").into());
    tr.set_inspector_name(c.t("slint.inspector.name").into());
    tr.set_inspector_kind(c.t("slint.inspector.kind").into());
    tr.set_inspector_size(c.t("slint.inspector.size").into());
    tr.set_inspector_modified(c.t("slint.inspector.modified").into());
    tr.set_inspector_created(c.t("slint.inspector.created").into());
    tr.set_inspector_path(c.t("slint.inspector.path").into());
    tr.set_history_title(c.t("pane.history.title").into());
    tr.set_history_empty(c.t("slint.history.empty").into());
    tr.set_history_undo(c.t("slint.history.undo").into());
    tr.set_fav_title(c.t("pane.favorites.title").into());
    tr.set_fav_empty(c.t("slint.fav.empty").into());
    tr.set_fav_recents(c.t("slint.fav.recents").into());
    tr.set_preview_title(c.t("pane.preview.title").into());
    tr.set_preview_select(c.t("slint.preview.select").into());
    tr.set_preview_truncated(c.t("slint.preview.truncated").into());
    // Menú de dirección del botón "+".
    tr.set_split_right(c.t("slint.split.right").into());
    tr.set_split_down(c.t("slint.split.down").into());
    tr.set_split_left(c.t("slint.split.left").into());
    tr.set_split_up(c.t("slint.split.up").into());
    // Otros.
    tr.set_fav_pin(c.t("slint.fav.pin").into());
    tr.set_drag_move(c.t("slint.drag.move").into());
    tr.set_drop_as_tab(c.t("slint.drop.as_tab").into());
    // Ventana de configuración.
    tr.set_cfg_title(c.t("settings.title").into());
    tr.set_cfg_close(c.t("slint.cfg.close").into());
    tr.set_cfg_cat_general(c.t("slint.cfg.cat_general").into());
    tr.set_cfg_cat_ops(c.t("slint.cfg.cat_ops").into());
    tr.set_cfg_cat_paste(c.t("slint.cfg.cat_paste").into());
    tr.set_cfg_cat_appearance(c.t("slint.cfg.cat_appearance").into());
    tr.set_cfg_cat_shortcuts(c.t("slint.cfg.cat_shortcuts").into());
    tr.set_cfg_cat_import(c.t("slint.cfg.cat_import").into());
    tr.set_cfg_show_parent(c.t("settings.show_parent").into());
    tr.set_cfg_icon_only(c.t("settings.icon_only").into());
    tr.set_cfg_bar_position(c.t("settings.bar_position").into());
    tr.set_cfg_bar_top(c.t("settings.bar.top").into());
    tr.set_cfg_bar_side(c.t("settings.bar.side").into());
    tr.set_cfg_size_no_subdirs(c.t("slint.cfg.size_no_subdirs").into());
    tr.set_cfg_autostart(c.t("slint.cfg.autostart").into());
    tr.set_cfg_date_format(c.t("slint.cfg.date_format").into());
    tr.set_cfg_size_format(c.t("slint.cfg.size_format").into());
    tr.set_cfg_size_auto(c.t("slint.cfg.size_auto").into());
    tr.set_cfg_size_bytes(c.t("slint.cfg.size_bytes").into());
    tr.set_cfg_size_kb(c.t("slint.cfg.size_kb").into());
    tr.set_cfg_size_mb(c.t("slint.cfg.size_mb").into());
    tr.set_cfg_row_density(c.t("slint.cfg.row_density").into());
    tr.set_cfg_density_compact(c.t("slint.cfg.density_compact").into());
    tr.set_cfg_density_comfortable(c.t("slint.cfg.density_comfortable").into());
    tr.set_cfg_ops_mode(c.t("slint.cfg.ops_mode").into());
    tr.set_cfg_ops_queue(c.t("slint.cfg.ops_queue").into());
    tr.set_cfg_ops_parallel(c.t("slint.cfg.ops_parallel").into());
    tr.set_cfg_confirm_trash(c.t("slint.cfg.confirm_trash").into());
    tr.set_cfg_show_op_summary(c.t("slint.cfg.show_op_summary").into());
    tr.set_cfg_paste_confirm(c.t("slint.cfg.paste_confirm").into());
    tr.set_cfg_paste_name(c.t("slint.cfg.paste_name").into());
    tr.set_cfg_paste_ext(c.t("slint.cfg.paste_ext").into());
    tr.set_cfg_theme(c.t("slint.cfg.theme").into());
    tr.set_cfg_language(c.t("slint.cfg.language").into());
    tr.set_cfg_icon_set(c.t("slint.cfg.icon_set").into());
    tr.set_cfg_shortcuts_hint(c.t("slint.cfg.shortcuts_hint").into());
    tr.set_cfg_capturing(c.t("settings.shortcuts.capturing").into());
    tr.set_cfg_press(c.t("slint.cfg.press").into());
    tr.set_cfg_no_shortcut(c.t("settings.shortcuts.none").into());
    tr.set_cfg_change(c.t("slint.cfg.change").into());
    tr.set_cfg_reset(c.t("slint.cfg.reset").into());
    tr.set_cfg_reset_all(c.t("settings.shortcuts.reset_all").into());
    tr.set_cfg_export_hint(c.t("slint.cfg.export_hint").into());
    tr.set_cfg_export_lang(c.t("slint.cfg.export_lang").into());
    tr.set_cfg_export_theme(c.t("slint.cfg.export_theme").into());
    tr.set_cfg_export_config(c.t("slint.cfg.export_config").into());
    tr.set_cfg_import(c.t("slint.cfg.import").into());
    tr.set_cfg_config_dir(c.t("settings.config_dir").into());
    tr.set_cfg_version(c.t("settings.version").into());
}
