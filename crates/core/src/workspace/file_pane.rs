// Naygo — estado de un panel de archivos (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `FilePaneState` es el estado de un panel de archivos: dónde está parado, qué
//! lista, su historial de navegación, su filtro de carpetas. No toca disco: la UI
//! le inyecta las entradas (vía el motor de `listing`) y le pide navegar.

use crate::fs_model::{Entry, SortSpec, ViewMode};
use crate::workspace::nav_history::NavHistory;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Estado de un panel de archivos. Lo serializable se persiste; `entries` no
/// (se re-lista al abrir) y `history` tampoco (arranca limpio cada sesión).
#[derive(Clone, Debug)]
pub struct FilePaneState {
    pub current_dir: PathBuf,
    pub entries: Vec<Entry>,
    pub sort: SortSpec,
    pub view: ViewMode,
    pub focused: Option<usize>,
    pub selected: Vec<usize>,
    pub history: NavHistory,
    /// Si es `false`, el panel oculta las carpetas (muestra solo archivos).
    pub show_dirs: bool,
    /// RESERVADO para una fase futura (filtro de texto). Siempre `None` en 2A.
    pub text_filter: Option<String>,
}

/// Lo que se persiste de un panel de archivos (sin entries ni history).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilePanePersist {
    pub current_dir: PathBuf,
    pub sort: SortSpec,
    pub view: ViewMode,
    pub show_dirs: bool,
    pub text_filter: Option<String>,
}

impl FilePaneState {
    /// Crea un panel parado en `dir`, con su historial ya apuntando a `dir`.
    pub fn new(dir: PathBuf) -> Self {
        let mut history = NavHistory::new();
        history.push(dir.clone());
        FilePaneState {
            current_dir: dir,
            entries: Vec::new(),
            sort: SortSpec::default(),
            view: ViewMode::default(),
            focused: None,
            selected: Vec::new(),
            history,
            show_dirs: true,
            text_filter: None,
        }
    }

    /// Entrada con foco, si existe.
    pub fn focused_entry(&self) -> Option<&Entry> {
        self.focused.and_then(|i| self.entries.get(i))
    }

    /// Navega a una carpeta nueva: registra en el historial y limpia entries/foco.
    /// (La UI lanzará el listado de `dir` tras llamar esto.)
    pub fn navigate_to(&mut self, dir: PathBuf) {
        self.history.push(dir.clone());
        self.enter(dir);
    }

    /// Va atrás en el historial. Devuelve la nueva carpeta si se movió.
    pub fn go_back(&mut self) -> Option<PathBuf> {
        let path = self.history.back().map(Path::to_path_buf)?;
        self.enter(path.clone());
        Some(path)
    }

    /// Va adelante en el historial. Devuelve la nueva carpeta si se movió.
    pub fn go_forward(&mut self) -> Option<PathBuf> {
        let path = self.history.forward().map(Path::to_path_buf)?;
        self.enter(path.clone());
        Some(path)
    }

    /// Sube al directorio padre (entra al historial). Devuelve el padre si existe.
    pub fn go_up(&mut self) -> Option<PathBuf> {
        let parent = self.current_dir.parent()?.to_path_buf();
        self.navigate_to(parent.clone());
        Some(parent)
    }

    /// Reemplaza la carpeta actual sin tocar el historial (uso interno).
    fn enter(&mut self, dir: PathBuf) {
        self.current_dir = dir;
        self.entries.clear();
        self.focused = None;
        self.selected.clear();
    }

    /// Estado persistible (sin entries ni history).
    pub fn to_persist(&self) -> FilePanePersist {
        FilePanePersist {
            current_dir: self.current_dir.clone(),
            sort: self.sort,
            view: self.view,
            show_dirs: self.show_dirs,
            text_filter: self.text_filter.clone(),
        }
    }

    /// Reconstruye desde lo persistido (historial nuevo apuntando a la carpeta).
    pub fn from_persist(p: FilePanePersist) -> Self {
        let mut s = FilePaneState::new(p.current_dir);
        s.sort = p.sort;
        s.view = p.view;
        s.show_dirs = p.show_dirs;
        s.text_filter = p.text_filter;
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn nuevo_apunta_su_historial_a_la_carpeta() {
        let s = FilePaneState::new(p("C:/a"));
        assert_eq!(s.current_dir, p("C:/a"));
        assert_eq!(s.history.current(), Some(p("C:/a").as_path()));
        assert!(s.show_dirs);
        assert!(s.text_filter.is_none());
    }

    #[test]
    fn navigate_y_back_actualizan_carpeta_e_historial() {
        let mut s = FilePaneState::new(p("C:/a"));
        s.navigate_to(p("C:/a/b"));
        assert_eq!(s.current_dir, p("C:/a/b"));
        let back = s.go_back();
        assert_eq!(back, Some(p("C:/a")));
        assert_eq!(s.current_dir, p("C:/a"));
        let fwd = s.go_forward();
        assert_eq!(fwd, Some(p("C:/a/b")));
    }

    #[test]
    fn navegar_limpia_entries_y_foco() {
        let mut s = FilePaneState::new(p("C:/a"));
        s.focused = Some(3);
        s.selected = vec![1, 2];
        s.navigate_to(p("C:/a/b"));
        assert!(s.entries.is_empty());
        assert!(s.focused.is_none());
        assert!(s.selected.is_empty());
    }

    #[test]
    fn persist_round_trip_conserva_lo_serializable() {
        let mut s = FilePaneState::new(p("C:/a"));
        s.show_dirs = false;
        let restored = FilePaneState::from_persist(s.to_persist());
        assert_eq!(restored.current_dir, p("C:/a"));
        assert!(!restored.show_dirs);
        // El historial se reinicia apuntando a la carpeta.
        assert_eq!(restored.history.current(), Some(p("C:/a").as_path()));
    }
}
