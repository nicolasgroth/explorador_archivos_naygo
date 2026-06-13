// Naygo — arranque de la capa UI en Slint (Fase 1).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Para forzar el renderizador por software (caso VM sin GPU):
//   $env:SLINT_BACKEND="winit-software"; cargo run -p naygo-ui-slint
mod bridge;
mod controller;
mod keys;
mod listing;

use controller::Controller;
use slint::{ModelRc, SharedString, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let start = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/"));

    let ctrl = Rc::new(RefCell::new(Controller::new(start)));
    let model: Rc<VecModel<RowData>> = Rc::new(VecModel::default());
    ui.set_rows(ModelRc::from(model.clone()));

    // Refresca el modelo + path + scroll desde el estado del controller.
    let refresh = {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let model = model.clone();
        move || {
            let c = ctrl.borrow();
            let rows: Vec<RowData> = c.rows().into_iter().map(to_row_data).collect();
            model.set_vec(rows);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_current_path(SharedString::from(c.current_path().as_str()));
                ui.set_panel_scroll_y(c.focus_scroll_y());
            }
        }
    };

    // Timer del listado: drena por lotes ~30ms mientras hay listado activo; se apaga al
    // terminar (0 trabajo en reposo). Se (re)arranca tras cada navegacion.
    let listing_timer = Rc::new(slint::Timer::default());
    let start_timer: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let timer = listing_timer.clone();
        Rc::new(move || {
            let ctrl = ctrl.clone();
            let refresh = refresh.clone();
            let timer2 = timer.clone();
            timer.start(
                TimerMode::Repeated,
                std::time::Duration::from_millis(30),
                move || {
                    let done = ctrl.borrow_mut().pump_listing();
                    refresh();
                    if done {
                        timer2.stop();
                    }
                },
            );
        })
    };
    start_timer(); // listado inicial

    // Cablear callbacks.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_row_double_clicked(move |i| {
            if ctrl.borrow_mut().on_row_double_clicked(i as usize) {
                start_timer();
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        ui.on_row_clicked(move |i| {
            ctrl.borrow_mut().on_row_clicked(i as usize);
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
        ui.on_sort_by(move |col| {
            ctrl.borrow_mut().on_sort_by(col.as_str());
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

    refresh();
    ui.run()
}

/// Convierte la fila plana del bridge al `RowData` generado por Slint.
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
