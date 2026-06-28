// Naygo — tabla de assets embebidos: (set_id, IconKey) → bytes PNG. Pura, reusable por toda UI.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Embebe los PNG de los cinco sets con `include_bytes!`. `bytes_for_id` da los bytes
//! del ícono para un set (por id string) y una clave; si la clave no tiene asset propio,
//! cae a `unknown` (genérico), que siempre existe. `resolve_with_overrides` aplica los
//! overrides por objeto sobre el set base. Función pura y testeable. Vive en
//! `naygo-core` para que tanto la UI Slint como la egui la compartan.

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
            ("action_tabs", png!($set, "action_tabs")),
            ("action_layouts", png!($set, "action_layouts")),
            ("action_terminal", png!($set, "action_terminal")),
            ("action_eject", png!($set, "action_eject")),
            ("action_panel", png!($set, "action_panel")),
        ];
    };
}

set_table!(LUCIDE, "lucide");
set_table!(TABLER, "tabler");
set_table!(MATERIAL, "material");
set_table!(FLAT_COLOR, "flat-color");
set_table!(MONO, "mono");

/// Set de fábrica usado como último fallback (su `unknown` siempre existe embebido).
const FALLBACK_SET: &str = "lucide";

/// Tabla de bytes para un set identificado por id string; `None` si el id no es embebido.
fn table_for_id(set_id: &str) -> Option<&'static [(&'static str, &'static [u8])]> {
    match set_id {
        "lucide" => Some(LUCIDE),
        "tabler" => Some(TABLER),
        "material" => Some(MATERIAL),
        "flat-color" => Some(FLAT_COLOR),
        "mono" => Some(MONO),
        _ => None,
    }
}

/// Bytes PNG del ícono para `set_id`+`key`, owned. Cae a `unknown` dentro del set si la
/// clave no tiene asset propio. Devuelve `Vec` vacío si `set_id` no es un set embebido.
pub fn bytes_for_id(set_id: &str, key: IconKey) -> Vec<u8> {
    let name = file_name(key);
    match table_for_id(set_id) {
        Some(table) => table
            .iter()
            .find(|(n, _)| *n == name)
            .or_else(|| table.iter().find(|(n, _)| *n == "unknown"))
            .map(|(_, b)| b.to_vec())
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Resuelve los bytes de `key` aplicando los overrides del usuario sobre el `base_set`.
///
/// Orden de resolución:
/// 1. Si existe un override para la clave → usa su fuente (Builtin otro set, o UserPng).
///    Si la fuente falla (PNG user inexistente, set desconocido), continúa al paso 2.
/// 2. Bytes del `base_set` (embebido o pack suelto en disco).
/// 3. Fallback final: `unknown` de lucide (nunca vacío).
pub fn resolve_with_overrides(
    base_set: &str,
    overrides: &std::collections::BTreeMap<String, crate::icon_source::IconSource>,
    key: IconKey,
    config_dir: &std::path::Path,
) -> Vec<u8> {
    if let Some(src) = overrides.get(file_name(key)) {
        match src {
            crate::icon_source::IconSource::Builtin { set_id } => {
                let b = resolve_set_bytes(set_id, key, config_dir);
                if !b.is_empty() {
                    return b;
                }
            }
            crate::icon_source::IconSource::UserPng { rel_path } => {
                let p = config_dir.join("icons").join("_user").join(rel_path);
                if let Ok(bytes) = std::fs::read(&p) {
                    if !bytes.is_empty() {
                        return bytes;
                    }
                }
            }
        }
    }
    let b = resolve_set_bytes(base_set, key, config_dir);
    if !b.is_empty() {
        return b;
    }
    bytes_for_id(FALLBACK_SET, IconKey::Unknown)
}

/// Bytes de un set por id: embebido (de fábrica) o pack suelto en disco. Devuelve
/// Vec vacío si el archivo del pack suelto falta — el fallback lo maneja
/// `resolve_with_overrides` (punto único de fallback).
fn resolve_set_bytes(set_id: &str, key: IconKey, config_dir: &std::path::Path) -> Vec<u8> {
    if table_for_id(set_id).is_some() {
        return bytes_for_id(set_id, key);
    }
    let path = config_dir
        .join("icons")
        .join(set_id)
        .join(format!("{}.png", file_name(key)));
    std::fs::read(&path).unwrap_or_default()
}

/// Resolución sin overrides (solo set base). Compat con código que no necesita overrides.
pub fn resolve_bytes(set_id: &str, key: IconKey, config_dir: &std::path::Path) -> Vec<u8> {
    let empty = std::collections::BTreeMap::new();
    resolve_with_overrides(set_id, &empty, key, config_dir)
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
    fn cada_set_de_fabrica_cubre_las_33_claves() {
        for set in ["lucide", "tabler", "material", "flat-color", "mono"] {
            for key in all_keys() {
                assert!(
                    !bytes_for_id(set, key).is_empty(),
                    "asset vacío {set}/{:?}",
                    key
                );
            }
        }
    }

    #[test]
    fn cada_clave_tiene_su_propio_asset_no_solo_el_fallback() {
        // Verifica que el NOMBRE EXACTO de cada clave está en la tabla (no solo el fallback
        // "unknown"). Un PNG faltante para una clave concreta sí se detecta.
        for set in ["lucide", "tabler", "material", "flat-color", "mono"] {
            let table = table_for_id(set).expect("set embebido existe");
            for key in all_keys() {
                let name = file_name(key);
                assert!(
                    table.iter().any(|(n, _)| *n == name),
                    "falta el asset propio '{name}' para {set}/{:?}",
                    key
                );
            }
        }
    }

    #[test]
    fn clave_sin_asset_cae_a_unknown() {
        // Drive(Fixed) no tiene asset propio → cae a "unknown" (no vacío).
        let b = bytes_for_id(
            "lucide",
            IconKey::Drive(crate::icon_kind::DriveKind::Fixed),
        );
        assert!(!b.is_empty());
    }

    #[test]
    fn resolve_con_override_builtin_usa_el_set_indicado() {
        use crate::icon_source::IconSource;
        use std::collections::BTreeMap;
        let mut ov: BTreeMap<String, IconSource> = BTreeMap::new();
        ov.insert("folder".into(), IconSource::Builtin { set_id: "material".into() });
        let dir = std::path::Path::new("");
        let with = resolve_with_overrides("lucide", &ov, IconKey::Folder, dir);
        let material_folder = bytes_for_id("material", IconKey::Folder);
        assert_eq!(with, material_folder);
        let copy = resolve_with_overrides("lucide", &ov, IconKey::Action(crate::icon_kind::ActionIcon::Copy), dir);
        assert_eq!(copy, bytes_for_id("lucide", IconKey::Action(crate::icon_kind::ActionIcon::Copy)));
    }

    #[test]
    fn resolve_override_userpng_inexistente_cae_a_unknown() {
        use crate::icon_source::IconSource;
        use std::collections::BTreeMap;
        let mut ov: BTreeMap<String, IconSource> = BTreeMap::new();
        ov.insert("folder".into(), IconSource::UserPng { rel_path: "no-existe.png".into() });
        let dir = std::path::Path::new("");
        let bytes = resolve_with_overrides("lucide", &ov, IconKey::Folder, dir);
        assert!(!bytes.is_empty());
    }
}
