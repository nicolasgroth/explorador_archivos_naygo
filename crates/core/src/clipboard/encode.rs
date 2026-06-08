// Naygo — codificación de imagen del portapapeles a PNG/JPG (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Codifica una `ClipboardImage` (RGBA8) a bytes PNG o JPG en memoria, vía el crate
//! `image`. Sin Windows. PNG es sin pérdida; JPG usa `jpg_quality` (1..=100).

use super::{ClipboardImage, ImageFmt};
use std::io::Cursor;

/// Error al codificar una imagen.
#[derive(Debug)]
pub enum EncodeError {
    /// La longitud de `rgba` no coincide con `width * height * 4`.
    BadBuffer,
    /// El codificador del crate `image` falló.
    Encode(String),
}

/// Codifica `img` (RGBA8) a bytes PNG o JPG. `jpg_quality` (1..=100) solo aplica a JPG.
pub fn encode_image(
    img: &ClipboardImage,
    fmt: ImageFmt,
    jpg_quality: u8,
) -> Result<Vec<u8>, EncodeError> {
    let expected = img.width as usize * img.height as usize * 4;
    if img.rgba.len() != expected {
        return Err(EncodeError::BadBuffer);
    }
    let buf = image::RgbaImage::from_raw(img.width, img.height, img.rgba.clone())
        .ok_or(EncodeError::BadBuffer)?;
    let mut out = Cursor::new(Vec::new());
    match fmt {
        ImageFmt::Png => {
            image::DynamicImage::ImageRgba8(buf)
                .write_to(&mut out, image::ImageFormat::Png)
                .map_err(|e| EncodeError::Encode(e.to_string()))?;
        }
        ImageFmt::Jpg => {
            let rgb = image::DynamicImage::ImageRgba8(buf).to_rgb8();
            let q = jpg_quality.clamp(1, 100);
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q);
            enc.encode_image(&rgb)
                .map_err(|e| EncodeError::Encode(e.to_string()))?;
        }
    }
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(w: u32, h: u32) -> ClipboardImage {
        let rgba = (0..(w * h)).flat_map(|_| [255u8, 0, 0, 255]).collect();
        ClipboardImage { width: w, height: h, rgba }
    }

    #[test]
    fn png_round_trip_conserva_dimensiones() {
        let img = sample(4, 3);
        let bytes = encode_image(&img, ImageFmt::Png, 90).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (4, 3));
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255]);
    }

    #[test]
    fn jpg_round_trip_conserva_dimensiones() {
        let img = sample(8, 8);
        let bytes = encode_image(&img, ImageFmt::Jpg, 85).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.to_rgba8().dimensions(), (8, 8));
    }

    #[test]
    fn error_si_rgba_inconsistente() {
        let bad = ClipboardImage { width: 2, height: 2, rgba: vec![0, 0, 0] };
        assert!(encode_image(&bad, ImageFmt::Png, 90).is_err());
    }
}
