// Naygo — teclado y foco del detalle de archivos de operaciones.
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
    import { OpFileListModal } from "../../ui-gen/ui/op-file-list-modal.slint";
    export component HistoryProbe inherits Window {
        width: 500px; height: 350px;
        in-out property <bool> modal;
        in-out property <bool> active: true;
        in-out property <int> operation-id: 42;
        callback opened(int);
        OpsPanel {
            width: 100%; height: 100%; active: root.active; focus-blocked: root.modal;
            history-count: 1;
            history-rows: [{ index: root.operation-id, label: "Copiar", status: "Listo", has-file-list: true, known-files: 3 }];
            focus-pane => { root.active = true; }
            show-files(id) => { root.opened(id); root.modal = true; }
        }
        OpFileListModal { width: 100%; height: 100%; open: root.modal; close => { root.modal = false; } }
    }
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
            let mut p = vec![slint::Rgb8Pixel::default(); 500 * 350];
            r.render(&mut p, 500);
        });
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn detalle_historial_recupera_enter_y_rechaza_identidad_cambiada() {
    let w = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(w.clone()))).unwrap();
    let ui = HistoryProbe::new().unwrap();
    let count = Rc::new(Cell::new(0));
    ui.on_opened({
        let count = count.clone();
        move |id| {
            assert_eq!(id, 42);
            count.set(count.get() + 1);
        }
    });
    ui.show().unwrap();
    w.set_size(slint::PhysicalSize::new(500, 350));
    pump(&w);
    ui.set_active(false);
    pump(&w);
    // El control debe ser alcanzable sin mouse. El único otro control enfocable del panel es
    // el filtro del historial; probar varios Tab cubre ambas posiciones sin depender del orden.
    for _ in 0..8 {
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Tab.into(),
        });
        pump(&w);
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Return.into(),
        });
        pump(&w);
        if count.get() > 0 {
            break;
        }
    }
    assert_eq!(count.get(), 1, "Tab y Enter abren el detalle sin mouse");
    assert!(ui.get_active(), "el enlace activa su panel");
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&w);
    let mut link_position = None;
    for y in (35..250).step_by(4) {
        let position = LogicalPosition::new(40.0, y as f32);
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
        if count.get() > 1 {
            link_position = Some(position);
            break;
        }
    }
    assert_eq!(count.get(), 2, "el enlace abrió el detalle con mouse");
    assert!(ui.get_active(), "el enlace activa su panel");
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&w);
    assert!(!ui.get_modal());
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(count.get(), 3, "Enter reabre el detalle");
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: " ".into() });
    pump(&w);
    assert_eq!(count.get(), 4, "espacio también abre el detalle");
    ui.set_operation_id(43);
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(
        count.get(),
        4,
        "una fila reemplazada no debe activar otra operación"
    );
    ui.set_operation_id(42);
    pump(&w);
    let position = link_position.unwrap();
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
    assert_eq!(count.get(), 5);
    ui.set_active(false);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(count.get(), 5, "el panel inactivo no recupera el foco");
}
