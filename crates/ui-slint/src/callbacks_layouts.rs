// Naygo — cableado de callbacks de plantillas de disposición, renombrado por lotes y ayuda.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers de las plantillas de layout (aplicar/guardar/borrar), del renombrado por lotes
// (F5: setters del spec, aplicar, cerrar) y del cierre de la ayuda (F1). Extraídos de
// `main.rs` sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::wire::WireCtx;
use crate::*;
use std::rc::Rc;

/// Registra los callbacks de plantillas de disposición, batch-rename y ayuda.
pub(crate) fn wire_layouts(ui: &AppWindow, ctx: &WireCtx, refresh_layouts: &Rc<dyn Fn()>) {
    let WireCtx {
        ctrl,
        sync_rows,
        sync_layout,
        start_timer,
        ..
    } = ctx;
    // Aplicar una plantilla: reconstruye el workspace y relanza el contenido de cada panel.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let refresh_layouts = refresh_layouts.clone();
        ui.on_apply_layout(move |name| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            ctrl.borrow_mut().apply_template(name.as_str(), now);
            start_timer();
            sync_layout();
            refresh_layouts();
        });
    }
    // Guardar la disposición actual como plantilla de usuario.
    {
        let ctrl = ctrl.clone();
        let refresh_layouts = refresh_layouts.clone();
        ui.on_save_layout(move |name| {
            ctrl.borrow_mut().save_current_template(name.as_str());
            refresh_layouts();
        });
    }
    // Borrar una plantilla de usuario.
    {
        let ctrl = ctrl.clone();
        let refresh_layouts = refresh_layouts.clone();
        ui.on_delete_layout(move |name| {
            ctrl.borrow_mut().delete_template(name.as_str());
            refresh_layouts();
        });
    }
    // --- Renombrado por lotes (F5): setters del spec (cada uno re-renderiza el preview) ---
    macro_rules! batch_setter_str {
        ($on:ident, $method:ident) => {{
            let ctrl = ctrl.clone();
            let sync_rows = sync_rows.clone();
            ui.$on(move |v: slint::SharedString| {
                ctrl.borrow_mut().$method(v.as_str());
                sync_rows();
            });
        }};
    }
    macro_rules! batch_setter_bool {
        ($on:ident, $method:ident) => {{
            let ctrl = ctrl.clone();
            let sync_rows = sync_rows.clone();
            ui.$on(move |v: bool| {
                ctrl.borrow_mut().$method(v);
                sync_rows();
            });
        }};
    }
    batch_setter_str!(on_batch_set_template, batch_set_template);
    batch_setter_str!(on_batch_set_find, batch_set_find);
    batch_setter_str!(on_batch_set_replace, batch_set_replace);
    batch_setter_str!(on_batch_set_counter_start, batch_set_counter_start);
    batch_setter_str!(on_batch_set_counter_step, batch_set_counter_step);
    batch_setter_bool!(on_batch_set_regex, batch_set_regex);
    batch_setter_bool!(on_batch_set_include_ext, batch_set_include_ext);
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_batch_set_case(move |i| {
            ctrl.borrow_mut().batch_set_case(i);
            sync_rows();
        });
    }
    // Aplicar: lanza la op de batch-rename (deshacible) y refresca el panel.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_batch_apply(move || {
            ctrl.borrow_mut().batch_apply();
            start_timer();
            sync_layout();
        });
    }
    // Cerrar sin aplicar.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_batch_close(move || {
            ctrl.borrow_mut().batch_close();
            sync_rows();
        });
    }
    // Cerrar la ayuda (Esc/clic fuera/✕).
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_help_close(move || {
            ctrl.borrow_mut().help_close();
            sync_rows();
        });
    }
}
