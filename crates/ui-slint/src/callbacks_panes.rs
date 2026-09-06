// Naygo — cableado de callbacks multi-panel: swap/clonar/apilar, pestañas, arrastre de
// paneles, selector de destino y arrastre de splitters.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers de la disposición de paneles: intercambiar/clonar/apilar, seleccionar/cerrar
// pestañas, arrastrar paneles a zonas de drop, cerrar panel, selector de destino (pick) y
// arrastre de splitters (barra fantasma en vivo + commit + reset 50/50). Extraídos de
// `main.rs` sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::wire::WireCtx;
use crate::*;
use naygo_core::workspace::PaneId;

/// Registra los callbacks multi-panel (swap/clone/stack/tabs/drag/pick/splitters).
pub(crate) fn wire_panes(ui: &AppWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_layout,
        start_timer,
        area_of,
        ..
    } = ctx;
    // Bandeja temporal: selección, destinos y operaciones. Copiar/Mover abre primero el radar
    // de paneles Files visibles; «Otra carpeta…» queda como escape explícito en ese mismo radar.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_basket_remove(move |index| {
            ctrl.borrow_mut().basket_remove(index.max(0) as usize);
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_basket_select(move |index, control, shift| {
            if ctrl
                .borrow_mut()
                .basket_select_modified(index.max(0) as usize, control, shift)
            {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        // Igual que el arrastre desde Files, `DoDragDrop` se difiere al próximo turno: su loop
        // modal re-entra Slint y no puede convivir con un RefCell prestado por este callback.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_basket_drag_out(move |index| {
            let (paths, staging) = {
                let mut c = ctrl.borrow_mut();
                let Some(path) = c.basket.items().get(index.max(0) as usize).cloned() else {
                    return;
                };
                if !c.basket_selection.contains(&path) {
                    c.basket_select(index.max(0) as usize);
                }
                (c.basket_action_paths(), c.basket_staging.clone())
            };
            if paths.is_empty() {
                return;
            }
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_drag_in_progress(true);
                }
                let _staging = staging; // Conservar fuentes virtuales durante todo el bucle OLE.
                let _ = naygo_platform::dnd::start_drag(&paths);
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_drag_in_progress(false);
                    ui.set_drag_just_ended(true);
                }
            });
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_basket_select_all(move || {
            ctrl.borrow_mut().basket_select_all();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_basket_remove_selected(move || {
            ctrl.borrow_mut().basket_remove_selected();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_basket_clear(move || {
            ctrl.borrow_mut().basket_clear();
            sync_layout();
        });
    }
    for move_files in [false, true] {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let handler = move || {
            if ctrl.borrow_mut().basket_request_transfer(move_files) {
                start_timer();
            }
            sync_layout();
        };
        if move_files {
            ui.on_basket_move(handler);
        } else {
            ui.on_basket_copy(handler);
        }
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_basket_delete(move || {
            if ctrl.borrow_mut().basket_delete() {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        ui.on_basket_save_list(move || {
            // La raíz no se infiere silenciosamente: el usuario la elige. Las referencias bajo
            // ella se guardan relativas y las externas siguen absolutas, tal como exige el
            // formato portable. Cancelar cualquiera de los dos selectores no modifica nada.
            let initial_root = ctrl.borrow().active_dir();
            let mut root_dialog = rfd::FileDialog::new();
            if let Some(initial_root) = initial_root.as_deref() {
                root_dialog = root_dialog.set_directory(initial_root);
            }
            let Some(root) = root_dialog.pick_folder() else {
                return;
            };
            let text = ctrl.borrow().basket_naygolist_json(Some(root)).ok();
            if let (Some(text), Some(path)) = (
                text,
                rfd::FileDialog::new()
                    .add_filter("Naygo list", &["naygolist"])
                    .set_file_name("bandeja.naygolist")
                    .save_file(),
            ) {
                std::thread::spawn(move || {
                    let _ = std::fs::write(path, text);
                });
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_basket_open_list(move || {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Naygo list", &["naygolist"])
                .pick_file()
            else {
                return;
            };
            ctrl.borrow_mut().start_basket_import(path);
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_sync_set_mode(move |mode| {
            ctrl.borrow_mut().sync_set_mode(mode);
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_sync_set_delete(move |value| {
            ctrl.borrow_mut().sync_set_delete_extras(value);
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_sync_toggle(move |index| {
            ctrl.borrow_mut().sync_toggle_item(index.max(0) as usize);
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_sync_apply(move || {
            if ctrl.borrow_mut().sync_apply() {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_sync_close(move || {
            ctrl.borrow_mut().sync_close();
            sync_layout();
        });
    }
    // --- Acciones multi-panel (swap / clonar) + selector de destino ---
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let area_of = area_of.clone();
        ui.on_swap_panes(move || {
            let area = area_of();
            let acted = {
                let mut c = ctrl.borrow_mut();
                let Some(origin) = c.active_id() else {
                    return;
                };
                c.request_action(workspace_ctrl::PaneAction::Swap, origin, area)
            };
            if acted {
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
        ui.on_clone_pane(move || {
            let area = area_of();
            let acted = {
                let mut c = ctrl.borrow_mut();
                let Some(origin) = c.active_id() else {
                    return;
                };
                c.request_action(workspace_ctrl::PaneAction::Clone, origin, area)
            };
            if acted {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        ui.on_stack_pane(move || {
            let area = area_of();
            {
                let mut c = ctrl.borrow_mut();
                let Some(origin) = c.active_id() else {
                    return;
                };
                c.request_action(workspace_ctrl::PaneAction::Stack, origin, area);
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tab_select(move |id| {
            ctrl.borrow_mut().set_active_tab(PaneId(id as u64));
            // Cambiar de pestaña puede disparar el preview del nuevo foco.
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_tab_close(move |id| {
            ctrl.borrow_mut().close_tab(PaneId(id as u64));
            sync_layout();
        });
    }
    {
        // Durante el arrastre: resaltar la zona de drop bajo el puntero.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let area_of = area_of.clone();
        ui.on_pane_drag_move(move |id, x, y| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let area = area_of();
            let preview = ctrl.borrow().drop_preview(PaneId(id as u64), x, y, area);
            match preview {
                Some((r, is_tab)) => {
                    ui.set_drop_x(r.x);
                    ui.set_drop_y(r.y);
                    ui.set_drop_w(r.w);
                    ui.set_drop_h(r.h);
                    ui.set_drop_is_tab(is_tab);
                }
                None => {
                    ui.set_drop_w(0.0);
                    ui.set_drop_h(0.0);
                }
            }
        });
    }
    {
        // Al soltar: recomponer el layout y limpiar el resaltado.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        ui.on_pane_drag_drop(move |id, x, y| {
            let area = area_of();
            ctrl.borrow_mut()
                .perform_drop(PaneId(id as u64), x, y, area);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_drop_w(0.0);
                ui.set_drop_h(0.0);
            }
            sync_layout();
        });
    }
    // Cerrar/quitar un panel (X del título o clic derecho).
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_maximize_pane(move |id| {
            ctrl.borrow_mut().toggle_maximize(PaneId(id as u64));
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_close_pane(move |id| {
            ctrl.borrow_mut().close_pane(PaneId(id as u64));
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_pick_resolve(move |n| {
            if ctrl.borrow_mut().pick_resolve(n as usize) {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_pick_cancel(move || {
            ctrl.borrow_mut().pick_cancel();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_destination_radar_choose(move |index| {
            ctrl.borrow_mut()
                .destination_radar_resolve(index.max(0) as usize);
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_destination_radar_browse(move || {
            // El diálogo se abre sin mantener un préstamo al controlador. Si el usuario cancela,
            // el radar queda abierto y puede escoger un panel visible sin repetir el gesto.
            let Some(path) = rfd::FileDialog::new().pick_folder() else {
                return;
            };
            ctrl.borrow_mut().destination_radar_resolve_path(path);
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_destination_radar_cancel(move || {
            ctrl.borrow_mut().destination_radar_cancel();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let area_of = area_of.clone();
        let ui_weak = ui.as_weak();
        // ARRASTRE EN VIVO: NO reflowar el layout (eso no repinta bajo el render por software y
        // se ve distorsionado). Solo calcular dónde quedaría el borde y pintar la barra-fantasma
        // (un Rectangle por escalares, que sí repinta al instante). `px`/`py` = posición ABSOLUTA
        // del puntero en coords de contenido.
        ui.on_split_drag(move |index, px, py| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let area = area_of();
            let c = ctrl.borrow();
            let handles = c.split_handles(area);
            if let Some(h) = handles.get(index as usize) {
                if let Some((_f, bar)) = c.divider_at(&h.path.clone(), h.divider, area, px, py) {
                    ui.set_splitpreview_x(bar.x);
                    ui.set_splitpreview_y(bar.y);
                    ui.set_splitpreview_w(bar.w);
                    ui.set_splitpreview_h(bar.h);
                }
            }
        });
    }
    // COMMIT (al soltar): aplicar la fraction de verdad, reflowar UNA vez, y limpiar la fantasma.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        let ui_weak = ui.as_weak();
        ui.on_split_commit(move |index, px, py| {
            let area = area_of();
            {
                let mut c = ctrl.borrow_mut();
                let target = {
                    let handles = c.split_handles(area);
                    handles
                        .get(index as usize)
                        .map(|h| (h.path.clone(), h.divider))
                };
                if let Some((path, divider)) = target {
                    if let Some((f, _bar)) = c.divider_at(&path, divider, area, px, py) {
                        c.set_divider(&path, divider, f);
                    }
                }
            }
            // Ocultar la barra-fantasma (w/h = 0).
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_splitpreview_w(0.0);
                ui.set_splitpreview_h(0.0);
            }
            sync_layout();
        });
    }
    // DOBLE-CLIC en un divisor: repartir 50/50 sus dos paneles vecinos.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        ui.on_split_reset(move |index| {
            let area = area_of();
            {
                let mut c = ctrl.borrow_mut();
                // Copiar (path, divider) del handle ANTES de mutar: `handles` toma prestado
                // `c` y no puede vivir cuando se llama `set_divider` (patrón de la Fase 1).
                let target = {
                    let handles = c.split_handles(area);
                    handles
                        .get(index as usize)
                        .map(|h| (h.path.clone(), h.divider))
                };
                if let Some((path, divider)) = target {
                    c.set_divider(&path, divider, 0.5);
                }
            }
            sync_layout();
        });
    }
    {
        let sync_layout = sync_layout.clone();
        ui.on_content_resized(move || sync_layout());
    }
}
