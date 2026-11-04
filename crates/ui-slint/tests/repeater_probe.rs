// Naygo — experimento aislado sobre callbacks de un componente repetido.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

slint::slint! {
    struct Pane { id: int }
    component Panel inherits Rectangle {
        in property <int> id;
        in property <int> query;
        callback report(int, int);
        changed query => { root.report(root.id, root.query); }
    }
    export component Probe inherits Window {
        in property <[Pane]> panes;
        in-out property <int> query;
        callback report(int, int);
        for pane in root.panes: Panel {
            id: pane.id;
            query: root.query;
            report(id, q) => { root.report(id, q); }
        }
    }
}

#[test]
fn mide_entrega_de_cambio_en_componentes_repetidos() {
    let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
    );
    struct Headless(Rc<slint::platform::software_renderer::MinimalSoftwareWindow>);
    impl slint::platform::Platform for Headless {
        fn create_window_adapter(
            &self,
        ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            Ok(self.0.clone())
        }
    }
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = Probe::new().unwrap();
    ui.set_panes(ModelRc::new(VecModel::from(vec![
        Pane { id: 1 },
        Pane { id: 2 },
    ])));
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(400, 300));
    let mut pixels = vec![slint::Rgb8Pixel::default(); 400 * 300];
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, 400);
    });
    slint::platform::update_timers_and_animations();
    let events = Rc::new(RefCell::new(Vec::new()));
    let shared = events.clone();
    ui.on_report(move |id, q| shared.borrow_mut().push((id, q)));
    ui.set_query(11);
    eprintln!("immediate: {:?}", events.borrow());
    assert!(
        events.borrow().is_empty(),
        "el callback no se dispara al asignar la propiedad"
    );
    slint::platform::update_timers_and_animations();
    eprintln!("after update: {:?}", events.borrow());
    assert_eq!(events.borrow().as_slice(), &[(1, 11), (2, 11)]);
}
