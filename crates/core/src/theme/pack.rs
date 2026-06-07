// Naygo — "packs": preset que activa un tema + un set de íconos juntos.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Un `Pack` empareja un `ThemeId` con un `IconSet`. Activar un pack escribe ambos
//! ajustes (que siguen siendo independientes después). Embebidos + sueltos, patrón
//! i18n. Puro y testeable.

use crate::config::IconSet;
use crate::theme::ThemeId;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Un pack: nombre visible + tema + set de íconos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    pub name: String,
    pub theme: ThemeId,
    pub icon_set: IconSet,
}

/// Catálogo de packs: embebidos + sueltos de `<config_dir>/packs/*.json`.
pub struct PackCatalog {
    packs: Vec<Pack>,
}

const DARK_BLUE: &str = include_str!("packs/dark-blue.json");
const DARK_TEAL: &str = include_str!("packs/dark-teal.json");
const LIGHT: &str = include_str!("packs/light.json");
const HIGH_CONTRAST: &str = include_str!("packs/high-contrast.json");

impl PackCatalog {
    /// Carga embebidos + sueltos. JSON inválido se ignora.
    pub fn load(dir: &Path) -> PackCatalog {
        let mut packs: Vec<Pack> = Vec::new();
        for json in [DARK_BLUE, DARK_TEAL, LIGHT, HIGH_CONTRAST] {
            if let Ok(p) = serde_json::from_str::<Pack>(json) {
                packs.push(p);
            }
        }
        let pack_dir = dir.join("packs");
        if let Ok(entries) = std::fs::read_dir(&pack_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        if let Ok(p) = serde_json::from_str::<Pack>(&text) {
                            packs.push(p);
                        }
                    }
                }
            }
        }
        PackCatalog { packs }
    }

    /// Todos los packs (embebidos primero, luego sueltos en orden de lectura).
    pub fn packs(&self) -> &[Pack] {
        &self.packs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_round_trip_serde() {
        let p = Pack {
            name: "X".into(),
            theme: ThemeId::new("dark-blue"),
            icon_set: IconSet::Flat,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: Pack = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn catalog_tiene_los_cuatro_embebidos() {
        let cat = PackCatalog::load(Path::new("Z:/no/existe"));
        assert_eq!(cat.packs().len(), 4);
        let names: Vec<&str> = cat.packs().iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Dark Blue"));
        assert!(names.contains(&"High Contrast"));
    }

    #[test]
    fn pack_referencia_tema_por_id() {
        let cat = PackCatalog::load(Path::new("Z:/no/existe"));
        let db = cat.packs().iter().find(|p| p.name == "Dark Blue").unwrap();
        assert_eq!(db.theme, ThemeId::new("dark-blue"));
    }
}
