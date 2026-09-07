// Naygo — prueba de controles y tipografía por render software, sin ventanas nativas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, WindowAdapter,
    },
    ComponentHandle,
};
use std::rc::Rc;

slint::slint! {
    import { OpButton } from "../ui/ops-panel.slint";
    import { DriveBtn } from "../ui/drive-button.slint";
    import { FontChoice } from "../ui/font-choice.slint";
    export { Tr } from "../ui/i18n.slint";
    export component ControlsProbe inherits Window {
        width: 700px; height: 220px; background: #0e1622;
        in property <image> drive-icon;
        in property <string> family: "Segoe UI";
        callback font-chosen(string);
        OpButton { x: 20px; y: 20px; label: "Pausar"; show-pause: true; }
        OpButton { x: 130px; y: 20px; label: "Saltar"; show-skip: true; }
        DriveBtn { x: 250px; y: 20px; height: 30px; label: "C:"; icon: root.drive-icon; }
        DriveBtn { x: 350px; y: 20px; height: 30px; label: "USB"; removable: true; }
        FontChoice { x: 20px; y: 80px; width: 650px;
            label: "Interfaz"; default-label: "Sistema"; family: root.family;
            apply(family) => { root.font-chosen(family); }
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
fn icons_are_centered_in_full_height_buttons_and_fonts_render() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = ControlsProbe::new().unwrap();
    ui.global::<Tr>().set_dlg_accept("Aceptar".into());
    let mut icon = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(15, 15);
    icon.make_mut_slice()
        .fill(slint::Rgba8Pixel::new(255, 255, 255, 255));
    ui.set_drive_icon(slint::Image::from_rgba8(icon));
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(700, 220));
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/agent-out");
    std::fs::create_dir_all(&output).unwrap();
    for family in ["Segoe UI", "Consolas", "Naygo Missing Font Test"] {
        ui.set_family(family.into());
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        let mut pixels = vec![slint::Rgb8Pixel::default(); 700 * 220];
        window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 700);
        });
        for (left, right) in [(32, 45), (142, 155), (255, 270)] {
            let mut ys = Vec::new();
            for y in 20..50 {
                for x in left..right {
                    let p = pixels[y * 700 + x];
                    if p.r > 120 && p.g > 120 && p.b > 120 {
                        ys.push(y);
                    }
                }
            }
            assert!(!ys.is_empty(), "icon pixels at {left}");
            let center = (*ys.iter().min().unwrap() + *ys.iter().max().unwrap()) as f32 / 2.0;
            assert!(
                (center - 34.5).abs() <= 2.0,
                "icon at {left} centered at {center}, expected 34.5"
            );
        }
        image::save_buffer(
            output.join(format!("controls-{}.png", family.replace(' ', "-"))),
            &pixels
                .iter()
                .flat_map(|p| [p.r, p.g, p.b])
                .collect::<Vec<_>>(),
            700,
            220,
            image::ColorType::Rgb8,
        )
        .unwrap();
    }
    ui.hide().unwrap();
}
