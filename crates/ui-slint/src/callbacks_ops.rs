// Naygo — cableado de callbacks de operaciones de archivo: diálogos modales (borrar,
// conflicto, nombre, pegado), panel de progreso, retomar y deshacer.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers del motor de ops: confirmar/cancelar borrado, decisiones de conflicto (archivo y
// carpeta), modal de nombre (crear/comprimir/renombrar-en-conflicto), pegado, cancelar/
// pausar/reanudar/ver-archivos de una op, retomar ops interrumpidas, deshacer (con popup de
// confirmación), calcular tamaño y "abrir con…" del preview. Extraídos de `main.rs` sin
// cambio de comportamiento (refactor por tamaño de archivo).

use crate::ops_ctrl;
use crate::vm_builders::*;
use crate::wire::WireCtx;
use crate::*;
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// Registra los callbacks de operaciones de archivo y sus diálogos modales.
pub(crate) fn wire_ops(ui: &AppWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_rows,
        start_timer,
        pending_undo,
        ..
    } = ctx;
    {
        // Botón "abrir con el programa del sistema" del panel de vista previa: ShellExecute
        // sobre la ruta del archivo previsualizado (la app no edita; abre con el editor del SO).
        ui.on_preview_open(move |path| {
            if !path.is_empty() {
                let _ = naygo_platform::open::open_default(std::path::Path::new(path.as_str()));
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_preview_orbit(move |dx, dy| {
            if ctrl.borrow_mut().preview_orbit(dx, dy) {
                start_timer();
                sync_rows();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_preview_reset_camera(move || {
            if ctrl.borrow_mut().preview_reset_camera() {
                start_timer();
                sync_rows();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_preview_cancel(move || {
            if ctrl.borrow_mut().cancel_preview() {
                sync_rows();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        ui.on_preview_copy_text(move |text| {
            ctrl.borrow().copy_preview_text(text.as_str());
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_preview_zoom(move |delta| {
            if ctrl.borrow_mut().preview_zoom(delta) {
                start_timer();
                sync_rows();
            }
        });
    }
    {
        // Botón "Deshacer" del panel Historial: en vez de deshacer directo, abre un popup de
        // CONFIRMACIÓN (MessageVm kind 5) que explica QUÉ se hará (borrar N / devolver N + lista de
        // archivos). El deshacer real ocurre al confirmar (ver `on_message_confirm`, kind 5). El id
        // pendiente se guarda en `pending_undo`.
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let pending_undo = pending_undo.clone();
        let start_timer = start_timer.clone();
        ui.on_undo_entry(move |id| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let id = id as u64;
            // Componer el detalle (resumen + lista de líneas) desde el ctrl. Si la entrada ya no es
            // deshacible (ya deshecha / inválida), no se abre nada.
            let preview = ctrl.borrow().undo_preview(id);
            let Some((summary, lines)) = preview else {
                return;
            };
            let tr = ui.global::<Tr>();
            let cfg = ctrl.borrow();
            // Cuerpo del popup: el resumen y, debajo, hasta MAX_LINES nombres. Si sobran, una línea
            // final "(y N más)". Así el detalle esencial siempre cabe sin volver ilegible el modal.
            const MAX_LINES: usize = 5;
            let mut body = summary;
            let shown = lines.len().min(MAX_LINES);
            for line in lines.iter().take(shown) {
                body.push_str("\n• ");
                body.push_str(line);
            }
            if lines.len() > MAX_LINES {
                let more = cfg
                    .config
                    .t("slint.undo.and_more")
                    .replace("{n}", &(lines.len() - MAX_LINES).to_string());
                body.push('\n');
                body.push_str(&more);
            }
            drop(cfg);
            // Guardar el id a deshacer y abrir el popup de confirmación (2 botones).
            *pending_undo.borrow_mut() = Some(id);
            ui.set_message(MessageVm {
                kind: 5, // 5 = confirmar deshacer (2 botones, velo/Esc = cancelar)
                level: 0,
                title: tr.get_slint_undo_confirm_title(),
                body: body.into(),
                confirm_label: tr.get_history_undo(),
                cancel_label: tr.get_dlg_cancel(),
                danger: false,
            });
            // Modal abierto desde un clic: rearmar el timer para que el popup responda al instante.
            start_timer();
        });
    }
    {
        // Botón "Calcular" del Inspector: dispara el mismo cálculo async cancelable que F3
        // sobre la carpeta enfocada/actual. `pump_sizes` (drenado en el tick) y `sync_rows`
        // (vía `size_status`) se encargan de reflejar el progreso hasta que termine.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_calc_size(move || {
            ctrl.borrow_mut().compute_size_active();
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        ui.on_copy_provenance(move || {
            let _ = ctrl.borrow().copy_provenance();
        });
    }
    {
        let ctrl = ctrl.clone();
        let start_timer = start_timer.clone();
        ui.on_unblock_provenance(move || {
            // Confirmación explícita: quitar Zone.Identifier cambia la política de seguridad de
            // Windows. El diálogo es modal, pero la eliminación real sigue en worker.
            let (title, description) = {
                let c = ctrl.borrow();
                (
                    c.config.t("provenance.unblock_title"),
                    c.config.t("provenance.unblock_confirm"),
                )
            };
            let accepted = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title(&title)
                .set_description(&description)
                .set_buttons(rfd::MessageButtons::YesNo)
                .show();
            if accepted == rfd::MessageDialogResult::Yes {
                if ctrl.borrow_mut().unblock_provenance() {
                    start_timer();
                }
            }
        });
    }
    // --- Diálogos modales y panel de progreso de operaciones (F3) ---
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_delete_confirm(move || {
            // delete_confirm devuelve true si arrancó una op larga (eliminación permanente por el
            // motor). La papelera es atómica y no devuelve true, así que el panel no aparece para
            // ella. Si arrancó algo, asegurar el panel de Operaciones (auto-aparecer).
            let started = ctrl.borrow_mut().ops.delete_confirm();
            if started {
                ctrl.borrow_mut().ensure_ops_pane();
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_delete_cancel(move || {
            ctrl.borrow_mut().ops.dialog_cancel();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_conflict_decide(move |action, apply_all| {
            use naygo_core::ops::ConflictAction;
            // Mapeo int→acción (espeja op-dialogs.slint):
            //   0=Sobrescribir 1=Saltar 2=Mantener ambos (Rename sufijo)
            //   3=Renombrar antiguo (RenameExisting) 4=Saltar idénticos (SkipIdentical)
            // "Reemplazar todo"/"Saltar todo" = 0/1 con apply_all=true (la casilla/desplegable).
            let act = match action {
                0 => ConflictAction::Overwrite,
                2 => ConflictAction::Rename,
                3 => ConflictAction::RenameExisting,
                4 => ConflictAction::SkipIdentical,
                _ => ConflictAction::Skip,
            };
            // El id estable de la op en conflicto lo guarda el pending_dialog.
            let op_id = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::Conflict { op_id, .. }) = &c.ops.pending_dialog {
                    Some(*op_id)
                } else {
                    None
                }
            };
            if let Some(op_id) = op_id {
                ctrl.borrow_mut()
                    .ops
                    .resolve_conflict(op_id, act, apply_all);
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_conflict_cancel(move || {
            // Cancelar TODA la operación desde el modal de conflicto (botón "Cancelar todo",
            // Esc o clic fuera). Toma el id estable de la op en conflicto del pending_dialog,
            // igual que `on_conflict_decide`, y cancela su token sin enviar una decisión: el
            // worker está esperando con `recv_timeout` y, al ver el token cancelado, aborta.
            let op_id = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::Conflict { op_id, .. }) = &c.ops.pending_dialog {
                    Some(*op_id)
                } else {
                    None
                }
            };
            if let Some(op_id) = op_id {
                ctrl.borrow_mut().ops.cancel_conflict(op_id);
                // Mantener el timer vivo para que `pump_ops` drene el `Cancelled`/`Skipped` del
                // worker y cierre la op (progreso → historial), igual que un cancel normal.
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_conflict_rename(move || {
            // BUG 1: "Renombrar" en el conflicto NO decide el choque; abre el modal de nombre
            // (precargado con la sugerencia "(N)") para que el usuario escriba el nombre nuevo. La
            // resolución real ocurre al confirmar ese modal (ver `name_confirm` → RenameTo). El id
            // estable de la op en conflicto lo guarda el pending_dialog, igual que `conflict_decide`.
            let op_id = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::Conflict { op_id, .. }) = &c.ops.pending_dialog {
                    Some(*op_id)
                } else {
                    None
                }
            };
            if let Some(op_id) = op_id {
                ctrl.borrow_mut().ops.begin_conflict_rename(op_id);
            }
            // No se arranca ni reanuda nada todavía (el motor sigue esperando); solo cambió el
            // modal activo. `sync_rows` repinta el VM del diálogo (de conflicto a nombre).
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_folder_conflict_decide(move |decision, apply_all| {
            // Conflicto de CARPETA (P3): aplicar la decisión (0=fusionar 1=reemplazar 2=saltar) a
            // la op detenida en el conflicto. El id estable lo guarda el pending_dialog.
            let op_id = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::FolderConflict { op_id, .. }) =
                    &c.ops.pending_dialog
                {
                    Some(*op_id)
                } else {
                    None
                }
            };
            if let Some(op_id) = op_id {
                ctrl.borrow_mut()
                    .ops
                    .resolve_folder_conflict(op_id, decision, apply_all);
                // Reactivar el timer: si quedan más carpetas, `pump_ops` reabre el modal; si no,
                // arranca el motor (o el escaneo de la cola) y hay que drenarlo.
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_folder_conflict_cancel(move || {
            // Cancelar TODA la operación desde el conflicto de carpeta (botón, Esc o velo).
            let op_id = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::FolderConflict { op_id, .. }) =
                    &c.ops.pending_dialog
                {
                    Some(*op_id)
                } else {
                    None
                }
            };
            if let Some(op_id) = op_id {
                ctrl.borrow_mut().ops.cancel_folder_conflict(op_id);
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        ui.on_name_changed(move |v| {
            ctrl.borrow_mut().ops.name_changed(v.to_string());
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_name_confirm(move || {
            let compress_label = ctrl.borrow().config.t("ops.kind_compress");
            let confirmed = ctrl.borrow_mut().ops.name_confirm(&compress_label);
            if confirmed {
                // Re-listar el panel activo tras crear/pegar: el pegado escribe el archivo
                // directo (sin pasar por el motor de ops), y el listado debe refrescarse para
                // que aparezca. Además `refresh_active` re-lista con `enter()`, que LIMPIA la
                // selección/foco; sin esto, tras pegar quedaba un foco de vista "stale" que
                // bloqueaba los clics del mouse hasta reseleccionar.
                ctrl.borrow_mut().refresh_active();
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_name_cancel(move || {
            ctrl.borrow_mut().ops.dialog_cancel();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_paste_confirm(move || {
            // El pegado de texto/imagen se cablea con el journal/encode; por ahora cierra.
            ctrl.borrow_mut().ops.dialog_cancel();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_paste_cancel(move || {
            ctrl.borrow_mut().ops.dialog_cancel();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_op_cancel(move |id| {
            // `id` es el id ESTABLE de la op (lo emite `OpRowData.index`), no su posición.
            ctrl.borrow_mut().ops.cancel_op(id);
            sync_rows();
        });
    }
    // "Cancelar todo" (PUNTO 2): abre un modal de confirmación (MessageModal kind 4, de 2 botones).
    // La cancelación real ocurre en `on_message_confirm` (kind 4); aquí solo pedimos confirmación.
    {
        let ui_weak = ui.as_weak();
        let start_timer = start_timer.clone();
        ui.on_op_cancel_all(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let tr = ui.global::<Tr>();
            ui.set_message(MessageVm {
                kind: 4,  // 4 = confirmar "cancelar todas las operaciones"
                level: 1, // warning
                title: tr.get_ops_cancel_all_title(),
                body: tr.get_ops_cancel_all_q(),
                confirm_label: tr.get_ops_cancel_all_yes(),
                cancel_label: tr.get_ops_cancel_all_no(),
                danger: true,
            });
            // Modal abierto desde un clic: rearmar el timer para que el popup responda al instante.
            start_timer();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_op_pause(move |id| {
            ctrl.borrow_mut().ops.pause_op(id);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_op_resume(move |id| {
            ctrl.borrow_mut().ops.resume_op(id);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_op_skip(move |id| {
            // Saltar el archivo en curso aún no está soportado por el motor (no-op).
            ctrl.borrow_mut().ops.skip_op(id);
            sync_rows();
        });
    }
    {
        // "Ver N archivos" de una fila de historial: arma la lista completa de archivos de esa op
        // (pedida por demanda a `op_file_list`) y abre el modal con ella.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_op_show_files(move |id| {
            let (rows, label, context) = {
                let c = ctrl.borrow();
                let id = id as u64;
                let rows = c.ops.op_file_list(id);
                // Contexto del encabezado (tipo/origen/destino/estadísticas) — PUNTO 3.
                let context = c.ops.op_file_context(id);
                let label = c
                    .ops
                    .active_ops
                    .iter()
                    .find(|o| o.id == id)
                    .map(|o| o.label.clone())
                    .unwrap_or_default();
                (rows, label, context)
            };
            if let Some(ui) = ui_weak.upgrade() {
                let vms: Vec<OpFileVm> = rows.into_iter().map(to_op_file_vm).collect();
                ui.set_op_file_list(ModelRc::from(Rc::new(VecModel::from(vms))));
                ui.set_op_file_list_label(SharedString::from(label.as_str()));
                ui.set_op_file_context(to_op_file_context_vm(context));
                ui.set_op_file_list_open(true);
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_clipboard_history_pick(move |index| {
            if index >= 0 && ctrl.borrow_mut().paste_history_entry(index as usize) {
                start_timer();
            }
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_clipboard_history_cancel(move || {
            ctrl.borrow_mut().ops.pending_dialog = None;
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_resume_decide(move |id, action| {
            let id = id.to_string();
            let mut c = ctrl.borrow_mut();
            if action == 0 {
                if c.ops.resume(&id) {
                    // Retomar arranca una transferencia larga: mostrar el panel de Operaciones.
                    c.ensure_ops_pane();
                    drop(c);
                    start_timer();
                }
            } else {
                c.ops.discard(&id);
            }
            sync_rows();
        });
    }
}
