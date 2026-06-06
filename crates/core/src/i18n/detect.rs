// Naygo — detección del idioma por defecto a partir del locale del SO (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `pick_default_language` recibe el string de locale del SO (p. ej. "es-CL") ya
//! leído por la capa `platform`, y la lista de idiomas disponibles, y elige el
//! idioma por defecto. Puro y testeable: la lectura real del SO NO ocurre aquí.

use crate::i18n::LangId;

/// Elige el idioma por defecto a partir del `locale` del SO y los `available`.
/// Regla: si el prefijo del locale (antes de '-' o '_') matchea un idioma
/// disponible, usar ese; si el locale empieza con "es", usar "es"; en cualquier
/// otro caso, "en" (fallback internacional). Case-insensitive.
pub fn pick_default_language(locale: &str, available: &[LangId]) -> LangId {
    let lower = locale.to_ascii_lowercase();
    let prefix = lower.split(['-', '_']).next().unwrap_or("").to_string();

    // ¿Hay un idioma disponible que matchee el prefijo exacto?
    if let Some(found) = available.iter().find(|l| l.as_str() == prefix) {
        return found.clone();
    }
    // Español explícito.
    if prefix == "es" {
        return LangId::new("es");
    }
    // Fallback internacional.
    LangId::new("en")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn avail() -> Vec<LangId> {
        vec![LangId::new("es"), LangId::new("en")]
    }

    #[test]
    fn es_cl_elige_es() {
        assert_eq!(pick_default_language("es-CL", &avail()), LangId::new("es"));
    }

    #[test]
    fn es_a_secas_elige_es() {
        assert_eq!(pick_default_language("es", &avail()), LangId::new("es"));
    }

    #[test]
    fn en_us_elige_en() {
        assert_eq!(pick_default_language("en-US", &avail()), LangId::new("en"));
    }

    #[test]
    fn fr_fr_cae_a_en() {
        assert_eq!(pick_default_language("fr-FR", &avail()), LangId::new("en"));
    }

    #[test]
    fn vacio_cae_a_en() {
        assert_eq!(pick_default_language("", &avail()), LangId::new("en"));
    }

    #[test]
    fn es_con_underscore_y_mayusculas() {
        assert_eq!(pick_default_language("ES_es", &avail()), LangId::new("es"));
    }

    #[test]
    fn idioma_disponible_extra_se_usa_si_matchea() {
        let av = vec![LangId::new("es"), LangId::new("en"), LangId::new("fr")];
        assert_eq!(pick_default_language("fr-FR", &av), LangId::new("fr"));
    }
}
