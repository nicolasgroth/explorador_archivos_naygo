// Naygo — pruebas del editor pasivo y planes invalidados de recetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
fn drain(c: &mut WorkspaceCtrl) {
    for _ in 0..5000 {
        if c.pump_recipes() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("recipe worker timed out");
}
#[test]
fn loading_and_saving_are_passive_and_references_belong_to_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), data.path().into());
    assert!(c.recipe_open());
    let path = dir.path().join("Test.naygorecipe");
    c.recipe_save(Some(path.clone()));
    drain(&mut c);
    assert!(c.recipes.can_update(), "{}", c.recipes.report);
    assert!(!c.recipes.ready());
    assert!(!c.ops.any_running());
    assert!(c.search_job.is_none());
    c.recipe_close();
    assert!(c.recipe_open());
    c.recipe_read(path.clone(), false);
    drain(&mut c);
    assert!(!c.recipe_execute());
    assert!(c.search_job.is_none());
    assert_eq!(
        c.space_snapshot("Work").unwrap().saved_recipes,
        vec![path.clone()]
    );
    c.recipe_forget();
    assert!(path.exists());
    assert!(c.recipes.references.is_empty());
}
#[test]
fn changing_recipe_or_parameters_requires_a_new_review() {
    let dir = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), b"a").unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), data.path().into());
    c.recipe_open();
    c.recipes.draft.source = Source::Query;
    c.recipe_prepare();
    drain(&mut c);
    assert!(c.recipes.ready(), "{}", c.recipes.report);
    let mut changed = c.recipes.draft.clone();
    changed.output_name = "other".into();
    c.recipe_edit(changed, c.recipes.parameters.clone());
    assert!(!c.recipes.ready());
    assert!(!c.recipe_execute());
    c.recipe_prepare();
    drain(&mut c);
    assert!(c.recipes.ready());
    c.recipe_invalidate();
    assert!(!c.recipes.ready());
    assert!(!c.ops.any_running());
}
#[test]
fn closed_or_disconnected_worker_cannot_execute_late() {
    let dir = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), data.path().into());
    c.recipe_open();
    c.recipe_read(dir.path().join("missing.naygorecipe"), false);
    c.recipe_close();
    assert!(c.pump_recipes());
    assert!(!c.recipes.open);
    assert!(!c.recipes.ready());
    let (tx, rx) = mpsc::channel();
    drop(tx);
    c.recipes.rx = Some(rx);
    assert!(c.pump_recipes());
    assert!(!c.recipes.busy());
    assert!(!c.recipes.report.is_empty());
}
#[test]
fn execution_rechecks_the_frozen_parameters_even_without_widget_invalidation() {
    let dir = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), b"a").unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), data.path().into());
    c.recipe_open();
    c.recipes.draft.source = Source::Query;
    c.recipe_prepare();
    drain(&mut c);
    assert!(c.recipes.ready());
    c.recipes.parameters.parent = dir.path().join("elsewhere");
    assert!(!c.recipe_execute());
    assert!(!c.ops.any_running());
    assert!(c.recipes.open);
}
#[test]
fn explicit_execution_uses_operations_history_and_preserves_originals() {
    let dir = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let source = dir.path().join("a.txt");
    std::fs::write(&source, b"a").unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().into(), data.path().into());
    c.recipe_open();
    c.recipes.draft.source = Source::Query;
    c.recipes.draft.name = "Audit".into();
    c.recipes.draft.output_name = "out".into();
    c.recipe_prepare();
    drain(&mut c);
    assert!(c.recipes.ready());
    assert!(c.recipe_execute());
    assert!(!c.recipes.open);
    assert!(c.ops.active_ops.iter().any(|op| op.label.contains("Audit")));
    for _ in 0..5000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!c.ops.any_running());
    assert_eq!(std::fs::read(dir.path().join("out/a.txt")).unwrap(), b"a");
    assert!(source.exists());
}
