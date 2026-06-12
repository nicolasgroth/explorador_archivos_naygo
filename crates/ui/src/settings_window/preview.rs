// Naygo — sección Previsualización de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Ajustes del panel Preview: la lista (editable) de extensiones de texto que se
//! previsualizan. Las imágenes son fijas. El cambio se persiste por el watcher de
//! settings de `NaygoApp` (comparación con `last_saved_settings`).

use crate::app::NaygoApp;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (title, sub) = (app.tr("settings.preview"), app.tr("settings.preview.sub"));
    super::section_header(ui, &title, &sub);

    ui.label(app.tr("settings.preview.text_exts"));
    // Edición directa del CSV de extensiones (el parseo a lista normalizada lo hace
    // `core::preview` cuando el worker arranca; aquí solo se persiste el texto crudo).
    ui.add(
        egui::TextEdit::singleline(&mut app.settings.preview_text_exts)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(app.tr("settings.preview.text_exts_hint"))
            .weak()
            .small(),
    );
}
