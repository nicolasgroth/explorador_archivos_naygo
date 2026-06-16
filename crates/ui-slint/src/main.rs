// Naygo — arranque de la capa UI en Slint (Fase 2b: multi-panel + paneles especiales).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Subsistema GUI en release: sin ventana de consola negra al lanzar el .exe. En debug se
// conserva la consola para ver stderr/logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//
// Para forzar el renderizador por software (caso VM sin GPU):
//   $env:SLINT_BACKEND="winit-software"; cargo run -p naygo-ui-slint
//
// MODELOS ESTABLES (clave del rendimiento y de la corrección):
// Slint es modo retenido: un `for p in root.panes` recrea un panel por cada ELEMENTO del
// modelo. Si se reemplaza el VecModel entero en cada refresco, Slint destruye y recrea cada
// panel + sus ListView en cada tick → se pierde el scroll y se cortan los gestos. Por eso
// mantenemos modelos ESTABLES y los mutamos in situ:
//   - `panes`: un VecModel<PaneVm> que solo se reestructura cuando cambia la LISTA de
//     paneles o el ÁREA (agregar/quitar panel, resize).
//   - Por panel, según su tipo, un VecModel ESTABLE de filas (Files/Tree/Favoritos/
//     Recientes/Historial) que se actualiza con `set_vec` (mismo VecModel) → los ListView
//     conservan su scroll. Inspector/Preview son structs sueltas en el PaneVm.
// `sync_rows` (barato, en cada tick) actualiza el contenido. `sync_layout` (estructural)
// reconcilia la lista de paneles y splitters.
mod bridge;
mod config_ctrl;
mod devices;
mod i18n_keys;
mod icons;
mod keys;
mod listing;
mod ops_ctrl;
mod packs;
mod preview;
mod theme_apply;
mod tray;
mod watch;
mod workspace_ctrl;

