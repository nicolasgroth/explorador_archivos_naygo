// Naygo — barra de íconos: navegación + layouts + agregar panel + ajustes.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Barra de acciones con botones solo-ícono (tooltips). Posición configurable
//! (arriba o al costado), según `Settings.bar_position`. Atrás/adelante se
//! habilitan según el historial del panel activo.

use crate::app::NaygoApp;
use crate::input::Action;
use naygo_core::config::BarPosition;

/// Pinta la barra en la posición configurada. Debe llamarse al inicio de `ui()`.
pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    match app.settings.bar_position {
        BarPosition::Top => {
            egui::Panel::top("toolbar").show_inside(ui, |ui| {
                ui.horizontal(|ui| buttons(ui, app));
            });
        }
        BarPosition::Side => {
            egui::Panel::left("toolbar")
                .resizable(false)
                .exact_size(40.0)
                .show_inside(ui, |ui| {
                    ui.vertical(|ui| buttons(ui, app));
                });
        }
    }
}

fn buttons(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (can_back, can_forward) = app
        .workspace
        .active_files()
        .map(|f| (f.history.can_back(), f.history.can_forward()))
        .unwrap_or((false, false));

    if icon_button(ui, "◀", "Atrás (Alt+←)", can_back) {
        app.apply_action(Action::GoBack);
    }
    if icon_button(ui, "▶", "Adelante (Alt+→)", can_forward) {
        app.apply_action(Action::GoForward);
    }
    if icon_button(ui, "▲", "Subir un nivel (Backspace)", true) {
        app.apply_action(Action::GoUp);
    }
    if icon_button(ui, "⟳", "Refrescar", true) {
        if let (Some(id), Some(dir)) = (
            app.workspace.active_id(),
            app.workspace.active_files().map(|f| f.current_dir.clone()),
        ) {
            app.refresh_pane(id, dir);
        }
    }
    ui.separator();
    crate::templates_menu::layouts_button(ui, app);
    if icon_button(ui, "➕", "Agregar panel de archivos", true) {
        app.add_files_pane();
    }

    // Botón de ajustes: a la derecha del todo si la barra es horizontal (Top).
    if matches!(app.settings.bar_position, BarPosition::Top) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            settings_button(ui, app);
        });
    } else {
        settings_button(ui, app);
    }
}

/// Botón de ajustes: abre la ventana de Configuración (viewport separado).
fn settings_button(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let lbl = app.tr("toolbar.settings");
    if ui.button("⚙").on_hover_text(lbl).clicked() {
        app.settings_open = true;
    }
}

/// Un botón solo-ícono con tooltip; deshabilitado si `enabled` es false.
fn icon_button(ui: &mut egui::Ui, icon: &str, tip: &str, enabled: bool) -> bool {
    ui.add_enabled(enabled, egui::Button::new(icon))
        .on_hover_text(tip)
        .clicked()
}
