// Naygo — cableado de callbacks del rename inline (F2): commit, encadenar y cancelar.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Sesiones de rename inline en la celda Name: confirmar (Enter), encadenar a la fila vecina
// (↑/↓) y cancelar (Esc). OJO: el patrón de borrows (soltar `borrow_mut` ANTES de los
// `if let`) es intencional — evita el panic «already borrowed» del crash de F2. Extraídos
// de `main.rs` sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::vm_builders::*;
use crate::wire::WireCtx;
use crate::*;
use naygo_core::workspace::PaneId;

/// Registra los callbacks del editor de rename inline (F2).
pub(crate) fn wire_rename(ui: &AppWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_rows,
        start_timer,
        ..
    } = ctx;
    // Rename inline: confirmar (Enter). Renombra y cierra el editor. (6D)
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let ui_weak = ui.as_weak();
        ui.on_rename_commit(move |id, session, _pos, name| {
            let pane = PaneId(id as u64);
            if !ctrl.borrow().rename_session_is_active(pane, session as u32) {
                return;
            }
            ctrl.borrow_mut()
                .rename_commit(pane, session as u32, name.as_str());
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_rename_pane(-1);
                ui.set_rename_pos(-1);
            }
            start_timer();
            sync_rows();
        });
    }
    // Rename inline: encadenar (↑/↓). Confirma el actual y reabre el editor en la fila vecina. (6D)
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let ui_weak = ui.as_weak();
        ui.on_rename_chain(move |id, session, _pos, name, dir| {
            let pane = PaneId(id as u64);
            if !ctrl.borrow().rename_session_is_active(pane, session as u32) {
                return;
            }
            let next = ctrl
                .borrow_mut()
                .rename_chain(pane, session as u32, name.as_str(), dir);
            if let Some(ui) = ui_weak.upgrade() {
                match next {
                    Some(_) => {
                        let request = ctrl.borrow_mut().take_rename_request();
                        if let Some(request) = request {
                            show_rename_editor(&ui, &request);
                        } else {
                            ui.set_rename_pane(-1);
                            ui.set_rename_pos(-1);
                        }
                    }
                    None => {
                        ui.set_rename_pane(-1);
                        ui.set_rename_pos(-1);
                    }
                }
            }
            start_timer();
            sync_rows();
        });
    }
    // Rename inline: cancelar (Esc). Cierra el editor sin renombrar. (6D)
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_rename_cancel(move || {
            ctrl.borrow_mut().rename_cancel();
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_rename_pane(-1);
                ui.set_rename_pos(-1);
            }
        });
    }
    // Rename inline: texto vivo del editor. Solo computa el largo UTF-8 para `rename-cursor`
    // (cursor a restaurar tras un re-montaje del delegate; LineEdit no expone cursor-position
    // y al tipear el cursor va al final). No toca disco ni el controlador: es barato.
    {
        let ui_weak = ui.as_weak();
        ui.on_rename_edited(move |text| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_rename_cursor(text.len() as i32);
            }
        });
    }
    // F2 DENTRO del editor: cicla la selección (nombre → extensión → todo → nombre). Es la
    // misma acción que el F2 global (op_rename incrementa el `stage` sobre el mismo archivo),
    // pero llega por callback porque el gate de teclado no reenvía teclas durante el rename.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_rename_cycle(move || {
            ctrl.borrow_mut().op_rename();
            let request = ctrl.borrow_mut().take_rename_request();
            if let (Some(ui), Some(request)) = (ui_weak.upgrade(), request) {
                show_rename_editor(&ui, &request);
            }
        });
    }
}
