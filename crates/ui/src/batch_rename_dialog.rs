// Naygo — diálogo de batch-rename (R3): campos + preview en vivo Antes→Después.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Modal del renombrado en lote. La UI solo EDITA el `BatchSpec` y pinta el
//! preview que calcula `core::batch_rename` (puro); cualquier cambio en un campo
//! recalcula las filas en memoria. Aplicar se habilita solo con `can_apply` (cero
//! inválidos/colisiones y al menos un cambio) y devuelve el `OpRequest` de UNA
//! sola operación `BatchRename` (journaled → deshacible desde el Historial).

use naygo_core::batch_rename::{
    can_apply, preview, BatchItem, BatchSpec, CaseTransform, PreviewRow, RowStatus,
};
use naygo_core::i18n::I18n;
use naygo_core::ops::{ConflictPolicy, OpKind, OpRequest};

/// Estado del diálogo (vive en `NaygoApp` mientras está abierto).
pub struct BatchRenameState {
    pub items: Vec<BatchItem>,
    /// Nombres actuales del directorio (para detección de colisiones).
    pub existing_names: Vec<String>,
    pub spec: BatchSpec,
    pub utc_offset_secs: i64,
    rows: Vec<PreviewRow>,
}

impl BatchRenameState {
    pub fn new(items: Vec<BatchItem>, existing_names: Vec<String>, utc_offset_secs: i64) -> Self {
        let spec = BatchSpec::default();
        let rows = preview(&items, &spec, &existing_names, utc_offset_secs);
        Self {
            items,
            existing_names,
            spec,
            utc_offset_secs,
            rows,
        }
    }

    fn recompute(&mut self) {
        self.rows = preview(
            &self.items,
            &self.spec,
            &self.existing_names,
            self.utc_offset_secs,
        );
    }
}

/// Resultado del frame del diálogo.
pub enum BatchDialogResult {
    /// Sigue abierto.
    Open,
    Cancelled,
    /// Aplicar: la op lista para `start_op`.
    Apply(OpRequest),
}

/// Pinta el modal. Llamar cada frame mientras el estado exista.
pub fn show(
    ctx: &egui::Context,
    i18n: &I18n,
    theme: &crate::theme_apply::ActiveTheme,
    state: &mut BatchRenameState,
) -> BatchDialogResult {
    let mut result = BatchDialogResult::Open;
    let mut changed = false;

    let resp = egui::Modal::new(egui::Id::new("naygo_batch_rename")).show(ctx, |ui| {
        ui.set_min_width(560.0);
        ui.heading(i18n.t("batch.title"));
        ui.label(
            egui::RichText::new(
                i18n.t("batch.count")
                    .replace("{n}", &state.items.len().to_string()),
            )
            .weak(),
        );
        ui.add_space(8.0);

        // ── Plantilla ──
        ui.horizontal(|ui| {
            ui.label(i18n.t("batch.template"));
            changed |= ui
                .add(egui::TextEdit::singleline(&mut state.spec.template).desired_width(260.0))
                .changed();
            changed |= ui
                .checkbox(&mut state.spec.include_ext, i18n.t("batch.include_ext"))
                .changed();
        });

        // Ayuda de comodines, plegada por defecto.
        ui.collapsing(i18n.t("batch.help"), |ui| {
            ui.label(
                egui::RichText::new(i18n.t("batch.help.text"))
                    .weak()
                    .monospace(),
            );
        });
        ui.add_space(6.0);

        // ── Buscar / reemplazar ──
        ui.horizontal(|ui| {
            ui.label(i18n.t("batch.find"));
            changed |= ui
                .add(egui::TextEdit::singleline(&mut state.spec.find).desired_width(150.0))
                .changed();
            ui.label(i18n.t("batch.replace"));
            changed |= ui
                .add(egui::TextEdit::singleline(&mut state.spec.replace).desired_width(150.0))
                .changed();
            changed |= ui
                .checkbox(&mut state.spec.use_regex, i18n.t("batch.regex"))
                .changed();
        });
        ui.add_space(6.0);

        // ── Mayúsculas + contador ──
        ui.horizontal(|ui| {
            ui.label(i18n.t("batch.case"));
            for (case, key) in [
                (CaseTransform::None, "batch.case.none"),
                (CaseTransform::Lower, "batch.case.lower"),
                (CaseTransform::Upper, "batch.case.upper"),
                (CaseTransform::Title, "batch.case.title"),
            ] {
                changed |= ui
                    .selectable_value(&mut state.spec.case, case, i18n.t(key))
                    .changed();
            }
        });
        ui.horizontal(|ui| {
            ui.label(i18n.t("batch.counter"));
            ui.label(egui::RichText::new(i18n.t("batch.counter.start")).weak());
            changed |= ui
                .add(egui::DragValue::new(&mut state.spec.counter_start).speed(1))
                .changed();
            ui.label(egui::RichText::new(i18n.t("batch.counter.step")).weak());
            changed |= ui
                .add(egui::DragValue::new(&mut state.spec.counter_step).speed(1))
                .changed();
        });
        ui.add_space(8.0);
        ui.separator();

        if changed {
            state.recompute();
        }

        // ── Preview Antes → Después ──
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                egui::Grid::new("batch_preview")
                    .num_columns(2)
                    .striped(true)
                    .min_col_width(250.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(i18n.t("batch.before")).strong());
                        ui.label(egui::RichText::new(i18n.t("batch.after")).strong());
                        ui.end_row();
                        for row in &state.rows {
                            ui.label(egui::RichText::new(&row.old_name).weak());
                            match &row.status {
                                RowStatus::Ok => {
                                    ui.label(&row.new_name);
                                }
                                RowStatus::Unchanged => {
                                    ui.label(
                                        egui::RichText::new(i18n.t("batch.unchanged"))
                                            .weak()
                                            .italics(),
                                    );
                                }
                                RowStatus::Invalid(reason) => {
                                    ui.colored_label(
                                        theme.error(),
                                        format!("⚠ {}", i18n.t("batch.invalid")),
                                    )
                                    .on_hover_text(reason);
                                }
                                RowStatus::Collision => {
                                    ui.colored_label(
                                        theme.error(),
                                        format!(
                                            "⚠ {} → {}",
                                            i18n.t("batch.collision"),
                                            row.new_name
                                        ),
                                    );
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
        ui.add_space(10.0);

        // ── Aplicar / Cancelar ──
        ui.horizontal(|ui| {
            let ok = can_apply(&state.rows);
            if ui
                .add_enabled(ok, egui::Button::new(i18n.t("batch.apply")))
                .clicked()
            {
                // Solo las filas con cambio real; el plan reordena por dependencia.
                let (sources, new_names): (Vec<_>, Vec<_>) = state
                    .rows
                    .iter()
                    .filter(|r| r.status == RowStatus::Ok)
                    .map(|r| (r.path.clone(), r.new_name.clone()))
                    .unzip();
                result = BatchDialogResult::Apply(OpRequest {
                    kind: OpKind::BatchRename { new_names },
                    sources,
                    dest_dir: None,
                    conflict: ConflictPolicy::Skip,
                });
            }
            if ui.button(i18n.t("batch.cancel")).clicked() {
                result = BatchDialogResult::Cancelled;
            }
        });
    });

    // Esc / clic fuera = cancelar.
    if matches!(result, BatchDialogResult::Open) && resp.should_close() {
        result = BatchDialogResult::Cancelled;
    }
    result
}
