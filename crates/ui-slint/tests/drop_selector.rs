// Naygo — el selector de drop exige una elección consciente.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Key, Platform, WindowAdapter, WindowEvent,
    },
    ComponentHandle,
};
use std::{cell::Cell, rc::Rc};

slint::slint! {
    import { DestinationRadar } from "../ui/destination-radar.slint";
    export component Probe inherits Window {
        width: 640px; height: 480px;
        callback chosen(int);
        DestinationRadar {
            width: 100%; height: 100%; require-explicit: true; title: "Elegir";
            rows: [{number: 1, label: "Panel 1", path: "C:\\destino"}];
            choose(i) => { root.chosen(i); }
        }
    }
}
struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

#[test]
fn enter_no_elige_hasta_mover_la_seleccion() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = Probe::new().unwrap();
    let chosen = Rc::new(Cell::new(-1));
    ui.on_chosen({
        let chosen = chosen.clone();
        move |i| chosen.set(i)
    });
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(640, 480));
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    window.draw_if_needed(|r| {
        let mut pixels = vec![slint::Rgb8Pixel::default(); 640 * 480];
        r.render(&mut pixels, 640);
    });
    slint::platform::update_timers_and_animations();
    let key = |k: Key| {
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: k.into() });
    };
    key(Key::Return);
    assert_eq!(chosen.get(), -1);
    key(Key::DownArrow);
    key(Key::Return);
    assert_eq!(chosen.get(), 0);
}
