// Naygo — foco de teclado del panel tras el diálogo de nuevas carpetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use slint::{
    platform::{
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        Key, Platform, WindowAdapter, WindowEvent,
    },
    ComponentHandle,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

slint::slint! {
    import { FilePanel } from "../ui/file-panel.slint";
    import { DropViewport } from "../ui/types.slint";
    export { DropViewport }
    import { NewFolder } from "../ui/new-folder.slint";
    import { TreePanel } from "../ui/tree-panel.slint";
    import { OpDialogs } from "../ui/op-dialogs.slint";
    import { MessageModal } from "../ui/message-modal.slint";
    import { RetryDialog } from "../ui/retry-dialog.slint";
    import { CommandPalette } from "../ui/command-palette.slint";
    import { DestinationRadar } from "../ui/destination-radar.slint";
    import { TaskSpacesDialog } from "../ui/task-spaces-dialog.slint";
    import { SavedQueryDialog } from "../ui/saved-query-dialog.slint";
    import { RecipeDialog } from "../ui/recipe-dialog.slint";
    import { ComparisonPointsDialog } from "../ui/comparison-points-dialog.slint";
    import { DeliveryDialog } from "../ui/delivery-dialog.slint";
    import { OpFileListModal } from "../ui/op-file-list-modal.slint";
    import { ToolbarPath } from "../ui/toolbar-path.slint";
    import { ListingExportDialog } from "../ui/listing-export-dialog.slint";
    export component FolderFocusProbe inherits Window {
        width: 800px; height: 600px;
        in property <length> panel-height: 600px;
        in property <bool> folder-open;
        in property <bool> tree-mode;
        in property <bool> deep-mode;
        in property <bool> export-open;
        callback export-cancel();
        in property <int> op-kind;
        in property <int> message-kind;
        in property <bool> retry-open;
        in property <bool> palette-open;
        in property <bool> path-editing;
        in property <bool> toolbar-editing;
        callback path-edited(string);
        callback toolbar-edited(string);
        in property <int> extra-kind;
        callback extra-close();
        callback retry-close();
        callback palette-close();
        callback delete-confirm();
        callback delete-cancel();
        callback message-cancel();
        callback panel-key(string);
        callback viewport-changed(DropViewport);
        callback tree-key(string);
        callback tree-shortcut(string, bool) -> bool;
        FilePanel {
            width: 100%; height: root.panel-height; active: !root.tree-mode; new-folder-open: root.folder-open;
            deep-active: root.deep-mode;
            modal-open: root.op-kind > 0 || root.message-kind > 0;
            focus-blocked: root.retry-open || root.palette-open || root.extra-kind > 0 || root.export-open;
            editing: root.path-editing;
            toolbar-path-editing: root.toolbar-editing;
            path-edit-changed(id, value) => { root.path-edited(value); }
            visible: !root.tree-mode;
            key(text, c, s, a) => { root.panel-key(text); }
            drop-viewport-changed(id, viewport) => { root.viewport-changed(viewport); }
        }
        TreePanel {
            width: 100%; height: 100%; active: root.tree-mode; visible: root.tree-mode;
            new-folder-open: root.folder-open;
            modal-open: root.op-kind > 0 || root.message-kind > 0;
            focus-blocked: root.retry-open || root.palette-open || root.extra-kind > 0;
            tree-key(key) => { root.tree-key(key); }
            shortcut(text, ctrl, shift, alt) => { root.tree-shortcut(text, ctrl) }
        }
        if root.toolbar-editing: ToolbarPath {
            x: 0; y: 0; width: 600px; height: 35px; editing: true;
            focus-blocked: root.retry-open || root.palette-open || root.extra-kind > 0
                || root.folder-open || root.op-kind > 0 || root.message-kind > 0;
            edited(value) => { root.toolbar-edited(value); }
        }
        NewFolder {
            width: 100%; height: 100%;
            vm: { active: root.folder-open, text: "", dir: "C:\\fixture", can-create: true };
        }
        OpDialogs {
            width: 100%; height: 100%; vm: { kind: root.op-kind, del-count: 1 };
            delete-confirm => { root.delete-confirm(); }
            delete-cancel => { root.delete-cancel(); }
        }
        MessageModal {
            width: 100%; height: 100%; vm: { kind: root.message-kind };
            message-cancel => { root.message-cancel(); }
        }
        ListingExportDialog {
            width: 100%; height: 100%; active: root.export-open;
            cancel => { root.export-cancel(); }
        }
        if root.retry-open: RetryDialog {
            width: 100%; height: 100%;
            action(v) => { if v == 0 { root.retry-close(); } }
        }
        if root.palette-open: CommandPalette {
            width: 100%; height: 100%; dismiss => { root.palette-close(); }
        }
        if root.extra-kind == 1: DestinationRadar { width: 100%; height: 100%; dismiss => { root.extra-close(); } }
        if root.extra-kind == 2: TaskSpacesDialog { width: 100%; height: 100%; action(a, s) => { if a == 6 { root.extra-close(); } } }
        if root.extra-kind == 3: SavedQueryDialog { width: 100%; height: 100%; action(a, d) => { if a == 5 { root.extra-close(); } } }
        if root.extra-kind == 4: RecipeDialog { width: 100%; height: 100%; action(a) => { if a == 8 { root.extra-close(); } } }
        if root.extra-kind == 5: ComparisonPointsDialog { width: 100%; height: 100%; action(a) => { if a == 8 { root.extra-close(); } } }
        if root.extra-kind == 6: DeliveryDialog { width: 100%; height: 100%; dismiss => { root.extra-close(); } }
        OpFileListModal { width: 100%; height: 100%; open: root.extra-kind == 7; focus-suspended: root.retry-open; close => { root.extra-close(); } }
    }
}

