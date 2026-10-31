// Naygo — regresiones de navegación y activación por teclado de pestañas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Key, Platform, WindowAdapter, WindowEvent,
    },
    ComponentHandle, ModelRc, VecModel,
};
use std::{cell::Cell, rc::Rc};

slint::slint! {
    import { TabBar } from "../ui/tab-bar.slint";
    import { TabVm } from "../ui/types.slint";

    export component TabBarProbe inherits Window {
        callback selected(int);
        callback closed(int);
        in property <[TabVm]> tabs;

        tab-bar := TabBar {
            x: 0px;
            y: 0px;
            width: 320px;
            height: 26px;
            tabs: root.tabs;
            active-tab-index: 0;
            select(id) => { root.selected(id); }
            close(id) => { root.closed(id); }
        }
    }
}

struct Headless(Rc<MinimalSoftwareWindow>);

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

fn key_event(ui: &TabBarProbe, pressed: bool, key: impl Into<slint::SharedString>) {
    let event = if pressed {
        WindowEvent::KeyPressed { text: key.into() }
    } else {
        WindowEvent::KeyReleased { text: key.into() }
    };
    ui.window().dispatch_event(event);
}

fn click_first_tab(ui: &TabBarProbe) {
    let position = slint::LogicalPosition::new(18.0, 13.0);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: slint::platform::PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: slint::platform::PointerEventButton::Left,
    });
}

fn probe() -> TabBarProbe {
    let ui = TabBarProbe::new().unwrap();
    ui.set_tabs(ModelRc::from(Rc::new(VecModel::from(vec![
        TabVm {
            id: 10,
            label: "Panel uno".into(),
            active: true,
        },
        TabVm {
            id: 20,
            label: "Panel dos".into(),
            active: false,
        },
        TabVm {
            id: 30,
            label: "Panel tres".into(),
            active: false,
        },
    ]))));
    ui
}

#[test]
fn pestañas_exponen_navegación_por_flechas_y_activación_con_enter_o_espacio() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window))).unwrap();
    let ui = probe();
    let selected = Rc::new(Cell::new(-1));
    ui.on_selected({
        let selected = selected.clone();
        move |id| selected.set(id)
    });
    ui.show().unwrap();
    click_first_tab(&ui);

    key_event(&ui, true, Key::RightArrow);
    assert_eq!(selected.get(), 10, "mover el foco no activa otra pestaña");
    key_event(&ui, false, " ");
    assert_eq!(selected.get(), 20);
    click_first_tab(&ui);

    key_event(&ui, true, Key::LeftArrow);
    key_event(&ui, true, Key::Return);
    assert_eq!(
        selected.get(),
        10,
        "la flecha izquierda queda en la primera pestaña"
    );
}
