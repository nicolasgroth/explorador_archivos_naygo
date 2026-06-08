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

    // Precalcular las etiquetas (tooltips) antes de los widgets que toman
    // `&mut app`, para no enredar los préstamos.
    let lbl_back = app.tr("toolbar.back");
    let lbl_forward = app.tr("toolbar.forward");
    let lbl_up = app.tr("toolbar.up");
    let lbl_refresh = app.tr("toolbar.refresh");
    let lbl_add_pane = app.tr("toolbar.add_pane");
    let lbl_copy = app.tr("op.copy");
    let lbl_cut = app.tr("op.cut");
    let lbl_paste = app.tr("op.paste");
    let lbl_delete = app.tr("op.delete");
    let lbl_new_file = app.tr("op.new_file");
    let lbl_new_folder = app.tr("op.new_folder");

    if icon_button(ui, "◀", &lbl_back, can_back) {
        app.apply_action(Action::GoBack);
    }
    if icon_button(ui, "▶", &lbl_forward, can_forward) {
        app.apply_action(Action::GoForward);
    }
    if icon_button(ui, "▲", &lbl_up, true) {
        app.apply_action(Action::GoUp);
    }
    if icon_button(ui, "⟳", &lbl_refresh, true) {
        if let (Some(id), Some(dir)) = (
            app.workspace.active_id(),
            app.workspace.active_files().map(|f| f.current_dir.clone()),
        ) {
            app.refresh_pane(id, dir);
        }
    }
    ui.separator();
    // Operaciones de archivo: mismos disparadores que el teclado / menú contextual.
    if icon_button(ui, "⧉", &lbl_copy, true) {
        app.apply_action(Action::Copy);
    }
    if icon_button(ui, "✂", &lbl_cut, true) {
        app.apply_action(Action::Cut);
    }
    if icon_button(ui, "📋", &lbl_paste, true) {
        app.apply_action(Action::Paste);
    }
    if icon_button(ui, "🗑", &lbl_delete, true) {
        app.apply_action(Action::Delete);
    }
    if icon_button(ui, "🗋", &lbl_new_file, true) {
        app.apply_action(Action::NewFile);
    }
    if icon_button(ui, "🗀", &lbl_new_folder, true) {
        app.apply_action(Action::NewDir);
    }

    ui.separator();
    crate::templates_menu::layouts_button(ui, app);
    if icon_button(ui, "➕", &lbl_add_pane, true) {
        app.add_files_pane();
    }

    ui.separator();
    // Strip de unidades de acceso rápido. Clic → navegar el panel activo a la raíz.
    // Se construye `drive_roots` (clonando fuera de `app.drives_cache`) ANTES del
    // bucle para no tener a `app` prestado mientras el cuerpo llama a `app.*`.
    let drive_roots: Vec<(String, std::path::PathBuf)> = app
        .drives_cache
        .iter()
        .map(|d| (d.path.to_string_lossy().into_owned(), d.path.clone()))
        .collect();
    let mut navigate_to: Option<std::path::PathBuf> = None;
    for (label, path) in &drive_roots {
        // Mostrar la letra de unidad (p. ej. "C:") de forma compacta.
        let short = label.trim_end_matches(['\\', '/']).to_string();
        if icon_button(ui, &short, label, true) {
            navigate_to = Some(path.clone());
        }
    }
    if let Some(path) = navigate_to {
        app.navigate_active_to(path);
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
