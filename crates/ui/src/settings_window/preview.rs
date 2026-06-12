// Naygo — sección Previsualización de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Tabla editable de reglas de previsualización: por extensión, un check on/off y un
//! alias opcional ("tratar como" otra extensión). Las imágenes y los textos aparecen
//! como filas. El cambio se persiste por el watcher de settings de `NaygoApp`.

use crate::app::NaygoApp;
use naygo_core::preview::PreviewRule;

/// Extensiones que ofrece el combo "tratar como" (concretas; el motor solo distingue
/// texto/imagen, pero mostrarlas es más claro y deja la puerta a resaltado futuro).
const TREAT_AS_OPTIONS: &[&str] = &[
    "txt", "log", "md", "json", "xml", "csv", "toml", "yaml", "ini", "html", "rs",
];

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (title, sub) = (app.tr("settings.preview"), app.tr("settings.preview.sub"));
    super::section_header(ui, &title, &sub);

    // Etiquetas resueltas ANTES de prestar `app.settings.preview_rules` en mutable.
    let l_ext = app.tr("settings.preview.col_ext");
    let l_on = app.tr("settings.preview.col_enabled");
    let l_as = app.tr("settings.preview.col_treat_as");
    let l_own = app.tr("settings.preview.treat_as_own");
    let l_add = app.tr("settings.preview.add");
    let l_remove = app.tr("settings.preview.remove");
    let l_hint = app.tr("settings.preview.hint");

    ui.label(egui::RichText::new(l_hint).weak().small());
    ui.add_space(6.0);

    // Encabezados de la tabla.
    ui.horizontal(|ui| {
        ui.add_sized(
            [90.0, 18.0],
            egui::Label::new(egui::RichText::new(l_ext).strong()),
        );
        ui.add_sized(
            [50.0, 18.0],
            egui::Label::new(egui::RichText::new(l_on).strong()),
        );
        ui.label(egui::RichText::new(l_as).strong());
    });

    // Índice a quitar tras pintar (no se puede mutar el Vec mientras se itera).
    let mut remove: Option<usize> = None;
    for (i, rule) in app.settings.preview_rules.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            // Extensión editable (corta).
            ui.add_sized(
                [90.0, 22.0],
                egui::TextEdit::singleline(&mut rule.ext).hint_text("ext"),
            );
            // Check on/off.
            ui.add_sized(
                [50.0, 22.0],
                egui::Checkbox::without_text(&mut rule.enabled),
            );
            // Combo "tratar como": (propia) o una extensión concreta.
            let current = rule.treat_as.clone().unwrap_or_else(|| l_own.clone());
            egui::ComboBox::from_id_salt(("treat_as", i))
                .selected_text(current)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(rule.treat_as.is_none(), &l_own)
                        .clicked()
                    {
                        rule.treat_as = None;
                    }
                    for opt in TREAT_AS_OPTIONS {
                        let sel = rule.treat_as.as_deref() == Some(*opt);
                        if ui.selectable_label(sel, *opt).clicked() {
                            rule.treat_as = Some((*opt).to_string());
                        }
                    }
                });
            if ui.button("🗑").on_hover_text(&l_remove).clicked() {
                remove = Some(i);
            }
        });
    }

    if let Some(i) = remove {
        app.settings.preview_rules.remove(i);
    }

    ui.add_space(6.0);
    if ui.button(format!("+ {l_add}")).clicked() {
        app.settings.preview_rules.push(PreviewRule {
            ext: String::new(),
            enabled: true,
            treat_as: None,
        });
    }
}
