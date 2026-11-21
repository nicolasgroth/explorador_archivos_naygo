// Naygo — rutas verbatim (\\?\) para llamadas Win32 sin el límite de MAX_PATH.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Las APIs Win32 con nombre de ruta (`CopyFile2`, `MoveFileExW`, ...) fallan con rutas de más
//! de 260 caracteres salvo que el proceso declare `longPathAware` y el sistema tenga
//! `LongPathsEnabled`. El prefijo verbatim `\\?\` levanta ese límite en cualquier caso, pero
//! desactiva la normalización de Win32: por eso [`to_verbatim`] normaliza antes (separadores
//! `/` → `\`, quita `.` y resuelve `..` de forma léxica, igual que haría Win32).

use std::path::{Path, PathBuf};

/// Convierte una ruta absoluta de Windows a su forma verbatim:
///
/// - `C:\x` → `\\?\C:\x`
/// - `\\srv\sh\x` → `\\?\UNC\srv\sh\x`
///
/// Las rutas ya verbatim (`\\?\…`), de dispositivo (`\\.\…`), relativas o relativas a una
/// unidad (`C:x`) se devuelven sin cambios. En plataformas que no son Windows es la identidad.
pub fn to_verbatim(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        to_verbatim_windows(path)
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

#[cfg(windows)]
fn to_verbatim_windows(path: &Path) -> PathBuf {
    use std::path::{Component, Prefix};

    let mut components = path.components();
    let prefix = match components.next() {
        Some(Component::Prefix(prefix)) => prefix,
        _ => return path.to_path_buf(),
    };
    // Sin raíz tras el prefijo (`C:x`) la ruta depende del directorio actual de esa unidad.
    if !path.has_root() {
        return path.to_path_buf();
    }
    let mut result = std::ffi::OsString::from(match prefix.kind() {
        Prefix::Disk(letter) => format!(r"\\?\{}:", char::from(letter)),
        Prefix::UNC(server, share) => format!(
            r"\\?\UNC\{}\{}",
            server.to_string_lossy(),
            share.to_string_lossy()
        ),
        // Verbatim, VerbatimUNC, VerbatimDisk y DeviceNS: ya están en su forma final.
        _ => return path.to_path_buf(),
    });

    // Normalización léxica: es lo que Win32 hace con una ruta no verbatim, y `\\?\` la omite.
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for component in components {
        match component {
            Component::Normal(part) => parts.push(part),
            // `..` sobre la raíz se queda en la raíz, como en Win32.
            Component::ParentDir => {
                parts.pop();
            }
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    if parts.is_empty() {
        result.push(r"\");
    }
    for part in parts {
        result.push(r"\");
        result.push(part);
    }
    PathBuf::from(result)
}

#[cfg(all(test, windows))]
mod tests {
    use super::to_verbatim;
    use std::path::{Path, PathBuf};

    fn verbatim(path: &str) -> PathBuf {
        to_verbatim(Path::new(path))
    }

    #[test]
    fn unidad_absoluta_gana_prefijo_verbatim() {
        assert_eq!(verbatim(r"C:\x"), PathBuf::from(r"\\?\C:\x"));
        assert_eq!(
            verbatim(r"D:\carpeta\archivo.txt"),
            PathBuf::from(r"\\?\D:\carpeta\archivo.txt")
        );
    }

    #[test]
    fn raiz_de_unidad_conserva_la_barra() {
        assert_eq!(verbatim(r"C:\"), PathBuf::from(r"\\?\C:\"));
    }

    #[test]
    fn unc_pasa_a_verbatim_unc() {
        assert_eq!(verbatim(r"\\srv\sh\x"), PathBuf::from(r"\\?\UNC\srv\sh\x"));
        assert_eq!(verbatim(r"\\srv\sh"), PathBuf::from(r"\\?\UNC\srv\sh\"));
    }

    #[test]
    fn ya_verbatim_no_se_toca() {
        for path in [r"\\?\C:\x", r"\\?\UNC\srv\sh\x", r"\\?\Volume{abc}\x"] {
            assert_eq!(verbatim(path), PathBuf::from(path));
        }
    }

    #[test]
    fn relativa_o_de_dispositivo_no_se_toca() {
        for path in [r"x\y", "archivo.txt", r"C:x", r"\\.\COM1", r"\sin\unidad"] {
            assert_eq!(verbatim(path), PathBuf::from(path));
        }
    }

    #[test]
    fn normaliza_separadores_punto_y_punto_punto() {
        assert_eq!(verbatim(r"C:/a/./b/../c\d"), PathBuf::from(r"\\?\C:\a\c\d"));
        assert_eq!(verbatim(r"C:\..\x"), PathBuf::from(r"\\?\C:\x"));
    }

    #[test]
    fn ruta_larga_queda_utilizable_por_std() {
        let dir = tempfile::tempdir().unwrap();
        let mut long = dir.path().to_path_buf();
        while long.as_os_str().len() < 300 {
            long.push("segmento_de_ruta_larga");
        }
        std::fs::create_dir_all(&long).unwrap();
        let file = long.join("f.txt");
        std::fs::write(&file, b"x").unwrap();

        let converted = to_verbatim(&file);
        assert!(converted.as_os_str().to_string_lossy().starts_with(r"\\?\"));
        assert_eq!(std::fs::read(&converted).unwrap(), b"x");
    }
}
