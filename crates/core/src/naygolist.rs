// Naygo — formato portable y versionado de bandejas .naygolist.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaygoList {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    pub entries: Vec<NaygoListEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaygoListEntry {
    pub path: PathBuf,
}

impl NaygoList {
    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>, root: Option<PathBuf>) -> Self {
        let entries = paths
            .into_iter()
            .map(|path| {
                let stored = root
                    .as_deref()
                    .and_then(|root| path.strip_prefix(root).ok())
                    .map(Path::to_path_buf)
                    .unwrap_or(path);
                NaygoListEntry { path: stored }
            })
            .collect();
        Self {
            version: FORMAT_VERSION,
            root,
            entries,
        }
    }

    /// Resuelve referencias sin consultar el filesystem: las entradas ausentes se conservan.
    pub fn resolve_paths(&self) -> Result<Vec<PathBuf>, String> {
        if self.version != FORMAT_VERSION {
            return Err(format!("unsupported .naygolist version {}", self.version));
        }
        Ok(self
            .entries
            .iter()
            .map(|entry| {
                if entry.path.is_absolute() {
                    entry.path.clone()
                } else {
                    self.root
                        .as_ref()
                        .map(|root| root.join(&entry.path))
                        .unwrap_or_else(|| entry.path.clone())
                }
            })
            .collect())
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
    pub fn from_json(text: &str) -> Result<Self, String> {
        let list: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        list.resolve_paths()?;
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guarda_relativas_y_recupera_ausentes_sin_descartarlas() {
        let root = PathBuf::from("C:/trabajo");
        let list = NaygoList::from_paths(
            [root.join("existe.txt"), PathBuf::from("D:/fuera.txt")],
            Some(root.clone()),
        );
        assert_eq!(list.entries[0].path, PathBuf::from("existe.txt"));
        let loaded = NaygoList::from_json(&list.to_json().unwrap()).unwrap();
        assert_eq!(
            loaded.resolve_paths().unwrap(),
            vec![root.join("existe.txt"), PathBuf::from("D:/fuera.txt")]
        );
    }
}
