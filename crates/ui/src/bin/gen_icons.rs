// Naygo — generador de los PNG iniciales de íconos (set propio, reemplazable).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
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
        ("action_settings", [90, 120, 170, 255]),
    ]
}

fn main() {
    for set in ["flat", "fluent", "mono"] {
        let dir = Path::new("assets/icons").join(set);
        std::fs::create_dir_all(&dir).expect("crear dir");
        let mono = set == "mono";
        for (name, color) in icon_specs() {
            let path = dir.join(format!("{name}.png"));
            make_icon(&path, color, mono);
        }
        println!("generado set: {set}");
    }
    println!("listo. {} íconos x 3 sets.", icon_specs().len());
}
