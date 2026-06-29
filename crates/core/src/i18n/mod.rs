// Naygo — internacionalización: catálogo de textos por clave (lógica pura).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `I18n` resuelve un texto por clave en el idioma activo, con fallback al idioma
//! de respaldo (ES) y, si falta, a la clave misma (visible, nunca panic). ES y EN
//! van embebidos; además se pueden cargar `lang/*.json` sueltos al lado del `.exe`.
//! No toca egui ni Windows; la lectura del locale del SO vive en `platform`.

pub mod detect;

pub use detect::pick_default_language;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Identificador de idioma (código corto: "es", "en", "fr"...).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LangId(pub String);

impl LangId {
    pub fn new(code: &str) -> Self {
        LangId(code.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Catálogo de un idioma: clave → texto.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub lang: String,
    map: HashMap<String, String>,
}

impl Catalog {
    /// Parsea un catálogo desde JSON plano (clave: texto). JSON inválido → vacío.
    pub fn from_json(lang: &str, json: &str) -> Catalog {
        let map: HashMap<String, String> = serde_json::from_str(json).unwrap_or_else(|e| {
            tracing::warn!("catálogo i18n '{lang}' ilegible: {e}");
            HashMap::new()
        });
        Catalog {
            lang: lang.to_string(),
            map,
        }
    }

    /// Texto de `key`, o `None` si la clave no está.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }

    /// Mergea otro catálogo encima (las claves de `other` ganan).
    pub fn merge(&mut self, other: &Catalog) {
        for (k, v) in &other.map {
            self.map.insert(k.clone(), v.clone());
        }
    }
}

/// Catálogos embebidos en el binario (siempre disponibles).
const ES_JSON: &str = include_str!("es.json");
const EN_JSON: &str = include_str!("en.json");

/// Estado de i18n: idioma activo + fallback (ES) + idiomas disponibles.
pub struct I18n {
    active: Catalog,
    fallback: Catalog,
    available: Vec<LangId>,
    catalogs: HashMap<String, Catalog>,
}

impl I18n {
    /// Construye con los embebidos (ES/EN) más los `lang/*.json` de `dir`, y activa
    /// `lang` (si no existe, cae a EN, y si tampoco, a ES).
    pub fn load(dir: &Path, lang: &LangId) -> I18n {
        let mut catalogs: HashMap<String, Catalog> = HashMap::new();
        catalogs.insert("es".into(), Catalog::from_json("es", ES_JSON));
        catalogs.insert("en".into(), Catalog::from_json("en", EN_JSON));

        // Cargar archivos sueltos: dir/lang/*.json (cada uno mergea/añade).
        let lang_dir = dir.join("lang");
        if let Ok(entries) = std::fs::read_dir(&lang_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(code) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(text) = std::fs::read_to_string(&path) {
                            let cat = Catalog::from_json(code, &text);
                            catalogs.entry(code.to_string()).or_default().merge(&cat);
                            catalogs.get_mut(code).unwrap().lang = code.to_string();
                        }
                    }
                }
            }
        }

        let available: Vec<LangId> = {
            let mut v: Vec<LangId> = catalogs.keys().map(|k| LangId::new(k)).collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v
        };

        let fallback = catalogs.get("es").cloned().unwrap_or_default();
        let active = catalogs
            .get(lang.as_str())
            .or_else(|| catalogs.get("en"))
            .or_else(|| catalogs.get("es"))
            .cloned()
            .unwrap_or_default();

