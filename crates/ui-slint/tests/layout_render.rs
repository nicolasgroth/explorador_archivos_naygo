// Naygo — regresión visual por software, sin abrir ventanas nativas ni tocar la sesión instalada.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, WindowAdapter,
    },
    ComponentHandle, Model,
};
use std::rc::Rc;

slint::slint! {
    import { BreadcrumbBar } from "../ui/breadcrumb-bar.slint";
    import { BasketPanel } from "../ui/basket-panel.slint";
    import { BasketRowVm } from "../ui/types.slint";
    export { BasketRowVm }
    export { Tr } from "../ui/i18n.slint";
    export component LayoutProbe inherits Window {
        in property <int> probe-width: 900;
        in property <bool> labels: false;
        out property <float> actual-width: self.width / 1px;
        width: root.probe-width * 1px; height: 460px; background: #0e1622;
        callback navigate(string);
        callback action(int);
        callback select(int);
        callback focus-basket();
        callback drag(int);
        in property <[BasketRowVm]> basket-rows: [{ name: "informe.txt", path: "D:\\Documentos\\informe.txt", selected: true }];
        BreadcrumbBar {
            x: 0px; y: 0px; width: parent.width; height: 28px; available-height: parent.height;
            segments: [{ label: "D:\\", path: "D:\\" }, { label: "Empresas", path: "D:\\Empresas" },
                { label: "ISGroth", path: "D:\\Empresas\\ISGroth" }, { label: "proyectos", path: "D:\\Empresas\\ISGroth\\proyectos" }];
            navigate(p) => { root.navigate(p); }
        }
        BasketPanel {
            show-labels: root.labels;
            x: 0px; y: 38px; width: parent.width; height: parent.height - 38px;
            rows: root.basket-rows;
            focus-pane => { root.focus-basket(); }
            select(index, control, shift) => { root.select(index); }
            drag-out(index) => { root.drag(index); }
            selected-count: 1;
            copy => { root.action(0); } move-files => { root.action(1); }
            delete-files => { root.action(2); } delivery => { root.action(3); }
            save-list => { root.action(4); } open-list => { root.action(5); }
            select-all => { root.action(6); } remove-selected => { root.action(7); }
            clear => { root.action(8); }
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
fn render_compact_paths_and_wrapping_basket_without_native_windows() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = LayoutProbe::new().unwrap();
    ui.global::<Tr>()
        .set_basket_title("Bandeja temporal".into());
    ui.global::<Tr>()
        .set_delivery_title("Preparar entrega".into());
    ui.global::<Tr>()
        .set_basket_scope("Acciones sobre marcados. Quitar conserva originales.".into());
    let action = Rc::new(std::cell::Cell::new(-1));
    let observed = action.clone();
    ui.on_action(move |index| observed.set(index));
    ui.show().unwrap();
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/agent-out");
    std::fs::create_dir_all(&output).unwrap();
    for width in [900, 360, 200, 110] {
        ui.set_probe_width(width as i32);
        window.set_size(slint::PhysicalSize::new(width, 460));
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        let mut pixels = vec![slint::Rgb8Pixel::default(); width as usize * 460];
        assert!(window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, width as usize);
        }));
        assert_eq!(ui.get_actual_width(), width as f32);
        assert!(
            pixels[..width as usize * 28]
                .iter()
                .any(|p| p.r > 80 && p.g > 80 && p.b > 80),
            "breadcrumb must be visible"
        );
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        if width == 900 {
            assert!(
                (0..28).all(
                    |y| pixels[y * width as usize + 350..(y + 1) * width as usize]
                        .iter()
                        .all(|p| p.r < 80)
                ),
                "short path must not stretch segments across the panel"
            );
        }
        image::save_buffer(
            output.join(format!("layout-051-{width}.png")),
            &bytes,
            width,
            460,
            image::ColorType::Rgb8,
        )
        .unwrap();
        let columns = ((width - 12 + 5) / 35).clamp(1, 8);
        for index in 0..8 {
            let position = slint::LogicalPosition::new(
                (21 + index % columns * 35) as f32,
                (77 + index / columns * 33) as f32,
            );
            action.set(-1);
            for event in [
                slint::platform::WindowEvent::PointerPressed {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                },
                slint::platform::WindowEvent::PointerReleased {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                },
            ] {
                window.dispatch_event(event);
            }
            assert_eq!(
                action.get(),
                index as i32,
                "action {index} at width {width}"
            );
        }
    }
    ui.set_labels(true);
    // Un sync de activación actualiza la fila sin destruir su TouchArea entre down/up.
    ui.set_probe_width(900);
    window.set_size(slint::PhysicalSize::new(900, 460));
    ui.set_labels(false);
    let basket = Rc::new(slint::VecModel::from(vec![BasketRowVm {
        name: "informe.txt".into(),
        path: "D:/Documentos/informe.txt".into(),
        selected: false,
        ..Default::default()
    }]));
    ui.set_basket_rows(slint::ModelRc::from(basket.clone()));
    let live_model = basket.clone();
    ui.on_focus_basket(move || {
        let mut row = live_model.row_data(0).unwrap();
        row.selected = true;
        live_model.set_row_data(0, row);
    });
    let selected = Rc::new(std::cell::Cell::new(-1));
    let observed = selected.clone();
    ui.on_select(move |index| observed.set(index));
    let dragged = Rc::new(std::cell::Cell::new(-1));
    let observed = dragged.clone();
    ui.on_drag(move |index| observed.set(index));
    window.request_redraw();
    let mut pixels = vec![slint::Rgb8Pixel::default(); 900 * 460];
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, 900);
    });
    let position = slint::LogicalPosition::new(100.0, 133.0);
    window.dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position,
        button: slint::platform::PointerEventButton::Left,
    });
    window.dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position,
        button: slint::platform::PointerEventButton::Left,
    });
    assert_eq!(
        selected.get(),
        0,
        "basket mouse selection survives activation sync"
    );
    selected.set(-1);
    window.dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position,
        button: slint::platform::PointerEventButton::Left,
    });
    window.dispatch_event(slint::platform::WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(120.0, 133.0),
    });
    window.dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position,
        button: slint::platform::PointerEventButton::Left,
    });
    assert_eq!(dragged.get(), 0);
    assert_eq!(
        selected.get(),
        -1,
        "drag must not become a selection click on release"
    );
    ui.set_labels(true);
    for width in [900, 200] {
        ui.set_probe_width(width);
        window.set_size(slint::PhysicalSize::new(width as u32, 460));
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        let mut pixels = vec![slint::Rgb8Pixel::default(); width as usize * 460];
        window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, width as usize);
        });
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        image::save_buffer(
            output.join(format!("basket-labels-{width}.png")),
            &bytes,
            width as u32,
            460,
            image::ColorType::Rgb8,
        )
        .unwrap();
        let columns = ((width - 12 + 5) / 159).clamp(1, 8);
        for index in 0..4 {
            let position = slint::LogicalPosition::new(
                (21 + index % columns * 159) as f32,
                (77 + index / columns * 33) as f32,
            );
            action.set(-1);
            for event in [
                slint::platform::WindowEvent::PointerPressed {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                },
                slint::platform::WindowEvent::PointerReleased {
                    position,
                    button: slint::platform::PointerEventButton::Left,
                },
            ] {
                window.dispatch_event(event);
            }
            assert_eq!(
                action.get(),
                index,
                "labeled action {index} at width {width}"
            );
        }
    }
    // Desplegar las acciones secundarias en el panel estrecho y probar Guardar lista.
    let position = slint::LogicalPosition::new(100.0, 210.0);
    for event in [
        slint::platform::WindowEvent::PointerPressed {
            position,
            button: slint::platform::PointerEventButton::Left,
        },
        slint::platform::WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        },
    ] {
        window.dispatch_event(event);
    }
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    let mut pixels = vec![slint::Rgb8Pixel::default(); 200 * 460];
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, 200);
    });
    image::save_buffer(
        output.join("basket-labels-expanded-200.png"),
        &pixels
            .iter()
            .flat_map(|p| [p.r, p.g, p.b])
            .collect::<Vec<_>>(),
        200,
        460,
        image::ColorType::Rgb8,
    )
    .unwrap();
    let position = slint::LogicalPosition::new(21.0, 209.0);
    action.set(-1);
    for event in [
        slint::platform::WindowEvent::PointerPressed {
            position,
            button: slint::platform::PointerEventButton::Left,
        },
        slint::platform::WindowEvent::PointerReleased {
            position,
            button: slint::platform::PointerEventButton::Left,
        },
    ] {
        window.dispatch_event(event);
    }
    assert_eq!(
        action.get(),
        4,
        "expanded secondary action remains reachable"
    );
    ui.hide().unwrap();
}

#[test]
fn full_license_and_version_notes_are_embedded_from_canonical_documents() {
    let license = include_str!("../../../LICENSE");
    for clause in [
        "Permission is hereby granted",
        "The above copyright notice",
        "THE SOFTWARE IS PROVIDED",
        "SOFTWARE.",
        "ngroth@gmail.com",
    ] {
        assert!(license.contains(clause));
    }
    let notes = naygo_core::changelog::release_notes(
        include_str!("../../../CHANGELOG.md"),
        env!("CARGO_PKG_VERSION"),
    )
    .unwrap();
    assert!(!notes.sections.is_empty());
}
