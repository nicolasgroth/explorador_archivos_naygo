// Naygo — panel de archivos: vista Detalle (columnas) sobre un FilePaneState.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta las entradas del panel `id` del workspace en columnas. Respeta
//! `show_dirs` (oculta carpetas si está off). Clic selecciona; doble clic sobre
//! una carpeta emite una `NavigateTo`; clic en el panel lo activa. No hace I/O.

use crate::docking::PaneRequest;
use naygo_core::fs_model::{Entry, EntryKind};
use naygo_core::workspace::{PaneId, Workspace};

pub fn show(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    id: PaneId,
    pending: &mut Vec<PaneRequest>,
) {
    let Some(pane) = workspace.pane(id) else {
        return;
    };
    let Some(f) = pane.files.as_ref() else {
        return;
    };
    let focused = f.focused;
    let show_dirs = f.show_dirs;
    let current = f.current_dir.display().to_string();
    // Clonamos las entradas a pintar para no re-prestar `workspace` dentro de los
    // closures de ScrollArea/Grid. Aceptable en Fase 2A (se optimizará luego).
    let entries: Vec<Entry> = f.entries.clone();

    // Breadcrumb simple (la versión clicable fina es pulido posterior).
    ui.horizontal(|ui| {
        ui.monospace(current);
    });
    ui.separator();

    let mut clicked: Option<usize> = None;
    let mut activated: Option<usize> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new(("file_grid", id.0))
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Nombre");
                ui.strong("Tamaño");
                ui.strong("Modificado");
                ui.end_row();

                for (i, entry) in entries.iter().enumerate() {
                    if !show_dirs && entry.is_dir() {
                        continue;
                    }
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
        if let Some(f) = workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.focused = Some(i);
        }
        pending.push(PaneRequest::Activate { id });
    }
    if let Some(i) = activated {
        if let Some(entry) = entries.get(i) {
            if entry.is_dir() {
                pending.push(PaneRequest::Activate { id });
                pending.push(PaneRequest::NavigateTo {
                    id,
                    dir: entry.path.clone(),
                });
            }
        }
    }
}

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
        format!("{bytes} B")
    }
}

/// PROVISIONAL: segundos epoch hasta tener i18n (fase 2C).
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
