// Naygo — proceso real de preview sin UI, configuración ni sesión personal.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
#![cfg(windows)]

use naygo_core::{preview_wire::Response, CancellationToken};
use naygo_platform::preview_process;
use std::{path::Path, time::Instant};
#[path = "fixtures/animated_images.rs"]
mod animated_images;
#[path = "fixtures/blocked_pipe.rs"]
mod blocked_pipe;

fn request(path: &Path, kind: &str) -> Vec<u8> {
    request_with_text_limit(
        path,
        kind,
        naygo_core::preview::DEFAULT_TEXT_MAX_BYTES as usize,
    )
}

fn request_with_text_limit(path: &Path, kind: &str, text_max_bytes: usize) -> Vec<u8> {
    let messages: serde_json::Map<String, serde_json::Value> = [
        "reader_stopped",
        "not_previewable",
        "archive_bad",
        "read",
        "image_big",
        "cancelled",
        "decode",
        "svg_big",
        "svg_bad",
        "rasterize",
        "pdf_big",
        "pdf_pages",
        "pdf_no_text",
    ]
    .into_iter()
    .map(|key| (key.into(), key.into()))
    .collect();
    serde_json::to_vec(
        &serde_json::json!({"path":path, "kind":kind, "text_max_bytes":text_max_bytes, "messages":messages,
        "labels":["files", "folders", "uncompressed", "more", "and {n} more"]}),
    )
    .unwrap()
}
fn executable() -> std::path::PathBuf {
    // Only this test harness accepts an override to validate the actual release artifact.
    std::env::var_os("NAYGO_PREVIEW_EXE")
        .map(Into::into)
        .unwrap_or_else(|| Path::new(env!("CARGO_BIN_EXE_naygo")).to_owned())
}
fn decode(path: &Path, kind: &str) -> Response {
    let started = Instant::now();
    let output = preview_process::run(
        &executable(),
        request(path, kind),
        &CancellationToken::new(),
    )
    .unwrap();
    eprintln!(
        "reader {kind}: {} ms, peak committed {} bytes",
        started.elapsed().as_millis(),
        output.peak_committed_bytes
    );
    assert!(
        output.peak_committed_bytes > 0
            && output.peak_committed_bytes <= preview_process::MEMORY_LIMIT
    );
    output.response
}

fn decode_with_text_limit(path: &Path, kind: &str, text_max_bytes: usize) -> Response {
    preview_process::run(
        &executable(),
        request_with_text_limit(path, kind, text_max_bytes),
        &CancellationToken::new(),
    )
    .unwrap()
    .response
}

fn make_pdf(path: &Path) {
    use lopdf::{dictionary, Document, Stream};
    let mut doc = Document::with_version("1.5");
    let pages = doc.new_object_id();
    let font = doc.add_object(
        dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"},
    );
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf (Naygo probe) Tj ET".to_vec(),
    ));
    let page = doc.add_object(dictionary! {"Type" => "Page", "Parent" => pages,
    "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
    "Resources" => dictionary! {"Font" => dictionary! {"F1" => font}}, "Contents" => content});
    doc.objects.insert(
        pages,
        dictionary! {"Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1}.into(),
    );
    let catalog = doc.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages});
    doc.trailer.set("Root", catalog);
    doc.save(path).unwrap();
}

