// Naygo — regresión del cableado real de edición superior y validación asíncrona.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use crate::workspace_ctrl::WorkspaceCtrl;
use naygo_core::workspace::layout::Rect;
use slint::platform::{
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    Platform, WindowAdapter,
};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

struct Headless;
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
    }
}
fn wait_validation(ui: &AppWindow) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while ui.get_edit_validating() && Instant::now() < deadline {
        slint::platform::update_timers_and_animations();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !ui.get_edit_validating(),
        "local validation must finish without blocking UI"
    );
}

#[test]
fn toolbar_callbacks_keep_captured_pane_and_discard_cancelled_or_closed_destinations() {
    slint::platform::set_platform(Box::new(Headless)).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().join("base");
    let child = base.join("child ñ");
    let other = tmp.path().join("other");
    std::fs::create_dir_all(&child).unwrap();
    std::fs::create_dir_all(other.join("child ñ")).unwrap();
    let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new_in(
        base.clone(),
        tmp.path().join("settings"),
    )));
    let first = ctrl.borrow().active_id().unwrap();
    ctrl.borrow_mut().add_pane_split(Rect {
        x: 0.,
        y: 0.,
        w: 1000.,
        h: 700.,
    });
    let second = ctrl.borrow().active_id().unwrap();
    assert_ne!(first, second);
    assert!(ctrl.borrow_mut().navigate_pane_to(second, other.clone()));
    ctrl.borrow_mut().set_active(first);
    let ui = AppWindow::new().unwrap();
    let ctx = WireCtx {
        ctrl: ctrl.clone(),
        sync_rows: Rc::new(|| {}),
        sync_layout: Rc::new(|| {}),
        start_timer: Rc::new(|| {}),
        area_of: Rc::new(|| Rect {
            x: 0.,
            y: 0.,
            w: 1000.,
            h: 700.,
        }),
        palette_cmds: Default::default(),
        palette_cmd_indices: Default::default(),
        pending_eject: Default::default(),
        pending_eject_panes: Default::default(),
        pending_undo: Default::default(),
    };
    wire_pathbar(&ui, &ctx);
    ui.invoke_toolbar_path_start();
    assert!(ui.get_edit_toolbar());
    assert_eq!(ui.get_edit_pane(), first.0 as i32);
    let confirmed = ui.get_edit_base();
    ui.set_edit_text("child ñ".into());
    ui.invoke_path_edit_changed(first.0 as i32, ui.get_edit_text());
    ctrl.borrow_mut().set_active(second);
    assert_eq!(ui.get_edit_base(), confirmed);
    // Favorite operates on the captured confirmed directory, never the draft or new active pane.
    ui.invoke_fav_toggle(ui.get_edit_pane());
    assert!(ctrl.borrow().is_pane_dir_favorite(first));
    assert!(!ctrl.borrow().is_pane_dir_favorite(second));
    assert_eq!(ui.get_edit_text(), "child ñ");
    ui.invoke_path_edit_commit(ui.get_edit_pane(), ui.get_edit_text());
    wait_validation(&ui);
    assert_eq!(
        std::path::PathBuf::from(ctrl.borrow().path_of(first)),
        child
    );
    assert_eq!(
        std::path::PathBuf::from(ctrl.borrow().path_of(second)),
        other
    );
    assert_eq!(ui.get_edit_pane(), -1);
    assert!(!ui.get_edit_toolbar());
    assert_eq!(ui.get_path_refocus_revision(), 1);

    ctrl.borrow_mut().set_active(second);
    ui.invoke_toolbar_path_start();
    ui.set_edit_text("missing".into());
    ui.invoke_path_edit_commit(second.0 as i32, ui.get_edit_text());
    wait_validation(&ui);
    assert!(!ui.get_edit_error().is_empty());
    assert_eq!(ui.get_edit_text(), "missing");
    assert!(ui.get_edit_toolbar());
    ui.invoke_path_edit_commit(second.0 as i32, "child ñ".into());
    ui.invoke_path_edit_cancel(second.0 as i32);
    for _ in 0..10 {
        slint::platform::update_timers_and_animations();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        std::path::PathBuf::from(ctrl.borrow().path_of(second)),
        other
    );
    assert_eq!(ui.get_edit_pane(), -1);

    ui.invoke_toolbar_path_start();
    ctrl.borrow_mut().close_pane(second);
    ui.invoke_path_edit_commit(second.0 as i32, "child ñ".into());
    assert_eq!(ui.get_edit_pane(), -1);
    assert!(!ui.get_edit_toolbar());
    assert_eq!(
        std::path::PathBuf::from(ctrl.borrow().path_of(first)),
        child
    );
    // Starting the panel editor after the toolbar shares state but not two rendered inputs.
    ui.invoke_toolbar_path_start();
    ui.invoke_path_edit_start(first.0 as i32);
    assert!(!ui.get_edit_toolbar());
    assert_eq!(ui.get_edit_pane(), first.0 as i32);
    ui.invoke_path_edit_cancel(first.0 as i32);
}
