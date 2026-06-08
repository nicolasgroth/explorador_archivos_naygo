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

/// Pinta el panel y devuelve los índices (en `active_ops`) cuyas ✕ se pulsaron.
pub fn show(
    ui: &mut egui::Ui,
    active_ops: &[ActiveOp],
    i18n: &I18n,
    expanded: &mut bool,
) -> Vec<usize> {
    let mut to_cancel = Vec::new();

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
                if op_row(ui, op, i18n, *expanded) {
                    to_cancel.push(i);
                }
                ui.separator();
            }
        });

    to_cancel
}

/// Pinta una fila de operación. Devuelve `true` si se pulsó su ✕ (cancelar).
fn op_row(ui: &mut egui::Ui, op: &ActiveOp, i18n: &I18n, expanded: bool) -> bool {
    let mut cancel = false;

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
            // Detalle/Exportar son refinamientos de Task 11; se dejan como stubs
            // deshabilitados para no prometer lo que aún no existe.
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new(i18n.t("ops.view_detail")).small());
                ui.add_enabled(false, egui::Button::new(i18n.t("ops.export")).small());
            });
        }
        return cancel;
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
                cancel = true;
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

    cancel
}
