// Naygo — catálogo de sets de íconos: embebidos + packs sueltos del usuario.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lista los sets de íconos disponibles: los 5 de fábrica (lucide/tabler/material/
//! flat-color/mono) más los packs sueltos descubiertos en `<config_dir>/icons/<nombre>/`.
//! Patrón análogo a `theme::ThemeCatalog`. Puro salvo el `read_dir` de descubrimiento.

use std::path::Path;

/// Un set de íconos disponible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconSetInfo {
    /// Id estable (lucide/tabler/material/flat-color/mono o el nombre de la carpeta suelta).
    pub id: String,
    /// Etiqueta a mostrar.
    pub label: String,
    /// `true` si es uno de los 5 de fábrica.
    pub builtin: bool,
    /// Si los íconos se tiñen al color del tema (línea/máscara) o traen su color (flat-color).
    pub tintable: bool,
}

/// Catálogo de sets disponibles.
pub struct IconSetCatalog {
    sets: Vec<IconSetInfo>,
}

impl IconSetCatalog {
    /// Construye el catálogo: 5 de fábrica + sueltos de `<dir>/icons/<nombre>/`.
    /// Tolerante: si `read_dir` falla, solo los de fábrica.
    pub fn load(dir: &Path) -> IconSetCatalog {
        let mut sets = vec![
            IconSetInfo { id: "lucide".into(),     label: "Lucide".into(),     builtin: true, tintable: true },
            IconSetInfo { id: "tabler".into(),      label: "Tabler".into(),     builtin: true, tintable: true },
            IconSetInfo { id: "material".into(),    label: "Material".into(),   builtin: true, tintable: true },
            IconSetInfo { id: "flat-color".into(),  label: "Flat Color".into(), builtin: true, tintable: false },
            IconSetInfo { id: "mono".into(),        label: "Mono".into(),       builtin: true, tintable: true },
        ];
        let factory_ids = ["lucide", "tabler", "material", "flat-color", "mono"];
        let icons_dir = dir.join("icons");
        if let Ok(entries) = std::fs::read_dir(&icons_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if !factory_ids.contains(&name) {
                            sets.push(IconSetInfo {
                                id: name.to_string(),
                                label: name.to_string(),
                                builtin: false,
                                tintable: false,
                            });
                        }
                    }
                }
            }
        }
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
    fn cinco_sets_de_fabrica_presentes() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        for id in ["lucide", "tabler", "material", "flat-color", "mono"] {
            assert!(cat.contains(id), "falta el set de fábrica {id}");
        }
        assert_eq!(cat.available().len(), 5);
    }

    #[test]
    fn tintable_correcto_por_set() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        let by = |id: &str| cat.available().iter().find(|s| s.id == id).unwrap().tintable;
        assert!(by("lucide"));
        assert!(by("tabler"));
        assert!(by("material"));
        assert!(by("mono"));
        assert!(!by("flat-color")); // trae su propio color
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
        let info = cat.available().iter().find(|s| s.id == "mi-pack").unwrap();
        assert!(!info.builtin);
        assert!(!info.tintable);
    }

    #[test]
    fn descubre_packs_sueltos() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons").join("mi-pack")).unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert!(cat.contains("mi-pack"));
        let info = cat.available().iter().find(|s| s.id == "mi-pack").unwrap();
        assert!(!info.builtin);
        assert!(!info.tintable);
    }

    #[test]
    fn icons_dir_ausente_solo_fabrica() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(cat.available().len(), 5);
    }

    #[test]
    fn pack_suelto_no_duplica_id_fabrica() {
        let dir = tempfile::tempdir().unwrap();
        // Crear una carpeta con el mismo id que uno de los sets de fábrica.
        std::fs::create_dir_all(dir.path().join("icons").join("lucide")).unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(cat.available().iter().filter(|s| s.id == "lucide").count(), 1);
    }
}
