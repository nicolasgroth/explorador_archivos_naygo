// Naygo — generador de los PNG iniciales de íconos (set propio, reemplazable).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
// Autor: Nicolás Groth <ngroth@gmail.com>.
//
// Genera un PNG 32x32 por IconKey por set bajo assets/icons/{flat,fluent,mono}/.
// Los íconos son formas de color simples por categoría — placeholders de calidad
// suficiente para validar la infraestructura; se reemplazan luego por packs
// profesionales (ver assets/icons/README.md). Correr con:
//   cargo run -p naygo-ui --bin gen_icons

use image::{Rgba, RgbaImage};
use std::path::Path;

const SIZE: u32 = 32;

/// Dibuja un rectángulo relleno (con margen transparente) de un color y lo guarda.
fn make_icon(path: &Path, fill: [u8; 4], mono: bool) {
    let mut img = RgbaImage::new(SIZE, SIZE);
    let color = if mono {
        // En el set mono, usar un gris claro uniforme (se teñirá en la UI).
        [200, 200, 200, 255]
    } else {
        fill
    };
    for y in 0..SIZE {
        for x in 0..SIZE {
            let border = x < 3 || y < 3 || x >= SIZE - 3 || y >= SIZE - 3;
            let px = if border {
                Rgba([0, 0, 0, 0])
            } else {
                Rgba(color)
            };
            img.put_pixel(x, y, px);
        }
    }
    img.save(path).expect("guardar PNG");
}

/// Glifos dibujables para los íconos de acción nuevos (que no tienen arte propio).
#[derive(Clone, Copy)]
enum Glyph {
    Plus,
    Swap,
    Clone,
    NewWindow,
}

/// Dibuja un glifo simple (trazos) centrado sobre fondo transparente, para los íconos
/// de acción nuevos que no tienen arte propio (swap, clone, add_pane, new_window). En
/// el set mono usa gris claro (se tiñe en la UI).
fn make_glyph(path: &Path, color: [u8; 4], mono: bool, glyph: Glyph) {
    let mut img = RgbaImage::new(SIZE, SIZE);
    let c = if mono { [200, 200, 200, 255] } else { color };
    let put = |img: &mut RgbaImage, x: i32, y: i32| {
        if x >= 0 && y >= 0 && (x as u32) < SIZE && (y as u32) < SIZE {
            img.put_pixel(x as u32, y as u32, Rgba(c));
        }
    };
    // Trazo grueso: pinta un cuadrado 2x2 por punto para que se vea a 32px.
    let dot = |img: &mut RgbaImage, x: i32, y: i32| {
        for dy in 0..2 {
            for dx in 0..2 {
                put(img, x + dx, y + dy);
            }
        }
    };
    let hline = |img: &mut RgbaImage, x0: i32, x1: i32, y: i32| {
        for x in x0..=x1 {
            dot(img, x, y);
        }
    };
    let vline = |img: &mut RgbaImage, y0: i32, y1: i32, x: i32| {
        for y in y0..=y1 {
            dot(img, x, y);
        }
    };
    match glyph {
        Glyph::Plus => {
            hline(&mut img, 8, 22, 15);
            vline(&mut img, 8, 22, 15);
        }
        Glyph::Swap => {
            // Dos flechas horizontales opuestas (arriba →, abajo ←).
            hline(&mut img, 8, 22, 11);
            dot(&mut img, 20, 9);
            dot(&mut img, 22, 11);
            dot(&mut img, 20, 13);
            hline(&mut img, 8, 22, 20);
            dot(&mut img, 10, 18);
            dot(&mut img, 8, 20);
            dot(&mut img, 10, 22);
        }
        Glyph::Clone => {
            // Dos rectángulos solapados (copiar).
            hline(&mut img, 8, 18, 9);
            hline(&mut img, 8, 18, 19);
            vline(&mut img, 9, 19, 8);
            vline(&mut img, 9, 19, 18);
            hline(&mut img, 13, 23, 13);
            hline(&mut img, 13, 23, 23);
            vline(&mut img, 13, 23, 13);
            vline(&mut img, 13, 23, 23);
        }
        Glyph::NewWindow => {
            // Ventana con un "+" en la esquina superior derecha.
            hline(&mut img, 7, 21, 9);
            hline(&mut img, 7, 21, 21);
            vline(&mut img, 9, 21, 7);
            vline(&mut img, 9, 21, 21);
            hline(&mut img, 16, 24, 7);
            vline(&mut img, 3, 11, 20);
        }
    }
    img.save(path).expect("guardar PNG");
}

