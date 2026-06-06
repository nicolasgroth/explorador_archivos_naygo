// Naygo — sección Atajos (solo-lectura) de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let title = app.tr("settings.shortcuts");
    ui.heading(title);
    ui.add_space(8.0);
    let note = app.tr("settings.shortcuts.readonly");
    ui.label(egui::RichText::new(note).weak());
    ui.add_space(6.0);

    let rows: &[(&str, &str)] = &[
        ("↑ / ↓", "shortcut.move"),
        ("Enter", "shortcut.activate"),
        ("Backspace", "shortcut.up"),
        ("Alt + ← / →", "shortcut.backforward"),
        ("Tab", "shortcut.switchpane"),
        ("Esc", "shortcut.cancel"),
    ];
    egui::Grid::new("shortcuts_grid")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for (keys, desc_key) in rows {
                ui.monospace(*keys);
                let desc = app.tr(desc_key);
                ui.label(desc);
                ui.end_row();
            }
        });
}
