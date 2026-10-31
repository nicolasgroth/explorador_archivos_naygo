// Naygo — interacción y geometría de la ruta superior con renderer software.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Key, Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

slint::slint! {
    import { ToolbarPath } from "../ui/toolbar-path.slint";
    export component ToolbarProbe inherits Window {
        in property <int> probe-width: 420;
        width: root.probe-width * 1px; height: 220px;
        in-out property <string> value;
        in property <string> confirmed;
        in-out property <bool> editing;
        in property <int> session;
        in property <[string]> suggestions;
        in property <string> error;
        out property <bool> focused: bar.input-focused;
        callback start(); callback copy(); callback favorite(); callback cancel();
        callback suggest(string); callback commit(string);
        bar := ToolbarPath {
            x: 6px; y: 2px; width: root.width - 12px; height: 30px;
            editing: root.editing; confirmed: root.confirmed; draft <=> root.value;
            session: root.session; suggestions: root.suggestions; error: root.error;
            start => { root.start(); }
            copy => { root.copy(); } toggle-favorite => { root.favorite(); }
            cancel => { root.cancel(); }
            suggest(value) => { root.suggest(value); }
            commit(value) => { root.commit(value); }
        }
    }
}
struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn pump() {
    for _ in 0..3 {
        slint::platform::update_timers_and_animations();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
fn key(ui: &ToolbarProbe, value: slint::SharedString) {
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: value.clone(),
    });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text: value });
    pump();
}
fn click(ui: &ToolbarProbe, x: f32, y: f32) {
    let position = slint::LogicalPosition::new(x, y);
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    pump();
}
#[test]
fn toolbar_preserves_draft_and_exposes_actions_at_narrow_width() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = ToolbarProbe::new().unwrap();
    let starts = Rc::new(Cell::new(0));
    let copies = Rc::new(Cell::new(0));
    let favorites = Rc::new(Cell::new(0));
    let cancels = Rc::new(Cell::new(0));
    let commits = Rc::new(RefCell::new(Vec::new()));
    let suggestions = Rc::new(RefCell::new(Vec::new()));
    ui.on_start({
        let n = starts.clone();
        move || n.set(n.get() + 1)
    });
    ui.on_copy({
        let n = copies.clone();
        move || n.set(n.get() + 1)
    });
    ui.on_favorite({
        let n = favorites.clone();
        move || n.set(n.get() + 1)
    });
    ui.on_cancel({
        let n = cancels.clone();
        move || n.set(n.get() + 1)
    });
    ui.on_commit({
        let n = commits.clone();
        move |v| n.borrow_mut().push(v.to_string())
    });
    ui.on_suggest({
        let n = suggestions.clone();
        move |v| n.borrow_mut().push(v.to_string())
    });
    let confirmed = format!("C:\\{}", "carpeta\\".repeat(100));
    ui.set_confirmed(confirmed.clone().into());
    ui.set_value(confirmed.clone().into());
    ui.set_probe_width(260);
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(260, 220));
    pump();
    click(&ui, 185., 16.);
    click(&ui, 212., 16.);
    click(&ui, 240., 16.);
    assert_eq!((starts.get(), copies.get(), favorites.get()), (1, 1, 1));
    ui.set_editing(true);
    ui.set_session(1);
    // Conditional Slint children mount lazily during layout/render. A real event loop
    // renders a frame before the newly mounted editor's zero-delay focus timer runs.
    let mut frame = vec![slint::Rgb8Pixel::default(); 260 * 220];
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut frame, 260);
    });
    pump();
    assert!(ui.get_focused());
    key(&ui, "subcarpeta ñ".into());
    assert_eq!(ui.get_value(), "subcarpeta ñ");
    // Copy/favorite do not commit or replace a draft, including while validation reports error.
    ui.set_error("Carpeta no encontrada".into());
    pump();
    click(&ui, 212., 16.);
    click(&ui, 240., 16.);
    assert_eq!((copies.get(), favorites.get()), (2, 2));
    assert_eq!(ui.get_value(), "subcarpeta ñ");
    assert!(commits.borrow().is_empty());
    assert_eq!(ui.get_confirmed(), confirmed);
    ui.set_error("".into());
    ui.set_suggestions(Rc::new(slint::VecModel::from(vec!["uno".into(), "dos".into()])).into());
    pump();
    key(&ui, Key::DownArrow.into());
    key(&ui, Key::Return.into());
    assert_eq!(*suggestions.borrow(), vec!["uno"]);
    assert!(commits.borrow().is_empty());
    click(&ui, 50., 68.);
    assert_eq!(*suggestions.borrow(), vec!["uno", "dos"]);
    ui.set_suggestions(slint::ModelRc::default());
    pump();
    key(&ui, Key::Return.into());
    assert_eq!(*commits.borrow(), vec!["subcarpeta ñ"]);
    key(&ui, Key::Escape.into());
    assert_eq!(cancels.get(), 1);
    // Keep a reproducible synthetic rendering; never read or capture a user's path/content.
    ui.set_error("Carpeta no encontrada".into());
    pump();
    let mut pixels = vec![slint::Rgb8Pixel::default(); 260 * 220];
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, 260);
    });
    let bytes: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/agent-out/rp2-toolbar-260.png");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::save_buffer(path, &bytes, 260, 220, image::ColorType::Rgb8).unwrap();
    ui.hide().unwrap();
}
