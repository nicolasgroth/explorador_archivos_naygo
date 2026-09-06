// Naygo — callbacks del asistente de entregas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use crate::{wire::WireCtx, AppWindow};
use slint::ComponentHandle;
use std::path::PathBuf;

pub(crate) fn wire_delivery(ui: &AppWindow, ctx: &WireCtx) {
    {
        let ctrl = ctx.ctrl.clone();
        let sync = ctx.sync_layout.clone();
        let weak = ui.as_weak();
        ui.on_delivery_open(move || {
            let opened = ctrl.borrow_mut().delivery_open();
            if opened {
                if let Some(ui) = weak.upgrade() {
                    let path = ctrl
                        .borrow()
                        .active_dir()
                        .unwrap_or_default()
                        .join("delivery");
                    ui.set_delivery_destination(path.to_string_lossy().as_ref().into());
                }
                sync();
            }
        });
    }
    {
        let weak = ui.as_weak();
        let ctrl = ctx.ctrl.clone();
        ui.on_delivery_browse(move || {
            if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                ctrl.borrow_mut().delivery_invalidate();
                if let Some(ui) = weak.upgrade() {
                    ui.set_delivery_destination(
                        parent.join("delivery").to_string_lossy().as_ref().into(),
                    );
                }
            }
        });
    }
    {
        let ctrl = ctx.ctrl.clone();
        let sync = ctx.sync_layout.clone();
        let timer = ctx.start_timer.clone();
        ui.on_delivery_prepare(
            move |destination, mode, root, zip, hashes, groups, rename| {
                let mut destination = PathBuf::from(destination.trim());
                if zip
                    && !destination
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
                {
                    let mut name = destination.into_os_string();
                    name.push(".zip");
                    destination = PathBuf::from(name);
                }
                ctrl.borrow_mut().delivery_prepare(
                    destination,
                    mode,
                    PathBuf::from(root.trim()),
                    zip,
                    hashes,
                    naygo_core::delivery::ReviewOptions {
                        group_names: if groups.is_empty() {
                            vec![String::new()]
                        } else {
                            groups.lines().map(str::to_owned).collect()
                        },
                        rename_flat_duplicates: rename,
                    },
                );
                timer();
                sync();
            },
        );
    }
    {
        let ctrl = ctx.ctrl.clone();
        let sync = ctx.sync_layout.clone();
        let timer = ctx.start_timer.clone();
        ui.on_delivery_result_action(move |action| {
            match action {
                0 => {
                    let path = ctrl.borrow().delivery_result_location(true);
                    if let Some(path) = path {
                        let area = ctrl.borrow().last_area;
                        ctrl.borrow_mut().open_dir_in_new_pane(path, area);
                    }
                }
                1 => {
                    let path = ctrl.borrow().delivery_result_location(false);
                    if let Some(path) = path {
                        if let Err(error) =
                            naygo_platform::clipboard::write_text(&path.to_string_lossy())
                        {
                            ctrl.borrow_mut().pending_shell_error = Some(format!("{error:?}"));
                        }
                    }
                }
                2 => {
                    ctrl.borrow_mut().delivery_return_to_selection();
                }
                _ => {
                    ctrl.borrow_mut().delivery_results.latest = None;
                }
            }
            timer();
            sync();
        });
    }
    {
        let ctrl = ctx.ctrl.clone();
        let sync = ctx.sync_layout.clone();
        ui.on_delivery_invalidate(move || {
            ctrl.borrow_mut().delivery_invalidate();
            sync();
        });
    }
    {
        let ctrl = ctx.ctrl.clone();
        let sync = ctx.sync_layout.clone();
        let timer = ctx.start_timer.clone();
        ui.on_delivery_publish(move || {
            ctrl.borrow_mut().delivery_execute();
            timer();
            sync();
        });
    }
    {
        let ctrl = ctx.ctrl.clone();
        let sync = ctx.sync_layout.clone();
        ui.on_delivery_dismiss(move || {
            ctrl.borrow_mut().delivery_close();
            sync();
        });
    }
}
