// Naygo — parámetros del editor, diálogos nativos y ejecución explícita de recetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use crate::{wire::WireCtx, AppWindow};
use naygo_core::{
    delivery::Output,
    recipe::{Layout, Parameters, Recipe, Source},
};
use slint::ComponentHandle;
use std::path::PathBuf;

fn input(ui: &AppWindow) -> Option<(Recipe, Parameters)> {
    let query = crate::callbacks_queries::parse_draft(ui.get_recipe_query())?;
    let mut recipe = Recipe::from_query(&query);
    recipe.source = match ui.get_recipe_source() {
        0 => Source::Selection,
        1 => Source::Query,
        _ => return None,
    };
    recipe.output_name = ui.get_recipe_output_name().trim().into();
    recipe.output = if ui.get_recipe_zip() {
        Output::Zip
    } else {
        Output::Folder
    };
    recipe.layout = match ui.get_recipe_layout() {
        0 => Layout::Flat,
        1 => Layout::Grouped,
        2 => Layout::Relative,
        _ => return None,
    };
    recipe.number_duplicates = ui.get_recipe_number_duplicates();
    recipe.hashes = ui.get_recipe_hashes();
    Some((
        recipe,
        Parameters {
            roots: query.roots,
            parent: PathBuf::from(ui.get_recipe_parent().trim()),
        },
    ))
}
pub(crate) fn wire_recipes(ui: &AppWindow, ctx: &WireCtx) {
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_layout.clone();
    let timer = ctx.start_timer.clone();
    let weak = ui.as_weak();
    ui.on_recipe_action(move |action| {
        if action == 0 {
            ctrl.borrow_mut().recipe_open();
        } else if action == 8 {
            ctrl.borrow_mut().recipe_close();
        } else if !ctrl.borrow().recipes.busy() {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            if matches!(action, 3 | 4 | 6 | 7) {
                let Some((recipe, parameters)) = input(&ui) else {
                    ctrl.borrow_mut().recipe_invalid();
                    sync();
                    return;
                };
                ctrl.borrow_mut().recipe_edit(recipe, parameters);
            }
            match action {
                1 | 3 | 5 => {
                    let key = match action {
                        1 => "recipes.open",
                        3 => "spaces.save_as",
                        _ => "recipes.import_query",
                    };
                    let title = ctrl.borrow().config.t(key);
                    let directory = ctrl.borrow().config.config_dir.clone();
                    let extension = if action == 5 {
                        "naygosearch"
                    } else {
                        "naygorecipe"
                    };
                    let dialog = rfd::FileDialog::new()
                        .set_title(title)
                        .set_directory(directory)
                        .add_filter("Naygo", &[extension]);
                    let path = if action == 3 {
                        dialog.save_file()
                    } else {
                        dialog.pick_file()
                    };
                    if let Some(mut path) = path {
                        if action == 3 {
                            if !path
                                .extension()
                                .is_some_and(|e| e.eq_ignore_ascii_case(extension))
                            {
                                let mut name = path.into_os_string();
                                name.push(".naygorecipe");
                                path = name.into();
                            }
                            ctrl.borrow_mut().recipe_save(Some(path));
                        } else {
                            ctrl.borrow_mut().recipe_read(path, action == 5);
                        }
                    }
                }
                4 => ctrl.borrow_mut().recipe_save(None),
                6 => ctrl.borrow_mut().recipe_prepare(),
                7 => {
                    ctrl.borrow_mut().recipe_execute();
                }
                9 => ctrl.borrow_mut().recipe_forget(),
                10 => {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        ctrl.borrow_mut().recipe_invalidate();
                        ui.set_recipe_parent(path.display().to_string().into());
                    }
                }
                _ => {}
            }
        }
        timer();
        sync();
    });
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_rows.clone();
    ui.on_recipe_invalidate(move || {
        ctrl.borrow_mut().recipe_invalidate();
        sync();
    });
}
