// Naygo — workspace: paneles independientes componibles (lógica pura).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Modelo del espacio de trabajo: una colección de paneles independientes
//! (archivos / árbol / inspector), cuál está activo, y la disposición. No depende
//! de egui ni de Windows: la UI traduce esto a egui_dock.

pub mod file_pane;
pub mod layout;
pub mod nav_history;
pub mod template;

pub use file_pane::{FilePanePersist, FilePaneState};
pub use layout::{DockNode, SerializableDockLayout, SplitDir};
pub use nav_history::NavHistory;

use serde::{Deserialize, Serialize};

/// Identificador único y estable de un panel dentro del workspace.
/// Estable: no cambia aunque el panel se reordene en la UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u64);

/// Qué tipo de panel es.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanePurpose {
    /// Lista de archivos navegable.
    Files,
    /// Árbol de carpetas (esqueleto en Fase 2A).
    Tree,
    /// Inspector de metadatos del elemento enfocado en el panel activo.
    Inspector,
}

/// Un panel concreto del workspace. Solo los `Files` llevan `FilePaneState`.
#[derive(Clone, Debug)]
pub struct PaneNode {
    pub id: PaneId,
    pub purpose: PanePurpose,
    /// Estado del panel de archivos; `None` para Tree/Inspector.
    pub files: Option<FilePaneState>,
}

/// El espacio de trabajo: paneles + cuál está activo + la disposición.
#[derive(Clone, Debug)]
pub struct Workspace {
    panes: Vec<PaneNode>,
    active: Option<PaneId>,
    next_id: u64,
    /// Disposición visual (traducida a/desde egui_dock por la capa ui).
    pub layout: SerializableDockLayout,
}

impl Workspace {
    /// Workspace vacío.
    pub fn new() -> Self {
        Workspace {
            panes: Vec::new(),
            active: None,
            next_id: 0,
            layout: SerializableDockLayout::empty(),
        }
    }

    /// Agrega un panel del tipo dado y devuelve su id. Si es el primer panel,
    /// queda activo. Para `Files`, crea su `FilePaneState` parado en `dir`
    /// (ignorado para Tree/Inspector).
    pub fn add_pane(&mut self, purpose: PanePurpose, dir: std::path::PathBuf) -> PaneId {
        let id = PaneId(self.next_id);
        self.next_id += 1;
        let files = match purpose {
            PanePurpose::Files => Some(FilePaneState::new(dir)),
            _ => None,
        };
        self.panes.push(PaneNode { id, purpose, files });
        if self.active.is_none() {
            self.active = Some(id);
        }
        id
    }

    /// Quita el panel `id`. Si era el activo, reasigna el activo al primer panel
    /// `Files` restante (o a cualquier panel, o `None` si no queda ninguno).
    pub fn remove_pane(&mut self, id: PaneId) {
        self.panes.retain(|p| p.id != id);
        if self.active == Some(id) {
            self.active = self
                .panes
                .iter()
                .find(|p| p.purpose == PanePurpose::Files)
                .or_else(|| self.panes.first())
                .map(|p| p.id);
        }
    }

    /// El id del panel activo, si hay alguno.
    pub fn active_id(&self) -> Option<PaneId> {
        self.active
    }

    /// Marca `id` como activo si existe.
    pub fn set_active(&mut self, id: PaneId) {
        if self.panes.iter().any(|p| p.id == id) {
            self.active = Some(id);
        }
    }

    /// Referencia a un panel por id.
    pub fn pane(&self, id: PaneId) -> Option<&PaneNode> {
        self.panes.iter().find(|p| p.id == id)
    }

    /// Referencia mutable a un panel por id.
    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut PaneNode> {
        self.panes.iter_mut().find(|p| p.id == id)
    }

    /// El `FilePaneState` del panel `Files` activo (lo que refleja el inspector).
    /// Si el activo no es `Files`, devuelve el primer `Files` que haya.
    pub fn active_files(&self) -> Option<&FilePaneState> {
        self.active
            .and_then(|id| self.pane(id))
            .filter(|p| p.purpose == PanePurpose::Files)
            .and_then(|p| p.files.as_ref())
            .or_else(|| {
                self.panes
                    .iter()
                    .find(|p| p.purpose == PanePurpose::Files)
                    .and_then(|p| p.files.as_ref())
            })
    }

    /// Versión mutable de `active_files`.
    pub fn active_files_mut(&mut self) -> Option<&mut FilePaneState> {
        let target = self
            .active
            .filter(|id| {
                self.pane(*id)
                    .map(|p| p.purpose == PanePurpose::Files)
                    .unwrap_or(false)
            })
            .or_else(|| {
                self.panes
                    .iter()
                    .find(|p| p.purpose == PanePurpose::Files)
                    .map(|p| p.id)
            })?;
        self.pane_mut(target).and_then(|p| p.files.as_mut())
    }

    /// Itera los paneles (orden de inserción).
    pub fn panes(&self) -> &[PaneNode] {
        &self.panes
    }

    /// Itera los paneles mutables.
    pub fn panes_mut(&mut self) -> &mut [PaneNode] {
        &mut self.panes
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn pane_id_es_comparable_y_ordenable() {
        assert_eq!(PaneId(1), PaneId(1));
        assert!(PaneId(1) < PaneId(2));
    }

    #[test]
    fn pane_purpose_round_trip_serde() {
        let json = serde_json::to_string(&PanePurpose::Files).unwrap();
        let back: PanePurpose = serde_json::from_str(&json).unwrap();
        assert_eq!(back, PanePurpose::Files);
    }

    #[test]
    fn primer_panel_queda_activo() {
        let mut w = Workspace::new();
        let id = w.add_pane(PanePurpose::Files, PathBuf::from("C:/"));
        assert_eq!(w.active_id(), Some(id));
    }

    #[test]
    fn quitar_el_activo_reasigna_a_otro_files() {
        let mut w = Workspace::new();
        let a = w.add_pane(PanePurpose::Files, PathBuf::from("C:/a"));
        let b = w.add_pane(PanePurpose::Files, PathBuf::from("C:/b"));
        w.set_active(a);
        w.remove_pane(a);
        assert_eq!(w.active_id(), Some(b));
    }

    #[test]
    fn active_files_apunta_al_panel_files_activo() {
        let mut w = Workspace::new();
        let _tree = w.add_pane(PanePurpose::Tree, PathBuf::new());
        let files = w.add_pane(PanePurpose::Files, PathBuf::from("C:/x"));
        w.set_active(files);
        assert_eq!(
            w.active_files().map(|f| f.current_dir.clone()),
            Some(PathBuf::from("C:/x"))
        );
    }

    #[test]
    fn tree_no_tiene_file_pane_state() {
        let mut w = Workspace::new();
        let t = w.add_pane(PanePurpose::Tree, PathBuf::new());
        assert!(w.pane(t).unwrap().files.is_none());
    }
}
