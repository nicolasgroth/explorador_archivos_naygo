// Naygo — parseo de argumentos de línea de comandos (carpeta inicial).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Decide la carpeta inicial de Naygo a partir de los argumentos. Soporta
//! `naygo.exe <ruta>`: si el primer argumento posicional es un directorio existente,
//! esa es la carpeta inicial; en cualquier otro caso (ausente, archivo, inexistente,
//! vacío) no hay override y la app arranca en su carpeta por defecto.

use std::path::{Path, PathBuf};

/// El primer argumento posicional, si lo hay (sin tocar disco). `args` son los
/// argumentos SIN el ejecutable (`&args[1..]`). Cadena vacía → `None`.
pub fn first_positional(args: &[String]) -> Option<&str> {
    args.iter().map(|s| s.as_str()).find(|s| !s.is_empty())
}

/// Resuelve la carpeta inicial: `Some(dir)` solo si el primer argumento es un
/// directorio existente. Validación de existencia mediante el predicado `is_dir`
/// (inyectable para test puro; en producción se pasa `|p| p.is_dir()`).
pub fn resolve_initial_dir(
    args: &[String],
    is_dir: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let candidate = first_positional(args)?;
    let path = PathBuf::from(candidate);
    if is_dir(&path) {
        Some(path)
    } else {
        None
    }
}

/// Atajo de producción: usa `Path::is_dir` real.
pub fn parse_initial_dir(args: &[String]) -> Option<PathBuf> {
    resolve_initial_dir(args, |p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_args_es_none() {
        assert_eq!(first_positional(&[]), None);
        assert_eq!(resolve_initial_dir(&[], |_| true), None);
    }

    #[test]
    fn arg_vacio_se_ignora() {
        let args = vec![String::new()];
        assert_eq!(first_positional(&args), None);
    }

    #[test]
    fn primer_arg_no_vacio() {
        let args = vec!["C:\\Users".to_string(), "ignorado".to_string()];
        assert_eq!(first_positional(&args), Some("C:\\Users"));
    }

    #[test]
    fn dir_existente_da_some() {
        let args = vec!["cualquier".to_string()];
        let got = resolve_initial_dir(&args, |_| true);
        assert_eq!(got, Some(PathBuf::from("cualquier")));
    }

    #[test]
    fn no_dir_da_none() {
        let args = vec!["cualquier".to_string()];
        assert_eq!(resolve_initial_dir(&args, |_| false), None);
    }

    #[test]
    fn dir_real_via_tempfile() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_string_lossy().to_string();
        let args = vec![p.clone()];
        assert_eq!(parse_initial_dir(&args), Some(PathBuf::from(p)));
    }

    #[test]
    fn archivo_no_es_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.txt");
        std::fs::write(&file, b"x").unwrap();
        let args = vec![file.to_string_lossy().to_string()];
        assert_eq!(parse_initial_dir(&args), None);
    }

    #[test]
    fn ruta_inexistente_es_none() {
        let args = vec!["Z:\\no\\existe\\naygo-xyz".to_string()];
        assert_eq!(parse_initial_dir(&args), None);
    }
}
