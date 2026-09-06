// Naygo — regresiones de cobertura, formato, hashes y límites de los puntos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use std::fs;

fn scope(root: &Path) -> Scope {
    Scope {
        root: root.to_owned(),
        recursive: true,
        exclusions: vec![],
        hashed: false,
    }
}
fn snap(root: &Path) -> ComparisonPoint {
    capture(scope(root), &CancellationToken::new(), |_| {}).unwrap()
}
fn diff(a: &ComparisonPoint, b: &ComparisonPoint) -> Comparison {
    compare(a, b, &CancellationToken::new()).unwrap()
}
fn file(path: &str) -> Record {
    Record {
        path: path.into(),
        kind: EntryKind::File,
        bytes: 1,
        modified_ns: Some(1),
        sha256: None,
    }
}

#[test]
fn capture_roundtrip_and_metadata_only() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("a"), "abc").unwrap();
    let p = snap(t.path());
    assert_eq!(p.entries.len(), 1);
    assert!(p.complete());
    let bytes = p.to_bytes().unwrap();
    let doc = t.path().join("point.naygopoint");
    fs::write(&doc, bytes).unwrap();
    let (loaded, revision) = read(&doc, &CancellationToken::new()).unwrap();
    assert_eq!(revision.len(), 64);
    assert_eq!(
        diff(&p, &loaded).counts[ChangeKind::SameMetadata as usize],
        1
    );
}
#[test]
fn added_modified_missing_and_no_missing_actions() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("a"), "a").unwrap();
    fs::write(t.path().join("b"), "b").unwrap();
    let p = snap(t.path());
    fs::remove_file(t.path().join("a")).unwrap();
    fs::write(t.path().join("b"), "longer").unwrap();
    fs::write(t.path().join("c"), "new").unwrap();
    let r = diff(&p, &snap(t.path()));
    assert_eq!(&r.counts[..3], &[1, 1, 1]);
    assert!(r
        .rows
        .iter()
        .filter(|r| r.kind == ChangeKind::Missing)
        .all(|r| !r.actionable));
}
#[test]
fn partial_current_never_proves_missing() {
    let t = tempfile::tempdir().unwrap();
    let mut before = snap(t.path());
    before.entries = vec![file("blocked/a"), file("clear/b")];
    let mut now = snap(t.path());
    now.gaps.push("blocked".into());
    let r = diff(&before, &now);
    assert_eq!(
        r.rows
            .iter()
            .find(|r| r.path == Path::new("blocked/a"))
            .unwrap()
            .kind,
        ChangeKind::Unknown
    );
    assert_eq!(
        r.rows
            .iter()
            .find(|r| r.path == Path::new("clear/b"))
            .unwrap()
            .kind,
        ChangeKind::Missing
    );
    assert!(r.partial);
}
#[test]
fn partial_baseline_never_proves_added() {
    let t = tempfile::tempdir().unwrap();
    let mut a = snap(t.path());
    a.gaps.push(PathBuf::new());
    let mut b = snap(t.path());
    b.entries.push(file("new"));
    assert_eq!(diff(&a, &b).counts[ChangeKind::Added as usize], 0);
}
#[test]
fn truncated_current_never_proves_missing() {
    let t = tempfile::tempdir().unwrap();
    let mut a = snap(t.path());
    a.entries.push(file("old"));
    let mut b = snap(t.path());
    b.truncated = true;
    assert_eq!(diff(&a, &b).counts[ChangeKind::Missing as usize], 0);
}
#[test]
fn inaccessible_root_is_partial_not_an_empty_complete_folder() {
    let t = tempfile::tempdir().unwrap();
    let p = snap(&t.path().join("gone"));
    assert!(!p.complete());
    assert_eq!(p.gaps, vec![PathBuf::new()]);
    let r = diff(&p, &p);
    assert!(r.partial);
    assert!(r.rows.iter().any(|r| r.kind == ChangeKind::Unknown));
}
#[test]
fn directory_changes_are_not_recursive_delivery_candidates() {
    let t = tempfile::tempdir().unwrap();
    let a = snap(t.path());
    fs::create_dir(t.path().join("new")).unwrap();
    let r = diff(&a, &snap(t.path()));
    assert_eq!(r.rows[0].kind, ChangeKind::Added);
    assert!(!r.rows[0].actionable);
}
#[test]
fn exclusions_and_shallow_scope() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir(t.path().join("ignore")).unwrap();
    fs::write(t.path().join("ignore/a"), "a").unwrap();
    let mut s = scope(t.path());
    s.recursive = false;
    assert_eq!(
        capture(s.clone(), &CancellationToken::new(), |_| {})
            .unwrap()
            .entries
            .len(),
        1
    );
    s.recursive = true;
    s.exclusions.push("ignore".into());
    assert!(capture(s, &CancellationToken::new(), |_| {})
        .unwrap()
        .entries
        .is_empty());
}
#[test]
fn own_document_is_excluded_and_scope_is_immutable() {
    let t = tempfile::tempdir().unwrap();
    let mut s = scope(t.path());
    let path = t.path().join("baseline.naygopoint");
    s.exclude_document(&path).unwrap();
    let a = capture(s.clone(), &CancellationToken::new(), |_| {}).unwrap();
    fs::write(&path, a.to_bytes().unwrap()).unwrap();
    let b = capture(s, &CancellationToken::new(), |_| {}).unwrap();
    assert!(diff(&a, &b).rows.is_empty());
}
#[test]
fn incompatible_scopes_and_roots_rejected() {
    let t = tempfile::tempdir().unwrap();
    let a = snap(t.path());
    let mut b = a.clone();
    b.scope.hashed = true;
    assert!(matches!(
        compare(&a, &b, &CancellationToken::new()),
        Err(SpaceError::Changed)
    ));
    b = a.clone();
    b.resolved_root = Some(t.path().join("other"));
    assert!(matches!(
        compare(&a, &b, &CancellationToken::new()),
        Err(SpaceError::Changed)
    ));
}
#[test]
fn hashes_verify_content_and_detect_same_metadata_edits() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("a"), "abc").unwrap();
    let mut s = scope(t.path());
    s.hashed = true;
    let a = capture(s.clone(), &CancellationToken::new(), |_| {}).unwrap();
    assert_eq!(
        a.entries[0].sha256.as_deref(),
        Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );
    assert_eq!(diff(&a, &a).counts[ChangeKind::SameContent as usize], 1);
    fs::write(t.path().join("a"), "xyz").unwrap();
    let mut b = capture(s, &CancellationToken::new(), |_| {}).unwrap();
    b.entries[0].modified_ns = a.entries[0].modified_ns;
    assert_eq!(diff(&a, &b).rows[0].kind, ChangeKind::Modified);
}
#[test]
fn unavailable_hash_or_timestamp_is_unknown_not_equal() {
    let t = tempfile::tempdir().unwrap();
    let mut a = snap(t.path());
    a.entries.push(file("a"));
    let mut b = a.clone();
    b.entries[0].modified_ns = None;
    assert_eq!(diff(&a, &b).rows[0].kind, ChangeKind::Unknown);
    a.scope.hashed = true;
    b = a.clone();
    assert_eq!(diff(&a, &b).rows[0].kind, ChangeKind::Unknown);
}
#[test]
fn cancelled_capture_read_and_compare_do_not_complete() {
    let t = tempfile::tempdir().unwrap();
    let p = snap(t.path());
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        capture(scope(t.path()), &token, |_| {}),
        Err(SpaceError::Cancelled)
    ));
    assert!(matches!(
        read(&t.path().join("missing"), &token),
        Err(SpaceError::Cancelled)
    ));
    assert!(matches!(
        compare(&p, &p, &token),
        Err(SpaceError::Cancelled)
    ));
}
#[test]
fn cancellation_during_scan_discards_partial_point() {
    let t = tempfile::tempdir().unwrap();
    for n in 0..4 {
        fs::write(t.path().join(n.to_string()), "data").unwrap();
    }
    let token = CancellationToken::new();
    assert!(matches!(
        capture(scope(t.path()), &token, |_| token.cancel()),
        Err(SpaceError::Cancelled)
    ));
}
#[test]
fn invalid_documents_are_rejected() {
    let t = tempfile::tempdir().unwrap();
    let original = snap(t.path());
    for path in ["../escape", "a/../escape", "a/./b", "C:drive", "*.txt", ""] {
        let mut p = original.clone();
        p.entries.push(file(path));
        assert!(p.validate().is_err(), "{path}");
    }
    let mut p = original.clone();
    p.version = 999;
    assert!(matches!(p.validate(), Err(SpaceError::Version)));
    p = original.clone();
    p.entries = vec![file("a"), file("a")];
    assert!(p.validate().is_err());
    p = original;
    p.entries.push(file("a"));
    p.entries[0].sha256 = Some("invalid".into());
    assert!(p.validate().is_err());
}
#[test]
fn result_rows_bounded_but_counts_include_all_changes() {
    let t = tempfile::tempdir().unwrap();
    let a = snap(t.path());
    let mut b = a.clone();
    b.entries = (0..MAX_ROWS + 1)
        .map(|n| file(&format!("file-{n}")))
        .collect();
    let r = diff(&a, &b);
    assert_eq!(r.rows.len(), MAX_ROWS);
    assert!(r.rows_truncated);
    assert_eq!(r.counts[0], MAX_ROWS + 1);
}
#[test]
fn entry_and_path_budget_are_enforced() {
    let t = tempfile::tempdir().unwrap();
    let mut p = snap(t.path());
    p.entries = (0..=MAX_ENTRIES).map(|n| file(&n.to_string())).collect();
    assert!(matches!(p.validate(), Err(SpaceError::Limit)));
}

#[test]
fn exact_unknown_entry_is_not_actionable_even_with_metadata() {
    let t = tempfile::tempdir().unwrap();
    let a = snap(t.path());
    let mut b = a.clone();
    b.entries.push(file("ambiguous"));
    b.gaps.push("ambiguous".into());
    let report = diff(&a, &b);
    assert_eq!(report.rows[0].kind, ChangeKind::Unknown);
    assert!(!report.rows[0].actionable);
    assert_eq!(diff(&b, &b).rows[0].kind, ChangeKind::Unknown);
}

#[test]
fn trailing_separators_in_exclusions_and_duplicate_paths_are_normalized() {
    let t = tempfile::tempdir().unwrap();
    let mut s = scope(t.path());
    s.exclusions.push("tmp/".into());
    assert!(!s.includes(Path::new("tmp/file")));
    assert!(!s.includes(Path::new("tmp")));
    let mut p = snap(t.path());
    p.entries = vec![file("a/b"), file("a//b")];
    assert!(p.validate().is_err());
}
