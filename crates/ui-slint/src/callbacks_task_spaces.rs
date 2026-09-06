// Naygo — comandos del gestor de espacios por tarea.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use crate::{wire::WireCtx, AppWindow};
use slint::ComponentHandle;
use std::path::PathBuf;

pub(crate) fn wire_task_spaces(ui: &AppWindow, ctx: &WireCtx) {
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_layout.clone();
    let timer = ctx.start_timer.clone();
    let weak = ui.as_weak();
    ui.on_spaces_action(move |action, root| {
        let root = (!root.trim().is_empty()).then(|| PathBuf::from(root.trim()));
        match action {
            0 => {
                ctrl.borrow_mut().spaces_open();
            }
            1 | 2 => {
                if ctrl.borrow().task_spaces.busy() {
                    return;
                }
                // Sin RefCell prestado durante el diálogo nativo (puede bombear eventos).
                let directory = ctrl.borrow().config.config_dir.clone();
                let title = ctrl.borrow().config.t(if action == 1 {
                    "spaces.save_as"
                } else {
                    "spaces.open"
                });
                let dialog = rfd::FileDialog::new()
                    .set_title(title)
                    .set_directory(directory)
                    .add_filter("Naygo", &["naygospace"]);
                let path = if action == 1 {
                    dialog.save_file()
                } else {
                    dialog.pick_file()
                };
                if let Some(mut path) = path {
                    if action == 1 {
                        if !path
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("naygospace"))
                        {
                            let mut name = path.into_os_string();
                            name.push(".naygospace");
                            path = name.into();
                        }
                        ctrl.borrow_mut().spaces_save(Some(path), root, false);
                    } else {
                        ctrl.borrow_mut().spaces_read(path, root);
                    }
                }
            }
            3 => ctrl.borrow_mut().spaces_save(None, None, false),
            4 => ctrl.borrow_mut().spaces_apply_pending(),
            5 => ctrl.borrow_mut().spaces_save(None, None, true),
            7 => ctrl.borrow_mut().task_spaces.recents.clear(),
            10..=19 => ctrl
                .borrow_mut()
                .spaces_read_recent((action - 10) as usize, root),
            _ => ctrl.borrow_mut().spaces_close(),
        }
        if action == 0 {
            if let Some(ui) = weak.upgrade() {
                ui.set_spaces_root("".into());
            }
        }
        timer();
        sync();
    });
}
