// Naygo — arranque de la capa UI en Slint (Fase 2a: multi-panel).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Para forzar el renderizador por software (caso VM sin GPU):
//   $env:SLINT_BACKEND="winit-software"; cargo run -p naygo-ui-slint
mod bridge;
mod keys;
mod listing;
mod workspace_ctrl;

use naygo_core::workspace::layout::{Rect, SplitDir};
use naygo_core::workspace::PaneId;
use slint::{ModelRc, SharedString, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use workspace_ctrl::WorkspaceCtrl;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let start = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
    let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new(start)));

    // Reconstruye los modelos (paneles + splitters + path activo) desde el estado.
    let refresh = {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let area = Rect {
                x: 0.0,
                y: 0.0,
                w: ui.get_content_w().max(0.0),
                h: ui.get_content_h().max(0.0),
            };
            let c = ctrl.borrow();
            let active = c.active_id();
            let panes: Vec<PaneVm> = c
                .pane_rects(area)
                .into_iter()
                .map(|(id, r)| {
                    let rows: Vec<RowData> = c.rows_of(id).into_iter().map(to_row_data).collect();
                    PaneVm {
                        id: id.0 as i32,
                        x: r.x,
                        y: r.y,
                        w: r.w,
                        h: r.h,
                        path: SharedString::from(c.path_of(id).as_str()),
                        rows: ModelRc::from(Rc::new(VecModel::from(rows))),
                        active: Some(id) == active,
                    }
                })
                .collect();
            ui.set_panes(ModelRc::from(Rc::new(VecModel::from(panes))));

            let splits: Vec<SplitVm> = c
                .split_handles(area)
                .into_iter()
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
            ui.set_splits(ModelRc::from(Rc::new(VecModel::from(splits))));

            if let Some(id) = active {
                ui.set_active_path(SharedString::from(c.path_of(id).as_str()));
            }
        }
    };

    // Timer que drena los listados activos; se apaga cuando todos terminan.
    let timer = Rc::new(slint::Timer::default());
    let start_timer: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let timer = timer.clone();
        Rc::new(move || {
            let ctrl = ctrl.clone();
            let refresh = refresh.clone();
            let timer2 = timer.clone();
            timer.start(
                TimerMode::Repeated,
                std::time::Duration::from_millis(30),
                move || {
                    let all_done = ctrl.borrow_mut().pump_listings();
                    refresh();
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
        let refresh = refresh.clone();
        ui.on_row_clicked(move |id, pos| {
            ctrl.borrow_mut()
                .on_row_clicked(PaneId(id as u64), pos as usize);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_row_double_clicked(move |id, pos| {
            if ctrl
                .borrow_mut()
                .on_row_double_clicked(PaneId(id as u64), pos as usize)
            {
                start_timer();
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        ui.on_sort_by(move |_id, col| {
            ctrl.borrow_mut().on_sort_by(col.as_str());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        ui.on_activate(move |id| {
            ctrl.borrow_mut().set_active(PaneId(id as u64));
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_key(move |text, c, s, a| {
            if ctrl.borrow_mut().on_key(text.as_str(), c, s, a) {
                start_timer();
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_go_up(move || {
            if ctrl.borrow_mut().on_go_up() {
                start_timer();
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_add_pane(move || {
            ctrl.borrow_mut().add_pane_split();
            start_timer();
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let ui_weak = ui.as_weak();
        ui.on_split_drag(move |index, dx, dy| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let area = Rect {
                x: 0.0,
                y: 0.0,
                w: ui.get_content_w().max(1.0),
                h: ui.get_content_h().max(1.0),
            };
            let mut c = ctrl.borrow_mut();
            let handles = c.split_handles(area);
            if let Some(h) = handles.get(index as usize) {
                // Nueva fracción ≈ (centro de la barra + delta) / dimensión del área. Para
                // un split a un nivel es exacto; en anidados es una aproximación que se
                // afina si hace falta (el área del split anidado difiere del total).
                let (pos, total) = if matches!(h.dir, SplitDir::Horizontal) {
                    (h.rect.x + h.rect.w / 2.0 + dx, area.w.max(1.0))
                } else {
                    (h.rect.y + h.rect.h / 2.0 + dy, area.h.max(1.0))
                };
                let path = h.path.clone();
                c.set_fraction(&path, pos / total);
            }
            drop(c);
            refresh();
        });
    }

    refresh();
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
