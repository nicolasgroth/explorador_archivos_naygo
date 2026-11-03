// Naygo — el tick del timer de UI: drena workers, watchers, canales y repinta (30 ms).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// El timer drena listados de archivos + árbol + preview + operaciones + metadata + búsqueda +
// watchers de carpeta/dispositivos + drag&drop OLE + tray + hotkey global + instancia única +
// autocompletado async de la path-bar, y se apaga cuando todo está en reposo (0 trabajo). El
// preview cambia structs del PaneVm → en cada tick corre `sync_rows`. Extraído de `main.rs`
// sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::tray;
use crate::win_helpers::*;
use crate::workspace_ctrl::WorkspaceCtrl;
use crate::*;
use naygo_core::workspace::layout::Rect;
use naygo_core::workspace::PaneId;
use slint::{ModelRc, SharedString, TimerMode, VecModel};
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

/// Solo enruta el punto entregado por OLE. Ausencia o fallo de hit-test no autoriza a
/// elegir el panel activo: este puede apuntar a una carpeta ajena al gesto del usuario.
fn route_external_drop(
    ctrl: &mut WorkspaceCtrl,
    content: Option<(f32, f32)>,
    payload: &naygo_platform::drop_target::DropPayload,
) -> bool {
    content.is_some_and(|(x, y)| {
        ctrl.drop_at_with_staging(
            x,
            y,
            payload.move_,
            payload.copy_forced,
            payload.paths.clone(),
            payload.staging.clone(),
        )
    })
}

#[cfg(test)]
mod drop_routing_tests {
    use super::*;

    #[test]
    fn invalid_or_missing_coordinates_never_route_to_active_panel() {
        let active = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let file = source.path().join("fixture.txt");
        std::fs::write(&file, b"fixture").unwrap();
        let mut ctrl =
            WorkspaceCtrl::new_in(active.path().to_path_buf(), config.path().to_path_buf());
        ctrl.set_area(Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        });
        let payload = naygo_platform::drop_target::DropPayload {
            preparation: None,
            delivery_cancel: None,
            paths: vec![file.clone()],
            move_: false,
            copy_forced: true,
            screen_x: 0,
            screen_y: 0,
            staging: None,
            error: None,
        };
        assert!(!route_external_drop(&mut ctrl, None, &payload));
        assert!(!route_external_drop(
            &mut ctrl,
            Some((-50.0, -50.0)),
            &payload
        ));
        assert!(ctrl.pending_drop.is_none());
        assert!(!active.path().join("fixture.txt").exists());
        assert_eq!(std::fs::read(file).unwrap(), b"fixture");
    }
}

/// Dependencias que el tick necesita del arranque de `main` (canales, handles y flags
/// compartidos). Agruparlas evita una firma de 15 parámetros sueltos.
pub(crate) struct TickDeps {
    pub start_animation_timer: Rc<dyn Fn()>,
    pub ctrl: Rc<RefCell<WorkspaceCtrl>>,
    pub sync_rows: Rc<dyn Fn()>,
    pub sync_layout: Rc<dyn Fn()>,
    pub timer: Rc<slint::Timer>,
    pub apply_device_change: Rc<dyn Fn()>,
    pub waker: naygo_platform::dir_watch::Waker,
    pub ui_weak: slint::Weak<AppWindow>,
    pub drop_tx: std::sync::mpsc::Sender<naygo_platform::drop_target::DropPayload>,
    pub drop_rx: Rc<std::sync::mpsc::Receiver<naygo_platform::drop_target::DropPayload>>,
    pub drag_tx: std::sync::mpsc::Sender<naygo_platform::drop_target::DragHover>,
    pub drag_rx: Rc<std::sync::mpsc::Receiver<naygo_platform::drop_target::DragHover>>,
    pub drop_guard: Rc<RefCell<Option<naygo_platform::drop_target::DropTargetGuard>>>,
    /// Último intento de registro OLE: un guard inerte se reintenta, pero nunca
    /// en cada tick para no llenar el log mientras winit termina de realizar el HWND.
    pub drop_registration_attempt: Rc<Cell<Option<std::time::Instant>>>,
    pub tray: Rc<Option<tray::Tray>>,
    pub tray_active: Rc<Cell<bool>>,
    pub hotkey_id: Rc<std::cell::Cell<Option<u32>>>,
    pub geometry_restored: Rc<std::cell::Cell<bool>>,
    pub si_show_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub si_shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Se instala después de construir las closures de refresco; permite repintar la toolbar,
    /// galerías y picker cuando el worker entrega PNGs externos.
    pub icon_refresh: crate::DeferredCallback,
    pub pack_io: Rc<RefCell<crate::packs::PackIo>>,
    pub pack_result: crate::packs::PackOutcomeCallback,
    /// Reaplica globales de UI cuando el snapshot inicial de configuración llega del worker.
    pub startup_refresh: crate::DeferredCallback,
    /// Repinta la tira al terminar el worker de unidades o expulsión.
    pub drive_refresh: crate::DeferredCallback,
}

