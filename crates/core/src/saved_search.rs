// Naygo — consultas guardadas versionadas y criterios relativos evaluados al ejecutar.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use crate::search::SearchOptions;
use crate::task_space::{checkpoint, revision, valid_name, SpaceError};
use crate::CancellationToken;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub const VERSION: u32 = 1;
pub const MAX_BYTES: usize = 256 * 1024;
pub const MAX_ROOTS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelativeDate {
    #[default]
    Any,
    Today,
    ThisWeek,
    Last7Days,
    Last30Days,
    ThisMonth,
}

impl RelativeDate {
    /// El offset local lo proporciona platform; core no consulta el reloj ni Windows.
    pub fn lower_bound(self, now: i64, offset: i64) -> Option<i64> {
        let day = (now + offset).div_euclid(86_400);
        match self {
            Self::Any => None,
            Self::Today => Some(day * 86_400 - offset),
            Self::ThisWeek => Some((day - (day + 3).rem_euclid(7)) * 86_400 - offset),
            Self::Last7Days => Some(now - 7 * 86_400),
            Self::Last30Days => Some(now - 30 * 86_400),
            Self::ThisMonth => {
                let (_, _, month_day, _, _) = crate::format::civil_from_epoch(now + offset);
                Some((day - i64::from(month_day) + 1) * 86_400 - offset)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedSearch {
    pub version: u32,
    pub name: String,
    pub roots: Vec<PathBuf>,
    pub options: SearchOptions,
    pub min_bytes: Option<u64>,
    pub max_bytes: Option<u64>,
    pub modified: RelativeDate,
}

impl SavedSearch {
    pub fn validate(&self) -> Result<(), SpaceError> {
        if self.version != VERSION {
            return Err(SpaceError::Version);
        }
        if !valid_name(&self.name)
            || self.roots.is_empty()
            || self
                .min_bytes
                .zip(self.max_bytes)
                .is_some_and(|(a, b)| a > b)
            || self.roots.iter().any(|r| {
                !r.is_absolute()
                    || r.components().any(|c| matches!(c, Component::ParentDir))
                    || r.to_string_lossy().contains('\0')
            })
        {
            return Err(SpaceError::Invalid);
        }
        if self.roots.len() > MAX_ROOTS
            || self.options.name_query.len() > 4096
            || self.options.content_query.len() > 4096
            || self
                .roots
                .iter()
                .map(|p| p.as_os_str().len())
                .sum::<usize>()
                > MAX_BYTES / 4
        {
            return Err(SpaceError::Limit);
        }
        Ok(())
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, SpaceError> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|_| SpaceError::Invalid)?;
        if bytes.len() > MAX_BYTES {
            return Err(SpaceError::Limit);
        }
        Ok(bytes)
    }
    /// Deduplicación léxica sin I/O. Con recursión, una raíz padre cubre sus hijas.
    pub fn normalized_roots(&self) -> Vec<PathBuf> {
        let mut roots = self.roots.clone();
        roots.sort_by_key(|p| p.components().count());
        let mut out: Vec<PathBuf> = Vec::new();
        for root in roots {
            if !out.iter().any(|parent| {
                path_key(parent) == path_key(&root)
                    || (self.options.recursive && is_under(&root, parent))
            }) {
                out.push(root);
            }
        }
        out
    }
}

pub(crate) fn path_key(path: &Path) -> String {
    let key = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}
fn is_under(path: &Path, parent: &Path) -> bool {
    let parent = path_key(parent);
    path_key(path)
        .strip_prefix(&parent)
        .is_some_and(|tail| tail.starts_with('/'))
}

pub fn read(path: &Path, token: &CancellationToken) -> Result<(SavedSearch, String), SpaceError> {
    checkpoint(token)?;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    checkpoint(token)?;
    if bytes.len() > MAX_BYTES {
        return Err(SpaceError::Limit);
    }
    let query: SavedSearch = serde_json::from_slice(&bytes).map_err(|_| SpaceError::Invalid)?;
    query.validate()?;
    Ok((query, revision(&bytes)))
}