#[test]
fn file_panel_publica_geometria_de_soltar_sin_hover() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = FolderFocusProbe::new().unwrap();
    let viewport = Rc::new(RefCell::new(DropViewport::default()));
    ui.on_viewport_changed({
        let viewport = viewport.clone();
        move |updated| *viewport.borrow_mut() = updated
    });
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(800, 600));
    pump(&window);
    let initial = viewport.borrow().clone();
    assert!(
        initial.width > 0.0 && initial.height > 0.0 && initial.row_height > 0.0,
        "la lista debe informar su geometría inicial: {initial:?}"
    );
    ui.set_panel_height(500.0);
    pump(&window);
    assert!(viewport.borrow().height < initial.height);
}

struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

fn pump(window: &MinimalSoftwareWindow) {
    for _ in 0..3 {
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|renderer| {
            let mut pixels = vec![slint::Rgb8Pixel::default(); 800 * 600];
            renderer.render(&mut pixels, 800);
        });
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn closing_folder_modal_restores_keyboard_to_active_file_panel() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let ui = FolderFocusProbe::new().unwrap();
    ui.show().unwrap();
    window.set_size(slint::PhysicalSize::new(800, 600));
    let received = Rc::new(Cell::new(0));
    ui.on_panel_key({
        let received = received.clone();
        move |_| received.set(received.get() + 1)
    });
    pump(&window);
    ui.set_folder_open(true);
    pump(&window);
    ui.set_folder_open(false);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        1,
        "el panel activo debe recibir las flechas sin clic"
    );

    let tree_keys = Rc::new(Cell::new(0));
    ui.on_tree_key({
        let tree_keys = tree_keys.clone();
        move |_| tree_keys.set(tree_keys.get() + 1)
    });
    let shortcut = Rc::new(Cell::new(false));
    ui.on_tree_shortcut({
        let shortcut = shortcut.clone();
        move |key, ctrl| {
            let recognized = key == "n" && ctrl;
            shortcut.set(recognized);
            recognized
        }
    });
    ui.set_tree_mode(true);
    pump(&window);
    ui.set_folder_open(true);
    pump(&window);
    ui.set_folder_open(false);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        tree_keys.get(),
        1,
        "el árbol recupera las flechas tras el modal"
    );
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "n".into() });
    ui.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    assert!(
        shortcut.get(),
        "el atajo de creación llega al keymap desde el árbol"
    );

    ui.set_tree_mode(false);
    pump(&window);
    ui.on_delete_confirm({
        let weak = ui.as_weak();
        move || weak.unwrap().set_op_kind(0)
    });
    ui.set_op_kind(1);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&window);
    assert_eq!(ui.get_op_kind(), 0, "Enter confirma y cierra el diálogo");
    let before = received.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        before + 1,
        "el panel recupera teclado al confirmar borrado"
    );
    ui.on_export_cancel({
        let weak = ui.as_weak();
        move || weak.unwrap().set_export_open(false)
    });
    for deep in [false, true] {
        ui.set_deep_mode(deep);
        ui.set_export_open(true);
        pump(&window);
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        pump(&window);
        assert!(!ui.get_export_open(), "Esc cancela la exportación");
        let before_export = received.get();
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::DownArrow.into(),
        });
        assert_eq!(
            received.get(),
            before_export + 1,
            "el panel recupera el teclado, deep={deep}"
        );
    }
    ui.set_op_kind(1);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Return.into(),
    });
    pump(&window);
    assert_eq!(ui.get_op_kind(), 0);
    let before_deep = received.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        before_deep + 1,
        "la vista profunda recupera teclado tras confirmar"
    );
    ui.set_deep_mode(false);
    ui.on_delete_cancel({
        let weak = ui.as_weak();
        move || weak.unwrap().set_op_kind(0)
    });
    ui.set_op_kind(1);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    let before_cancel = received.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        before_cancel + 1,
        "cancelar también devuelve el foco"
    );

    ui.set_tree_mode(true);
    ui.set_op_kind(1);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    let before_tree = tree_keys.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        tree_keys.get(),
        before_tree + 1,
        "el árbol activo también recupera el teclado"
    );

    ui.set_tree_mode(false);
    ui.set_op_kind(1);
    pump(&window);
    ui.set_folder_open(true);
    ui.set_op_kind(0);
    pump(&window);
    let before_other_modal = received.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        before_other_modal,
        "no robar foco a otro diálogo"
    );
    ui.set_folder_open(false);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        before_other_modal + 1,
        "al cerrar el último diálogo vuelve el foco"
    );
    ui.on_message_cancel({
        let weak = ui.as_weak();
        move || weak.unwrap().set_message_kind(0)
    });
    ui.set_op_kind(1);
    pump(&window);
    ui.set_message_kind(1);
    ui.set_op_kind(0);
    pump(&window);
    let before_message = received.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        before_message,
        "el aviso conserva el foco al cerrar la operación"
    );
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    assert_eq!(ui.get_message_kind(), 0, "Escape llega al aviso");
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        before_message + 1,
        "el panel recupera el foco al cerrar el aviso"
    );
    ui.on_retry_close({
        let weak = ui.as_weak();
        move || weak.unwrap().set_retry_open(false)
    });
    ui.set_retry_open(true);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    assert!(!ui.get_retry_open(), "Escape cierra reintento");
    let before_retry = received.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        before_retry + 1,
        "el teclado vuelve tras reintento"
    );

    ui.on_palette_close({
        let weak = ui.as_weak();
        move || weak.unwrap().set_palette_open(false)
    });
    ui.set_palette_open(true);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    assert!(!ui.get_palette_open(), "Escape cierra la paleta");
    let before_palette = received.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        received.get(),
        before_palette + 1,
        "el teclado vuelve tras la paleta"
    );

    ui.set_retry_open(true);
    pump(&window);
    ui.set_tree_mode(true);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    assert!(
        !ui.get_retry_open(),
        "un cambio de panel no roba el foco al diálogo"
    );
    let before_retry_tree = tree_keys.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        tree_keys.get(),
        before_retry_tree + 1,
        "tras cerrar vuelve al panel ahora activo"
    );
    ui.on_extra_close({
        let weak = ui.as_weak();
        move || weak.unwrap().set_extra_kind(0)
    });
    for kind in 1..=7 {
        ui.set_extra_kind(kind);
        pump(&window);
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        pump(&window);
        assert_eq!(ui.get_extra_kind(), 0, "Escape cierra diálogo {kind}");
        let before = tree_keys.get();
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::DownArrow.into(),
        });
        assert_eq!(
            tree_keys.get(),
            before + 1,
            "el árbol recibe teclado tras diálogo {kind}"
        );
    }

    ui.set_extra_kind(7);
    pump(&window);
    ui.set_retry_open(true);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    assert!(!ui.get_retry_open());
    assert_eq!(
        ui.get_extra_kind(),
        7,
        "el detalle permanece abierto al cerrar reintento"
    );
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    assert_eq!(
        ui.get_extra_kind(),
        0,
        "el detalle recupera Escape al cerrar reintento"
    );

    ui.set_op_kind(1);
    pump(&window);
    ui.set_retry_open(true);
    pump(&window);
    ui.set_op_kind(0);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    assert!(
        !ui.get_retry_open(),
        "cerrar una operación no quita el foco al reintento abierto"
    );
    let before = tree_keys.get();
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::DownArrow.into(),
    });
    assert_eq!(
        tree_keys.get(),
        before + 1,
        "cerrar el último diálogo restituye el teclado"
    );

    ui.set_tree_mode(false);
    ui.set_path_editing(true);
    let path_changes = Rc::new(RefCell::new(String::new()));
    ui.on_path_edited({
        let path_changes = path_changes.clone();
        move |value| *path_changes.borrow_mut() = value.to_string()
    });
    pump(&window);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "a".into() });
    pump(&window);
    assert!(
        path_changes.borrow().as_str() == "a",
        "el editor de ruta recibe texto inicialmente"
    );
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "c".into() });
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::LeftArrow.into(),
    });
    ui.set_palette_open(true);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "b".into() });
    pump(&window);
    assert!(
        path_changes.borrow().as_str() == "abc",
        "el editor de ruta recupera foco, texto y posición del cursor"
    );

    ui.set_path_editing(false);
    ui.set_toolbar_editing(true);
    let toolbar_changes = Rc::new(RefCell::new(String::new()));
    ui.on_toolbar_edited({
        let toolbar_changes = toolbar_changes.clone();
        move |value| *toolbar_changes.borrow_mut() = value.to_string()
    });
    pump(&window);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
    pump(&window);
    assert_eq!(
        toolbar_changes.borrow().as_str(),
        "x",
        "el editor superior recibe texto"
    );
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "z".into() });
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::LeftArrow.into(),
    });
    ui.set_palette_open(true);
    pump(&window);
    ui.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Escape.into(),
    });
    pump(&window);
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "y".into() });
    pump(&window);
    assert_eq!(
        toolbar_changes.borrow().as_str(),
        "xyz",
        "el editor superior recupera foco, texto y posición del cursor"
    );
}
