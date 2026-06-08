// Naygo — pegado inteligente: decidir qué hacer con el portapapeles del SO (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA del pegado inteligente. `platform` lee el portapapeles del SO y lo
//! normaliza a `ClipboardContent`; aquí `decide_paste` decide la acción (`PastePlan`)
//! y `encode_image` codifica una imagen del portapapeles a PNG/JPG. Sin Windows ni egui.

pub mod encode;
pub mod naming;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Límite de píxeles de una imagen del portapapeles aceptable (≈512 megapíxeles).
/// Evita asignar memoria absurda ante un DIB corrupto.
pub const MAX_IMAGE_PIXELS: u64 = 512 * 1024 * 1024;

/// Contenido del portapapeles del SO, ya leído por `platform` y normalizado.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipboardContent {
    /// Archivos (CF_HDROP). `cut` = el Preferred DropEffect es MOVE.
    Files { paths: Vec<PathBuf>, cut: bool },
    /// Texto plano (CF_UNICODETEXT).
    Text(String),
    /// Imagen (CF_DIB) ya pasada a RGBA8.
    Image(ClipboardImage),
    /// Nada usable en el portapapeles.
    Empty,
}

/// Imagen del portapapeles: RGBA8 sin comprimir + dimensiones.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipboardImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>, // longitud esperada = width * height * 4
}

/// Formato de salida al pegar una imagen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFmt {
    Png,
    Jpg,
}

impl ImageFmt {
    /// Extensión de archivo (sin punto).
    pub fn ext(self) -> &'static str {
        match self {
            ImageFmt::Png => "png",
            ImageFmt::Jpg => "jpg",
        }
    }
}

/// Qué hará el pegado, decidido a partir del contenido + config. Resultado puro.
#[derive(Clone, Debug, PartialEq)]
pub enum PastePlan {
    /// Transferencia de archivos → motor de ops-A.
    Transfer { paths: Vec<PathBuf>, cut: bool },
    /// Crear un archivo de texto con `body` en `path`.
    CreateText { path: PathBuf, body: String },
    /// Crear una imagen en `path` con el formato dado.
    CreateImage {
        path: PathBuf,
        fmt: ImageFmt,
        img: ClipboardImage,
    },
    /// Nada que pegar.
    Nothing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_fmt_ext() {
        assert_eq!(ImageFmt::Png.ext(), "png");
        assert_eq!(ImageFmt::Jpg.ext(), "jpg");
    }

    #[test]
    fn image_fmt_serde_round_trip() {
        let j = serde_json::to_string(&ImageFmt::Jpg).unwrap();
        let back: ImageFmt = serde_json::from_str(&j).unwrap();
        assert_eq!(back, ImageFmt::Jpg);
    }
}
