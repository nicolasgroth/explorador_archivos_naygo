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

#[cfg(test)]
mod tests {
    use super::*;

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
}
