// Naygo — interacción de puntero con el submenú contextual de carpetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle, LogicalPosition,
};
use std::{cell::Cell, rc::Rc};

slint::slint! {
    import { ContextMenu } from "../../ui-gen/ui/context-menu.slint";
    export component ContextProbe inherits Window {
        width: 800px; height: 700px;
        in property <length> menu-x: 100px;
        callback explorer();
        ContextMenu {
            width: 100%; height: 100%;
            vm: { active: true, x: root.menu-x, y: 100px, target-is-folder: true, has-selection: true };
            explorer => { root.explorer(); }
        }
    }
}

struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn pump(window: &MinimalSoftwareWindow) {
    for _ in 0..3 {
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|renderer| {
            let mut pixels = vec![slint::Rgb8Pixel::default(); 800 * 700];
            renderer.render(&mut pixels, 800);
        });
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[test]
fn pointer_can_cross_into_open_submenu_and_choose_explorer() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = ContextProbe::new().unwrap();
    let clicked = Rc::new(Cell::new(0));
    ui.on_explorer({
        let clicked = clicked.clone();
        move || clicked.set(clicked.get() + 1)
    });
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(800, 700));
    pump(&window);
    for (x, y) in [
        (150., 115.),
        (300., 115.),
        (310., 115.),
        (350., 115.),
        (350., 200.),
    ] {
        ui.window().dispatch_event(WindowEvent::PointerMoved {
            position: LogicalPosition::new(x, y),
        });
        pump(&window);
    }
    let position = LogicalPosition::new(350., 200.);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    pump(&window);
    assert_eq!(
        clicked.get(),
        1,
        "el submenú debe seguir accesible al mover el puntero"
    );
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(5., 5.),
    });
    ui.set_menu_x(700.0);
    pump(&window);
    for (x, y) in [
        (750., 115.),
        (600., 115.),
        (585., 115.),
        (540., 115.),
        (500., 200.),
    ] {
        ui.window().dispatch_event(WindowEvent::PointerMoved {
            position: LogicalPosition::new(x, y),
        });
        pump(&window);
    }
    let position = LogicalPosition::new(500., 200.);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    pump(&window);
    assert_eq!(
        clicked.get(),
        2,
        "junto al borde el submenú debe abrir hacia la izquierda"
    );
}
