// Naygo — cableado de callbacks del menú contextual (propio + nativo del Shell), modal de
// carpetas nuevas, aviso "carpeta no encontrada", búsqueda y acciones de toolbar.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers de: clic derecho en fila/zona vacía, todas las acciones del menú contextual
// (abrir/copiar/cortar/pegar/renombrar/borrar/comprimir/extraer/terminal/nativo), nueva(s)
// carpeta(s), opciones del aviso in-place de carpeta perdida, búsqueda recursiva (Ctrl+F) y
// botones de toolbar (nueva carpeta, terminales). Extraídos de `main.rs` sin cambio de
// comportamiento (refactor por tamaño de archivo).

use crate::win_helpers::*;
use crate::wire::WireCtx;
use crate::*;
use naygo_core::workspace::PaneId;

/// Registra los callbacks del menú contextual, carpetas nuevas, missing, búsqueda y toolbar.
pub(crate) fn wire_ctx_menu(ui: &AppWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_rows,
        sync_layout,
        start_timer,
        area_of,
        ..
    } = ctx;
    // --- Menú contextual (clic derecho): acciones propias + nativo ---
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_row_context(move |id, _pos, x, y| {
            ctrl.borrow_mut().open_context_menu(PaneId(id as u64), x, y);
            sync_rows();
            // El menú contextual necesita que el event loop siga vivo para resaltar sus ítems al
            // pasar el mouse: rearmar el timer al abrirlo (igual criterio que con los modales).
            start_timer();
        });
    }
    // Clic derecho en la zona vacía del panel → menú contextual de la carpeta (modo carpeta).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_empty_context(move |id, x, y| {
            ctrl.borrow_mut()
                .open_folder_context_menu(PaneId(id as u64), x, y);
            sync_rows();
            start_timer();
        });
    }
    // Modo carpeta: abrir el Explorador de Windows en la carpeta.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_explorer(move || {
            ctrl.borrow_mut().ctx_open_explorer();
            sync_rows();
        });
    }
    // Modo carpeta: abrir el modal "nueva(s) carpeta(s)".
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_ctx_new_folder(move || {
            ctrl.borrow_mut().ctx_new_folder();
            sync_rows();
            // Abrir un modal desde el mouse: rearmar el timer para que responda al instante.
            start_timer();
        });
    }
    // Modal "nueva(s) carpeta(s)": editar texto / crear / cancelar.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_new_folder_set_text(move |t| {
            ctrl.borrow_mut().new_folder_set_text(&t);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_new_folder_create(move || {
            ctrl.borrow_mut().new_folder_apply();
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_new_folder_close(move || {
            ctrl.borrow_mut().new_folder_close();
            sync_rows();
        });
    }
    // "Carpeta no encontrada" IN-PLACE por panel: reintentar / subir / elegir / cerrar. Cada
    // handler recibe el pane-id del panel que muestra el aviso.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_missing_retry(move |id| {
            ctrl.borrow_mut().missing_folder_retry(PaneId(id as u64));
            start_timer();
            // Reconstruir el PaneVm con el estado actual de `missing`: el aviso se refresca al
            // instante (antes solo se actualizaba al redimensionar, porque el listado async ya
            // había apagado el timer cuando corría el siguiente sync_rows).
            sync_rows();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_missing_ancestor(move |id| {
            ctrl.borrow_mut()
                .missing_folder_go_ancestor(PaneId(id as u64));
            start_timer();
            sync_rows();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_missing_choose(move |id| {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                ctrl.borrow_mut()
                    .missing_folder_choose(PaneId(id as u64), path);
                start_timer();
            }
            sync_rows();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_missing_close_pane(move |id| {
            ctrl.borrow_mut()
                .missing_folder_close_pane(PaneId(id as u64));
            start_timer();
            sync_rows();
            sync_layout();
        });
    }
    // Búsqueda recursiva (Ctrl+F / lupa): lanzar / cerrar / detener / abrir resultado / alternar.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_search_run(move |q| {
            ctrl.borrow_mut().start_search(q.to_string());
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_search_close(move || {
            ctrl.borrow_mut().close_search();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_search_cancel(move || {
            ctrl.borrow_mut().cancel_search();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_search_open(move |i| {
            ctrl.borrow_mut().open_search_hit(i.max(0) as usize);
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_search_toggle(move || {
            // La lupa alterna: si hay panel abierto, lo cierra; si no, lo abre vacío (sin lanzar
            // todavía — el usuario escribe y pulsa Enter/Buscar). Abrir = sembrar un job inactivo
            // mostrando el panel; lo modelamos arrancando una búsqueda con query vacía no sirve
            // (no abre), así que abrimos con un marcador: reusamos open_empty_search.
            let open = ctrl.borrow().search_open();
            if open {
                ctrl.borrow_mut().close_search();
            } else {
                ctrl.borrow_mut().open_empty_search();
            }
            sync_rows();
        });
    }
    // Toolbar: nueva carpeta en el panel activo (abre el modal).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_new_folder_toolbar(move || {
            ctrl.borrow_mut().new_folder_open_active();
            sync_rows();
            // Abrir un modal desde el mouse: rearmar el timer para que responda al instante.
            start_timer();
        });
    }
    // Toolbar: combo de terminales en el panel activo (0=PS,1=CMD,2=WT,3=WSL).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_toolbar_terminal(move |term| {
            ctrl.borrow_mut().terminal_active(term);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_dismiss(move || {
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_open(move || {
            ctrl.borrow_mut().ctx_open();
            sync_rows();
        });
    }
    // Submenú "Abrir ▸" (solo target carpeta): Abrir aquí / en otro panel / en panel nuevo.
    // "Abrir en el Explorador de Windows" reusa `ctx_open_explorer` (ya cableado arriba).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_ctx_open_here(move || {
            if ctrl.borrow_mut().ctx_open_here() {
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let area_of = area_of.clone();
        ui.on_ctx_open_other_pane(move || {
            let area = area_of();
            let acted = ctrl.borrow_mut().ctx_open_other_pane(area);
            ctrl.borrow_mut().close_context_menu();
            if acted {
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let area_of = area_of.clone();
        ui.on_ctx_open_new_pane(move || {
            let area = area_of();
            let acted = ctrl.borrow_mut().ctx_open_new_pane(area);
            ctrl.borrow_mut().close_context_menu();
            if acted {
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_open_with(move || {
            ctrl.borrow_mut().ctx_open_with();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_copy(move || {
            ctrl.borrow_mut().op_copy();
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_cut(move || {
            ctrl.borrow_mut().op_cut();
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_ctx_paste(move || {
            if ctrl.borrow_mut().op_paste() {
                start_timer();
            }
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let ui_weak = ui.as_weak();
        ui.on_ctx_rename(move || {
            ctrl.borrow_mut().op_rename();
            ctrl.borrow_mut().close_context_menu();
            // Abrir el editor inline en la fila pedida (igual que el camino de F2).
            let rename_request = ctrl.borrow_mut().take_rename_request();
            if let Some(request) = rename_request {
                if let Some(ui) = ui_weak.upgrade() {
                    show_rename_editor(&ui, &request);
                }
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_ctx_delete(move || {
            ctrl.borrow_mut().op_delete(false);
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
            // Abrir el modal de confirmar borrado desde el MOUSE: rearmar el timer para que el
            // popup responda al instante (hover + primer clic), sin esperar un clic de despertar.
            // El camino de teclado (`on_key`) ya rearma; este es su equivalente para el menú.
            start_timer();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_copy_path(move || {
            ctrl.borrow_mut().ctx_copy_path();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_copy_names(move || {
            ctrl.borrow_mut().ctx_copy_names();
            sync_rows();
        });
    }
    // Comprimir: abre el modal de nombre del .zip (su confirmación arranca la op vía
    // `on_name_confirm`). Solo cierra el menú y abre el modal aquí.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_ctx_compress(move || {
            ctrl.borrow_mut().op_compress_prompt();
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
            // Rearmar el timer para que el modal de nombre responda al instante (igual que
            // el camino de borrar desde el menú).
            start_timer();
        });
    }
    // Extraer aquí: subcarpeta con el nombre del zip dentro de la carpeta actual.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_ctx_extract_here(move || {
            ctrl.borrow_mut().op_extract_here();
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
            start_timer();
        });
    }
    // Extraer en…: elegir la carpeta destino con el diálogo nativo de carpeta.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_ctx_extract_to(move || {
            // Cerrar el menú antes de abrir el diálogo nativo (bloqueante).
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
            if let Some(dest) = rfd::FileDialog::new().pick_folder() {
                ctrl.borrow_mut().op_extract_to(dest);
                start_timer();
            }
        });
    }
    // Abrir terminal en la carpeta: 0=PowerShell, 1=CMD, 2=Windows Terminal.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_terminal_ps(move || {
            ctrl.borrow_mut().ctx_open_terminal(0);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_terminal_cmd(move || {
            ctrl.borrow_mut().ctx_open_terminal(1);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_terminal_wt(move || {
            ctrl.borrow_mut().ctx_open_terminal(2);
            sync_rows();
        });
    }
    {
        // "Más opciones de Windows…": invoca el menú nativo del Shell con el HWND de winit.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_native(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let (targets, x, y) = {
                let c = ctrl.borrow();
                match &c.context_menu {
                    Some(cm) => (cm.targets.clone(), cm.x, cm.y),
                    None => return,
                }
            };
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
            if let Some(hwnd) = naygo_hwnd(&ui) {
                // Coords de pantalla = posición de la ventana + posición del clic en la ventana.
                let pos = ui.window().position();
                let sx = pos.x + x as i32;
                let sy = pos.y + y as i32;
                let _ =
                    naygo_platform::context_menu::show_native_context_menu(hwnd, &targets, sx, sy);
            }
        });
    }
}