#[test]
fn isolated_text_highlighting_and_cached_mesh_rotation() {
    let dir = tempfile::tempdir().unwrap();
    let text = dir.path().join("á texto.txt");
    std::fs::write(
        &text,
        (0..300).map(|n| format!("línea {n}\n")).collect::<String>(),
    )
    .unwrap();
    assert!(
        matches!(decode(&text, "Text"), Response::Text { text, truncated: true, .. }
        if text.starts_with("línea 0\n") && text.lines().count() == 250)
    );
    let large_text = dir.path().join("large.txt");
    std::fs::write(&large_text, "x\n".repeat(400_000)).unwrap();
    assert!(matches!(
        decode_with_text_limit(&large_text, "Text", 512 * 1024),
        Response::Text {
            truncated: true,
            bytes_read: Some(bytes_read),
            ..
        } if bytes_read == 512 * 1024
    ));
    let xml = dir.path().join("alias.custom");
    std::fs::write(&xml, "<a>á中</a>\n<b/>").unwrap();
    let mut bytes: serde_json::Value = serde_json::from_slice(&request(&xml, "Text")).unwrap();
    bytes["lang"] = serde_json::to_value(naygo_core::preview::CodeLang::Xml).unwrap();
    let output = preview_process::run(
        &executable(),
        serde_json::to_vec(&bytes).unwrap(),
        &CancellationToken::new(),
    )
    .unwrap();
    let Response::StyledText {
        text,
        lines,
        truncated: false,
        ..
    } = output.response
    else {
        panic!("missing highlighting")
    };
    let reconstructed = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(text, reconstructed);
    assert_eq!(text, "<a>á中</a>\n<b/>");

    let stl = dir.path().join("triangle.stl");
    std::fs::write(&stl, "solid test\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid test\n").unwrap();
    assert!(matches!(
        decode(&stl, "Mesh"),
        Response::Image {
            width: 800,
            height: 600,
            ..
        }
    ));
    let Response::Scene(scene) = decode(&stl, "Scene") else {
        panic!("missing geometry")
    };
    assert_eq!(scene.info.triangle_count, 1);
    std::fs::remove_file(&stl).unwrap(); // Only our owned fixture: rotation must need no file.
    let mut camera = naygo_core::mesh_preview::MeshCamera::default();
    camera.orbit(45., 0.);
    assert!(naygo_core::mesh_preview::render_scene(
        &scene,
        800,
        600,
        camera,
        &CancellationToken::new()
    )
    .is_ok());
    assert!(matches!(decode(&stl, "Scene"), Response::Message(_)));

    let model = dir.path().join("triangle.3mf");
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&model).unwrap());
        zip.start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"<model unit="millimeter"><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources><build><item objectid="1"/></build></model>"#).unwrap();
        zip.finish().unwrap();
    }
    assert!(
        matches!(decode(&model, "Scene"), Response::Scene(scene) if scene.info.triangle_count == 1)
    );
    assert!(matches!(decode(&model, "Mesh"), Response::Image { .. }));
}

#[test]
fn isolated_3mf_scene_preserves_base_material_color() {
    use naygo_core::mesh_preview::{
        render_scene_with_appearance, MeshAppearance, MeshCamera, MeshColor, MeshSurface,
    };
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("colored.3mf");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    zip.start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br##"<model><resources><basematerials id="7"><base name="red" displaycolor="#FF0000"/></basematerials><object id="1" pid="7" pindex="0"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources><build><item objectid="1"/></build></model>"##).unwrap();
    zip.finish().unwrap();
    let Response::Scene(scene) = decode(&path, "Scene") else {
        panic!("missing geometry")
    };
    std::fs::remove_file(&path).unwrap();
    let preview = render_scene_with_appearance(
        &scene,
        80,
        80,
        MeshCamera::default(),
        MeshAppearance {
            color: MeshColor::Source,
            surface: MeshSurface::Solid,
        },
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(preview
        .rgba
        .chunks_exact(4)
        .any(|p| p[0] > 60 && p[1] == 0 && p[2] == 0));
}

#[test]
fn text_reader_cancel_and_timeout_while_actual_file_read_is_blocked() {
    use std::time::Duration;
    for cancel in [true, false] {
        let pipe = blocked_pipe::BlockedPipe::new();
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let bytes = request(&pipe.path(), "Text");
        let worker =
            std::thread::spawn(move || preview_process::run(&executable(), bytes, &worker_token));
        pipe.wait_connected(); // File::open succeeded, but the fixture sends no bytes.
        let started = Instant::now();
        if cancel {
            token.cancel();
        }
        let result = worker.join().unwrap();
        if cancel {
            assert!(matches!(result, Err(preview_process::Error::Cancelled)));
            assert!(started.elapsed() < Duration::from_secs(2));
        } else {
            assert!(matches!(result, Err(preview_process::Error::Timeout)));
            assert!(started.elapsed() < Duration::from_secs(7));
        }
        eprintln!(
            "blocked text cancel={cancel}: {} ms",
            started.elapsed().as_millis()
        );
    }
}

#[test]
fn isolated_images_svg_archive_and_corrupt_input() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("á image.png");
    image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]))
        .save(&png)
        .unwrap();
    assert_eq!(
        decode(&png, "Image"),
        Response::Image {
            width: 3,
            height: 2,
            rgba: [10, 20, 30, 255].repeat(6)
        }
    );
    let svg = dir.path().join("huge-canvas.svg");
    std::fs::write(&svg, br##"<svg xmlns="http://www.w3.org/2000/svg" width="10000000" height="5000000"><rect width="10000000" height="5000000" fill="#123456"/></svg>"##).unwrap();
    assert!(matches!(
        decode(&svg, "Svg"),
        Response::Image {
            width: 2048,
            height: 1024,
            ..
        }
    ));
    // Local resources referenced from SVG must not be read (embedded data remains supported).
    let external = dir.path().join("external.svg");
    std::fs::write(&external, format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="3" height="2"><image href="{}" width="3" height="2"/></svg>"#,
        png.to_string_lossy().replace('\\', "/"))).unwrap();
    assert!(
        matches!(decode(&external, "Svg"), Response::Image { rgba, .. } if rgba.iter().all(|byte| *byte == 0))
    );
    let zip = dir.path().join("sample.zip");
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&zip).unwrap());
    archive
        .start_file("hello.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    std::io::Write::write_all(&mut archive, b"hello").unwrap();
    archive.finish().unwrap();
    assert!(
        matches!(decode(&zip, "Archive"), Response::Text { text, .. } if text.contains("hello.txt"))
    );
    let pdf = dir.path().join("sample.pdf");
    make_pdf(&pdf);
    assert!(
        matches!(decode(&pdf, "Pdf"), Response::Text { text, .. } if text.contains("Naygo probe"))
    );
    std::fs::write(&png, b"not an image").unwrap();
    assert!(matches!(decode(&png, "Image"), Response::Message(_)));
    assert!(matches!(
        decode(&dir.path().join("missing.pdf"), "Pdf"),
        Response::Message(_)
    ));

    // Optional private acceptance sample: read-only, never copied or named in test output.
    if let Some(sample) = std::env::var_os("NAYGO_PREVIEW_SAMPLE") {
        for _ in 0..3 {
            assert!(matches!(
                decode(Path::new(&sample), "Image"),
                Response::Image {
                    width: 800,
                    height: 450,
                    ..
                }
            ));
        }
    }
}

