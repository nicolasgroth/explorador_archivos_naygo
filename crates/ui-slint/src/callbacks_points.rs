// Naygo — diálogos nativos y callbacks de puntos de comparación.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use crate::{wire::WireCtx, AppWindow};
use naygo_core::comparison_point::Scope;
use slint::ComponentHandle;
use std::path::PathBuf;

pub(crate) fn wire_points(ui: &AppWindow, ctx: &WireCtx) {
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_layout.clone();
    let timer = ctx.start_timer.clone();
    let weak = ui.as_weak();
    ui.on_points_action(move |action| {
        match action {
            0 => {
                ctrl.borrow_mut().points_open();
            }
            1 | 2 | 4 => {
                if ctrl.borrow().comparison_points.busy() {
                    return;
                }
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let scope = Scope {
                    root: PathBuf::from(ui.get_points_root().trim()),
                    recursive: ui.get_points_recursive(),
                    hashed: ui.get_points_hashed(),
                    exclusions: ui
                        .get_points_exclusions()
                        .lines()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                        .collect(),
                };
                let title = ctrl.borrow().config.t(match action {
                    1 => "points.capture",
                    2 => "points.open",
                    _ => "points.export",
                });
                let directory = ctrl.borrow().config.config_dir.clone();
                let dialog = rfd::FileDialog::new()
                    .set_title(title)
                    .set_directory(directory)
                    .add_filter("Naygo", &["naygopoint"]);
                let path = if action == 2 {
                    dialog.pick_file()
                } else {
                    dialog.save_file()
                };
                if let Some(mut path) = path {
                    if action != 2
                        && !path
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("naygopoint"))
                    {
                        let mut name = path.into_os_string();
                        name.push(".naygopoint");
                        path = name.into();
                    }
                    match action {
                        1 => ctrl.borrow_mut().points_capture(path, scope),
                        2 => ctrl.borrow_mut().points_read(path),
                        _ => ctrl.borrow_mut().points_export(path),
                    }
                }
            }
            3 => ctrl.borrow_mut().points_compare(),
            5 => ctrl.borrow_mut().points_delete(),
            6 => ctrl.borrow_mut().points_to_basket(),
            7 => ctrl.borrow_mut().points_new(),
            _ => ctrl.borrow_mut().points_close(),
        }
        timer();
        sync();
    });
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_rows.clone();
    ui.on_points_select(move |index, selected| {
        ctrl.borrow_mut().points_select(index, selected);
        sync();
    });
}
