// Naygo — modelo de filesystem: tipos POCO sin lógica de I/O.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Tipos planos que describen lo que se ve en un panel. No tocan el disco: son
//! datos que el motor de `listing` produce y que la UI consume.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// Si una entrada es archivo, carpeta o un tipo que no pudimos clasificar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    Directory,
    File,
    /// Symlink, junction, device, etc. — se muestra pero no se asume navegable.
    Other,
}

/// Una entrada del filesystem tal como la pinta la UI.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    /// Tamaño en bytes. `None` para carpetas (se calcula bajo demanda en otra fase).
    pub size: Option<u64>,
    /// Fecha de última modificación, si el SO la entrega.
    pub modified: Option<SystemTime>,
    /// Fecha de creación, si el SO la entrega.
    pub created: Option<SystemTime>,
    /// Atributo "oculto" (en Windows). Se rellena leyendo los atributos reales del FS.
    pub hidden: bool,
    /// Atributo "de sistema" (en Windows). Se rellena leyendo los atributos reales del FS.
    pub system: bool,
}

impl Entry {
    /// `true` si es una carpeta navegable.
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Directory
    }
}

/// Modos de vista del panel de archivos. En la Fase 1 solo `Details` se pinta.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    #[default]
    Details,
    List,
    Icons,
}

/// Clave por la que se ordena un panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SortKey {
    Name,
    Extension,
    Size,
    Modified,
    Created,
    Kind,
}

/// Especificación de ordenamiento: por qué clave y en qué dirección.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SortSpec {
    pub key: SortKey,
    pub ascending: bool,
    /// Si las carpetas van siempre antes que los archivos (estilo Explorer).
    pub dirs_first: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        SortSpec {
            key: SortKey::Name,
            ascending: true,
            dirs_first: true,
        }
    }
}

/// Estado de un panel de archivos: dónde está parado, qué ve y qué hay seleccionado.
#[derive(Clone, Debug)]
pub struct PaneState {
    pub current_dir: PathBuf,
    pub entries: Vec<Entry>,
    pub sort: SortSpec,
    pub view: ViewMode,
    /// Índice de la entrada con foco dentro de `entries`, si hay alguna.
    pub focused: Option<usize>,
    /// Índices marcados (selección múltiple).
    pub selected: Vec<usize>,
}

impl PaneState {
    /// Crea un panel vacío parado en `dir`.
    pub fn new(dir: PathBuf) -> Self {
        PaneState {
            current_dir: dir,
            entries: Vec::new(),
            sort: SortSpec::default(),
            view: ViewMode::default(),
            focused: None,
            selected: Vec::new(),
        }
    }

    /// Entrada actualmente con foco, si existe.
    pub fn focused_entry(&self) -> Option<&Entry> {
        self.focused.and_then(|i| self.entries.get(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_directory_es_dir() {
        let e = Entry {
            name: "docs".into(),
            path: PathBuf::from("C:/docs"),
            kind: EntryKind::Directory,
            size: None,
            modified: None,
            created: None,
            hidden: false,
            system: false,
        };
        assert!(e.is_dir());
    }

    #[test]
    fn pane_nuevo_no_tiene_foco_ni_seleccion() {
        let p = PaneState::new(PathBuf::from("C:/"));
        assert!(p.focused_entry().is_none());
        assert!(p.selected.is_empty());
        assert_eq!(p.view, ViewMode::Details);
        assert_eq!(p.sort.key, SortKey::Name);
    }
}
