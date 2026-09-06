// Naygo — recetas declarativas: criterios portables, parámetros y planes congelados.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use crate::{
    delivery::{self, DeliveryPlan, Output, ReviewOptions, Structure},
    saved_search::{RelativeDate, SavedSearch},
    search::{SearchMsg, SearchOptions, MAX_HITS},
    task_space::{checkpoint, revision, valid_name, SpaceError},
    CancellationToken,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

pub const MAX_BYTES: usize = 256 * 1024;
#[cfg(test)]
mod tests;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Selection,
    Query,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    Flat,
    Grouped,
    Relative,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub version: u32,
    pub name: String,
    pub source: Source,
    pub name_query: String,
    pub content_query: String,
    pub ignore_case: bool,
    pub use_wildcards: bool,
    pub recursive: bool,
    pub min_bytes: Option<u64>,
    pub max_bytes: Option<u64>,
    pub modified: RelativeDate,
    /// Único parámetro de texto permitido: {date}. No es un intérprete de scripts.
    pub output_name: String,
    pub output: Output,
    pub layout: Layout,
    pub number_duplicates: bool,
    pub hashes: bool,
}
impl Recipe {
    pub fn from_query(query: &SavedSearch) -> Self {
        Self {
            version: 1,
            name: query.name.clone(),
            source: Source::Query,
            name_query: query.options.name_query.clone(),
            content_query: query.options.content_query.clone(),
            ignore_case: query.options.ignore_case,
            use_wildcards: query.options.use_wildcards,
            recursive: query.options.recursive,
            min_bytes: query.min_bytes,
            max_bytes: query.max_bytes,
            modified: query.modified,
            output_name: "delivery-{date}".into(),
            output: Output::Folder,
            layout: Layout::Flat,
            number_duplicates: false,
            hashes: false,
        }
    }
    pub fn query(&self, roots: Vec<PathBuf>) -> SavedSearch {
        SavedSearch {
            version: 1,
            name: self.name.clone(),
            roots,
            options: SearchOptions {
                name_query: self.name_query.clone(),
                content_query: self.content_query.clone(),
                ignore_case: self.ignore_case,
                use_wildcards: self.use_wildcards,
                recursive: self.recursive,
            },
            min_bytes: self.min_bytes,
            max_bytes: self.max_bytes,
            modified: self.modified,
        }
    }
    pub fn validate(&self) -> Result<(), SpaceError> {
        if self.version != 1 {
            return Err(SpaceError::Version);
        }
        if !valid_name(&self.name)
            || self
                .min_bytes
                .zip(self.max_bytes)
                .is_some_and(|(a, b)| a > b)
            || self.output_name.len() > 240
            || self.name_query.len() > 4096
            || self.content_query.len() > 4096
        {
            return Err(SpaceError::Invalid);
        }
        self.resolved_name("2026-01-01")?;
        Ok(())
    }
    pub fn resolved_name(&self, date: &str) -> Result<String, SpaceError> {
        if date.len() != 10
            || !date.bytes().enumerate().all(|(i, b)| {
                if i == 4 || i == 7 {
                    b == b'-'
                } else {
                    b.is_ascii_digit()
                }
            })
        {
            return Err(SpaceError::Invalid);
        }
        let mut name = self.output_name.replace("{date}", date);
        if name.contains(['{', '}']) {
            return Err(SpaceError::Invalid);
        }
        if self.output == Output::Zip && !name.to_lowercase().ends_with(".zip") {
            name.push_str(".zip");
        }
        if !crate::ops::names::is_valid_name(&name)
            || crate::task_space::reserved_name(&name)
            || Path::new(&name).components().count() != 1
            || name.encode_utf16().count() > 240
        {
            return Err(SpaceError::Invalid);
        }
        Ok(name)
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, SpaceError> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|_| SpaceError::Invalid)?;
        if bytes.len() > MAX_BYTES {
            return Err(SpaceError::Limit);
        }
        Ok(bytes)
    }
}
pub fn read(path: &Path, token: &CancellationToken) -> Result<(Recipe, String), SpaceError> {
    checkpoint(token)?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    checkpoint(token)?;
    if bytes.len() > MAX_BYTES {
        return Err(SpaceError::Limit);
    }
    let recipe: Recipe = serde_json::from_slice(&bytes).map_err(|_| SpaceError::Invalid)?;
    recipe.validate()?;
    Ok((recipe, revision(&bytes)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameters {
    pub roots: Vec<PathBuf>,
    pub parent: PathBuf,
}
pub struct Prepared {
    pub recipe: Recipe,
    pub parameters: Parameters,
    pub plan: DeliveryPlan,
    pub sources: Vec<PathBuf>,
    pub examined: usize,
}
pub struct EvaluationTime {
    pub now: i64,
    pub lower: Option<i64>,
    pub date: String,
}
#[derive(Debug)]
pub enum RecipeError {
    Document(SpaceError),
    Delivery(delivery::DeliveryError),
    Partial,
    Empty,
    SelectionFilesOnly,
}
impl From<SpaceError> for RecipeError {
    fn from(e: SpaceError) -> Self {
        Self::Document(e)
    }
}
impl From<delivery::DeliveryError> for RecipeError {
    fn from(e: delivery::DeliveryError) -> Self {
        Self::Delivery(e)
    }
}

/// Lectura/revisión solamente: ni crea destinos ni ejecuta copias. Llamar desde un worker.
pub fn prepare(
    recipe: Recipe,
    parameters: Parameters,
    selected: Vec<PathBuf>,
    time: EvaluationTime,
    token: &CancellationToken,
    mut progress: impl FnMut(usize),
) -> Result<Prepared, RecipeError> {
    let EvaluationTime { now, lower, date } = time;
    checkpoint(token)?;
    recipe.validate()?;
    if !parameters.parent.is_absolute()
        || parameters
            .parent
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        || parameters.parent.to_string_lossy().contains('\0')
        || (recipe.modified != RelativeDate::Any && lower.is_none())
        || parameters.roots.len() > 64
        || parameters.roots.iter().any(|p| {
            !p.is_absolute()
                || p.components().any(|c| matches!(c, Component::ParentDir))
                || p.to_string_lossy().contains('\0')
        })
    {
        return Err(SpaceError::Invalid.into());
    }
    let query = recipe.query(parameters.roots.clone());
    let mut paths = BTreeMap::new();
    let mut partial = false;
    let mut examined = 0;
    if recipe.source == Source::Query {
        query.validate()?;
        crate::search_many::visit(&query, now, lower, token, |msg| {
            match msg {
                SearchMsg::Hit(entry) => {
                    examined += 1;
                    if entry.kind == crate::EntryKind::File {
                        paths.insert(crate::saved_search::path_key(&entry.path), entry.path);
                    }
                }
                SearchMsg::Progress { dirs_scanned } => progress(dirs_scanned),
                SearchMsg::Done {
                    partial: p,
                    hit_cap,
                } => partial |= p || hit_cap,
                SearchMsg::Cancelled => partial = true,
                _ => {}
            }
            true
        });
    } else {
        if selected.len() > MAX_HITS {
            return Err(SpaceError::Limit.into());
        }
        for path in selected {
            checkpoint(token)?;
            if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
                return Err(SpaceError::Invalid.into());
            }
            let metadata = std::fs::symlink_metadata(&path).map_err(SpaceError::Io)?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || crate::search_many::reparse(&metadata)
            {
                return Err(RecipeError::SelectionFilesOnly);
            }
            examined += 1;
            progress(examined);
            if !crate::search::matches_query(
                &path.file_name().unwrap_or_default().to_string_lossy(),
                &recipe.name_query,
                recipe.ignore_case,
                recipe.use_wildcards,
            ) || recipe.min_bytes.is_some_and(|n| metadata.len() < n)
                || recipe.max_bytes.is_some_and(|n| metadata.len() > n)
            {
                continue;
            }
            if let Some(lower) = lower {
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
                match modified {
                    Some(t) if t < lower || t > now => continue,
                    None => {
                        partial = true;
                        continue;
                    }
                    _ => {}
                }
            }
            if !recipe.content_query.is_empty() {
                match crate::search_many::contains(
                    &path,
                    &recipe.content_query,
                    recipe.ignore_case,
                    metadata.len(),
                    token,
                ) {
                    Some(true) => {}
                    Some(false) => continue,
                    None => {
                        partial = true;
                        continue;
                    }
                }
            }
            paths.insert(crate::saved_search::path_key(&path), path);
        }
    }
    checkpoint(token)?;
    if partial {
        return Err(RecipeError::Partial);
    }
    let sources: Vec<_> = paths.into_values().collect();
    if sources.is_empty() {
        return Err(RecipeError::Empty);
    }
    let structure = match recipe.layout {
        Layout::Flat => Structure::Flat,
        Layout::Grouped => Structure::Grouped,
        Layout::Relative if parameters.roots.len() == 1 => {
            Structure::Relative(parameters.roots[0].clone())
        }
        _ => return Err(SpaceError::Invalid.into()),
    };
    let destination = parameters.parent.join(recipe.resolved_name(&date)?);
    let mut plan = delivery::prepare_review(
        &sources,
        &destination,
        &structure,
        recipe.output,
        &ReviewOptions {
            group_names: vec![],
            rename_flat_duplicates: recipe.number_duplicates,
        },
        token,
    )?;
    if recipe.hashes {
        plan.capture_hashes(token)?;
    }
    Ok(Prepared {
        recipe,
        parameters,
        plan,
        sources,
        examined,
    })
}
