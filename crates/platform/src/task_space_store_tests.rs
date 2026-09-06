// Naygo — pruebas de publicación de espacios sin pérdida por sobrescritura.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use naygo_core::config::{WorkspacePersist, CONFIG_VERSION};
use naygo_core::workspace::{DockNode, FilePaneState, PaneId, PanePurpose, SerializableDockLayout};

fn sample(root: &Path) -> TaskSpace {
    naygo_core::task_space::from_workspace(
        "Task".into(),
        WorkspacePersist {
            version: CONFIG_VERSION,
            layout: SerializableDockLayout {
                root: Some(DockNode::Leaf(PaneId(1))),
            },
            active: Some(PaneId(1)),
            files: vec![(
                PaneId(1),
                FilePaneState::new(root.to_path_buf()).to_persist(),
            )],
            purposes: vec![(PaneId(1), PanePurpose::Files)],
            tree_links: vec![],
        },
    )
    .unwrap()
}

#[test]
fn create_update_and_collision_preserve_user_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Task.naygospace");
    let token = CancellationToken::new();
    let mut s = sample(dir.path());
    let first = save(&path, &s, None, &token).unwrap();
    assert_eq!(read(&path, &token).unwrap().1, first);
    s.basket.push(dir.path().join("missing.txt"));
    assert!(matches!(
        save(&path, &s, None, &token),
        Err(SpaceError::Exists)
    ));
    assert_eq!(read(&path, &token).unwrap().1, first);
    let second = save(&path, &s, Some(&first), &token).unwrap();
    assert_ne!(first, second);
    assert_eq!(read(&path, &token).unwrap().0.basket, s.basket);
    assert!(matches!(
        save(&path, &s, Some(&first), &token),
        Err(SpaceError::Changed)
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn corrupt_file_cancellation_and_io_error_leave_no_owned_temp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Task.naygospace");
    let token = CancellationToken::new();
    let s = sample(dir.path());
    std::fs::write(&path, b"corrupt user data").unwrap();
    assert!(matches!(
        save(&path, &s, Some("old"), &token),
        Err(SpaceError::Invalid)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"corrupt user data");
    assert!(save(&dir.path().join("absent/Task.naygospace"), &s, None, &token).is_err());
    token.cancel();
    assert!(matches!(
        save(&dir.path().join("other.naygospace"), &s, None, &token),
        Err(SpaceError::Cancelled)
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn concurrent_creators_publish_exactly_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Task.naygospace");
    let s = sample(dir.path());
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let path = path.clone();
            let s = s.clone();
            std::thread::spawn(move || save(&path, &s, None, &CancellationToken::new()))
        })
        .collect();
    let successes = handles
        .into_iter()
        .filter_map(|h| h.join().unwrap().ok())
        .count();
    assert_eq!(successes, 1);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
