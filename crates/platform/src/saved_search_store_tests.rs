// Naygo — pruebas de guardado seguro de consultas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
#[test]
fn create_update_conflict_and_cancel_preserve_query_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Test.naygosearch");
    let mut query = SavedSearch {
        version: 1,
        name: "Test".into(),
        roots: vec![dir.path().into()],
        options: Default::default(),
        min_bytes: None,
        max_bytes: None,
        modified: Default::default(),
    };
    let token = CancellationToken::new();
    let first = save(&path, &query, None, &token).unwrap();
    assert!(matches!(
        save(&path, &query, None, &token),
        Err(SpaceError::Exists)
    ));
    query.name = "Changed".into();
    let second = save(&path, &query, Some(&first), &token).unwrap();
    assert_ne!(first, second);
    assert!(matches!(
        save(&path, &query, Some(&first), &token),
        Err(SpaceError::Changed)
    ));
    token.cancel();
    assert!(matches!(
        save(&path, &query, Some(&second), &token),
        Err(SpaceError::Cancelled)
    ));
    assert_eq!(
        saved_search::read(&path, &CancellationToken::new())
            .unwrap()
            .0,
        query
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
