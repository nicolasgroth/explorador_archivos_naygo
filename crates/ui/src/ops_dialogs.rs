// Naygo — diálogos modales de operaciones (confirmar borrado / conflicto / nombre).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Modales que la UI muestra ANTES de lanzar una operación: confirmación de
//! borrado (papelera o permanente), resolución de conflicto (sobrescribir/saltar/
//! renombrar) y entrada de nombre (renombrar / crear archivo o carpeta). Cada
//! función pinta el modal cuando hay algo pendiente y reporta la decisión del
//! usuario; el estado "qué hay pendiente" vive en `NaygoApp::pending_dialog`.
//!
//! Diseño clave (Task 9): el conflicto se resuelve aquí, devolviendo UNA política
//! concreta que el llamador pone en `req.conflict` antes de `start_op`. Nunca se
//! pasa `ConflictPolicy::Ask` al motor (su canal de conflicto se descarta en ops-A).

use naygo_core::i18n::I18n;

/// Decisión del usuario ante un conflicto de nombre. Se mapea a `ConflictPolicy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictChoice {
    Overwrite,
    Skip,
    Rename,
}

/// Modal de confirmación de borrado. `permanent` elige el cuerpo (papelera vs.
/// irreversible). Devuelve `Some(true)` si se confirmó, `Some(false)` si se
/// canceló (o se cerró el modal), `None` si sigue abierto sin decisión.
pub fn confirm_delete(
    ctx: &egui::Context,
    i18n: &I18n,
    count: usize,
    permanent: bool,
) -> Option<bool> {
    let title = i18n.t("op.confirm_delete_title");
    let body_key = if permanent {
        "op.confirm_permanent_body"
    } else {
        "op.confirm_delete_body"
    };
    let body = i18n.t(body_key).replace("{n}", &count.to_string());

    let mut result = None;
    let resp = egui::Modal::new(egui::Id::new("naygo_confirm_delete")).show(ctx, |ui| {
        ui.set_min_width(320.0);
        ui.heading(title);
        ui.add_space(8.0);
        ui.label(body);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(i18n.t("op.delete")).clicked() {
                result = Some(true);
            }
            if ui.button(i18n.t("op.cancel")).clicked() {
                result = Some(false);
            }
        });
    });
    // Cerrar con Esc / clic fuera = cancelar.
    if result.is_none() && resp.should_close() {
        result = Some(false);
    }
    result
}

/// Modal de conflicto: «ya existe {name}». Devuelve la elección o `None` si sigue
/// abierto. Cerrar con Esc / clic fuera equivale a Saltar (no destructivo).
pub fn conflict(ctx: &egui::Context, i18n: &I18n, name: &str) -> Option<ConflictChoice> {
    let title = i18n.t("op.conflict_title");
    let body = i18n.t("op.conflict_body").replace("{name}", name);

    let mut result = None;
    let resp = egui::Modal::new(egui::Id::new("naygo_conflict")).show(ctx, |ui| {
        ui.set_min_width(360.0);
        ui.heading(title);
        ui.add_space(8.0);
        ui.label(body);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button(i18n.t("op.overwrite")).clicked() {
                result = Some(ConflictChoice::Overwrite);
            }
            if ui.button(i18n.t("op.skip")).clicked() {
                result = Some(ConflictChoice::Skip);
            }
            if ui.button(i18n.t("op.rename")).clicked() {
                result = Some(ConflictChoice::Rename);
            }
            if ui.button(i18n.t("op.cancel")).clicked() {
                result = Some(ConflictChoice::Skip);
            }
        });
    });
    if result.is_none() && resp.should_close() {
        result = Some(ConflictChoice::Skip);
    }
    result
}

/// Resultado de un modal de entrada de nombre.
pub enum NameResult {
    /// El usuario confirmó con un nombre válido.
    Confirmed(String),
    /// El usuario canceló (botón Cancelar / Esc / clic fuera).
    Cancelled,
}

