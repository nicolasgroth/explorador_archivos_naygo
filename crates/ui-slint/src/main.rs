// Naygo — arranque de la capa UI en Slint (Fase 2a: multi-panel).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Para forzar el renderizador por software (caso VM sin GPU):
//   $env:SLINT_BACKEND="winit-software"; cargo run -p naygo-ui-slint
//
// MODELOS ESTABLES (clave del rendimiento y de la corrección):
// Slint es modo retenido: un `for p in root.panes` recrea un FilePanel por cada
// ELEMENTO del modelo. Si se reemplaza el VecModel entero en cada refresco (como hacía
// la primera versión de la Fase 2a), Slint destruye y recrea cada FilePanel + su ListView
// en cada tick (30 ms) y en cada interacción → se pierde el scroll y se corta el gesto de
// doble clic. Por eso aquí mantenemos modelos ESTABLES y los mutamos in situ:
//   - `panes_model`: un VecModel<PaneVm> que solo se reestructura cuando cambia la LISTA de
//     paneles o el ÁREA (agregar/quitar panel, redimensionar la ventana o un split).
//   - `rows_models`: un VecModel<RowData> ESTABLE por panel; su contenido se actualiza con
//     `set_vec` (mismo VecModel) → el ListView conserva su estado de scroll.
//   - `splits_model`: idem para las barras de splitter.
// `sync_rows` (barato, corre en cada tick) solo toca el contenido de filas + flags
// activo/path. `sync_layout` (estructural) reconcilia la lista de paneles y splitters.
mod bridge;
mod keys;
mod listing;
mod workspace_ctrl;

