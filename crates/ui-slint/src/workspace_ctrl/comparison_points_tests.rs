// Naygo — contrato UI: apertura pasiva, selección segura y cancelación de puntos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
fn drain(c: &mut WorkspaceCtrl) {
    for _ in 0..5000 {
        if c.pump_comparison_points() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("point worker timed out");
}
fn scope(root: &Path) -> Scope {
    Scope {
        root: root.into(),
        recursive: true,
        hashed: true,
        exclusions: vec![],
    }
}

#[test]
fn restart_load_is_passive_then_compare_and_export_preserve_baseline() {
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    std::fs::write(data.path().join("a"), "before").unwrap();
    let path = config.path().join("baseline.naygopoint");
    let mut c = WorkspaceCtrl::new_in(data.path().into(), config.path().into());
    assert!(c.points_open());
    assert!(!c.query_open(None));
    c.points_capture(path.clone(), scope(data.path()));
    drain(&mut c);
    assert!(
        c.comparison_points.loaded(),
        "{}",
        c.comparison_points.report
    );
    let original = std::fs::read(&path).unwrap();
    drop(c);
    std::fs::write(data.path().join("a"), "after and larger").unwrap();
    let mut c = WorkspaceCtrl::new_in(data.path().into(), config.path().into());
    c.points_open();
    c.points_read(path.clone());
    drain(&mut c);
    assert!(c.comparison_points.rows.is_empty());
    assert!(c.search_job.is_none());
    c.points_compare();
    drain(&mut c);
    assert_eq!(c.comparison_points.rows.len(), 1);
    assert!(c.comparison_points.rows[0].actionable);
    let exported = config.path().join("copy.naygopoint");
    c.points_export(exported.clone());
    drain(&mut c);
    assert_eq!(std::fs::read(exported).unwrap(), original);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(c.comparison_points.loaded.as_ref().unwrap().path, path);
    c.points_select(0, true);
    c.points_to_basket();
    assert!(!c.comparison_points.open);
    assert!(c.basket.items().contains(&data.path().join("a")));
}
#[test]
fn missing_rows_never_become_basket_sources_even_from_forced_callback() {
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let file = data.path().join("gone");
    std::fs::write(&file, "a").unwrap();
    let mut c = WorkspaceCtrl::new_in(data.path().into(), config.path().into());
    c.points_open();
    c.points_capture(config.path().join("base.naygopoint"), scope(data.path()));
    drain(&mut c);
    std::fs::remove_file(file).unwrap();
    c.points_compare();
    drain(&mut c);
    c.points_select(0, true);
    assert!(!c.comparison_points.selected());
    c.points_to_basket();
    assert!(c.basket.is_empty());
}
#[test]
fn delete_requires_confirmation_and_cancel_preserves_document_and_data() {
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let path = config.path().join("base.naygopoint");
    let mut c = WorkspaceCtrl::new_in(data.path().into(), config.path().into());
    c.points_open();
    c.points_capture(path.clone(), scope(data.path()));
    drain(&mut c);
    c.points_delete();
    assert!(c.comparison_points.confirm_delete);
    assert!(!c.comparison_points.busy());
    c.points_close();
    assert!(!c.comparison_points.confirm_delete);
    assert!(c.comparison_points.open);
    assert!(path.exists());
}
#[test]
fn closing_cancels_worker_and_completion_does_not_reopen_modal() {
    let data = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(data.path().into(), data.path().into());
    c.points_open();
    c.points_job(|token, _| {
        while !token.is_cancelled() {
            std::thread::yield_now();
        }
        Err(SpaceError::Cancelled)
    });
    c.points_close();
    drain(&mut c);
    assert!(!c.comparison_points.open);
}
#[test]
fn compare_clears_old_selection_immediately_and_disconnect_does_not_stick() {
    let data = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(data.path().into(), config.path().into());
    c.points_open();
    c.points_capture(config.path().join("base.naygopoint"), scope(data.path()));
    drain(&mut c);
    std::fs::write(data.path().join("new"), "a").unwrap();
    c.points_compare();
    drain(&mut c);
    c.points_select(0, true);
    assert!(c.comparison_points.selected());
    c.points_compare();
    assert!(!c.comparison_points.selected());
    drain(&mut c);
    let (tx, rx) = mpsc::channel();
    drop(tx);
    c.comparison_points.rx = Some(rx);
    assert!(c.pump_comparison_points());
    assert!(!c.comparison_points.busy());
}
