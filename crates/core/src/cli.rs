// Naygo — parseo de argumentos de línea de comandos (carpeta inicial).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Decide la carpeta inicial de Naygo a partir de los argumentos. Soporta
//! `naygo.exe <ruta>`: si el primer argumento posicional es un directorio existente,
//! esa es la carpeta inicial; en cualquier otro caso (ausente, archivo, inexistente,
//! vacío) no hay override y la app arranca en su carpeta por defecto.

use std::ffi::{OsStr, OsString};
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
    pub dir_arg_raw: Option<OsString>,
    pub theme: Option<String>,
    pub layout: Option<String>,
    pub help: bool,
    pub version: bool,
    /// Arrancar minimizado en la bandeja (sin mostrar la ventana). Lo escribe la entrada
    /// Run del registro cuando `autostart_minimized` está activo.
    pub tray: bool,
    /// Pide a una instancia ya abierta que termine limpiamente y sale. Uso interno del instalador.
    pub shutdown: bool,
}

/// Parsea los argumentos (SIN el ejecutable). `is_dir` valida la carpeta posicional (inyectable
/// para test puro). Reglas: `--help`/`--version`/`--tray` ponen su flag; `--theme <v>`/
/// `--layout <v>` consumen el siguiente token como valor (si no hay, se ignoran); el primer
/// token que no sea flag ni valor-de-flag es la carpeta posicional. Flags desconocidos se
/// ignoran. Nunca panic.
pub fn parse_args(args: &[String], is_dir: impl Fn(&Path) -> bool) -> CliArgs {
    let args_os = args.iter().map(OsString::from).collect::<Vec<_>>();
    parse_args_os(&args_os, is_dir)
}

/// Parsea argumentos nativos sin exigir UTF-8 para rutas POSIX. Los nombres de flags y los
/// valores de `--theme`/`--layout` siguen siendo texto UTF-8; un valor no representable se
/// consume y se ignora. Las rutas posicionales se conservan como `OsString`/`PathBuf`.
pub fn parse_args_os(args: &[OsString], is_dir: impl Fn(&Path) -> bool) -> CliArgs {
    let mut out = CliArgs::default();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].to_str();
        match arg {
            Some("--help" | "-h") => out.help = true,
            Some("--version" | "-v") => out.version = true,
            Some("--tray") => out.tray = true,
            Some("--shutdown") => out.shutdown = true,
            Some("--theme") => {
                if let Some(v) = args.get(i + 1) {
                    out.theme = v.to_str().map(str::to_owned);
                    i += 1;
                }
            }
            Some("--layout") => {
                if let Some(v) = args.get(i + 1) {
                    out.layout = v.to_str().map(str::to_owned);
                    i += 1;
                }
            }
            Some(value) if value.starts_with("--") => {}
            _ if !args[i].is_empty() && out.dir_arg_raw.is_none() => {
                let path = PathBuf::from(&args[i]);
                out.dir_arg_raw = Some(args[i].clone());
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

/// Representación escapada y segura para avisos. Conserva legible el texto UTF-8 y escapa
/// bytes/código units no representables; nunca sustituye una ruta inválida por U+FFFD.
pub fn display_os_argument(value: &OsStr) -> String {
    if let Some(text) = value.to_str() {
        return text.to_owned();
    }
    let debug = format!("{value:?}");
    debug
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
        .unwrap_or(&debug)
        .to_owned()
}

/// Atajo de producción para argumentos nativos: usa `Path::is_dir`.
pub fn parse_args_os_real(args: &[OsString]) -> CliArgs {
    parse_args_os(args, |p| p.is_dir())
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
        assert_eq!(a.dir_arg_raw.as_deref(), Some(OsStr::new("D:\\dir")));
        assert!(a.theme.is_none() && a.layout.is_none() && !a.help && !a.version);
    }

    #[test]
    fn parse_carpeta_invalida_dir_none_pero_raw_some() {
        let a = parse_args(&[s("D:\\no-existe")], |_| false);
        assert_eq!(a.dir, None);
        assert_eq!(a.dir_arg_raw.as_deref(), Some(OsStr::new("D:\\no-existe")));
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
    fn parse_flag_tray() {
        let a = parse_args(&[s("--tray")], |_| true);
        assert!(a.tray, "--tray activa el arranque minimizado");
        assert!(!parse_args(&[s("D:\\dir")], |_| true).tray);
        let b = parse_args(&[s("--tray"), s("D:\\dir")], |_| true);
        assert!(b.tray && b.dir == Some(PathBuf::from("D:\\dir")));
    }

    #[test]
    fn parse_flag_shutdown() {
        let a = parse_args(&[s("--shutdown")], |_| true);
        assert!(a.shutdown);
        assert!(a.dir.is_none());
    }

    #[test]
    fn parse_valor_de_theme_no_es_carpeta() {
        let a = parse_args(&[s("--theme"), s("D:\\dir")], |_| true);
        assert_eq!(a.theme.as_deref(), Some("D:\\dir"));
        assert_eq!(a.dir, None);
        assert_eq!(a.dir_arg_raw, None);
    }

    #[cfg(unix)]
    #[test]
    fn parse_args_os_preserva_ruta_posix_no_utf8() {
        use std::os::unix::ffi::OsStringExt;

        let temp = tempfile::tempdir().unwrap();
        let name = OsString::from_vec(vec![b'd', b'i', b'r', b'-', 0xff]);
        let path = temp.path().join(&name);
        std::fs::create_dir(&path).unwrap();
        let args = vec![path.as_os_str().to_os_string()];

        let parsed = parse_args_os(&args, |candidate| candidate.is_dir());

        assert_eq!(parsed.dir.as_deref(), Some(path.as_path()));
        assert_eq!(parsed.dir_arg_raw.as_deref(), Some(path.as_os_str()));
    }

    #[cfg(unix)]
    #[test]
    fn display_os_argument_escapa_bytes_no_utf8_solo_al_mostrar() {
        use std::os::unix::ffi::OsStringExt;

        assert_eq!(
            display_os_argument(OsStr::new("C:\\carpeta")),
            "C:\\carpeta"
        );
        let path = OsString::from_vec(b"/tmp/directorio-\xff".to_vec());
        let displayed = display_os_argument(&path);
        assert!(displayed.starts_with("/tmp/directorio-\\x"));
        assert!(!displayed.contains('\u{fffd}'));
    }

    #[cfg(unix)]
    #[test]
    fn parse_args_os_no_confunde_ruta_no_utf8_con_flag() {
        use std::os::unix::ffi::OsStringExt;

        let arg = OsString::from_vec(b"/tmp/--ruta-\xff".to_vec());
        let parsed = parse_args_os(std::slice::from_ref(&arg), |_| true);

        assert_eq!(parsed.dir, Some(PathBuf::from(&arg)));
        assert!(!parsed.help);
    }
}