use naygo_core::workspace::layout::{Rect, SplitDir};
use naygo_core::workspace::PaneId;
use slint::{Model, ModelRc, SharedString, TimerMode, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use workspace_ctrl::WorkspaceCtrl;

slint::include_modules!();

/// Modelos estables que persisten entre refrescos (ver nota de cabecera).
struct Models {
    panes: Rc<VecModel<PaneVm>>,
    splits: Rc<VecModel<SplitVm>>,
    /// Un VecModel de filas ESTABLE por panel (se actualiza in situ, no se recrea).
    rows: HashMap<PaneId, Rc<VecModel<RowData>>>,
    /// IDs de panel en el orden actual del modelo `panes` (para detectar cambios de lista).
    pane_ids: Vec<PaneId>,
    /// Área con la que se construyó la estructura actual (para detectar resize/splits).
    area: Rect,
}

impl Models {
    fn new() -> Models {
        Models {
            panes: Rc::new(VecModel::default()),
            splits: Rc::new(VecModel::default()),
            rows: HashMap::new(),
            pane_ids: Vec::new(),
            area: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
        }
    }

    /// Devuelve (o crea) el VecModel de filas estable del panel `id`.
    fn rows_for(&mut self, id: PaneId) -> Rc<VecModel<RowData>> {
        self.rows
            .entry(id)
            .or_insert_with(|| Rc::new(VecModel::default()))
            .clone()
    }
}

fn rects_equal(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 0.5
        && (a.y - b.y).abs() < 0.5
        && (a.w - b.w).abs() < 0.5
        && (a.h - b.h).abs() < 0.5
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let start = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
    let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new(start)));
    let models = Rc::new(RefCell::new(Models::new()));

    // Enlaza los modelos estables a la ventana una sola vez.
    ui.set_panes(ModelRc::from(models.borrow().panes.clone()));
    ui.set_splits(ModelRc::from(models.borrow().splits.clone()));

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

    // Actualiza SOLO el contenido (filas + activo + path) sin tocar la estructura. Barato:
    // corre en cada tick del timer. Mantiene los mismos VecModel → el ListView conserva su
    // scroll y los gestos no se interrumpen.
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
            let mut m = models.borrow_mut();
            for (i, &id) in m.pane_ids.clone().iter().enumerate() {
                // Actualiza filas in situ (mismo VecModel).
                let rows: Vec<RowData> = c.rows_of(id).into_iter().map(to_row_data).collect();
                let rows_model = m.rows_for(id);
                rows_model.set_vec(rows);
                // Actualiza flags del PaneVm sin recrear el elemento (preserva el FilePanel).
                if let Some(mut pv) = m.panes.row_data(i) {
                    let is_active = Some(id) == active;
                    let path = SharedString::from(c.path_of(id).as_str());
                    if pv.active != is_active || pv.path != path {
                        pv.active = is_active;
                        pv.path = path;
                        m.panes.set_row_data(i, pv);
                    }
                }
            }
            if let Some(id) = active {
                ui.set_active_path(SharedString::from(c.path_of(id).as_str()));
            }
        }
    };

    // Reconcilia la ESTRUCTURA (lista de paneles + splitters) con el estado del core. Solo
    // reconstruye cuando cambia la lista de IDs o el área (agregar/quitar panel, resize,
    // arrastre de split). Tras reestructurar, sincroniza las filas.
    let sync_layout: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let models = models.clone();
        let area_of = area_of.clone();
        let sync_rows = sync_rows.clone();
        Rc::new(move || {
            let area = area_of();
            let pane_rects = ctrl.borrow().pane_rects(area);
            let split_handles = ctrl.borrow().split_handles(area);
            let new_ids: Vec<PaneId> = pane_rects.iter().map(|(id, _)| *id).collect();

            let mut m = models.borrow_mut();
            let structure_changed = new_ids != m.pane_ids || !rects_equal(area, m.area);

            if structure_changed {
                // Reconstruye el modelo de paneles, reutilizando los VecModel de filas
                // estables existentes (los recién creados arrancan vacíos y se llenan en
                // sync_rows). Limpia los modelos de filas de paneles que ya no existen.
                let active = ctrl.borrow().active_id();
                let panes: Vec<PaneVm> = pane_rects
                    .iter()
                    .map(|(id, r)| {
                        let rows_model = m.rows_for(*id);
                        PaneVm {
                            id: id.0 as i32,
                            x: r.x,
                            y: r.y,
                            w: r.w,
                            h: r.h,
                            path: SharedString::from(ctrl.borrow().path_of(*id).as_str()),
                            rows: ModelRc::from(rows_model),
                            active: Some(*id) == active,
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

                m.rows.retain(|id, _| new_ids.contains(id));
                m.pane_ids = new_ids;
                m.area = area;
            }
            drop(m);
            sync_rows();
        })
    };

    // Timer que drena los listados activos; se apaga cuando todos terminan. Cada tick solo
    // sincroniza filas (barato); la estructura no cambia mientras se listan archivos.
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
                    let all_done = ctrl.borrow_mut().pump_listings();
                    sync_rows();
                    if all_done {
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
        ui.on_row_clicked(move |id, pos| {
            ctrl.borrow_mut()
                .on_row_clicked(PaneId(id as u64), pos as usize);
            sync_rows();
        });
    }
    {
        let ctrl = ctrl.clone();
        let sync_layout = sync_layout.clone();
        let start_timer = start_timer.clone();
        ui.on_row_double_clicked(move |id, pos| {
            if ctrl
                .borrow_mut()
                .on_row_double_clicked(PaneId(id as u64), pos as usize)
            {
                start_timer();
            }
            // Navegar cambia el path (estructura del PaneVm) → reconcilia.
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
        ui.on_activate(move |id| {
            ctrl.borrow_mut().set_active(PaneId(id as u64));
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
            // Una tecla puede navegar (cambia path) o cambiar el panel activo → reconcilia.
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
        let area_of = area_of.clone();
        ui.on_split_drag(move |index, dx, dy| {
            let area = area_of();
            {
                let mut c = ctrl.borrow_mut();
                let handles = c.split_handles(area);
                if let Some(h) = handles.get(index as usize) {
                    // Nueva fracción ≈ (centro de la barra + delta) / dimensión del área.
                    // Para un split a un nivel es exacto; en anidados es una aproximación.
                    let (pos, total) = if matches!(h.dir, SplitDir::Horizontal) {
                        (h.rect.x + h.rect.w / 2.0 + dx, area.w.max(1.0))
                    } else {
                        (h.rect.y + h.rect.h / 2.0 + dy, area.h.max(1.0))
                    };
                    let path = h.path.clone();
                    c.set_fraction(&path, pos / total);
                }
            }
            // El arrastre cambia los rects → reconcilia geometría.
            sync_layout();
        });
    }

    // Reflow cuando cambia el tamaño del área de contenido (resize de ventana). Sin esto, un
    // resize en reposo (timer apagado) no repartiría los paneles.
    {
        let sync_layout = sync_layout.clone();
        ui.on_content_resized(move || sync_layout());
    }

    sync_layout();
    ui.run()
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
    }
}
