// Naygo — auditoría de foco de botones de operaciones ante diálogos.
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
    import { OpsPanel } from "../../ui-gen/ui/ops-panel.slint";
    import { OpRowVm } from "../../ui-gen/ui/types.slint";
    export { OpRowVm }
    import { MessageModal } from "../../ui-gen/ui/message-modal.slint";
    import { CommandPalette } from "../../ui-gen/ui/command-palette.slint";
    import { LineEdit } from "std-widgets.slint";
    export component OpsFocusProbe inherits Window {
        width: 400px; height: 300px;
        in-out property <bool> modal;
        in-out property <bool> extra-open;
        in-out property <bool> active: true;
        in-out property <string> other-text;
        in property <[OpRowVm]> queued;
        callback pressed(); callback dismiss(); callback cancel-row(int);
        OpsPanel { width: 100%; height: 100%; active: root.active;
            running-count: root.queued.length == 0 ? 1 : 0;
            queued-count: root.queued.length; queued-rows: root.queued;
            focus-blocked: root.modal || root.extra-open; focus-pane => { root.active = true; }
            cancel(id) => { root.cancel-row(id); }
            cancel-all => { root.pressed(); } }
        MessageModal { width: 100%; height: 100%; vm: {kind: root.modal ? 1 : 0}; message-cancel => { root.dismiss(); } }
        if root.extra-open: CommandPalette { width: 100%; height: 100%; }
        if !root.active && !root.modal && !root.extra-open: Rectangle {
            x: 100px; y: 100px; width: 200px; height: 30px;
            other := LineEdit { width: 100%; height: 100%; text <=> root.other-text; }
            init => { other-focus.running = true; }
            other-focus := Timer { interval: 0ms; running: false; triggered => { self.running = false; other.focus(); } }
        }
    }
}

#[test]
fn cancelar_fila_recupera_foco_solo_para_la_misma_operacion() {
    let w = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(w.clone()))).unwrap();
    let ui = OpsFocusProbe::new().unwrap();
    let queue = Rc::new(slint::VecModel::from(vec![OpRowVm {
        index: 17,
        ..Default::default()
    }]));
    ui.set_queued(slint::ModelRc::from(queue.clone()));
    let cancelled = Rc::new(Cell::new(0));
    ui.on_cancel_row({
        let cancelled = cancelled.clone();
        move |id| cancelled.set(id)
    });
    ui.show().unwrap();
    w.set_size(slint::PhysicalSize::new(400, 300));
    pump(&w);
    for _ in 0..8 {
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Tab.into(),
        });
        pump(&w);
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Return.into(),
        });
        pump(&w);
        if cancelled.get() != 0 {
            break;
        }
    }
    assert_eq!(
        cancelled.get(),
        17,
        "Tab y Enter alcanzan la acción de la fila"
    );
    cancelled.set(0);
    ui.set_modal(true);
    pump(&w);
    ui.set_modal(false);
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(
        cancelled.get(),
        17,
        "la operación conserva su botón tras cerrar el modal"
    );

    cancelled.set(0);
    ui.set_modal(true);
    pump(&w);
    slint::Model::set_row_data(
        &*queue,
        0,
        OpRowVm {
            index: 29,
            ..Default::default()
        },
    );
    pump(&w);
    ui.set_modal(false);
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(
        cancelled.get(),
        0,
        "Enter no cancela una operación que reutilizó la fila"
    );
}
struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn pump(w: &MinimalSoftwareWindow) {
    for _ in 0..5 {
        slint::platform::update_timers_and_animations();
        w.draw_if_needed(|r| {
            let mut p = vec![slint::Rgb8Pixel::default(); 400 * 300];
            r.render(&mut p, 400);
        });
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn boton_operaciones_recupera_teclado_tras_dialogo() {
    let w = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(w.clone()))).unwrap();
    let ui = OpsFocusProbe::new().unwrap();
    let presses = Rc::new(Cell::new(0));
    ui.on_pressed({
        let presses = presses.clone();
        move || presses.set(presses.get() + 1)
    });
    ui.on_dismiss({
        let weak = ui.as_weak();
        move || weak.unwrap().set_modal(false)
    });
    ui.show().unwrap();
    w.set_size(slint::PhysicalSize::new(400, 300));
    pump(&w);
    for x in (250..400).step_by(5) {
        for y in (5..60).step_by(5) {
            let position = LogicalPosition::new(x as f32, y as f32);
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
            pump(&w);
            if presses.get() > 0 {
                break;
            }
        }
        if presses.get() > 0 {
            break;
        }
    }
    assert_eq!(presses.get(), 1, "se enfocó Cancelar todo");
    ui.set_modal(true);
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&w);
    assert!(!ui.get_modal());
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(
        presses.get(),
        2,
        "el botón recupera Enter tras cerrar el diálogo"
    );
    ui.set_modal(true);
    pump(&w);
    ui.set_extra_open(true);
    pump(&w);
    ui.set_modal(false);
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(presses.get(), 2, "no restaurar mientras queda otro overlay");
    ui.set_extra_open(false);
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(presses.get(), 3, "restaurar tras cerrar el último overlay");
    ui.set_modal(true);
    pump(&w);
    ui.set_active(false);
    ui.set_modal(false);
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "q".into() });
    pump(&w);
    assert_eq!(
        ui.get_other_text().as_str(),
        "q",
        "no robar foco al nuevo panel"
    );
}
