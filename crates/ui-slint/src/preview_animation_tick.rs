// Naygo — reloj exclusivo del preview: no reconstruye listados, árbol ni workspace.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use crate::{models::Models, workspace_ctrl::WorkspaceCtrl, AppWindow};
#[cfg(not(windows))]
use slint::ComponentHandle;
use slint::Model;
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

pub(crate) fn build(
    ctrl: Rc<RefCell<WorkspaceCtrl>>,
    models: Rc<RefCell<Models>>,
    ui: slint::Weak<AppWindow>,
) -> Rc<dyn Fn()> {
    let timer = Rc::new(slint::Timer::default());
    Rc::new(move || {
        if timer.running() || !ctrl.borrow().preview.animation_tick_needed() {
            return;
        }
        let ctrl = ctrl.clone();
        let models = models.clone();
        let ui = ui.clone();
        let weak_timer = Rc::downgrade(&timer);
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(timer) = weak_timer.upgrade() else {
                    return;
                };
                let Some(ui) = ui.upgrade() else {
                    timer.stop();
                    return;
                };
                let Ok(mut c) = ctrl.try_borrow_mut() else {
                    return;
                };
                let now = Instant::now();
                #[cfg(windows)]
                let visible = crate::win_helpers::naygo_hwnd(&ui)
                    .is_some_and(naygo_platform::window::content_visible);
                #[cfg(not(windows))]
                let visible = ui.window().is_visible();
                if !visible {
                    c.preview.set_visible(false, now);
                }
                if c.preview.poll_animation(now) {
                    let m = models.borrow();
                    for (index, id) in m.pane_ids.iter().enumerate() {
                        if c.ws.pane(*id).is_some_and(|p| {
                            p.purpose == naygo_core::workspace::PanePurpose::Preview
                        }) {
                            if let Some(mut pane) = m.panes.row_data(index) {
                                pane.preview = crate::vm_builders::current_preview_vm(&c);
                                m.panes.set_row_data(index, pane);
                            }
                        }
                    }
                }
                if !c.preview.animation_tick_needed() {
                    timer.stop();
                }
            },
        );
    })
}