#[test]
fn economical_policy_reaches_isolated_static_and_animation_readers() {
    use naygo_core::preview_animation_wire::Event;
    use std::{sync::mpsc, time::Duration};
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("large.png");
    image::RgbaImage::from_pixel(1600, 800, image::Rgba([20, 40, 60, 255]))
        .save(&png)
        .unwrap();
    let svg = dir.path().join("vector.svg");
    std::fs::write(&svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><rect width="80" height="40" fill="red"/></svg>"#).unwrap();
    for (path, kind) in [(&png, "Image"), (&svg, "Svg")] {
        let mut request: serde_json::Value = serde_json::from_slice(&request(path, kind)).unwrap();
        request["economical"] = true.into();
        let output = preview_process::run(
            &executable(),
            serde_json::to_vec(&request).unwrap(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert!(matches!(
            output.response,
            Response::Image {
                width: 1024,
                height: 512,
                ..
            }
        ));
    }
    let path = dir.path().join("animation.gif");
    {
        let file = std::fs::File::create(&path).unwrap();
        let mut encoder = image::codecs::gif::GifEncoder::new(file);
        for value in [0, 255] {
            encoder
                .encode_frame(image::Frame::from_parts(
                    image::RgbaImage::from_pixel(800, 400, image::Rgba([value, 0, 0, 255])),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(100, 1),
                ))
                .unwrap();
        }
    }
    let mut request: serde_json::Value = serde_json::from_slice(&request(&path, "Image")).unwrap();
    request["economical"] = true.into();
    let (credits, requests) = mpsc::sync_channel(1);
    let (tx, events) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        preview_process::run_animation(
            &executable(),
            serde_json::to_vec(&request).unwrap(),
            &CancellationToken::new(),
            requests,
            tx,
        )
    });
    for time in [0, 100_000] {
        credits.send(()).unwrap();
        assert!(
            matches!(events.recv_timeout(Duration::from_secs(7)).unwrap().unwrap(),
            Event::Frame { at_us, image: Response::Image { width: 512, height: 256, .. } } if at_us == time)
        );
    }
    credits.send(()).unwrap();
    assert!(matches!(
        events
            .recv_timeout(Duration::from_secs(7))
            .unwrap()
            .unwrap(),
        Event::End { at_us: 200_000 }
    ));
    worker.join().unwrap().unwrap();
}

#[test]
fn malformed_request_exits_without_initializing_application() {
    let result = preview_process::run(&executable(), b"{}".to_vec(), &CancellationToken::new());
    assert!(matches!(result, Err(preview_process::Error::Failed)));
}

fn animation_probe(path: &Path, frame_count: usize, natural_end: bool, pause: bool) {
    use naygo_core::preview_animation_wire::Event;
    use std::{sync::mpsc, time::Duration};
    let (credits, requests) = mpsc::sync_channel(1);
    let (tx, events) = mpsc::sync_channel(1);
    let token = CancellationToken::new();
    let worker_token = token.clone();
    let bytes = request(path, "Image");
    let started = Instant::now();
    let worker = std::thread::spawn(move || {
        preview_process::run_animation(&executable(), bytes, &worker_token, requests, tx)
    });
    let mut previous = None;
    let mut distinct = false;
    for index in 0..frame_count {
        credits.send(()).unwrap();
        let Event::Frame {
            at_us,
            image:
                Response::Image {
                    width,
                    height,
                    rgba,
                },
        } = events
            .recv_timeout(Duration::from_secs(7))
            .unwrap()
            .unwrap()
        else {
            panic!("expected animated frame");
        };
        assert_eq!(at_us, index as u64 * 100_000);
        assert!(width <= 768 && height <= 768);
        distinct |= previous.as_ref().is_some_and(|old| *old != rgba);
        previous = Some(rgba);
        if pause && index == 0 {
            // Pauses/valid delays longer than the old 5-s whole-process deadline must survive.
            assert!(matches!(
                events.recv_timeout(Duration::from_millis(5200)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ));
            assert!(!worker.is_finished());
        }
    }
    assert!(distinct, "animation must not repeat a static poster");
    if natural_end {
        credits.send(()).unwrap();
        assert!(matches!(
            events
                .recv_timeout(Duration::from_secs(7))
                .unwrap()
                .unwrap(),
            Event::End { .. }
        ));
        let peak = worker.join().unwrap().unwrap();
        assert!(peak > 0 && peak <= preview_process::MEMORY_LIMIT);
        eprintln!(
            "animation: {frame_count} frames, {} ms including pauses, peak {peak} bytes",
            started.elapsed().as_millis()
        );
    } else {
        let cancel_started = Instant::now();
        token.cancel();
        assert!(matches!(
            worker.join().unwrap(),
            Err(preview_process::Error::Cancelled)
        ));
        assert!(cancel_started.elapsed() < Duration::from_secs(2));
        eprintln!(
            "animation: {frame_count} frames, {} ms, cancelled in {} ms",
            started.elapsed().as_millis(),
            cancel_started.elapsed().as_millis()
        );
    }
}

#[test]
fn real_animation_readers_pause_finish_and_cancel_without_unbounded_queues() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes, count) in [
        ("sample.webp", animated_images::webp(1), 2),
        ("sample.apng", animated_images::apng(), 4),
        ("sample.gif", animated_images::gif(), 4),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        animation_probe(&path, count, true, name.ends_with("webp"));
    }
    let infinite = dir.path().join("infinite.webp");
    std::fs::write(&infinite, animated_images::webp(0)).unwrap();
    animation_probe(&infinite, 8, false, false);
    if let Some(sample) = std::env::var_os("NAYGO_PREVIEW_SAMPLE") {
        // The user's sample is read-only; not included in fixtures, logs, or repository.
        animation_probe(Path::new(&sample), 55, false, false);
    }
}

