// Naygo — puente del editor de consultas y selección de resultados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use crate::{wire::WireCtx, AppWindow, QueryDraftVm};
use naygo_core::{
    saved_search::{RelativeDate, SavedSearch, VERSION},
    search::SearchOptions,
};
use std::path::PathBuf;

pub(crate) fn draft_vm(q: &SavedSearch) -> QueryDraftVm {
    QueryDraftVm {
        name: q.name.clone().into(),
        roots: q
            .roots
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n")
            .into(),
        name_query: q.options.name_query.clone().into(),
        content_query: q.options.content_query.clone().into(),
        min_bytes: q.min_bytes.map_or(String::new(), |n| n.to_string()).into(),
        max_bytes: q.max_bytes.map_or(String::new(), |n| n.to_string()).into(),
        modified: match q.modified {
            RelativeDate::Any => 0,
            RelativeDate::Today => 1,
            RelativeDate::ThisWeek => 2,
            RelativeDate::Last7Days => 3,
            RelativeDate::Last30Days => 4,
            RelativeDate::ThisMonth => 5,
        },
        ignore_case: q.options.ignore_case,
        use_wildcards: q.options.use_wildcards,
        recursive: q.options.recursive,
    }
}
pub(crate) fn parse_draft(d: QueryDraftVm) -> Option<SavedSearch> {
    fn size(s: &str) -> Option<Option<u64>> {
        if s.trim().is_empty() {
            Some(None)
        } else {
            s.trim().parse().ok().map(Some)
        }
    }
    Some(SavedSearch {
        version: VERSION,
        name: d.name.trim().into(),
        roots: d
            .roots
            .lines()
            .filter(|s| !s.trim().is_empty())
            .map(|s| PathBuf::from(s.trim()))
            .collect(),
        options: SearchOptions {
            name_query: d.name_query.to_string(),
            content_query: d.content_query.to_string(),
            ignore_case: d.ignore_case,
            use_wildcards: d.use_wildcards,
            recursive: d.recursive,
        },
        min_bytes: size(&d.min_bytes)?,
        max_bytes: size(&d.max_bytes)?,
        modified: match d.modified {
            0 => RelativeDate::Any,
            1 => RelativeDate::Today,
            2 => RelativeDate::ThisWeek,
            3 => RelativeDate::Last7Days,
            4 => RelativeDate::Last30Days,
            5 => RelativeDate::ThisMonth,
            _ => return None,
        },
    })
}
pub(crate) fn wire_queries(ui: &AppWindow, ctx: &WireCtx) {
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_layout.clone();
    let timer = ctx.start_timer.clone();
    ui.on_query_prepare(
        move |name, root, content, ignore_case, use_wildcards, recursive| {
            let mut c = ctrl.borrow_mut();
            let advanced = c
                .search_job
                .as_ref()
                .is_some_and(|j| j.details.advanced.is_some());
            c.query_open((!advanced).then(|| {
                (
                    SearchOptions {
                        name_query: name.to_string(),
                        content_query: content.to_string(),
                        ignore_case,
                        use_wildcards,
                        recursive,
                    },
                    root.to_string(),
                )
            }));
            drop(c);
            timer();
            sync();
        },
    );
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_layout.clone();
    let timer = ctx.start_timer.clone();
    ui.on_query_action(move |action, draft| {
        if action == 5 {
            ctrl.borrow_mut().query_close();
        } else if !ctrl.borrow().saved_queries.busy() {
            if action == 7 {
                ctrl.borrow_mut().query_forget();
            } else {
                let query = parse_draft(draft);
                if action != 1 && query.as_ref().is_none_or(|q| q.validate().is_err()) {
                    let mut c = ctrl.borrow_mut();
                    c.saved_queries.report = c.config.t("queries.invalid");
                    drop(c);
                    sync();
                    return;
                }
                match action {
                    1 | 2 => {
                        let directory = ctrl.borrow().config.config_dir.clone();
                        let title = ctrl.borrow().config.t(if action == 1 {
                            "queries.load"
                        } else {
                            "spaces.save_as"
                        });
                        let dialog = rfd::FileDialog::new()
                            .set_title(title)
                            .set_directory(directory)
                            .add_filter("Naygo", &["naygosearch"]);
                        let path = if action == 1 {
                            dialog.pick_file()
                        } else {
                            dialog.save_file()
                        };
                        if let Some(mut path) = path {
                            if action == 1 {
                                ctrl.borrow_mut().query_read(path);
                            } else if let Some(query) = query {
                                if !path
                                    .extension()
                                    .is_some_and(|e| e.eq_ignore_ascii_case("naygosearch"))
                                {
                                    let mut name = path.into_os_string();
                                    name.push(".naygosearch");
                                    path = name.into();
                                }
                                ctrl.borrow_mut().query_save(Some(path), query);
                            }
                        }
                    }
                    3 => {
                        if let Some(query) = query {
                            ctrl.borrow_mut().query_save(None, query);
                        }
                    }
                    4 => {
                        if let Some(query) = query {
                            ctrl.borrow_mut().query_execute(query);
                        }
                    }
                    _ => {}
                }
            }
        }
        timer();
        sync();
    });
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_rows.clone();
    let timer = ctx.start_timer.clone();
    ui.on_search_select(move |i, control, shift| {
        if i >= 0 {
            ctrl.borrow_mut().search_select(i as usize, control, shift);
        }
        timer();
        sync();
    });
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_layout.clone();
    let timer = ctx.start_timer.clone();
    ui.on_search_to_basket(move || {
        ctrl.borrow_mut().search_to_basket();
        timer();
        sync();
    });
    let ctrl = ctx.ctrl.clone();
    let sync = ctx.sync_rows.clone();
    let timer = ctx.start_timer.clone();
    ui.on_search_rerun(move || {
        ctrl.borrow_mut().query_rerun();
        timer();
        sync();
    });
}
