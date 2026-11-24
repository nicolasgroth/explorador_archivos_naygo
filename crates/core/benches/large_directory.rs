// Naygo — benchmark reproducible del modelo de carpetas grandes.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use naygo_core::columns::ColumnKind;
use naygo_core::fs_model::{Entry, EntryKind, SortKey};
use naygo_core::listing_changes::{highlight_paths, ChangeKind, ListingChange};
use naygo_core::workspace::FilePaneState;
use naygo_core::{spawn_listing, CancellationToken, ListingOutcome};
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

fn entries(count: usize) -> Vec<Entry> {
    (0..count)
        .rev()
        .map(|i| Entry {
            name: format!("informe-proyecto-{i:06}.dat"),
            path: PathBuf::from(format!(r"C:\benchmark\informe-proyecto-{i:06}.dat")),
            kind: EntryKind::File,
            size: Some((i as u64 + 1) * 137),
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(i as u64)),
            created: None,
            hidden: false,
            system: false,
        })
        .collect()
}

fn measure(label: &str, rounds: usize, structured: bool, mut run: impl FnMut()) {
    run(); // calentamiento: inicializa cachés y evita medir el primer acceso.
    let mut samples = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let start = Instant::now();
        run();
        samples.push(start.elapsed().as_secs_f64() * 1_000.0);
    }
    if structured {
        let mut ordered = samples.clone();
        ordered.sort_by(f64::total_cmp);
        let middle = ordered.len() / 2;
        let median_ms = if ordered.len() % 2 == 0 {
            (ordered[middle - 1] + ordered[middle]) / 2.0
        } else {
            ordered[middle]
        };
        let p95_ms = (rounds >= 20).then(|| ordered[(rounds * 95).div_ceil(100) - 1]);
        let mean_ms = samples.iter().sum::<f64>() / samples.len() as f64;
        println!(
            "{}",
            serde_json::json!({
                "type": "measurement",
                "label": label,
                "rounds": rounds,
                "samples_ms": samples,
                "mean_ms": mean_ms,
                "median_ms": median_ms,
                "p95_ms": p95_ms,
                "rustc": std::env::var("NAYGO_BENCH_RUSTC_VERSION").ok(),
                "os": std::env::consts::OS,
                "os_version": std::env::var("NAYGO_BENCH_OS_VERSION").ok(),
                "arch": std::env::consts::ARCH,
                "cpu": std::env::var("NAYGO_BENCH_CPU").ok(),
                "parallelism": std::thread::available_parallelism().ok().map(usize::from),
                "naygo_version": env!("CARGO_PKG_VERSION"),
            })
        );
    } else {
        let total_ms = samples.iter().sum::<f64>();
        println!(
            "{label:<30} total={total_ms:>9.6} ms  promedio={:>9.6} ms",
            total_ms / rounds as f64
        );
    }
}

fn scenario(count: usize, structured: bool) {
    let mut pane = FilePaneState::new(PathBuf::from(r"C:\benchmark"));
    pane.entries = entries(count);
    if !structured {
        println!("\nDirectorio sintético: {count} entradas");
    }
    measure(
        &format!("{count} filas: vista inicial + sort nombre"),
        3,
        structured,
        || {
            // Simula un lote nuevo del listing: invalida el caché aunque la cantidad sea idéntica.
            pane.entries_changed();
            pane.sort.key = SortKey::Name;
            black_box(pane.view_indices());
        },
    );
    measure(
        &format!("{count} filas: vista cacheada"),
        100,
        structured,
        || {
            black_box(pane.view_indices());
        },
    );
    measure(
        &format!("{count} filas: vista compartida cacheada"),
        100,
        structured,
        || {
            black_box(pane.view_indices_shared());
        },
    );
    measure(
        &format!("{count} filas: sort por tamaño"),
        3,
        structured,
        || {
            pane.sort.key = if pane.sort.key == SortKey::Size {
                SortKey::Modified
            } else {
                SortKey::Size
            };
            black_box(pane.view_indices());
        },
    );
    pane.table.set_filter(
        ColumnKind::Name,
        naygo_core::filter::ColumnFilter::Text {
            contains: "proyecto-0001".into(),
            case_sensitive: false,
        },
    );
    measure(
        &format!("{count} filas: filtro + sort"),
        3,
        structured,
        || {
            pane.sort.ascending = !pane.sort.ascending;
            black_box(pane.view_indices());
        },
    );
}

