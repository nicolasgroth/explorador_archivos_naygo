// Naygo — panel Historial: operaciones recientes con deshacer validado (R2b).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lista el historial de deshacer (más nuevo arriba). Cada entrada muestra etiqueta,
//! hace-cuánto y nº de ítems; su botón "Deshacer" se VALIDA al pintar: si el inverso
//! ya no aplica (rutas movidas/ocupadas por pasos posteriores), aparece deshabilitado
//! con el motivo en el tooltip — la protección acordada para deshacer fuera de orden.
//! "Deshacer hasta aquí" deshace en orden seguro (de lo más nuevo hacia esa entrada).
//! El panel NO ejecuta nada: emite ids a `undo_clicks` y `NaygoApp` los procesa tras
//! pintar (patrón de acciones diferidas). No hace I/O salvo los `exists()` de la
//! validación (metadata local, barata).

use naygo_core::ops::undo::{validate, UndoEntry};

/// Formatea "hace cuánto" a partir de epochs (sin chrono; relativo es suficiente).
fn ago(now_epoch: u64, when_epoch: u64, i18n: &naygo_core::i18n::I18n) -> String {
    let d = now_epoch.saturating_sub(when_epoch);
    if d < 60 {
        i18n.t("undo.ago_secs").replace("{n}", &d.to_string())
    } else if d < 3600 {
        i18n.t("undo.ago_min").replace("{n}", &(d / 60).to_string())
    } else {
        i18n.t("undo.ago_hours")
            .replace("{n}", &(d / 3600).to_string())
    }
}

pub fn show(
    ui: &mut egui::Ui,
    entries: &[UndoEntry],
    i18n: &naygo_core::i18n::I18n,
    theme: &crate::theme_apply::ActiveTheme,
    now_epoch: u64,
    undo_clicks: &mut Vec<u64>,
) {
    if entries.is_empty() {
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(i18n.t("undo.empty")).weak());
        });
        return;
    }

    let lbl_undo = i18n.t("undo.button").to_string();
    let lbl_upto = i18n.t("undo.upto").to_string();
    let lbl_undone = i18n.t("undo.undone").to_string();

    // Más nuevo arriba. `enumerate` sobre el orden original para "hasta aquí".
    for (idx, e) in entries.iter().enumerate().rev() {
        let items_lbl = i18n
            .t("undo.items")
            .replace("{n}", &e.actions.len().to_string());
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(&e.label)
                        .color(ui.visuals().strong_text_color())
                        .size(13.0),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {}",
                        ago(now_epoch, e.when_epoch_secs, i18n),
                        items_lbl
                    ))
                    .weak()
                    .size(11.0),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if e.undone {
                    ui.label(egui::RichText::new(&lbl_undone).weak().size(11.0));
                    return;
                }
                // Validación al pintar: el botón se deshabilita con el MOTIVO si el
                // inverso ya no aplica. exists() por acción: barato (metadata local).
                match validate(&e.actions) {
                    Ok(()) => {
                        if ui
                            .button(egui::RichText::new(&lbl_undo).color(theme.accent()))
                            .clicked()
                        {
                            undo_clicks.push(e.id);
                        }
                        // Deshacer en orden seguro desde lo más nuevo hasta ESTA
                        // entrada inclusive (solo si hay más nuevas sin deshacer).
                        let newer_pending = entries[idx + 1..].iter().any(|n| !n.undone);
                        if newer_pending && ui.small_button(&lbl_upto).clicked() {
                            for n in entries[idx..].iter().rev() {
                                if !n.undone {
                                    undo_clicks.push(n.id);
                                }
                            }
                        }
                    }
                    Err(reason) => {
                        ui.add_enabled(false, egui::Button::new(&lbl_undo))
                            .on_disabled_hover_text(i18n.t("undo.invalid").replace("{e}", &reason));
                    }
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
    }
}
