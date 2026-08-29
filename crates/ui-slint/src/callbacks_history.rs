// Naygo — cableado de callbacks de árbol, historial, menús de toolbar, discos y el
// dispatcher del modal temático (expulsar USB / confirmar drop / deshacer / cancelar todo).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Handlers de: panel de árbol, historial de carpetas (recientes + ▾ de atrás/adelante),
// menú del "ojo" (visibilidad), tira de unidades (navegar / expulsar / refrescar) y las
// confirmaciones del MessageModal (kinds 1/3/4/5). Extraídos de `main.rs` sin cambio de
// comportamiento (refactor por tamaño de archivo).

use crate::vm_builders::*;
use crate::win_helpers::*;
use crate::wire::WireCtx;
use crate::*;
use naygo_core::workspace::PaneId;
use slint::{ModelRc, VecModel};
use std::rc::Rc;

/// Registra los callbacks de árbol/historial/menús/discos y el dispatcher del MessageModal.
pub(crate) fn wire_history(ui: &AppWindow, ctx: &WireCtx, refresh_drives: &Rc<dyn Fn()>) {
    let WireCtx {
        ctrl,
        sync_rows,
        sync_layout,
        start_timer,
        pending_eject,
        pending_eject_panes,
        pending_undo,
        ..
    } = ctx;
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tree_toggle(move |id, path| {
            ctrl.borrow_mut()
                .tree_toggle(PaneId(id as u64), std::path::PathBuf::from(path.as_str()));
            start_timer();
            sync_layout();
        });
    }
    {
        // Clic derecho en el árbol: reutiliza el menú de carpeta del Files, con la ruta física
        // de la fila elegida. `last_active_files` conserva el destino correcto para abrir aquí.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_tree_context(move |id, path, x, y| {
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
    // Navegación por teclado del árbol (↑↓←→/Enter): mueve el cursor, expande/colapsa o navega.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tree_key(move |id, key| {
            ctrl.borrow_mut().tree_key(PaneId(id as u64), key.as_str());
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tree_navigate(move |id, path| {
            if ctrl
                .borrow_mut()
                .navigate_tree_to(PaneId(id as u64), std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tree_link_toggle(move |id| {
            if ctrl.borrow_mut().toggle_tree_link(PaneId(id as u64)) {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_nav_navigate(move |path| {
            if ctrl
                .borrow_mut()
                .navigate_active_to(std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
        });
    }
    // Ícono de historial de carpetas (reloj) en la toolbar: arma las filas y abre el menú flotante.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_history_open(move |x| {
            let rows: Vec<NavRow> = {
                let mut c = ctrl.borrow_mut();
                c.recents.remove_missing();
                c.recent_rows().into_iter().map(to_nav_row).collect()
            };
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_history_rows(ModelRc::new(VecModel::from(rows)));
                ui.set_history_menu_x(x);
                // Cerrar los demás menús para que nunca haya dos abiertos a la vez (discos +
                // historiales ▾ de Atrás/Adelante).
                ui.set_drive_menu_path("".into());
                ui.set_back_history_menu_open(false);
                ui.set_fwd_history_menu_open(false);
                ui.set_history_menu_open(true);
            }
        });
    }
    // Menú del "ojo" (visibilidad): abrir anclado al botón. Cierra los demás menús flotantes.
    {
        let ui_weak = ui.as_weak();
        ui.on_open_view_menu(move |x| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_view_menu_x(x);
                // No tener dos menús abiertos a la vez.
                ui.set_history_menu_open(false);
                ui.set_back_history_menu_open(false);
                ui.set_fwd_history_menu_open(false);
                ui.set_drive_menu_path("".into());
                ui.set_view_menu_open(true);
            }
        });
    }
    // Toggles del menú del "ojo": alternan el flag (persiste), re-arman los árboles (las
    // subcarpetas se re-listan filtradas) y refrescan la vista. Los paneles se refiltran solos en
    // el próximo `sync_rows`; el menú queda abierto para alternar varios de una.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_toggle_show_hidden(move || {
            {
                let mut c = ctrl.borrow_mut();
                let v = c.config.settings.show_hidden;
                c.config.set_show_hidden(!v);
                c.refresh_trees_visibility();
            }
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_toggle_show_system(move || {
            {
                let mut c = ctrl.borrow_mut();
                let v = c.config.settings.show_system;
                c.config.set_show_system(!v);
                c.refresh_trees_visibility();
            }
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_toggle_hide_dotfiles(move || {
            {
                let mut c = ctrl.borrow_mut();
                let v = c.config.settings.hide_dotfiles;
                c.config.set_hide_dotfiles(!v);
                c.refresh_trees_visibility();
            }
            start_timer();
            sync_rows();
        });
    }
    // Elegir una carpeta del menú de historial: navega el panel activo.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_history_pick(move |path| {
            if ctrl
                .borrow_mut()
                .navigate_active_to(std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
            sync_rows();
        });
    }
    // ▾ del botón Atrás: arma las carpetas hacia atrás del panel activo y abre el menú anclado.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_open_back_history(move |x| {
            let items: Vec<HistoryItemVm> = ctrl
                .borrow()
                .back_history_entries()
                .iter()
                .map(|p| path_to_history_item(p))
                .collect();
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_history_items(ModelRc::new(VecModel::from(items)));
                ui.set_back_history_menu_x(x);
                // No tener dos menús abiertos a la vez: cerrar adelante + recientes + discos.
                ui.set_fwd_history_menu_open(false);
                ui.set_history_menu_open(false);
                ui.set_drive_menu_path("".into());
                ui.set_back_history_menu_open(true);
            }
        });
    }
    // ▾ del botón Adelante: arma las carpetas hacia adelante del panel activo y abre el menú.
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_open_forward_history(move |x| {
            let items: Vec<HistoryItemVm> = ctrl
                .borrow()
                .forward_history_entries()
                .iter()
                .map(|p| path_to_history_item(p))
                .collect();
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_history_items(ModelRc::new(VecModel::from(items)));
                ui.set_fwd_history_menu_x(x);
                ui.set_back_history_menu_open(false);
                ui.set_history_menu_open(false);
                ui.set_drive_menu_path("".into());
                ui.set_fwd_history_menu_open(true);
            }
        });
    }
    // Elegir una entrada del menú ▾ de Atrás: el controlador traduce el índice del menú al de la
    // pila y salta sin re-apilar. Refresca el listado y los menús se cierran solos en la UI.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_back_history(move |menu_index| {
            if ctrl
                .borrow_mut()
                .go_back_history(menu_index.max(0) as usize)
            {
                start_timer();
            }
            sync_layout();
            sync_rows();
        });
    }
    // Elegir una entrada del menú ▾ de Adelante: ídem con la rama de adelante.
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_forward_history(move |menu_index| {
            if ctrl
                .borrow_mut()
                .go_forward_history(menu_index.max(0) as usize)
            {
                start_timer();
            }
            sync_layout();
            sync_rows();
        });
    }
    // Clic en una unidad de la tira de discos de la toolbar: navega el panel activo a su raíz.
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_nav_drive(move |path| {
            if ctrl
                .borrow_mut()
                .navigate_active_to(std::path::PathBuf::from(path.as_str()))
            {
                start_timer();
            }
            sync_layout();
        });
    }
    // Expulsar de forma segura una unidad extraíble (botón ⏏ o clic derecho sobre la unidad).
    // Es una llamada de un solo disparo (desmontar+expulsar, sin sondeo); en la práctica retorna
    // de inmediato. El resultado se anuncia con un toast localizado. NUNCA fuerza: si el volumen
    // está en uso, el toast lo dice y la unidad queda intacta. En éxito refrescamos la tira (la
    // unidad desaparece) — además el device_watch la detectará igual.
    {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let pending_eject = pending_eject.clone();
        let pending_eject_panes = pending_eject_panes.clone();
        let start_timer = start_timer.clone();
        ui.on_eject_drive(move |path| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            // Confirmación con el modal temático de Naygo (evita sacar una unidad por un clic
            // accidental). La expulsión real ocurre en on_message_confirm; aquí solo guardamos el
            // path pendiente y abrimos el modal.
            let tr = ui.global::<Tr>();
            // Detectar paneles con carpeta abierta en el disco a expulsar.
            let afectados: Vec<u64> = {
                let c = ctrl.borrow();
                c.panes_on_drive(std::path::Path::new(path.as_str()))
                    .into_iter()
                    .map(|(id, _)| id.0)
                    .collect()
            };
            *pending_eject_panes.borrow_mut() = afectados.clone();
            *pending_eject.borrow_mut() = Some(path.to_string());
            let (body, confirm_label) = if afectados.is_empty() {
                (
                    tr.get_drive_eject_confirm()
                        .replace("{drive}", path.as_str())
                        .into(),
                    tr.get_drive_eject(),
                )
            } else {
                let cuerpo: slint::SharedString = tr
                    .get_drive_eject_with_panes()
                    .replace("{drive}", path.as_str())
                    .replace("{n}", &afectados.len().to_string())
                    .into();
                (cuerpo, tr.get_drive_eject_anyway())
            };
            ui.set_message(MessageVm {
                kind: 1,
                level: 1, // warning
                title: tr.get_drive_eject_confirm_title(),
                body,
                confirm_label,
                cancel_label: tr.get_dlg_cancel(),
                danger: false,
            });
            // Modal abierto desde el mouse: rearmar el timer para que el popup responda al
            // instante (hover + primer clic), sin esperar un clic de despertar.
            start_timer();
        });
    }
    // Confirmación del modal temático: ejecuta la expulsión pendiente (kind 1) o el drop entre
    // paneles pendiente (kind 3). Es el dispatcher único de confirmaciones del MessageModal.
    {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let refresh_drives = refresh_drives.clone();
        let pending_eject = pending_eject.clone();
        let pending_eject_panes = pending_eject_panes.clone();
        let pending_undo = pending_undo.clone();
        let sync_layout = sync_layout.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_message_confirm(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let kind = ui.get_message().kind;
            // Cerrar el modal.
            ui.set_message(MessageVm::default());
            // kind 5 = confirmar DESHACER → ejecutar el undo de la entrada pendiente. El popup ya
            // mostró el detalle de lo que se hará; aquí recién se dispara.
            if kind == 5 {
                if let Some(id) = pending_undo.borrow_mut().take() {
                    if ctrl.borrow_mut().undo_entry(id) {
                        start_timer();
                    }
                    sync_rows();
                }
                return;
            }
            // kind 3 = confirmar drop entre paneles → arrancar la op real.
            if kind == 3 {
                let started = ctrl.borrow_mut().confirm_pending_drop();
                if started {
                    // Refrescar la disposición (puede haber aparecido el panel Operaciones) y
                    // rearmar el timer para que `pump_ops` drene el progreso de la op nueva.
                    sync_layout();
                    start_timer();
                    // BUG B (el modal de CONFLICTO no aparecía / aparecía "muerto"): la op puede
                    // chocar con un archivo que ya existe y `pump_ops` abrirá el modal de conflicto
                    // un par de ticks después. Como venimos de la cadena drop→DoDragDrop, la ventana
                    // arrastra el déficit de foreground del bucle modal OLE; la traemos al frente
                    // AHORA para que ese segundo modal reciba bien el primer clic (no uno de
                    // reactivación). `any_modal_open()` ya mantiene el timer vivo para el conflicto.
                    if let Some(hwnd) = naygo_hwnd(&ui) {
                        naygo_platform::window::bring_to_front(hwnd);
                    }
                }
                return;
            }
            // kind 4 = confirmar "cancelar todas las operaciones" → cancelar todas.
            if kind == 4 {
                ctrl.borrow_mut().ops.cancel_all_ops();
                // Rearmar el timer: `pump_ops` debe drenar el cierre de cada op cancelada.
                start_timer();
                return;
            }
            // kind 1 = confirmación de expulsar.
            if kind == 1 {
                // Soltar los watchers de los paneles que tienen el disco abierto, ANTES de
                // expulsar, para que el "en uso" no lo cause la propia app. Tras expulsar, esos
                // paneles mostrarán el aviso in-place "elegir carpeta" (pane_dir_missing lo detecta).
                let panes = std::mem::take(&mut *pending_eject_panes.borrow_mut());
                if !panes.is_empty() {
                    let mut c = ctrl.borrow_mut();
                    for pid in &panes {
                        c.release_pane_watcher(PaneId(*pid));
                    }
                }
                if let Some(path) = pending_eject.borrow_mut().take() {
                    let outcome = ctrl
                        .borrow()
                        .eject_drive(std::path::PathBuf::from(path.as_str()));
                    let tr = ui.global::<Tr>();
                    let msg = match outcome {
                        workspace_ctrl::EjectOutcome::Ok => {
                            // Marcar los paneles como "soltados por expulsión" SOLO si se expulsó
                            // de verdad (si falla, el disco sigue montado y no corresponde el texto).
                            {
                                let mut c = ctrl.borrow_mut();
                                for pid in &panes {
                                    c.mark_pane_ejected(PaneId(*pid));
                                }
                                // El disco ya no está: recalcular el caché para que esos paneles
                                // muestren el aviso "carpeta no encontrada" de inmediato.
                                c.refresh_missing_cache();
                            }
                            refresh_drives();
                            sync_layout();
                            tr.get_drive_eject_ok()
                        }
                        workspace_ctrl::EjectOutcome::InUse
                        | workspace_ctrl::EjectOutcome::Failed(_) => {
                            refresh_drives();
                            tr.get_drive_eject_in_use_external()
                        }
                    };
                    ui.invoke_show_toast(msg);
                }
            }
        });
    }
    // Cancelación del modal temático: cierra y descarta lo pendiente (path de expulsar y/o drop
    // entre paneles). Descartar el drop pendiente es lo seguro: no se copia ni mueve nada.
    {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let pending_eject = pending_eject.clone();
        let pending_eject_panes = pending_eject_panes.clone();
        let pending_undo = pending_undo.clone();
        ui.on_message_cancel(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            ui.set_message(MessageVm::default());
            pending_eject.borrow_mut().take();
            pending_eject_panes.borrow_mut().clear();
            // Cancelar el deshacer pendiente (si el popup era el de confirmar deshacer): NO se
            // ejecuta nada, solo se descarta el id guardado.
            pending_undo.borrow_mut().take();
            // Si había un drop entre paneles esperando confirmación, descartarlo (no-op si no había).
            ctrl.borrow_mut().cancel_pending_drop();
        });
    }
    // Refrescar unidades a mano (botón ⟳): re-escanea y reconstruye la tira. Útil para unidades
    // de red, que no disparan WM_DEVICECHANGE.
    {
        let refresh_drives = refresh_drives.clone();
        ui.on_refresh_drives(move || refresh_drives());
    }
}
