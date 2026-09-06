// Naygo — puntos de comparación locales, versionados y con cobertura explícita.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use crate::{
    fs_model::EntryKind,
    task_space::{checkpoint, revision, SpaceError},
    CancellationToken,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io::Read,
    path::{Component, Path, PathBuf},
};

mod diff;
mod scan;
pub use diff::{compare, Change, ChangeKind, Comparison};
pub use scan::capture;
#[cfg(test)]
mod tests;

pub const MAX_ENTRIES: usize = 50_000;
pub const MAX_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_PATH_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_ROWS: usize = 5_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub root: PathBuf,
    pub recursive: bool,
    /// Prefijos relativos de ruta; no son patrones glob.
    pub exclusions: Vec<PathBuf>,
    pub hashed: bool,
}

pub(crate) fn key(path: &Path) -> String {
    // Normalizar separadores redundantes/finales antes de comparar prefijos o duplicados.
    let normalized: PathBuf = path.components().collect();
    let value = normalized.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}
fn relative(path: &Path, allow_empty: bool) -> bool {
    (allow_empty || !path.as_os_str().is_empty())
        && path
            .to_str()
            .is_some_and(|s| !s.contains(['\0', '*', '?', '"', '<', '>', '|']) && s.len() <= 4096)
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        && !path
            .to_string_lossy()
            .split(['/', '\\'])
            .any(|s| s == ".." || s == "." || s.contains(':'))
}
pub(crate) fn under(path: &Path, prefix: &Path) -> bool {
    let (p, base) = (key(path), key(prefix));
    base.is_empty() || p == base || p.strip_prefix(&base).is_some_and(|s| s.starts_with('/'))
}
impl Scope {
    pub fn validate(&self) -> Result<(), SpaceError> {
        if !self.root.is_absolute()
            || self
                .root
                .to_str()
                .is_none_or(|s| s.contains('\0') || s.len() > 4096)
            || self
                .root
                .components()
                .any(|c| matches!(c, Component::ParentDir))
            || self.exclusions.len() > 64
            || self.exclusions.iter().any(|p| !relative(p, false))
        {
            return Err(SpaceError::Invalid);
        }
        Ok(())
    }
    pub fn includes(&self, path: &Path) -> bool {
        relative(path, false)
            && (self.recursive || path.components().count() == 1)
            && !self.exclusions.iter().any(|e| under(path, e))
    }
    /// No incluir en la captura el documento que se va a publicar dentro de la raíz.
    pub fn exclude_document(&mut self, path: &Path) -> Result<(), SpaceError> {
        let root_parts: Vec<_> = self.root.components().collect();
        let path_parts: Vec<_> = path.components().collect();
        if path_parts.len() > root_parts.len()
            && root_parts
                .iter()
                .zip(&path_parts)
                .all(|(a, b)| key(Path::new(a.as_os_str())) == key(Path::new(b.as_os_str())))
        {
            let relative: PathBuf = path_parts[root_parts.len()..]
                .iter()
                .map(|c| c.as_os_str())
                .collect();
            if self.includes(&relative) {
                self.exclusions.push(relative);
            }
        }
        self.validate()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub bytes: u64,
    pub modified_ns: Option<u64>,
    pub sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonPoint {
    pub version: u32,
    pub captured_at: u64,
    pub scope: Scope,
    pub resolved_root: Option<PathBuf>,
    pub entries: Vec<Record>,
    /// Vacío = raíz inaccesible. Una zona desconocida nunca demuestra ausencia.
    pub gaps: Vec<PathBuf>,
    pub truncated: bool,
}
impl ComparisonPoint {
    pub fn validate(&self) -> Result<(), SpaceError> {
        if self.version != 1 {
            return Err(SpaceError::Version);
        }
        self.scope.validate()?;
        if self.captured_at > 253_402_300_799 {
            return Err(SpaceError::Invalid);
        }
        if self.entries.len() > MAX_ENTRIES
            || self.gaps.len() > MAX_ENTRIES + 1
            || self
                .entries
                .iter()
                .map(|e| e.path.as_os_str().len())
                .sum::<usize>()
                + self.gaps.iter().map(|p| p.as_os_str().len()).sum::<usize>()
                > MAX_PATH_BYTES
        {
            return Err(SpaceError::Limit);
        }
        let mut seen = HashSet::new();
        if self
            .resolved_root
            .as_ref()
            .is_some_and(|p| !p.is_absolute())
            || (self.resolved_root.is_none() && !self.gaps.iter().any(|p| p.as_os_str().is_empty()))
            || self.entries.iter().any(|e| {
                !self.scope.includes(&e.path)
                    || !seen.insert(key(&e.path))
                    || e.sha256.as_ref().is_some_and(|s| {
                        !self.scope.hashed
                            || e.kind != EntryKind::File
                            || s.len() != 64
                            || !s.bytes().all(|b| b.is_ascii_hexdigit())
                    })
            })
            || self.gaps.iter().any(|p| {
                !relative(p, true) || (!p.as_os_str().is_empty() && !self.scope.includes(p))
            })
        {
            return Err(SpaceError::Invalid);
        }
        Ok(())
    }
    pub fn complete(&self) -> bool {
        !self.truncated && self.gaps.is_empty()
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, SpaceError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| SpaceError::Invalid)?;
        if bytes.len() > MAX_BYTES {
            return Err(SpaceError::Limit);
        }
        Ok(bytes)
    }
}
pub fn read(
    path: &Path,
    token: &CancellationToken,
) -> Result<(ComparisonPoint, String), SpaceError> {
    checkpoint(token)?;
    let mut file = std::fs::File::open(path)?.take((MAX_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    let mut block = [0; 65536];
    loop {
        checkpoint(token)?;
        let n = file.read(&mut block)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&block[..n]);
        if bytes.len() > MAX_BYTES {
            return Err(SpaceError::Limit);
        }
    }
    let point: ComparisonPoint = serde_json::from_slice(&bytes).map_err(|_| SpaceError::Invalid)?;
    point.validate()?;
    checkpoint(token)?;
    Ok((point, revision(&bytes)))
}
