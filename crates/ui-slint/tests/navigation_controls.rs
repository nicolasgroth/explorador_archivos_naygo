// Naygo — regresiones visuales del resaltado y reinicio del modal de carpetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Platform, WindowAdapter, WindowEvent,
    },
    ComponentHandle,
};
use std::{cell::RefCell, rc::Rc};
slint::slint! {
    import { MatchedName } from "../ui/matched-name.slint";
    import { NewFolder } from "../ui/new-folder.slint";
    export { Theme } from "../ui/theme.slint";
    export component NavigationProbe inherits Window {
        width: 600px; height: 420px; background: black;
        in property <bool> folder-active;
        callback edited(string);
        MatchedName {
            x: 10px; y: 10px; width: 570px; height: 30px;
            prefix: "_"; matched: "escu"; suffix: "ltura_alce_voronoi";
            text-color: white;
        }
        NewFolder {
            width: 100%; height: 100%;
            vm: {active: root.folder-active, text: "", dir: "C:\\sample", can-create: false};
            set-text(value) => { root.edited(value); }
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
fn matched_name_is_contiguous_and_folder_editor_resets_between_openings() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = NavigationProbe::new().unwrap();
    ui.global::<Theme>()
        .set_accent(slint::Color::from_rgb_u8(255, 0, 0));
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(600, 420));
    slint::platform::update_timers_and_animations();
    window.request_redraw();
    let mut pixels = vec![slint::Rgb8Pixel::default(); 600 * 420];
    window.draw_if_needed(|r| {
        r.render(&mut pixels, 600);
    });
    let red_x: Vec<_> = pixels
        .iter()
        .take(600 * 40)
        .enumerate()
        .filter(|(_, p)| p.r > 100 && p.g < 30 && p.b < 30)
        .map(|(i, _)| i % 600)
        .collect();
    assert!(!red_x.is_empty());
    assert!(
        *red_x.iter().min().unwrap() < 30,
        "sin espacio distribuido antes del match"
    );
    assert!(
        *red_x.iter().max().unwrap() < 90,
        "match usa su ancho natural"
    );
    let white_right = pixels
        .iter()
        .take(600 * 40)
        .enumerate()
        .filter(|(_, p)| p.r > 100 && p.g > 100 && p.b > 100)
        .map(|(i, _)| i % 600)
        .max()
        .unwrap();
    assert!(
        white_right < 270,
        "el sufijo sigue contiguo y no se empuja a la derecha"
    );
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/agent-out");
    std::fs::create_dir_all(&out).unwrap();
    let bytes: Vec<_> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
    image::save_buffer(
        out.join("nav052-highlight.png"),
        &bytes,
        600,
        420,
        image::ColorType::Rgb8,
    )
    .unwrap();
    let edited = Rc::new(RefCell::new(String::new()));
    ui.on_edited({
        let value = edited.clone();
        move |t| *value.borrow_mut() = t.into()
    });
    for text in ["old", "new"] {
        ui.set_folder_active(true);
        slint::platform::update_timers_and_animations();
        window.request_redraw();
        window.draw_if_needed(|r| {
            r.render(&mut pixels, 600);
        });
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.into() });
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text: text.into() });
        slint::platform::update_timers_and_animations();
        assert_eq!(
            edited.borrow().as_str(),
            text,
            "cada apertura empieza vacía y enfocada"
        );
        ui.set_folder_active(false);
        slint::platform::update_timers_and_animations();
    }
}
