// Naygo — comprimir/extraer .zip: lógica pura (sin UI ni Windows), cancelable y testeable.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Crear y extraer archivos `.zip`. Puro: recibe rutas + un `CancellationToken` + callbacks de
//! progreso/conflicto; no conoce egui/Slint/Win32. El crate `zip` ya es dependencia (lo usa el
//! preview). Protección zip-slip al extraer (mismo criterio que `icon_pack::import`).

use crate::cancel::CancellationToken;
use std::path::{Path, PathBuf};

/// Resultado de UNA entrada procesada (para el resumen y el deshacer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveOpItem {
    /// Ruta REAL escrita: el `.zip` (comprimir) o el archivo/carpeta extraído (extraer).
    pub path: PathBuf,
    pub outcome: ArchiveOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchiveOutcome {
    Done,
    Skipped,
    Failed(String),
}

/// Decisión ante un conflicto al extraer (un archivo destino ya existe).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractConflict {
    Overwrite,
    Skip,
    KeepBoth,
    Cancel,
}

/// Error tipado de las operaciones de archivo comprimido. Ningún panic.
#[derive(Debug)]
pub enum ArchiveError {
    Io(std::io::Error),
    Zip(String),
    Cancelled,
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Io(e) => write!(f, "io: {e}"),
            ArchiveError::Zip(s) => write!(f, "zip: {s}"),
            ArchiveError::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl From<std::io::Error> for ArchiveError {
    fn from(e: std::io::Error) -> Self {
        ArchiveError::Io(e)
    }
}

/// Nombre por defecto del `.zip` a crear desde `sources`:
/// - 1 ítem (archivo o carpeta) → su nombre base + ".zip".
/// - varios ítems → "archivos.zip".
/// El nombre base de un archivo CONSERVA su parte sin extensión: "informe.txt" → "informe.zip".
/// Una carpeta usa su nombre tal cual: "proyecto" → "proyecto.zip".
pub fn default_zip_name(sources: &[PathBuf]) -> String {
    if sources.len() == 1 {
        let p = &sources[0];
        // file_stem para archivos con extensión; para carpetas (sin "extensión") file_stem == nombre.
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            return format!("{stem}.zip");
        }
    }
    "archivos.zip".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_zip_name_un_archivo_usa_su_stem() {
        assert_eq!(default_zip_name(&[PathBuf::from("C:/x/informe.txt")]), "informe.zip");
    }

    #[test]
    fn default_zip_name_una_carpeta_usa_su_nombre() {
        assert_eq!(default_zip_name(&[PathBuf::from("C:/x/proyecto")]), "proyecto.zip");
    }

    #[test]
    fn default_zip_name_varios_es_generico() {
        let v = vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")];
        assert_eq!(default_zip_name(&v), "archivos.zip");
    }
}
