// Naygo — clic derecho sobre selección múltiple en el panel de archivos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle, LogicalPosition,
};
use std::{cell::RefCell, rc::Rc};

slint::slint! {
    import { FilePanel } from "../../ui-gen/ui/file-panel.slint";
    export component Probe inherits Window {
        width: 700px; height: 500px;
        callback row-clicked(int);
        callback row-context(int);
        FilePanel {
            width: 100%; height: 100%; active: true;
            rows: [{ name: "a", selected: true }, { name: "b", selected: true }, { name: "c" }];
            columns: [{ kind: 0, label: "Name", width: 300px }];
            row-clicked(id, pos, c, s) => { root.row-clicked(pos); }
            row-context(id, pos, x, y) => { root.row-context(pos); }
        }
    }
}
struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn pump(w: &MinimalSoftwareWindow) {
    for _ in 0..3 {
        slint::platform::update_timers_and_animations();
        w.draw_if_needed(|r| {
            let mut p = vec![slint::Rgb8Pixel::default(); 700 * 500];
            r.render(&mut p, 700);
        });
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[test]
fn right_click_passes_row_to_context_without_emitting_left_click() {
    let w = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(w.clone()))).unwrap();
    let ui = Probe::new().unwrap();
    let clicks = Rc::new(RefCell::new(Vec::new()));
    let contexts = Rc::new(RefCell::new(Vec::new()));
    ui.on_row_clicked({
        let v = clicks.clone();
        move |p| v.borrow_mut().push(p)
    });
    ui.on_row_context({
        let v = contexts.clone();
        move |p| v.borrow_mut().push(p)
    });
    ui.show().unwrap();
    w.set_size(slint::PhysicalSize::new(700, 500));
    pump(&w);
    for y in [62., 106.] {
        let position = LogicalPosition::new(100., y);
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Right,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Right,
        });
        pump(&w);
    }
    assert_eq!(&*contexts.borrow(), &[0, 2]);
    assert!(
        clicks.borrow().is_empty(),
        "el clic derecho no debe generar un clic izquierdo"
    );
}
