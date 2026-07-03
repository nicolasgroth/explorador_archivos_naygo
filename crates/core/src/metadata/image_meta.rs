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
