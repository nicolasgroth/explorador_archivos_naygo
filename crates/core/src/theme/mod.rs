// Naygo — sistema de temas (color sets): datos puros, sin egui ni Windows.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Un `Theme` es una paleta de colores con nombre + base claro/oscuro. Los temas se
//! cargan de JSON embebidos y de archivos sueltos (patrón i18n), son tolerantes a
//! campos faltantes, y se aplican en caliente. Puro y testeable; la traducción a
//! `egui::Visuals` vive en la capa `ui`.

pub mod pack;

use serde::{Deserialize, Serialize};

/// Base visual de la que se derivan los neutros (la capa ui parte de
/// `Visuals::light()`/`dark()` según esto).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeBase {
    Light,
    Dark,
}

/// Un color RGB. Se serializa como cadena hex "#rrggbb".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ThemeColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        ThemeColor { r, g, b }
    }

    /// Parsea "#rrggbb" o "rrggbb" (case-insensitive). `None` si es inválido.
    pub fn from_hex(s: &str) -> Option<ThemeColor> {
        let h = s.strip_prefix('#').unwrap_or(s);
        if h.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&h[0..2], 16).ok()?;
        let g = u8::from_str_radix(&h[2..4], 16).ok()?;
        let b = u8::from_str_radix(&h[4..6], 16).ok()?;
        Some(ThemeColor { r, g, b })
    }

    /// Formatea como "#rrggbb".
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl Serialize for ThemeColor {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<ThemeColor, D::Error> {
        let s = String::deserialize(d)?;
        ThemeColor::from_hex(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("color hex inválido: {s}")))
    }
}

/// Identificador estable de un tema (su nombre-clave, p. ej. el file_stem del JSON).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThemeId(pub String);

impl ThemeId {
    pub fn new(id: &str) -> Self {
        ThemeId(id.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

use std::collections::HashMap;
use std::path::Path;

/// Paleta completa de un tema (tokens propios de Naygo).
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub base: ThemeBase,
    pub accent: ThemeColor,
    pub panel_bg: ThemeColor,
    pub row_bg: ThemeColor,
    pub row_alt_bg: ThemeColor,
    pub text: ThemeColor,
    pub text_dim: ThemeColor,
    pub selection_bg: ThemeColor,
    pub active_bar: ThemeColor,
    pub error: ThemeColor,
    /// Color de resaltado para archivos recién aparecidos (vigilancia de carpeta).
    pub highlight: ThemeColor,
    pub border: ThemeColor,
}

impl Serialize for Theme {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Theme", 13)?;
        st.serialize_field("name", &self.name)?;
        st.serialize_field("base", &self.base)?;
        st.serialize_field("accent", &self.accent)?;
        st.serialize_field("panel_bg", &self.panel_bg)?;
        st.serialize_field("row_bg", &self.row_bg)?;
        st.serialize_field("row_alt_bg", &self.row_alt_bg)?;
        st.serialize_field("text", &self.text)?;
        st.serialize_field("text_dim", &self.text_dim)?;
        st.serialize_field("selection_bg", &self.selection_bg)?;
        st.serialize_field("active_bar", &self.active_bar)?;
        st.serialize_field("error", &self.error)?;
        st.serialize_field("highlight", &self.highlight)?;
        st.serialize_field("border", &self.border)?;
        st.end()
    }
}

/// Forma cruda para deserializar tolerando faltantes (cada color es opcional).
#[derive(Deserialize)]
struct ThemeRaw {
    name: Option<String>,
    base: Option<ThemeBase>,
    accent: Option<ThemeColor>,
    panel_bg: Option<ThemeColor>,
    row_bg: Option<ThemeColor>,
    row_alt_bg: Option<ThemeColor>,
    text: Option<ThemeColor>,
    text_dim: Option<ThemeColor>,
    selection_bg: Option<ThemeColor>,
    active_bar: Option<ThemeColor>,
    error: Option<ThemeColor>,
    highlight: Option<ThemeColor>,
    border: Option<ThemeColor>,
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Theme, D::Error> {
        let raw = ThemeRaw::deserialize(d)?;
        let base = raw.base.unwrap_or(ThemeBase::Dark);
        let def = Theme::defaults_for(base, raw.name.clone().unwrap_or_default());
        Ok(Theme {
            name: raw.name.unwrap_or(def.name),
            base,
            accent: raw.accent.unwrap_or(def.accent),
            panel_bg: raw.panel_bg.unwrap_or(def.panel_bg),
            row_bg: raw.row_bg.unwrap_or(def.row_bg),
            row_alt_bg: raw.row_alt_bg.unwrap_or(def.row_alt_bg),
            text: raw.text.unwrap_or(def.text),
            text_dim: raw.text_dim.unwrap_or(def.text_dim),
            selection_bg: raw.selection_bg.unwrap_or(def.selection_bg),
            active_bar: raw.active_bar.unwrap_or(def.active_bar),
            error: raw.error.unwrap_or(def.error),
            highlight: raw.highlight.unwrap_or(def.highlight),
            border: raw.border.unwrap_or(def.border),
        })
    }
}

impl Theme {
    /// Parsea un tema desde JSON (tolerante a campos faltantes). `None` si el JSON
    /// es inválido estructuralmente.
    pub fn from_json(text: &str) -> Option<Theme> {
        serde_json::from_str(text).ok()
    }

