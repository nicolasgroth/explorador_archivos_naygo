// Naygo — continuidad del editor F2 ante diálogos de operación.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Key, Platform, PointerEventButton, WindowAdapter, WindowEvent,
    },
    ComponentHandle, LogicalPosition, ModelRc, VecModel,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
slint::slint! {
    import { FilePanel } from "../ui/file-panel.slint";
    import { OpDialogs } from "../ui/op-dialogs.slint";
    import { CommandPalette } from "../ui/command-palette.slint";
    import { RowData } from "../ui/types.slint";
    export { RowData }
    export component Probe inherits Window {
        width: 800px; height: 600px;
        in property <bool> modal;
        in property <bool> editing: true;
        in-out property <bool> seeded;
        changed editing => { if root.editing { root.seeded = false; } }
        in property <bool> pane-active: true;
        in property <bool> palette-open;
        in-out property <string> draft: "file.txt";
        in property <[RowData]> rows;
        callback conflict-skip();
        callback palette-close();
        callback committed(int, string);
        callback cancelled();
        callback chained(int, string, int);
        FilePanel {
            width: 100%; height: 100%; active: root.pane-active;
            rows: root.rows;
            columns: [{kind: 0, label: "Name", width: 300px}];
            rename-pos: root.editing ? 0 : -1; rename-session: 42; rename-draft <=> root.draft;
            rename-seeded <=> root.seeded;
            rename-selection-start: 0; rename-selection-end: 4;
            rename-cursor: 1;
            modal-open: root.modal;
            focus-blocked: root.palette-open;
            rename-commit(session, pos, name) => { root.committed(session, name); }
            rename-cancel => { root.cancelled(); }
            rename-chain(session, pos, name, dir) => { root.chained(session, name, dir); }
        }
        OpDialogs { width: 100%; height: 100%; vm: { kind: root.modal ? 2 : 0 }; conflict-decide(a, all) => { root.conflict-skip(); } }
        if root.palette-open: CommandPalette { width: 100%; height: 100%; dismiss => { root.palette-close(); } }
    }
}
struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}
fn pump(w: &MinimalSoftwareWindow) {
    for _ in 0..4 {
        slint::platform::update_timers_and_animations();
        w.draw_if_needed(|r| {
            let mut p = vec![slint::Rgb8Pixel::default(); 800 * 600];
            r.render(&mut p, 800);
        });
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[test]
fn conflict_does_not_commit_rename_and_return_preserves_draft() {
    let w = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(w.clone()))).unwrap();
    let ui = Probe::new().unwrap();
    let row = RowData {
        name: "file.txt".into(),
        focused: true,
        selected: true,
        ..Default::default()
    };
    let rows = Rc::new(VecModel::from(vec![row.clone()]));
    ui.set_rows(ModelRc::from(rows.clone()));
    let skips = Rc::new(Cell::new(0));
    ui.on_conflict_skip({
        let skips = skips.clone();
        move || skips.set(skips.get() + 1)
    });
    let commits = Rc::new(RefCell::new(Vec::new()));
    let active = Rc::new(Cell::new(true));
    ui.on_committed({
        let commits = commits.clone();
        let active = active.clone();
        let weak = ui.as_weak();
        move |session, name| {
            // El controlador consume la sesión antes de destruir el editor.
            if session == 42 && active.replace(false) {
                commits.borrow_mut().push(name.to_string());
                weak.unwrap().set_editing(false);
            }
        }
    });
    ui.show().unwrap();
    w.set_size(slint::PhysicalSize::new(800, 600));
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::LeftArrow.into(),
    });
    pump(&w);
    assert!(commits.borrow().is_empty());
    ui.set_modal(true);
    pump(&w);
    assert!(
        commits.borrow().is_empty(),
        "el conflicto no debe confirmar F2"
    );
    ui.set_modal(false);
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "y".into() });
    pump(&w);
    assert_eq!(
        ui.get_draft().as_str(),
        "yx.txt",
        "el cursor se conserva si el editor no se reconstruye"
    );
    ui.set_modal(true);
    pump(&w);
    rows.set_vec(vec![row]);
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&w);
    assert_eq!(
        skips.get(),
        1,
        "la reconstrucción no debe robar el foco al conflicto"
    );
    assert!(commits.borrow().is_empty());
    ui.set_modal(false);
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "z".into() });
    pump(&w);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(
        &*commits.borrow(),
        &["yzx.txt".to_string()],
        "Enter confirma el mismo borrador una sola vez"
    );

    // Un clic intencional fuera del editor conserva el contrato anterior de confirmar.
    active.set(true);
    ui.set_draft("file.txt".into());
    ui.set_editing(true);
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "z".into() });
    pump(&w);
    let position = LogicalPosition::new(600.0, 400.0);
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    pump(&w);
    assert_eq!(
        &*commits.borrow(),
        &["yzx.txt".to_string(), "z.txt".to_string()],
        "clic fuera confirma una vez"
    );

    // Escape cancela antes de desmontar; la pérdida de foco no puede confirmar después.
    let cancellations = Rc::new(Cell::new(0));
    ui.on_cancelled({
        let n = cancellations.clone();
        let active = active.clone();
        let weak = ui.as_weak();
        move || {
            active.set(false);
            n.set(n.get() + 1);
            weak.unwrap().set_editing(false);
        }
    });
    active.set(true);
    ui.set_draft("file.txt".into());
    ui.set_editing(true);
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "q".into() });
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&w);
    assert_eq!(cancellations.get(), 1);
    assert_eq!(commits.borrow().len(), 2, "Escape no confirma");

    // La flecha entrega la sesión y el borrador al controlador para el rename encadenado.
    let chained = Rc::new(RefCell::new(Vec::new()));
    ui.on_chained({
        let v = chained.clone();
        let active = active.clone();
        let weak = ui.as_weak();
        move |session, name, direction| {
            active.set(false);
            v.borrow_mut().push((session, name.to_string(), direction));
            weak.unwrap().set_editing(false);
        }
    });
    active.set(true);
    ui.set_draft("file.txt".into());
    ui.set_editing(true);
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "r".into() });
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    pump(&w);
    assert_eq!(&*chained.borrow(), &[(42, "r.txt".to_string(), 1)]);
    assert_eq!(commits.borrow().len(), 2);

    // Un cambio de panel durante el conflicto no confirma; al volver se recupera F2.
    active.set(true);
    ui.set_draft("file.txt".into());
    ui.set_editing(true);
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "z".into() });
    ui.set_modal(true);
    pump(&w);
    ui.set_pane_active(false);
    pump(&w);
    ui.set_modal(false);
    pump(&w);
    assert_eq!(commits.borrow().len(), 2);
    ui.set_pane_active(true);
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(
        &*commits.borrow(),
        &[
            "yzx.txt".to_string(),
            "z.txt".to_string(),
            "zx.txt".to_string()
        ]
    );

    // La paleta usa el bloqueo de foco de overlays, diferente al modal de operaciones.
    ui.on_palette_close({
        let weak = ui.as_weak();
        move || weak.unwrap().set_palette_open(false)
    });
    active.set(true);
    ui.set_draft("file.txt".into());
    ui.set_editing(true);
    pump(&w);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "p".into() });
    ui.set_palette_open(true);
    pump(&w);
    assert_eq!(commits.borrow().len(), 3, "la paleta no confirma F2");
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&w);
    assert!(!ui.get_palette_open());
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "q".into() });
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&w);
    assert_eq!(commits.borrow().last().unwrap(), "pq.txt");
}
