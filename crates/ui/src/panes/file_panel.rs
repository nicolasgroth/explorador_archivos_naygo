// Naygo — panel de archivos: vista Detalle (columnas) con íconos sobre FilePaneState.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta las entradas del panel `id` en columnas, cada una con su ícono de tipo
//! (textura cacheada del set activo). Respeta `show_dirs`. Si `show_parent_entry`
//! y hay padre, pinta una fila ".." arriba (UI pura, no una Entry). Clic
//! selecciona; doble clic / Enter sobre carpeta o ".." navega. No hace I/O.

use crate::docking::PaneRequest;
use crate::icons::IconProvider;
use naygo_core::fs_model::Entry;
use naygo_core::icon_kind::{icon_key_for, IconKey};
use naygo_core::workspace::{PaneId, Workspace};

const ICON_SIZE: f32 = 16.0;

pub fn show(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    id: PaneId,
    pending: &mut Vec<PaneRequest>,
    icons: &IconProvider,
    show_parent_entry: bool,
    i18n: &naygo_core::i18n::I18n,
) {
    let Some(pane) = workspace.pane(id) else {
        return;
    };
    let Some(f) = pane.files.as_ref() else {
        return;
    };
    let focused = f.focused;
    let show_dirs = f.show_dirs;
    let current_dir = f.current_dir.clone();
    let entries: Vec<Entry> = f.entries.clone();

    // ¿Mostrar la fila ".."? Solo si la opción está activa y hay carpeta padre.
    let parent = if show_parent_entry {
        current_dir.parent().map(|p| p.to_path_buf())
    } else {
        None
    };

    ui.horizontal(|ui| {
        ui.monospace(current_dir.display().to_string());
    });
    ui.separator();

    let mut clicked: Option<usize> = None;
    let mut activated: Option<usize> = None;
    let mut parent_activated = false;

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new(("file_grid", id.0))
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.strong(i18n.t("col.name"));
                ui.strong(i18n.t("col.size"));
                ui.strong(i18n.t("col.modified"));
                ui.end_row();

                // Fila ".." (si corresponde).
                if parent.is_some() {
                    let resp = icon_row(ui, icons, IconKey::ParentDir, "..", false);
                    // ".." sube con UN solo clic (además del doble): no hay nada que
                    // "seleccionar" en ella, a diferencia de una carpeta real que
                    // selecciona con un clic y entra con doble. Asimetría intencional
                    // (estilo Total Commander); no "corregir" a solo-doble-clic.
                    if resp.double_clicked() || resp.clicked() {
                        parent_activated = true;
                    }
                    ui.label("");
                    ui.label("");
                    ui.end_row();
                }

                for (i, entry) in entries.iter().enumerate() {
                    if !show_dirs && entry.is_dir() {
                        continue;
                    }
                    let selected = focused == Some(i);
                    let key = icon_key_for(entry);
                    let resp = icon_row(ui, icons, key, &entry.name, selected);
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

    if parent_activated {
        if let Some(dir) = parent {
            pending.push(PaneRequest::Activate { id });
            pending.push(PaneRequest::NavigateTo { id, dir });
        }
    }
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

/// Pinta una fila "[ícono] nombre" como un único elemento clicable. Devuelve el
/// `Response` combinado del ícono Y el label, así clicar en cualquiera de los dos
/// (incluida el área del ícono) selecciona/activa la fila.
fn icon_row(
    ui: &mut egui::Ui,
    icons: &IconProvider,
    key: IconKey,
    name: &str,
    selected: bool,
) -> egui::Response {
    ui.horizontal(|ui| {
        let tex = icons.texture(key);
        // `sense` clicks en la imagen para que el ícono no sea un hueco muerto.
        let img = ui.add(
            egui::Image::new(tex)
                .fit_to_exact_size(egui::vec2(ICON_SIZE, ICON_SIZE))
                .sense(egui::Sense::click()),
        );
        let label = ui.selectable_label(selected, name);
        img.union(label)
    })
    .inner
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
