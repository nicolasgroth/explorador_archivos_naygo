// Naygo — pegado inteligente: decidir qué hacer con el portapapeles del SO (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA del pegado inteligente. `platform` lee el portapapeles del SO y lo
//! normaliza a `ClipboardContent`; aquí `decide_paste` decide la acción (`PastePlan`)
//! y `encode_image` codifica una imagen del portapapeles a PNG/JPG. Sin Windows ni egui.

pub mod encode;
pub mod naming;

use crate::clipboard::naming::expand_name_template;
use crate::config::Settings;
use crate::ops::names::dedup_name;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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

/// Decide la acción de pegado a partir del contenido del portapapeles + config.
/// Puro: `exists` consulta si una ruta ya existe (en producción, el FS; en tests, un
/// closure). `now_secs` alimenta la expansión de `{fecha}` en los nombres.
pub fn decide_paste(
    content: &ClipboardContent,
    dest_dir: &Path,
    settings: &Settings,
    now_secs: u64,
    exists: &dyn Fn(&Path) -> bool,
) -> PastePlan {
    match content {
        ClipboardContent::Files { paths, cut } => {
            if paths.is_empty() {
                return PastePlan::Nothing;
            }
            PastePlan::Transfer {
                paths: paths.clone(),
                cut: *cut,
            }
        }
        ClipboardContent::Text(body) => {
            let stem = expand_name_template(&settings.paste_text_name, now_secs);
            let name = format!("{stem}.{}", settings.paste_text_ext);
            let path = dedup_name(&dest_dir.join(name), exists);
            PastePlan::CreateText {
                path,
                body: body.clone(),
            }
        }
        ClipboardContent::Image(img) => {
            let pixels = img.width as u64 * img.height as u64;
            let ok = img.width > 0
                && img.height > 0
                && pixels <= MAX_IMAGE_PIXELS
                && img.rgba.len() as u64 == pixels * 4;
            if !ok {
                return PastePlan::Nothing;
            }
            let fmt = settings.paste_image_fmt;
            let stem = expand_name_template(&settings.paste_image_name, now_secs);
            let name = format!("{stem}.{}", fmt.ext());
            let path = dedup_name(&dest_dir.join(name), exists);
            PastePlan::CreateImage {
                path,
                fmt,
                img: img.clone(),
            }
        }
        ClipboardContent::Empty => PastePlan::Nothing,
    }
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

    use crate::config::Settings;
    use std::collections::HashSet;
    use std::path::Path;

    fn settings() -> Settings {
        Settings::default()
    }

    fn none_exists(_: &Path) -> bool {
        false
    }

    #[test]
    fn files_a_transfer() {
        let c = ClipboardContent::Files {
            paths: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")],
            cut: true,
        };
        let plan = decide_paste(&c, Path::new("D:/dst"), &settings(), 0, &none_exists);
        assert_eq!(
            plan,
            PastePlan::Transfer {
                paths: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")],
                cut: true
            }
        );
    }

    #[test]
    fn files_vacio_a_nothing() {
        let c = ClipboardContent::Files {
            paths: vec![],
            cut: false,
        };
        assert_eq!(
            decide_paste(&c, Path::new("D:/dst"), &settings(), 0, &none_exists),
            PastePlan::Nothing
        );
    }

    #[test]
    fn text_a_create_text_con_nombre_plantilla() {
        let c = ClipboardContent::Text("hola".into());
        let plan = decide_paste(&c, Path::new("D:/dst"), &settings(), 0, &none_exists);
        match plan {
            PastePlan::CreateText { path, body } => {
                assert_eq!(body, "hola");
                assert_eq!(path, Path::new("D:/dst/pegado 1970-01-01 00-00.txt"));
            }
            other => panic!("esperaba CreateText, vino {other:?}"),
        }
    }

    #[test]
    fn text_dedup_si_existe() {
        let taken: HashSet<PathBuf> = [PathBuf::from("D:/dst/pegado 1970-01-01 00-00.txt")]
            .into_iter()
            .collect();
        let exists = |p: &Path| taken.contains(p);
        let c = ClipboardContent::Text("x".into());
        let plan = decide_paste(&c, Path::new("D:/dst"), &settings(), 0, &exists);
        match plan {
            PastePlan::CreateText { path, .. } => {
                assert_eq!(path, Path::new("D:/dst/pegado 1970-01-01 00-00 (2).txt"));
            }
            other => panic!("esperaba CreateText dedup, vino {other:?}"),
        }
    }

    #[test]
    fn image_a_create_image() {
        let img = ClipboardImage {
            width: 2,
            height: 2,
            rgba: vec![0u8; 16],
        };
        let c = ClipboardContent::Image(img.clone());
        let plan = decide_paste(&c, Path::new("D:/dst"), &settings(), 0, &none_exists);
        match plan {
            PastePlan::CreateImage {
                path,
                fmt,
                img: got,
            } => {
                assert_eq!(fmt, ImageFmt::Png);
                assert_eq!(got, img);
                assert_eq!(path, Path::new("D:/dst/captura 1970-01-01 00-00.png"));
            }
            other => panic!("esperaba CreateImage, vino {other:?}"),
        }
    }

    #[test]
    fn image_dims_absurdas_a_nothing() {
        let bad = ClipboardImage {
            width: 2,
            height: 2,
            rgba: vec![0u8; 3],
        };
        assert_eq!(
            decide_paste(
                &ClipboardContent::Image(bad),
                Path::new("D:/dst"),
                &settings(),
                0,
                &none_exists
            ),
            PastePlan::Nothing
        );
        let zero = ClipboardImage {
            width: 0,
            height: 5,
            rgba: vec![],
        };
        assert_eq!(
            decide_paste(
                &ClipboardContent::Image(zero),
                Path::new("D:/dst"),
                &settings(),
                0,
                &none_exists
            ),
            PastePlan::Nothing
        );
    }

    #[test]
    fn empty_a_nothing() {
        assert_eq!(
            decide_paste(
                &ClipboardContent::Empty,
                Path::new("D:/dst"),
                &settings(),
                0,
                &none_exists
            ),
            PastePlan::Nothing
        );
    }
}
