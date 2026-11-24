// Naygo — benchmark reproducible del rasterizador STL/3MF.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use naygo_core::mesh_preview::{
    load_scene, render_scene_with_appearance, MeshAppearance, MeshCamera, MeshColor, MeshSurface,
};
use naygo_core::CancellationToken;
use std::io::Write;
use std::time::{Duration, Instant};

const GRID: usize = 220;
const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;
const ROUNDS: usize = 7;

fn vertex(x: usize, y: usize) -> [f32; 3] {
    let x = x as f32 / GRID as f32;
    let y = y as f32 / GRID as f32;
    [
        x * 160.0,
        y * 160.0,
        ((x * 17.0).sin() * (y * 13.0).cos()) * 18.0,
    ]
}

fn write_triangle(file: &mut impl Write, points: [[f32; 3]; 3]) {
    for value in [0.0f32, 0.0, 1.0]
        .into_iter()
        .chain(points.into_iter().flatten())
    {
        file.write_all(&value.to_le_bytes()).unwrap();
    }
    file.write_all(&[0, 0]).unwrap();
}

fn create_fixture() -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("naygo-mesh-preview-bench-")
        .tempdir()
        .expect("crear fixture temporal");
    let path = dir.path().join("surface.stl");
    let mut file = std::fs::File::create(path).expect("crear STL sintético");
    file.write_all(&[0; 80]).unwrap();
    let triangle_count = (GRID * GRID * 2) as u32;
    file.write_all(&triangle_count.to_le_bytes()).unwrap();
    for y in 0..GRID {
        for x in 0..GRID {
            let a = vertex(x, y);
            let b = vertex(x + 1, y);
            let c = vertex(x, y + 1);
            let d = vertex(x + 1, y + 1);
            write_triangle(&mut file, [a, b, c]);
            write_triangle(&mut file, [b, d, c]);
        }
    }
    dir
}

fn create_colored_fixture() -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("naygo-colored-3mf-bench-")
        .tempdir()
        .expect("crear fixture temporal");
    let file = std::fs::File::create(dir.path().join("colored.3mf")).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
        .unwrap();
    {
        let mut xml = std::io::BufWriter::new(&mut zip);
        write!(xml, r##"<model><resources><basematerials id="2"><base name="red" displaycolor="#D46060"/><base name="blue" displaycolor="#6080D4"/></basematerials><object id="1" pid="2" pindex="0"><mesh><vertices>"##).unwrap();
        for y in 0..=GRID {
            for x in 0..=GRID {
                let [x, y, z] = vertex(x, y);
                write!(xml, "<vertex x=\"{x}\" y=\"{y}\" z=\"{z}\"/>").unwrap();
            }
        }
        write!(xml, "</vertices><triangles>").unwrap();
        for y in 0..GRID {
            for x in 0..GRID {
                let a = y * (GRID + 1) + x;
                let b = a + 1;
                let c = a + GRID + 1;
                let d = c + 1;
                let p = (x + y) % 2;
                write!(xml, "<triangle v1=\"{a}\" v2=\"{b}\" v3=\"{c}\" p1=\"{p}\"/><triangle v1=\"{b}\" v2=\"{d}\" v3=\"{c}\" p1=\"{p}\"/>").unwrap();
            }
        }
        write!(
            xml,
            "</triangles></mesh></object></resources><build><item objectid=\"1\"/></build></model>"
        )
        .unwrap();
        xml.flush().unwrap();
    }
    zip.finish().unwrap();
    dir
}

fn measure_material_colors() {
    let fixture = create_colored_fixture();
    let scene = load_scene(
        &fixture.path().join("colored.3mf"),
        &CancellationToken::new(),
    )
    .expect("leer fixture 3MF");
    let colors = [MeshColor::Blue, MeshColor::Source];
    let mut times = [Vec::with_capacity(ROUNDS), Vec::with_capacity(ROUNDS)];
    println!("colored fixture triangles: {}", scene.info.triangle_count);
    let render = |color| {
        let image = render_scene_with_appearance(
            &scene,
            WIDTH,
            HEIGHT,
            MeshCamera::default(),
            MeshAppearance {
                color,
                surface: MeshSurface::Solid,
            },
            &CancellationToken::new(),
        )
        .expect("rasterizar fixture 3MF");
        std::hint::black_box(image.rgba);
    };
    for color in colors {
        render(color);
    }
    for round in 0..ROUNDS {
        for offset in 0..colors.len() {
            let index = (round + offset) % colors.len();
            let start = Instant::now();
            render(colors[index]);
            times[index].push(start.elapsed());
        }
    }
    for (color, samples) in colors.into_iter().zip(times.iter_mut()) {
        samples.sort_unstable();
        let median = samples[samples.len() / 2].as_secs_f64() * 1000.0;
        let all_ms: Vec<_> = samples.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
        println!(
            "3MF color {color:?}: {WIDTH}x{HEIGHT}, median {median:.3} ms, sorted_ms={all_ms:?}"
        );
    }
}

fn main() {
    let fixture = create_fixture();
    let scene = load_scene(
        &fixture.path().join("surface.stl"),
        &CancellationToken::new(),
    )
    .expect("leer fixture STL");
    let surfaces = [
        MeshSurface::Solid,
        MeshSurface::Edges,
        MeshSurface::Wireframe,
    ];
    let mut times = [
        Vec::with_capacity(ROUNDS),
        Vec::with_capacity(ROUNDS),
        Vec::with_capacity(ROUNDS),
    ];
    println!("fixture triangles: {}", scene.info.triangle_count);
    // Warm each path once; then rotate their order to distribute drift across modes.
    for surface in surfaces {
        let image = render_scene_with_appearance(
            &scene,
            WIDTH,
            HEIGHT,
            MeshCamera::default(),
            MeshAppearance {
                surface,
                ..Default::default()
            },
            &CancellationToken::new(),
        )
        .expect("calentar rasterizador");
        std::hint::black_box(image.rgba);
    }
    for round in 0..ROUNDS {
        for offset in 0..surfaces.len() {
            let index = (round + offset) % surfaces.len();
            let start = Instant::now();
            let image = render_scene_with_appearance(
                &scene,
                WIDTH,
                HEIGHT,
                MeshCamera::default(),
                MeshAppearance {
                    surface: surfaces[index],
                    ..Default::default()
                },
                &CancellationToken::new(),
            )
            .expect("rasterizar fixture");
            std::hint::black_box(image.rgba);
            times[index].push(start.elapsed());
        }
    }
    for (surface, samples) in surfaces.into_iter().zip(times.iter_mut()) {
        samples.sort_unstable();
        let median: Duration = samples[samples.len() / 2];
        let all_ms = samples
            .iter()
            .map(|duration| duration.as_secs_f64() * 1000.0)
            .collect::<Vec<_>>();
        println!(
            "mesh preview {surface:?}: {WIDTH}x{HEIGHT}, median {:.3} ms, sorted_ms={all_ms:?}",
            median.as_secs_f64() * 1000.0
        );
    }
    measure_material_colors();
}
