// Naygo — Cancelar/Reintentar sobre preview Slint real con miniatura sintética.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle,
};
use std::{cell::Cell, rc::Rc};
slint::slint! {
    import { PreviewPanel } from "../ui/preview-panel.slint";
    export { Theme } from "../ui/theme.slint";
    export component PreviewProbe inherits Window {
        width: 420px; height: 240px;
        in property <bool> retry;
        in property <image> poster;
        in property <int> animation-state;
        in property <bool> mesh;
        callback cancel(); callback again();
        callback animation-toggle();
        callback color-toggle(); callback surface-toggle();
        PreviewPanel {
            width: 100%; height: 100%;
            view: {mode: root.retry ? 3 : 2, path: "model.stl", image: root.poster,
                message: "Cancelado", loading: false, can-cancel: !root.retry, can-retry: root.retry,
                animation-state: root.animation-state, mesh-interactive: root.mesh};
            cancel-preview => { root.cancel(); }
            retry-preview => { root.again(); }
            toggle-animation => { root.animation-toggle(); }
            cycle-color => { root.color-toggle(); }
            cycle-surface => { root.surface-toggle(); }
        }
    }
}
struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn click(ui: &PreviewProbe, x: f32, y: f32) {
    let position = slint::LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    slint::platform::update_timers_and_animations();
}
#[test]
fn cancel_is_clickable_over_ready_poster_and_retry_is_explicit() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = PreviewProbe::new().unwrap();
    let cancels = Rc::new(Cell::new(0));
    let retries = Rc::new(Cell::new(0));
    ui.on_cancel({
        let n = cancels.clone();
        move || n.set(n.get() + 1)
    });
    ui.on_again({
        let n = retries.clone();
        move || n.set(n.get() + 1)
    });
    // Colores exclusivos para localizar los controles por su render, sin coordenadas
    // frágiles que dependan de la longitud traducida del título o de otros botones.
    ui.global::<Theme>()
        .set_error(slint::Color::from_rgb_u8(230, 17, 33));
    ui.global::<Theme>()
        .set_surface_raised(slint::Color::from_rgb_u8(17, 230, 33));
    ui.set_poster(slint::Image::from_rgba8(slint::SharedPixelBuffer::new(
        2, 2,
    )));
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(420, 240));
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/agent-out");
    std::fs::create_dir_all(&out).unwrap();
    for retry in [false, true] {
        ui.set_retry(retry);
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        let mut pixels = vec![slint::Rgb8Pixel::default(); 420 * 240];
        window.draw_if_needed(|r| {
            r.render(&mut pixels, 420);
        });
        let color = if retry { (17, 230, 33) } else { (230, 17, 33) };
        let index = pixels[..420 * 32]
            .iter()
            .position(|p| (p.r, p.g, p.b) == color)
            .expect("control visible junto al título");
        let bytes: Vec<_> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        image::save_buffer(
            out.join(if retry {
                "rp4a-preview-retry.png"
            } else {
                "rp4a-preview-cancel.png"
            }),
            &bytes,
            420,
            240,
            image::ColorType::Rgb8,
        )
        .unwrap();
        click(&ui, (index % 420) as f32 + 1., (index / 420) as f32 + 2.);
    }
    assert_eq!(cancels.get(), 1);
    assert_eq!(retries.get(), 1);
    let toggles = Rc::new(Cell::new(0));
    ui.on_animation_toggle({
        let n = toggles.clone();
        move || n.set(n.get() + 1)
    });
    ui.set_retry(false);
    for state in [1, 2, 3] {
        // Each action replaces the previous control under the cursor. Move off the header
        // before locating its normal-state color; hover legitimately changes that color.
        ui.window().dispatch_event(WindowEvent::PointerMoved {
            position: slint::LogicalPosition::new(8., 220.),
        });
        ui.set_animation_state(state);
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        let mut pixels = vec![slint::Rgb8Pixel::default(); 420 * 240];
        window.draw_if_needed(|r| {
            r.render(&mut pixels, 420);
        });
        let bytes: Vec<_> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
        image::save_buffer(
            out.join(format!("rp5-preview-animation-{state}.png")),
            &bytes,
            420,
            240,
            image::ColorType::Rgb8,
        )
        .unwrap();
        let index = pixels[..420 * 32]
            .iter()
            .position(|p| (p.r, p.g, p.b) == (17, 230, 33))
            .expect("animation action visible");
        click(&ui, (index % 420) as f32 + 2., (index / 420) as f32 + 2.);
    }
    assert_eq!(toggles.get(), 3);
    let color_toggles = Rc::new(Cell::new(0));
    let surface_toggles = Rc::new(Cell::new(0));
    ui.on_color_toggle({
        let n = color_toggles.clone();
        move || n.set(n.get() + 1)
    });
    ui.on_surface_toggle({
        let n = surface_toggles.clone();
        move || n.set(n.get() + 1)
    });
    ui.set_animation_state(0);
    ui.set_mesh(true);
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    let mut pixels = vec![slint::Rgb8Pixel::default(); 420 * 240];
    window.draw_if_needed(|r| {
        r.render(&mut pixels, 420);
    });
    let index = pixels[..420 * 32]
        .iter()
        .position(|p| (p.r, p.g, p.b) == (117, 181, 222))
        .expect("3D color control visible");
    let x = (index % 420) as f32 + 3.;
    let y = (index / 420) as f32 + 3.;
    click(&ui, x, y);
    click(&ui, x + 26., y);
    assert_eq!(color_toggles.get(), 1);
    assert_eq!(surface_toggles.get(), 1);
    ui.hide().unwrap();
}