use naygo_core::workspace::layout::{Rect, SplitDir};
use naygo_core::workspace::{PaneId, PanePurpose};
use slint::{Model, ModelRc, SharedPixelBuffer, SharedString, TimerMode, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use workspace_ctrl::WorkspaceCtrl;

slint::include_modules!();

/// Modelos de lista ESTABLES de un panel (solo el que aplica a su tipo se usa).
struct PaneModels {
    rows: Rc<VecModel<RowData>>,
    tree: Rc<VecModel<TreeRow>>,
    favs: Rc<VecModel<NavRow>>,
    recents: Rc<VecModel<NavRow>>,
    hist: Rc<VecModel<HistRow>>,
}

impl PaneModels {
    fn new() -> PaneModels {
        PaneModels {
            rows: Rc::new(VecModel::default()),
            tree: Rc::new(VecModel::default()),
            favs: Rc::new(VecModel::default()),
            recents: Rc::new(VecModel::default()),
            hist: Rc::new(VecModel::default()),
        }
    }
}

/// Modelos estables que persisten entre refrescos (ver nota de cabecera).
struct Models {
    panes: Rc<VecModel<PaneVm>>,
    splits: Rc<VecModel<SplitVm>>,
    /// Candidatos del selector de panel destino (vacío = sin selector).
    picks: Rc<VecModel<PickVm>>,
    /// Modelos de lista estables por panel (se actualizan in situ, no se recrean).
    per_pane: HashMap<PaneId, PaneModels>,
    /// IDs de panel VISIBLES en el orden actual del modelo `panes`.
    pane_ids: Vec<PaneId>,
    /// Grupos de pestañas con los que se construyó la estructura (para detectar cambios de
    /// agrupación que no alteran los ids visibles, p. ej. activar otra pestaña).
    groups: Vec<(Vec<PaneId>, usize)>,
    /// Área con la que se construyó la estructura actual (para detectar resize).
    area: Rect,
}

impl Models {
    fn new() -> Models {
        Models {
            panes: Rc::new(VecModel::default()),
            splits: Rc::new(VecModel::default()),
            picks: Rc::new(VecModel::default()),
            per_pane: HashMap::new(),
            pane_ids: Vec::new(),
            groups: Vec::new(),
            area: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
        }
    }

    fn models_for(&mut self, id: PaneId) -> &PaneModels {
        self.per_pane.entry(id).or_insert_with(PaneModels::new)
    }
}

fn rects_equal(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 0.5
        && (a.y - b.y).abs() < 0.5
        && (a.w - b.w).abs() < 0.5
        && (a.h - b.h).abs() < 0.5
}

fn purpose_to_int(p: PanePurpose) -> i32 {
    match p {
        PanePurpose::Files => 0,
        PanePurpose::Tree => 1,
        PanePurpose::Inspector => 2,
        PanePurpose::History => 3,
        PanePurpose::Favorites => 4,
        PanePurpose::Preview => 5,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let start = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
    let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new(start)));
    // Restaurar la sesión anterior (paneles y carpetas) si hay una guardada; si no, se
    // conserva el panel único del arranque por defecto.
    ctrl.borrow_mut().load_session();
    // Volcar los textos del idioma activo al global Tr (la UI arranca traducida).
    i18n_keys::apply(&ui, &ctrl.borrow().config);
    // Volcar los colores del tema activo al global Theme (la UI arranca con el tema guardado).
    theme_apply::apply(&ui, ctrl.borrow().config.active_theme());

    // Splash de arranque (Fase 5F): solo en release. Ventana breve de bienvenida que se cierra
    // sola a ~1.2s. La ventana principal se construye por detrás (el splash no la bloquea). En
    // debug se omite (arranque directo). Se mantiene vivo en una variable de la función `main`.
    // Nota: el Splash usa los colores POR DEFECTO del global Theme (azul marino), que coinciden
    // con el tema default — no hace falta aplicarle el tema activo (es una pantalla efímera).
    #[cfg(not(debug_assertions))]
    let _splash_keepalive = match Splash::new() {
        Ok(splash) => {
            let _ = splash.show();
            let splash = Rc::new(splash);
            let splash_for_timer = splash.clone();
            let timer = slint::Timer::default();
            timer.start(
                slint::TimerMode::SingleShot,
                std::time::Duration::from_millis(1200),
                move || {
                    let _ = splash_for_timer.hide();
                },
            );
            Some((splash, timer))
        }
        Err(_) => None,
    };

    let models = Rc::new(RefCell::new(Models::new()));

    ui.set_panes(ModelRc::from(models.borrow().panes.clone()));
    ui.set_splits(ModelRc::from(models.borrow().splits.clone()));
    ui.set_picks(ModelRc::from(models.borrow().picks.clone()));

    let area_of = {
        let ui_weak = ui.as_weak();
        move || {
            ui_weak
                .upgrade()
                .map(|ui| Rect {
                    x: 0.0,
                    y: 0.0,
                    w: ui.get_content_w().max(0.0),
                    h: ui.get_content_h().max(0.0),
                })
                .unwrap_or(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                })
        }
    };

    // Actualiza SOLO el contenido (filas + structs + flags) sin tocar la estructura. Barato:
    // corre en cada tick. Mantiene los mismos VecModel → los ListView conservan su scroll.
    let sync_rows = {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let models = models.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            // `borrow_mut` porque `rows_of` necesita mutar el IconCache (decodifica on-demand).
            let mut c = ctrl.borrow_mut();
            let active = c.active_id();
            let hl_secs = c.highlight_secs();
            let hl_now = std::time::Instant::now();
            // Datos compartidos (no dependen del panel concreto): favoritos, recientes,
            // historial, inspector, preview se derivan del estado global / panel activo.
            let favs: Vec<NavRow> = c.favorite_rows().into_iter().map(to_nav_row).collect();
            let recents: Vec<NavRow> = c.recent_rows().into_iter().map(to_nav_row).collect();
            let hist: Vec<HistRow> = c.history_rows().into_iter().map(to_hist_row).collect();
            let inspector = to_inspector_vm(c.inspector_info());

            let mut m = models.borrow_mut();
            for (i, &id) in m.pane_ids.clone().iter().enumerate() {
                let purpose = c.purpose_of(id);
                // Actualiza los modelos de lista que apliquen al tipo, in situ.
                match purpose {
                    Some(PanePurpose::Files) => {
                        let rows: Vec<RowData> = c
                            .rows_of(id, hl_secs, hl_now)
                            .into_iter()
                            .map(to_row_data)
                            .collect();
                        m.models_for(id).rows.set_vec(rows);
                    }
                    Some(PanePurpose::Tree) => {
                        let rows: Vec<TreeRow> =
                            c.tree_rows(id).into_iter().map(to_tree_row).collect();
                        m.models_for(id).tree.set_vec(rows);
                    }
                    Some(PanePurpose::Favorites) => {
                        m.models_for(id).favs.set_vec(favs.clone());
                        m.models_for(id).recents.set_vec(recents.clone());
                    }
                    Some(PanePurpose::History) => {
                        m.models_for(id).hist.set_vec(hist.clone());
                    }
                    _ => {}
                }
                // Actualiza los campos del PaneVm sin recrear el elemento.
                if let Some(mut pv) = m.panes.row_data(i) {
                    let is_active = Some(id) == active;
                    let path = SharedString::from(c.path_of(id).as_str());
                    let mut changed = false;
                    if pv.active != is_active {
                        pv.active = is_active;
                        changed = true;
                    }
                    if pv.path != path {
                        pv.path = path;
                        changed = true;
                    }
                    // Inspector/Preview son structs sueltas: se setean según el tipo.
                    if purpose == Some(PanePurpose::Inspector) {
                        pv.inspector = inspector.clone();
                        changed = true;
                    }
                    if purpose == Some(PanePurpose::Preview) {
                        pv.preview = current_preview_vm(&c);
                        changed = true;
                    }
                    if changed {
                        m.panes.set_row_data(i, pv);
                    }
                }
            }
            if let Some(id) = active {
                ui.set_active_path(SharedString::from(c.path_of(id).as_str()));
            }
            ui.set_status(SharedString::from(c.status_line().as_str()));
            // Operaciones de archivo (F3): modal activo + filas de progreso + retomar.
            ui.set_op_dialog(to_op_dialog_vm(c.ops.dialog_vm()));
            let op_rows: Vec<OpRowVm> = c.ops.op_rows().into_iter().map(to_op_row_vm).collect();
            ui.set_op_rows(ModelRc::from(Rc::new(VecModel::from(op_rows))));
            let resume_rows: Vec<ResumeRowVm> = c
                .ops
                .resume_rows()
                .into_iter()
                .map(|(id, label)| ResumeRowVm {
                    id: SharedString::from(id.as_str()),
                    label: SharedString::from(label.as_str()),
                })
                .collect();
            ui.set_resume_rows(ModelRc::from(Rc::new(VecModel::from(resume_rows))));
            // Menú contextual: posición + si hay menú nativo disponible (hay HWND).
            let ctx = match &c.context_menu {
                Some(cm) => ContextMenuVm {
                    active: true,
                    x: cm.x,
                    y: cm.y,
                    has_native: naygo_hwnd(&ui).is_some(),
                },
                None => ContextMenuVm {
                    active: false,
                    x: 0.0,
                    y: 0.0,
                    has_native: false,
                },
            };
            ui.set_ctx_menu(ctx);
        }
    };

    // Reconcilia la ESTRUCTURA (paneles + splitters) con el estado del core. Solo
    // reconstruye cuando cambia la lista de IDs o el área. Tras reestructurar, sincroniza.
    let sync_layout: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let models = models.clone();
        let area_of = area_of.clone();
        let sync_rows = sync_rows.clone();
        Rc::new(move || {
            let area = area_of();
            ctrl.borrow_mut().set_area(area);
            let pane_rects = ctrl.borrow().pane_rects(area);
            let split_handles = ctrl.borrow().split_handles(area);
            // Grupos de pestañas: solo se PINTA la pestaña activa de cada grupo (todas
            // comparten rect). Los miembros ocultos se filtran; al activo se le adjunta la
            // lista de pestañas para que pinte la barra.
            let groups = ctrl.borrow().tab_groups();
            let grouped: std::collections::HashSet<PaneId> =
                groups.iter().flat_map(|(m, _)| m.iter().copied()).collect();
            let active_members: std::collections::HashSet<PaneId> = groups
                .iter()
                .filter_map(|(m, a)| m.get(*a).copied())
                .collect();
            // Rects visibles: panel no agrupado, o la pestaña activa de su grupo.
            let visible: Vec<(PaneId, Rect)> = pane_rects
                .iter()
                .filter(|(id, _)| !grouped.contains(id) || active_members.contains(id))
                .copied()
                .collect();
            let new_ids: Vec<PaneId> = visible.iter().map(|(id, _)| *id).collect();
            // Todos los ids del layout (visibles + ocultos) para conservar sus modelos.
            let all_ids: Vec<PaneId> = pane_rects.iter().map(|(id, _)| *id).collect();

            let mut m = models.borrow_mut();
            // La estructura cambió si cambió la lista visible, el área, o algún grupo
            // (apilar/activar pestaña no cambia los ids visibles pero sí las barras).
            let structure_changed =
                new_ids != m.pane_ids || !rects_equal(area, m.area) || groups != m.groups;

            if structure_changed {
                let active = ctrl.borrow().active_id();
                let panes: Vec<PaneVm> = visible
                    .iter()
                    .map(|(id, r)| {
                        let c = ctrl.borrow();
                        let purpose = c.purpose_of(*id).map(purpose_to_int).unwrap_or(0);
                        // Si este id es la pestaña activa de un grupo, armar su barra.
                        let tabs: Vec<TabVm> = groups
                            .iter()
                            .find(|(mem, a)| mem.get(*a) == Some(id))
                            .map(|(mem, a)| {
                                mem.iter()
                                    .enumerate()
                                    .map(|(i, mid)| TabVm {
                                        id: mid.0 as i32,
                                        label: SharedString::from(c.pane_label(*mid).as_str()),
                                        active: i == *a,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        drop(c);
                        let pm = m.models_for(*id);
                        PaneVm {
                            id: id.0 as i32,
                            x: r.x,
                            y: r.y,
                            w: r.w,
                            h: r.h,
                            path: SharedString::from(ctrl.borrow().path_of(*id).as_str()),
                            active: Some(*id) == active,
                            purpose,
                            title: SharedString::from(ctrl.borrow().pane_label(*id).as_str()),
                            rows: ModelRc::from(pm.rows.clone()),
                            segments: {
                                let segs: Vec<PathSeg> = ctrl
                                    .borrow()
                                    .path_segments_of(*id)
                                    .into_iter()
                                    .map(|(label, path)| PathSeg {
                                        label: SharedString::from(label.as_str()),
                                        path: SharedString::from(path.as_str()),
                                    })
                                    .collect();
                                ModelRc::from(Rc::new(VecModel::from(segs)))
                            },
                            tree_rows: ModelRc::from(pm.tree.clone()),
                            favs: ModelRc::from(pm.favs.clone()),
                            recents: ModelRc::from(pm.recents.clone()),
                            hist_rows: ModelRc::from(pm.hist.clone()),
                            inspector: InspectorVm::default(),
                            preview: PreviewVm::default(),
                            tabs: ModelRc::from(Rc::new(VecModel::from(tabs))),
                        }
                    })
                    .collect();
                m.panes.set_vec(panes);

                let splits: Vec<SplitVm> = split_handles
                    .iter()
                    .enumerate()
                    .map(|(i, h)| SplitVm {
                        index: i as i32,
                        x: h.rect.x,
                        y: h.rect.y,
                        w: h.rect.w,
                        h: h.rect.h,
                        horizontal: matches!(h.dir, SplitDir::Horizontal),
                    })
                    .collect();
                m.splits.set_vec(splits);

                m.per_pane.retain(|id, _| all_ids.contains(id));
                m.pane_ids = new_ids;
                m.groups = groups;
                m.area = area;
            }

            // Selector de panel destino: rect de cada candidato (orden visual) + su número.
            // Se reconstruye siempre (puede aparecer/desaparecer sin cambio de estructura).
            let picks: Vec<PickVm> = {
                let c = ctrl.borrow();
                match &c.pending_pick {
                    Some(pick) => {
                        let rects: std::collections::HashMap<PaneId, Rect> =
                            c.pane_rects(area).into_iter().collect();
                        pick.candidates
                            .iter()
                            .enumerate()
                            .filter_map(|(i, id)| {
                                rects.get(id).map(|r| PickVm {
                                    x: r.x,
                                    y: r.y,
                                    w: r.w,
                                    h: r.h,
                                    number: (i + 1) as i32,
                                })
                            })
                            .collect()
                    }
                    None => Vec::new(),
                }
            };
            m.picks.set_vec(picks);

            drop(m);
            sync_rows();
        })
    };

    // Waker para los watchers (carpeta/dispositivos): desde su hilo, encolan en el event loop
    // de Slint una llamada a `wake()` de la ventana (re-arranca el timer si dormía).
    // `slint::Weak` es Send; el closure del event loop corre en el hilo de UI.
    let waker: naygo_platform::dir_watch::Waker = {
        let ui_weak = ui.as_weak();
        std::sync::Arc::new(move || {
            let ui_weak = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_wake();
                }
            });
        })
    };

    // Watcher de dispositivos (Fase 5B): detecta USB enchufado/quitado. Vive toda la sesión.
    let devices = Rc::new(devices::Devices::start(waker.clone()));
    // HOME para reubicar paneles cuya unidad desapareció.
    let home: Rc<std::path::PathBuf> = Rc::new(
        std::env::var_os("USERPROFILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("C:/")),
    );

    // Drag&drop OLE — RECIBIR (Fase 5D): canal de archivos soltados sobre la ventana. El
    // registro del IDropTarget se hace en el primer tick (cuando el HWND ya es válido) y el
    // guard vive toda la sesión. Por simplicidad el drop va al panel ACTIVO (fallback del
    // diseño: no se mapea el punto de drop a un panel concreto).
    let (drop_tx, drop_rx) = std::sync::mpsc::channel::<naygo_platform::drop_target::DropPayload>();
    let drop_rx = Rc::new(drop_rx);
    let drop_guard: Rc<RefCell<Option<naygo_platform::drop_target::DropTargetGuard>>> =
        Rc::new(RefCell::new(None));

    // Tray (Fase 5E): ícono en bandeja con menú Abrir/Salir, solo si el ajuste lo pide. Vive
    // toda la sesión. `tray_active` lo lee el handler de cierre para decidir si oculta a la
    // bandeja o sale de verdad.
    let tray: Rc<Option<tray::Tray>> = Rc::new(if ctrl.borrow().config.settings.tray_enabled {
        let t = {
            let c = ctrl.borrow();
            tray::create(
                &c.config.t("slint.tray.open"),
                &c.config.t("slint.tray.exit"),
                waker.clone(),
            )
        };
        t
    } else {
        None
    });
    let tray_active = tray.is_some();

    // Timer que drena listados de archivos + árbol + preview; se apaga cuando todo está en
    // reposo (0 trabajo). El preview cambia structs del PaneVm → en cada tick sync_rows.
    let timer = Rc::new(slint::Timer::default());
    let start_timer: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let timer = timer.clone();
        let devices = devices.clone();
        let home = home.clone();
        let waker = waker.clone();
        let ui_weak = ui.as_weak();
        let drop_tx = drop_tx.clone();
        let drop_rx = drop_rx.clone();
        let drop_guard = drop_guard.clone();
        let tray = tray.clone();
        Rc::new(move || {
            let ctrl = ctrl.clone();
            let sync_rows = sync_rows.clone();
            let timer2 = timer.clone();
            let waker = waker.clone();
            let devices = devices.clone();
            let home = home.clone();
            let ui_weak = ui_weak.clone();
            let drop_tx = drop_tx.clone();
            let drop_rx = drop_rx.clone();
            let drop_guard = drop_guard.clone();
            let tray = tray.clone();
            timer.start(
                TimerMode::Repeated,
                std::time::Duration::from_millis(30),
                move || {
                    let now = std::time::Instant::now();
                    // Registrar el destino de drop OLE una sola vez, cuando el HWND ya es válido
                    // (primer tick con la ventana realizada). El guard vive toda la sesión.
                    if drop_guard.borrow().is_none() {
                        if let Some(ui) = ui_weak.upgrade() {
                            if let Some(hwnd) = naygo_hwnd(&ui) {
                                let g = naygo_platform::drop_target::register(
                                    hwnd,
                                    drop_tx.clone(),
                                    waker.clone(),
                                );
                                *drop_guard.borrow_mut() = Some(g);
                            }
                        }
                    }
                    // Drag&drop OLE — RECIBIR (F5D): archivos soltados sobre la ventana → copiar
                    // (o mover si Shift) a la carpeta del panel activo (fallback del diseño).
                    while let Ok(payload) = drop_rx.try_recv() {
                        if let Some(active) = ctrl.borrow().active_id() {
                            ctrl.borrow_mut()
                                .drop_external(active, payload.paths, payload.move_);
                        }
                    }
                    // Tray (F5E): drenar los mensajes del ícono de bandeja. Abrir = mostrar y
                    // elevar la ventana; Salir = terminar el bucle de verdad.
                    if let Some(t) = tray.as_ref() {
                        while let Ok(msg) = t.rx.try_recv() {
                            match msg {
                                tray::TrayMsg::Open => {
                                    if let Some(ui) = ui_weak.upgrade() {
                                        let _ = ui.show();
                                        ui.window().set_minimized(false);
                                    }
                                }
                                tray::TrayMsg::Exit => {
                                    ctrl.borrow().save_session();
                                    let _ = slint::quit_event_loop();
                                }
                            }
                        }
                    }
                    // Watcher de dispositivos (F5B): si cambiaron las unidades (USB), reubicar
                    // los paneles cuya carpeta desapareció y re-listarlos.
                    if devices.drives_changed() {
                        let moved = ctrl.borrow_mut().relocate_orphans(&home);
                        for id in moved {
                            let dir = ctrl
                                .borrow()
                                .ws
                                .pane(id)
                                .and_then(|p| p.files.as_ref())
                                .map(|f| f.current_dir.clone());
                            if let Some(dir) = dir {
                                ctrl.borrow_mut().start_listing(id, dir);
                            }
                        }
                    }
                    // Asegurar que cada panel Files vigile su carpeta actual (barato si nada
                    // cambió). Arranca/re-arranca watchers tras navegar/agregar/cerrar paneles.
                    ctrl.borrow_mut().reconcile_watchers(waker.clone());
                    let files_done = ctrl.borrow_mut().pump_listings();
                    let tree_done = ctrl.borrow_mut().pump_tree();
                    let preview_busy = ctrl.borrow_mut().drive_preview(now);
                    let preview_ready = ctrl.borrow_mut().preview.poll().is_some();
                    let _ = preview_ready;
                    // Drenar el progreso de las operaciones de archivo (F3).
                    let ops_done = ctrl.borrow_mut().ops.pump_ops();
                    // Watcher de carpeta (F5A): aplicar los cambios detectados a cada panel y
                    // marcar como nuevos los archivos recién aparecidos (para resaltarlos).
                    let batches = ctrl.borrow_mut().watchers.drain();
                    for (pane, events) in batches {
                        let nuevas = ctrl.borrow_mut().apply_watch_events(PaneId(pane), &events);
                        ctrl.borrow_mut().watchers.mark_fresh(pane, nuevas, now);
                    }
                    // Limpiar los resaltados vencidos y saber si queda alguno (para seguir
                    // pintando hasta que se apaguen).
                    let hl_secs = ctrl.borrow().highlight_secs();
                    ctrl.borrow_mut().watchers.prune(hl_secs, now);
                    let fresh_pending = ctrl.borrow().watchers.any_fresh(hl_secs, now);
                    sync_rows();
                    // Persistir la sesión si cambió (agregar/cerrar/navegar paneles). Barato
                    // si no cambió. Antes de parar el timer, así el último cambio se guarda.
                    ctrl.borrow_mut().maybe_persist_session();
                    // El watcher corre en su propio hilo y despierta la UI con el waker; el
                    // timer puede dormir cuando no hay trabajo NI resaltados pendientes.
                    if files_done && tree_done && !preview_busy && ops_done && !fresh_pending {
                        timer2.stop();
                    }
                },
            );
        })
    };
    start_timer();

    // `wake`: re-arranca el timer cuando un worker (watcher) despierta la UI. Corre en el hilo
    // de UI (lo dispara invoke_from_event_loop), así que puede tocar el `start_timer` (Rc).
    {
        let start_timer = start_timer.clone();
        ui.on_wake(move || start_timer());
    }

    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let sync_layout = sync_layout.clone();
        ui.on_row_clicked(move |id, pos| {
            // El doble-clic se detecta en Rust (no en Slint): on_row_clicked devuelve true
            // si este clic completó un doble-clic, en cuyo caso navegó/abrió.
            let navigated = ctrl.borrow_mut().on_row_clicked(
                PaneId(id as u64),
                pos as usize,
                std::time::Instant::now(),
            );
            // Cambiar el foco/navegar puede disparar un preview o cambiar el layout.
            start_timer();
            if navigated {
                sync_layout();
            } else {
                sync_rows();
            }
        });
    }
    {
        // Doble clic NATIVO de Slint (cronometrado por el SO): camino primario para abrir
        // carpetas, robusto ante la latencia del hilo de UI bajo render por software (caso
        // VM). La detección por tiempo en Rust (en on_row_clicked) queda de respaldo; el
        // controlador evita la doble navegación con una marca temporal.
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_row_double_clicked(move |id, pos| {
            if ctrl
                .borrow_mut()
                .on_row_double_clicked_native(PaneId(id as u64), pos as usize)
            {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        // Arrastre OLE hacia afuera (Fase 5C): saca los archivos seleccionados del panel
        // hacia el Explorer/escritorio/otra app. `start_drag` es BLOQUEANTE (corre el bucle OLE
        // de Windows hasta soltar), que es el comportamiento nativo esperado.
        let ctrl = ctrl.clone();
        ui.on_row_drag_out(move |_id| {
            let paths = ctrl.borrow().selected_paths();
            if paths.is_empty() {
                return;
            }
            let _ = naygo_platform::dnd::start_drag(&paths);
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_sort_by(move |_id, col| {
            ctrl.borrow_mut().on_sort_by(col.as_str());
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_activate(move |id| {
            ctrl.borrow_mut().set_active(PaneId(id as u64));
            start_timer();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let ui_weak = ui.as_weak();
        ui.on_key(move |text, c, s, a| {
            if ctrl.borrow_mut().on_key(text.as_str(), c, s, a) {
                start_timer();
            }
            // Atajo "editar ruta" (Ctrl+L / F4): abrir el editor de la path-bar del panel pedido.
            if let Some(pane) = ctrl.borrow_mut().take_edit_path_request() {
                if let Some(ui) = ui_weak.upgrade() {
                    let path = ctrl.borrow().path_of(pane);
                    let sugg = ctrl.borrow().path_autocomplete(&path);
                    ui.set_edit_pane(pane.0 as i32);
                    ui.set_edit_text(path.into());
                    ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                        sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
                    ))));
                }
            }
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_go_up(move || {
            if ctrl.borrow_mut().on_go_up() {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_add_pane(move || {
            ctrl.borrow_mut().add_pane_split();
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_add_pane_of(move |purpose| {
            ctrl.borrow_mut().add_pane_of(int_to_purpose(purpose));
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_add_pane_dir(move |dir| {
            // 0=derecha 1=abajo 2=izquierda 3=arriba.
            let (split, first) = match dir {
                1 => (SplitDir::Vertical, false),
                2 => (SplitDir::Horizontal, true),
                3 => (SplitDir::Vertical, true),
                _ => (SplitDir::Horizontal, false),
            };
            ctrl.borrow_mut().add_pane_split_dir(split, first);
            start_timer();
            sync_layout();
        });
    }

    // --- Ventana de configuración (Fase 4) ---
    // Reconstruye el SettingsVm + filas de atajos desde ConfigCtrl y los vuelca a la UI.
    let refresh_config_vm: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        Rc::new(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let c = ctrl.borrow();
            ui.set_settings_vm(build_settings_vm(&c.config));
            let rows: Vec<ShortcutRowVm> = c
                .config
                .shortcut_list()
                .into_iter()
                .map(|(key, label, chord)| ShortcutRowVm {
                    action_key: key.into(),
                    label: label.into(),
                    chord_text: chord.into(),
                    conflict: SharedString::new(),
                })
                .collect();
            ui.set_shortcut_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
            ui.set_config_dir(c.config.config_dir.to_string_lossy().to_string().into());
            ui.set_app_version(env!("CARGO_PKG_VERSION").into());
        })
    };
    refresh_config_vm();
    // Vuelca los íconos de acción de la toolbar desde el IconCache a la AppWindow. Se llama al
    // arrancar y al cambiar el set de íconos. Decodifica una vez (cacheado): clonar es barato.
    let refresh_toolbar_icons: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        Rc::new(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            use naygo_core::icon_kind::{ActionIcon, IconKey};
            let mut c = ctrl.borrow_mut();
            let ic = |c: &mut workspace_ctrl::WorkspaceCtrl, a: ActionIcon| {
                c.icons.get(IconKey::Action(a))
            };
            ui.set_ic_up(ic(&mut c, ActionIcon::Up));
            ui.set_ic_panel(ic(&mut c, ActionIcon::NewWindow));
            ui.set_ic_swap(ic(&mut c, ActionIcon::SwapPanes));
            ui.set_ic_clone(ic(&mut c, ActionIcon::ClonePath));
            ui.set_ic_tabs(ic(&mut c, ActionIcon::AddPane));
            ui.set_ic_settings(ic(&mut c, ActionIcon::Settings));
        })
    };
    refresh_toolbar_icons();
    // Acción cuyo atajo se está capturando (la setea cfg-shortcut-capture; la lee cfg-capture-key).
    let capturing_action: Rc<RefCell<Option<naygo_core::keymap::Action>>> =
        Rc::new(RefCell::new(None));

    // Toggles/combos/text que solo persisten (no requieren refrescar la vista de paneles).
    macro_rules! cfg_setter {
        ($on:ident, $arg:ty, $method:ident) => {{
            let ctrl = ctrl.clone();
            let refresh = refresh_config_vm.clone();
            ui.$on(move |v: $arg| {
                ctrl.borrow_mut().config.$method(v);
                refresh();
            });
        }};
    }
    cfg_setter!(on_cfg_set_ops_mode, i32, set_ops_mode);
    cfg_setter!(on_cfg_set_confirm_trash, bool, set_confirm_trash);
    cfg_setter!(on_cfg_set_show_op_summary, bool, set_show_op_summary);
    cfg_setter!(on_cfg_set_show_parent, bool, set_show_parent);
    cfg_setter!(on_cfg_set_icon_only, bool, set_icon_only);
    cfg_setter!(on_cfg_set_bar_position, i32, set_bar_position);
    cfg_setter!(on_cfg_set_size_no_subdirs, bool, set_size_no_subdirs);
    cfg_setter!(on_cfg_set_autostart, bool, set_autostart);
    cfg_setter!(on_cfg_set_date_format, i32, set_date_format);
    cfg_setter!(on_cfg_set_paste_confirm, bool, set_paste_confirm);
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        ui.on_cfg_set_paste_text_name(move |v| {
            ctrl.borrow_mut().config.set_paste_text_name(v.to_string());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        ui.on_cfg_set_paste_text_ext(move |v| {
            ctrl.borrow_mut().config.set_paste_text_ext(v.to_string());
            refresh();
        });
    }
    // Cambio de idioma en caliente: persiste + re-vuelca todos los textos a Tr.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        ui.on_cfg_set_language(move |code| {
            let lang = naygo_core::i18n::LangId::new(&code);
            ctrl.borrow_mut().config.set_language(lang);
            if let Some(ui) = ui_weak.upgrade() {
                i18n_keys::apply(&ui, &ctrl.borrow().config);
            }
            refresh();
        });
    }
    // Cambio de tema en caliente: persiste + re-vuelca los colores a Theme.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        ui.on_cfg_set_theme(move |id| {
            ctrl.borrow_mut()
                .config
                .set_theme(naygo_core::theme::ThemeId::new(&id));
            if let Some(ui) = ui_weak.upgrade() {
                theme_apply::apply(&ui, ctrl.borrow().config.active_theme());
            }
            refresh();
        });
    }
    // Cambio de set de íconos en caliente: persiste + apunta el IconCache al set nuevo. Las
    // filas repintan en el próximo tick (sync_rows consulta el cache, que decodifica el set
    // nuevo on-demand). El refresh re-snapshotea el VM de config para reflejar la selección.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let refresh_icons = refresh_toolbar_icons.clone();
        ui.on_cfg_set_icon_set(move |id| {
            {
                let mut c = ctrl.borrow_mut();
                c.config.set_icon_set(id.to_string());
                // Tomar el id ya coaccionado por el catálogo (un id inválido cayó a "flat").
                let active = c.config.settings.icon_set.clone();
                c.icons.set_active(active);
            }
            // Repintar los íconos de la toolbar con el set nuevo (borrow ya liberado arriba).
            refresh_icons();
            refresh();
        });
    }
    // Editor de atajos: capturar la acción a reasignar.
    {
        let capturing = capturing_action.clone();
        ui.on_cfg_shortcut_capture(move |key| {
            *capturing.borrow_mut() = config_ctrl::ConfigCtrl::action_from_key(&key);
        });
    }
    // Captura de la combinación: si hay acción en captura y el chord es válido, reasigna.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let capturing = capturing_action.clone();
        ui.on_cfg_capture_key(move |text, c, s, a| {
            let action = match capturing.borrow_mut().take() {
                Some(act) => act,
                None => return,
            };
            // Esc cancela la captura sin reasignar (la UI ya salió del modo captura).
            if text == keys::escape_char().to_string() {
                return;
            }
            if let Some(chord) = keys::chord_from(&text, c, s, a) {
                ctrl.borrow_mut().config.rebind(action, chord);
                refresh();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        ui.on_cfg_shortcut_reset(move |key| {
            if let Some(action) = config_ctrl::ConfigCtrl::action_from_key(&key) {
                ctrl.borrow_mut().config.reset_shortcut(action);
                refresh();
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        ui.on_cfg_shortcuts_reset_all(move || {
            ctrl.borrow_mut().config.reset_all_shortcuts();
            refresh();
        });
    }
    // Import/Export de packs (.zip) — Fase 4E. Selector de archivo nativo (rfd); el resultado
    // se informa con un MessageDialog (errores) sin bloquear el resto de la UI.
    {
        let ctrl = ctrl.clone();
        ui.on_cfg_export_language(move || {
            let c = ctrl.borrow();
            let code = c.config.settings.language.as_str().to_string();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Pack Naygo (.zip)", &["zip"])
                .set_file_name(format!("naygo-idioma-{code}.zip"))
                .save_file()
            {
                report(packs::export_lang(&c.config.config_dir, &code, &path));
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        ui.on_cfg_export_theme(move || {
            let c = ctrl.borrow();
            let id = c.config.settings.theme.as_str().to_string();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Pack Naygo (.zip)", &["zip"])
                .set_file_name(format!("naygo-tema-{id}.zip"))
                .save_file()
            {
                report(packs::export_theme(&c.config.config_dir, &id, &path));
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        ui.on_cfg_export_config(move || {
            let c = ctrl.borrow();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Pack Naygo (.zip)", &["zip"])
                .set_file_name("naygo-config.zip")
                .save_file()
            {
                report(packs::export_config(&c.config.config_dir, &path));
            }
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh_config_vm.clone();
        let ui_weak = ui.as_weak();
        ui.on_cfg_import_pack(move || {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Pack Naygo (.zip)", &["zip"])
                .pick_file()
            else {
                return;
            };
            // Importar y, según el tipo, recargar el catálogo correspondiente.
            let config_dir = ctrl.borrow().config.config_dir.clone();
            match packs::import_zip(&config_dir, &path) {
                Ok(kind) => {
                    {
                        let mut cb = ctrl.borrow_mut();
                        let lang = cb.config.settings.language.clone();
                        let theme = cb.config.settings.theme.clone();
                        match kind {
                            packs::ImportKind::Lang(_) => {
                                cb.config.i18n = naygo_core::i18n::I18n::load(&config_dir, &lang);
                            }
                            packs::ImportKind::Theme(_) => {
                                cb.config.themes =
                                    naygo_core::theme::ThemeCatalog::load(&config_dir, &theme);
                            }
                            packs::ImportKind::Config => {
                                let fresh = config_ctrl::ConfigCtrl::new(config_dir.clone());
                                cb.config = fresh;
                            }
                        }
                    }
                    // Reaplicar textos y colores por si cambió el catálogo activo.
                    if let Some(ui) = ui_weak.upgrade() {
                        i18n_keys::apply(&ui, &ctrl.borrow().config);
                        theme_apply::apply(&ui, ctrl.borrow().config.active_theme());
                    }
                    refresh();
                }
                Err(e) => report(Err::<(), String>(e)),
            }
        });
    }
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
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tree_navigate(move |path| {
            if ctrl
                .borrow_mut()
                .navigate_active_to(std::path::PathBuf::from(path.as_str()))
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
        // El texto del editor cambió: recalcular el autocompletado.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_path_edit_changed(move |_id, text| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let sugg = ctrl.borrow().path_autocomplete(text.as_str());
            ui.set_edit_text(text);
            ui.set_edit_suggestions(ModelRc::from(Rc::new(VecModel::from(
                sugg.into_iter().map(SharedString::from).collect::<Vec<_>>(),
            ))));
        });
    }
    {
        // Enter en el editor: navegar a la ruta tecleada (si existe como carpeta) y salir.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_path_edit_commit(move |id, text| {
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
        let ui_weak = ui.as_weak();
        ui.on_path_edit_cancel(move |_id| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_edit_pane(-1);
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
    {
        // Botón "Deshacer" del panel Historial: deshace la entrada por id.
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_undo_entry(move |id| {
            if ctrl.borrow_mut().undo_entry(id as u64) {
                start_timer();
            }
            sync_rows();
        });
    }
    // --- Diálogos modales y panel de progreso de operaciones (F3) ---
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_delete_confirm(move || {
            if ctrl.borrow_mut().ops.delete_confirm() {
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
            let act = match action {
                0 => ConflictAction::Overwrite,
                2 => ConflictAction::Rename,
                _ => ConflictAction::Skip,
            };
            // El op_index del conflicto activo lo guarda el pending_dialog.
            let idx = {
                let c = ctrl.borrow();
                if let Some(ops_ctrl::OpDialog::Conflict { op_index, .. }) = &c.ops.pending_dialog {
                    Some(*op_index)
                } else {
                    None
                }
            };
            if let Some(idx) = idx {
                ctrl.borrow_mut().ops.resolve_conflict(idx, act, apply_all);
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
            if ctrl.borrow_mut().ops.name_confirm() {
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
        ui.on_op_cancel(move |idx| {
            ctrl.borrow_mut().ops.cancel_op(idx as usize);
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
                    drop(c);
                    start_timer();
                }
            } else {
                c.ops.discard(&id);
            }
            sync_rows();
        });
    }
    // --- Menú contextual (clic derecho): acciones propias + nativo ---
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_row_context(move |_id, _pos, x, y| {
            ctrl.borrow_mut().open_context_menu(x, y);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_dismiss(move || {
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_open(move || {
            ctrl.borrow_mut().ctx_open();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_open_with(move || {
            ctrl.borrow_mut().ctx_open_with();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_copy(move || {
            ctrl.borrow_mut().op_copy();
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_cut(move || {
            ctrl.borrow_mut().op_cut();
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        ui.on_ctx_paste(move || {
            if ctrl.borrow_mut().op_paste() {
                start_timer();
            }
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_rename(move || {
            ctrl.borrow_mut().op_rename();
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_delete(move || {
            ctrl.borrow_mut().op_delete(false);
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_copy_path(move || {
            ctrl.borrow_mut().ctx_copy_path();
            sync_rows();
        });
    }
    {
        // "Más opciones de Windows…": invoca el menú nativo del Shell con el HWND de winit.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_rows = sync_rows.clone();
        ui.on_ctx_native(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let (targets, x, y) = {
                let c = ctrl.borrow();
                match &c.context_menu {
                    Some(cm) => (cm.targets.clone(), cm.x, cm.y),
                    None => return,
                }
            };
            ctrl.borrow_mut().close_context_menu();
            sync_rows();
            if let Some(hwnd) = naygo_hwnd(&ui) {
                // Coords de pantalla = posición de la ventana + posición del clic en la ventana.
                let pos = ui.window().position();
                let sx = pos.x + x as i32;
                let sy = pos.y + y as i32;
                let _ =
                    naygo_platform::context_menu::show_native_context_menu(hwnd, &targets, sx, sy);
            }
        });
    }
    // --- Acciones multi-panel (swap / clonar) + selector de destino ---
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let area_of = area_of.clone();
        ui.on_swap_panes(move || {
            let area = area_of();
            let acted = {
                let mut c = ctrl.borrow_mut();
                let Some(origin) = c.active_id() else {
                    return;
                };
                c.request_action(workspace_ctrl::PaneAction::Swap, origin, area)
            };
            if acted {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        let area_of = area_of.clone();
        ui.on_clone_pane(move || {
            let area = area_of();
            let acted = {
                let mut c = ctrl.borrow_mut();
                let Some(origin) = c.active_id() else {
                    return;
                };
                c.request_action(workspace_ctrl::PaneAction::Clone, origin, area)
            };
            if acted {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        ui.on_stack_pane(move || {
            let area = area_of();
            {
                let mut c = ctrl.borrow_mut();
                let Some(origin) = c.active_id() else {
                    return;
                };
                c.request_action(workspace_ctrl::PaneAction::Stack, origin, area);
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_tab_select(move |id| {
            ctrl.borrow_mut().set_active_tab(PaneId(id as u64));
            // Cambiar de pestaña puede disparar el preview del nuevo foco.
            start_timer();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_tab_close(move |id| {
            ctrl.borrow_mut().close_tab(PaneId(id as u64));
            sync_layout();
        });
    }
    {
        // Durante el arrastre: resaltar la zona de drop bajo el puntero.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let area_of = area_of.clone();
        ui.on_pane_drag_move(move |id, x, y| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let area = area_of();
            let preview = ctrl.borrow().drop_preview(PaneId(id as u64), x, y, area);
            match preview {
                Some((r, is_tab)) => {
                    ui.set_drop_x(r.x);
                    ui.set_drop_y(r.y);
                    ui.set_drop_w(r.w);
                    ui.set_drop_h(r.h);
                    ui.set_drop_is_tab(is_tab);
                }
                None => {
                    ui.set_drop_w(0.0);
                    ui.set_drop_h(0.0);
                }
            }
        });
    }
    {
        // Al soltar: recomponer el layout y limpiar el resaltado.
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        ui.on_pane_drag_drop(move |id, x, y| {
            let area = area_of();
            ctrl.borrow_mut()
                .perform_drop(PaneId(id as u64), x, y, area);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_drop_w(0.0);
                ui.set_drop_h(0.0);
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_pick_resolve(move |n| {
            if ctrl.borrow_mut().pick_resolve(n as usize) {
                start_timer();
            }
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        ui.on_pick_cancel(move || {
            ctrl.borrow_mut().pick_cancel();
            sync_layout();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let area_of = area_of.clone();
        ui.on_split_drag(move |index, dx, dy| {
            let area = area_of();
            {
                let mut c = ctrl.borrow_mut();
                let handles = c.split_handles(area);
                if let Some(h) = handles.get(index as usize) {
                    let (pos, total) = if matches!(h.dir, SplitDir::Horizontal) {
                        (h.rect.x + h.rect.w / 2.0 + dx, area.w.max(1.0))
                    } else {
                        (h.rect.y + h.rect.h / 2.0 + dy, area.h.max(1.0))
                    };
                    let path = h.path.clone();
                    c.set_fraction(&path, pos / total);
                }
            }
            sync_layout();
        });
    }
    {
        let sync_layout = sync_layout.clone();
        ui.on_content_resized(move || sync_layout());
    }

    // Al cerrar la ventana (Fase 5E, arregla la deuda de F4): persistir la sesión y luego SALIR
    // DE VERDAD (quit_event_loop), salvo que el usuario haya pedido "cerrar a bandeja" y el tray
    // esté activo, en cuyo caso se oculta a la bandeja y la app sigue viva a propósito.
    {
        let ctrl = ctrl.clone();
        ui.window().on_close_requested(move || {
            ctrl.borrow().save_session();
            let close_to_tray = ctrl.borrow().config.settings.close_to_tray;
            if tray::should_quit_on_close(close_to_tray, tray_active) {
                let _ = slint::quit_event_loop();
            }
            // En ambos casos HideWindow: al salir, el loop ya está marcado para terminar; al ir
            // a bandeja, la ventana se oculta y el proceso sigue.
            slint::CloseRequestResponse::HideWindow
        });
    }

    // Al arrancar: si hay operaciones interrumpidas (journal), ofrecer retomarlas.
    ctrl.borrow_mut().ops.scan_resume();
    sync_layout();
    ui.run()
}

fn int_to_purpose(p: i32) -> PanePurpose {
    match p {
        1 => PanePurpose::Tree,
        2 => PanePurpose::Inspector,
        3 => PanePurpose::History,
        4 => PanePurpose::Favorites,
        5 => PanePurpose::Preview,
        _ => PanePurpose::Files,
    }
}

/// Muestra un diálogo de error si el resultado de un import/export falló; silencioso si OK.
fn report<T>(r: Result<T, String>) {
    if let Err(e) = r {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Naygo")
            .set_description(e)
            .show();
    }
}

/// Construye el `SettingsVm` (snapshot para la ventana de config) desde el ConfigCtrl.
fn build_settings_vm(c: &config_ctrl::ConfigCtrl) -> SettingsVm {
    use naygo_core::config::{BarPosition, OpsMode};
    let s = &c.settings;
    let languages: Vec<SharedString> = c
        .i18n
        .available()
        .iter()
        .map(|l| SharedString::from(l.as_str()))
        .collect();
    let themes: Vec<SharedString> = c
        .themes
        .available()
        .iter()
        .map(|t| SharedString::from(t.as_str()))
        .collect();
    let icon_sets: Vec<SharedString> = naygo_core::icon_set::IconSetCatalog::load(&c.config_dir)
        .available()
        .iter()
        .map(|s| SharedString::from(s.id.as_str()))
        .collect();
    SettingsVm {
        bar_position: if s.bar_position == BarPosition::Side {
            1
        } else {
            0
        },
        icon_only: s.icon_only,
        show_parent: s.show_parent_entry,
        ops_mode: if s.ops_mode == OpsMode::Parallel {
            1
        } else {
            0
        },
        confirm_trash: s.confirm_trash,
        show_op_summary: s.show_op_summary,
        size_no_subdirs: s.size_no_subdirs,
        autostart: s.autostart,
        date_format: match s.date_format {
            naygo_core::format::DateFormat::IsoMinute => 0,
            naygo_core::format::DateFormat::IsoDate => 1,
            naygo_core::format::DateFormat::DmyMinute => 2,
            naygo_core::format::DateFormat::DmyDate => 3,
        },
        paste_confirm: s.paste_confirm,
        paste_text_name: s.paste_text_name.clone().into(),
        paste_text_ext: s.paste_text_ext.clone().into(),
        language: s.language.as_str().into(),
        theme: s.theme.as_str().into(),
        icon_set: s.icon_set.as_str().into(),
        languages: ModelRc::from(Rc::new(VecModel::from(languages))),
        themes: ModelRc::from(Rc::new(VecModel::from(themes))),
        icon_sets: ModelRc::from(Rc::new(VecModel::from(icon_sets))),
    }
}

/// El HWND de la ventana de Naygo (backend winit), para el menú contextual del Shell.
/// `None` si no se puede obtener (otro backend) — entonces se oculta "Más opciones de
/// Windows…". Usa raw-window-handle vía el feature `raw-window-handle-06` de slint.
fn naygo_hwnd(ui: &AppWindow) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = ui.window().window_handle();
    match handle.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(isize::from(h.hwnd)),
        _ => None,
    }
}

/// El `PreviewVm` actual a partir del último resultado guardado en el controlador. El
/// resultado vivo se entrega por `poll()` en el timer y se cachea en el ctrl; aquí lo
/// reconstruimos para pintarlo. (Mantener la última vista evita parpadeo entre ticks.)
fn current_preview_vm(c: &WorkspaceCtrl) -> PreviewVm {
    match c.preview.last_view() {
        Some(preview::ViewCache::Text { text, truncated }) => PreviewVm {
            mode: 1,
            text: SharedString::from(text.as_str()),
            truncated: *truncated,
            image: slint::Image::default(),
            message: SharedString::new(),
        },
        Some(preview::ViewCache::Image {
            rgba,
            width,
            height,
        }) => {
            let buf = SharedPixelBuffer::clone_from_slice(rgba, *width, *height);
            PreviewVm {
                mode: 2,
                text: SharedString::new(),
                truncated: false,
                image: slint::Image::from_rgba8(buf),
                message: SharedString::new(),
            }
        }
        Some(preview::ViewCache::Message(m)) => PreviewVm {
            mode: 3,
            text: SharedString::new(),
            truncated: false,
            image: slint::Image::default(),
            message: SharedString::from(m.as_str()),
        },
        None => PreviewVm {
            mode: 0,
            text: SharedString::new(),
            truncated: false,
            image: slint::Image::default(),
            message: SharedString::new(),
        },
    }
}

fn to_row_data(r: bridge::PlainRow) -> RowData {
    RowData {
        name: SharedString::from(r.name.as_str()),
        ext: SharedString::from(r.ext.as_str()),
        size: SharedString::from(r.size.as_str()),
        modified: SharedString::from(r.modified.as_str()),
        is_dir: r.is_dir,
        selected: r.selected,
        focused: r.focused,
        cut: r.cut,
        highlight: r.highlight,
        icon: r.icon,
    }
}

fn to_op_dialog_vm(d: ops_ctrl::OpDialogVmData) -> OpDialogVm {
    OpDialogVm {
        kind: d.kind,
        del_count: d.del_count,
        del_permanent: d.del_permanent,
        conflict_name: SharedString::from(d.conflict_name.as_str()),
        name_title: SharedString::from(d.name_title.as_str()),
        name_value: SharedString::from(d.name_value.as_str()),
        name_valid: d.name_valid,
        paste_name: SharedString::from(d.paste_name.as_str()),
        paste_is_image: d.paste_is_image,
    }
}

fn to_op_row_vm(r: ops_ctrl::OpRowData) -> OpRowVm {
    OpRowVm {
        index: r.index,
        label: SharedString::from(r.label.as_str()),
        percent: r.percent,
        status: SharedString::from(r.status.as_str()),
        running: r.running,
    }
}

fn to_nav_row(r: bridge::NavRow) -> NavRow {
    NavRow {
        label: SharedString::from(r.label.as_str()),
        path: SharedString::from(r.path.as_str()),
        icon: r.icon,
    }
}

fn to_hist_row(r: bridge::HistRow) -> HistRow {
    HistRow {
        id: r.id as i32,
        label: SharedString::from(r.label.as_str()),
        when: SharedString::from(r.when.as_str()),
        count: r.count,
        undoable: r.undoable,
        reason: SharedString::from(r.reason.as_str()),
    }
}

fn to_tree_row(r: bridge::TreeRow) -> TreeRow {
    TreeRow {
        depth: r.depth,
        name: SharedString::from(r.name.as_str()),
        path: SharedString::from(r.path.as_str()),
        expanded: r.expanded,
        has_children: r.has_children,
        is_drive: r.is_drive,
        active: r.active,
        loading: r.loading,
        error: r.error,
        disk_percent: r.disk_percent,
        icon: r.icon,
    }
}

fn to_inspector_vm(i: bridge::InspectorInfo) -> InspectorVm {
    InspectorVm {
        present: i.present,
        name: SharedString::from(i.name.as_str()),
        kind: SharedString::from(i.kind.as_str()),
        path: SharedString::from(i.path.as_str()),
        size: SharedString::from(i.size.as_str()),
        modified: SharedString::from(i.modified.as_str()),
        created: SharedString::from(i.created.as_str()),
    }
}
