// Naygo — pruebas de criterios y recorridos multiraíz acotados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use crate::saved_search::{RelativeDate, VERSION};
use crate::search::SearchOptions;

fn query(root: &Path) -> SavedSearch {
    SavedSearch {
        version: VERSION,
        name: "Audit".into(),
        roots: vec![root.into()],
        options: SearchOptions::default(),
        min_bytes: None,
        max_bytes: None,
        modified: RelativeDate::Any,
    }
}
fn messages(q: SavedSearch) -> Vec<SearchMsg> {
    let rx = spawn(q, 2_000_000_000, 0, CancellationToken::new());
    let mut out = vec![];
    loop {
        let m = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("worker timed out");
        let end = matches!(m, SearchMsg::Done { .. } | SearchMsg::Cancelled);
        out.push(m);
        if end {
            return out;
        }
    }
}
fn paths(messages: &[SearchMsg]) -> Vec<std::path::PathBuf> {
    messages
        .iter()
        .filter_map(|m| {
            if let SearchMsg::Hit(e) = m {
                Some(e.path.clone())
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn overlapping_roots_are_scanned_once_but_siblings_remain() {
    let dir = tempfile::tempdir().unwrap();
    let child = dir.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let sibling = tempfile::tempdir().unwrap();
    std::fs::write(child.join("report.txt"), "abc").unwrap();
    std::fs::write(sibling.path().join("report.txt"), "def").unwrap();
    let mut q = query(dir.path());
    q.roots
        .extend([child.clone(), sibling.path().into(), dir.path().into()]);
    q.options.name_query = ".txt".into();
    let m = messages(q.clone());
    assert_eq!(paths(&m).len(), 2);
    assert!(m.contains(&SearchMsg::Progress { dirs_scanned: 3 }));
    q.options.recursive = false;
    assert_eq!(q.normalized_roots().len(), 3);
    assert_eq!(paths(&messages(q)).len(), 2);
}
#[test]
#[cfg(unix)]
fn explicit_nested_symlink_root_is_not_lost_when_parent_does_not_follow_links() {
    let dir = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    std::fs::write(external.path().join("report.txt"), "report").unwrap();
    let link = dir.path().join("linked");
    std::os::unix::fs::symlink(external.path(), &link).unwrap();
    let mut q = query(dir.path());
    q.roots.push(link.clone());
    q.options.name_query = ".txt".into();
    assert_eq!(paths(&messages(q)), vec![link.join("report.txt")]);
}

#[test]
fn size_content_and_case_filters_intersect() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.XML"), "Needle").unwrap();
    std::fs::write(dir.path().join("b.xml"), "needle longer").unwrap();
    std::fs::write(dir.path().join("c.txt"), "Needle").unwrap();
    let mut q = query(dir.path());
    q.options.name_query = "*.xml".into();
    q.options.use_wildcards = true;
    q.options.content_query = "needle".into();
    q.min_bytes = Some(6);
    q.max_bytes = Some(6);
    assert_eq!(paths(&messages(q.clone())), vec![dir.path().join("a.XML")]);
    q.options.ignore_case = false;
    assert!(paths(&messages(q)).is_empty());
}
#[test]
fn missing_root_and_binary_content_are_reported_as_partial() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("binary"), [0u8, 2, 3]).unwrap();
    std::fs::write(dir.path().join("text"), "yes").unwrap();
    let mut q = query(dir.path());
    q.roots.push(dir.path().join("absent"));
    q.options.recursive = false;
    q.options.content_query = "yes".into();
    let m = messages(q);
    assert_eq!(paths(&m).len(), 1);
    assert!(m.contains(&SearchMsg::Coverage {
        unreadable: 1,
        content_skipped: 1,
        directory_cap: false
    }));
    assert!(m.contains(&SearchMsg::Done {
        partial: true,
        hit_cap: false
    }));
}
#[test]
fn cancelled_query_emits_no_hits_or_false_success() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a"), "a").unwrap();
    let token = CancellationToken::new();
    token.cancel();
    let rx = spawn(query(dir.path()), 2_000_000_000, 0, token);
    let m: Vec<_> = rx.iter().collect();
    assert!(paths(&m).is_empty());
    assert_eq!(m.last(), Some(&SearchMsg::Cancelled));
}
#[test]
fn hit_limit_is_global_across_roots() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..MAX_HITS + 2 {
        std::fs::write(dir.path().join(format!("file-{i}")), []).unwrap();
    }
    let m = messages(query(dir.path()));
    assert_eq!(paths(&m).len(), MAX_HITS);
    assert!(m.contains(&SearchMsg::Done {
        partial: false,
        hit_cap: true
    }));
}
#[test]
fn oversized_content_is_skipped_without_reading_it() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("huge");
    std::fs::File::create(&p)
        .unwrap()
        .set_len(MAX_CONTENT_FILE_BYTES + 1)
        .unwrap();
    let mut q = query(dir.path());
    q.options.content_query = "anything".into();
    let m = messages(q);
    assert!(paths(&m).is_empty());
    assert!(m.contains(&SearchMsg::Coverage {
        unreadable: 0,
        content_skipped: 1,
        directory_cap: false
    }));
}
#[test]
fn dates_are_evaluated_at_execution_and_boundaries_inclusive() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a");
    let file = std::fs::File::create(&p).unwrap();
    let now = 1_900_000_000;
    file.set_times(
        std::fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(now - 7 * 86_400)),
    )
    .unwrap();
    drop(file);
    let mut q = query(dir.path());
    q.modified = RelativeDate::Last7Days;
    let (tx, rx) = channel();
    run(
        &q,
        now as i64,
        q.modified.lower_bound(now as i64, 0),
        &CancellationToken::new(),
        &tx,
    );
    drop(tx);
    assert_eq!(paths(&rx.iter().collect::<Vec<_>>()).len(), 1);
    let (tx, rx) = channel();
    run(
        &q,
        now as i64 + 1,
        q.modified.lower_bound(now as i64 + 1, 0),
        &CancellationToken::new(),
        &tx,
    );
    drop(tx);
    assert!(paths(&rx.iter().collect::<Vec<_>>()).is_empty());
}
#[test]
fn saved_criteria_roundtrip_rejects_invalid_paths_sizes_versions_and_limits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("query.naygosearch");
    let mut q = query(dir.path());
    std::fs::write(&path, q.to_bytes().unwrap()).unwrap();
    assert_eq!(
        crate::saved_search::read(&path, &CancellationToken::new())
            .unwrap()
            .0,
        q
    );
    q.min_bytes = Some(2);
    q.max_bytes = Some(1);
    assert!(q.validate().is_err());
    q.min_bytes = None;
    q.max_bytes = None;
    q.roots = vec!["relative".into()];
    assert!(q.validate().is_err());
    q.roots = vec![dir.path().join("../escape")];
    assert!(q.validate().is_err());
    q.roots = vec![dir.path().into(); 65];
    assert!(q.validate().is_err());
    q.roots = vec![dir.path().into()];
    q.version += 1;
    assert!(q.validate().is_err());
    std::fs::write(&path, vec![b' '; crate::saved_search::MAX_BYTES + 1]).unwrap();
    assert!(crate::saved_search::read(&path, &CancellationToken::new()).is_err());
}
#[test]
fn local_fixed_offset_calendar_week_starts_monday() {
    // 1970-01-01 UTC is Thursday. UTC-4 local midnight = 04:00 UTC.
    assert_eq!(
        RelativeDate::ThisWeek.lower_bound(12 * 3600, -4 * 3600),
        Some(-3 * 86_400 + 4 * 3600)
    );
    assert_eq!(
        RelativeDate::Today.lower_bound(12 * 3600, -4 * 3600),
        Some(4 * 3600)
    );
}
