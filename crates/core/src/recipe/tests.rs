// Naygo — contratos de recetas portables, revisión y publicación segura.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
fn draft() -> Recipe {
    Recipe::from_query(&SavedSearch {
        version: 1,
        name: "PDFs".into(),
        roots: vec![],
        options: SearchOptions {
            name_query: "*.pdf".into(),
            use_wildcards: true,
            recursive: true,
            ..Default::default()
        },
        min_bytes: None,
        max_bytes: None,
        modified: RelativeDate::Any,
    })
}
fn params(dir: &Path) -> Parameters {
    Parameters {
        roots: vec![dir.into()],
        parent: dir.into(),
    }
}
fn time() -> EvaluationTime {
    EvaluationTime {
        now: i64::MAX / 2,
        lower: None,
        date: "2026-09-05".into(),
    }
}
fn review(r: Recipe, p: Parameters, files: Vec<PathBuf>) -> Result<Prepared, RecipeError> {
    prepare(r, p, files, time(), &CancellationToken::new(), |_| {})
}
#[test]
fn document_is_portable_versioned_and_not_a_script() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recipe.naygorecipe");
    let r = draft();
    let bytes = r.to_bytes().unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value.get("roots").is_none());
    assert!(value.get("parent").is_none());
    assert!(value.get("sources").is_none());
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(read(&path, &CancellationToken::new()).unwrap().0, r);
    value["shell"] = "cmd.exe".into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(
        read(&path, &CancellationToken::new()),
        Err(SpaceError::Invalid)
    ));
    let mut r = r;
    r.version = 2;
    assert!(matches!(r.validate(), Err(SpaceError::Version)));
}
#[test]
fn output_template_cannot_escape_destination_or_interpret_tokens() {
    let mut r = draft();
    assert_eq!(
        r.resolved_name("2026-09-05").unwrap(),
        "delivery-2026-09-05"
    );
    r.output = Output::Zip;
    assert_eq!(
        r.resolved_name("2026-09-05").unwrap(),
        "delivery-2026-09-05.zip"
    );
    for name in [
        "../escape",
        "..\\escape",
        "C:\\escape",
        "{shell}",
        "bad/name",
        "NUL",
        "end.",
    ] {
        r.output = Output::Folder;
        r.output_name = name.into();
        assert!(r.validate().is_err(), "{name}");
    }
}
#[test]
fn validation_rejects_limits_and_contradictory_filters() {
    let mut r = draft();
    r.min_bytes = Some(20);
    r.max_bytes = Some(10);
    assert!(r.validate().is_err());
    r.max_bytes = None;
    r.content_query = "x".repeat(4097);
    assert!(r.validate().is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large");
    std::fs::write(&path, vec![b' '; MAX_BYTES + 1]).unwrap();
    assert!(matches!(
        read(&path, &CancellationToken::new()),
        Err(SpaceError::Limit)
    ));
}
#[test]
fn query_plan_is_read_only_and_frozen_against_new_matches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.pdf"), b"first").unwrap();
    std::fs::write(dir.path().join("b.txt"), b"skip").unwrap();
    let prepared = review(draft(), params(dir.path()), vec![]).unwrap();
    assert!(!prepared.plan.destination.exists());
    assert_eq!(prepared.sources.len(), 1);
    std::fs::write(dir.path().join("new.pdf"), b"later").unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    let out = delivery::execute(&prepared.plan, &CancellationToken::new(), &tx).unwrap();
    assert_eq!(std::fs::read(out.join("a.pdf")).unwrap(), b"first");
    assert!(!out.join("new.pdf").exists());
    assert!(out.join("naygo-manifest.json").exists());
    assert!(dir.path().join("a.pdf").exists());
    assert!(review(draft(), params(dir.path()), vec![]).is_err());
    assert!(delivery::execute(&prepared.plan, &CancellationToken::new(), &tx).is_err());
}
#[test]
fn changed_sources_fail_without_publishing() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("a.pdf");
    std::fs::write(&source, b"old").unwrap();
    let mut r = draft();
    r.hashes = true;
    let prepared = review(r, params(dir.path()), vec![]).unwrap();
    std::fs::write(&source, b"new and longer").unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    assert!(matches!(
        delivery::execute(&prepared.plan, &CancellationToken::new(), &tx),
        Err(delivery::DeliveryError::Changed(_))
    ));
    assert!(!prepared.plan.destination.exists());
}
#[test]
fn missing_roots_are_partial_not_a_smaller_success() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.pdf"), b"yes").unwrap();
    let mut p = params(dir.path());
    p.roots.push(dir.path().join("missing"));
    assert!(matches!(
        review(draft(), p, vec![]),
        Err(RecipeError::Partial)
    ));
    assert!(!dir.path().join("delivery-2026-09-05").exists());
}
#[test]
fn selection_filters_size_content_and_deduplicates() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    std::fs::write(&a, b"Wanted text").unwrap();
    std::fs::write(&b, b"no").unwrap();
    let mut r = draft();
    r.source = Source::Selection;
    r.min_bytes = Some(5);
    r.content_query = "wanted".into();
    r.ignore_case = true;
    let prepared = review(r, params(dir.path()), vec![a.clone(), a.clone(), b]).unwrap();
    assert_eq!(prepared.sources, vec![a]);
    assert_eq!(prepared.examined, 3);
}
#[test]
fn selection_directories_and_incomplete_content_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = draft();
    r.source = Source::Selection;
    assert!(matches!(
        review(r.clone(), params(dir.path()), vec![dir.path().into()]),
        Err(RecipeError::SelectionFilesOnly)
    ));
    let source = dir.path().join("large.pdf");
    let file = std::fs::File::create(&source).unwrap();
    file.set_len(crate::search::MAX_CONTENT_FILE_BYTES + 1)
        .unwrap();
    drop(file);
    r.content_query = "needle".into();
    assert!(matches!(
        review(r, params(dir.path()), vec![source]),
        Err(RecipeError::Partial)
    ));
}
#[test]
fn duplicate_output_names_require_explicit_numbering() {
    let dir = tempfile::tempdir().unwrap();
    for n in ["one", "two"] {
        std::fs::create_dir(dir.path().join(n)).unwrap();
        std::fs::write(dir.path().join(n).join("a.pdf"), b"data").unwrap();
    }
    assert!(matches!(
        review(draft(), params(dir.path()), vec![]),
        Err(RecipeError::Delivery(delivery::DeliveryError::Conflicts(_)))
    ));
    let mut r = draft();
    r.number_duplicates = true;
    let p = review(r, params(dir.path()), vec![]).unwrap();
    assert_eq!(p.sources.len(), 2);
    assert_ne!(p.plan.entries[0].relative, p.plan.entries[1].relative);
}
#[test]
fn parameters_and_date_bound_must_be_valid() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = params(dir.path());
    p.parent = "relative".into();
    assert!(matches!(
        review(draft(), p, vec![]),
        Err(RecipeError::Document(SpaceError::Invalid))
    ));
    let mut r = draft();
    r.modified = RelativeDate::ThisMonth;
    assert!(matches!(
        review(r, params(dir.path()), vec![]),
        Err(RecipeError::Document(SpaceError::Invalid))
    ));
}
#[test]
fn cancelled_preparation_and_execution_never_publish() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.pdf"), b"data").unwrap();
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        prepare(draft(), params(dir.path()), vec![], time(), &token, |_| {}),
        Err(RecipeError::Document(SpaceError::Cancelled))
    ));
    let prepared = review(draft(), params(dir.path()), vec![]).unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    assert!(matches!(
        delivery::execute(&prepared.plan, &token, &tx),
        Err(delivery::DeliveryError::Cancelled)
    ));
    assert!(!prepared.plan.destination.exists());
}
#[test]
fn empty_query_does_not_create_delivery() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        review(draft(), params(dir.path()), vec![]),
        Err(RecipeError::Empty)
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
#[test]
fn month_filter_zip_and_relative_layout_reuse_reviewed_inventory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("source")).unwrap();
    let source = dir.path().join("source/a.pdf");
    std::fs::write(&source, b"pdf fixture").unwrap();
    let modified = std::fs::metadata(&source)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mut r = draft();
    r.modified = RelativeDate::ThisMonth;
    r.output = Output::Zip;
    r.layout = Layout::Relative;
    r.hashes = true;
    let prepared = prepare(
        r,
        params(dir.path()),
        vec![],
        EvaluationTime {
            now: modified + 1,
            lower: Some(modified - 1),
            date: "2026-09-05".into(),
        },
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    let out = delivery::execute(&prepared.plan, &CancellationToken::new(), &tx).unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(out).unwrap()).unwrap();
    assert!(zip.by_name("source/a.pdf").is_ok());
    let manifest: serde_json::Value =
        serde_json::from_reader(zip.by_name("naygo-manifest.json").unwrap()).unwrap();
    assert!(manifest["entries"][0]["sha256"].is_string());
    assert!(source.exists());
}
