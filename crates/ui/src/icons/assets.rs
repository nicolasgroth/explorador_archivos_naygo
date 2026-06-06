// Naygo — tabla de assets embebidos: (IconSet, IconKey) → bytes PNG.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Embebe los PNG de los tres sets con `include_bytes!`. `bytes_for` da los bytes
//! del ícono para un set y una clave; si la clave no tiene asset propio, cae a
//! `unknown` (genérico), que siempre existe. Función pura y testeable.

// `bytes_for`/`all_keys` se consumen en Tarea 5 (vía `IconProvider`); hoy solo los
// usan los tests. naygo-ui es binario y `pub` no suprime el dead_code, así que se
// permite explícitamente para no romper `clippy -D warnings`.
#![allow(dead_code)]

use naygo_core::config::IconSet;
use naygo_core::icon_kind::{FileCategory, IconKey};

/// Nombre de archivo (sin extensión) para una `IconKey`. Debe coincidir con lo que
/// generó `gen_icons.rs`.
fn file_name(key: IconKey) -> &'static str {
    match key {
        IconKey::Folder => "folder",
        IconKey::ParentDir => "parent",
        IconKey::Drive(_) => "drive",
        IconKey::Unknown => "unknown",
        IconKey::File(cat) => match cat {
            FileCategory::Image => "file_image",
            FileCategory::Video => "file_video",
            FileCategory::Audio => "file_audio",
            FileCategory::Document => "file_document",
            FileCategory::Code => "file_code",
            FileCategory::Archive => "file_archive",
            FileCategory::Executable => "file_executable",
            FileCategory::Model3D => "file_model3d",
            FileCategory::Font => "file_font",
            FileCategory::Generic => "file_generic",
        },
    }
}

/// Macro: embebe el PNG de un set+nombre. La ruta es relativa a este archivo
/// fuente (crates/ui/src/icons/) → sube a la raíz del repo a assets/icons/.
macro_rules! png {
    ($set:literal, $name:literal) => {
        include_bytes!(concat!(
            "../../../../assets/icons/",
            $set,
            "/",
            $name,
            ".png"
        )) as &'static [u8]
    };
}

/// Lista de los 14 nombres con sus bytes embebidos, por set.
macro_rules! set_table {
    ($konst:ident, $set:literal) => {
        const $konst: &[(&str, &[u8])] = &[
            ("folder", png!($set, "folder")),
            ("parent", png!($set, "parent")),
            ("file_image", png!($set, "file_image")),
            ("file_video", png!($set, "file_video")),
            ("file_audio", png!($set, "file_audio")),
            ("file_document", png!($set, "file_document")),
            ("file_code", png!($set, "file_code")),
            ("file_archive", png!($set, "file_archive")),
            ("file_executable", png!($set, "file_executable")),
            ("file_model3d", png!($set, "file_model3d")),
            ("file_font", png!($set, "file_font")),
            ("file_generic", png!($set, "file_generic")),
            ("drive", png!($set, "drive")),
            ("unknown", png!($set, "unknown")),
        ];
    };
}

set_table!(FLAT, "flat");
set_table!(FLUENT, "fluent");
set_table!(MONO, "mono");

/// Tabla de bytes para un set.
fn table_for(set: IconSet) -> &'static [(&'static str, &'static [u8])] {
    match set {
        IconSet::Flat => FLAT,
        IconSet::Fluent => FLUENT,
        IconSet::Mono => MONO,
    }
}

/// Bytes PNG del ícono para `set`+`key`; cae a `unknown` si no hay asset (no falla).
pub fn bytes_for(set: IconSet, key: IconKey) -> &'static [u8] {
    let name = file_name(key);
    let table = table_for(set);
    table
        .iter()
        .find(|(n, _)| *n == name)
        .or_else(|| table.iter().find(|(n, _)| *n == "unknown"))
        .map(|(_, b)| *b)
        .unwrap_or(&[])
}

/// Todas las claves que la app pinta (para precargar el atlas).
pub fn all_keys() -> Vec<IconKey> {
    use FileCategory::*;
    let mut v = vec![
        IconKey::Folder,
        IconKey::ParentDir,
        IconKey::Drive(naygo_core::icon_kind::DriveKind::Unknown),
        IconKey::Unknown,
    ];
    for cat in [
        Image, Video, Audio, Document, Code, Archive, Executable, Model3D, Font,
        Generic,
    ] {
        v.push(IconKey::File(cat));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_clave_tiene_bytes_no_vacios() {
        for set in [IconSet::Flat, IconSet::Fluent, IconSet::Mono] {
            for key in all_keys() {
                assert!(
                    !bytes_for(set, key).is_empty(),
                    "asset vacío para {:?}/{:?}",
                    set,
                    key
                );
            }
        }
    }

    #[test]
    fn clave_sin_asset_cae_a_unknown() {
        let b = bytes_for(
            IconSet::Flat,
            IconKey::Drive(naygo_core::icon_kind::DriveKind::Fixed),
        );
        assert!(!b.is_empty());
    }
}
