// Naygo — nombres de archivo: validación y resolución de conflictos (puro).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Funciones puras sobre nombres: validar caracteres prohibidos en Windows y generar
//! el siguiente nombre libre ante un conflicto (`archivo (2).ext`).

use std::path::{Path, PathBuf};

/// Caracteres prohibidos en nombres de archivo de Windows.
const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// `true` si `name` es un nombre de archivo/carpeta válido (no vacío, sin caracteres
/// prohibidos en Windows, sin ser solo espacios/puntos).
pub fn is_valid_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if name.trim().is_empty() || name.chars().all(|c| c == '.') {
        return false;
    }
    // Windows recorta silenciosamente el espacio o punto FINAL del nombre completo
    // (p. ej. "informe " e "informe." quedan como "informe" en disco). Rechazarlos
    // evita una vista previa engañosa: dos filas distintas que en disco colisionan.
    // Un punto interno (como en "a.b.txt") sigue siendo válido; solo importa el final.
    if name.ends_with(' ') || name.ends_with('.') {
        return false;
    }
    !name
        .chars()
        .any(|c| FORBIDDEN.contains(&c) || (c as u32) < 0x20)
}

/// Separa una ruta relativa en sus componentes, aceptando `\` y `/` como divisores.
/// No interpreta unidades ni rutas absolutas: cada trozo se valida después.
fn split_relative(rel: &str) -> Vec<&str> {
    rel.split(['\\', '/']).collect()
}

/// `true` si `rel` es una ruta RELATIVA segura para crear una jerarquía dentro de un
/// directorio destino (p. ej. `a\b\c` o `a/b/c`). Reglas:
/// - Cada componente (separado por `\` o `/`) debe pasar [`is_valid_name`].
/// - Ningún componente puede ser `.` ni `..` (no se permite escapar del destino).
/// - Ningún componente puede ser vacío (rechaza `a\\b`, rutas que empiezan o terminan
///   en separador, y por tanto rutas absolutas tipo `\abs`).
///
/// Es la validación por-componente que usa el plan de `CreateDir`/`CreateFile`, en vez de
/// rechazar el string completo con [`is_valid_name`] (que prohíbe los separadores a propósito).
pub fn is_valid_relative_path(rel: &str) -> bool {
    if rel.is_empty() {
        return false;
    }
    let parts = split_relative(rel);
    // `split` siempre devuelve al menos un elemento; basta con validar cada componente.
    parts
        .iter()
        .all(|part| *part != "." && *part != ".." && is_valid_name(part))
}

/// Descompone `rel` en sus componentes validados como ruta relativa segura. Devuelve `None`
/// si la ruta no es válida (ver [`is_valid_relative_path`]). Los componentes se recortan de
/// espacios igual que [`is_valid_name`] los acepta, pero NO se altera su contenido.
pub fn relative_components(rel: &str) -> Option<Vec<String>> {
    if !is_valid_relative_path(rel) {
        return None;
    }
    Some(split_relative(rel).iter().map(|s| s.to_string()).collect())
}

