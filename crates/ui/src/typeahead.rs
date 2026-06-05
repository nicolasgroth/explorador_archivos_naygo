// Naygo — type-ahead: saltar a la entrada cuyo nombre empieza con lo tipeado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica pura del type-ahead, separada para testearla sin egui. Dada la lista de
//! nombres, el prefijo tipeado y desde qué índice buscar, devuelve el índice de la
//! primera coincidencia (case-insensitive), envolviendo al inicio si hace falta.

/// Busca la primera entrada cuyo nombre empieza con `prefix` (case-insensitive),
/// empezando en `start` y dando la vuelta. Devuelve `None` si nada coincide.
pub fn find_match(names: &[String], prefix: &str, start: usize) -> Option<usize> {
    if names.is_empty() || prefix.is_empty() {
        return None;
    }
    let prefix_lower = prefix.to_lowercase();
    let n = names.len();
    for offset in 0..n {
        let i = (start + offset) % n;
        if names[i].to_lowercase().starts_with(&prefix_lower) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        ["Apple", "banana", "Blueberry", "cherry"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn salta_a_la_primera_coincidencia() {
        assert_eq!(find_match(&names(), "b", 0), Some(1)); // banana
    }

    #[test]
    fn es_case_insensitive() {
        assert_eq!(find_match(&names(), "BL", 0), Some(2)); // Blueberry
    }

    #[test]
    fn da_la_vuelta_buscando_desde_el_medio() {
        // Desde índice 3 (cherry), buscar "a" debe envolver hasta Apple (0).
        assert_eq!(find_match(&names(), "a", 3), Some(0));
    }

    #[test]
    fn sin_coincidencia_devuelve_none() {
        assert_eq!(find_match(&names(), "z", 0), None);
    }
}
