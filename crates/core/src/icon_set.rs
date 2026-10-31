// Naygo — catálogo de sets de íconos: embebidos + packs sueltos del usuario.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Lista los sets de íconos disponibles: los sets de fábrica más los packs sueltos descubiertos
//! en `<data_dir>/icons/<nombre>/`. También lee `<legacy_config_dir>/icons` para conservar
//! instalaciones anteriores sin mover archivos durante el arranque.
//! Patrón análogo a `theme::ThemeCatalog`. Puro salvo el `read_dir` de descubrimiento.

use std::path::Path;

/// Un set de íconos disponible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconSetInfo {
    /// Id estable (uno de fábrica o el nombre de la carpeta suelta).
    pub id: String,
    /// Etiqueta a mostrar.
    pub label: String,
    /// `true` si es uno de los sets de fábrica.
    pub builtin: bool,
    /// Si los íconos se tiñen al color del tema (línea/máscara) o traen su color (flat-color).
    pub tintable: bool,
}

/// Catálogo de sets disponibles.
pub struct IconSetCatalog {
    sets: Vec<IconSetInfo>,
}

impl IconSetCatalog {
    /// Construye el catálogo: sets de fábrica + sueltos de `<dir>/icons/<nombre>/`.
    /// Tolerante: si `read_dir` falla, solo los de fábrica.
    pub fn load(dir: &Path) -> IconSetCatalog {
        Self::load_with_legacy(dir, None)
    }

    /// Carga primero los packs de datos y añade los de la ubicación antigua. En caso de id
    /// duplicado prevalece el pack bajo `data_dir`. No migra ni modifica archivos.
    pub fn load_with_legacy(data_dir: &Path, legacy_config_dir: Option<&Path>) -> IconSetCatalog {
        let mut catalog = Self::embedded();
        let sets = &mut catalog.sets;
        // Mantener en sync con los IDs de fábrica de `embedded`: si agregas un set nuevo allá,
        // agrégalo aquí o su carpeta se detectaría como pack suelto duplicado.
        let factory_ids = [
            "lucide",
            "fluent",
            "tabler",
            "material",
            "flat-color",
            "mono",
            "kenney-game",
            "kenney-board",
            "vivid",
            "pastel",
        ];
        let mut seen = std::collections::HashSet::new();
        for root in std::iter::once(data_dir).chain(legacy_config_dir) {
            let icons_dir = root.join("icons");
            let Ok(entries) = std::fs::read_dir(&icons_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if factory_ids.contains(&name) || !seen.insert(name.to_string()) {
                    continue;
                }
                sets.push(IconSetInfo {
                    id: name.to_string(),
                    label: name.to_string(),
                    builtin: false,
                    tintable: false,
                });
            }
        }
        catalog
    }

    /// Construye solo los sets embebidos; no consulta el filesystem.
    pub fn embedded() -> IconSetCatalog {
        let sets = vec![
            IconSetInfo {
                id: "lucide".into(),
                label: "Lucide".into(),
                builtin: true,
                tintable: true,
            },
            IconSetInfo {
                id: "fluent".into(),
                label: "Fluent".into(),
                builtin: true,
                tintable: false,
            },
            IconSetInfo {
                id: "tabler".into(),
                label: "Tabler".into(),
                builtin: true,
                tintable: true,
            },
            IconSetInfo {
                id: "material".into(),
                label: "Material".into(),
                builtin: true,
                tintable: true,
            },
            IconSetInfo {
                id: "flat-color".into(),
                label: "Flat Color".into(),
                builtin: true,
                tintable: false,
            },
            IconSetInfo {
                id: "mono".into(),
                label: "Mono".into(),
                builtin: true,
                tintable: true,
            },
            IconSetInfo {
                id: "kenney-game".into(),
                // El ID permanece estable para no invalidar configuraciones existentes.
                // La etiqueta describe el estilo, sin exponer el proveedor del recurso.
                label: "Outline".into(),
                builtin: true,
                tintable: true,
            },
            IconSetInfo {
                id: "kenney-board".into(),
                label: "Tiles".into(),
                builtin: true,
                tintable: true,
            },
            IconSetInfo {
                id: "vivid".into(),
                label: "Vivid".into(),
                builtin: true,
                tintable: false,
            },
            IconSetInfo {
                id: "pastel".into(),
                label: "Pastel".into(),
                builtin: true,
                tintable: false,
            },
        ];
        IconSetCatalog { sets }
    }

