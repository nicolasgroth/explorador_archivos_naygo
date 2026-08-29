// Naygo — closures de refresco: VM de configuración, íconos de toolbar, discos y plantillas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Factories de los closures `refresh_*` que vuelcan estado del controlador a la UI: el
// SettingsVm completo de la ventana de Configuración, los 18 íconos de la toolbar, la tira
// de unidades de disco y la lista de plantillas de disposición. Se llaman al arrancar y tras
// cada cambio relevante (idioma/tema/set de íconos/plantillas/unidades). Extraídos de
// `main.rs` sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::vm_builders::*;
use crate::workspace_ctrl::WorkspaceCtrl;
use crate::*;
use slint::{ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

/// Construye el closure que reconstruye el SettingsVm + filas de atajos desde ConfigCtrl
/// y los vuelca a la ventana de Configuración (y el SettingsVm de la toolbar).
pub(crate) fn build_refresh_config_vm(
    ctrl: Rc<RefCell<WorkspaceCtrl>>,
    ui_weak: slint::Weak<AppWindow>,
    cfg_weak: slint::Weak<ConfigWindow>,
) -> Rc<dyn Fn()> {
    let ctrl = ctrl.clone();
    let ui_weak = ui_weak.clone();
    let cfg_weak = cfg_weak.clone();
    Rc::new(move || {
        // Fase 1: extraer todo lo necesario del borrow inmutable antes de liberarlo.
        // Esto permite un borrow_mut posterior para los íconos (IconCache::get es &mut).
        let (
            settings_vm,
            recent_limit,
            frequent_dirs_limit,
            auto_hl,
            anim_enabled,
            footer_en,
            footer_preset,
            footer_tpl,
            footer_prev,
            home_dir,
            shortcut_rows,
            prev_rules,
            theme_cards,
            editing_active,
            editing_name,
            editing_base_index,
            editing_flat_inactive,
            editing_use_inactive_row_color,
            editing_hexes,
            editing_rs,
            editing_gs,
            editing_bs,
            config_dir_str,
            // Datos para la grilla de íconos (se usan en fase 2).
            icon_set_id,
            icon_overrides,
            icon_catalog_info, // Vec<(id, label, tintable)>
            icon_tintable,
        ) = {
            let c = ctrl.borrow();
            let settings_vm = build_settings_vm(&c.config);
            let recent_limit = c.config.settings.recent_limit as i32;
            let frequent_dirs_limit = c.config.settings.frequent_dirs_limit as i32;
            let auto_hl = c.config.auto_highlight_code();
            let anim_enabled = c.config.animations_enabled();
            let footer_en = c.config.footer_enabled();
            let footer_preset = c.config.footer_preset_index();
            let footer_tpl = c.config.footer_custom_template().to_string();
            let footer_prev = c.config.footer_preview().to_string();
            let home_dir = c.config.home_dir().to_string();
            let shortcut_rows: Vec<ShortcutRowVm> = c
                .config
                .shortcut_list()
                .into_iter()
                .map(|(key, label, chord)| ShortcutRowVm {
                    action_key: key.into(),
                    label: label.into(),
                    chord_text: chord.into(),
                    conflict: SharedString::new(),
                })
                .collect();
            let prev_rules: Vec<PreviewRuleVm> = c
                .preview_rules()
                .into_iter()
                .map(|(ext, enabled, view_index, lang_index)| PreviewRuleVm {
                    ext: ext.into(),
                    enabled,
                    view_index,
                    lang_index,
                })
                .collect();
            let active_theme = c.config.settings.theme.clone();
            let col =
                |tc: naygo_core::theme::ThemeColor| slint::Color::from_rgb_u8(tc.r, tc.g, tc.b);
            let theme_cards: Vec<ThemeCardVm> = c
                .config
                .themes
                .available()
                .iter()
                .map(|id| {
                    let t = c.config.themes.get(id);
                    ThemeCardVm {
                        id: id.as_str().into(),
                        name: t.name.clone().into(),
                        active: id.as_str() == active_theme.as_str(),
                        is_builtin: naygo_core::theme::is_builtin_id(id.as_str()),
                        sw_panel: col(t.panel_bg),
                        sw_accent: col(t.accent),
                        sw_row: col(t.row_bg),
                        sw_text: col(t.text),
                        sw_highlight: col(t.highlight),
                    }
                })
                .collect();
            let editing_active = c.config.is_editing_theme();
            let editing_name = c.config.editing_name().to_string();
            let editing_base_index = c.config.editing_base_index();
            let editing_flat_inactive = c.config.editing_flat_inactive();
            let editing_use_inactive_row_color = c.config.editing_use_inactive_row_color();
            let n = config_ctrl::THEME_TOKEN_COUNT;
            let mut editing_hexes: Vec<SharedString> = Vec::with_capacity(n);
            let mut editing_rs: Vec<i32> = Vec::with_capacity(n);
            let mut editing_gs: Vec<i32> = Vec::with_capacity(n);
            let mut editing_bs: Vec<i32> = Vec::with_capacity(n);
            if editing_active {
                for idx in 0..n {
                    editing_hexes.push(c.config.editing_token_hex(idx).into());
                    let (r, g, b) = c.config.editing_token_rgb(idx);
                    editing_rs.push(r as i32);
                    editing_gs.push(g as i32);
                    editing_bs.push(b as i32);
                }
            }
            let config_dir_str = c.config.config_dir.to_string_lossy().to_string();
            // Datos para la grilla de íconos (Task 15).
            let icon_set_id = c.config.settings.icon_set.clone();
            let icon_overrides = c.config.settings.icon_overrides.clone();
            let catalog = naygo_core::icon_set::IconSetCatalog::load(&c.config.config_dir);
            let icon_catalog_info: Vec<(String, String, bool)> = catalog
                .available()
                .iter()
                .map(|s| (s.id.clone(), s.label.clone(), s.tintable))
                .collect();
            let icon_tintable = catalog.is_tintable(&icon_set_id);
            (
                settings_vm,
                recent_limit,
                frequent_dirs_limit,
                auto_hl,
                anim_enabled,
                footer_en,
                footer_preset,
                footer_tpl,
                footer_prev,
                home_dir,
                shortcut_rows,
                prev_rules,
                theme_cards,
                editing_active,
                editing_name,
                editing_base_index,
                editing_flat_inactive,
                editing_use_inactive_row_color,
                editing_hexes,
                editing_rs,
                editing_gs,
                editing_bs,
                config_dir_str,
                icon_set_id,
                icon_overrides,
                icon_catalog_info,
                icon_tintable,
            )
            // c se libera aquí al salir del bloque
        };

        // Fase 2: borrow_mut para obtener los íconos del cache (IconCache::get es &mut).
        // Se hace DESPUÉS de liberar el borrow inmutable de arriba.
        let (icon_rows_vm, icon_set_labels_vm): (ModelRc<IconRowVm>, ModelRc<SharedString>) = {
            let mut c = ctrl.borrow_mut();
            // Etiquetas de cada set (mismo orden que icon_sets en SettingsVm).
            let set_labels: Vec<SharedString> = icon_catalog_info
                .iter()
                .map(|(_, label, _)| SharedString::from(label.as_str()))
                .collect();
            // Descripción de la fuente de un ícono: "(base)" o "· override".
            let active_label = icon_catalog_info
                .iter()
                .find(|(id, _, _)| id == &icon_set_id)
                .map(|(_, l, _)| l.as_str())
                .unwrap_or(&icon_set_id);
            let rows: Vec<IconRowVm> = naygo_core::icons::all_keys()
                .into_iter()
                .map(|key| {
                    let key_str = naygo_core::icon_source::key_to_string(key);
                    let overridden = icon_overrides.contains_key(&key_str);
                    let origin: SharedString = if overridden {
                        match icon_overrides.get(&key_str) {
                            Some(naygo_core::icon_source::IconSource::Builtin { set_id }) => {
                                let lbl = icon_catalog_info
                                    .iter()
                                    .find(|(id, _, _)| id == set_id)
                                    .map(|(_, l, _)| l.as_str())
                                    .unwrap_or(set_id.as_str());
                                format!("{lbl} · override").into()
                            }
                            Some(naygo_core::icon_source::IconSource::UserPng { .. }) => {
                                c.config.t("settings.icons.origin_user_png").into()
                            }
                            None => format!("{active_label} (base)").into(),
                        }
                    } else {
                        format!("{active_label} (base)").into()
                    };
                    let group: i32 = if key_str.starts_with("action_") { 0 } else { 1 };
                    let icon_img = c.icons.get(key);
                    IconRowVm {
                        key: key_str.clone().into(),
                        label: c.config.t(&format!("icons.obj.{key_str}")).into(),
                        icon: icon_img,
                        origin,
                        overridden,
                        group,
                    }
                })
                .collect();
            (
                ModelRc::from(Rc::new(VecModel::from(rows))),
                ModelRc::from(Rc::new(VecModel::from(set_labels))),
            )
        };

        // Fase 2b: tarjetas de la galería de sets (render_for_set no usa el cache mutable).
        let icon_set_cards_vm: ModelRc<IconSetCardVm> = {
            use naygo_core::icon_kind::{ActionIcon, FileCategory, IconKey};
            let sample_keys = [
                IconKey::Folder,
                IconKey::File(FileCategory::Image),
                IconKey::Action(ActionIcon::Copy),
                IconKey::Action(ActionIcon::Settings),
                IconKey::Action(ActionIcon::Back),
            ];
            let config_path = std::path::Path::new(&config_dir_str);
            let c = ctrl.borrow();
            let tint = theme_text_rgb(&c.config.settings, &c.config.themes);
            drop(c);
            let active = &icon_set_id;
            let cards: Vec<IconSetCardVm> = icon_catalog_info
                .iter()
                .map(|(id, label, tintable)| {
                    let samples: Vec<slint::Image> = sample_keys
                        .iter()
                        .map(|k| {
                            crate::icons::render_for_set(
                                *k,
                                id.as_str(),
                                *tintable,
                                tint,
                                config_path,
                            )
                        })
                        .collect();
                    IconSetCardVm {
                        id: id.as_str().into(),
                        label: label.as_str().into(),
                        samples: ModelRc::from(Rc::new(VecModel::from(samples))),
                        selected: id == active,
                    }
                })
                .collect();
            ModelRc::from(Rc::new(VecModel::from(cards)))
        };

        // Fase 3: volcar todo a la UI. El SettingsVm se emite con los campos de íconos ya
        // rellenos antes de llamar a set_vm / set_settings_vm.
        let mut settings_vm = settings_vm;
        settings_vm.icon_rows = icon_rows_vm;
        settings_vm.icon_set_labels = icon_set_labels_vm;
        settings_vm.icon_set_tintable = icon_tintable;
        settings_vm.icon_set_cards = icon_set_cards_vm;

        // La AppWindow sigue usando `settings-vm` en la toolbar (icon-only, alto de fila),
        // así que se mantiene actualizado ahí además de en la ventana de configuración.
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_settings_vm(settings_vm.clone());
        }
        let Some(cfg) = cfg_weak.upgrade() else {
            return;
        };
        cfg.set_vm(settings_vm);
        // Poblar el campo de límite de recientes (no está en SettingsVm).
        cfg.set_recent_limit(recent_limit);
        cfg.set_frequent_dirs_limit(frequent_dirs_limit);
        // Auto-resaltado de código + footer (mostrar/plantilla/template/preview) + Home:
        // campos que no viven en SettingsVm; se vuelcan directo a las props de la ventana.
        cfg.set_auto_highlight_code(auto_hl);
        cfg.set_animations_enabled(anim_enabled);
        cfg.set_footer_enabled(footer_en);
        cfg.set_footer_preset_index(footer_preset);
        cfg.set_footer_custom_template(footer_tpl.into());
        cfg.set_footer_preview(footer_prev.into());
        cfg.set_home_dir(home_dir.into());
        cfg.set_shortcuts(ModelRc::from(Rc::new(VecModel::from(shortcut_rows))));
        // Reglas de previsualización (C3).
        cfg.set_preview_rules(ModelRc::from(Rc::new(VecModel::from(prev_rules))));
        // Nombres legibles de los lenguajes de código para el combobox (orden de CodeLang::all()).
        let lang_names: Vec<SharedString> = naygo_core::preview::CodeLang::all()
            .iter()
            .map(|l| SharedString::from(code_lang_label(*l)))
            .collect();
        cfg.set_lang_names(ModelRc::from(Rc::new(VecModel::from(lang_names))));
        // Tarjetas de tema para la galería de selección (config → Apariencia).
        cfg.set_theme_cards(ModelRc::from(Rc::new(VecModel::from(theme_cards))));
        // Estado del editor de temas (config → Apariencia). Cuando hay un tema en edición se
        // vuelcan su nombre/base, el flag "paneles inactivos planos" y los 12 tokens (hex +
        // r/g/b por canal, para inicializar el color-picker, que no parsea hex).
        cfg.set_editing_active(editing_active);
        cfg.set_editing_name(editing_name.into());
        cfg.set_editing_base_index(editing_base_index);
        cfg.set_editing_flat_inactive(editing_flat_inactive);
        cfg.set_editing_use_inactive_row_color(editing_use_inactive_row_color);
        cfg.set_editing_token_hex(ModelRc::from(Rc::new(VecModel::from(editing_hexes))));
        cfg.set_editing_token_r(ModelRc::from(Rc::new(VecModel::from(editing_rs))));
        cfg.set_editing_token_g(ModelRc::from(Rc::new(VecModel::from(editing_gs))));
        cfg.set_editing_token_b(ModelRc::from(Rc::new(VecModel::from(editing_bs))));
        cfg.set_config_dir(config_dir_str.into());
        cfg.set_app_version(naygo_full_version().into());
        // Sección "Novedades": parsear el CHANGELOG embebido y volcar las notas de la
        // versión actual. Se setea una sola vez (no cambia en runtime).
        {
            let notes = naygo_core::changelog::release_notes(CHANGELOG, env!("CARGO_PKG_VERSION"));
            let sections: Vec<NewsSection> = notes
                .map(|n| {
                    n.sections
                        .into_iter()
                        .map(|s| NewsSection {
                            category: s.category.into(),
                            items: ModelRc::new(VecModel::from(
                                s.items
                                    .into_iter()
                                    .map(SharedString::from)
                                    .collect::<Vec<_>>(),
                            )),
                        })
                        .collect()
                })
                .unwrap_or_default();
            cfg.set_release_notes(ModelRc::new(VecModel::from(sections)));
        }
    })
}