    /// Paleta default neutra para un `base` (relleno de campos faltantes).
    fn defaults_for(base: ThemeBase, name: String) -> Theme {
        let c = ThemeColor::new;
        match base {
            ThemeBase::Dark => Theme {
                name,
                base,
                accent: c(0x2f, 0x81, 0xf7),
                panel_bg: c(0x1e, 0x1e, 0x1e),
                row_bg: c(0x1e, 0x1e, 0x1e),
                row_alt_bg: c(0x23, 0x23, 0x23),
                text: c(0xd4, 0xd4, 0xd4),
                text_dim: c(0x9b, 0x9b, 0x9b),
                selection_bg: c(0x37, 0x37, 0x3d),
                active_bar: c(0x2f, 0x81, 0xf7),
                error: c(0xe0, 0x6c, 0x5b),
                highlight: c(0x2e, 0x7d, 0x32),
                border: c(0x3a, 0x3a, 0x3a),
            },
            ThemeBase::Light => Theme {
                name,
                base,
                accent: c(0x17, 0x69, 0xe0),
                panel_bg: c(0xfb, 0xfb, 0xfb),
                row_bg: c(0xfb, 0xfb, 0xfb),
                row_alt_bg: c(0xf4, 0xf4, 0xf4),
                text: c(0x1f, 0x1f, 0x1f),
                text_dim: c(0x6b, 0x6b, 0x6b),
                selection_bg: c(0xdc, 0xeb, 0xff),
                active_bar: c(0x17, 0x69, 0xe0),
                error: c(0xc0, 0x39, 0x2b),
                highlight: c(0xc8, 0xe6, 0xc9),
                border: c(0xdd, 0xdd, 0xdd),
            },
        }
    }
}

/// Catálogo de temas: 4 embebidos + sueltos de `<config_dir>/themes/*.json`.
pub struct ThemeCatalog {
    themes: HashMap<String, Theme>,
    available: Vec<ThemeId>,
}

const DARK_BLUE_JSON: &str = include_str!("builtin/dark-blue.json");
const DARK_TEAL_JSON: &str = include_str!("builtin/dark-teal.json");
const LIGHT_JSON: &str = include_str!("builtin/light.json");
const HIGH_CONTRAST_JSON: &str = include_str!("builtin/high-contrast.json");

impl ThemeCatalog {
    /// Id del tema por defecto (Dark Blue).
    pub fn default_id() -> ThemeId {
        ThemeId::new("dark-blue")
    }