    /// Los sets disponibles (de fábrica primero, luego sueltos).
    pub fn available(&self) -> &[IconSetInfo] {
        &self.sets
    }

    /// ¿Existe un set con este id?
    pub fn contains(&self, id: &str) -> bool {
        self.sets.iter().any(|s| s.id == id)
    }

    /// ¿El set con este id se tiñe al color del tema? `false` si el id no existe.
    pub fn is_tintable(&self, set_id: &str) -> bool {
        self.sets
            .iter()
            .find(|s| s.id == set_id)
            .map(|s| s.tintable)
            .unwrap_or(false)
    }

    /// Resuelve un id a uno válido: si no existe, cae a "lucide".
    pub fn resolve(&self, id: &str) -> String {
        if self.contains(id) {
            id.to_string()
        } else {
            "lucide".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diez_sets_de_fabrica_presentes_incluyendo_las_paletas_de_color() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        for id in [
            "lucide",
            "fluent",
            "tabler",
            "material",
            "flat-color",
            "mono",
            "kenney-game",
            "kenney-board",
            "vivid",
            "pastel",
        ] {
            assert!(cat.contains(id), "falta el set de fábrica {id}");
        }
        assert_eq!(cat.available().len(), 10);
    }

    #[test]
    fn tintable_correcto_por_set() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        let by = |id: &str| {
            cat.available()
                .iter()
                .find(|s| s.id == id)
                .unwrap()
                .tintable
        };
        assert!(by("lucide"));
        assert!(by("tabler"));
        assert!(by("material"));
        assert!(by("mono"));
        assert!(by("kenney-game"));
        assert!(by("kenney-board"));
        assert!(!by("flat-color")); // trae su propio color
        assert!(!by("vivid")); // trae una paleta semántica propia
        assert!(!by("pastel"));
    }

    #[test]
    fn is_tintable_por_id() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert!(cat.is_tintable("lucide"));
        assert!(!cat.is_tintable("flat-color"));
        assert!(!cat.is_tintable("no-existe"));
    }

    #[test]
    fn resolve_desconocido_cae_a_lucide() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(cat.resolve("no-existe"), "lucide");
        assert_eq!(cat.resolve("material"), "material");
    }

    #[test]
    fn pack_suelto_importado_es_tintable_false_por_defecto() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons").join("mi-pack")).unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert!(cat.contains("mi-pack"));
        let info = cat.available().iter().find(|s| s.id == "mi-pack").unwrap();
        assert!(!info.builtin);
        assert!(!info.tintable);
    }

    #[test]
    fn catalogo_combina_data_con_config_legacy_sin_duplicar_ids() {
        let data = tempfile::tempdir().unwrap();
        let legacy = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(data.path().join("icons").join("compartido")).unwrap();
        std::fs::create_dir_all(data.path().join("icons").join("solo-data")).unwrap();
        std::fs::create_dir_all(legacy.path().join("icons").join("compartido")).unwrap();
        std::fs::create_dir_all(legacy.path().join("icons").join("solo-legacy")).unwrap();

        let catalog = IconSetCatalog::load_with_legacy(data.path(), Some(legacy.path()));
        let user_ids = catalog
            .available()
            .iter()
            .filter(|set| !set.builtin)
            .map(|set| set.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(user_ids.len(), 3);
        assert!(user_ids.contains("compartido"));
        assert!(user_ids.contains("solo-data"));
        assert!(user_ids.contains("solo-legacy"));
    }

    #[test]
    fn icons_dir_ausente_solo_fabrica() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(cat.available().len(), 10);
    }

    #[test]
    fn pack_suelto_no_duplica_id_fabrica() {
        let dir = tempfile::tempdir().unwrap();
        // Crear una carpeta con el mismo id que uno de los sets de fábrica.
        std::fs::create_dir_all(dir.path().join("icons").join("lucide")).unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(
            cat.available().iter().filter(|s| s.id == "lucide").count(),
            1
        );
    }
}
