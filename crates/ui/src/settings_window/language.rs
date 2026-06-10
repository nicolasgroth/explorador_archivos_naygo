// Naygo — sección Idioma de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::i18n::LangId;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (title, sub) = (app.tr("settings.language"), app.tr("settings.language.sub"));
    super::section_header(ui, &title, &sub);

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