        I18n {
            active,
            fallback,
            available,
            catalogs,
        }
    }

    /// Texto de `key`: activo → fallback (ES) → la clave misma (nunca panic).
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.active
            .get(key)
            .or_else(|| self.fallback.get(key))
            .unwrap_or(key)
    }

    /// Cambia el idioma activo si está cargado. Devuelve true si cambió.
    pub fn set_language(&mut self, lang: &LangId) -> bool {
        if let Some(cat) = self.catalogs.get(lang.as_str()) {
            self.active = cat.clone();
            true
        } else {
            false
        }
    }

    /// Idioma activo.
    pub fn active_lang(&self) -> LangId {
        LangId::new(&self.active.lang)
    }

    /// Idiomas disponibles (ordenados).
    pub fn available(&self) -> &[LangId] {
        &self.available
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i18n_de_prueba() -> I18n {
        I18n::load(Path::new("Z:/no/existe/naygo-test"), &LangId::new("es"))
    }

    /// Paridad ES/EN: ambos JSON embebidos deben tener EXACTAMENTE el mismo conjunto de
    /// claves (y ninguna vacía). Atrapa que alguien agregue/quite una clave en un idioma
    /// y olvide el otro — un riesgo en cada fase que toca i18n.
    #[test]
    fn es_en_tienen_las_mismas_claves() {
        use std::collections::BTreeSet;
        let es: std::collections::HashMap<String, String> =
            serde_json::from_str(ES_JSON).expect("es.json válido");
        let en: std::collections::HashMap<String, String> =
            serde_json::from_str(EN_JSON).expect("en.json válido");
        let es_keys: BTreeSet<&String> = es.keys().collect();
        let en_keys: BTreeSet<&String> = en.keys().collect();
        let solo_es: Vec<_> = es_keys.difference(&en_keys).collect();
        let solo_en: Vec<_> = en_keys.difference(&es_keys).collect();
        assert!(solo_es.is_empty(), "claves solo en es.json: {solo_es:?}");
        assert!(solo_en.is_empty(), "claves solo en en.json: {solo_en:?}");
        assert!(
            es.values().all(|v| !v.is_empty()),
            "es.json tiene valores vacíos"
        );
        assert!(
            en.values().all(|v| !v.is_empty()),
            "en.json tiene valores vacíos"
        );
    }

    /// Cada acción del keymap tiene su nombre traducido en AMBOS idiomas.
    #[test]
    fn cada_accion_tiene_nombre_en_ambos_idiomas() {
        let es: std::collections::HashMap<String, String> = serde_json::from_str(ES_JSON).unwrap();
        let en: std::collections::HashMap<String, String> = serde_json::from_str(EN_JSON).unwrap();
        for a in crate::keymap::Action::all() {
            let k = a.i18n_key();
            assert!(es.contains_key(k), "falta {k} en es.json");
            assert!(en.contains_key(k), "falta {k} en en.json");
        }
    }

    #[test]
    fn t_resuelve_clave_del_activo() {
        let i = i18n_de_prueba();
        assert_eq!(i.t("app.loading"), "Listando…");
    }

    #[test]
    fn t_clave_inexistente_devuelve_la_clave() {
        let i = i18n_de_prueba();
        assert_eq!(i.t("clave.que.no.existe"), "clave.que.no.existe");
    }

    #[test]
    fn set_language_cambia_el_activo() {
        let mut i = i18n_de_prueba();
        assert!(i.set_language(&LangId::new("en")));
        assert_eq!(i.t("app.loading"), "Listing…");
    }

    #[test]
    fn set_language_idioma_inexistente_no_cambia() {
        let mut i = i18n_de_prueba();
        assert!(!i.set_language(&LangId::new("zz")));
        assert_eq!(i.t("app.loading"), "Listando…");
    }

    #[test]
    fn fallback_a_es_si_falta_en_el_activo() {
        // Prueba REAL del fallback activo→ES: cargamos un `es.json` suelto con una
        // clave que NO existe en EN; activamos EN; `t()` de esa clave debe caer al
        // fallback ES (no a la clave cruda). Ejercita la rama de fallback de verdad.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("lang")).unwrap();
        std::fs::write(
            dir.path().join("lang").join("es.json"),
            r#"{"solo.en.es": "valor en español"}"#,
        )
        .unwrap();
        let mut i = I18n::load(dir.path(), &LangId::new("en"));
        // Activo = EN (no tiene la clave); fallback = ES (sí la tiene tras el merge).
        assert_eq!(i.active_lang(), LangId::new("en"));
        assert_eq!(i.t("solo.en.es"), "valor en español", "cae al fallback ES");
        // Y una clave inexistente en ambos sí devuelve la clave cruda.
        assert_eq!(i.t("no.existe.nada"), "no.existe.nada");
        // Sanidad: cambiar a ES la resuelve directo.
        i.set_language(&LangId::new("es"));
        assert_eq!(i.t("solo.en.es"), "valor en español");
    }

    #[test]
    fn catalog_json_invalido_queda_vacio() {
        let c = Catalog::from_json("es", "{ no es json");
        assert!(c.get("x").is_none());
    }

    #[test]
    fn disponibles_incluye_es_y_en() {
        let i = i18n_de_prueba();
        let codes: Vec<&str> = i.available().iter().map(|l| l.as_str()).collect();
        assert!(codes.contains(&"es"));
        assert!(codes.contains(&"en"));
    }
}