#[test]
fn animation_cancel_bypasses_full_output_and_static_has_no_residual_process() {
    use naygo_core::preview_animation_wire::Event;
    use std::{sync::mpsc, time::Duration};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("static.png");
    image::RgbaImage::from_pixel(2, 1, image::Rgba([0, 255, 0, 255]))
        .save(&path)
        .unwrap();
    let (credits, requests) = mpsc::sync_channel(1);
    let (tx, events) = mpsc::sync_channel(1);
    let bytes = request(&path, "Image");
    let worker = std::thread::spawn(move || {
        preview_process::run_animation(
            &executable(),
            bytes,
            &CancellationToken::new(),
            requests,
            tx,
        )
    });
    credits.send(()).unwrap();
    assert!(matches!(
        events
            .recv_timeout(Duration::from_secs(7))
            .unwrap()
            .unwrap(),
        Event::Static(Response::Image {
            width: 2,
            height: 1,
            ..
        })
    ));
    worker.join().unwrap().unwrap();
    assert!(credits.send(()).is_err());

    let path = dir.path().join("infinite.webp");
    std::fs::write(&path, animated_images::webp(0)).unwrap();
    let (credits, requests) = mpsc::sync_channel(1);
    let (tx, events) = mpsc::sync_channel(1);
    let bytes = request(&path, "Image");
    let token = CancellationToken::new();
    let worker_token = token.clone();
    let worker = std::thread::spawn(move || {
        preview_process::run_animation(&executable(), bytes, &worker_token, requests, tx)
    });
    credits.send(()).unwrap();
    events
        .recv_timeout(Duration::from_secs(7))
        .unwrap()
        .unwrap();
    credits.send(()).unwrap();
    // Leave the second response unread, simulating pause with a full output queue.
    std::thread::sleep(Duration::from_millis(250));
    let started = Instant::now();
    token.cancel();
    assert!(matches!(
        worker.join().unwrap(),
        Err(preview_process::Error::Cancelled)
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
}
