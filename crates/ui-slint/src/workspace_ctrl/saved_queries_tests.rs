// Naygo — pruebas de contexto y persistencia explícita de consultas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
fn drain(c: &mut WorkspaceCtrl) {
    for _ in 0..5000 {
        if c.pump_saved_queries() && c.pump_search() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("query worker timed out");
}
#[test]
fn worker_disconnect_is_reported_partial_instead_of_running_forever() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), dir.path().into());
    c.open_empty_search();
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    let job = c.search_job.as_mut().unwrap();
    job.done = false;
    job.rx = rx;
    assert!(c.pump_search());
    assert!(c.search_job.as_ref().unwrap().partial);
}

#[test]
fn loading_and_saving_never_execute_and_unknown_revisions_do_not_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), dir.path().into());
    assert!(c.query_open(None));
    let path = dir.path().join("Audit.naygosearch");
    c.query_save(Some(path.clone()), c.saved_queries.draft.clone());
    drain(&mut c);
    assert!(c.saved_queries.can_update(), "{}", c.saved_queries.report);
    assert!(c.search_job.is_none());
    c.query_close();
    assert!(c.query_open(None));
    c.query_read(path.clone());
    drain(&mut c);
    assert!(c.search_job.is_none());
    assert_eq!(c.saved_queries.references, vec![path.clone()]);
    assert_eq!(
        c.space_snapshot("Work").unwrap().saved_searches,
        vec![path.clone()]
    );
    let mut other = c.saved_queries.draft.clone();
    other.name = "Other".into();
    std::fs::write(&path, other.to_bytes().unwrap()).unwrap();
    c.query_save(None, c.saved_queries.draft.clone());
    drain(&mut c);
    assert_eq!(
        saved_search::read(&path, &CancellationToken::new())
            .unwrap()
            .0,
        other
    );
    c.query_forget();
    assert!(path.exists());
    assert!(c.saved_queries.references.is_empty());
}
#[test]
fn results_own_keyboard_preview_properties_and_basket_without_touching_files_selection() {
    let dir = tempfile::tempdir().unwrap();
    for n in ["a.txt", "b.txt"] {
        std::fs::write(dir.path().join(n), n).unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), dir.path().into());
    c.query_open(None);
    let mut query = c.saved_queries.draft.clone();
    query.options.name_query = ".txt".into();
    c.query_execute(query);
    drain(&mut c);
    assert_eq!(c.search_job.as_ref().unwrap().hits.len(), 2);
    assert!(c.search_select(0, false, false));
    let first = c.search_selected_entry().unwrap().path.clone();
    c.run_action(Action::MoveDown);
    assert_ne!(c.search_selected_entry().unwrap().path, first);
    assert!(c.search_context);
    assert!(!c.inspector_info().name.is_empty());
    c.run_action(Action::DeletePermanent);
    assert!(c.ops.pending_dialog.is_none());
    assert!(dir.path().join("a.txt").exists());
    c.run_action(Action::SelectAll);
    assert_eq!(c.search_marked_count(), 2);
    c.search_to_basket();
    assert_eq!(c.basket.len(), 2);
}
#[test]
fn rerunning_an_older_result_does_not_change_loaded_query_identity() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), dir.path().into());
    c.query_open(None);
    let mut first = c.saved_queries.draft.clone();
    first.name = "First".into();
    c.query_execute(first.clone());
    drain(&mut c);
    c.query_open(None);
    let mut second = first.clone();
    second.name = "Second".into();
    let path = dir.path().join("Second.naygosearch");
    c.query_save(Some(path.clone()), second.clone());
    drain(&mut c);
    c.query_close();
    c.query_rerun();
    drain(&mut c);
    assert_eq!(
        c.search_job.as_ref().unwrap().details.advanced.as_ref(),
        Some(&first)
    );
    assert_eq!(c.saved_queries.draft, second);
    assert_eq!(c.saved_queries.loaded.as_ref().unwrap().0, path);
}
#[test]
fn cancelling_a_pending_load_cannot_replace_the_editor_draft() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), dir.path().into());
    c.query_open(None);
    let draft = c.saved_queries.draft.clone();
    let mut other = draft.clone();
    other.name = "Other".into();
    let path = dir.path().join("Other.naygosearch");
    std::fs::write(&path, other.to_bytes().unwrap()).unwrap();
    c.query_read(path);
    c.query_close();
    drain(&mut c);
    assert_eq!(c.saved_queries.draft, draft);
    assert!(!c.saved_queries.open);
}
