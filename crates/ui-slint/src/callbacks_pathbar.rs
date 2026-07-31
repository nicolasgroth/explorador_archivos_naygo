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
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// Registra los callbacks de la path-bar y del árbol de favoritos editable.
pub(crate) fn wire_pathbar(ui: &AppWindow, ctx: &WireCtx) {
    let WireCtx {
        ctrl,
        sync_rows,
        sync_layout,
        start_timer,
        ..
    } = ctx;
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
        // Entrar a modo edición: cargar la ruta actual del panel y sus candidatos.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_path_edit_start(move |id| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let path = ctrl.borrow().path_of(PaneId(id as u64));
            let sugg = ctrl.borrow().path_autocomplete(&path);
            ui.set_edit_pane(id);
            ui.set_edit_text(path.into());
            ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
            ))));
        });
    }
    {
        // El texto del editor cambió: pedir el autocompletado ASYNC (worker con debounce).
        // Antes hacía un `read_dir` por tecla en el hilo de UI: contra un share de red caído
        // tipear una ruta congelaba la app. El tick entrega el resultado (ver el timer).
        let ctrl = ctrl.clone();
        let start_timer = start_timer.clone();
        let ui_weak = ui.as_weak();
        ui.on_path_edit_changed(move |_id, text| {
            ctrl.borrow_mut()
                .request_path_autocomplete(text.to_string(), std::time::Instant::now());
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_edit_text(text);
            }
            // Mantener el timer vivo para que el debounce venza y el resultado llegue.
            start_timer();
        });
    }
    {
        // Enter en el editor: navegar a la ruta tecleada (si existe como carpeta) y salir.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_path_edit_commit(move |id, text| {
            ctrl.borrow_mut().cancel_path_autocomplete();
            let dir = std::path::PathBuf::from(text.as_str());
            if dir.is_dir() && ctrl.borrow_mut().navigate_pane_to(PaneId(id as u64), dir) {
                start_timer();
            }
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_edit_pane(-1);
            }
            sync_layout();
        });
    }
    {
        // Esc: salir de edición sin navegar.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_path_edit_cancel(move |_id| {
            ctrl.borrow_mut().cancel_path_autocomplete();
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_edit_pane(-1);
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
            ctrl.borrow().copy_pane_path(PaneId(id as u64));
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_status(ui.global::<Tr>().get_pathbar_copied());
            }
        });
    }
    {
        // Clic en un candidato: completar el último segmento del editor y seguir editando.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_path_suggestion_clicked(move |_id, name| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let buffer = ui.get_edit_text().to_string();
            let (parent, _) = naygo_core::path_segments::split_edit_buffer(&buffer);
            // Completar: padre + nombre elegido + separador (listo para seguir bajando).
            let completed = format!("{parent}{name}\\");
            let sugg = ctrl.borrow().path_autocomplete(&completed);
            ui.set_edit_text(completed.into());
            ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
            ))));
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
