// Naygo — regresiones de ciclo de vida del preview, con canales y datos aislados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use std::sync::mpsc;

#[test]
fn a_b_a_drops_old_channel_and_cancels_its_token() {
    let mut state = PreviewState::new();
    let now = Instant::now();
    let a = PathBuf::from("a.txt");
    state.set_wanted(Some(a.clone()), now);
    let token = CancellationToken::new();
    state.token = Some(token.clone());
    let (tx, rx) = mpsc::channel();
    state.rx = Some(rx);
    state.set_wanted(Some(PathBuf::from("b.txt")), now);
    state.set_wanted(Some(a.clone()), now);
    assert!(token.is_cancelled());
    assert!(tx.send((a, Payload::Message("stale".into()))).is_err());
    assert!(state.last_view().is_none());
    assert!(state.should_start(now + PREVIEW_DEBOUNCE));
}

#[test]
fn disconnected_worker_is_terminal_until_explicit_retry() {
    let mut state = PreviewState::new();
    let now = Instant::now();
    state.set_wanted(Some("a.txt".into()), now);
    let (tx, rx) = mpsc::channel();
    state.rx = Some(rx);
    drop(tx);
    assert!(matches!(state.poll(), Some(Payload::Message(_))));
    assert!(!state.busy());
    assert!(!state.should_start(now + Duration::from_secs(60)));
    assert!(state.can_retry());
    assert!(state.retry(now));
    assert!(state.should_start(now + PREVIEW_DEBOUNCE));
}

#[test]
fn changing_text_limit_reloads_only_the_active_text_preview() {
    let now = Instant::now();
    let mut state = PreviewState::new();
    state.set_wanted(Some(PathBuf::from("a.txt")), now);
    state.set_text_max_bytes(512 * 1024);
    assert_eq!(state.wanted.as_deref(), Some(Path::new("a.txt")));
    assert!(state.should_start(Instant::now() + PREVIEW_DEBOUNCE));

    state.set_wanted(Some(PathBuf::from("photo.png")), now);
    state.set_text_max_bytes(1024 * 1024);
    assert_eq!(state.wanted.as_deref(), Some(Path::new("photo.png")));
}

#[test]
fn mesh_presentation_coalesces_during_load_and_resets_on_new_file() {
    let mut state = PreviewState::new();
    state.set_wanted(Some("first.stl".into()), Instant::now());
    state.visible = false;
    assert!(!state.cycle_mesh_color());
    assert!(!state.cycle_mesh_surface());
    state.visible = true;
    let (_tx, rx) = mpsc::channel();
    state.mesh_load_rx = Some(rx);
    assert!(state.cycle_mesh_color());
    assert!(state.cycle_mesh_surface());
    assert_eq!(state.mesh_appearance_indices(), (1, 1));
    assert!(state.mesh_dirty);
    assert!(state.mesh_load_rx.is_some());
    assert!(state.mesh_render_rx.is_none());
    assert!(state.cycle_mesh_color());
    assert_eq!(state.mesh_appearance_indices(), (2, 1));
    assert!(state.cycle_mesh_color());
    assert_eq!(state.mesh_appearance_indices(), (3, 1));
    assert!(state.cycle_mesh_color());
    assert_eq!(state.mesh_appearance_indices(), (0, 1));
    state.set_wanted(Some("second.stl".into()), Instant::now());
    assert_eq!(state.mesh_appearance_indices(), (0, 0));
}

#[test]
fn hidden_preview_cancels_work_and_resumes_only_current_target() {
    let mut state = PreviewState::new();
    let now = Instant::now();
    state.set_wanted(Some("a.txt".into()), now);
    let token = CancellationToken::new();
    state.token = Some(token.clone());
    state.last = state.cache_text("previous", false, Some(64), &None);
    state.set_visible(false, now);
    assert!(token.is_cancelled());
    assert!(state.last_view().is_none());
    state.set_wanted(Some("b.txt".into()), now);
    assert!(!state.busy());
    assert!(!state.should_start(now + PREVIEW_DEBOUNCE));
    state.set_visible(true, now);
    assert_eq!(state.wanted, Some(PathBuf::from("b.txt")));
    assert!(state.should_start(now + PREVIEW_DEBOUNCE));
    assert!(state.cancel());
    state.set_visible(false, now);
    state.set_visible(true, now);
    assert!(!state.should_start(now + PREVIEW_DEBOUNCE));
    assert!(state.can_retry());
}

#[test]
fn thumbnail_does_not_hide_cancel_and_mesh_gestures_are_coalesced() {
    let mut state = PreviewState::new();
    let now = Instant::now();
    state.set_wanted(Some("model.stl".into()), now);
    state.loaded = state.wanted.clone();
    state.last = state.cache_image(&[0; 4], 1, 1);
    let token = CancellationToken::new();
    state.mesh_token = Some(token.clone());
    let (_tx, rx) = mpsc::channel();
    state.mesh_load_rx = Some(rx);
    for _ in 0..100 {
        assert!(state.orbit_mesh(1., 1.));
    }
    assert_ne!(
        state.mesh_camera,
        naygo_core::mesh_preview::MeshCamera::default()
    );
    assert!(state.mesh_dirty);
    assert!(state.mesh_render_rx.is_none());
    assert!(state.mesh_load_rx.is_some());
    assert!(!state.loading());
    assert!(state.can_cancel(now));
    assert!(state.cancel());
    assert!(token.is_cancelled());
    assert!(!state.mesh_interactive());
    assert!(!state.orbit_mesh(1., 1.));
    assert!(!state.busy());
}

