// Naygo — pruebas del editor real con teclado y render software, sin ventanas nativas.
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
    import { PathEditor } from "../ui/path-editor.slint";
    export component PathProbe inherits Window {
        in property <int> probe-width: 420;
        width: root.probe-width * 1px; height: 80px; background: #0e1622;
        in-out property <string> value;
        in property <int> session;
        in property <int> caret: -1;
        in property <int> caret-revision;
        in property <int> suggestion-index: -1;
        in property <string> error;
        out property <bool> editor-focused: editor.has-focus;
        callback commit(string);
        callback cancel();
        callback accept-suggestion();
        editor := PathEditor {
            x: 10px; y: 10px; width: parent.width - 20px; height: 30px;
            text <=> root.value;
            session: root.session;
            caret: root.caret;
            caret-revision: root.caret-revision;
            suggestion-index: root.suggestion-index;
            error: root.error;
            commit(value) => { root.commit(value); }
            cancel() => { root.cancel(); }
            accept-suggestion() => { root.accept-suggestion(); }
        }
    }
}

struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

fn key(window: &slint::Window, text: slint::SharedString) {
    window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    window.dispatch_event(WindowEvent::KeyReleased { text });
}

// A native event loop runs change handlers and then the newly armed zero-delay timers.
// A single update only schedules the caret timer; it cannot already fire it.
fn pump() {
    for _ in 0..3 {
        slint::platform::update_timers_and_animations();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn native_caret_selection_undo_and_session_survive_layout_and_errors() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = PathProbe::new().unwrap();
    ui.set_value("C:\\abc".into());
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(420, 80));
    pump();
    assert!(ui.get_editor_focused());
    key(ui.window(), Key::End.into());
    key(ui.window(), Key::LeftArrow.into());
    key(ui.window(), "\\".into());
    assert_eq!(
        ui.get_value(),
        "C:\\ab\\c",
        "separator must insert at caret"
    );
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    key(ui.window(), "z".into());
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    assert_eq!(
        ui.get_value(),
        "C:\\abc",
        "native undo must undo the insertion"
    );

    ui.set_caret(3);
    ui.set_caret_revision(1);
    pump();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    key(ui.window(), Key::RightArrow.into());
    key(ui.window(), Key::RightArrow.into());
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });
    key(ui.window(), "ñ".into());
    assert_eq!(
        ui.get_value(),
        "C:\\ñc",
        "typing replaces only selected text"
    );

    ui.set_error("Carpeta no disponible".into());
    ui.set_probe_width(260);
    window.set_size(slint::PhysicalSize::new(260, 80));
    pump();
    let mut pixels = vec![slint::Rgb8Pixel::default(); 260 * 80];
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, 260);
    }));
    assert!(
        (15..35).any(|y| (230..243).any(|x| {
            let p = pixels[y * 260 + x];
            p.r > 120 && p.g > 120 && p.b > 120
        })),
        "cancel action must remain visible at a narrow width"
    );
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/agent-out");
    std::fs::create_dir_all(&output).unwrap();
    image::save_buffer(
        output.join("rp1-editor-260.png"),
        &pixels
            .iter()
            .flat_map(|p| [p.r, p.g, p.b])
            .collect::<Vec<_>>(),
        260,
        80,
        image::ColorType::Rgb8,
    )
    .unwrap();
    key(ui.window(), "x".into());
    assert_eq!(
        ui.get_value(),
        "C:\\ñxc",
        "error/resize must preserve caret and draft"
    );
    let committed = Rc::new(Cell::new(0));
    let cancelled = Rc::new(Cell::new(0));
    let selected = Rc::new(Cell::new(0));
    ui.on_commit({
        let count = committed.clone();
        move |_| count.set(count.get() + 1)
    });
    ui.on_cancel({
        let count = cancelled.clone();
        move || count.set(count.get() + 1)
    });
    ui.on_accept_suggestion({
        let count = selected.clone();
        move || count.set(count.get() + 1)
    });
    key(ui.window(), Key::Return.into());
    assert_eq!(committed.get(), 1);
    ui.set_suggestion_index(0);
    key(ui.window(), Key::Return.into());
    assert_eq!(selected.get(), 1);
    assert_eq!(
        committed.get(),
        1,
        "selected suggestion must not commit stale draft"
    );
    key(ui.window(), Key::Escape.into());
    assert_eq!(cancelled.get(), 1);

    ui.set_session(1);
    pump();
    key(ui.window(), "D:\\next".into());
    assert_eq!(
        ui.get_value(),
        "D:\\next",
        "a new session selects the whole path"
    );
    let long_path = format!("C:\\{}", "carpeta\\".repeat(100));
    ui.set_value(long_path.clone().into());
    ui.set_session(2);
    pump();
    key(ui.window(), Key::End.into());
    key(ui.window(), "final".into());
    assert_eq!(ui.get_value(), format!("{long_path}final"));
    ui.set_caret(3);
    ui.set_caret_revision(2);
    pump();
    key(ui.window(), "ñ\\".into());
    assert_eq!(
        ui.get_value(),
        format!("C:\\ñ\\{}final", "carpeta\\".repeat(100))
    );
    ui.hide().unwrap();
}