/// Modal con un campo de texto + Aceptar/Cancelar, para renombrar o crear. `buf`
/// es el estado del campo (vive en `NaygoApp`). `title_key` es la clave i18n del
/// encabezado. Devuelve `None` mientras sigue abierto. Si el nombre es inválido,
/// el botón Aceptar queda deshabilitado y se muestra un aviso.
pub fn name_input(
    ctx: &egui::Context,
    i18n: &I18n,
    title_key: &str,
    buf: &mut String,
) -> Option<NameResult> {
    let mut result = None;
    let resp = egui::Modal::new(egui::Id::new("naygo_name_input")).show(ctx, |ui| {
        ui.set_min_width(340.0);
        ui.heading(i18n.t(title_key));
        ui.add_space(8.0);
        ui.label(i18n.t("op.name_label"));
        let edit = ui.add(egui::TextEdit::singleline(buf).desired_width(f32::INFINITY));
        edit.request_focus();
        let valid = naygo_core::ops::is_valid_name(buf.trim());
        if !buf.trim().is_empty() && !valid {
            ui.colored_label(
                egui::Color32::from_rgb(220, 80, 80),
                i18n.t("op.invalid_name"),
            );
        }
        // Enter en el campo confirma si es válido.
        let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let ok = ui.add_enabled(valid, egui::Button::new(i18n.t("op.ok")));
            if (ok.clicked() || enter) && valid {
                result = Some(NameResult::Confirmed(buf.trim().to_string()));
            }
            if ui.button(i18n.t("op.cancel")).clicked() {
                result = Some(NameResult::Cancelled);
            }
        });
    });
    if result.is_none() && resp.should_close() {
        result = Some(NameResult::Cancelled);
    }
    result
}

/// Decisión del usuario sobre las operaciones interrumpidas.
// Consumido en Task 7 (app.rs).
#[allow(dead_code)]
pub enum ResumeChoice {
    /// Retomar la operación con este id.
    Resume(String),
    /// Descartar la operación con este id.
    Discard(String),
    /// Retomar todas las pendientes.
    ResumeAll,
    /// Descartar todas.
    DiscardAll,
}

/// Modal de retomar: lista las operaciones interrumpidas (id + label + progreso) con
/// Retomar/Descartar por op, y Retomar todas / Descartar todas si hay más de una.
/// Devuelve `Some(choice)` cuando el usuario actúa, `None` mientras sigue abierto.
/// `items` = (id, label, done, total).
///
/// A diferencia de los otros modales, este NO se cierra solo con Esc / clic fuera:
/// la operación sigue pendiente y se vuelve a ofrecer. El llamador (Task 7) la quita
/// de la lista cuando llega una decisión, lo que hace desaparecer el modal.
// Consumido en Task 7 (app.rs).
#[allow(dead_code)]
pub fn resume_dialog(
    ctx: &egui::Context,
    i18n: &I18n,
    items: &[(String, String, usize, usize)],
) -> Option<ResumeChoice> {
    let mut choice = None;
    egui::Modal::new(egui::Id::new("resume_ops_modal")).show(ctx, |ui| {
        ui.set_max_width(440.0);
        ui.heading(i18n.t("resume.title"));
        ui.label(i18n.t("resume.body"));
        ui.add_space(6.0);
        for (id, label, done, total) in items {
            ui.group(|ui| {
                ui.label(egui::RichText::new(label).strong());
                let prog = i18n
                    .t("resume.progress")
                    .replace("{done}", &done.to_string())
                    .replace("{total}", &total.to_string());
                ui.label(egui::RichText::new(prog).weak());
                ui.horizontal(|ui| {
                    if ui.button(i18n.t("resume.resume")).clicked() {
                        choice = Some(ResumeChoice::Resume(id.clone()));
                    }
                    if ui.button(i18n.t("resume.discard")).clicked() {
                        choice = Some(ResumeChoice::Discard(id.clone()));
                    }
                });
            });
        }
        if items.len() > 1 {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(i18n.t("resume.resume_all")).clicked() {
                    choice = Some(ResumeChoice::ResumeAll);
                }
                if ui.button(i18n.t("resume.discard_all")).clicked() {
                    choice = Some(ResumeChoice::DiscardAll);
                }
            });
        }
    });
    choice
}
