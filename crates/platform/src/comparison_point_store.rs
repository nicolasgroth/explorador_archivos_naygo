// Naygo — publicación atómica y retirada explícita de puntos locales.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use naygo_core::{
    comparison_point::{self, ComparisonPoint},
    task_space::{checkpoint, SpaceError},
    CancellationToken,
};
use std::path::Path;
#[cfg(test)]
#[path = "comparison_point_store_tests.rs"]
mod tests;

fn validate_path(path: &Path) -> Result<(), SpaceError> {
    if !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("naygopoint"))
    {
        return Err(SpaceError::Invalid);
    }
    Ok(())
}
/// Un punto es inmutable: crear/exportar nunca reemplaza un documento existente.
pub fn save(
    path: &Path,
    point: &ComparisonPoint,
    token: &CancellationToken,
) -> Result<String, SpaceError> {
    validate_path(path)?;
    checkpoint(token)?;
    crate::task_space_store::write_document(path, &point.to_bytes()?, None, token, |path, token| {
        comparison_point::read(path, token).map(|(_, revision)| revision)
    })
}
/// Sólo retira el documento abierto y sin cambios. Nunca borra sus archivos referenciados.
pub fn recycle(path: &Path, expected: &str, token: &CancellationToken) -> Result<(), SpaceError> {
    validate_path(path)?;
    // La papelera del Shell resuelve rutas físicas: no retirar el destino de un enlace.
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(SpaceError::Invalid);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(SpaceError::Invalid);
        }
    }
    let (_, revision) = comparison_point::read(path, token)?;
    if revision != expected {
        return Err(SpaceError::Changed);
    }
    checkpoint(token)?;
    let probe = token.clone();
    let (tx, _rx) = std::sync::mpsc::channel();
    let receipts = crate::trash::move_to_trash_with_progress(
        &[path.to_owned()],
        tx,
        std::sync::Arc::new(move || probe.is_cancelled()),
    )
    .map_err(|e| SpaceError::Io(std::io::Error::other(format!("{e:?}"))))?;
    if receipts.is_empty() {
        return Err(SpaceError::Cancelled);
    }
    Ok(())
}