// Vuelca los 18 íconos de acción de la toolbar al AppWindow (props tb-*).
// Se llama al arrancar y cada vez que cambia el set de íconos o el tema (re-tintado).
pub(crate) fn build_refresh_toolbar_icons(
    ctrl: Rc<RefCell<WorkspaceCtrl>>,
    ui_weak: slint::Weak<AppWindow>,
) -> Rc<dyn Fn()> {
    let ctrl = ctrl.clone();
    let ui_weak = ui_weak.clone();
    Rc::new(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        use naygo_core::icon_kind::{ActionIcon, IconKey};
        let mut c = ctrl.borrow_mut();
        ui.set_tb_back(c.icons.get(IconKey::Action(ActionIcon::Back)));
        ui.set_tb_forward(c.icons.get(IconKey::Action(ActionIcon::Forward)));
        ui.set_tb_up(c.icons.get(IconKey::Action(ActionIcon::Up)));
        ui.set_tb_refresh(c.icons.get(IconKey::Action(ActionIcon::Refresh)));
        ui.set_tb_home(c.icons.get(IconKey::Action(ActionIcon::Home)));
        ui.set_tb_search(c.icons.get(IconKey::Action(ActionIcon::Search)));
        ui.set_tb_show_hidden(c.icons.get(IconKey::Action(ActionIcon::ShowHidden)));
        ui.set_tb_history(c.icons.get(IconKey::Action(ActionIcon::History)));
        ui.set_tb_favorites(c.icons.get(IconKey::Action(ActionIcon::Favorites)));
        ui.set_tb_split(c.icons.get(IconKey::Action(ActionIcon::Split)));
        ui.set_tb_panel(c.icons.get(IconKey::Action(ActionIcon::Panel)));
        ui.set_tb_tabs(c.icons.get(IconKey::Action(ActionIcon::Tabs)));
        ui.set_tb_swap(c.icons.get(IconKey::Action(ActionIcon::SwapPanes)));
        ui.set_tb_clone(c.icons.get(IconKey::Action(ActionIcon::ClonePath)));
        ui.set_tb_new_folder(c.icons.get(IconKey::Action(ActionIcon::NewFolder)));
        ui.set_tb_terminal(c.icons.get(IconKey::Action(ActionIcon::Terminal)));
        ui.set_tb_layouts(c.icons.get(IconKey::Action(ActionIcon::Layouts)));
        ui.set_tb_settings(c.icons.get(IconKey::Action(ActionIcon::Settings)));
    })
}