fn highlight_scenario(
    count: usize,
    changed_every: usize,
    include_baseline: bool,
    structured: bool,
) {
    let rows = entries(count);
    let changes: Vec<_> = rows
        .iter()
        .step_by(changed_every)
        .map(|entry| ListingChange {
            path: entry.path.clone(),
            kind: ChangeKind::Modified,
        })
        .collect();
    if !structured {
        println!(
            "\nResaltado por visita: {count} filas, {} modificadas",
            changes.len()
        );
    }
    let expected = changes.len();
    if include_baseline {
        measure(
            &format!("{count} filas: búsqueda previa por nombre"),
            3,
            structured,
            || {
                let matches = rows
                    .iter()
                    .filter(|entry| {
                        changes.iter().any(|change| {
                            change.path.file_name().and_then(|name| name.to_str())
                                == Some(entry.name.as_str())
                        })
                    })
                    .count();
                assert_eq!(matches, expected);
                black_box(matches);
            },
        );
    }
    measure(
        &format!("{count} filas: índice por ruta completa"),
        10,
        structured,
        || {
            let paths = highlight_paths(&changes);
            let matches = rows
                .iter()
                .filter(|entry| paths.contains(&entry.path))
                .count();
            assert_eq!(matches, expected);
            black_box(matches);
        },
    );
}

fn create_filesystem_fixture(root: &Path, count: usize) -> PathBuf {
    let directory = root.join(format!("files-{count}"));
    std::fs::create_dir(&directory).expect("crear directorio del benchmark de disco");
    for index in 0..count {
        let path = directory.join(format!("entry-{index:06}.dat"));
        std::fs::File::create(path).expect("crear archivo del benchmark de disco");
    }
    directory
}

fn measure_filesystem_listing(directory: &Path, expected_entries: usize) {
    let started = Instant::now();
    let (receiver, worker) = spawn_listing(directory.to_path_buf(), CancellationToken::new());
    let mut entries_seen = 0usize;
    let mut first_entry_ms = None;
    let mut first_batch_ms = None;
    let terminal = loop {
        match receiver.try_recv_entry() {
            Ok(_entry) => {
                entries_seen += 1;
                if first_entry_ms.is_none() {
                    first_entry_ms = Some(started.elapsed().as_secs_f64() * 1_000.0);
                }
                if entries_seen == 1_024 || entries_seen == expected_entries {
                    first_batch_ms = Some(started.elapsed().as_secs_f64() * 1_000.0);
                }
            }
            Err(TryRecvError::Empty) => match receiver.try_recv_terminal() {
                Ok(outcome) => break outcome,
                Err(TryRecvError::Empty) => thread::sleep(Duration::from_millis(1)),
                Err(TryRecvError::Disconnected) => {
                    panic!("el worker cerró el canal terminal sin resultado")
                }
            },
            Err(TryRecvError::Disconnected) => match receiver.try_recv_terminal() {
                Ok(outcome) => break outcome,
                Err(_) => panic!("el worker cerró ambos canales sin resultado terminal"),
            },
        }
    };
    worker
        .join()
        .expect("el worker de listado debe terminar sin panic");
    let total_ms = started.elapsed().as_secs_f64() * 1_000.0;
    assert_eq!(
        entries_seen, expected_entries,
        "cantidad de entradas listadas"
    );
    assert_eq!(
        terminal,
        ListingOutcome::Complete,
        "resultado terminal del listado"
    );
    let first_entry_ms = first_entry_ms.expect("debe llegar la primera entrada");
    let first_batch_ms = first_batch_ms.expect("debe completarse el primer lote");

    if std::env::args().any(|arg| arg == "--json") {
        println!(
            "{}",
            serde_json::json!({
                "type": "filesystem_listing",
                "entries": entries_seen,
                "first_entry_ms": first_entry_ms,
                "first_batch_1024_ms": first_batch_ms,
                "total_ms": total_ms,
                "os_version": std::env::var("NAYGO_BENCH_OS_VERSION").ok(),
                "cpu": std::env::var("NAYGO_BENCH_CPU").ok(),
                "rustc": std::env::var("NAYGO_BENCH_RUSTC_VERSION").ok(),
                "naygo_version": env!("CARGO_PKG_VERSION"),
            })
        );
    } else {
        println!(
            "Listado de disco: {entries_seen} entradas, primera={first_entry_ms:.3} ms, lote 1024={first_batch_ms:.3} ms, total={total_ms:.3} ms"
        );
    }
    black_box(entries_seen);
}

