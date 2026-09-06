// Naygo — persistencia de recetas protegida por revisión y cancelación.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
#[test]
fn create_update_conflict_cancel_and_read_are_safe() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Test.naygorecipe");
    let mut recipe = Recipe::from_query(&naygo_core::saved_search::SavedSearch {
        version: 1,
        name: "Test".into(),
        roots: vec![],
        options: Default::default(),
        min_bytes: None,
        max_bytes: None,
        modified: Default::default(),
    });
    let token = CancellationToken::new();
    let first = save(&path, &recipe, None, &token).unwrap();
    assert!(matches!(
        save(&path, &recipe, None, &token),
        Err(SpaceError::Exists)
    ));
    recipe.name = "Other".into();
    let second = save(&path, &recipe, Some(&first), &token).unwrap();
    assert_ne!(first, second);
    assert!(matches!(
        save(&path, &recipe, Some(&first), &token),
        Err(SpaceError::Changed)
    ));
    token.cancel();
    assert!(matches!(
        save(&path, &recipe, Some(&second), &token),
        Err(SpaceError::Cancelled)
    ));
    assert_eq!(
        recipe::read(&path, &CancellationToken::new()).unwrap().0,
        recipe
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