/// Muestra la confirmación genérica sólo después de que el probe worker confirmó destino libre.
fn show_drop_confirmation(
    ui_weak: &slint::Weak<AppWindow>,
    pd: &crate::workspace_ctrl::PendingDrop,
) {
    let Some(ui) = ui_weak.upgrade() else {
        return;
    };
    let tr = ui.global::<Tr>();
    let dest_name = pd
        .dest_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| pd.dest_dir.display().to_string());
    let more_tmpl = tr.get_drop_confirm_more().to_string();
    let items = pd.names_summary(|n| more_tmpl.replace("{n}", &n.to_string()));
    let template = if pd.is_move {
        tr.get_drop_confirm_move()
    } else {
        tr.get_drop_confirm_copy()
    };
    let body = template
        .replace("{items}", &items)
        .replace("{dest}", &dest_name);
    let confirm_label = if pd.is_move {
        tr.get_dlg_move()
    } else {
        tr.get_dlg_copy()
    };
    ui.set_message(MessageVm {
        kind: 3,
        level: 0,
        title: tr.get_drop_confirm_title(),
        body: body.into(),
        confirm_label,
        cancel_label: tr.get_dlg_cancel(),
        danger: false,
    });
    // OLE acaba de salir de su bucle modal: diferir el foco para que el primer clic active el botón.
    let uiw2 = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = uiw2.upgrade() {
            if let Some(hwnd) = naygo_hwnd(&ui) {
                naygo_platform::window::bring_to_front(hwnd);
            }
        }
    });
}