fn measure_filesystem_cancellation(directory: &Path, expected_entries: usize) {
    let token = CancellationToken::new();
    let (receiver, worker) = spawn_listing(directory.to_path_buf(), token.clone());
    let mut entries_seen = 0usize;
    while entries_seen < 1_024 {
        match receiver.try_recv_entry() {
            Ok(_entry) => entries_seen += 1,
            Err(TryRecvError::Empty) => thread::sleep(Duration::from_millis(1)),
            Err(TryRecvError::Disconnected) => {
                panic!("el listado terminó antes de acumular 1.024 entradas")
            }
        }
    }

    let cancel_started = Instant::now();
    token.cancel();
    worker
        .join()
        .expect("el worker cancelado debe terminar sin panic");
    let cancel_latency_ms = cancel_started.elapsed().as_secs_f64() * 1_000.0;

    while receiver.try_recv_entry().is_ok() {}
    assert_eq!(
        receiver.try_recv_terminal(),
        Ok(ListingOutcome::Cancelled),
        "el worker debe informar cancelación"
    );
    assert!(
        expected_entries > entries_seen,
        "el fixture debe conservar filas pendientes para cancelar"
    );

    if std::env::args().any(|arg| arg == "--json") {
        println!(
            "{}",
            serde_json::json!({
                "type": "filesystem_listing_cancellation",
                "entries": expected_entries,
                "cancel_after_entries": entries_seen,
                "cancel_latency_ms": cancel_latency_ms,
                "terminal": "cancelled",
                "os_version": std::env::var("NAYGO_BENCH_OS_VERSION").ok(),
                "cpu": std::env::var("NAYGO_BENCH_CPU").ok(),
                "rustc": std::env::var("NAYGO_BENCH_RUSTC_VERSION").ok(),
                "naygo_version": env!("CARGO_PKG_VERSION"),
            })
        );
    } else {
        println!(
            "Cancelación local: {entries_seen}/{expected_entries} entradas consumidas, worker terminó en {cancel_latency_ms:.3} ms"
        );
    }
}

fn filesystem_scenario() {
    let fixture = tempfile::Builder::new()
        .prefix("naygo-w07-listing-")
        .tempdir()
        .expect("crear fixture temporal del benchmark");
    let root = fixture.path();
    assert!(
        !root.to_string_lossy().starts_with(r"\\"),
        "el benchmark local de filesystem requiere un temporal en una ruta local"
    );
    for count in [10_000, 100_000] {
        let directory = create_filesystem_fixture(root, count);
        for _ in 0..3 {
            measure_filesystem_listing(&directory, count);
        }
        for _ in 0..3 {
            measure_filesystem_cancellation(&directory, count);
        }
    }
}

fn main() {
    let structured = std::env::args().any(|arg| arg == "--json");
    let include_filesystem = std::env::args().any(|arg| arg == "--filesystem");
    if structured {
        println!(
            "{}",
            serde_json::json!({
                "type": "environment",
                "rustc": std::env::var("NAYGO_BENCH_RUSTC_VERSION").ok(),
                "os": std::env::consts::OS,
                "os_version": std::env::var("NAYGO_BENCH_OS_VERSION").ok(),
                "arch": std::env::consts::ARCH,
                "cpu": std::env::var("NAYGO_BENCH_CPU").ok(),
                "ram_bytes": std::env::var("NAYGO_BENCH_RAM_BYTES")
                    .ok()
                    .and_then(|value| value.parse::<u64>().ok()),
                "parallelism": std::thread::available_parallelism().ok().map(usize::from),
                "naygo_version": env!("CARGO_PKG_VERSION"),
                "dataset": [10_000, 100_000],
            })
        );
    } else {
        println!("Naygo benchmark de navegación (release, renderer independiente)");
    }
    scenario(10_000, structured);
    scenario(100_000, structured);
    highlight_scenario(10_000, 10, true, structured);
    highlight_scenario(100_000, 10, false, structured);
    if include_filesystem {
        filesystem_scenario();
    }
}
