// Naygo — pruebas de publicación inmutable y protección de retirada de puntos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
#[test]
fn immutable_save_export_cancel_and_stale_recycle() {
    let dir = tempfile::tempdir().unwrap();
    let token = CancellationToken::new();
    let point = comparison_point::capture(
        comparison_point::Scope {
            root: dir.path().into(),
            recursive: true,
            hashed: false,
            exclusions: vec![],
        },
        &token,
        |_| {},
    )
    .unwrap();
    let path = dir.path().join("test.naygopoint");
    let rev = save(&path, &point, &token).unwrap();
    let original = std::fs::read(&path).unwrap();
    assert!(matches!(
        save(&path, &point, &token),
        Err(SpaceError::Exists)
    ));
    assert!(matches!(
        save(&dir.path().join("wrong.txt"), &point, &token),
        Err(SpaceError::Invalid)
    ));
    assert!(matches!(
        recycle(&path, "stale", &token),
        Err(SpaceError::Changed)
    ));
    token.cancel();
    assert!(matches!(
        recycle(&path, &rev, &token),
        Err(SpaceError::Cancelled)
    ));
    let cancelled = dir.path().join("cancel.naygopoint");
    assert!(matches!(
        save(&cancelled, &point, &token),
        Err(SpaceError::Cancelled)
    ));
    assert!(!cancelled.exists());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
