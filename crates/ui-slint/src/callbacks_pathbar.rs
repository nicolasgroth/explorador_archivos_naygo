// Naygo — cableado de callbacks de la path-bar (breadcrumbs, edición con autocompletado
// async) y del árbol de favoritos editable.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers de la barra de ruta: clic en breadcrumb, modo edición (inicio/cambio/commit/
// cancel), clic en sugerencia, ★ favorito y copiar ruta; y del árbol de favoritos:
// expandir, nuevo grupo, renombrar, eliminar, mover y destinos del submenú "Mover a…".
// Extraídos de `main.rs` sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::wire::WireCtx;
use crate::*;
use naygo_core::workspace::PaneId;
use slint::{ModelRc, SharedString};
use std::rc::Rc;

#[cfg(test)]
#[path = "callbacks_pathbar_tests.rs"]
mod tests;

/// Registra los callbacks de la path-bar y del árbol de favoritos editable.
pub(crate) fn wire_pathbar(ui: &AppWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_rows,
        sync_layout,
        start_timer,
        ..
    } = ctx;
    let validation_timer = Rc::new(slint::Timer::default());
    let validation_cancel = Rc::new(std::cell::RefCell::new(
        None::<naygo_core::CancellationToken>,
    ));
    // --- Barra de ruta (breadcrumbs + edición + autocompletado) ---
    {
        // Clic en un breadcrumb: navegar ese panel a la ruta del segmento.
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_path_segment_clicked(move |id, path| {
            if ctrl
                .borrow_mut()
                .navigate_pane_to(PaneId(id as u64), std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        // Clic derecho en un breadcrumb: menú completo de la carpeta específica, incluyendo
        // abrir aquí/en otro panel/nuevo panel, carpeta nueva, borrar y Shell de Windows.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_path_segment_context(move |id, path, x, y| {
            ctrl.borrow_mut().open_path_folder_context_menu(
                PaneId(id as u64),
                std::path::PathBuf::from(path.as_str()),
                x,
                y,
            );
            sync_rows();
            start_timer();
        });
    }
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let start_timer = start_timer.clone();
        let validation_cancel = validation_cancel.clone();
        ui.on_path_edit_start(move |id| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let path = ctrl.borrow().path_of(PaneId(id as u64));
            if path.is_empty() {
                return;
            }
            if let Some(cancel) = validation_cancel.borrow_mut().take() {
                cancel.cancel();
            }
            ctrl.borrow_mut().cancel_path_autocomplete();
            ui.set_edit_revision(ui.get_edit_revision().wrapping_add(1));
            ui.set_edit_session(ui.get_edit_session().wrapping_add(1));
            ui.set_edit_error(SharedString::new());
            ui.set_edit_validating(false);
            ui.set_edit_base(path.clone().into());
            ui.set_edit_text(path.clone().into());
            ui.set_edit_caret(-1);
            ui.set_edit_suggestions(ModelRc::default());
            ui.set_edit_toolbar(false);
            ui.set_edit_pane(id);
            ctrl.borrow_mut()
                .request_path_autocomplete(path, std::time::Instant::now());
            start_timer();
        });
    }
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_toolbar_path_start(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            // Capturar ahora, no resolver active_id al confirmar ni al elegir sugerencias.
            let Some(id) = ctrl.borrow().active_id() else {
                return;
            };
            ui.invoke_path_edit_start(id.0 as i32);
            if ui.get_edit_pane() == id.0 as i32 {
                ui.set_edit_toolbar(true);
                ui.set_toolbar_path_favorite(ctrl.borrow().is_pane_dir_favorite(id));
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let start_timer = start_timer.clone();
        let validation_cancel = validation_cancel.clone();
        ui.on_path_edit_changed(move |id, text| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if ui.get_edit_pane() != id {
                return;
            }
            if let Some(cancel) = validation_cancel.borrow_mut().take() {
                cancel.cancel();
            }
            ui.set_edit_revision(ui.get_edit_revision().wrapping_add(1));
            ui.set_edit_error(SharedString::new());
            ui.set_edit_validating(false);
            ui.set_edit_suggestions(ModelRc::default());
            let base = std::path::PathBuf::from(ui.get_edit_base().as_str());
            let buffer = std::path::PathBuf::from(text.as_str());
            let absolute = base.join(buffer).to_string_lossy().into_owned();
            // El worker recibe ruta absoluta, pero la respuesta conserva el borrador original.
            ctrl.borrow_mut().request_path_autocomplete_at(
                text.to_string(),
                absolute,
                std::time::Instant::now(),
            );
            start_timer();
        });
    }
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let timer = validation_timer.clone();
        let validation_cancel = validation_cancel.clone();
        ui.on_path_edit_commit(move |id, text| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if ui.get_edit_pane() != id {
                return;
            }
            timer.stop();
            if let Some(cancel) = validation_cancel.borrow_mut().take() {
                cancel.cancel();
            }
            ui.set_edit_validating(false);
            ctrl.borrow_mut().cancel_path_autocomplete();
            ui.set_edit_suggestions(ModelRc::default());
            ui.set_edit_revision(ui.get_edit_revision().wrapping_add(1));
            let revision = ui.get_edit_revision();
            let invalid = ctrl.borrow().config.t("path.edit.invalid");
            let cleaned = match naygo_core::path_input::normalize(text.as_str()) {
                Ok(path) => path,
                Err(_) => {
                    ui.set_edit_error(invalid.into());
                    return;
                }
            };
            let base = ui.get_edit_base().to_string();
            if base.is_empty() || ctrl.borrow().path_of(PaneId(id as u64)) != base {
                ui.invoke_path_edit_cancel(id);
                return;
            }
            let path = std::path::PathBuf::from(&base).join(cleaned);
            let job = match crate::path_validation::PathValidation::start(path) {
                Ok(job) => job,
                Err(_) => {
                    ui.set_edit_error(ctrl.borrow().config.t("path.edit.busy").into());
                    return;
                }
            };
            ui.set_edit_error(ctrl.borrow().config.t("path.edit.checking").into());
            *validation_cancel.borrow_mut() = Some(job.cancellation_token());
            ui.set_edit_validating(true);
            let weak_timer = Rc::downgrade(&timer);
            let ui_weak = ui.as_weak();
            let ctrl = ctrl.clone();
            let sync_layout = sync_layout.clone();
            let start_timer = start_timer.clone();
            // Capturamos panel, carpeta base y revisión. Cambiar foco no cambia el destino.
            // El timer no posee al Rc de sí mismo: no crea un ciclo que sobreviva a la ventana.
            timer.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(30),
                move || {
                    let stop = || {
                        if let Some(timer) = weak_timer.upgrade() {
                            timer.stop();
                        }
                    };
                    let Some(ui) = ui_weak.upgrade() else {
                        stop();
                        return;
                    };
                    if ui.get_edit_revision() != revision || ui.get_edit_pane() != id {
                        stop();
                        return;
                    }
                    if ctrl.borrow().path_of(PaneId(id as u64)) != base {
                        stop();
                        ui.invoke_path_edit_cancel(id);
                        return;
                    }
                    let Some(result) = job.poll(std::time::Instant::now()) else {
                        return;
                    };
                    stop();
                    ui.set_edit_validating(false);
                    match result {
                        Ok(path) => {
                            let navigated =
                                ctrl.borrow_mut().navigate_pane_to(PaneId(id as u64), path);
                            if navigated {
                                ui.invoke_path_edit_cancel(id);
                                start_timer();
                                sync_layout();
                            } else {
                                ui.set_edit_error(invalid.clone().into());
                            }
                        }
                        Err(crate::path_validation::ValidationError::Timeout) => {
                            ui.set_edit_error(ctrl.borrow().config.t("path.edit.timeout").into());
                        }
                        Err(_) => ui.set_edit_error(invalid.clone().into()),
                    }
                },
            );
        });
    }
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let timer = validation_timer.clone();
        let validation_cancel = validation_cancel.clone();
        ui.on_path_edit_cancel(move |id| {
            if let Some(ui) = ui_weak.upgrade() {
                if ui.get_edit_pane() != id {
                    return;
                }
                timer.stop();
                if let Some(cancel) = validation_cancel.borrow_mut().take() {
                    cancel.cancel();
                }
                ctrl.borrow_mut().cancel_path_autocomplete();
                ui.set_edit_revision(ui.get_edit_revision().wrapping_add(1));
                ui.set_edit_pane(-1);
                if ui.get_edit_toolbar() {
                    ui.set_edit_toolbar(false);
                    ui.set_path_refocus_revision(ui.get_path_refocus_revision().wrapping_add(1));
                }
                let c = ctrl.borrow();
                ui.set_toolbar_path_favorite(
                    c.active_id().is_some_and(|id| c.is_pane_dir_favorite(id)),
                );
                ui.set_edit_input_focused(false);
                ui.set_edit_validating(false);
                ui.set_edit_error(SharedString::new());
                ui.set_edit_suggestions(ModelRc::default());
            }
        });
    }
    // ★ favorito de la path-bar: anclar/quitar la carpeta de ese panel.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_toggle(move |id| {
            ctrl.borrow_mut().toggle_favorite_dir(PaneId(id as u64));
            sync_rows();
        });
    }
    // 📋 copiar la ruta de la carpeta de ese panel al portapapeles. Además del toast (Slint) y
    // el ✓ del ícono, se anuncia en la barra de estado (se restaura en la próxima interacción).
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_copy_path(move |id| {
            ctrl.borrow_mut().copy_pane_path(PaneId(id as u64));
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_status(ui.global::<Tr>().get_pathbar_copied());
            }
        });
    }
    {
        let ui_weak = ui.as_weak();
        ui.on_path_suggestion_clicked(move |id, name| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if ui.get_edit_pane() != id {
                return;
            }
            let (parent, _) =
                naygo_core::path_segments::split_edit_buffer(ui.get_edit_text().as_str());
            let completed = format!("{parent}{name}\\");
            ui.set_edit_text(completed.into());
            ui.set_edit_caret(ui.get_edit_text().len() as i32);
            ui.set_edit_caret_revision(ui.get_edit_caret_revision().wrapping_add(1));
            ui.invoke_path_edit_changed(id, ui.get_edit_text());
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_remove(move |path| {
            ctrl.borrow_mut()
                .remove_favorite(std::path::Path::new(path.as_str()));
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_pin_current(move || {
            ctrl.borrow_mut().toggle_favorite_active();
            sync_rows();
        });
    }
    // --- Árbol de favoritos editable (panel + menú ▾ del toolbar) ---
    {
        // Expandir/colapsar un grupo del árbol por su ruta de nombres (estado de UI, no persiste).
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_toggle_expand(move |name_path| {
            ctrl.borrow_mut().fav_toggle_expand(name_path.as_str());
            sync_rows();
        });
    }
    {
        // Crear un grupo nuevo: (parent-group-id serializado, nombre). Persiste y refresca.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_new_group(move |parent_gid, name| {
            ctrl.borrow_mut()
                .fav_new_group(parent_gid.as_str(), name.as_str());
            sync_rows();
        });
    }
    {
        // Renombrar un grupo: (group-id serializado, nombre nuevo). Persiste y refresca.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_rename_group(move |gid, name| {
            ctrl.borrow_mut()
                .fav_rename_group(gid.as_str(), name.as_str());
            sync_rows();
        });
    }
    {
        // Renombrar un favorito solo cambia su alias visible; la ruta permanece como identidad
        // estable para navegar, mover dentro de grupos y los atajos Ctrl+1..9.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_rename_favorite(move |path, alias| {
            ctrl.borrow_mut()
                .fav_rename_favorite(path.as_str(), alias.as_str());
            sync_rows();
        });
    }
    {
        // Eliminar un nodo: (is-group, group-id, path). Persiste y refresca.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_delete_node(move |is_group, gid, path| {
            ctrl.borrow_mut()
                .fav_delete_node(is_group, gid.as_str(), path.as_str());
            sync_rows();
        });
    }
    {
        // Mover un nodo: (is-group, src-group-id, src-path, dest-group-id; "" = raíz). Persiste.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_fav_move_node(move |is_group, src_gid, src_path, dest_gid| {
            ctrl.borrow_mut().fav_move_node(
                is_group,
                src_gid.as_str(),
                src_path.as_str(),
                dest_gid.as_str(),
            );
            sync_rows();
        });
    }
    {
        // "Mover a…": al abrir el submenú, recalcular los destinos VÁLIDOS para el nodo elegido
        // (Rust excluye el propio grupo y sus descendientes) y volcarlos en `fav-move-targets`.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_fav_build_move_targets(move |is_group, src_gid| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let targets: Vec<PathSeg> = ctrl
                .borrow()
                .fav_move_targets(is_group, src_gid.as_str())
                .into_iter()
                .map(|(label, gid)| PathSeg {
                    label: SharedString::from(label),
                    path: SharedString::from(gid),
                })
                .collect();
            ui.set_fav_move_targets(ModelRc::from(Rc::new(VecModel::from(targets))));
        });
    }
}
