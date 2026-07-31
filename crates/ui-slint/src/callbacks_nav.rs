// Naygo — cableado de callbacks de navegación y teclado global del panel.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers de navegación: activar panel, atrás/adelante con botones del mouse, el `on_key`
// global del panel (atajos, paleta, editor de ruta, rename F2, menús de toolbar), subir /
// atrás / adelante / home y agregar paneles. Extraídos de `main.rs` sin cambio de
// comportamiento (refactor por tamaño de archivo).

use crate::vm_builders::*;
use crate::wire::WireCtx;
use crate::*;
use naygo_core::workspace::layout::SplitDir;
use naygo_core::workspace::PaneId;
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// Registra los callbacks de teclado global del panel y activación/navegación por mouse.
pub(crate) fn wire_nav_keys(ui: &AppWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_rows,
        sync_layout,
        start_timer,
        palette_cmds,
        palette_cmd_indices,
        ..
    } = ctx;
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_activate(move |id| {
            ctrl.borrow_mut().set_active(PaneId(id as u64));
            start_timer();
            sync_rows();
        });
    }
    // Limpiar el filtro visual por tipeo (clic en la ✕ de la mini-barra del panel).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_filter_clear(move |_id| {
            if ctrl.borrow_mut().clear_filter() {
                start_timer();
                sync_rows();
            }
        });
    }
    // Botones laterales del mouse: atrás/adelante en el panel donde se hizo clic.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_nav_back(move |id| {
            // El borrow del controlador se LIBERA antes de start_timer()/sync_rows(): esos
            // closures vuelven a tomar `ctrl`, y dejar `c` vivo causaba un doble-borrow del
            // RefCell (panic → cierre abrupto) cuando on_go_back devolvía false (sin historial).
            let moved = {
                let mut c = ctrl.borrow_mut();
                c.set_active(PaneId(id as u64));
                c.on_go_back()
            };
            if moved {
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_nav_forward(move |id| {
            // Igual que nav_back: liberar el borrow antes de start_timer()/sync_rows().
            let moved = {
                let mut c = ctrl.borrow_mut();
                c.set_active(PaneId(id as u64));
                c.on_go_forward()
            };
            if moved {
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let ui_weak = ui.as_weak();
        let palette_cmds = palette_cmds.clone();
        let palette_cmd_indices = palette_cmd_indices.clone();
        ui.on_key(move |text, c, s, a| {
            // Con la paleta de comandos abierta, su propio overlay (FocusScope del LineEdit) maneja
            // el teclado. Suspendemos el on_key global del panel para que las teclas (Enter/letras/
            // flechas) no disparen acciones por debajo. Mismo criterio que con un modal abierto.
            if let Some(ui) = ui_weak.upgrade() {
                if ui.get_palette_open() {
                    return;
                }
            }
            if ctrl.borrow_mut().on_key(text.as_str(), c, s, a) {
                start_timer();
            }
            // ¿La tecla pidió abrir la paleta de comandos (Ctrl+P)? Construir los comandos vigentes,
            // mostrar todos (query vacía) y abrir el overlay. El LineEdit toma el foco en `init`.
            if ctrl.borrow_mut().take_open_palette_request() {
                if let Some(ui) = ui_weak.upgrade() {
                    let cmds = ctrl.borrow().build_palette_commands();
                    let matches = naygo_core::palette::filter_and_rank(&cmds, "");
                    let (items, idxs) = palette_items_from_matches(&cmds, &matches);
                    *palette_cmds.borrow_mut() = cmds;
                    *palette_cmd_indices.borrow_mut() = idxs;
                    ui.set_palette_results(ModelRc::from(Rc::new(VecModel::from(items))));
                    ui.set_palette_query(SharedString::new());
                    ui.set_palette_selected(0);
                    ui.set_palette_open(true);
                    // El overlay de la paleta roba el foco al panel: su `key-released` no llega aquí.
                    // Limpiamos los modificadores para no dejar Ctrl pegado tras Ctrl+P al cerrar.
                    ctrl.borrow_mut().clear_modifiers();
                }
            }
            // Atajo "editar ruta" (Ctrl+L / F4): abrir el editor de la path-bar del panel pedido.
            let edit_path_request = ctrl.borrow_mut().take_edit_path_request();
            if let Some(pane) = edit_path_request {
                if let Some(ui) = ui_weak.upgrade() {
                    let path = ctrl.borrow().path_of(pane);
                    let sugg = ctrl.borrow().path_autocomplete(&path);
                    ui.set_edit_pane(pane.0 as i32);
                    ui.set_edit_text(path.into());
                    ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                        sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
                    ))));
                }
            }
            // Rename inline (F2): abrir el editor en la celda Name de la fila pedida.
            let rename_request = ctrl.borrow_mut().take_rename_request();
            if let Some(request) = rename_request {
                if let Some(ui) = ui_weak.upgrade() {
                    show_rename_editor(&ui, &request);
                }
            }
            // Atajo "abrir configuración" (Ctrl+Shift+O): reusa el handler del engranaje de la toolbar.
            if ctrl.borrow_mut().take_open_config_request() {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_open_config();
                }
            }
            // Atajos que abren menús/acciones de la toolbar cuyos props viven en la AppWindow
            // (Favoritos / Disposiciones / Refrescar unidades). El controlador no puede tocarlos, así
            // que dejó una petición; acá la aplicamos. Los menús se anclan a un x razonable (la zona
            // de la toolbar donde vive cada botón); no necesitan la posición exacta para funcionar.
            // El `borrow_mut` se suelta ANTES del `if let`: el temporal del scrutinee viviría
            // hasta el fin del bloque, donde `clear_modifiers` vuelve a tomar `ctrl` → panic
            // (mismo bug que el crash de F2 con `take_rename_request`).
            let toolbar_menu_request = ctrl.borrow_mut().take_toolbar_menu_request();
            if let Some(req) = toolbar_menu_request {
                if let Some(ui) = ui_weak.upgrade() {
                    use workspace_ctrl::ToolbarMenuRequest as Tmr;
                    // No tener dos menús flotantes abiertos a la vez.
                    ui.set_history_menu_open(false);
                    ui.set_back_history_menu_open(false);
                    ui.set_fwd_history_menu_open(false);
                    ui.set_drive_menu_path("".into());
                    ui.set_view_menu_open(false);
                    // x de anclaje del menú flotante abierto por teclado. Aunque no hay botón al que
                    // anclarse en el gesto (vino de un atajo), la AppWindow expone la posición REAL de
                    // cada botón (fav-btn-x / layouts-btn-x), siempre al día. Las leemos para que el
                    // menú salga justo bajo su botón, igual que al abrirlo con el mouse.
                    match req {
                        Tmr::RefreshDrives => ui.invoke_refresh_drives(),
                        Tmr::Favorites => {
                            ui.set_layout_menu_open(false);
                            ui.set_fav_menu_x(ui.get_fav_btn_x());
                            ui.set_fav_menu_open(true);
                        }
                        Tmr::Layouts => {
                            ui.set_fav_menu_open(false);
                            ui.set_layout_menu_x(ui.get_layouts_btn_x());
                            ui.set_layout_menu_open(true);
                        }
                    }
                    // El menú flotante puede robar el foco: limpiar modificadores para no dejar Ctrl
                    // pegado tras el atajo.
                    ctrl.borrow_mut().clear_modifiers();
                }
            }
            start_timer();
            sync_layout();
        });
    }
    // Soltado de tecla: resetea `ctrl_down`/`shift_down` en el controlador con los modificadores
    // vigentes. Sin esto, tras un Ctrl+C el flag quedaba pegado en `true` y el siguiente doble-clic
    // en carpeta abría-en-otro-panel en vez de navegar. No refresca UI ni timers: sólo estado.
    {
        let ctrl = ctrl.clone();
        ui.on_key_release(move |c, s, a| {
            ctrl.borrow_mut().on_key_release(c, s, a);
        });
    }
}

