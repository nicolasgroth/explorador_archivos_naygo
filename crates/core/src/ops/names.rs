// Naygo — nombres de archivo: validación y resolución de conflictos (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

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
    !name.chars().any(|c| FORBIDDEN.contains(&c) || (c as u32) < 0x20)
}

/// Dada una ruta destino candidata y un predicado `exists`, devuelve la primera ruta
/// libre añadiendo " (N)" antes de la extensión si hace falta. Pura: el caller provee
/// `exists` (en tests es un set; en el motor es `Path::exists`).
pub fn dedup_name(candidate: &Path, exists: &dyn Fn(&Path) -> bool) -> PathBuf {
    if !exists(candidate) {
        return candidate.to_path_buf();
    }
    let dir = candidate.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let stem = candidate
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let ext = candidate.extension().and_then(|s| s.to_str()).map(|s| s.to_string());
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
        for bad in ["a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b"] {
            assert!(!is_valid_name(bad), "{bad} debería ser inválido");
        }
        assert!(!is_valid_name(""), "vacío inválido");
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
        let taken: HashSet<PathBuf> = [
            PathBuf::from("C:/x/a.txt"),
            PathBuf::from("C:/x/a (2).txt"),
        ].into_iter().collect();
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
