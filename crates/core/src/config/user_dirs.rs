// Naygo — resolución portable de directorios XDG del usuario.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Resuelve los directorios de configuración, datos, caché y estado del usuario en Linux.
//!
//! La resolución no crea directorios ni hace I/O. Las rutas XDG relativas se ignoran según
//! la especificación; los fallbacks se construyen bajo un `HOME` absoluto y nunca usan el cwd
//! ni el directorio de instalación. La capa de aplicación debe crear/escribir estas rutas en
//! workers y conservar el comportamiento portable existente en Windows.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const APP_DIRECTORY: &str = "naygo";

/// Directorios XDG resueltos para una sesión de usuario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserDirs {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
}

/// Error al resolver rutas de usuario sin un `HOME` absoluto válido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserDirsError {
    MissingAbsoluteHome,
}

impl std::fmt::Display for UserDirsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingAbsoluteHome => {
                f.write_str("HOME falta o no es una ruta absoluta; no se pueden resolver rutas XDG")
            }
        }
    }
}

impl std::error::Error for UserDirsError {}

impl UserDirs {
    /// Resuelve los directorios que debe usar la instancia de producción. Linux usa XDG;
    /// Windows conserva la configuración portable junto al ejecutable. No crea directorios ni
    /// hace I/O, por lo que la creación puede diferirse a los workers que escriben cada archivo.
    pub fn for_current_platform() -> Result<Self, UserDirsError> {
        #[cfg(target_os = "linux")]
        {
            Self::from_environment()
        }

        #[cfg(not(target_os = "linux"))]
        {
            let portable = super::portable_dir();
            Ok(Self {
                config: portable.clone(),
                data: portable.clone(),
                cache: portable.clone(),
                state: portable,
            })
        }
    }

    /// Resuelve XDG desde el entorno actual. No modifica el entorno ni toca el filesystem.
    pub fn from_environment() -> Result<Self, UserDirsError> {
        Self::from_values(
            std::env::var_os("HOME").as_deref(),
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            std::env::var_os("XDG_DATA_HOME").as_deref(),
            std::env::var_os("XDG_CACHE_HOME").as_deref(),
            std::env::var_os("XDG_STATE_HOME").as_deref(),
        )
    }

    /// Resuelve rutas desde valores inyectados para que los tests no muten el entorno global.
    pub fn from_values(
        home: Option<&OsStr>,
        config_home: Option<&OsStr>,
        data_home: Option<&OsStr>,
        cache_home: Option<&OsStr>,
        state_home: Option<&OsStr>,
    ) -> Result<Self, UserDirsError> {
        let home = home
            .map(Path::new)
            .filter(|path| is_absolute(path))
            .ok_or(UserDirsError::MissingAbsoluteHome)?;

        Ok(Self {
            config: resolve_base(config_home, home.join(".config")),
            data: resolve_base(data_home, home.join(".local/share")),
            cache: resolve_base(cache_home, home.join(".cache")),
            state: resolve_base(state_home, home.join(".local/state")),
        }
        .append_app_directory())
    }

    fn append_app_directory(self) -> Self {
        Self {
            config: self.config.join(APP_DIRECTORY),
            data: self.data.join(APP_DIRECTORY),
            cache: self.cache.join(APP_DIRECTORY),
            state: self.state.join(APP_DIRECTORY),
        }
    }
}

fn resolve_base(value: Option<&OsStr>, fallback: PathBuf) -> PathBuf {
    value
        .map(Path::new)
        .filter(|path| is_absolute(path))
        .map(Path::to_path_buf)
        .unwrap_or(fallback)
}

/// `has_root` accepts POSIX `/...` paths while unit tests run on Windows, where
/// `Path::is_absolute` additionally requires a drive/UNC prefix.
fn is_absolute(path: &Path) -> bool {
    path.has_root()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(value: &str) -> &OsStr {
        OsStr::new(value)
    }

    #[test]
    fn xdg_paths_are_absolute_and_use_the_naygo_subdirectory() {
        let dirs = UserDirs::from_values(
            Some(os("/home/nico")),
            Some(os("/mnt/config")),
            Some(os("/mnt/data")),
            Some(os("/mnt/cache")),
            Some(os("/mnt/state")),
        )
        .unwrap();

        assert_eq!(dirs.config, PathBuf::from("/mnt/config/naygo"));
        assert_eq!(dirs.data, PathBuf::from("/mnt/data/naygo"));
        assert_eq!(dirs.cache, PathBuf::from("/mnt/cache/naygo"));
        assert_eq!(dirs.state, PathBuf::from("/mnt/state/naygo"));
    }

    #[test]
    fn missing_or_relative_xdg_values_fall_back_under_absolute_home() {
        let dirs = UserDirs::from_values(
            Some(os("/home/nico")),
            Some(os("relative-config")),
            None,
            Some(os("relative-cache")),
            Some(os("relative-state")),
        )
        .unwrap();

        assert_eq!(dirs.config, PathBuf::from("/home/nico/.config/naygo"));
        assert_eq!(dirs.data, PathBuf::from("/home/nico/.local/share/naygo"));
        assert_eq!(dirs.cache, PathBuf::from("/home/nico/.cache/naygo"));
        assert_eq!(dirs.state, PathBuf::from("/home/nico/.local/state/naygo"));
        for path in [&dirs.config, &dirs.data, &dirs.cache, &dirs.state] {
            assert!(is_absolute(path));
            assert!(!path.starts_with("."));
        }
    }

    #[test]
    fn missing_or_relative_home_is_an_error_instead_of_a_cwd_fallback() {
        assert_eq!(
            UserDirs::from_values(None, None, None, None, None),
            Err(UserDirsError::MissingAbsoluteHome)
        );
        assert_eq!(
            UserDirs::from_values(Some(os("relative-home")), None, None, None, None),
            Err(UserDirsError::MissingAbsoluteHome)
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn production_directories_preserve_portable_layout_outside_linux() {
        let dirs = UserDirs::for_current_platform().unwrap();
        let portable = super::super::portable_dir();
        assert_eq!(dirs.config, portable);
        assert_eq!(dirs.data, portable);
        assert_eq!(dirs.cache, portable);
        assert_eq!(dirs.state, portable);
    }
}
