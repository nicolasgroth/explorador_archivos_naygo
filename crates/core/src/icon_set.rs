// Naygo — catálogo de sets de íconos: embebidos + packs sueltos del usuario.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lista los sets de íconos disponibles: los 3 embebidos (flat/fluent/mono) más los
//! packs sueltos descubiertos en `<config_dir>/icons/<nombre>/`. Patrón análogo a
//! `theme::ThemeCatalog`. Puro salvo el `read_dir` de descubrimiento.

use std::path::Path;

/// Un set de íconos disponible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconSetInfo {
    /// Id estable (flat/fluent/mono o el nombre de la carpeta suelta).
    pub id: String,
    /// Etiqueta a mostrar.
    pub label: String,
    /// `true` si es uno de los 3 embebidos.
    pub builtin: bool,
}

/// Catálogo de sets disponibles.
pub struct IconSetCatalog {
    sets: Vec<IconSetInfo>,
}

impl IconSetCatalog {
    /// Construye el catálogo: 3 embebidos + sueltos de `<dir>/icons/<nombre>/`.
    /// Tolerante: si `read_dir` falla, solo los embebidos.
    pub fn load(dir: &Path) -> IconSetCatalog {
        let mut sets = vec![
            IconSetInfo {
                id: "flat".into(),
                label: "Flat".into(),
                builtin: true,
            },
            IconSetInfo {
                id: "fluent".into(),
                label: "Fluent".into(),
                builtin: true,
            },
            IconSetInfo {
                id: "mono".into(),
                label: "Mono".into(),
                builtin: true,
            },
        ];
        let icons_dir = dir.join("icons");
        if let Ok(entries) = std::fs::read_dir(&icons_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if !["flat", "fluent", "mono"].contains(&name) {
                            sets.push(IconSetInfo {
                                id: name.to_string(),
                                label: name.to_string(),
                                builtin: false,
                            });
                        }
                    }
                }
            }
        }
        IconSetCatalog { sets }
    }

    /// Los sets disponibles (embebidos primero, luego sueltos).
    pub fn available(&self) -> &[IconSetInfo] {
        &self.sets
    }

    /// ¿Existe un set con este id?
    pub fn contains(&self, id: &str) -> bool {
        self.sets.iter().any(|s| s.id == id)
    }

    /// Resuelve un id a uno válido: si no existe, cae a "flat".
    pub fn resolve(&self, id: &str) -> String {
        if self.contains(id) {
            id.to_string()
        } else {
            "flat".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embebidos_siempre_presentes() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert!(cat.contains("flat") && cat.contains("fluent") && cat.contains("mono"));
        assert_eq!(cat.available().len(), 3);
    }

    #[test]
    fn descubre_packs_sueltos() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons").join("mi-pack")).unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert!(cat.contains("mi-pack"));
        let info = cat.available().iter().find(|s| s.id == "mi-pack").unwrap();
        assert!(!info.builtin);
    }

    #[test]
    fn icons_dir_ausente_solo_embebidos() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(cat.available().len(), 3);
    }

    #[test]
    fn resolve_desconocido_cae_a_flat() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(cat.resolve("no-existe"), "flat");
        assert_eq!(cat.resolve("mono"), "mono");
    }

    #[test]
    fn pack_suelto_no_duplica_id_embebido() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons").join("flat")).unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(cat.available().iter().filter(|s| s.id == "flat").count(), 1);
    }
}
