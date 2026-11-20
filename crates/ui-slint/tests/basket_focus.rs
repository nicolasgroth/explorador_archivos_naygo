// Naygo — continuidad del teclado de la bandeja tras diálogos superpuestos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Key, Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle, LogicalPosition,
};
use std::{cell::Cell, rc::Rc, time::Duration};

slint::slint! {
    import { LineEdit } from "std-widgets.slint";
    import { BasketPanel } from "../ui/basket-panel.slint";
    import { OpDialogs } from "../ui/op-dialogs.slint";
    import { CommandPalette } from "../ui/command-palette.slint";
    export component BasketFocusProbe inherits Window {
        width: 600px; height: 400px;
        in-out property <int> focused: 0;
        in-out property <bool> modal;
        in-out property <bool> extra-open;
        in-out property <bool> active: true;
        in-out property <string> other-text;
        callback selected(int);
        callback dismiss();
        callback opened();
        BasketPanel {
            width: 100%; height: 100%; active: root.active; focused-row: root.focused;
            focus-blocked: root.modal || root.extra-open;
            rows: [{name: "a.txt", path: "a"}, {name: "b.txt", path: "b"}];
            select(i, c, s) => { root.focused = i; root.selected(i); }
            open-list => { root.opened(); }
        }
        OpDialogs {
            width: 100%; height: 100%; vm: {kind: root.modal ? 1 : 0, del-count: 1};
            delete-cancel => { root.dismiss(); }
        }
        if root.extra-open: CommandPalette { width: 100%; height: 100%; }
        if !root.active && !root.modal && !root.extra-open: Rectangle {
            x: 350px; y: 5px; width: 200px; height: 30px;
            other := LineEdit { width: 100%; height: 100%; text <=> root.other-text; }
            init => { other-focus.running = true; }
            other-focus := Timer { interval: 0ms; running: false; triggered => { self.running = false; other.focus(); } }
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
    for _ in 0..5 {
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|renderer| {
            let mut pixels = vec![slint::Rgb8Pixel::default(); 600 * 400];
            renderer.render(&mut pixels, 600);
        });
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn closing_operation_dialog_returns_keyboard_to_basket_list() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = BasketFocusProbe::new().unwrap();
    let clicks = Rc::new(Cell::new(0));
    let opened = Rc::new(Cell::new(0));
    ui.on_selected({
        let clicks = clicks.clone();
        move |_| clicks.set(clicks.get() + 1)
    });
    ui.on_opened({
        let opened = opened.clone();
        move || opened.set(opened.get() + 1)
    });
    ui.on_dismiss({
        let weak = ui.as_weak();
        move || weak.unwrap().set_modal(false)
    });
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(600, 400));
    pump(&window);
    // Ubicar una fila por su callback, sin depender de altura o idioma del encabezado.
    let mut row_position = None;
    for y in (60..260).step_by(5) {
        let position = LogicalPosition::new(200.0, y as f32);
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
        pump(&window);
        if clicks.get() > 0 {
            row_position = Some(position);
            break;
        }
    }
    assert!(clicks.get() > 0, "se debe enfocar una fila de bandeja");
    ui.set_focused(0);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    pump(&window);
    assert_eq!(
        ui.get_focused(),
        1,
        "la bandeja recibe flechas antes del diálogo"
    );
    ui.set_modal(true);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    assert!(!ui.get_modal(), "Escape cierra el diálogo");
    ui.set_focused(0);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    pump(&window);
    assert_eq!(ui.get_focused(), 1, "la bandeja recupera flechas al cerrar");

    // Dos overlays: cerrar el primero no puede devolver el teclado antes del último.
    ui.set_focused(0);
    ui.set_modal(true);
    pump(&window);
    ui.set_extra_open(true);
    pump(&window);
    ui.set_modal(false);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    pump(&window);
    assert_eq!(
        ui.get_focused(),
        0,
        "la paleta conserva el foco mientras está abierta"
    );
    ui.set_extra_open(false);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    pump(&window);
    assert_eq!(
        ui.get_focused(),
        1,
        "la lista recupera foco al cerrar el último overlay"
    );

    // Si cambia el panel, no devolver foco a la bandeja inactiva.
    ui.set_modal(true);
    pump(&window);
    ui.set_active(false);
    ui.set_modal(false);
    pump(&window);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "q".into() });
    pump(&window);
    assert_eq!(
        ui.get_other_text().as_str(),
        "q",
        "el panel activo conserva el teclado"
    );
    ui.set_active(true);
    pump(&window);
    let position = row_position.unwrap();
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    pump(&window);
    ui.set_focused(0);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    pump(&window);
    assert_eq!(ui.get_focused(), 1, "al volver se recupera la lista");

    // Un botón interno no debe perder su contexto de teclado después del diálogo.
    for y in (20..80).step_by(3) {
        let position = LogicalPosition::new(195.0, y as f32);
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
        pump(&window);
        if opened.get() > 0 {
            break;
        }
    }
    assert!(opened.get() > 0, "se debe enfocar el botón abrir lista");
    let before = opened.get();
    ui.set_modal(true);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&window);
    assert_eq!(
        opened.get(),
        before + 1,
        "el botón enfocado vuelve a recibir Enter"
    );
}
