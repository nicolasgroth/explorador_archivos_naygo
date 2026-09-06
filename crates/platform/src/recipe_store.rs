// Naygo — persistencia atómica de recetas, sin ejecución al importar.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use naygo_core::{
    recipe::{self, Recipe},
    task_space::SpaceError,
    CancellationToken,
};
use std::path::Path;
#[cfg(test)]
#[path = "recipe_store_tests.rs"]
mod tests;
pub fn save(
    path: &Path,
    recipe: &Recipe,
    expected: Option<&str>,
    token: &CancellationToken,
) -> Result<String, SpaceError> {
    if !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("naygorecipe"))
    {
        return Err(SpaceError::Invalid);
    }
    crate::task_space_store::write_document(
        path,
        &recipe.to_bytes()?,
        expected,
        token,
        |path, token| recipe::read(path, token).map(|(_, revision)| revision),
    )
}
