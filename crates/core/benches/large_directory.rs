// Naygo — benchmark reproducible del modelo de carpetas grandes.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use naygo_core::columns::ColumnKind;
use naygo_core::fs_model::{Entry, EntryKind, SortKey};
use naygo_core::workspace::FilePaneState;
use std::hint::black_box;
use std::path::PathBuf;
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

fn measure(label: &str, rounds: usize, mut run: impl FnMut()) {
    run(); // calentamiento: inicializa cachés y evita medir el primer acceso.
    let start = Instant::now();
    for _ in 0..rounds {
        run();
    }
    let elapsed = start.elapsed();
    println!(
        "{label:<30} total={:>9.3} ms  promedio={:>9.3} ms",
        elapsed.as_secs_f64() * 1_000.0,
        elapsed.as_secs_f64() * 1_000.0 / rounds as f64
    );
}

fn scenario(count: usize) {
    let mut pane = FilePaneState::new(PathBuf::from(r"C:\benchmark"));
    pane.entries = entries(count);
    println!("\nDirectorio sintético: {count} entradas");
    measure("vista inicial + sort nombre", 3, || {
        // Simula un lote nuevo del listing: invalida el caché aunque la cantidad sea idéntica.
        pane.entries_changed();
        pane.sort.key = SortKey::Name;
        black_box(pane.view_indices());
    });
    measure("vista cacheada", 100, || {
        black_box(pane.view_indices());
    });
    measure("sort por tamaño", 3, || {
        pane.sort.key = if pane.sort.key == SortKey::Size {
            SortKey::Modified
        } else {
            SortKey::Size
        };
        black_box(pane.view_indices());
    });
    pane.table.set_filter(
        ColumnKind::Name,
        naygo_core::filter::ColumnFilter::Text {
            contains: "proyecto-0001".into(),
            case_sensitive: false,
        },
    );
    measure("filtro + sort", 3, || {
        pane.sort.ascending = !pane.sort.ascending;
        black_box(pane.view_indices());
    });
}

fn main() {
    println!("Naygo benchmark de navegación (release, renderer independiente)");
    scenario(10_000);
    scenario(100_000);
}
