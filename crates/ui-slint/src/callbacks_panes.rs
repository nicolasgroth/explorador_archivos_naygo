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
