// Naygo — pruebas de espacios de trabajo versionados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use crate::workspace::{DockNode, FilePaneState, SerializableDockLayout};

fn sample(root: &Path) -> TaskSpace {
    from_workspace(
        "Proyecto".into(),
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
            operations_placement: None,
        },
    )
    .unwrap()
}

#[test]
fn query_references_roundtrip_rebase_and_legacy_spaces_default_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut space = sample(dir.path());
    let mut legacy = serde_json::to_value(&space).unwrap();
    legacy.as_object_mut().unwrap().remove("saved_searches");
    assert!(serde_json::from_value::<TaskSpace>(legacy)
        .unwrap()
        .saved_searches
        .is_empty());
    space
        .saved_searches
        .push(dir.path().join("queries/a.naygosearch"));
    let portable = space.with_relative_paths(dir.path()).unwrap();
    let other = dir.path().join("other");
    assert_eq!(
        portable.resolved(Some(&other)).unwrap().saved_searches,
        vec![other.join("queries/a.naygosearch")]
    );
    assert_eq!(
        space.saved_searches,
        vec![dir.path().join("queries/a.naygosearch")]
    );
}
#[test]
fn recipe_references_are_portable_and_legacy_spaces_default_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut space = sample(dir.path());
    let mut legacy = serde_json::to_value(&space).unwrap();
    legacy.as_object_mut().unwrap().remove("saved_recipes");
    assert!(serde_json::from_value::<TaskSpace>(legacy)
        .unwrap()
        .saved_recipes
        .is_empty());
    space.saved_recipes = vec![dir.path().join("recipe.naygorecipe")];
    let portable = space.with_relative_paths(dir.path()).unwrap();
    assert_eq!(
        portable.saved_recipes,
        vec![PathBuf::from("recipe.naygorecipe")]
    );
    let other = tempfile::tempdir().unwrap();
    assert_eq!(
        portable.resolved(Some(other.path())).unwrap().saved_recipes,
        vec![other.path().join("recipe.naygorecipe")]
    );
    space.saved_recipes = vec![dir.path().join("recipe.naygorecipe"); 65];
    assert!(matches!(space.validate(), Err(SpaceError::Limit)));
}

#[test]
fn favorites_scope_defaults_global_and_private_favorites_are_portable() {
    let dir = tempfile::tempdir().unwrap();
    let mut space = sample(dir.path());
    let legacy: serde_json::Value = serde_json::from_slice(&space.to_bytes().unwrap()).unwrap();
    let mut legacy = legacy;
    legacy.as_object_mut().unwrap().remove("favorite_scope");
    legacy.as_object_mut().unwrap().remove("favorites");
    legacy["version"] = serde_json::json!(1);
    let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
    let legacy: TaskSpace = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(legacy.favorite_scope, FavoriteScope::Global);
    assert!(legacy.favorites.is_none());

    let legacy_path = dir.path().join("legacy.naygospace");
    std::fs::write(&legacy_path, legacy_bytes).unwrap();
    let (migrated, _) = read(&legacy_path, &CancellationToken::new()).unwrap();
    assert_eq!(migrated.version, VERSION);
    assert_eq!(migrated.favorite_scope, FavoriteScope::Global);

    let favorite_path = dir.path().join("photos");
    let mut favorites = Favorites::new();
    favorites.add_favorite(&favorite_path);
    space.favorite_scope = FavoriteScope::PerSpace;
    space.favorites = Some(favorites);

    let portable = space.with_relative_paths(dir.path()).unwrap();
    let favorite = portable.favorites.as_ref().unwrap().list_flat().remove(0);
    assert_eq!(favorite.path, PathBuf::from("photos"));
    assert_eq!(favorite.label, "photos");
    assert!(portable.references_under(Path::new("photos")));

    let new_root = dir.path().join("other");
    let resolved = space
        .with_relative_paths(dir.path())
        .unwrap()
        .resolved(Some(&new_root))
        .unwrap();
    assert_eq!(
        resolved.favorites.unwrap().list_flat()[0].path,
        new_root.join("photos")
    );
    assert_eq!(space.favorite_scope, FavoriteScope::PerSpace);
}

#[test]
fn favorites_scope_and_payload_must_match_and_private_tree_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let mut space = sample(dir.path());
    space.favorite_scope = FavoriteScope::PerSpace;
    assert!(matches!(space.validate(), Err(SpaceError::Invalid)));

    space.favorite_scope = FavoriteScope::Global;
    space.favorites = Some(Favorites::new());
    assert!(matches!(space.validate(), Err(SpaceError::Invalid)));

    space.favorite_scope = FavoriteScope::PerSpace;
    let mut favorites = Favorites::new();
    for index in 0..=MAX_FAVORITES {
        favorites.add_favorite(&dir.path().join(format!("folder-{index}")));
    }
    space.favorites = Some(favorites);
    assert!(matches!(space.validate(), Err(SpaceError::Limit)));
}