/// Construye `start_timer`: la factory que (re)arranca el timer de 30 ms con el tick completo.
pub(crate) fn build_start_timer(deps: TickDeps) -> Rc<dyn Fn()> {
    let TickDeps {
        start_animation_timer,
        ctrl,
        sync_rows,
        sync_layout,
        timer,
        apply_device_change,
        waker,
        ui_weak,
        drop_tx,
        drop_rx,
        drag_tx,
        drag_rx,
        drop_guard,
        drop_registration_attempt,
        tray,
        tray_active,
        hotkey_id,
        geometry_restored,
        si_show_requested,
        si_shutdown_requested,
        icon_refresh,
        pack_io,
        pack_result,
        startup_refresh,
        drive_refresh,
    } = deps;
    #[cfg(not(windows))]
    let _ = (hotkey_id, geometry_restored);
    let ctrl = ctrl.clone();
    let sync_rows = sync_rows.clone();
    let sync_layout = sync_layout.clone();
    let timer = timer.clone();
    let apply_device_change = apply_device_change.clone();
    let waker = waker.clone();
    let drop_tx = drop_tx.clone();
    let drop_rx = drop_rx.clone();
    let drag_tx = drag_tx.clone();
    let drag_rx = drag_rx.clone();
    let drop_guard = drop_guard.clone();
    let drop_registration_attempt = drop_registration_attempt.clone();
    let tray = tray.clone();
    // Solo se LEE el id del hotkey (en el tick, bajo cfg windows). El registro en sí lo
    // mantiene vivo el binding `global_hotkey_slot` del scope de `main`, no este closure.
    #[cfg(windows)]
    let hotkey_id = hotkey_id.clone();
    // Clone propio para esta factory: el original del scope de `main` lo necesita también
    // el bloque de `on_wake` (mismo one-shot compartido).
    #[cfg(windows)]
    let geometry_restored = geometry_restored.clone();
    Rc::new(move || {
        let start_animation_timer = start_animation_timer.clone();
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let sync_layout = sync_layout.clone();
        let timer2 = timer.clone();
        let waker = waker.clone();
        let apply_device_change = apply_device_change.clone();
        let ui_weak = ui_weak.clone();
        let drop_tx = drop_tx.clone();
        let drop_rx = drop_rx.clone();
        let drag_tx = drag_tx.clone();
        let drag_rx = drag_rx.clone();
        let drop_guard = drop_guard.clone();
        let drop_registration_attempt = drop_registration_attempt.clone();
        let tray = tray.clone();
        let tray_active = tray_active.clone();
        // Solo se clona `hotkey_id` para LEER si hay registro vivo en el tick (bajo cfg
        // windows). El registro lo mantiene vivo el binding `global_hotkey_slot` del scope de
        // `main` (vive hasta después de `ui.run()`, toda la sesión), NO este closure.
        #[cfg(windows)]
        let hotkey_id = hotkey_id.clone();
        // Flag "muéstrate" de la instancia única (lo marca el hilo vigilante cuando otra
        // instancia del exe avisó antes de salir).
        let si_show_requested = si_show_requested.clone();
        let si_shutdown_requested = si_shutdown_requested.clone();
        let icon_refresh = icon_refresh.clone();
        let pack_io = pack_io.clone();
        let pack_result = pack_result.clone();
        let startup_refresh = startup_refresh.clone();
        let drive_refresh = drive_refresh.clone();
        // One-shot de geometría guardada (se intenta en cada tick hasta aplicarla; barata:
        // un get() de Cell una vez consumida).
        #[cfg(windows)]
        let geometry_restored = geometry_restored.clone();
        timer.start(
            TimerMode::Repeated,
            std::time::Duration::from_millis(30),
            move || {
                let _tick_metric = crate::metrics::span("ui.tick");
                let now = std::time::Instant::now();
                // Cinturón de seguridad anti-reentrancia: el bucle modal de `DoDragDrop`
                // (arrastre OLE hacia afuera) corre dentro del mismo hilo de UI y RE-ENTRA
                // este timer mientras dura el arrastre. Si en ese momento ya hay un
                // `borrow_mut` de `ctrl` vivo más arriba en la pila, repintar aquí
                // reventaría con «already borrowed». Probamos con `try_borrow_mut`: si el
                // controlador está prestado, SALTAMOS este tick por completo (el siguiente
                // tick repinta; perder un frame de 30ms es invisible). El probe es solo de
                // liveness: soltamos el préstamo de inmediato.
                if ctrl.try_borrow_mut().is_err() {
                    return;
                }
                let startup_was_loading = ctrl
                    .try_borrow()
                    .is_ok_and(|controller| controller.startup_loading());
                let startup_applied = ctrl
                    .try_borrow_mut()
                    .is_ok_and(|mut controller| controller.pump_startup_load());
                let startup_finished = startup_was_loading
                    && ctrl
                        .try_borrow()
                        .is_ok_and(|controller| !controller.startup_loading());
                if startup_applied || startup_finished {
                    sync_rows();
                    sync_layout();
                    if let Some(refresh) = startup_refresh.borrow().as_ref() {
                        refresh();
                    }
                }
                // Registrar el destino de drop OLE una sola vez, cuando el HWND ya es válido
                // (primer tick con la ventana realizada). El guard vive toda la sesión.
                let retry_due = drop_registration_attempt.get().is_none_or(|last| {
                    now.duration_since(last) >= std::time::Duration::from_secs(1)
                });
                let needs_registration = drop_guard
                    .borrow()
                    .as_ref()
                    .is_none_or(|guard| !guard.is_registered());
                if needs_registration && retry_due {
                    drop_registration_attempt.set(Some(now));
                    if let Some(ui) = ui_weak.upgrade() {
                        if let Some(hwnd) = naygo_hwnd(&ui) {
                            let g = naygo_platform::drop_target::register(
                                hwnd,
                                drop_tx.clone(),
                                drag_tx.clone(),
                                waker.clone(),
                            );
                            if g.is_registered() {
                                *drop_guard.borrow_mut() = Some(g);
                            }
                        }
                    }
                }
                // Drag&drop OLE — RECIBIR: archivos soltados sobre la ventana → copiar (o
                // mover) a la carpeta del panel que está BAJO el cursor. El payload trae el
                // punto del cursor en coords de PANTALLA (físicas); lo convertimos a coords de
                // CONTENIDO (el mismo sistema que usa pane_rects/drop_hit) y enrutamos con
                // `drop_at`. Sirve tanto para drags intra-app (entre paneles) como para drags
                // desde el Explorador de Windows (ahora caen en el panel apuntado, no en el
                // activo).
                //
                // Conversión pantalla→contenido:
                //   cliente = ScreenToClient(hwnd, pantalla)    (físicos; el SO descuenta el
                //             marco y la barra de título nativos de un golpe)
                //   win_log = cliente / scale_factor            (a coords lógicas)
                //   content = win_log - (0, TOP_BAR_H)          (descontar la barra superior
                //             de Naygo, que SÍ vive dentro del área de cliente, sobre `content`)
                // El área de contenido tiene origen (0,0) bajo la barra (ver app-window.slint:
                // `content` empieza en y=TOP_BAR_H, x=0), así que en X no hay offset lateral.
                // ScreenToClient ya quitó el marco del SO: NO restar nada más por ese lado.
                // TOP_BAR_H = 34px lógicos (alto de la barra superior de Naygo).
                const TOP_BAR_H: f32 = 34.0;
                // HOVER del arrastre (resaltar el panel bajo el cursor: borde + título). Drenar
                // ANTES del drop. `Over{screen}` → coords de contenido (MISMA fórmula que el
                // drop) → `pane_at` (paneles Files o Bandeja) → `set_drag_over`. `Leave` (salir o
                // soltar) limpia. Coalescemos a la ÚLTIMA posición del lote: el SO dispara
                // `DragOver` muchísimo y solo importa dónde está el cursor AHORA. `set_drag_over`
                // solo marca cambio si el panel difiere, así no re-pintamos en cada movimiento.
                // RE-ENTRANCIA: cada acceso a `ctrl` va en su propio statement (el `Ref`/`RefMut`
                // temporal muere al terminar la línea) para no solapar borrow con borrow_mut.
                // Si un arrastre acaba de terminar, el bucle modal de DoDragDrop se tragó el
                // key-release de Shift/Ctrl → quedaron pegados en el controlador. Limpiarlos
                // aquí (hilo de UI, con acceso a `ctrl`) evita que el siguiente clic/drag haga
                // una selección por rango fantasma. Flag de un solo uso: se baja tras limpiar.
                if let Some(ui) = ui_weak.upgrade() {
                    if ui.get_drag_just_ended() {
                        ui.set_drag_just_ended(false);
                        ctrl.borrow_mut().clear_modifiers();
                    }
                }
                {
                    let mut last: Option<naygo_platform::drop_target::DragHover> = None;
                    while let Ok(msg) = drag_rx.try_recv() {
                        last = Some(msg);
                    }
                    if let Some(msg) = last {
                        let new_over = match msg {
                            naygo_platform::drop_target::DragHover::Leave => None,
                            naygo_platform::drop_target::DragHover::Over { screen_x, screen_y } => {
                                ui_weak.upgrade().and_then(|ui| {
                                    let client = naygo_hwnd(&ui).and_then(|hwnd| {
                                        naygo_platform::window::screen_to_client(
                                            hwnd, screen_x, screen_y,
                                        )
                                    });
                                    client.and_then(|(client_x, client_y)| {
                                        let scale = ui.window().scale_factor().max(0.01);
                                        let cx = client_x as f32 / scale;
                                        let cy = client_y as f32 / scale - TOP_BAR_H;
                                        // `pane_at` es &self; el Ref temporal muere al ligar el let.
                                        ctrl.borrow()
                                            .pane_at(cx, cy)
                                            .map(|pane| (pane, client_y as f32 / scale))
                                    })
                                })
                            }
                        };
                        // Además del panel, refrescamos la Y que consume FilePanel para hit-test
                        // de filas D-1. El borrow_mut va solo, tras soltar el borrow de arriba.
                        let (pane, client_y) = new_over
                            .map(|(pane, y)| (Some(pane), Some(y)))
                            .unwrap_or((None, None));
                        let changed = ctrl.borrow_mut().set_drag_over_at(pane, client_y);
                        if changed {
                            sync_rows();
                        }
                    }
                }
                while let Ok(mut payload) = drop_rx.try_recv() {
                    if let Some(preparation) = payload.preparation.take() {
                        let mut controller = ctrl.borrow_mut();
                        let label = controller.config.t("ops.preparing_drop");
                        controller.ops.start_drop_preparation(preparation, label);
                        continue;
                    }
                    if payload
                        .delivery_cancel
                        .as_ref()
                        .is_some_and(|token| token.is_cancelled())
                    {
                        continue;
                    }
                    if let Some(error) = payload.error.as_ref() {
                        let base = ctrl.borrow().config.t("drop.virtual_failed");
                        let key = match error.as_str() {
                            naygo_platform::drop_target::DROP_EMPTY => Some("drop.empty"),
                            naygo_platform::drop_target::DROP_BAD_MEDIUM => Some("drop.bad_medium"),
                            naygo_platform::drop_target::DROP_GETDATA_FAILED => {
                                Some("drop.getdata_failed")
                            }
                            _ => None,
                        };
                        let detail = key
                            .map(|key| ctrl.borrow().config.t(key))
                            .unwrap_or_else(|| error.clone());
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.invoke_show_toast(format!("{base}: {detail}").into());
                        }
                        crate::logging::log_line(&format!(
                            "drop virtual no materializado: {error}"
                        ));
                        continue;
                    }
                    let mut routed = false;
                    if let Some(ui) = ui_weak.upgrade() {
                        // Tras el bucle modal de `DoDragDrop` (OLE), la ventana de Naygo deja
                        // de ser la foreground del SO: el primer clic en el modal de conflicto
                        // que pueda levantar `drop_at` solo reactivaría la ventana en vez de
                        // accionar el botón. La traemos al frente AQUÍ, antes de procesar el
                        // drop, para que el modal aparezca con la ventana ya enfocada y el
                        // primer clic vaya al botón. No toca `ctrl`, así que no hay riesgo de
                        // doble-préstamo.
                        if let Some(hwnd) = naygo_hwnd(&ui) {
                            naygo_platform::window::bring_to_front(hwnd);
                        }
                        // Punto del drop en coords de CLIENTE (físicas) vía Win32. Si no hay
                        // HWND o ScreenToClient falla, no se puede determinar el destino.
                        let client = naygo_hwnd(&ui).and_then(|hwnd| {
                            naygo_platform::window::screen_to_client(
                                hwnd,
                                payload.screen_x,
                                payload.screen_y,
                            )
                        });
                        let content = client.map(|(client_x, client_y)| {
                            let scale = ui.window().scale_factor().max(0.01);
                            let cx = client_x as f32 / scale;
                            let cy = client_y as f32 / scale - TOP_BAR_H;
                            (cx, cy)
                        });
                        routed = route_external_drop(&mut ctrl.borrow_mut(), content, &payload);
                    }
                    // Nunca redirigir al panel activo: podría apuntar a una carpeta distinta de
                    // la que el usuario eligió. Rechazar también los drops sin coordenadas.
                    if !routed {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.invoke_show_toast(
                                ctrl.borrow().config.t("drop.invalid_target").into(),
                            );
                        }
                    }
                    // Tras soltar, el resaltado de arrastre debe irse aunque el `Leave` del
                    // OLE no haya llegado/procesado todavía (defensa: que el borde no se quede
                    // pegado). Si ya estaba limpio, `set_drag_over` no marca cambio.
                    // Separa la mutación del render: sync_rows vuelve a tomar `ctrl` y debe
                    // ejecutarse después de que este RefMut quede fuera de alcance.
                    let drag_over_cleared = { ctrl.borrow_mut().set_drag_over(None) };
                    if drag_over_cleared {
                        sync_rows();
                    }
                    // CONFIRMAR AL SOLTAR (PUNTO 1b): si `drop_at` dejó un drop pendiente (intra-app
                    // entre paneles), NO se ejecutó todavía. Abrimos el modal de confirmación
                    // "¿Copiar/Mover N a «destino»?". Al confirmar, `on_message_confirm` (kind 3)
                    // llama a `confirm_pending_drop`; al cancelar, `cancel_pending_drop` lo descarta.
                    let pending = ctrl.borrow_mut().take_pending_drop_confirmation();
                    // Los destinos lógicos como la bandeja se resuelven de inmediato y no crean
                    // pendiente ni probe. Los drops a Files esperan el resultado del worker.
                    let direct_drop_ran = routed
                        && pending.is_none()
                        && ctrl.borrow().pending_drop.is_none()
                        && ctrl.borrow().drop_collision_rx.is_none();
                    // Un drop directo también puede haber mutado solo la bandeja temporal. En
                    // ambos casos sincronizar filas ahora evita que el panel parezca parpadear o
                    // quede vacío hasta el siguiente evento periódico.
                    if direct_drop_ran {
                        sync_rows();
                    }
                    if let Some(pd) = pending {
                        show_drop_confirmation(&ui_weak, &pd);
                    } else if direct_drop_ran {
                        // La bandeja u otro destino lógico pudo mutar el modelo de filas.
                        sync_layout();
                        let uiw2 = ui_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = uiw2.upgrade() {
                                if let Some(hwnd) = naygo_hwnd(&ui) {
                                    naygo_platform::window::bring_to_front(hwnd);
                                }
                            }
                        });
                    }
                }
                // La preconsulta se drena en ticks posteriores. Sólo entonces se abre la
                // confirmación genérica o se lanza el drop en conflicto directamente.
                // `Started` sincroniza filas y layout, closures que vuelven a prestar `ctrl`.
                // Guardar el evento en una sentencia propia libera el RefMut antes del match.
                let drop_probe_event = { ctrl.borrow_mut().pump_drop_collision_probe() };
                match drop_probe_event {
                    crate::workspace_ctrl::DropProbeEvent::Confirmation => {
                        if let Some(pd) = ctrl.borrow_mut().take_pending_drop_confirmation() {
                            show_drop_confirmation(&ui_weak, &pd);
                        }
                    }
                    crate::workspace_ctrl::DropProbeEvent::Started => {
                        sync_rows();
                        sync_layout();
                        let uiw2 = ui_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = uiw2.upgrade() {
                                if let Some(hwnd) = naygo_hwnd(&ui) {
                                    naygo_platform::window::bring_to_front(hwnd);
                                }
                            }
                        });
                    }
                    crate::workspace_ctrl::DropProbeEvent::Idle
                    | crate::workspace_ctrl::DropProbeEvent::Waiting
                    | crate::workspace_ctrl::DropProbeEvent::Failed => {}
                }
                // Tray (F5E): drenar los mensajes del ícono de bandeja. Abrir = mostrar y
                // elevar la ventana; Salir = terminar el bucle de verdad.
                if let Some(t) = tray.as_ref() {
                    while let Ok(msg) = t.rx.try_recv() {
                        match msg {
                            #[cfg(target_os = "linux")]
                            tray::TrayMsg::Availability(available) => {
                                // Linux crea y controla el indicador en un worker GLib; solo el
                                // resultado asíncrono puede autorizar ocultar la ventana.
                                tray_active
                                    .set(available && ctrl.borrow().config.settings.tray_enabled);
                                if !available {
                                    crate::logging::log_line(
                                        "[tray] backend GLib no disponible; cierre seguirá visible",
                                    );
                                }
                            }
                            tray::TrayMsg::Open => {
                                if let Some(ui) = ui_weak.upgrade() {
                                    restore_window(&ui);
                                }
                            }
                            tray::TrayMsg::NewPane => {
                                // Traer al frente y abrir un panel nuevo (divide el activo).
                                let area = if let Some(ui) = ui_weak.upgrade() {
                                    restore_window(&ui);
                                    Rect {
                                        x: 0.0,
                                        y: 0.0,
                                        w: ui.get_content_w().max(0.0),
                                        h: ui.get_content_h().max(0.0),
                                    }
                                } else {
                                    Rect {
                                        x: 0.0,
                                        y: 0.0,
                                        w: 0.0,
                                        h: 0.0,
                                    }
                                };
                                ctrl.borrow_mut().add_pane_split(area);
                                sync_layout();
                            }
                            tray::TrayMsg::OpenConfig => {
                                // Reusa el handler del engranaje de la toolbar (refresca el VM
                                // y muestra la ventana de config), así no duplicamos lógica.
                                if let Some(ui) = ui_weak.upgrade() {
                                    restore_window(&ui);
                                    ui.invoke_open_config();
                                }
                            }
                            tray::TrayMsg::CenterWindow => {
                                // Rescatar una ventana "perdida": restaurar (mostrar, devolver
                                // botón de barra, des-minimizar, al frente) y reposicionar a una
                                // esquina segura siempre visible (80,80).
                                if let Some(ui) = ui_weak.upgrade() {
                                    restore_window(&ui);
                                    ui.window()
                                        .set_position(slint::LogicalPosition::new(80.0, 80.0));
                                }
                            }
                            tray::TrayMsg::Exit => {
                                let tickets = {
                                    let c = ctrl.borrow();
                                    c.config.save();
                                    let mut tickets = c.config.persistence_tickets();
                                    match c.save_session_async() {
                                        Ok(ticket) => tickets.push(ticket),
                                        Err(error) => crate::logging::log_line(&format!(
                                            "no se pudo encolar la sesión al salir: {error}"
                                        )),
                                    }
                                    tickets
                                };
                                // Quitar el ícono de la bandeja ANTES de salir, para que no
                                // quede "fantasma" hasta que Windows lo repinte al pasar el
                                // mouse. `Drop` al terminar el proceso no basta (no es síncrono).
                                if let Some(t) = tray.as_ref() {
                                    t.hide_icon();
                                }
                                tray::quit_after_persistence(tickets);
                            }
                        }
                    }
                }
                // Hotkey global: ¿se presionó? Muestra y trae Naygo al frente. El flag lo
                // marca el handler del crate (que además despertó este loop). Se CONSUME
                // SIEMPRE (aunque el atajo esté desactivado): un press residual no debe
                // quedar pendiente y disparar la ventana al re-activar el atajo en Config.
                // `hotkey_id` confirma que hay un registro vivo antes de actuar.
                #[cfg(windows)]
                {
                    let pressed = naygo_platform::global_hotkey::was_pressed();
                    if pressed && hotkey_id.get().is_some() {
                        if let Some(ui) = ui_weak.upgrade() {
                            toggle_window_visibility(&ui, tray_active.get());
                        }
                    }
                }
                // Instancia única: ¿otra instancia del exe pidió "muéstrate"? (el usuario
                // volvió a lanzar Naygo desde el ícono anclado o con "Abrir en Naygo").
                // Restaurar la ventana y, si dejó una carpeta en el spool, abrirla en un
                // panel NUEVO (split del activo), sin perder lo que el usuario tenía.
                if si_shutdown_requested.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    let tickets = {
                        let c = ctrl.borrow();
                        c.config.save();
                        let mut tickets = c.config.persistence_tickets();
                        tickets.extend(c.additional_persistence_tickets());
                        match c.save_session_async() {
                            Ok(ticket) => tickets.push(ticket),
                            Err(error) => crate::logging::log_line(&format!(
                                "no se pudo encolar la sesión al cerrar instancia: {error}"
                            )),
                        }
                        tickets
                    };
                    if let Some(t) = tray.as_ref() {
                        t.hide_icon();
                    }
                    tray::quit_after_persistence(tickets);
                } else if si_show_requested.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    if let Some(ui) = ui_weak.upgrade() {
                        restore_window(&ui);
                        while let Some(dir) = naygo_platform::single_instance::take_open_request() {
                            let area = Rect {
                                x: 0.0,
                                y: 0.0,
                                w: ui.get_content_w().max(0.0),
                                h: ui.get_content_h().max(0.0),
                            };
                            ctrl.borrow_mut().open_dir_in_new_pane(dir, area);
                        }
                        sync_layout();
                    }
                }
                // Geometría guardada: aplicarla apenas la ventana esté visible (una vez por
                // sesión). Intentarlo desde el tick garantiza ≤30 ms tras cualquier
                // restauración/show — el salto a la posición guardada es imperceptible (el
                // on_wake solo lo disparan los watchers y puede tardar minutos).
                #[cfg(windows)]
                if let Some(ui) = ui_weak.upgrade() {
                    try_restore_saved_geometry(&ui, &ctrl, &geometry_restored);
                }
                // Watcher de dispositivos (F5B): si cambiaron las unidades (USB), reubicar
                // los paneles cuya carpeta desapareció, re-listarlos y refrescar la tira de
                // discos. La misma lógica corre en `on_wake` (refresh en vivo).
                apply_device_change();
                let drives_changed = ctrl.borrow_mut().pump_drive_refresh(waker.clone());
                if drives_changed {
                    if let Some(refresh) = drive_refresh.borrow().as_ref() {
                        refresh();
                    }
                    sync_rows();
                    sync_layout();
                }
                if let Some(completion) = ctrl.borrow_mut().pump_eject_drive() {
                    let mut message = None;
                    if let Some(ui) = ui_weak.upgrade() {
                        let tr = ui.global::<Tr>();
                        match completion.outcome {
                            crate::workspace_ctrl::EjectOutcome::Ok => {
                                {
                                    let mut c = ctrl.borrow_mut();
                                    for pane in completion.panes {
                                        c.mark_pane_ejected(pane);
                                    }
                                    c.refresh_missing_cache();
                                }
                                sync_rows();
                                sync_layout();
                                message = Some(tr.get_drive_eject_ok());
                            }
                            crate::workspace_ctrl::EjectOutcome::InUse => {
                                message = Some(tr.get_drive_eject_in_use_external());
                            }
                            crate::workspace_ctrl::EjectOutcome::Failed(error) => {
                                crate::logging::log_line(&format!(
                                    "expulsar USB {}: fallo — {error}",
                                    completion.root.display()
                                ));
                                message = Some(tr.get_drive_eject_failed());
                            }
                        }
                    }
                    if let Some(refresh) = drive_refresh.borrow().as_ref() {
                        refresh();
                    }
                    if let (Some(ui), Some(message)) = (ui_weak.upgrade(), message) {
                        ui.invoke_show_toast(message);
                    }
                }
                // Asegurar que cada panel Files vigile su carpeta actual (barato si nada
                // cambió). Arranca/re-arranca watchers tras navegar/agregar/cerrar paneles.
                ctrl.borrow_mut().reconcile_watchers(waker.clone());
                let mut listing_budget = crate::listing::PollBudget::new();
                // Si ambos grupos compiten, reserva una mitad para cada uno; si sólo uno está
                // activo puede usar todo el presupuesto. Cada controlador rota el primer worker.
                let (mut files_allowance, mut tree_allowance) = {
                    let ctrl = ctrl.borrow();
                    match (
                        ctrl.has_file_listing_work(),
                        !ctrl.tree_listings.is_empty() || !ctrl.pending_tree_listings.is_empty(),
                    ) {
                        (true, true) => (512, 512),
                        (true, false) => (1_024, 0),
                        (false, true) => (0, 1_024),
                        (false, false) => (0, 0),
                    }
                };
                let files_done = ctrl
                    .borrow_mut()
                    .pump_listings_with_budget(&mut listing_budget, &mut files_allowance);
                let tree_done = ctrl
                    .borrow_mut()
                    .pump_tree_with_budget(&mut listing_budget, &mut tree_allowance);
                let preview_window_visible = ui_weak.upgrade().is_some_and(|ui| {
                    #[cfg(windows)]
                    {
                        naygo_hwnd(&ui).is_some_and(naygo_platform::window::content_visible)
                    }
                    #[cfg(not(windows))]
                    {
                        ui.window().is_visible()
                    }
                });
                let preview_busy = ctrl
                    .borrow_mut()
                    .drive_preview_visible(now, preview_window_visible);
                let preview_ready = ctrl.borrow_mut().preview.poll().is_some();
                start_animation_timer();
                let _ = preview_ready;
                // Autocompletado async de la path-bar (por tecla): arrancar el worker vencido
                // el debounce y aplicar su resultado SOLO si el editor sigue abierto y con el
                // mismo buffer (un resultado tardío de un tipeo anterior se descarta).
                // Drenar ANTES de decidir reposo: si acaba una consulta vencida y hay un
                // borrador nuevo, el mismo tick debe lanzar/dejar pendiente la última petición.
                if let Some((buffer, sugg)) = ctrl.borrow_mut().poll_autocomplete() {
                    if let Some(ui) = ui_weak.upgrade() {
                        if ui.get_edit_pane() >= 0 && ui.get_edit_text().as_str() == buffer {
                            ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                                sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
                            ))));
                        }
                    }
                }
                let ac_busy = ctrl.borrow_mut().drive_autocomplete(now);
                // Autocompletado de «Buscar desde»: canal separado para que el resultado nunca
                // aparezca en la path-bar (ni viceversa). Se aplica solo al buffer vigente.
                if let Some((buffer, sugg)) = ctrl.borrow_mut().poll_search_autocomplete() {
                    if let Some(ui) = ui_weak.upgrade() {
                        if ui.get_search_root_buffer().as_str() == buffer {
                            ui.set_search_root_suggestions(ModelRc::from(Rc::new(VecModel::from(
                                sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
                            ))));
                        }
                    }
                }
                let search_ac_busy = ctrl.borrow_mut().drive_search_autocomplete(now);
                // Probe async de "carpeta no encontrada": aplicar los resultados al caché.
                let missing_done = ctrl.borrow_mut().pump_missing_probe();
                let icon_pump = ctrl.borrow_mut().icons.pump_external();
                for error in icon_pump.errors {
                    crate::logging::log_line(&format!(
                        "no se pudo cargar un ícono externo: {error}"
                    ));
                }
                if icon_pump.changed {
                    if let Some(refresh) = icon_refresh.borrow().as_ref() {
                        refresh();
                    }
                }
                // Importar/exportar/comprimir corre en el worker serial; aquí sólo se aplican
                // los resultados ya listos y se actualizan los modelos/catálogos.
                let pack_outcomes = pack_io.borrow().drain();
                for outcome in pack_outcomes {
                    if let Some(apply) = pack_result.borrow().as_ref() {
                        apply(outcome);
                    }
                }
                let paste_done = ctrl.borrow_mut().pump_paste_probe();
                let basket_clipboard_done = ctrl.borrow_mut().pump_basket_clipboard();
                if let Some(error) = ctrl.borrow_mut().take_paste_probe_error() {
                    crate::logging::log_line(&format!(
                        "consulta de destino al pegar falló: {error}"
                    ));
                    let message = ctrl.borrow().config.t("ops.plan_failed");
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.invoke_show_toast(message.into());
                    }
                }
                // Drenar el progreso de las operaciones de archivo (F3).
                let ops_done = ctrl.borrow_mut().ops.pump_ops();
                ctrl.borrow_mut().reconcile_completed_operations();
                if let Some(error) = ctrl.borrow_mut().ops.take_clipboard_error() {
                    crate::logging::log_line(&format!("operación de portapapeles falló: {error}"));
                    let message = ctrl.borrow().config.t("ops.plan_failed");
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.invoke_show_toast(format!("{message}: {error}").into());
                    }
                }
                let sync_done = ctrl.borrow_mut().pump_sync();
                let text_transform_done = ctrl.borrow_mut().pump_text_transform();
                // Extraer el error en una sentencia separada. Un `if let` sobre `borrow_mut()`
                // mantiene vivo el préstamo mutable durante TODO el cuerpo, por lo que leer la
                // configuración para el toast provocaba `RefCell already mutably borrowed`.
                let plan_error = ctrl.borrow_mut().ops.take_plan_error();
                if let Some(error) = plan_error {
                    let message = ctrl.borrow().config.t("ops.plan_failed");
                    // El texto genérico era insuficiente para drops de 7-Zip/WinRAR: la causa
                    // tipada incluye si el archivo temporal materializado dejó de poder leerse,
                    // o si el destino cae dentro del origen. Mantener ese detalle permite
                    // diagnosticar la fuente real sin hacer I/O adicional en el hilo de UI.
                    let detail = format!("{error:?}");
                    crate::logging::log_line(&format!("operación no preparada: {detail}"));
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.invoke_show_toast(format!("{message}: {detail}").into());
                    }
                }
                // Un pegado de texto/imagen cuya escritura async falló (disco lleno,
                // permisos, share caído): avisar con toast — antes el error se tragaba y el
                // usuario creía haber pegado.
                if let Some((_path, err)) = ctrl.borrow_mut().ops.take_paste_error() {
                    let base = ctrl.borrow().config.t("paste.error");
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.invoke_show_toast(format!("{base}: {err}").into());
                    }
                }
                if let Some(err) = ctrl.borrow_mut().take_shell_error() {
                    let base = ctrl.borrow().config.t("shell.error");
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.invoke_show_toast(format!("{base}: {err}").into());
                    }
                }
                if let Some(message) = ctrl.borrow_mut().pending_listing_error.take() {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.invoke_show_toast(message.into());
                    }
                }
                // Drenar el cálculo de tamaño de carpeta (F3 «calcular tamaño»).
                let size_done = ctrl.borrow_mut().pump_sizes();
                // Drenar la lectura de metadata por tipo del archivo enfocado (worker async).
                // `sync_rows` (más abajo) repuebla el VM cada tick. El estado que decide si el
                // timer puede dormir se re-lee DESPUÉS de `sync_rows` (que es quien puede LANZAR
                // un job nuevo al enfocar otro archivo): si aquí ya está drenado pero sync_rows
                // lanza uno, `meta_done` recalculado abajo lo detecta y mantiene el timer vivo.
                ctrl.borrow_mut().pump_meta();
                let zone_unblock_done = ctrl.borrow_mut().pump_zone_unblock();
                let basket_import_done = ctrl.borrow_mut().pump_basket_import();
                let delivery_done = ctrl.borrow_mut().pump_delivery();
                let spaces_was_busy = ctrl.borrow().task_spaces.busy();
                let spaces_done = ctrl.borrow_mut().pump_task_spaces();
                let retry_done = ctrl.borrow_mut().pump_retry();
                let queries_done = ctrl.borrow_mut().pump_saved_queries();
                let points_done = ctrl.borrow_mut().pump_comparison_points();
                let recipes_done = ctrl.borrow_mut().pump_recipes();
                if spaces_was_busy && spaces_done {
                    sync_layout();
                }
                // Drenar la búsqueda recursiva en vuelo (Ctrl+F / lupa).
                let search_done = ctrl.borrow_mut().pump_search();
                // Drenar el listado profundo en vuelo (vista profunda / toggle).
                let deep_changed = ctrl.borrow_mut().deep_poll();
                // Watcher de carpeta (F5A): aplicar los cambios detectados a cada panel y
                // marcar como nuevos los archivos recién aparecidos (para resaltarlos). La
                // metadata de cada evento YA viene resuelta del hilo del watcher (la UI no
                // hace syscalls por archivo cambiado).
                let batches = ctrl.borrow_mut().watchers.drain();
                for (pane, batch) in batches {
                    let nuevas = ctrl.borrow_mut().apply_watch_events_resolved(
                        PaneId(pane),
                        &batch.events,
                        &batch.resolved,
                    );
                    ctrl.borrow_mut().watchers.mark_fresh(pane, nuevas, now);
                }
                // Limpiar los resaltados vencidos y saber si queda alguno (para seguir
                // pintando hasta que se apaguen).
                let hl_secs = ctrl.borrow().highlight_secs();
                ctrl.borrow_mut().watchers.prune(hl_secs, now);
                // Reflejar la poda en el `highlighted` de cada panel: si la opción "archivos
                // nuevos al final" está activa, las filas que ya no están frescas dejan de
                // quedarse al final y vuelven a su orden normal.
                ctrl.borrow_mut()
                    .sync_highlighted_from_watchers(hl_secs, now);
                let fresh_pending = ctrl.borrow().watchers.any_fresh(hl_secs, now);
                sync_rows();
                // Re-leer el estado de la metadata DESPUÉS de sync_rows: puede haber lanzado un
                // job nuevo (al enfocar otro archivo). Si sigue leyendo, el timer no debe dormir.
                let meta_done = !ctrl.borrow().meta_loading();
                // Persistir la sesión si cambió (agregar/cerrar/navegar paneles). Barato
                // si no cambió. Antes de parar el timer, así el último cambio se guarda.
                ctrl.borrow_mut().maybe_persist_session();
                // ¿Hay algún modal/overlay abierto? Con el render por software y el modo bajo
                // consumo, si dejáramos dormir el timer aquí el event loop no procesaría más
                // eventos de mouse (hover/move) y los botones del modal no responderían hasta un
                // clic "de despertar". Mientras un modal esté en pantalla, el timer SIGUE VIVO.
                // Cubre los modales del controlador (`any_modal_open`) + los que viven en la UI:
                // el MessageModal (expulsar USB/errores: `MessageVm.kind != 0`) y la paleta de
                // comandos (`palette_open`). En reposo NORMAL (sin modal) este flag es false y el
                // timer se duerme igual que antes: el bajo consumo no se ve afectado.
                let modal_open = ctrl.borrow().any_modal_open()
                    || ui_weak
                        .upgrade()
                        .map(|ui| ui.get_message().kind != 0 || ui.get_palette_open())
                        .unwrap_or(false);
                // El watcher corre en su propio hilo y despierta la UI con el waker; el
                // timer puede dormir cuando no hay trabajo NI resaltados pendientes NI modales.
                if files_done
                    && tree_done
                    && !ctrl.borrow().has_file_listing_work()
                    && ctrl.borrow().tree_listings.is_empty()
                    && ctrl.borrow().pending_tree_listings.is_empty()
                    && !preview_busy
                    && !ac_busy
                    && !search_ac_busy
                    && missing_done
                    && paste_done
                    && basket_clipboard_done
                    && ops_done
                    && sync_done
                    && text_transform_done
                    && size_done
                    && meta_done
                    && zone_unblock_done
                    && basket_import_done
                    && ctrl.borrow().drop_collision_rx.is_none()
                    && delivery_done
                    && spaces_done
                    && retry_done
                    && queries_done
                    && points_done
                    && recipes_done
                    && search_done
                    && !deep_changed
                    && !fresh_pending
                    && !modal_open
                {
                    timer2.stop();
                }
            },
        );
    })
}
