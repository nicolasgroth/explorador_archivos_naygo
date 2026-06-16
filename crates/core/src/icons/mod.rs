// Naygo — tabla de assets embebidos: (IconSet, IconKey) → bytes PNG. Pura, reusable por toda UI.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Embebe los PNG de los tres sets con `include_bytes!`. `bytes_for` da los bytes
//! del ícono para un set y una clave; si la clave no tiene asset propio, cae a
//! `unknown` (genérico), que siempre existe. Función pura y testeable. Vive en
//! `naygo-core` para que tanto la UI Slint como la egui la compartan.

use crate::config::IconSet;
use crate::icon_kind::{ActionIcon, FileCategory, IconKey};

/// Nombre de archivo (sin extensión) para una `IconKey`. Debe coincidir con lo que
/// generó `gen_icons.rs`.
pub fn file_name(key: IconKey) -> &'static str {
    match key {
        IconKey::Folder => "folder",
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
        IconKey::Action(a) => a.file_name(),
    }
}

/// Macro: embebe el PNG de un set+nombre. La ruta es relativa a este archivo
/// fuente (crates/core/src/icons/) → sube a la raíz del repo a assets/icons/.
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

/// Lista de los nombres con sus bytes embebidos, por set (13 base + acciones de barra).
macro_rules! set_table {
    ($konst:ident, $set:literal) => {
        const $konst: &[(&str, &[u8])] = &[
            ("folder", png!($set, "folder")),
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
            ("action_back", png!($set, "action_back")),
            ("action_forward", png!($set, "action_forward")),
            ("action_up", png!($set, "action_up")),
            ("action_refresh", png!($set, "action_refresh")),
            ("action_copy", png!($set, "action_copy")),
            ("action_cut", png!($set, "action_cut")),
            ("action_paste", png!($set, "action_paste")),
            ("action_delete", png!($set, "action_delete")),
            ("action_new_file", png!($set, "action_new_file")),
            ("action_new_folder", png!($set, "action_new_folder")),
            ("action_add_pane", png!($set, "action_add_pane")),
            ("action_swap_panes", png!($set, "action_swap_panes")),
            ("action_clone_path", png!($set, "action_clone_path")),
            ("action_new_window", png!($set, "action_new_window")),
            ("action_settings", png!($set, "action_settings")),
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
        IconKey::Drive(crate::icon_kind::DriveKind::Unknown),
        IconKey::Unknown,
    ];
    for cat in [
        Image, Video, Audio, Document, Code, Archive, Executable, Model3D, Font, Generic,
    ] {
        v.push(IconKey::File(cat));
    }
    for a in ActionIcon::all() {
        v.push(IconKey::Action(*a));
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
            IconKey::Drive(crate::icon_kind::DriveKind::Fixed),
        );
        assert!(!b.is_empty());
    }

    #[test]
    fn cada_clave_tiene_su_propio_asset_no_solo_el_fallback() {
        // Más estricto que el test anterior: verifica que el NOMBRE EXACTO de cada
        // clave está en la tabla (no que caiga al fallback "unknown"). Así un PNG
        // faltante para una clave concreta sí se detecta.
        for set in [IconSet::Flat, IconSet::Fluent, IconSet::Mono] {
            let table = table_for(set);
            for key in all_keys() {
                let name = file_name(key);
                assert!(
                    table.iter().any(|(n, _)| *n == name),
                    "falta el asset propio '{name}' para {:?}/{:?}",
                    set,
                    key
                );
            }
        }
    }
}