#[test]
fn portable_roundtrip_keeps_original_immutable() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = sample(&dir.path().join("sub"));
    s.basket.push(dir.path().join("sub/item.txt"));
    s.visual_filters.push((PaneId(1), "report".into()));
    let original = s.to_bytes().unwrap();
    let portable = s.with_relative_paths(dir.path()).unwrap();
    assert_eq!(portable.workspace.files[0].1.current_dir, Path::new("sub"));
    assert_eq!(
        portable.resolved(None).unwrap().to_bytes().unwrap(),
        original
    );
    let other = dir.path().join("other");
    let rebased = portable.resolved(Some(&other)).unwrap();
    assert_eq!(rebased.basket[0], other.join("sub/item.txt"));
    assert_eq!(s.to_bytes().unwrap(), original);
    assert_eq!(rebased.visual_filters, s.visual_filters);
}

#[cfg(unix)]
#[test]
fn task_space_persists_non_utf8_paths_in_basket_and_references() {
    use std::os::unix::ffi::OsStringExt;

    let path = PathBuf::from(std::ffi::OsString::from_vec(
        b"/tmp/work-\xff/item-\xfe".to_vec(),
    ));
    let root = PathBuf::from(std::ffi::OsString::from_vec(b"/tmp/work-\xff".to_vec()));
    let mut space = sample(&root);
    space.basket.push(path.clone());
    space.saved_searches.push(path.clone());
    space.saved_recipes.push(path.clone());

    let bytes = space.to_bytes().unwrap();
    assert!(std::str::from_utf8(&bytes)
        .unwrap()
        .contains(r#""encoding":"unix-bytes-v1""#));
    let restored: TaskSpace = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.basket, vec![path.clone()]);
    assert_eq!(restored.saved_searches, vec![path.clone()]);
    assert_eq!(restored.saved_recipes, vec![path]);
    assert_eq!(restored.workspace.files[0].1.current_dir, root);
}

#[test]
fn root_itself_can_be_relative_and_unrelated_root_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let s = sample(dir.path());
    let portable = s.with_relative_paths(dir.path()).unwrap();
    assert!(portable.workspace.files[0]
        .1
        .current_dir
        .as_os_str()
        .is_empty());
    assert!(s.with_relative_paths(&dir.path().join("other")).is_err());
    assert!(s.resolved(Some(dir.path())).is_err());
}

#[test]
fn rejects_traversal_unknown_version_and_oversize_basket() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = sample(dir.path()).with_relative_paths(dir.path()).unwrap();
    s.basket.push("../escape".into());
    assert!(matches!(s.validate(), Err(SpaceError::Invalid)));
    s.basket.clear();
    s.version = VERSION + 1;
    assert!(matches!(s.validate(), Err(SpaceError::Version)));
    s.version = VERSION;
    s.basket.resize(MAX_BASKET + 1, "item".into());
    assert!(matches!(s.validate(), Err(SpaceError::Limit)));
}

#[test]
fn malformed_pane_relationships_never_restore_partial_space() {
    let dir = tempfile::tempdir().unwrap();
    let s = sample(dir.path());
    let mut bad = s.clone();
    bad.workspace.files.clear();
    assert!(bad.validate().is_err());
    let mut bad = s.clone();
    bad.workspace.active = Some(PaneId(33));
    assert!(bad.validate().is_err());
    let mut bad = s.clone();
    bad.workspace.purposes.push((PaneId(1), PanePurpose::Files));
    assert!(bad.validate().is_err());
    let mut bad = s.clone();
    bad.workspace.tree_links.push((PaneId(1), PaneId(1)));
    assert!(bad.validate().is_err());
    let mut bad = s;
    bad.visual_filters.push((PaneId(2), "x".into()));
    assert!(bad.validate().is_err());
}

#[test]
fn reader_is_bounded_cancellable_and_rejects_extra_executable_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("space");
    let token = CancellationToken::new();
    let s = sample(dir.path());
    let bytes = s.to_bytes().unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(read(&path, &token).unwrap().1, revision(&bytes));
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["execute"] = "anything".into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(read(&path, &token), Err(SpaceError::Invalid)));
    std::fs::write(&path, vec![b' '; MAX_BYTES + 1]).unwrap();
    assert!(matches!(read(&path, &token), Err(SpaceError::Limit)));
    token.cancel();
    assert!(matches!(read(&path, &token), Err(SpaceError::Cancelled)));
}

#[test]
fn names_reject_windows_devices_and_path_injection() {
    for name in [
        "", "../x", "a/b", "a\\b", "nul", "CON.txt", "LPT1", "COM¹", "project.", "space ",
    ] {
        assert!(!valid_name(name), "{name}");
    }
    assert!(valid_name("Auditoría – septiembre"));
}

#[test]
fn staging_reference_detection_is_component_based_and_covers_panes_and_basket() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("stage");
    let mut s = sample(&root.join("sub"));
    assert!(s.references_under(&root));
    s.workspace.files[0].1.current_dir = dir.path().join("staged-other");
    assert!(!s.references_under(&root));
    s.basket.push(root.join("file.txt"));
    assert!(s.references_under(&root));
}
