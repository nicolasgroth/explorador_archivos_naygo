// Naygo — identidad léxica de rutas según la plataforma, sin consultar el filesystem.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use std::ffi::OsStr;
use std::path::Path;
#[cfg(not(windows))]
use std::path::PathBuf;

/// Clave de identidad léxica que conserva los bytes nativos cuando el SO distingue nombres.
/// No resuelve symlinks, aliases, mayúsculas por directorio ni cambios concurrentes del disco.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathIdentityKey {
    #[cfg(windows)]
    normalized: String,
    #[cfg(not(windows))]
    native: PathBuf,
}

/// Obtiene una clave estable sin `to_string_lossy` en Unix. Windows conserva la política
/// case-insensitive histórica de Naygo; POSIX compara componentes nativos de forma exacta.
pub fn key(path: &Path) -> PathIdentityKey {
    #[cfg(windows)]
    {
        PathIdentityKey {
            normalized: path.to_string_lossy().replace('/', "\\").to_lowercase(),
        }
    }
    #[cfg(not(windows))]
    {
        PathIdentityKey {
            native: path.to_path_buf(),
        }
    }
}

/// Compara un componente sin convertir bytes POSIX no UTF-8 a texto sustituto.
pub fn component_eq(left: &OsStr, right: &OsStr) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

/// Prueba pertenencia léxica a un directorio padre. No resuelve symlinks ni accede al disco.
pub fn is_under(path: &Path, parent: &Path) -> bool {
    #[cfg(windows)]
    {
        let path = key(path);
        let parent = key(parent);
        let Some(tail) = path.normalized.strip_prefix(&parent.normalized) else {
            return false;
        };
        !tail.is_empty() && (parent.normalized.ends_with('\\') || tail.starts_with('\\'))
    }
    #[cfg(not(windows))]
    {
        path != parent && path.starts_with(parent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn posix_identity_preserves_case_and_non_utf8_components() {
        use std::os::unix::ffi::OsStrExt;

        assert_ne!(key(Path::new("/tmp/a")), key(Path::new("/tmp/A")));
        assert_ne!(
            key(Path::new(OsStr::from_bytes(b"/tmp/name-\xff"))),
            key(Path::new(OsStr::from_bytes(b"/tmp/name-\xfe"))),
        );
        assert!(!component_eq(OsStr::new("a"), OsStr::new("A")));
    }

    #[cfg(windows)]
    #[test]
    fn windows_identity_keeps_legacy_case_and_separator_behavior() {
        assert_eq!(key(Path::new(r"C:\Temp\A")), key(Path::new("c:/temp/a")));
        assert!(component_eq(OsStr::new("A"), OsStr::new("a")));
    }

    #[test]
    fn parent_check_requires_a_component_boundary() {
        assert!(is_under(
            Path::new("/tmp/parent/child"),
            Path::new("/tmp/parent")
        ));
        assert!(!is_under(
            Path::new("/tmp/parentish"),
            Path::new("/tmp/parent")
        ));
        assert!(!is_under(
            Path::new("/tmp/parent"),
            Path::new("/tmp/parent")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn parent_check_accepts_a_drive_root() {
        assert!(is_under(Path::new(r"C:\folder"), Path::new(r"C:\")));
    }
}
