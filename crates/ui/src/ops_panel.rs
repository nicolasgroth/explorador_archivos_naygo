// Naygo — panel de operaciones en curso (progreso + cancelar + resumen).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta el panel acoplado de operaciones de archivo. Por cada op muestra una
//! línea compacta (etiqueta, barra de progreso ligada a bytes reales, porcentaje
//! y botón de cancelar) y, si el panel está expandido, una línea de detalle con los
//! archivos hechos/total, los bytes y el archivo actual. Las ops terminadas
//! muestran su resumen (hechos/omitidos/con error). No hace I/O ni lógica de
//! negocio: solo pinta `ActiveOp`s y devuelve los índices de las ops a cancelar.

use crate::app::ActiveOp;
use crate::panes::file_panel::human_size;
use naygo_core::i18n::I18n;
use naygo_core::ops::OpOutcome;

/// Color de error para outcomes `Failed`. Igual al usado en el resto de la app
/// cuando no se dispone del color del tema activo en este punto.
const ERROR_COLOR: egui::Color32 = egui::Color32::from_rgb(0xe0, 0x6c, 0x5b);

/// Resultado de pintar el panel: índices a cancelar + (opcional) índice de la op
/// cuyo resumen se pidió exportar este frame. `app.rs` resuelve el resto (el
/// panel no conoce el directorio destino ni hace I/O).
#[derive(Default)]
pub struct OpsPanelOutput {
    /// Índices (en `active_ops`) cuyas ✕ se pulsaron.
    pub cancel: Vec<usize>,
    /// Índice de la op cuyo resumen se pidió exportar (botón "Exportar…").
    pub export: Option<usize>,
}

/// Pinta el panel y devuelve qué ops cancelar/exportar (ver [`OpsPanelOutput`]).
pub fn show(
    ui: &mut egui::Ui,
    active_ops: &[ActiveOp],
    i18n: &I18n,
    expanded: &mut bool,
) -> OpsPanelOutput {
    let mut out = OpsPanelOutput::default();

    ui.horizontal(|ui| {
        ui.strong(i18n.t("ops.panel_title"));
        ui.separator();
        let toggle_label = if *expanded {
            i18n.t("ops.collapse")
        } else {
            i18n.t("ops.expand")
        };
        if ui.small_button(toggle_label).clicked() {
            *expanded = !*expanded;
        }
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(160.0)
        .show(ui, |ui| {
            for (i, op) in active_ops.iter().enumerate() {
                let row = op_row(ui, op, i18n, *expanded, i);
                if row.cancel {
                    out.cancel.push(i);
                }
                if row.export {
                    out.export = Some(i);
                }
                ui.separator();
            }
        });

    out
}

/// Acciones que una fila pudo disparar este frame.
#[derive(Default)]
struct RowActions {
    cancel: bool,
    export: bool,
}

/// Pinta una fila de operación. `index` identifica la op de forma estable dentro
/// del frame para anclar el estado del desplegable "Ver detalle" en la memoria de
/// egui.
fn op_row(
    ui: &mut egui::Ui,
    op: &ActiveOp,
    i18n: &I18n,
    expanded: bool,
    index: usize,
) -> RowActions {
    let mut actions = RowActions::default();

    // Línea compacta: etiqueta + barra + ✕. Si ya terminó (hay summary), se muestra
    // el resumen en vez de la barra y sin botón de cancelar.
    if let Some(summary) = &op.summary {
        ui.horizontal(|ui| {
            ui.label(&op.label);
        });
        let line = i18n
            .t("ops.summary_done")
            .replace("{done}", &summary.count_done().to_string())
            .replace("{skipped}", &summary.count_skipped().to_string())
            .replace("{failed}", &summary.count_failed().to_string());
        ui.weak(line);
        if expanded {
            // Estado del desplegable "Ver detalle": un bool por op anclado en la
            // memoria de egui. Se conserva entre frames sin tocar `ActiveOp`.
            let detail_id = ui.id().with(("ops_detail", index));
            let mut detail_open = ui.data_mut(|d| d.get_temp::<bool>(detail_id).unwrap_or(false));

            ui.horizontal(|ui| {
                if ui.small_button(i18n.t("ops.view_detail")).clicked() {
                    detail_open = !detail_open;
                    ui.data_mut(|d| d.insert_temp(detail_id, detail_open));
                }
                if ui.small_button(i18n.t("ops.export")).clicked() {
                    actions.export = true;
                }
            });

            if detail_open {
                detail_list(ui, summary);
            }
        }
        return actions;
    }

    // En curso (o en cola): barra de progreso + ✕.
    let (fraction, percent_text) = match &op.progress {
        Some(p) if p.bytes_total > 0 => {
            let frac = (p.bytes_done as f32 / p.bytes_total as f32).clamp(0.0, 1.0);
            (frac, format!("{}%", (frac * 100.0).round() as u32))
        }
        // Sin total de bytes (p. ej. crear carpeta) o aún sin progreso: barra
        // indeterminada-ish (0.0 animada) para señalar actividad.
        _ => (0.0, String::new()),
    };

    ui.horizontal(|ui| {
        ui.label(&op.label);
        if !op.started {
            ui.weak(format!("({})", i18n.t("ops.queued")));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("✕").clicked() {
                actions.cancel = true;
            }
        });
    });

    let mut bar = egui::ProgressBar::new(fraction).animate(true);
    if !percent_text.is_empty() {
        bar = bar.text(percent_text);
    }
    ui.add(bar);

    if expanded {
        if let Some(p) = &op.progress {
            let files = format!("{}/{}", p.files_done, p.files_total);
            let bytes = format!(
                "{} / {}",
                human_size(p.bytes_done),
                human_size(p.bytes_total)
            );
            ui.weak(format!("{files} · {bytes}"));
            if !p.current.as_os_str().is_empty() {
                let name = p
                    .current
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.current.display().to_string());
                ui.weak(name);
            }
        }
    }

    actions
}

/// Pinta la lista por archivo del resumen: nombre + glifo de outcome (✓ hecho,
/// – omitido, ⚠ con error + motivo en color de error).
fn detail_list(ui: &mut egui::Ui, summary: &naygo_core::ops::OpSummary) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .max_height(120.0)
        .show(ui, |ui| {
            for (path, outcome) in &summary.items {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                ui.horizontal(|ui| match outcome {
                    OpOutcome::Done => {
                        ui.weak("✓");
                        ui.label(name);
                    }
                    OpOutcome::Skipped => {
                        ui.weak("–");
                        ui.weak(name);
                    }
                    OpOutcome::Failed(reason) => {
                        ui.colored_label(ERROR_COLOR, "⚠");
                        ui.label(name);
                        ui.colored_label(ERROR_COLOR, reason);
                    }
                });
            }
        });
}
