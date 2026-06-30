// Naygo — fuente de un ícono para una clave: un set embebido/pack, o un PNG propio.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `IconSource` describe de dónde sale el ícono de un objeto cuando el usuario lo
//! sobrescribe: `Builtin` apunta a otro set (por id); `UserPng` a un PNG propio bajo
//! `<config_dir>/icons/_user/`. Además, la conversión `IconKey` ↔ string estable
//! (`"action_back"`, `"file_image"`, `"drive"`, …) usada como clave del mapa de
//! overrides en `settings.json`. Puro y testeable.

use crate::icon_kind::{ActionIcon, DriveKind, FileCategory, IconKey};
use serde::{Deserialize, Serialize};

/// De dónde sale el ícono de un objeto sobrescrito por el usuario.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IconSource {
    /// Un ícono de otro set (id de fábrica o pack suelto) para la misma clave.
    Builtin { set_id: String },
    /// Un PNG propio del usuario, ruta relativa a `<config_dir>/icons/_user/`.
    UserPng { rel_path: String },
}

/// Clave estable de string para una `IconKey` (la misma que el nombre de archivo del
/// asset). Reutiliza `icons::file_name`.
pub fn key_to_string(key: IconKey) -> String {
    crate::icons::file_name(key).to_string()
}

/// Inversa de `key_to_string`. `None` si el string no corresponde a ninguna clave.
pub fn key_from_string(s: &str) -> Option<IconKey> {
    use FileCategory::*;
    let direct = match s {
        "folder" => Some(IconKey::Folder),
        "unknown" => Some(IconKey::Unknown),
        // Todos los DriveKind comparten el string "drive"; al deserializar usamos
        // Unknown (valor canónico; el DriveKind real lo aporta platform en runtime).
        "drive" => Some(IconKey::Drive(DriveKind::Unknown)),
        "file_image" => Some(IconKey::File(Image)),
        "file_video" => Some(IconKey::File(Video)),
        "file_audio" => Some(IconKey::File(Audio)),
        "file_document" => Some(IconKey::File(Document)),
        "file_code" => Some(IconKey::File(Code)),
        "file_archive" => Some(IconKey::File(Archive)),
        "file_executable" => Some(IconKey::File(Executable)),
        "file_model3d" => Some(IconKey::File(Model3D)),
        "file_font" => Some(IconKey::File(Font)),
        "file_generic" => Some(IconKey::File(Generic)),
        _ => None,
    };
    if direct.is_some() {
        return direct;
    }
    ActionIcon::all()
        .iter()
        .find(|a| a.file_name() == s)
        .map(|a| IconKey::Action(*a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_source_serde_round_trip() {
        let a = IconSource::Builtin {
            set_id: "material".into(),
        };
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"kind":"builtin","set_id":"material"}"#);
        let back: IconSource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, a);

        let b = IconSource::UserPng {
            rel_path: "ab12.png".into(),
        };
        let json = serde_json::to_string(&b).unwrap();
        let back: IconSource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn key_string_round_trip_todas_las_claves() {
        for key in crate::icons::all_keys() {
            let s = key_to_string(key);
            let back = key_from_string(&s).expect("clave válida");
            assert_eq!(back, key, "round-trip falló para {s}");
        }
    }

    #[test]
    fn key_from_string_desconocida_es_none() {
        assert!(key_from_string("no_existe").is_none());
    }

    #[test]
    fn las_6_claves_nuevas_round_trip() {
        for name in [
            "action_home",
            "action_search",
            "action_show_hidden",
            "action_history",
            "action_favorites",
            "action_split",
        ] {
            let key = key_from_string(name).expect("clave válida");
            assert_eq!(key_to_string(key), name);
        }
    }
}
