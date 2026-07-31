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
use std::cell::RefCell;
use std::rc::Rc;

/// Dependencias que el tick necesita del arranque de `main` (canales, handles y flags
/// compartidos). Agruparlas evita una firma de 15 parámetros sueltos.
pub(crate) struct TickDeps {
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
    pub tray: Rc<Option<tray::Tray>>,
    pub tray_active: bool,
    pub hotkey_id: Rc<std::cell::Cell<Option<u32>>>,
    pub geometry_restored: Rc<std::cell::Cell<bool>>,
    pub si_show_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Construye `start_timer`: la factory que (re)arranca el timer de 30 ms con el tick completo.
pub(crate) fn build_start_timer(deps: TickDeps) -> Rc<dyn Fn()> {
    let TickDeps {
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
        tray,
        tray_active,
        hotkey_id,
        geometry_restored,
        si_show_requested,
    } = deps;
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
        let tray = tray.clone();
        // Solo se clona `hotkey_id` para LEER si hay registro vivo en el tick (bajo cfg
        // windows). El registro lo mantiene vivo el binding `global_hotkey_slot` del scope de
        // `main` (vive hasta después de `ui.run()`, toda la sesión), NO este closure.
        #[cfg(windows)]
        let hotkey_id = hotkey_id.clone();
        // Flag "muéstrate" de la instancia única (lo marca el hilo vigilante cuando otra
        // instancia del exe avisó antes de salir).
        let si_show_requested = si_show_requested.clone();
        // One-shot de geometría guardada (se intenta en cada tick hasta aplicarla; barata:
        // un get() de Cell una vez consumida).
        #[cfg(windows)]
        let geometry_restored = geometry_restored.clone();
        timer.start(
            TimerMode::Repeated,
            std::time::Duration::from_millis(30),
            move || {
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
                // Registrar el destino de drop OLE una sola vez, cuando el HWND ya es válido
                // (primer tick con la ventana realizada). El guard vive toda la sesión.
                if drop_guard.borrow().is_none() {
                    if let Some(ui) = ui_weak.upgrade() {
                        if let Some(hwnd) = naygo_hwnd(&ui) {
                            let g = naygo_platform::drop_target::register(
                                hwnd,
                                drop_tx.clone(),
                                drag_tx.clone(),
                                waker.clone(),
                            );
                            *drop_guard.borrow_mut() = Some(g);
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
                // drop) → `pane_at` (solo paneles Files) → `set_drag_over`. `Leave` (salir o
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
                                        ctrl.borrow().pane_at(cx, cy)
                                    })
                                })
                            }
                        };
                        // Si cambió el panel resaltado, repintar (sync_rows lo refleja en cada
                        // PaneVm). El borrow_mut va solo, tras soltar el borrow de arriba.
                        let changed = ctrl.borrow_mut().set_drag_over(new_over);
                        if changed {
                            sync_rows();
                        }
                    }
                }
                while let Ok(payload) = drop_rx.try_recv() {
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
                        // HWND o ScreenToClient falla, caemos al fallback del panel activo.
                        let client = naygo_hwnd(&ui).and_then(|hwnd| {
                            naygo_platform::window::screen_to_client(
                                hwnd,
                                payload.screen_x,
                                payload.screen_y,
                            )
                        });
                        if let Some((client_x, client_y)) = client {
                            let scale = ui.window().scale_factor().max(0.01);
                            let cx = client_x as f32 / scale;
                            let cy = client_y as f32 / scale - TOP_BAR_H;
                            routed = ctrl.borrow_mut().drop_at(
                                cx,
                                cy,
                                payload.move_,       // move_hint (Shift del OLE)
                                payload.copy_forced, // copy_forced (Ctrl del OLE)
                                payload.paths.clone(),
                            );
                        }
                    }
                    // Fallback: si no se pudo enrutar por el punto (sin ventana, o el cursor
                    // no cayó sobre un panel Files), caer al panel activo como antes para no
                    // perder el drop.
                    if !routed {
                        // Extraer active_id() a un `let` propio para SOLTAR el Ref compartido
                        // ANTES del borrow_mut() de abajo. Si se hiciera el `if let` directo
                        // sobre `ctrl.borrow().active_id()`, el Ref del scrutinee vive durante
                        // todo el cuerpo del `if let` y choca con el borrow_mut() → panic
                        // "already borrowed" (mismo patrón que la ruta feliz ya evita arriba).
                        let active = ctrl.borrow().active_id();
                        if let Some(active) = active {
                            ctrl.borrow_mut().drop_external(
                                active,
                                payload.paths,
                                payload.move_,
                                payload.copy_forced,
                            );
                        }
                    }
                    // Tras soltar, el resaltado de arrastre debe irse aunque el `Leave` del
                    // OLE no haya llegado/procesado todavía (defensa: que el borde no se quede
                    // pegado). Si ya estaba limpio, `set_drag_over` no marca cambio.
                    if ctrl.borrow_mut().set_drag_over(None) {
                        sync_rows();
                    }
                    // CONFIRMAR AL SOLTAR (PUNTO 1b): si `drop_at` dejó un drop pendiente (intra-app
                    // entre paneles), NO se ejecutó todavía. Abrimos el modal de confirmación
                    // "¿Copiar/Mover N a «destino»?". Al confirmar, `on_message_confirm` (kind 3)
                    // llama a `confirm_pending_drop`; al cancelar, `cancel_pending_drop` lo descarta.
                    let pending = {
                        let c = ctrl.borrow();
                        c.pending_drop.clone()
                    };
                    // Con la confirmación de drop DESACTIVADA, `drop_at` arrancó la op directo
                    // y consumió el pendiente: `routed` es true pero `pending` quedó None.
                    // (Distinto de "cayó fuera de todo panel": ahí `routed` es false.)
                    let direct_drop_ran = routed && pending.is_none();
                    if let (Some(pd), Some(ui)) = (pending, ui_weak.upgrade()) {
                        let tr = ui.global::<Tr>();
                        let dest_name = pd
                            .dest_dir
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| pd.dest_dir.display().to_string());
                        // Nombres de lo que se va a copiar/mover (pedido de Nicolás): 1 →
                        // «a.txt»; pocos → «a.txt», «b.txt», …; muchos → primeros + " y N más"
                        // (el sufijo lo da la clave i18n `drop-confirm-more`, con {n} resuelto).
                        let more_tmpl = tr.get_drop_confirm_more().to_string();
                        let items = pd.names_summary(|n| more_tmpl.replace("{n}", &n.to_string()));
                        // Cuerpo: "¿Copiar/Mover <nombres> a «destino»?" — la plantilla i18n
                        // lleva {items} y {dest}; el verbo lo elige según copiar/mover.
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
                            kind: 3, // 3 = confirmar drop entre paneles (2 botones, velo=cancela)
                            level: 0,
                            title: tr.get_drop_confirm_title(),
                            body: body.into(),
                            confirm_label,
                            cancel_label: tr.get_dlg_cancel(),
                            danger: false,
                        });
                        // BUG A (doble clic en el modal kind 3): tras el bucle modal de
                        // `DoDragDrop` (OLE) la ventana de Naygo deja de ser la foreground del
                        // SO. Hacer `bring_to_front` AQUÍ (mismo tick que desenrolla el
                        // DoDragDrop) NO basta: el SO aún no terminó de soltar el foreground del
                        // arrastre, así que el primer clic se gasta reactivando la ventana. Por
                        // eso DIFERIMOS el `bring_to_front` un tick con `invoke_from_event_loop`
                        // —igual que `start_drag`—: cuando corre, el desenrollado del drag ya
                        // cerró y la ventana toma el foreground limpio, así el PRIMER clic va al
                        // botón. El timer sigue vivo porque el modal kind 3 mantiene
                        // `get_message().kind != 0`.
                        let uiw2 = ui_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = uiw2.upgrade() {
                                if let Some(hwnd) = naygo_hwnd(&ui) {
                                    naygo_platform::window::bring_to_front(hwnd);
                                }
                            }
                        });
                    } else if direct_drop_ran {
                        // Confirmación de drop DESACTIVADA o drop CON conflicto: `drop_at` ya
                        // arrancó la op directo (sin modal kind 3). Refrescar la disposición
                        // (pudo aparecer el panel Operaciones); el timer sigue vivo y `pump_ops`
                        // drenará el progreso y abrirá el modal de CONFLICTO si el archivo ya
                        // existía en el destino. Diferimos el `bring_to_front` un tick (igual
                        // que arriba) para que ESE modal (que llega un par de ticks después)
                        // reciba el foreground limpio y el primer clic vaya al botón.
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
                // Tray (F5E): drenar los mensajes del ícono de bandeja. Abrir = mostrar y
                // elevar la ventana; Salir = terminar el bucle de verdad.
                if let Some(t) = tray.as_ref() {
                    while let Ok(msg) = t.rx.try_recv() {
                        match msg {
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
                                ctrl.borrow().save_session();
                                // Quitar el ícono de la bandeja ANTES de salir, para que no
                                // quede "fantasma" hasta que Windows lo repinte al pasar el
                                // mouse. `Drop` al terminar el proceso no basta (no es síncrono).
                                if let Some(t) = tray.as_ref() {
                                    t.hide_icon();
                                }
                                let _ = slint::quit_event_loop();
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
                            toggle_window_visibility(&ui, tray_active);
                        }
                    }
                }
                // Instancia única: ¿otra instancia del exe pidió "muéstrate"? (el usuario
                // volvió a lanzar Naygo desde el ícono anclado o con "Abrir en Naygo").
                // Restaurar la ventana y, si dejó una carpeta en el spool, abrirla en un
                // panel NUEVO (split del activo), sin perder lo que el usuario tenía.
                if si_show_requested.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    if let Some(ui) = ui_weak.upgrade() {
                        restore_window(&ui);
                        if let Some(dir) = naygo_platform::single_instance::take_open_request() {
                            let area = Rect {
                                x: 0.0,
                                y: 0.0,
                                w: ui.get_content_w().max(0.0),
                                h: ui.get_content_h().max(0.0),
                            };
                            ctrl.borrow_mut().open_dir_in_new_pane(dir, area);
                            sync_layout();
                        }
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
                // Asegurar que cada panel Files vigile su carpeta actual (barato si nada
                // cambió). Arranca/re-arranca watchers tras navegar/agregar/cerrar paneles.
                ctrl.borrow_mut().reconcile_watchers(waker.clone());
                let files_done = ctrl.borrow_mut().pump_listings();
                let tree_done = ctrl.borrow_mut().pump_tree();
                let preview_busy = ctrl.borrow_mut().drive_preview(now);
                let preview_ready = ctrl.borrow_mut().preview.poll().is_some();
                let _ = preview_ready;
                // Autocompletado async de la path-bar (por tecla): arrancar el worker vencido
                // el debounce y aplicar su resultado SOLO si el editor sigue abierto y con el
                // mismo buffer (un resultado tardío de un tipeo anterior se descarta).
                let ac_busy = ctrl.borrow_mut().drive_autocomplete(now);
                if let Some((buffer, sugg)) = ctrl.borrow_mut().poll_autocomplete() {
                    if let Some(ui) = ui_weak.upgrade() {
                        if ui.get_edit_pane() >= 0 && ui.get_edit_text().as_str() == buffer {
                            ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                                sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
                            ))));
                        }
                    }
                }
                // Probe async de "carpeta no encontrada": aplicar los resultados al caché.
                let missing_done = ctrl.borrow_mut().pump_missing_probe();
                // Drenar el progreso de las operaciones de archivo (F3).
                let ops_done = ctrl.borrow_mut().ops.pump_ops();
                let plan_failed = ctrl.borrow_mut().ops.take_plan_error().is_some();
                if plan_failed {
                    let message = ctrl.borrow().config.t("ops.plan_failed");
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.invoke_show_toast(message.into());
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
                // Drenar el cálculo de tamaño de carpeta (F3 «calcular tamaño»).
                let size_done = ctrl.borrow_mut().pump_sizes();
                // Drenar la lectura de metadata por tipo del archivo enfocado (worker async).
                // `sync_rows` (más abajo) repuebla el VM cada tick. El estado que decide si el
                // timer puede dormir se re-lee DESPUÉS de `sync_rows` (que es quien puede LANZAR
                // un job nuevo al enfocar otro archivo): si aquí ya está drenado pero sync_rows
                // lanza uno, `meta_done` recalculado abajo lo detecta y mantiene el timer vivo.
                ctrl.borrow_mut().pump_meta();
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
                    && !preview_busy
                    && !ac_busy
                    && missing_done
                    && ops_done
                    && size_done
                    && meta_done
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