    /// Carga embebidos + sueltos de `<dir>/themes/*.json`. `active` se acepta por
    /// simetría con i18n (el llamador resuelve con `get`).
    pub fn load(dir: &Path, _active: &ThemeId) -> ThemeCatalog {
        let mut themes: HashMap<String, Theme> = HashMap::new();
        for (id, json) in [
            ("dark-blue", DARK_BLUE_JSON),
            ("dark-teal", DARK_TEAL_JSON),
            ("light", LIGHT_JSON),
            ("high-contrast", HIGH_CONTRAST_JSON),
        ] {
            if let Some(t) = Theme::from_json(json) {
                themes.insert(id.to_string(), t);
            }
        }
        let theme_dir = dir.join("themes");
        if let Ok(entries) = std::fs::read_dir(&theme_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(text) = std::fs::read_to_string(&path) {
                            if let Some(t) = Theme::from_json(&text) {
                                themes.insert(id.to_string(), t);
                            }
                        }
                    }
                }
            }
        }
        let mut available: Vec<ThemeId> = themes.keys().map(|k| ThemeId::new(k)).collect();
        available.sort_by(|a, b| a.0.cmp(&b.0));
        ThemeCatalog { themes, available }
    }

    /// Ids disponibles (ordenados).
    pub fn available(&self) -> &[ThemeId] {
        &self.available
    }

    /// Tema por id; si no existe, el default (dark-blue). Nunca panic.
    pub fn get(&self, id: &ThemeId) -> &Theme {
        self.themes
            .get(id.as_str())
            .or_else(|| self.themes.get(Self::default_id().as_str()))
            .expect("el tema default embebido siempre existe")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_hex_con_y_sin_almohadilla() {
        assert_eq!(
            ThemeColor::from_hex("#2f81f7"),
            Some(ThemeColor::new(0x2f, 0x81, 0xf7))
        );
        assert_eq!(
            ThemeColor::from_hex("2f81f7"),
            Some(ThemeColor::new(0x2f, 0x81, 0xf7))
        );
        assert_eq!(
            ThemeColor::from_hex("#2F81F7"),
            Some(ThemeColor::new(0x2f, 0x81, 0xf7))
        );
    }

    #[test]
    fn from_hex_invalido_es_none() {
        assert_eq!(ThemeColor::from_hex("#xyz"), None);
        assert_eq!(ThemeColor::from_hex("#12345"), None);
        assert_eq!(ThemeColor::from_hex(""), None);
    }

    #[test]
    fn to_hex_round_trip() {
        let c = ThemeColor::new(0x12, 0xab, 0xff);
        assert_eq!(c.to_hex(), "#12abff");
        assert_eq!(ThemeColor::from_hex(&c.to_hex()), Some(c));
    }

    #[test]
    fn serde_color_es_cadena_hex() {
        let c = ThemeColor::new(0x2f, 0x81, 0xf7);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"#2f81f7\"");
        let back: ThemeColor = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn serde_base_lowercase() {
        assert_eq!(serde_json::to_string(&ThemeBase::Dark).unwrap(), "\"dark\"");
        let b: ThemeBase = serde_json::from_str("\"light\"").unwrap();
        assert_eq!(b, ThemeBase::Light);
    }

    #[test]
    fn theme_round_trip_serde() {
        let t = Theme {
            name: "Test".into(),
            base: ThemeBase::Dark,
            accent: ThemeColor::new(1, 2, 3),
            panel_bg: ThemeColor::new(4, 5, 6),
            row_bg: ThemeColor::new(7, 8, 9),
            row_alt_bg: ThemeColor::new(10, 11, 12),
            text: ThemeColor::new(13, 14, 15),
            text_dim: ThemeColor::new(16, 17, 18),
            selection_bg: ThemeColor::new(19, 20, 21),
            active_bar: ThemeColor::new(22, 23, 24),
            error: ThemeColor::new(25, 26, 27),
            highlight: ThemeColor::new(31, 32, 33),
            border: ThemeColor::new(28, 29, 30),
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn theme_tolera_campos_faltantes() {
        let json = r#"{ "name": "Min", "base": "dark" }"#;
        let t: Theme = Theme::from_json(json).expect("parsea con defaults");
        assert_eq!(t.name, "Min");
        assert_eq!(t.base, ThemeBase::Dark);
        let _ = t.accent;
    }

    #[test]
    fn highlight_default_si_falta_en_json() {
        // Un JSON de tema sin "highlight" cae al default del tema (tolerante, no paniquea).
        let json = r##"{"name":"X","base":"dark","accent":"#2f81f7"}"##;
        let t: Theme = serde_json::from_str(json).unwrap();
        let _ = t.highlight; // existe y tiene algún valor
    }

    #[test]
    fn highlight_round_trip() {
        let t = Theme::defaults_for(ThemeBase::Dark, "X".into());
        let json = serde_json::to_string(&t).unwrap();
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(back.highlight, t.highlight);
    }

    #[test]
    fn catalog_tiene_los_cuatro_embebidos() {
        let cat = ThemeCatalog::load(
            std::path::Path::new("Z:/no/existe"),
            &ThemeCatalog::default_id(),
        );
        let ids: Vec<&str> = cat.available().iter().map(|i| i.as_str()).collect();
        for id in ["dark-blue", "dark-teal", "light", "high-contrast"] {
            assert!(ids.contains(&id), "falta el tema embebido {id}");
        }
    }

    #[test]
    fn catalog_default_es_dark_blue() {
        assert_eq!(ThemeCatalog::default_id(), ThemeId::new("dark-blue"));
    }

    #[test]
    fn catalog_get_id_desconocido_cae_al_default() {
        let cat = ThemeCatalog::load(
            std::path::Path::new("Z:/no/existe"),
            &ThemeCatalog::default_id(),
        );
        let t = cat.get(&ThemeId::new("no-existe"));
        assert_eq!(t.name, "Dark Blue");
    }

    #[test]
    fn embebidos_parsean_sin_panic() {
        let cat = ThemeCatalog::load(
            std::path::Path::new("Z:/no/existe"),
            &ThemeCatalog::default_id(),
        );
        assert_eq!(cat.get(&ThemeId::new("light")).base, ThemeBase::Light);
        assert_eq!(
            cat.get(&ThemeId::new("high-contrast")).name,
            "High Contrast"
        );
    }
}
