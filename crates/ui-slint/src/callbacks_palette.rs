// Naygo — cableado de callbacks de la paleta de comandos (Ctrl+P).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers de la paleta: re-filtrar al tipear, cerrar sin ejecutar y ejecutar el resultado
// elegido (con consumo de los pedidos que el comando deja: re-aplicar tema, abrir config).
// Extraídos de `main.rs` sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::theme_apply;
use crate::vm_builders::*;
use crate::wire::WireCtx;
use crate::*;
use slint::{ModelRc, VecModel};
use std::rc::Rc;

/// Registra los callbacks de la paleta de comandos (query / dismiss / run).
pub(crate) fn wire_palette(ui: &AppWindow, cfg_win: &ConfigWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_rows,
        sync_layout,
        start_timer,
        palette_cmds,
        palette_cmd_indices,
        ..
    } = ctx;
    // Paleta de comandos (Ctrl+P): se escribió en el campo → re-filtrar con la query nueva sobre
    // los comandos vigentes. Reconstruye los resultados y reinicia la selección al primero.
    {
        let ui_weak = ui.as_weak();
        let palette_cmds = palette_cmds.clone();
        let palette_cmd_indices = palette_cmd_indices.clone();
        ui.on_palette_query_changed(move |q| {
            if let Some(ui) = ui_weak.upgrade() {
                let cmds = palette_cmds.borrow();
                let matches = naygo_core::palette::filter_and_rank(&cmds, q.as_str());
                let (items, idxs) = palette_items_from_matches(&cmds, &matches);
                *palette_cmd_indices.borrow_mut() = idxs;
                ui.set_palette_results(ModelRc::from(Rc::new(VecModel::from(items))));
                ui.set_palette_selected(0);
            }
        });
    }
    // Paleta: cerrar sin ejecutar (Esc / clic fuera).
    {
        let ui_weak = ui.as_weak();
        ui.on_palette_dismiss(move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_palette_open(false);
            }
        });
    }
    // Paleta: ejecutar el resultado de índice `result_idx`. La fila lleva el índice del COMANDO
    // (tabla paralela `palette_cmd_indices`). Ejecuta, cierra la paleta, y consume los pedidos que
    // el comando pudo dejar: re-aplicar tema (a ambas ventanas) y/o abrir configuración. Refresca.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let cfg_weak = cfg_win.as_weak();
        let sync_layout = sync_layout.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let palette_cmds = palette_cmds.clone();
        let palette_cmd_indices = palette_cmd_indices.clone();
        ui.on_palette_run(move |result_idx| {
            let cmd_idx = palette_cmd_indices
                .borrow()
                .get(result_idx.max(0) as usize)
                .copied();
            let Some(cmd_idx) = cmd_idx else {
                // Sin resultados (índice fuera de rango): solo cerrar.
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_palette_open(false);
                }
                return;
            };
            {
                let cmds = palette_cmds.borrow();
                if ctrl.borrow_mut().execute_palette_command(&cmds, cmd_idx) {
                    start_timer();
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_palette_open(false);
            }
            // Tema elegido en la paleta: re-pintar ambas ventanas con el tema activo.
            let palette_theme_request = ctrl.borrow_mut().take_palette_theme_request();
            if palette_theme_request.is_some() {
                let c = ctrl.borrow();
                if let Some(ui) = ui_weak.upgrade() {
                    theme_apply::apply(&ui, c.config.active_theme());
                }
                if let Some(cfg) = cfg_weak.upgrade() {
                    theme_apply::apply(&cfg, c.config.active_theme());
                }
            }
            // "Abrir configuración" desde la paleta: reusa el handler del engranaje de la toolbar.
            if ctrl.borrow_mut().take_open_config_request() {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_open_config();
                }
            }
            sync_layout();
            sync_rows();
        });
    }
}
