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
mod keys;
mod listing;
mod ops_ctrl;
mod preview;
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
            let c = ctrl.borrow();
            let active = c.active_id();
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
                        let rows: Vec<RowData> =
                            c.rows_of(id).into_iter().map(to_row_data).collect();
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

    // Timer que drena listados de archivos + árbol + preview; se apaga cuando todo está en
    // reposo (0 trabajo). El preview cambia structs del PaneVm → en cada tick sync_rows.
    let timer = Rc::new(slint::Timer::default());
    let start_timer: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let timer = timer.clone();
        Rc::new(move || {
            let ctrl = ctrl.clone();
            let sync_rows = sync_rows.clone();
            let timer2 = timer.clone();
            timer.start(
                TimerMode::Repeated,
                std::time::Duration::from_millis(30),
                move || {
                    let now = std::time::Instant::now();
                    let files_done = ctrl.borrow_mut().pump_listings();
                    let tree_done = ctrl.borrow_mut().pump_tree();
                    let preview_busy = ctrl.borrow_mut().drive_preview(now);
                    let preview_ready = ctrl.borrow_mut().preview.poll().is_some();
                    let _ = preview_ready;
                    // Drenar el progreso de las operaciones de archivo (F3).
                    let ops_done = ctrl.borrow_mut().ops.pump_ops();
                    sync_rows();
                    if files_done && tree_done && !preview_busy && ops_done {
                        timer2.stop();
                    }
                },
            );
        })
    };
    start_timer();

    {
        let ctrl = ctrl.clone();
        let sync_rows = sync_rows.clone();
        let start_timer = start_timer.clone();
        let sync_layout = sync_layout.clone();
        ui.on_row_clicked(move |id, pos| {
            // El doble-clic se detecta en Rust (no en Slint): on_row_clicked devuelve true
            // si este clic completó un doble-clic, en cuyo caso navegó/abrió.
            let navigated = ctrl
                .borrow_mut()
                .on_row_clicked(PaneId(id as u64), pos as usize, std::time::Instant::now());
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
        ui.on_key(move |text, c, s, a| {
            if ctrl.borrow_mut().on_key(text.as_str(), c, s, a) {
                start_timer();
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
                let _ = naygo_platform::context_menu::show_native_context_menu(
                    hwnd, &targets, sx, sy,
                );
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
            let preview = ctrl
                .borrow()
                .drop_preview(PaneId(id as u64), x, y, area);
            match preview {
                Some(r) => {
                    ui.set_drop_x(r.x);
                    ui.set_drop_y(r.y);
                    ui.set_drop_w(r.w);
                    ui.set_drop_h(r.h);
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
            ctrl.borrow_mut().perform_drop(PaneId(id as u64), x, y, area);
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