/// Dada una ruta destino candidata y un predicado `exists`, devuelve la primera ruta
/// libre añadiendo " (N)" antes de la extensión si hace falta. Pura: el caller provee
/// `exists` (en tests es un set; en el motor es `Path::exists`).
pub fn dedup_name(candidate: &Path, exists: &dyn Fn(&Path) -> bool) -> PathBuf {
    if !exists(candidate) {
        return candidate.to_path_buf();
    }
    let dir = candidate
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let stem = candidate
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let ext = candidate
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());
    let mut n = 2u32;
    loop {
        let name = match &ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        let cand = dir.join(name);
        if !exists(&cand) {
            return cand;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn nombre_valido() {
        assert!(is_valid_name("informe.pdf"));
        assert!(is_valid_name("Carpeta nueva"));
    }

    #[test]
    fn nombre_invalido_caracteres_prohibidos() {
        for bad in [
            "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b",
        ] {
            assert!(!is_valid_name(bad), "{bad} debería ser inválido");
        }
        assert!(!is_valid_name(""), "vacío inválido");
    }

    #[test]
    fn nombre_invalido_espacio_o_punto_final() {
        // Windows recorta el espacio/punto final → el nombre real diferiría del mostrado.
        assert!(!is_valid_name("informe "), "espacio final inválido");
        assert!(!is_valid_name("informe."), "punto final inválido");
        assert!(!is_valid_name("informe.txt "), "espacio tras extensión inválido");
        // Nombres legítimos no deben verse afectados: un punto interno es válido.
        assert!(is_valid_name("informe.txt"), "extensión normal válida");
        assert!(is_valid_name("a.b.c"), "puntos internos válidos");
    }

    #[test]
    fn ruta_relativa_valida_acepta_anidadas() {
        // Una sola componente, y anidadas con ambos separadores.
        assert!(is_valid_relative_path("uno"));
        assert!(is_valid_relative_path("a\\b\\c"));
        assert!(is_valid_relative_path("a/b/c"));
        // Mezcla de separadores también vale.
        assert!(is_valid_relative_path("a\\b/c"));
    }

    #[test]
    fn ruta_relativa_rechaza_traversal_y_absoluta() {
        // `.` y `..` en cualquier posición no se permiten (no escapar del destino).
        assert!(!is_valid_relative_path("a\\..\\b"), ".. prohibido");
        assert!(!is_valid_relative_path("..\\b"), ".. al inicio prohibido");
        assert!(!is_valid_relative_path("a\\."), ". prohibido");
        // Empezar con separador (ruta absoluta tipo `\abs`) → primer componente vacío.
        assert!(!is_valid_relative_path("\\abs"), "absoluta prohibida");
        assert!(!is_valid_relative_path("/abs"), "absoluta prohibida");
    }

    #[test]
    fn ruta_relativa_rechaza_componente_vacio_y_chars() {
        // Doble separador → componente vacío.
        assert!(!is_valid_relative_path("a\\\\b"), "doble sep prohibido");
        // Termina en separador → último componente vacío.
        assert!(!is_valid_relative_path("a\\"), "termina en sep prohibido");
        // Cadena vacía.
        assert!(!is_valid_relative_path(""), "vacío prohibido");
        // Carácter prohibido en un segmento.
        assert!(
            !is_valid_relative_path("a\\b:c"),
            "char prohibido en segmento"
        );
        assert!(
            !is_valid_relative_path("a\\b*c"),
            "char prohibido en segmento"
        );
    }

    #[test]
    fn relative_components_descompone_y_rechaza() {
        assert_eq!(
            relative_components("a\\b\\c"),
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
        // El `/` también separa.
        assert_eq!(
            relative_components("a/b"),
            Some(vec!["a".to_string(), "b".to_string()])
        );
        // Una sola componente.
        assert_eq!(relative_components("solo"), Some(vec!["solo".to_string()]));
        // Inválidas devuelven None.
        assert_eq!(relative_components("a\\..\\b"), None);
        assert_eq!(relative_components("a\\\\b"), None);
        assert_eq!(relative_components(""), None);
    }

    #[test]
    fn dedup_sin_conflicto_devuelve_igual() {
        let exists = |_p: &std::path::Path| false;
        let out = dedup_name(&PathBuf::from("C:/x/a.txt"), &exists);
        assert_eq!(out, PathBuf::from("C:/x/a.txt"));
    }

    #[test]
    fn dedup_con_conflicto_agrega_sufijo() {
        let taken: HashSet<PathBuf> = [PathBuf::from("C:/x/a.txt")].into_iter().collect();
        let exists = |p: &std::path::Path| taken.contains(p);
        let out = dedup_name(&PathBuf::from("C:/x/a.txt"), &exists);
        assert_eq!(out, PathBuf::from("C:/x/a (2).txt"));
    }

    #[test]
    fn dedup_incrementa_si_2_tambien_existe() {
        let taken: HashSet<PathBuf> =
            [PathBuf::from("C:/x/a.txt"), PathBuf::from("C:/x/a (2).txt")]
                .into_iter()
                .collect();
        let exists = |p: &std::path::Path| taken.contains(p);
        let out = dedup_name(&PathBuf::from("C:/x/a.txt"), &exists);
        assert_eq!(out, PathBuf::from("C:/x/a (3).txt"));
    }

    #[test]
    fn dedup_sin_extension() {
        let taken: HashSet<PathBuf> = [PathBuf::from("C:/x/LEEME")].into_iter().collect();
        let exists = |p: &std::path::Path| taken.contains(p);
        let out = dedup_name(&PathBuf::from("C:/x/LEEME"), &exists);
        assert_eq!(out, PathBuf::from("C:/x/LEEME (2)"));
    }
}
