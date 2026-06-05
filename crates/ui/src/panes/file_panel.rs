// Naygo — panel de archivos: vista Detalle (columnas).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta las entradas del panel activo en columnas (Nombre, Tamaño, Modificado).
//! Clic selecciona; doble clic activa (entra a carpeta). El foco de teclado se
//! refleja resaltando la fila. No hace I/O: solo dibuja `state.pane.entries`.

use crate::app::UiState;
use naygo_core::fs_model::{Entry, EntryKind};

pub fn show(ui: &mut egui::Ui, state: &mut UiState) {
    let focused = state.pane.focused;
    let mut clicked: Option<usize> = None;
    let mut activated: Option<usize> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("file_grid")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Nombre");
                ui.strong("Tamaño");
                ui.strong("Modificado");
                ui.end_row();

                for (i, entry) in state.pane.entries.iter().enumerate() {
                    let selected = focused == Some(i);
                    let label = format!("{} {}", kind_glyph(entry.kind), entry.name);
                    let resp = ui.selectable_label(selected, label);
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                    if resp.double_clicked() {
                        activated = Some(i);
                    }
                    ui.label(format_size(entry));
                    ui.label(format_modified(entry));
                    ui.end_row();
                }
            });
    });

    if let Some(i) = clicked {
        state.pane.focused = Some(i);
    }
    if let Some(i) = activated {
        state.pane.focused = Some(i);
        state.apply_action(crate::input::Action::Activate);
    }
}

/// Glifo de texto provisional según el tipo. Los íconos reales del Shell llegan
/// con `platform::shell` en una fase posterior.
fn kind_glyph(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Directory => "[D]",
        EntryKind::File => "   ",
        EntryKind::Other => "[?]",
    }
}

fn format_size(entry: &Entry) -> String {
    match entry.size {
        Some(bytes) => human_size(bytes),
        None => String::new(),
    }
}

/// Formatea bytes en KB/MB/GB con un decimal.
fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// PROVISIONAL: muestra los segundos epoch como placeholder. El formato de fecha
/// legible (respetando i18n/locale) llega en una fase posterior junto con i18n.
fn format_modified(entry: &Entry) -> String {
    use std::time::UNIX_EPOCH;
    match entry
        .modified
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
    {
        Some(d) => format!("{}", d.as_secs()),
        None => String::new(),
    }
}
