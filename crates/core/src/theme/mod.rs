// Naygo — sistema de temas (color sets): datos puros, sin egui ni Windows.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Un `Theme` es una paleta de colores con nombre + base claro/oscuro. Los temas se
//! cargan de JSON embebidos y de archivos sueltos (patrón i18n), son tolerantes a
//! campos faltantes, y se aplican en caliente. Puro y testeable; la traducción a
//! `egui::Visuals` vive en la capa `ui`.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_hex_con_y_sin_almohadilla() {
        assert_eq!(ThemeColor::from_hex("#2f81f7"), Some(ThemeColor::new(0x2f, 0x81, 0xf7)));
        assert_eq!(ThemeColor::from_hex("2f81f7"), Some(ThemeColor::new(0x2f, 0x81, 0xf7)));
        assert_eq!(ThemeColor::from_hex("#2F81F7"), Some(ThemeColor::new(0x2f, 0x81, 0xf7)));
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
}
