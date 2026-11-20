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
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let ui_weak = ui.as_weak();
        ui.on_rename_cancel(move || {
            ctrl.borrow_mut().rename_cancel();
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_rename_pane(-1);
                ui.set_rename_pos(-1);
            }
            // Si la fuente faltaba durante un listado parcial, el modelo visual estaba
            // retenido para proteger el editor; al cancelar hay que reconciliarlo ya.
            start_timer();
            sync_rows();
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
    // El callback mantiene la sesión y calcula offsets del borrador, pues el gate de teclado
    // no reenvía teclas durante el rename y no debe reemplazar texto ni remountar el editor.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_rename_cycle(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            // F2 cambia la selección del editor actual, no su contenido. El controlador
            // mantiene el nombre y la ruta originales para confirmar de forma segura.
            let draft = ui.get_rename_draft().to_string();
            let selection = ctrl.borrow_mut().rename_cycle_selection(&draft);
            if let Some((start, end)) = selection {
                ui.set_rename_selection_start(start as i32);
                ui.set_rename_selection_end(end as i32);
                ui.set_rename_selection_revision(
                    ui.get_rename_selection_revision().wrapping_add(1),
                );
            }
        });
    }
}