/// (nombre de archivo, color RGBA) por cada IconKey. El nombre debe coincidir con
/// el que `assets.rs` espera (Tarea 4).
fn icon_specs() -> Vec<(&'static str, [u8; 4])> {
    vec![
        ("folder", [255, 196, 0, 255]),
        ("file_image", [76, 175, 80, 255]),
        ("file_video", [156, 39, 176, 255]),
        ("file_audio", [233, 30, 99, 255]),
        ("file_document", [33, 150, 243, 255]),
        ("file_code", [0, 188, 212, 255]),
        ("file_archive", [121, 85, 72, 255]),
        ("file_executable", [96, 125, 139, 255]),
        ("file_model3d", [255, 87, 34, 255]),
        ("file_font", [63, 81, 181, 255]),
        ("file_generic", [158, 158, 158, 255]),
        ("drive", [69, 90, 100, 255]),
        ("unknown", [120, 120, 120, 255]),
        // Acciones de la barra de herramientas (placeholders azul grisáceo;
        // Tarea 8 los reemplaza por íconos reales).
        ("action_back", [90, 120, 170, 255]),
        ("action_forward", [90, 120, 170, 255]),
        ("action_up", [90, 120, 170, 255]),
        ("action_refresh", [90, 120, 170, 255]),
        ("action_copy", [90, 120, 170, 255]),
        ("action_cut", [90, 120, 170, 255]),
        ("action_paste", [90, 120, 170, 255]),
        ("action_delete", [90, 120, 170, 255]),
        ("action_new_file", [90, 120, 170, 255]),
        ("action_new_folder", [90, 120, 170, 255]),
        ("action_add_pane", [90, 120, 170, 255]),
        ("action_swap_panes", [90, 120, 170, 255]),
        ("action_clone_path", [90, 120, 170, 255]),
        ("action_settings", [90, 120, 170, 255]),
    ]
}

fn main() {
    // No-destructivo por defecto: solo genera los PNG que FALTAN (placeholders para
    // claves nuevas). Con `--force` regenera todos (peligroso: pisa íconos curados).
    // Esto evita repetir la regresión del lote 2, donde un run completo aplastó los
    // íconos reales con cuadrados de color.
    let force = std::env::args().any(|a| a == "--force");
    let mut created = 0usize;
    for set in ["flat", "fluent", "mono"] {
        let dir = Path::new("assets/icons").join(set);
        std::fs::create_dir_all(&dir).expect("crear dir");
        let mono = set == "mono";
        for (name, color) in icon_specs() {
            let path = dir.join(format!("{name}.png"));
            if path.exists() && !force {
                continue;
            }
            make_icon(&path, color, mono);
            created += 1;
        }
    }
    // Íconos de acción con glifo propio (reemplazan los placeholders cuadrados). Estos
    // SÍ se regeneran siempre: son los que se veían como cuadritos en la toolbar.
    for set in ["flat", "fluent", "mono"] {
        let dir = Path::new("assets/icons").join(set);
        let mono = set == "mono";
        let color = [90, 120, 170, 255];
        make_glyph(&dir.join("action_add_pane.png"), color, mono, Glyph::Plus);
        make_glyph(&dir.join("action_swap_panes.png"), color, mono, Glyph::Swap);
        make_glyph(
            &dir.join("action_clone_path.png"),
            color,
            mono,
            Glyph::Clone,
        );
        make_glyph(
            &dir.join("action_new_window.png"),
            color,
            mono,
            Glyph::NewWindow,
        );
    }

    if force {
        println!("--force: regenerados TODOS los íconos (placeholders).");
    } else {
        println!("generados {created} íconos faltantes (los existentes se respetaron).");
    }
}