// Vuelca la tira de unidades de disco a la toolbar. Se llama al arrancar, al cambiar el set
// de íconos (los íconos de disco cambian) y al cambiar los dispositivos (USB conectado/sacado).
pub(crate) fn build_refresh_drives(
    ctrl: Rc<RefCell<WorkspaceCtrl>>,
    ui_weak: slint::Weak<AppWindow>,
) -> Rc<dyn Fn()> {
    let ctrl = ctrl.clone();
    let ui_weak = ui_weak.clone();
    Rc::new(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        let drives: Vec<NavRow> = ctrl
            .borrow_mut()
            .drive_strip()
            .into_iter()
            .map(to_nav_row)
            .collect();
        ui.set_drives(ModelRc::from(Rc::new(VecModel::from(drives))));
    })
}

// Vuelca la lista de plantillas de disposición (built-in + usuario) al menú de Layouts (F4).
// Se llama al arrancar y tras guardar/borrar una plantilla.
pub(crate) fn build_refresh_layouts(
    ctrl: Rc<RefCell<WorkspaceCtrl>>,
    ui_weak: slint::Weak<AppWindow>,
) -> Rc<dyn Fn()> {
    let ctrl = ctrl.clone();
    let ui_weak = ui_weak.clone();
    Rc::new(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        let rows: Vec<LayoutRow> = ctrl
            .borrow()
            .layout_templates()
            .into_iter()
            .map(|(name, builtin)| LayoutRow {
                name: SharedString::from(name.as_str()),
                builtin,
            })
            .collect();
        ui.set_layout_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
    })
}
