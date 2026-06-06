// Naygo — sección Idioma de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::i18n::LangId;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let title = app.tr("settings.language");
    ui.heading(title);
    ui.add_space(8.0);

    let langs: Vec<LangId> = app.i18n_available();
    let current = app.settings.language.clone();
    for lang in langs {
        let selected = lang == current;
        let key = format!("lang.{}", lang.as_str());
        let label = app.tr(&key);
        if ui.selectable_label(selected, label).clicked() {
            app.settings.language = lang;
        }
    }
}
