// Naygo — empaquetado de sets de íconos: export/import del archivo .naygoset (zip).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Un `.naygoset` es un zip con `manifest.json` + `icons/` (los PNG que el set
//! aporta). Export toma el set efectivo (base + overrides) y lo empaqueta autocontenido;
//! import lo valida y lo copia a `<config_dir>/icons/<nombre>/`. Tolerante a archivos
//! corruptos: una entrada inválida no aborta la importación.

use crate::icon_source::IconSource;
use serde::{Deserialize, Serialize};

/// Una entrada del manifest: qué objeto y de dónde sale su ícono.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverrideEntry {
    pub key: String,
    pub source: IconSource,
}

/// Manifest de un `.naygoset`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    pub author: String,
    pub base_set_id: String,
    #[serde(default)]
    pub overrides: Vec<OverrideEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let m = PackManifest {
            schema: 1,
            name: "Mi set".into(),
            author: "Nico".into(),
            base_set_id: "lucide".into(),
            overrides: vec![OverrideEntry {
                key: "folder".into(),
                source: IconSource::Builtin { set_id: "material".into() },
            }],
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: PackManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }
}
