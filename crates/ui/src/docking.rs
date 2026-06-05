// Naygo — integración de egui_dock para paneles dinámicos dockables.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Define cómo egui_dock pinta cada tab. Cada `PaneTab` delega en su panel.
//! El docking permite reacomodar/arrastrar los paneles (árbol, archivos,
//! inspector) en caliente, como pide el spec.

use crate::app::{PaneTab, UiState};
use crate::panes;

/// Implementa `TabViewer`: egui_dock le pide, por cada tab, su título y su UI.
pub struct NaygoTabViewer<'a> {
    pub state: &'a mut UiState,
}

impl egui_dock::TabViewer for NaygoTabViewer<'_> {
    type Tab = PaneTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            PaneTab::Tree => "Carpetas".into(),
            PaneTab::Files => "Archivos".into(),
            PaneTab::Inspector => "Propiedades".into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            PaneTab::Tree => panes::tree_panel::show(ui, self.state),
            PaneTab::Files => panes::file_panel::show(ui, self.state),
            PaneTab::Inspector => panes::inspector_panel::show(ui, self.state),
        }
    }
}
