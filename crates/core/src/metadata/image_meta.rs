// Naygo — proveedor de metadata para imágenes (dimensiones y tipo de color).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::{MetadataField, MetadataProvider};
use std::path::Path;

/// Metadata de imágenes usando la crate `image`. Lee dimensiones y tipo de color leyendo solo
/// la cabecera cuando es posible (barato). Vacío si el archivo no es una imagen legible.
pub struct ImageMeta;

impl MetadataProvider for ImageMeta {
    fn extensions(&self) -> &'static [&'static str] {
        &["png", "jpg", "jpeg", "gif", "bmp", "webp", "ico"]
    }

    fn read(&self, path: &Path) -> Vec<MetadataField> {
        // `image::image_dimensions` lee solo lo necesario para el tamaño (no decodifica todo).
        let Ok((w, h)) = image::image_dimensions(path) else {
            return Vec::new();
        };
        vec![MetadataField {
            label_key: "meta.dimensions",
            value: format!("{w} × {h}"),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensiones_declaradas() {
        let exts = ImageMeta.extensions();
        for e in ["png", "jpg", "jpeg", "gif", "bmp", "webp", "ico"] {
            assert!(exts.contains(&e), "falta la extensión {e}");
        }
    }

    #[test]
    fn png_valido_reporta_dimensiones() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.png");
        image::RgbaImage::new(7, 3).save(&p).unwrap();
        let fields = ImageMeta.read(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.dimensions" && f.value == "7 × 3"),
            "debe reportar dimensiones 7 × 3: {fields:?}"
        );
    }

    #[test]
    fn archivo_inexistente_da_vacio() {
        let fields = ImageMeta.read(std::path::Path::new(r"C:\no\existe.png"));
        assert!(fields.is_empty());
    }

    #[test]
    fn contenido_basura_da_vacio_sin_panic() {
        // Extensión de imagen pero contenido que no es imagen: no debe paniquear.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("roto.png");
        std::fs::write(&p, b"esto no es una imagen, solo texto").unwrap();
        assert!(ImageMeta.read(&p).is_empty());
    }

    #[test]
    fn png_truncado_da_vacio_sin_panic() {
        // Un PNG válido cortado a la mitad: cabecera reconocible pero datos incompletos.
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("ok.png");
        image::RgbaImage::new(8, 8).save(&ok).unwrap();
        let bytes = std::fs::read(&ok).unwrap();
        let p = dir.path().join("truncado.png");
        std::fs::write(&p, &bytes[..bytes.len() / 2]).unwrap();
        // No panic; el resultado puede ser vacío (lo normal) pero nunca una caída.
        let _ = ImageMeta.read(&p);
    }

    #[test]
    fn archivo_vacio_da_vacio_sin_panic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("vacio.jpg");
        std::fs::write(&p, b"").unwrap();
        assert!(ImageMeta.read(&p).is_empty());
    }
}