/// Registra los callbacks de subir/atrás/adelante/home y de agregar paneles (split).
pub(crate) fn wire_nav_go(ui: &AppWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_layout,
        start_timer,
        area_of,
        ..
    } = ctx;
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_up(move || {
            if ctrl.borrow_mut().on_go_up() {
                start_timer();
            }
            sync_layout();
        });
    }
    // Navegación tipo navegador: Atrás / Adelante / Inicio. Cada uno relanza el listado si se
    // movió (start_timer) y repinta (sync_layout, que además actualiza can-go-back/forward).
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_back(move || {
            if ctrl.borrow_mut().on_go_back() {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_forward(move || {
            if ctrl.borrow_mut().on_go_forward() {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_home(move || {
            if ctrl.borrow_mut().on_go_home() {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let area_of = area_of.clone();
        ui.on_add_pane(move || {
            ctrl.borrow_mut().add_pane_split(area_of());
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let area_of = area_of.clone();
        ui.on_add_pane_of(move |purpose| {
            ctrl.borrow_mut()
                .add_pane_of(int_to_purpose(purpose), area_of());
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_add_pane_dir(move |dir| {
            // 0=derecha 1=abajo 2=izquierda 3=arriba.
            let (split, first) = match dir {
                1 => (SplitDir::Vertical, false),
                2 => (SplitDir::Horizontal, true),
                3 => (SplitDir::Vertical, true),
                _ => (SplitDir::Horizontal, false),
            };
            ctrl.borrow_mut().add_pane_split_dir(split, first);
            start_timer();
            sync_layout();
        });
    }
}
