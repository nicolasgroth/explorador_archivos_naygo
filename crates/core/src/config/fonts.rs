// Naygo — preferencias tipográficas por función, sin incluir fuentes en distribución.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FontPreferences {
    /// Vacío: fuente del sistema elegida por el toolkit.
    pub interface: String,
    /// Vacío: heredar de la interfaz (nombres, tamaños y fechas juntos).
    pub listings: String,
    /// Vacío: Consolas, conservando alineación de texto/código.
    pub preview: String,
}

impl FontPreferences {
    pub fn set(&mut self, role: i32, family: &str) {
        let family: String = family
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(128)
            .collect();
        match role {
            0 => self.interface = family,
            1 => self.listings = family,
            2 => self.preview = family,
            _ => {}
        }
    }

    pub fn listings_family(&self) -> &str {
        if self.listings.is_empty() {
            &self.interface
        } else {
            &self.listings
        }
    }

    pub fn preview_family(&self) -> &str {
        if self.preview.is_empty() {
            "Consolas"
        } else {
            &self.preview
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_inheritance_and_round_trip() {
        let mut fonts: FontPreferences = serde_json::from_str("{}").unwrap();
        assert_eq!(fonts.listings_family(), "");
        assert_eq!(fonts.preview_family(), "Consolas");
        fonts.set(0, " Segoe UI ");
        assert_eq!(fonts.listings_family(), "Segoe UI");
        fonts.set(1, "Tahoma");
        fonts.set(2, "Cascadia Mono");
        assert_eq!(fonts.listings_family(), "Tahoma");
        let saved = serde_json::to_string(&fonts).unwrap();
        assert_eq!(
            serde_json::from_str::<FontPreferences>(&saved).unwrap(),
            fonts
        );
        fonts.set(1, "");
        assert_eq!(fonts.listings_family(), "Segoe UI");
        fonts.set(9, "ignore");
        assert_eq!(fonts.interface, "Segoe UI");
    }
}
