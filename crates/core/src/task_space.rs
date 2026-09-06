// Naygo — espacios por tarea versionados, sin listados ni contenido de archivos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use crate::config::{WorkspacePersist, CONFIG_VERSION};
use crate::workspace::{PaneId, PanePurpose, Workspace};
use crate::CancellationToken;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub const VERSION: u32 = 1;
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_BASKET: usize = 10_000;

#[cfg(test)]
#[path = "task_space_tests.rs"]
mod tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpace {
    pub version: u32,
    pub name: String,
    pub workspace: WorkspacePersist,
    pub basket: Vec<PathBuf>,
    #[serde(default)]
    pub saved_searches: Vec<PathBuf>,
    #[serde(default)]
    pub saved_recipes: Vec<PathBuf>,
    #[serde(default)]
    pub visual_filters: Vec<(PaneId, String)>,
    #[serde(default)]
    pub typeahead: String,
    #[serde(default)]
    pub filter_hide_nonmatches: bool,
    /// Si existe, las rutas del payload son relativas a esta raíz explícita.
    /// Puede sustituirse al abrir en otra máquina; nunca se adivina buscando en disco.
    pub base_root: Option<PathBuf>,
}

#[derive(Debug)]
pub enum SpaceError {
    Invalid,
    Version,
    Limit,
    OutsideRoot,
    Cancelled,
    Changed,
    Exists,
    Io(std::io::Error),
}
impl From<std::io::Error> for SpaceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn checkpoint(token: &CancellationToken) -> Result<(), SpaceError> {
    if token.is_cancelled() {
        Err(SpaceError::Cancelled)
    } else {
        Ok(())
    }
}

pub fn valid_name(name: &str) -> bool {
    !reserved_name(name)
        && crate::ops::names::is_valid_name(name)
        && name.encode_utf16().count() <= 80
        && Path::new(name).components().count() == 1
}

pub(crate) fn reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().to_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

impl TaskSpace {
    /// Comprobación léxica: no abre ni canonicaliza las referencias del snapshot.
    pub fn references_under(&self, root: &Path) -> bool {
        self.paths().any(|path| relative_to(path, root).is_some())
    }
    /// Normaliza el orden de mapas serializados, no el orden del layout ni de la bandeja.
    pub fn signature(&self) -> Result<String, SpaceError> {
        let mut normalized = self.clone();
        normalized.workspace.purposes.sort_by_key(|(id, _)| *id);
        normalized.workspace.files.sort_by_key(|(id, _)| *id);
        normalized.workspace.tree_links.sort_unstable();
        normalized.visual_filters.sort_by_key(|(id, _)| *id);
        Ok(revision(&normalized.to_bytes()?))
    }
    pub fn validate(&self) -> Result<(), SpaceError> {
        if self.version != VERSION || self.workspace.version != CONFIG_VERSION {
            return Err(SpaceError::Version);
        }
        if !valid_name(&self.name) {
            return Err(SpaceError::Invalid);
        }
        if self.workspace.purposes.len() > 64
            || self.basket.len() > MAX_BASKET
            || self.visual_filters.len() > 64
            || self.saved_searches.len() > 64
            || self.saved_recipes.len() > 64
            || self.typeahead.len() > 4096
        {
            return Err(SpaceError::Limit);
        }
        let layout = self.workspace.layout.pane_ids();
        let ids: HashSet<_> = self.workspace.purposes.iter().map(|(id, _)| *id).collect();
        if ids.is_empty()
            || ids.len() != self.workspace.purposes.len()
            || layout.len() != ids.len()
            || layout.iter().copied().collect::<HashSet<_>>() != ids
            // Los identificadores de la UI usan un entero firmado de 32 bits.
            || ids.iter().any(|id| id.0 >= i32::MAX as u64)
            || self.workspace.active.is_some_and(|id| !ids.contains(&id))
        {
            return Err(SpaceError::Invalid);
        }
        let file_ids: HashSet<_> = self
            .workspace
            .purposes
            .iter()
            .filter_map(|(id, purpose)| (*purpose == PanePurpose::Files).then_some(*id))
            .collect();
        if self.workspace.files.len() != file_ids.len()
            || self
                .workspace
                .files
                .iter()
                .map(|(id, _)| *id)
                .collect::<HashSet<_>>()
                != file_ids
            || self
                .visual_filters
                .iter()
                .any(|(id, text)| !file_ids.contains(id) || text.len() > 4096)
            || self
                .visual_filters
                .iter()
                .map(|(id, _)| *id)
                .collect::<HashSet<_>>()
                .len()
                != self.visual_filters.len()
            || self.workspace.tree_links.iter().any(|(tree, file)| {
                !file_ids.contains(file)
                    || !self
                        .workspace
                        .purposes
                        .contains(&(*tree, PanePurpose::Tree))
            })
        {
            return Err(SpaceError::Invalid);
        }
        for path in self.paths() {
            if path.as_os_str().len() > 32_768 {
                return Err(SpaceError::Limit);
            }
            if self.base_root.is_some() {
                if path
                    .components()
                    .any(|c| !matches!(c, Component::Normal(_)))
                {
                    return Err(SpaceError::Invalid);
                }
            } else if !path.is_absolute() {
                return Err(SpaceError::Invalid);
            }
        }
        if self
            .base_root
            .as_ref()
            .is_some_and(|root| !root.is_absolute())
        {
            return Err(SpaceError::Invalid);
        }
        if Workspace::from_persist(&self.workspace).is_none() {
            return Err(SpaceError::Invalid);
        }
        Ok(())
    }

    fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.workspace
            .files
            .iter()
            .map(|(_, f)| &f.current_dir)
            .chain(self.basket.iter())
            .chain(self.saved_searches.iter())
            .chain(self.saved_recipes.iter())
    }

    fn map_paths(
        &mut self,
        mut map: impl FnMut(&Path) -> Result<PathBuf, SpaceError>,
    ) -> Result<(), SpaceError> {
        for (_, file) in &mut self.workspace.files {
            file.current_dir = map(&file.current_dir)?;
        }
        for path in &mut self.basket {
            *path = map(path)?;
        }
        for path in &mut self.saved_searches {
            *path = map(path)?;
        }
        for path in &mut self.saved_recipes {
            *path = map(path)?;
        }
        Ok(())
    }

    /// Opera sobre un clon: un error de raíz nunca deja al caller con media conversión.
    pub fn with_relative_paths(&self, root: &Path) -> Result<Self, SpaceError> {
        self.validate()?;
        if self.base_root.is_some() || !root.is_absolute() {
            return Err(SpaceError::Invalid);
        }
        let mut out = self.clone();
        out.map_paths(|path| relative_to(path, root).ok_or(SpaceError::OutsideRoot))?;
        out.base_root = Some(root.to_path_buf());
        out.validate()?;
        Ok(out)
    }

    pub fn resolved(&self, override_root: Option<&Path>) -> Result<Self, SpaceError> {
        self.validate()?;
        let mut out = self.clone();
        if let Some(saved_root) = &self.base_root {
            let root = override_root.unwrap_or(saved_root);
            if !root.is_absolute() {
                return Err(SpaceError::Invalid);
            }
            out.map_paths(|path| Ok(root.join(path)))?;
            out.base_root = None;
        } else if override_root.is_some() {
            return Err(SpaceError::OutsideRoot);
        }
        out.validate()?;
        Ok(out)
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

fn relative_to(path: &Path, root: &Path) -> Option<PathBuf> {
    let mut parts = path.components();
    for expected in root.components() {
        let actual = parts.next()?;
        if actual.as_os_str().to_string_lossy().to_lowercase()
            != expected.as_os_str().to_string_lossy().to_lowercase()
        {
            return None;
        }
    }
    Some(parts.as_path().to_path_buf())
}

pub fn revision(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

/// Solo lee el documento pequeño, no toca las carpetas referenciadas.
pub fn read(path: &Path, token: &CancellationToken) -> Result<(TaskSpace, String), SpaceError> {
    checkpoint(token)?;
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    checkpoint(token)?;
    if bytes.len() > MAX_BYTES {
        return Err(SpaceError::Limit);
    }
    let space: TaskSpace = serde_json::from_slice(&bytes).map_err(|_| SpaceError::Invalid)?;
    space.validate()?;
    Ok((space, revision(&bytes)))
}

/// Adapta una disposición/sesión heredada explícitamente, sin cambiar su archivo original.
pub fn from_workspace(name: String, workspace: WorkspacePersist) -> Result<TaskSpace, SpaceError> {
    let space = TaskSpace {
        version: VERSION,
        name,
        workspace,
        basket: Vec::new(),
        saved_searches: Vec::new(),
        saved_recipes: Vec::new(),
        visual_filters: Vec::new(),
        typeahead: String::new(),
        filter_hide_nonmatches: false,
        base_root: None,
    };
    space.validate()?;
    Ok(space)
}
