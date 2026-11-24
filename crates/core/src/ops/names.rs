// Naygo — nombres de archivo: validación y resolución de conflictos (puro).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Funciones puras sobre nombres: validar caracteres prohibidos en Windows y generar
//! el siguiente nombre libre ante un conflicto (`archivo (2).ext`).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Caracteres prohibidos en nombres de archivo de Windows.
#[cfg(windows)]
const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// `true` si `name` es un nombre de archivo/carpeta válido (no vacío, sin caracteres
/// prohibidos en Windows, sin ser solo espacios/puntos).
#[cfg(windows)]
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

/// En POSIX sólo la barra y NUL están prohibidos en el nombre. Espacios, puntos finales,
/// barras invertidas, comillas y controles distintos de NUL son válidos.
#[cfg(not(windows))]
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.chars().any(|c| c == '/' || c == '\0')
}

/// Separa una ruta relativa por ambos separadores de Windows.
/// No interpreta unidades ni rutas absolutas: cada trozo se valida después.
#[cfg(windows)]
fn split_relative(rel: &str) -> Vec<&str> {
    rel.split(['\\', '/']).collect()
}

/// En POSIX la barra invertida es parte del nombre y nunca divide una ruta.
#[cfg(not(windows))]
fn split_relative(rel: &str) -> Vec<&str> {
    rel.split('/').collect()
}

/// `true` si `rel` es una ruta RELATIVA segura para crear una jerarquía dentro de un
/// directorio destino (p. ej. `a/b/c`, o `a\b\c` en Windows). Reglas:
/// - Cada componente, separado por la sintaxis nativa de ruta, debe pasar [`is_valid_name`].
/// - Ningún componente puede ser `.` ni `..` (no se permite escapar del destino).
/// - Ningún componente puede ser vacío (rechaza separadores dobles y rutas absolutas).
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
/// si la ruta no es válida (ver [`is_valid_relative_path`]). No altera el contenido de nombres.
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
    // Componer con OsStr conserva bytes Unix no UTF-8 y nombres Windows almacenados como UTF-16.
    let file_name = candidate.file_name().unwrap_or_else(|| OsStr::new(""));
    let stem = candidate.file_stem().unwrap_or(file_name);
    let ext = candidate.extension();
    let mut n = 2u32;
    loop {
        let mut name = OsString::from(stem);
        name.push(format!(" ({n})"));
        if let Some(ext) = ext {
            name.push(".");
            name.push(ext);
        }
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
    #[cfg(windows)]
    fn nombre_invalido_caracteres_prohibidos() {
        for bad in [
            "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b",
        ] {
            assert!(!is_valid_name(bad), "{bad} debería ser inválido");
        }
        assert!(!is_valid_name(""), "vacío inválido");
    }

    #[test]
    #[cfg(windows)]
    fn nombre_invalido_espacio_o_punto_final() {
        // Windows recorta el espacio/punto final → el nombre real diferiría del mostrado.
        assert!(!is_valid_name("informe "), "espacio final inválido");
        assert!(!is_valid_name("informe."), "punto final inválido");
        assert!(
            !is_valid_name("informe.txt "),
            "espacio tras extensión inválido"
        );
        // Nombres legítimos no deben verse afectados: un punto interno es válido.
        assert!(is_valid_name("informe.txt"), "extensión normal válida");
        assert!(is_valid_name("a.b.c"), "puntos internos válidos");
    }

    #[test]
    #[cfg(windows)]
    fn ruta_relativa_valida_acepta_anidadas() {
        // Una sola componente, y anidadas con ambos separadores.
        assert!(is_valid_relative_path("uno"));
        assert!(is_valid_relative_path("a\\b\\c"));
        assert!(is_valid_relative_path("a/b/c"));
        // Mezcla de separadores también vale.
        assert!(is_valid_relative_path("a\\b/c"));
    }

    #[test]
    #[cfg(windows)]
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
    #[cfg(windows)]
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
    #[cfg(windows)]
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

    #[test]
    #[cfg(unix)]
    fn posix_permite_caracteres_validos_y_rechaza_separadores_especiales() {
        for name in [
            "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", " ", "a.",
        ] {
            assert!(is_valid_name(name), "POSIX permite {name:?}");
        }
        for name in ["", ".", "..", "a/b", "a\0b"] {
            assert!(!is_valid_name(name), "POSIX debe rechazar {name:?}");
        }
    }

    #[test]
    #[cfg(unix)]
    fn posix_ruta_relativa_conserva_backslash_como_parte_del_nombre() {
        assert_eq!(
            relative_components("a\\b/c"),
            Some(vec!["a\\b".to_string(), "c".to_string()])
        );
        assert!(is_valid_relative_path("solo\\nombre"));
        assert!(!is_valid_relative_path("a/../b"));
        assert!(!is_valid_relative_path("a//b"));
        assert!(!is_valid_relative_path("/abs"));
    }

    #[test]
    #[cfg(unix)]
    fn dedup_preserva_bytes_no_utf8_del_nombre() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let candidate = PathBuf::from(OsString::from_vec(vec![b'a', 0xff, b'.', b't', b'x', b't']));
        let out = dedup_name(&candidate, &|path| path == candidate);
        assert_eq!(
            out.file_name().unwrap().as_bytes(),
            &[b'a', 0xff, b' ', b'(', b'2', b')', b'.', b't', b'x', b't']
        );
    }
}
