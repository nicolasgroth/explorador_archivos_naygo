// Naygo — cableado de callbacks de la ventana de Configuración (settings, temas, íconos,
// atajos, packs import/export) y del botón que la abre.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers de la ConfigWindow: toggles/combos que persisten, hotkey global (toggle +
// recaptura con reversión si el SO rechaza), idioma/tema en caliente, editor de temas,
// sets de íconos y overrides por objeto, editor de atajos, import/export de packs,
// factory reset, reglas de preview, Acerca de y cierre. Extraídos de `main.rs` sin cambio
// de comportamiento (refactor por tamaño de archivo).

use crate::vm_builders::*;
use crate::wire::WireCtx;
use crate::*;
use slint::{ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

/// Registra todos los callbacks de la ventana de Configuración y el `on_open_config` de la
/// AppWindow (el engranaje de la toolbar que la muestra).
pub(crate) fn wire_config(
    ui: &AppWindow,
    cfg_win: &Rc<ConfigWindow>,
    ctx: &WireCtx,
    refresh_config_vm: &Rc<dyn Fn()>,
    refresh_toolbar_icons: &Rc<dyn Fn()>,
    refresh_drives: &Rc<dyn Fn()>,
    rearm_hotkey: &Rc<dyn Fn() -> Result<(), String>>,
) {
    let crate::wire::WireCtx {
        ctrl, sync_layout, ..
    } = ctx;
    // Acción cuyo atajo se está capturando (la setea cfg-shortcut-capture; la lee cfg-capture-key).
    let capturing_action: Rc<RefCell<Option<naygo_core::keymap::Action>>> =
        Rc::new(RefCell::new(None));
    // Sentinel que distingue la captura del hotkey GLOBAL (no ligado a una `Action` del keymap) de
    // la captura de un atajo normal. La UI (config-window.slint) reusa el MISMO mecanismo de
    // captura (root.capturing + FocusScope + capture-key) para ambos casos: cuando el usuario
    // pulsa "Cambiar" en el control del hotkey global, `shortcut-capture` llega con esta clave en
    // vez de una `action-key` real; `on_capture_key` la reconoce y arma un `Chord` para
    // `set_global_hotkey` en lugar de `rebind`.
    const GLOBAL_HOTKEY_CAPTURE_KEY: &str = "__global_hotkey__";
    let capturing_global_hotkey: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));

    // Toggles/combos/text que solo persisten (no requieren refrescar la vista de paneles).
    // Todos los handlers se registran ahora en la ventana de configuración (`cfg_win`), no en la
    // AppWindow: los callbacks viven en ConfigWindow SIN el prefijo `cfg-`.
    macro_rules! cfg_setter {
        ($on:ident, $arg:ty, $method:ident) => {{
            let ctrl = ctrl.clone();
            let refresh = refresh_config_vm.clone();
            cfg_win.$on(move |v: $arg| {
                ctrl.borrow_mut().config.$method(v);
                refresh();
            });
        }};
    }
    // ops_mode no usa el macro: además de persistir, hay que aplicar el modo al motor de ops
    // (cola/paralelo) en caliente vía sync_ops_mode.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_ops_mode(move |v| {
            {
                let mut c = ctrl.borrow_mut();
                c.config.set_ops_mode(v);
                c.sync_ops_mode();
            }
            refresh();
        });
    }
    // Casilla "sin color" para filas inactivas: mismo preview en vivo que el resto del tema.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_set_use_inactive_row_color(move |v| {
            ctrl.borrow_mut()
                .config
                .set_editing_use_inactive_row_color(v);
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_frequent_dirs_limit_changed(move |v| {
            ctrl.borrow_mut().set_frequent_dirs_limit(v as usize);
            refresh();
        });
    }
    cfg_setter!(on_set_confirm_trash, bool, set_confirm_trash);
    cfg_setter!(on_set_window_title_mode, i32, set_window_title_mode);
    cfg_setter!(
        on_set_confirm_drop_between_panes,
        bool,
        set_confirm_drop_between_panes
    );
    cfg_setter!(on_set_show_op_summary, bool, set_show_op_summary);
    cfg_setter!(on_set_show_parent, bool, set_show_parent);
    cfg_setter!(on_set_icon_only, bool, set_icon_only);
    cfg_setter!(on_set_bar_position, i32, set_bar_position);
    cfg_setter!(on_set_size_no_subdirs, bool, set_size_no_subdirs);
    cfg_setter!(on_set_autostart, bool, set_autostart);
    cfg_setter!(on_set_autostart_minimized, bool, set_autostart_minimized);
    cfg_setter!(on_set_date_format, i32, set_date_format);
    cfg_setter!(on_set_size_format, i32, set_size_format);
    cfg_setter!(on_set_row_density, i32, set_row_density);
    // Avanzado (F3c).
    cfg_setter!(on_set_ops_display, i32, set_ops_display);
    cfg_setter!(on_set_paste_image_fmt, i32, set_paste_image_fmt);
    cfg_setter!(on_set_low_power_mode, i32, set_low_power_mode);
    cfg_setter!(on_set_new_items_at_end, bool, set_new_items_at_end);
    cfg_setter!(on_set_tray_enabled, bool, set_tray_enabled);
    cfg_setter!(on_set_close_to_tray, bool, set_close_to_tray);
    // Hotkey global: activar/desactivar re-arma el registro real en caliente. Si el SO rechaza la
    // combinación al activar (p. ej. ya está tomada por otra app), revertimos el flag a apagado
    // (no queremos dejar "activado" en la UI sin que surta efecto) y avisamos con un toast, igual
    // que el resto de los avisos de Config (import/export de packs).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let rearm = rearm_hotkey.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_set_global_hotkey_enabled(move |on| {
            ctrl.borrow_mut().config.set_global_hotkey_enabled(on);
            if on {
                if let Err(e) = rearm() {
                    ctrl.borrow_mut().config.set_global_hotkey_enabled(false);
                    if let Some(ui) = ui_weak.upgrade() {
                        let tmpl = ctrl.borrow().config.t("slint.cfg.global_hotkey_rejected");
                        ui.invoke_show_toast(tmpl.replace("{err}", &e).into());
                    }
                }
            } else {
                let _ = rearm(); // apagar = soltar el registro; no puede fallar
            }
            refresh();
        });
    }
    cfg_setter!(on_set_paste_confirm, bool, set_paste_confirm);
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_paste_text_name(move |v| {
            ctrl.borrow_mut().config.set_paste_text_name(v.to_string());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_paste_text_ext(move |v| {
            ctrl.borrow_mut().config.set_paste_text_ext(v.to_string());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_paste_image_name(move |v| {
            ctrl.borrow_mut().config.set_paste_image_name(v.to_string());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_paste_jpg_quality(move |v| {
            // El VM entrega un int; lo clampamos a 1–100 antes de guardar como u8
            // (el codificador JPEG no acepta 0: eleva a 1).
            ctrl.borrow_mut()
                .config
                .set_paste_jpg_quality(v.clamp(1, 100) as u8);
            refresh();
        });
    }
    // Límite de carpetas recientes (Avanzado): persiste + trunca la lista al nuevo tope.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_recent_limit_changed(move |v| {
            ctrl.borrow_mut().set_recent_limit(v as usize);
            refresh();
        });
    }
    // Auto-resaltado de código (Previsualización): persiste el toggle.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_auto_highlight_code(move |v| {
            ctrl.borrow_mut().config.set_auto_highlight_code(v);
            refresh();
        });
    }
    // Activar animaciones adicionales (brillo de la barra del panel de ops): persiste el toggle.
    // Además de refrescar la config, se propaga el flag AL INSTANTE al panel de operaciones: si el
    // usuario alterna el toggle con el panel abierto y sin operaciones que disparen un refresco de
    // filas, el brillo cambiaría recién en el próximo refresco; este empujón lo hace inmediato.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_set_animations_enabled(move |v| {
            ctrl.borrow_mut().config.set_animations_enabled(v);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_op_animations_enabled(v);
            }
            refresh();
        });
    }
    // Pie de panel (Avanzado): mostrar/ocultar.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_footer_enabled(move |v| {
            ctrl.borrow_mut().config.set_footer_enabled(v);
            refresh();
        });
    }
    // Pie de panel: plantilla por índice (0..3 fijas, 4=Personalizada).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_footer_preset_index(move |idx| {
            ctrl.borrow_mut().config.set_footer_preset_index(idx);
            refresh();
        });
    }
    // Pie de panel: template personalizado. Refresca para recalcular la vista previa en vivo.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_footer_custom_template(move |t| {
            ctrl.borrow_mut()
                .config
                .set_footer_custom_template(t.to_string());
            refresh();
        });
    }
    // Carpeta de inicio (Home, Avanzado): edición directa del campo.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_set_home_dir(move |dir| {
            ctrl.borrow_mut().config.set_home_dir(dir.to_string());
            refresh();
        });
    }
    // Carpeta de inicio: botón Examinar → diálogo de carpeta nativo; aplica la ruta elegida.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_browse_home(move || {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                ctrl.borrow_mut()
                    .config
                    .set_home_dir(path.to_string_lossy().to_string());
                refresh();
            }
        });
    }
    // Cambio de idioma en caliente: persiste + re-vuelca todos los textos a Tr. Se aplica a
    // AMBAS ventanas (principal y config), porque cada una tiene su propia copia del global Tr.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let sync_layout = sync_layout.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_set_language(move |code| {
            let lang = naygo_core::i18n::LangId::new(&code);
            ctrl.borrow_mut().config.set_language(lang);
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                i18n_keys::apply(&ui, &c.config);
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                i18n_keys::apply(&cfg, &c.config);
            }
            // Actualizar las etiquetas y mensajes de error del preview al nuevo idioma.
            let labels = archive_labels_from_config(&c.config);
            let msgs = preview_msgs_from_config(&c.config);
            drop(c);
            ctrl.borrow_mut().preview.set_archive_labels(labels);
            ctrl.borrow_mut().preview.set_preview_msgs(msgs);
            refresh();
            // Reconstruir las pestañas de los paneles: sus rótulos (Árbol/Operaciones/Vista
            // previa/…) los calcula `pane_label` en Rust, no son claves Tr, así que `refresh()`
            // (que solo toca el VM de Config) no los actualiza. `sync_layout` los re-rotula.
            sync_layout();
        });
    }
    // Cambio de tema en caliente: persiste + re-vuelca los colores a Theme. Se aplica a AMBAS
    // ventanas (principal y config), porque cada una tiene su propia copia del global Theme.
    // También re-tinta los íconos tintables para que adopten el color de texto del nuevo tema.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_set_theme(move |id| {
            ctrl.borrow_mut()
                .config
                .set_theme(naygo_core::theme::ThemeId::new(&id));
            {
                let mut c = ctrl.borrow_mut();
                // Re-tinto íconos tintables con el color de texto del nuevo tema.
                let active = c.config.settings.icon_set.clone();
                let tintable = naygo_core::icon_set::IconSetCatalog::load(&c.config.config_dir)
                    .is_tintable(&active);
                let rgb = theme_text_rgb(&c.config.settings, &c.config.themes);
                c.icons.set_tint(tintable, rgb);
            }
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh_icons();
            refresh();
        });
    }
    // === Editor de temas (config → Apariencia) ===
    // "Personalizar" (builtin) / "Editar" (de usuario): abre el editor y aplica el tema en edición
    // como preview en vivo en AMBAS ventanas. El refresh vuelca el estado del editor a la UI.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_customize(move |id| {
            ctrl.borrow_mut().config.duplicate_theme(&id);
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_edit(move |id| {
            ctrl.borrow_mut().config.edit_user_theme(&id);
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    // Eliminar un tema de usuario: borra el .json, recarga catálogo y, si era el activo, cae al
    // default. Re-aplica el tema activo resultante a ambas ventanas.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_delete(move |id| {
            ctrl.borrow_mut().config.delete_user_theme(&id);
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        });
    }
    // Nombre del tema en edición (no re-aplica preview: el nombre no es un color).
    {
        let ctrl = ctrl.clone();
        cfg_win.on_theme_set_name(move |name| {
            ctrl.borrow_mut().config.set_editing_name(name.to_string());
        });
    }
    // Base oscuro/claro del tema en edición.
    {
        let ctrl = ctrl.clone();
        cfg_win.on_theme_set_base(move |idx| {
            ctrl.borrow_mut().config.set_editing_base(idx);
        });
    }
    // Cambiar un token de color → preview en vivo en ambas ventanas + refresh (para repintar las
    // muestras/hex del editor).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_set_token(move |idx, hex| {
            ctrl.borrow_mut()
                .config
                .set_token_color(idx.max(0) as usize, &hex);
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    // Casilla "paneles inactivos planos" del editor → preview en vivo en ambas ventanas + refresh.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_set_flat_inactive(move |v| {
            ctrl.borrow_mut().config.set_editing_flat_inactive(v);
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    // Guardar el tema en edición: escribe el .json, lo deja activo y re-aplica el tema activo.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_save(move || {
            ctrl.borrow_mut().config.save_editing_theme();
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        });
    }
    // Restaurar de fábrica: resetea los 12 tokens del tema en edición al builtin del que se
    // duplicó y re-aplica el preview.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_restore(move || {
            ctrl.borrow_mut().config.restore_factory_editing();
            let c = ctrl.borrow();
            if let Some(t) = c.config.editing_theme() {
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, t);
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, t);
                }
            }
            drop(c);
            refresh();
        });
    }
    // Cancelar: descarta el tema en edición y re-aplica el tema que estaba activo antes (revierte
    // el preview).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_theme_cancel(move || {
            // Cancelar solo descarta el tema en edición: el tema ACTIVO nunca cambió (el editor solo
            // hacía preview en vivo, no persistía). No volver a llamar `set_theme` (evita una
            // escritura redundante de settings.json en el hilo de UI); el `apply(active_theme())` de
            // abajo revierte el preview al tema activo previo.
            ctrl.borrow_mut().config.cancel_editing();
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        });
    }
    // Cambio de set de íconos en caliente: persiste + apunta el IconCache al set nuevo. Las
    // filas repintan en el próximo tick (sync_rows consulta el cache, que decodifica el set
    // nuevo on-demand). El refresh re-snapshotea el VM de config para reflejar la selección.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        let refresh_drives = refresh_drives.clone();
        cfg_win.on_set_icon_set(move |id| {
            {
                let mut c = ctrl.borrow_mut();
                c.config.set_icon_set(id.to_string());
                // Tomar el id ya coaccionado por el catálogo (un id inválido cayó a "lucide").
                let active = c.config.settings.icon_set.clone();
                c.icons.set_active(active.clone());
                // Re-aplicar overrides y tinte para el set nuevo.
                let tintable = naygo_core::icon_set::IconSetCatalog::load(&c.config.config_dir)
                    .is_tintable(&active);
                let overrides = c.config.settings.icon_overrides.clone();
                let rgb = theme_text_rgb(&c.config.settings, &c.config.themes);
                c.icons.set_overrides(overrides);
                c.icons.set_tint(tintable, rgb);
            }
            // Repintar los íconos de la toolbar y de la tira de discos con el set nuevo.
            refresh_icons();
            refresh_drives();
            refresh();
        });
    }
    // Personalizar set de íconos: activa el set en caliente Y salta a la pestaña Íconos (cat 9).
    // Mismo flujo que on_set_icon_set, más el jump de categoría en la ventana de config.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        let refresh_drives = refresh_drives.clone();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_personalize_icon_set(move |id| {
            {
                let mut c = ctrl.borrow_mut();
                c.config.set_icon_set(id.to_string());
                let active = c.config.settings.icon_set.clone();
                c.icons.set_active(active.clone());
                let tintable = naygo_core::icon_set::IconSetCatalog::load(&c.config.config_dir)
                    .is_tintable(&active);
                let overrides = c.config.settings.icon_overrides.clone();
                let rgb = theme_text_rgb(&c.config.settings, &c.config.themes);
                c.icons.set_overrides(overrides);
                c.icons.set_tint(tintable, rgb);
            }
            // Saltar a la pestaña Íconos (cat 9) en la ventana de configuración.
            if let Some(cfg) = cfg_weak.upgrade() {
                cfg.set_cat(9);
            }
            refresh_icons();
            refresh_drives();
            refresh();
        });
    }
    // --- Íconos por objeto (Task 15): picker, overrides, PNG propio, import, export ---
    // Abrir el selector de ícono: construye las opciones (un tile por set de fábrica) y
    // las inyecta en el SettingsVm antes de llamar cfg.set_vm (ver nota en refresh_config_vm).
    {
        let ctrl = ctrl.clone();
        let cfg_weak = cfg_win.as_weak();
        let ui_weak = ui.as_weak();
        cfg_win.on_open_icon_picker(move |key| {
            let Some(cfg) = cfg_weak.upgrade() else {
                return;
            };
            // Construir las opciones con borrow_mut (icons.get necesita &mut).
            let choices: Vec<IconChoiceVm> = {
                let c = ctrl.borrow();
                let Some(icon_key) = naygo_core::icon_source::key_from_string(key.as_str()) else {
                    return;
                };
                let config_dir = c.config.config_dir.clone();
                let tint = theme_text_rgb(&c.config.settings, &c.config.themes);
                let catalog = naygo_core::icon_set::IconSetCatalog::load(&config_dir);
                drop(c);
                catalog
                    .available()
                    .iter()
                    .map(|info| {
                        let tintable = info.tintable;
                        let icon_img =
                            icons::render_for_set(icon_key, &info.id, tintable, tint, &config_dir);
                        IconChoiceVm {
                            set_id: info.id.as_str().into(),
                            set_label: info.label.as_str().into(),
                            icon: icon_img,
                        }
                    })
                    .collect()
            };
            // Construir el SettingsVm completo y sobreescribir los campos del picker.
            // Esto abre el overlay en la UI al setear icon_picker_key != "".
            let mut vm = {
                let c = ctrl.borrow();
                build_settings_vm(&c.config)
            };
            vm.icon_picker_key = key;
            vm.icon_picker_choices = ModelRc::from(Rc::new(VecModel::from(choices)));
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_settings_vm(vm.clone());
            }
            cfg.set_vm(vm);
        });
    }
    // Cerrar el selector: basta con hacer refresh (build_settings_vm devuelve icon_picker_key=""
    // por defecto, lo que cierra el overlay).
    {
        let refresh = refresh_config_vm.clone();
        cfg_win.on_close_icon_picker(move || {
            refresh();
        });
    }
    // Override a un set de íconos (Builtin): persiste + re-aplica cache + refresca.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        let refresh_drives = refresh_drives.clone();
        cfg_win.on_set_icon_override(move |key, set_id| {
            {
                let mut c = ctrl.borrow_mut();
                c.config.settings.icon_overrides.insert(
                    key.to_string(),
                    naygo_core::icon_source::IconSource::Builtin {
                        set_id: set_id.to_string(),
                    },
                );
                c.config.save();
                let overrides = c.config.settings.icon_overrides.clone();
                c.icons.set_overrides(overrides);
            }
            refresh_icons();
            refresh_drives();
            refresh();
        });
    }
    // Override a un PNG propio: abre selector de archivo, copia el PNG al directorio de usuario
    // y registra el override de tipo UserPng.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        let refresh_drives = refresh_drives.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_set_icon_override_png(move |key| {
            let Some(src_path) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .pick_file()
            else {
                return;
            };
            let config_dir = ctrl.borrow().config.config_dir.clone();
            let Some(rel_path) = copy_user_png(&config_dir, &src_path) else {
                // La copia falló: avisar con un toast (eprintln solo no se ve en GUI).
                if let Some(ui) = ui_weak.upgrade() {
                    let msg = ctrl.borrow().config.t("settings.icons.png_copy_err");
                    ui.invoke_show_toast(msg.into());
                }
                return;
            };
            {
                let mut c = ctrl.borrow_mut();
                c.config.settings.icon_overrides.insert(
                    key.to_string(),
                    naygo_core::icon_source::IconSource::UserPng { rel_path },
                );
                c.config.save();
                let overrides = c.config.settings.icon_overrides.clone();
                c.icons.set_overrides(overrides);
            }
            refresh_icons();
            refresh_drives();
            refresh();
        });
    }
    // Quitar el override de un ícono concreto: vuelve al set base.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        let refresh_drives = refresh_drives.clone();
        cfg_win.on_clear_icon_override(move |key| {
            {
                let mut c = ctrl.borrow_mut();
                c.config.settings.icon_overrides.remove(key.as_str());
                c.config.save();
                let overrides = c.config.settings.icon_overrides.clone();
                c.icons.set_overrides(overrides);
            }
            refresh_icons();
            refresh_drives();
            refresh();
        });
    }
    // Resetear todos los overrides: vuelve al set base para todos los íconos.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        let refresh_drives = refresh_drives.clone();
        cfg_win.on_reset_icon_overrides(move || {
            {
                let mut c = ctrl.borrow_mut();
                c.config.settings.icon_overrides.clear();
                c.config.save();
                c.icons.set_overrides(std::collections::BTreeMap::new());
            }
            refresh_icons();
            refresh_drives();
            refresh();
        });
    }
    // Importar un .naygoset: activa el set importado y aplica sus overrides.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        let refresh_drives = refresh_drives.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_import_icon_set(move || {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Set de íconos (.naygoset)", &["naygoset"])
                .pick_file()
            else {
                return;
            };
            let config_dir = ctrl.borrow().config.config_dir.clone();
            match naygo_core::icon_pack::import_pack(&path, &config_dir) {
                Ok(manifest) => {
                    {
                        let mut c = ctrl.borrow_mut();
                        // Activar el set recién importado y aplicar sus overrides.
                        let set_id = manifest.name.clone();
                        c.config.set_icon_set(set_id.clone());
                        let active = c.config.settings.icon_set.clone();
                        c.config.settings.icon_overrides.clear();
                        for entry in &manifest.overrides {
                            c.config
                                .settings
                                .icon_overrides
                                .insert(entry.key.clone(), entry.source.clone());
                        }
                        c.config.save();
                        c.icons.set_active(active.clone());
                        let catalog = naygo_core::icon_set::IconSetCatalog::load(&config_dir);
                        let tintable = catalog.is_tintable(&active);
                        let overrides = c.config.settings.icon_overrides.clone();
                        let rgb = theme_text_rgb(&c.config.settings, &c.config.themes);
                        c.icons.set_overrides(overrides);
                        c.icons.set_tint(tintable, rgb);
                    }
                    refresh_icons();
                    refresh_drives();
                    refresh();
                    if let Some(ui) = ui_weak.upgrade() {
                        let msg = ctrl.borrow().config.t("settings.icons.import_ok").into();
                        ui.invoke_show_toast(msg);
                    }
                }
                Err(e) => {
                    if let Some(ui) = ui_weak.upgrade() {
                        let base = ctrl.borrow().config.t("settings.icons.import_err");
                        ui.invoke_show_toast(format!("{base}: {e}").into());
                    }
                }
            }
        });
    }
    // Exportar el set efectivo actual a un .naygoset.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_export_icon_set(move || {
            let (set_id, overrides, config_dir) = {
                let c = ctrl.borrow();
                (
                    c.config.settings.icon_set.clone(),
                    c.config.settings.icon_overrides.clone(),
                    c.config.config_dir.clone(),
                )
            };
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Set de íconos (.naygoset)", &["naygoset"])
                .set_file_name(format!("{set_id}.naygoset"))
                .save_file()
            else {
                return;
            };
            match naygo_core::icon_pack::export_pack(
                &path,
                &set_id,
                "",
                &set_id,
                &overrides,
                &config_dir,
            ) {
                Ok(()) => {
                    if let Some(ui) = ui_weak.upgrade() {
                        let msg = ctrl.borrow().config.t("settings.icons.export_ok").into();
                        ui.invoke_show_toast(msg);
                    }
                }
                Err(e) => {
                    // Mismo patrón que import: toast (no modal) para ok y err.
                    if let Some(ui) = ui_weak.upgrade() {
                        let base = ctrl.borrow().config.t("settings.icons.export_err");
                        ui.invoke_show_toast(format!("{base}: {e}").into());
                    }
                }
            }
        });
    }
    // Editor de atajos: capturar la acción a reasignar (o el hotkey global, ver sentinel arriba).
    {
        let capturing = capturing_action.clone();
        let capturing_hotkey = capturing_global_hotkey.clone();
        cfg_win.on_shortcut_capture(move |key| {
            if key == GLOBAL_HOTKEY_CAPTURE_KEY {
                capturing_hotkey.set(true);
                *capturing.borrow_mut() = None;
            } else {
                capturing_hotkey.set(false);
                *capturing.borrow_mut() = config_ctrl::ConfigCtrl::action_from_key(&key);
            }
        });
    }
    // Captura de la combinación: si hay acción en captura y el chord es válido, reasigna. Si en
    // cambio se estaba capturando el HOTKEY GLOBAL, arma el Chord y lo valida/persiste vía
    // `set_global_hotkey`, re-arma el registro en caliente, y si el SO lo rechaza (o el chord no
    // tiene modificador) avisa con un toast y NO toca el hotkey anterior (sigue vigente).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let capturing = capturing_action.clone();
        let capturing_hotkey = capturing_global_hotkey.clone();
        let rearm = rearm_hotkey.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_capture_key(move |text, c, s, a| {
            if capturing_hotkey.take() {
                // Esc cancela la captura sin reasignar (la UI ya salió del modo captura).
                if text == keys::escape_char().to_string() {
                    return;
                }
                let Some(chord) = keys::chord_from(&text, c, s, a) else {
                    return;
                };
                // Guardar la combinación ANTERIOR: si el SO rechaza la nueva al re-armar, se
                // restaura (best-effort) para no dejar al usuario con un hotkey "activado pero sin
                // funcionar" mostrando una combinación muerta. Coherente con la ruta del toggle,
                // que también revierte a un estado limpio.
                let prev_chord = ctrl.borrow().config.settings.global_hotkey;
                let set_result = ctrl.borrow_mut().config.set_global_hotkey(chord);
                match set_result {
                    Ok(()) => {
                        if let Err(e) = rearm() {
                            // El SO rechazó la combinación nueva: revertir a la anterior y re-armar
                            // (que funcionaba), de modo que el usuario conserva su hotkey vigente.
                            let _ = ctrl.borrow_mut().config.set_global_hotkey(prev_chord);
                            let _ = rearm();
                            if let Some(ui) = ui_weak.upgrade() {
                                let tmpl =
                                    ctrl.borrow().config.t("slint.cfg.global_hotkey_rejected");
                                ui.invoke_show_toast(tmpl.replace("{err}", &e).into());
                            }
                        }
                        refresh();
                    }
                    Err(e) => {
                        if let Some(ui) = ui_weak.upgrade() {
                            let tmpl = ctrl.borrow().config.t("slint.cfg.global_hotkey_invalid");
                            ui.invoke_show_toast(tmpl.replace("{err}", &e).into());
                        }
                    }
                }
                return;
            }
            let action = match capturing.borrow_mut().take() {
                Some(act) => act,
                None => return,
            };
            // Esc cancela la captura sin reasignar (la UI ya salió del modo captura).
            if text == keys::escape_char().to_string() {
                return;
            }
            if let Some(chord) = keys::chord_from(&text, c, s, a) {
                ctrl.borrow_mut().config.rebind(action, chord);
                refresh();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_shortcut_reset(move |key| {
            if let Some(action) = config_ctrl::ConfigCtrl::action_from_key(&key) {
                ctrl.borrow_mut().config.reset_shortcut(action);
                refresh();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_shortcuts_reset_all(move || {
            ctrl.borrow_mut().config.reset_all_shortcuts();
            refresh();
        });
    }
    // Import/Export de packs (.zip) — Fase 4E. Selector de archivo nativo (rfd); el resultado
    // se informa con un MessageDialog (errores) sin bloquear el resto de la UI.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_export_language(move || {
            let c = ctrl.borrow();
            let code = c.config.settings.language.as_str().to_string();
            // El archivo es un .zip por dentro; solo cambia la extensión visible a .naygolang.
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Idioma Naygo", &["naygolang"])
                .set_file_name(format!("{code}.naygolang"))
                .save_file()
            {
                report(
                    &ui_weak,
                    packs::export_lang(&c.config.config_dir, &code, &path),
                );
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_export_theme(move || {
            let c = ctrl.borrow();
            let id = c.config.settings.theme.as_str().to_string();
            // El archivo es un .zip por dentro; solo cambia la extensión visible a .naygotheme.
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Tema Naygo", &["naygotheme"])
                .set_file_name(format!("{id}.naygotheme"))
                .save_file()
            {
                report(
                    &ui_weak,
                    packs::export_theme(&c.config.config_dir, &id, &path),
                );
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        cfg_win.on_export_config(move || {
            let c = ctrl.borrow();
            // El archivo es un .zip por dentro; solo cambia la extensión visible a .naygoconf.
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Configuración Naygo", &["naygoconf"])
                .set_file_name("config.naygoconf")
                .save_file()
            {
                report(&ui_weak, packs::export_config(&c.config.config_dir, &path));
            }
        });
    }
    // Import por tópico. Cada handler filtra SU extensión en el selector (.naygolang /
    // .naygotheme / .naygoconf), pero el backend `import_zip` detecta el tipo por el CONTENIDO
    // del zip: si el usuario fuerza otro archivo, manda el contenido (es la autoridad). La
    // recarga del catálogo se hace según el `ImportKind` que devuelve, igual para los tres.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_import_language(move || {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Idioma Naygo", &["naygolang"])
                .pick_file()
            else {
                return;
            };
            import_topic(&ctrl, &ui_weak, &cfg_weak, &refresh, &path);
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_import_theme(move || {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Tema Naygo", &["naygotheme"])
                .pick_file()
            else {
                return;
            };
            import_topic(&ctrl, &ui_weak, &cfg_weak, &refresh, &path);
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_import_config(move || {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Configuración Naygo", &["naygoconf"])
                .pick_file()
            else {
                return;
            };
            import_topic(&ctrl, &ui_weak, &cfg_weak, &refresh, &path);
        });
    }
    // Avanzado: factory reset. Restablece TODOS los ajustes y reaplica idioma/tema (cambian).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_factory_reset(move || {
            ctrl.borrow_mut().config.factory_reset();
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                i18n_keys::apply(&ui, &c.config);
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                i18n_keys::apply(&cfg, &c.config);
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        });
    }
    // C4: guardar la tabla del panel activo como plantilla por defecto.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_save_default_table(move || {
            ctrl.borrow_mut().save_default_table_from_active();
            refresh();
        });
    }
    // C4: limpiar la plantilla de tabla por defecto.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_clear_default_table(move || {
            ctrl.borrow_mut().clear_default_table();
            refresh();
        });
    }
    // C3: reglas de previsualización (toggle / tratar-como / quitar / agregar).
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_toggle(move |ext| {
            ctrl.borrow_mut().preview_rule_toggle(ext.as_str());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_set_view_mode(move |ext, idx| {
            ctrl.borrow_mut()
                .preview_rule_set_view_mode(ext.as_str(), idx);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_set_view_lang(move |ext, idx| {
            ctrl.borrow_mut()
                .preview_rule_set_view_lang(ext.as_str(), idx);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_remove(move |ext| {
            ctrl.borrow_mut().preview_rule_remove(ext.as_str());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        cfg_win.on_preview_add(move |ext| {
            ctrl.borrow_mut().preview_rule_add(ext.as_str());
            refresh();
        });
    }
    // Acerca de: abrir el repositorio en el navegador por defecto.
    {
        cfg_win.on_open_repo(move || {
            let _ = naygo_platform::open::open_default(std::path::Path::new(
                "https://github.com/nicolasgroth/explorador_archivos_naygo",
            ));
        });
    }
    // Acerca de (easter egg): primeros `n` caracteres del mensaje (substring que Slint no ofrece).
    // Lee el global Tr de la PROPIA ventana de config (donde se muestra el easter egg).
    {
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_egg_prefix(move |n| {
            let Some(cfg) = cfg_weak.upgrade() else {
                return SharedString::new();
            };
            let msg = cfg.global::<Tr>().get_about_egg_message();
            let take = n.max(0) as usize;
            SharedString::from(msg.chars().take(take).collect::<String>())
        });
    }
    // Cerrar la ventana de config (botón "cerrar" interno): la oculta (no destruye la instancia).
    {
        let cfg_weak = cfg_win.as_weak();
        cfg_win.on_close(move || {
            if let Some(cfg) = cfg_weak.upgrade() {
                let _ = cfg.hide();
            }
        });
    }
    // Abrir la ventana de config desde el engranaje de la toolbar: refresca el VM (para que abra
    // poblada) y la muestra.
    {
        let refresh = refresh_config_vm.clone();
        let cfg_weak = cfg_win.as_weak();
        let ctrl = ctrl.clone();
        ui.on_open_config(move || {
            crate::logging::breadcrumb("abrir configuración");
            refresh();
            // La ventana de config es otra ventana y roba el foco: el `key-released` no vuelve al
            // panel. Limpiamos los modificadores para no dejar Ctrl/Shift pegados al volver.
            ctrl.borrow_mut().clear_modifiers();
            if let Some(cfg) = cfg_weak.upgrade() {
                let _ = cfg.show();
            }
        });
    }
}