#[test]
fn capacity_wait_keeps_only_latest_target_and_is_cancellable() {
    let mut state = PreviewState::new();
    let mut exits = Vec::new();
    for _ in 0..workers::LIMIT {
        let (tx, rx) = mpsc::channel();
        exits.push(tx);
        drop(
            state
                .workers
                .spawn(move || {
                    let _ = rx.recv();
                })
                .unwrap(),
        );
    }
    let now = Instant::now();
    for index in 0..100 {
        state.set_wanted(Some(format!("missing-{index}.txt").into()), now);
        state.start();
        assert!(state.rx.is_none());
    }
    assert_eq!(state.wanted, Some(PathBuf::from("missing-99.txt")));
    assert!(state.can_cancel(now));
    assert!(state.cancel());
    for exit in exits {
        exit.send(()).unwrap();
    }
    assert!(!state.should_start(now + PREVIEW_DEBOUNCE));
}

#[test]
fn drop_cancels_both_tokens_without_waiting_for_workers() {
    let token = CancellationToken::new();
    let mesh = CancellationToken::new();
    let mut state = PreviewState::new();
    state.token = Some(token.clone());
    state.mesh_token = Some(mesh.clone());
    drop(state);
    assert!(token.is_cancelled());
    assert!(mesh.is_cancelled());
}

#[test]
fn economy_change_cancels_generation_preserves_rules_and_releases_cache() {
    let mut state = PreviewState::new();
    let now = Instant::now();
    state.set_wanted(Some("photo.png".into()), now);
    let rules = state.rules.clone();
    let token = CancellationToken::new();
    state.token = Some(token.clone());
    state.last = state.cache_image(&[0; 4], 1, 1);
    assert_eq!(state.retained.used(), 8);
    state.set_economical(true, now);
    assert!(token.is_cancelled());
    assert_eq!(state.wanted, Some("photo.png".into()));
    assert_eq!(state.rules, rules);
    assert_eq!(state.retained.used(), 0);
    assert!(state.quality.economical);
    assert!(state.should_start(now + PREVIEW_DEBOUNCE));
    state.last = state.cache_text("keep", false, Some(4), &None);
    state.set_economical(true, now);
    assert!(
        state.last.is_some(),
        "unchanged preference must not invalidate every tick"
    );
}

#[test]
fn shared_content_budget_rejects_before_ui_copy_and_releases_on_hide() {
    let mut state = PreviewState::new();
    state.retained = RetainedBudget::new(16);
    state.last = state.cache_image(&[0; 4], 1, 1);
    let image = state.last.as_ref().unwrap();
    let ViewCache::Image { image, .. } = image else {
        panic!("image expected")
    };
    let clone = image.clone();
    assert_eq!(clone, *image, "VM must share an existing image handle");
    let second = state.cache_image(&[0; 4], 1, 1).unwrap();
    assert_eq!(state.retained.used(), 16);
    assert!(state.cache_image(&[0; 4], 1, 1).is_none());
    assert!(state.cache_image(&[0; 3], 1, 1).is_none());
    assert!(state.cache_image(&[], u32::MAX, u32::MAX).is_none());
    assert!(state.cache_text("text", false, Some(4), &None).is_none());
    drop(second);
    state.set_visible(false, Instant::now());
    assert_eq!(state.retained.used(), 0);
}

#[test]
fn resource_failure_is_terminal_and_can_be_retried() {
    let mut state = PreviewState::new();
    let now = Instant::now();
    state.set_wanted(Some("image.png".into()), now);
    state.retained = RetainedBudget::new(1);
    let (tx, rx) = mpsc::channel();
    state.rx = Some(rx);
    tx.send((
        PathBuf::from("image.png"),
        Payload::Image {
            rgba: vec![0; 4],
            width: 1,
            height: 1,
        },
    ))
    .unwrap();
    assert!(matches!(state.poll(), Some(Payload::Message(_))));
    assert_eq!(state.retained.used(), 0);
    assert!(state.can_retry());
    assert!(!state.should_start(now + PREVIEW_DEBOUNCE));
    assert!(state.retry(now));
}

#[test]
fn retired_mesh_renderer_keeps_geometry_reservation_until_exit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("triangle.stl");
    std::fs::write(&path, "solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid t\n").unwrap();
    let scene = naygo_core::mesh_preview::load_scene(&path, &CancellationToken::new()).unwrap();
    let size = scene.retained_bytes().unwrap();
    assert!(size >= 48);
    let mut state = PreviewState::new();
    state.set_wanted(Some(path.clone()), Instant::now());
    let scene = Arc::new(RetainedScene {
        scene,
        _reservation: state.retained.reserve(size).unwrap(),
    });
    state.mesh_scene = Some((path, scene.clone()));
    let (exit, wait) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let retained = scene;
        wait.recv().unwrap();
        drop(retained);
    });
    state.cancel();
    assert_eq!(state.retained.used(), size);
    exit.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(state.retained.used(), 0);
}

#[test]
fn cancelled_request_skips_reading_even_nonexistent_file() {
    let token = CancellationToken::new();
    token.cancel();
    let messages = PreviewMessages::default();
    let state = PreviewState::new();
    let payload = build_payload(
        Path::new("nonexistent.zip"),
        &state.rules,
        true,
        &state.archive_labels,
        &messages,
        &token,
    );
    assert!(matches!(payload, Payload::Message(message) if message == messages.cancelled));
}
