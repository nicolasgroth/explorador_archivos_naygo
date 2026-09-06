// Naygo — persistencia atómica de consultas guardadas, exclusivamente en workers.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use naygo_core::{
    saved_search::{self, SavedSearch},
    task_space::SpaceError,
    CancellationToken,
};
use std::path::Path;
#[cfg(test)]
#[path = "saved_search_store_tests.rs"]
mod tests;

pub fn save(
    path: &Path,
    query: &SavedSearch,
    expected: Option<&str>,
    token: &CancellationToken,
) -> Result<String, SpaceError> {
    if !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("naygosearch"))
    {
        return Err(SpaceError::Invalid);
    }
    let bytes = query.to_bytes()?;
    crate::task_space_store::write_document(path, &bytes, expected, token, |path, token| {
        saved_search::read(path, token).map(|(_, hash)| hash)
    })
}
