// Naygo — puente i18n: vuelca los textos del idioma activo al global `Tr` de Slint. Una sola
// función (apply) mantiene sincronizados los catálogos (es.json/en.json) con las propiedades
// del global. Se llama al arrancar y cada vez que cambia el idioma.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::config_ctrl::ConfigCtrl;
use crate::Tr;
use slint::{ComponentHandle, Global};

/// Aplica todos los textos del idioma activo al global `Tr` de la ventana `ui`.
///
/// Genérico sobre la ventana: cada ventana Slint (AppWindow, ConfigWindow) tiene su PROPIA
/// copia del global `Tr`, así que hay que aplicarlo a cada instancia por separado.
pub fn apply<'a, W>(ui: &'a W, c: &ConfigCtrl)
where
    W: ComponentHandle,
    Tr<'a>: Global<'a, W>,
{
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
    tr.set_pathbar_fav_tip(c.t("slint.pathbar.fav_tip").into());
    tr.set_pathbar_copy_tip(c.t("slint.pathbar.copy_tip").into());
    tr.set_pathbar_copied(c.t("slint.pathbar.copied").into());
    tr.set_drive_eject_tip(c.t("slint.drive.eject_tip").into());
    tr.set_drive_eject(c.t("slint.drive.eject").into());
    tr.set_toolbar_refresh_drives(c.t("slint.toolbar.refresh_drives").into());
    tr.set_drive_eject_ok(c.t("slint.drive.eject_ok").into());
    tr.set_drive_eject_in_use(c.t("slint.drive.eject_in_use").into());
    tr.set_drive_eject_failed(c.t("slint.drive.eject_failed").into());
    tr.set_drive_eject_confirm_title(c.t("slint.drive.eject_confirm_title").into());
    tr.set_drive_eject_confirm(c.t("slint.drive.eject_confirm").into());
    tr.set_toolbar_layouts(c.t("slint.toolbar.layouts").into());
    tr.set_toolbar_layouts_tip(c.t("slint.toolbar.layouts_tip").into());
    tr.set_toolbar_new_folder(c.t("slint.toolbar.new_folder").into());
    tr.set_toolbar_new_folder_tip(c.t("slint.toolbar.new_folder_tip").into());
    tr.set_toolbar_terminal(c.t("slint.toolbar.terminal").into());
    tr.set_toolbar_terminal_tip(c.t("slint.toolbar.terminal_tip").into());
    tr.set_toolbar_terminal_wsl(c.t("slint.toolbar.terminal_wsl").into());
    tr.set_layout_save_current(c.t("slint.layout.save_current").into());
    tr.set_layout_save_title(c.t("slint.layout.save_title").into());
    tr.set_layout_save_placeholder(c.t("slint.layout.save_placeholder").into());
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
    tr.set_ctx_terminal_ps(c.t("slint.ctx.terminal_ps").into());
    tr.set_ctx_terminal_cmd(c.t("slint.ctx.terminal_cmd").into());
    tr.set_ctx_terminal_wt(c.t("slint.ctx.terminal_wt").into());
    tr.set_ctx_explorer(c.t("slint.ctx.explorer").into());
    tr.set_ctx_new_folder(c.t("slint.ctx.new_folder").into());
    tr.set_ctx_more_windows(c.t("slint.ctx.more_windows").into());
    tr.set_missing_title(c.t("slint.missing.title").into());
    tr.set_missing_body(c.t("slint.missing.body").into());
    tr.set_missing_retry(c.t("slint.missing.retry").into());
    tr.set_missing_ancestor(c.t("slint.missing.ancestor").into());
    tr.set_missing_choose(c.t("slint.missing.choose").into());
    tr.set_missing_close(c.t("slint.missing.close").into());
    tr.set_search_title(c.t("slint.search.title").into());
    tr.set_search_tip(c.t("slint.search.tip").into());
    tr.set_search_placeholder(c.t("slint.search.placeholder").into());
    tr.set_search_go(c.t("slint.search.go").into());
    tr.set_search_stop(c.t("slint.search.stop").into());
    tr.set_search_empty(c.t("slint.search.empty").into());
    tr.set_newfolder_title(c.t("slint.newfolder.title").into());
    tr.set_newfolder_in_dir(c.t("slint.newfolder.in_dir").into());
    tr.set_newfolder_hint(c.t("slint.newfolder.hint").into());
    tr.set_newfolder_create(c.t("slint.newfolder.create").into());
    tr.set_newfolder_cancel(c.t("slint.newfolder.cancel").into());
    // Ayuda (F1).
    tr.set_help_title(c.t("slint.help.title").into());
    tr.set_help_intro(c.t("slint.help.intro").into());
    tr.set_help_sec_panels(c.t("slint.help.sec_panels").into());
    tr.set_help_panels(c.t("slint.help.panels").into());
    tr.set_help_sec_nav(c.t("slint.help.sec_nav").into());
    tr.set_help_nav(c.t("slint.help.nav").into());
    tr.set_help_sec_cols(c.t("slint.help.sec_cols").into());
    tr.set_help_cols(c.t("slint.help.cols").into());
    tr.set_help_sec_ops(c.t("slint.help.sec_ops").into());
    tr.set_help_ops(c.t("slint.help.ops").into());
    tr.set_help_sec_layouts(c.t("slint.help.sec_layouts").into());
    tr.set_help_layouts(c.t("slint.help.layouts").into());
    tr.set_help_shortcuts(c.t("slint.help.shortcuts").into());
    tr.set_help_close(c.t("slint.help.close").into());
    // Ventana de renombrado por lotes (F5).
    tr.set_batch_title(c.t("slint.batch.title").into());
    tr.set_batch_template(c.t("slint.batch.template").into());
    tr.set_batch_find(c.t("slint.batch.find").into());
    tr.set_batch_replace(c.t("slint.batch.replace").into());
    tr.set_batch_regex(c.t("slint.batch.regex").into());
    tr.set_batch_include_ext(c.t("slint.batch.include_ext").into());
    tr.set_batch_case(c.t("slint.batch.case").into());
    tr.set_batch_case_none(c.t("slint.batch.case_none").into());
    tr.set_batch_case_lower(c.t("slint.batch.case_lower").into());
    tr.set_batch_case_upper(c.t("slint.batch.case_upper").into());
    tr.set_batch_case_title(c.t("slint.batch.case_title").into());
    tr.set_batch_counter(c.t("slint.batch.counter").into());
    tr.set_batch_counter_start(c.t("slint.batch.counter_start").into());
    tr.set_batch_counter_step(c.t("slint.batch.counter_step").into());
    tr.set_batch_col_before(c.t("slint.batch.col_before").into());
    tr.set_batch_col_after(c.t("slint.batch.col_after").into());
    tr.set_batch_collision(c.t("slint.batch.collision").into());
    tr.set_batch_invalid(c.t("slint.batch.invalid").into());
    tr.set_batch_items(c.t("slint.batch.items").into());
    tr.set_batch_apply(c.t("slint.batch.apply").into());
    // Menú/editor de columna (F2).
    tr.set_colmenu_sort_asc(c.t("slint.colmenu.sort_asc").into());
    tr.set_colmenu_sort_desc(c.t("slint.colmenu.sort_desc").into());
    tr.set_colmenu_filter(c.t("slint.colmenu.filter").into());
    tr.set_colmenu_clear_filter(c.t("slint.colmenu.clear_filter").into());
    tr.set_colmenu_hide(c.t("slint.colmenu.hide").into());
    tr.set_colmenu_move_left(c.t("slint.colmenu.move_left").into());
    tr.set_colmenu_move_right(c.t("slint.colmenu.move_right").into());
    tr.set_colfilter_contains(c.t("slint.colfilter.contains").into());
    tr.set_colfilter_case(c.t("slint.colfilter.case").into());
    tr.set_colfilter_min(c.t("slint.colfilter.min").into());
    tr.set_colfilter_max(c.t("slint.colfilter.max").into());
    tr.set_colfilter_types(c.t("slint.colfilter.types").into());
    tr.set_colfilter_no_ext(c.t("slint.colfilter.no_ext").into());
    tr.set_colfilter_apply(c.t("slint.colfilter.apply").into());
    tr.set_colfilter_clear(c.t("slint.colfilter.clear").into());
    tr.set_colfilter_size_hint(c.t("slint.colfilter.size_hint").into());
    tr.set_colfilter_date_hint(c.t("slint.colfilter.date_hint").into());
    tr.set_no_matches(c.t("slint.no_matches").into());
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
    tr.set_cfg_cat_about(c.t("slint.cfg.cat_about").into());
    tr.set_cfg_cat_advanced(c.t("slint.cfg.cat_advanced").into());
    tr.set_cfg_cat_preview(c.t("slint.cfg.cat_preview").into());
    tr.set_cfg_preview_hint(c.t("slint.cfg.preview_hint").into());
    tr.set_cfg_preview_ext(c.t("slint.cfg.preview_ext").into());
    tr.set_cfg_preview_on(c.t("slint.cfg.preview_on").into());
    tr.set_cfg_preview_as(c.t("slint.cfg.preview_as").into());
    tr.set_cfg_preview_add(c.t("slint.cfg.preview_add").into());
    tr.set_cfg_preview_add_ph(c.t("slint.cfg.preview_add_ph").into());
    tr.set_cfg_preview_add_title(c.t("slint.cfg.preview_add_title").into());
    tr.set_cfg_preview_as_ph(c.t("slint.cfg.preview_as_ph").into());
    tr.set_cfg_ops_display(c.t("slint.cfg.ops_display").into());
    tr.set_cfg_ops_display_panel(c.t("slint.cfg.ops_display_panel").into());
    tr.set_cfg_ops_display_modal(c.t("slint.cfg.ops_display_modal").into());
    tr.set_cfg_ops_display_always(c.t("slint.cfg.ops_display_always").into());
    tr.set_cfg_paste_image_fmt(c.t("slint.cfg.paste_image_fmt").into());
    tr.set_cfg_paste_fmt_png(c.t("slint.cfg.paste_fmt_png").into());
    tr.set_cfg_paste_fmt_jpg(c.t("slint.cfg.paste_fmt_jpg").into());
    tr.set_cfg_tray_enabled(c.t("slint.cfg.tray_enabled").into());
    tr.set_cfg_close_to_tray(c.t("slint.cfg.close_to_tray").into());
    tr.set_cfg_new_items_at_end(c.t("slint.cfg.new_items_at_end").into());
    tr.set_cfg_low_power(c.t("slint.cfg.low_power").into());
    tr.set_cfg_low_power_auto(c.t("slint.cfg.low_power_auto").into());
    tr.set_cfg_low_power_always(c.t("slint.cfg.low_power_always").into());
    tr.set_cfg_low_power_never(c.t("slint.cfg.low_power_never").into());
    tr.set_cfg_factory_reset(c.t("slint.cfg.factory_reset").into());
    tr.set_cfg_factory_reset_confirm(c.t("slint.cfg.factory_reset_confirm").into());
    // Sección "Acerca de".
    tr.set_about_version(c.t("slint.about.version").into());
    tr.set_about_author(c.t("about.author").into());
    tr.set_about_company(c.t("about.company").into());
    tr.set_about_license(c.t("about.license").into());
    tr.set_about_stack(c.t("slint.about.stack").into());
    tr.set_about_repo(c.t("about.repo").into());
    tr.set_about_egg_message(c.t("about.egg_message").into());
    tr.set_cfg_show_parent(c.t("settings.show_parent").into());
    tr.set_cfg_icon_only(c.t("settings.icon_only").into());
    tr.set_cfg_bar_position(c.t("settings.bar_position").into());
    tr.set_cfg_bar_top(c.t("settings.bar.top").into());
    tr.set_cfg_bar_side(c.t("settings.bar.side").into());
    tr.set_cfg_size_no_subdirs(c.t("slint.cfg.size_no_subdirs").into());
    tr.set_cfg_autostart(c.t("slint.cfg.autostart").into());
    tr.set_cfg_default_table(c.t("slint.cfg.default_table").into());
    tr.set_cfg_default_table_save(c.t("slint.cfg.default_table_save").into());
    tr.set_cfg_default_table_clear(c.t("slint.cfg.default_table_clear").into());
    tr.set_cfg_default_table_on(c.t("slint.cfg.default_table_on").into());
    tr.set_cfg_default_table_off(c.t("slint.cfg.default_table_off").into());
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
    tr.set_cfg_theme_preview(c.t("slint.cfg.theme_preview").into());
    tr.set_cfg_language(c.t("slint.cfg.language").into());
    tr.set_cfg_icon_set(c.t("slint.cfg.icon_set").into());
    tr.set_cfg_shortcuts_hint(c.t("slint.cfg.shortcuts_hint").into());
    tr.set_cfg_capturing(c.t("settings.shortcuts.capturing").into());
    tr.set_cfg_press(c.t("slint.cfg.press").into());
    tr.set_cfg_no_shortcut(c.t("settings.shortcuts.none").into());
    tr.set_cfg_sc_action(c.t("slint.cfg.sc_action").into());
    tr.set_cfg_sc_combo(c.t("slint.cfg.sc_combo").into());
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
