// Naygo — foco del cuadro de nombres del panel Favoritos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Key, Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle, LogicalPosition,
};
use std::{cell::RefCell, rc::Rc, time::Duration};

slint::slint! {
    import { FavoritesPanel } from "../ui/favorites-panel.slint";
    export component FavoritesFocusProbe inherits Window {
        width: 600px; height: 400px;
        callback created(string, string);
        FavoritesPanel {
            width: 100%; height: 100%; active: true;
            fav-tree: [{is-group: true, name: "Grupo", group-id: "grupo", name-path: "Grupo"}];
            new-group(parent, name) => { root.created(parent, name); }
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
        window.draw_if_needed(|r| {
            let mut pixels = vec![slint::Rgb8Pixel::default(); 600 * 400];
            r.render(&mut pixels, 600);
        });
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn click(ui: &FavoritesFocusProbe, x: f32, y: f32, button: PointerEventButton) {
    let position = LogicalPosition::new(x, y);
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    ui.window()
        .dispatch_event(WindowEvent::PointerPressed { position, button });
    ui.window()
        .dispatch_event(WindowEvent::PointerReleased { position, button });
}
#[test]
fn nuevo_grupo_recibe_texto_y_enter_sin_clic_adicional() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = FavoritesFocusProbe::new().unwrap();
    let created = Rc::new(RefCell::new(Vec::new()));
    ui.on_created({
        let created = created.clone();
        move |parent, name| {
            created
                .borrow_mut()
                .push((parent.to_string(), name.to_string()))
        }
    });
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(600, 400));
    pump(&window);
    click(&ui, 100.0, 42.0, PointerEventButton::Right);
    pump(&window);
    click(&ui, 115.0, 62.0, PointerEventButton::Left);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: "Nuevo".into(),
    });
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&window);
    assert_eq!(
        &*created.borrow(),
        &[("grupo".to_string(), "Nuevo".to_string())]
    );
}
