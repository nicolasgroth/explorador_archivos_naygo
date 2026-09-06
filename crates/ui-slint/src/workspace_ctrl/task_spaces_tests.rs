// Naygo — pruebas del cambio de tarea y sus límites de persistencia.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;

fn drain(c: &mut WorkspaceCtrl) {
    for _ in 0..3000 {
        let done = c.pump_task_spaces();
        if done {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("space worker timeout");
}
fn controller(dir: &std::path::Path) -> WorkspaceCtrl {
    WorkspaceCtrl::new_in(dir.to_path_buf(), dir.to_path_buf())
}

#[test]
fn explicit_roundtrip_keeps_layout_filter_basket_and_does_not_load_until_confirmed() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = controller(dir.path());
    c.basket.add([dir.path().join("missing-reference.txt")]);
    c.typeahead = "report".into();
    c.filter_hide_nonmatches = true;
    c.ws.active_files_mut()
        .unwrap()
        .set_visual_filter(Some("report".into()));
    c.spaces_open();
    let path = dir.path().join("Audit.naygospace");
    c.spaces_save(Some(path.clone()), None, false);
    drain(&mut c);
    assert!(c.task_spaces.can_update(), "{}", c.task_spaces.report);
    assert!(!c.task_spaces.dirty);
    let original = c.space_snapshot("Audit").unwrap().signature().unwrap();
    c.spaces_close();
    c.basket.clear();
    c.ws.active_files_mut().unwrap().set_visual_filter(None);
    c.spaces_open();
    assert!(c.task_spaces.dirty);
    c.spaces_read(path, None);
    drain(&mut c);
    assert!(c.basket.is_empty());
    assert!(c.task_spaces.has_pending());
    c.spaces_apply_pending();
    assert_eq!(c.basket.len(), 1);
    assert_eq!(c.typeahead, "report");
    assert!(c.filter_hide_nonmatches);
    assert_eq!(
        c.space_snapshot("Audit").unwrap().signature().unwrap(),
        original
    );
    assert!(!c.task_spaces.dirty);
    assert!(!c.task_spaces.open);
}

#[test]
fn new_controller_reopens_saved_space_and_rebases_without_reading_references() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Portable.naygospace");
    {
        let mut c = controller(dir.path());
        c.spaces_open();
        c.basket.add([dir.path().join("absent/item.txt")]);
        c.spaces_save(Some(path.clone()), Some(dir.path().to_path_buf()), false);
        drain(&mut c);
        assert!(c.task_spaces.can_update(), "{}", c.task_spaces.report);
    }
    let root = dir.path().join("disconnected");
    let mut c = controller(dir.path());
    c.spaces_open();
    c.spaces_read(path.clone(), Some(root.clone()));
    drain(&mut c);
    assert!(c.task_spaces.has_pending(), "{}", c.task_spaces.report);
    c.spaces_apply_pending();
    assert_eq!(c.active_dir(), Some(root.clone()));
    assert_eq!(c.basket.items(), &[root.join("absent/item.txt")]);
    c.spaces_open();
    c.spaces_save(None, None, false);
    drain(&mut c);
    let saved = task_space::read(&path, &CancellationToken::new())
        .unwrap()
        .0;
    assert_eq!(saved.base_root, Some(root));
    assert!(!c.task_spaces.dirty, "{}", c.task_spaces.report);
}

#[test]
fn external_edit_blocks_save_before_switch_and_preserves_current_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("A.naygospace");
    let other = dir.path().join("B.naygospace");
    let mut c = controller(dir.path());
    c.spaces_open();
    c.spaces_save(Some(path.clone()), None, false);
    drain(&mut c);
    let s = c.space_snapshot("B").unwrap();
    naygo_platform::task_space_store::save(&other, &s, None, &CancellationToken::new()).unwrap();
    c.spaces_read(other, None);
    drain(&mut c);
    c.basket.add([dir.path().join("keep.txt")]);
    std::fs::write(&path, b"external edit").unwrap();
    c.spaces_save(None, None, true);
    drain(&mut c);
    assert_eq!(c.basket.len(), 1);
    assert_eq!(c.task_spaces.name(), "A");
    assert!(c.task_spaces.has_pending());
    assert!(c.task_spaces.open);
    assert_eq!(std::fs::read(path).unwrap(), b"external edit");
}

#[test]
fn closing_review_never_changes_workspace_or_corrupt_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Broken.naygospace");
    let mut c = controller(dir.path());
    c.spaces_open();
    let before = c.space_snapshot("Before").unwrap().signature().unwrap();
    std::fs::write(&path, b"broken").unwrap();
    c.spaces_read(path.clone(), None);
    drain(&mut c);
    assert!(!c.task_spaces.has_pending());
    c.spaces_apply_pending();
    c.spaces_close();
    assert_eq!(
        c.space_snapshot("Before").unwrap().signature().unwrap(),
        before
    );
    assert_eq!(std::fs::read(path).unwrap(), b"broken");
}

#[test]
fn operation_keeps_captured_destination_when_workspace_changes() {
    use naygo_core::ops::{ConflictPolicy, OpKind, OpRequest};
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("destination");
    std::fs::create_dir(&destination).unwrap();
    let source = dir.path().join("source.txt");
    std::fs::write(&source, b"original").unwrap();
    let mut c = controller(dir.path());
    let stored = c.space_snapshot("Other").unwrap();
    c.task_spaces.pending = Some(SavedSpace {
        path: dir.path().join("Other.naygospace"),
        revision: "unused".into(),
        resolved: stored.clone(),
        stored,
    });
    c.ops.start_op(
        OpRequest {
            kind: OpKind::Copy,
            sources: vec![source.clone()],
            dest_dir: Some(destination.clone()),
            conflict: ConflictPolicy::Skip,
        },
        "Test copy".into(),
        false,
    );
    assert!(!c.ops.active_ops.is_empty());
    let id = c.ops.active_ops[0].id;
    c.spaces_apply_pending();
    assert_eq!(c.ops.active_ops[0].id, id);
    for _ in 0..5000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        std::fs::read(destination.join("source.txt")).unwrap(),
        b"original"
    );
    assert_eq!(std::fs::read(source).unwrap(), b"original");
}
