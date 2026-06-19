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
pub fn resolve_initial_dir(args: &[String], is_dir: impl Fn(&Path) -> bool) -> Option<PathBuf> {
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

/// Argumentos de línea de comandos ya parseados. `theme`/`layout` son las cadenas CRUDAS del
/// argumento (sin validar contra el catálogo: eso lo hace la UI, que sí lo conoce).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CliArgs {
    /// Carpeta posicional, SOLO si es un directorio existente (según `is_dir`).
    pub dir: Option<PathBuf>,
    /// El token posicional tal cual, aunque no sea un dir válido (para avisar "no es carpeta").
    pub dir_arg_raw: Option<String>,
    pub theme: Option<String>,
    pub layout: Option<String>,
    pub help: bool,
    pub version: bool,
}

/// Parsea los argumentos (SIN el ejecutable). `is_dir` valida la carpeta posicional (inyectable
/// para test puro). Reglas: `--help`/`--version` ponen su flag; `--theme <v>`/`--layout <v>`
/// consumen el siguiente token como valor (si no hay, se ignoran); el primer token que no sea
/// flag ni valor-de-flag es la carpeta posicional. Flags desconocidos se ignoran. Nunca panic.
pub fn parse_args(args: &[String], is_dir: impl Fn(&Path) -> bool) -> CliArgs {
    let mut out = CliArgs::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--help" | "-h" => out.help = true,
            "--version" | "-v" => out.version = true,
            "--theme" => {
                if let Some(v) = args.get(i + 1) {
                    out.theme = Some(v.clone());
                    i += 1;
                }
            }
            "--layout" => {
                if let Some(v) = args.get(i + 1) {
                    out.layout = Some(v.clone());
                    i += 1;
                }
            }
            _ if arg.starts_with("--") => {}
            _ if !arg.is_empty() && out.dir_arg_raw.is_none() => {
                out.dir_arg_raw = Some(arg.to_string());
                let path = PathBuf::from(arg);
                if is_dir(&path) {
                    out.dir = Some(path);
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Atajo de producción: usa `Path::is_dir`.
pub fn parse_args_real(args: &[String]) -> CliArgs {
    parse_args(args, |p| p.is_dir())
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

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn parse_sin_args_todo_vacio() {
        assert_eq!(parse_args(&[], |_| true), CliArgs::default());
    }

    #[test]
    fn parse_solo_carpeta_valida() {
        let a = parse_args(&[s("D:\\dir")], |_| true);
        assert_eq!(a.dir, Some(PathBuf::from("D:\\dir")));
        assert_eq!(a.dir_arg_raw.as_deref(), Some("D:\\dir"));
        assert!(a.theme.is_none() && a.layout.is_none() && !a.help && !a.version);
    }

    #[test]
    fn parse_carpeta_invalida_dir_none_pero_raw_some() {
        let a = parse_args(&[s("D:\\no-existe")], |_| false);
        assert_eq!(a.dir, None);
        assert_eq!(a.dir_arg_raw.as_deref(), Some("D:\\no-existe"));
    }

    #[test]
    fn parse_theme_y_layout() {
        let a = parse_args(
            &[s("--theme"), s("winxp"), s("--layout"), s("Mi plantilla")],
            |_| true,
        );
        assert_eq!(a.theme.as_deref(), Some("winxp"));
        assert_eq!(a.layout.as_deref(), Some("Mi plantilla"));
        assert_eq!(a.dir, None);
    }

    #[test]
    fn parse_orden_mezclado() {
        let a = parse_args(
            &[s("--theme"), s("x"), s("D:\\dir"), s("--layout"), s("y")],
            |_| true,
        );
        assert_eq!(a.dir, Some(PathBuf::from("D:\\dir")));
        assert_eq!(a.theme.as_deref(), Some("x"));
        assert_eq!(a.layout.as_deref(), Some("y"));
    }

    #[test]
    fn parse_flag_sin_valor_se_ignora_sin_panic() {
        assert_eq!(parse_args(&[s("--theme")], |_| true).theme, None);
        assert_eq!(parse_args(&[s("--layout")], |_| true).layout, None);
    }

    #[test]
    fn parse_help_y_version() {
        assert!(parse_args(&[s("--help")], |_| true).help);
        assert!(parse_args(&[s("--version")], |_| true).version);
    }

    #[test]
    fn parse_flag_desconocido_se_ignora() {
        let a = parse_args(&[s("--zzz"), s("D:\\dir")], |_| true);
        assert_eq!(a.dir, Some(PathBuf::from("D:\\dir")));
    }

    #[test]
    fn parse_valor_de_theme_no_es_carpeta() {
        let a = parse_args(&[s("--theme"), s("D:\\dir")], |_| true);
        assert_eq!(a.theme.as_deref(), Some("D:\\dir"));
        assert_eq!(a.dir, None);
        assert_eq!(a.dir_arg_raw, None);
    }
}
