// Naygo — panel de archivos: tabla rica (columnas dinámicas) con íconos.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta las entradas del panel `id` en una tabla de columnas dinámicas (las que
//! `TableState` marca visibles, en su orden). Cada encabezado tiene un botón `▾`
//! que abre el menú de columna (orden + filtro en vivo + mostrar/ocultar). El
//! pipeline (filtrar → ordenar) se calcula EN MEMORIA sobre las entries clonadas;
//! no muta el estado del panel (eso lo hace `NaygoApp` con los `TableAction`s).
//! Respeta `show_dirs`. Si `show_parent_entry` y hay padre, pinta una fila ".."
//! arriba (UI pura, no una Entry). Clic selecciona; doble clic / Enter sobre
//! carpeta o ".." navega. No hace I/O.

use crate::docking::PaneRequest;
use crate::icons::IconProvider;
use crate::table_actions::TableAction;
use naygo_core::columns::{ColumnKind, TableState};
use naygo_core::fs_model::{Entry, SortSpec};
use naygo_core::icon_kind::{icon_key_for, IconKey};
use naygo_core::workspace::{PaneId, Workspace};
use std::time::SystemTime;

const ICON_SIZE: f32 = 16.0;

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    id: PaneId,
    pending: &mut Vec<PaneRequest>,
    icons: &IconProvider,
    show_parent_entry: bool,
    i18n: &naygo_core::i18n::I18n,
    table_actions: &mut Vec<TableAction>,
) {
    let Some(pane) = workspace.pane(id) else {
        return;
    };
    let Some(f) = pane.files.as_ref() else {
        return;
    };
    let focused = f.focused;
    let show_dirs = f.show_dirs;
    let sort = f.sort; // SortSpec es Copy; se lee antes de los closures
    let current_dir = f.current_dir.clone();
    let table = f.table.clone();
    let all_entries: Vec<Entry> = f.entries.clone();

    // Conteo de extensiones sobre TODAS las entries actuales (no las filtradas):
    // así el menú de filtro de tipo muestra todas las opciones disponibles.
    let ext_counts = naygo_core::filter::extension_counts(&all_entries);

    // Pipeline en memoria: filtrar (solo si hay filtros) → ordenar. No muta el
    // estado del panel.
    let mut view: Vec<Entry> = if table.filters.is_empty() {
        all_entries.clone()
    } else {
        all_entries
            .iter()
            .filter(|e| naygo_core::filter::matches(e, &table.filters))
            .cloned()
            .collect()
    };
    naygo_core::sort::sort_entries(&mut view, &sort);

    // ¿Mostrar la fila ".."? Solo si la opción está activa y hay carpeta padre.
    let parent = if show_parent_entry {
        current_dir.parent().map(|p| p.to_path_buf())
    } else {
        None
    };

    let has_active_filters = !table.filters.is_empty();

    ui.horizontal(|ui| {
        ui.monospace(current_dir.display().to_string());
    });
    ui.separator();

    // Índice de la entry seleccionada se referencia respecto a la VISTA filtrada/
    // ordenada (lo que el usuario ve), no respecto a `f.entries`. El `focused` del
    // panel también se interpreta sobre la vista (consistente con el pintado).
    let mut clicked: Option<usize> = None;
    let mut activated: Option<usize> = None;
    let mut parent_activated = false;

    let visible_cols: Vec<naygo_core::columns::ColumnSpec> =
        table.visible_columns().cloned().collect();
    let num_columns = visible_cols.len().max(1);

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new(("file_grid", id.0))
            .num_columns(num_columns)
            .striped(true)
            .show(ui, |ui| {
                // Encabezados: título + indicadores (▲/▼ si es la columna de orden,
                // ⏷ si tiene filtro activo) + botón ▾ que abre el menú de columna.
                // Cada encabezado es una FUENTE de arrastre (payload = índice REAL en
                // `table.columns`) y un DESTINO: soltar la columna A sobre la columna B
                // la mueve a la posición de B (emite `TableAction::MoveColumn`).
                for col in &visible_cols {
                    // Índice REAL de esta columna en `table.columns` (no el visible).
                    let to_real = table
                        .columns
                        .iter()
                        .position(|c| c.kind == col.kind)
                        .unwrap();
                    let dnd_id = egui::Id::new(("colhdr", id.0, col.kind));
                    let resp = ui
                        .dnd_drag_source(dnd_id, to_real, |ui| {
                            column_header(
                                ui,
                                id,
                                col.kind,
                                &table,
                                sort,
                                &ext_counts,
                                i18n,
                                table_actions,
                            );
                        })
                        .response;
                    // Si se soltó un payload sobre este encabezado, mover esa columna
                    // a esta posición.
                    if let Some(from_real) = resp.dnd_release_payload::<usize>() {
                        if *from_real != to_real {
                            table_actions.push(TableAction::MoveColumn(*from_real, to_real));
                        }
                    }
                }
                ui.end_row();

                // Fila ".." (si corresponde).
                if parent.is_some() {
                    // ".." se ve como una carpeta normal (estilo Total Commander):
                    // usa el ícono Folder en lugar de uno especial de "subir".
                    let resp = icon_row(ui, icons, IconKey::Folder, "..", false);
                    // ".." sube con UN solo clic (además del doble): no hay nada que
                    // "seleccionar" en ella, a diferencia de una carpeta real que
                    // selecciona con un clic y entra con doble. Asimetría intencional
                    // (estilo Total Commander); no "corregir" a solo-doble-clic.
                    if resp.double_clicked() || resp.clicked() {
                        parent_activated = true;
                    }
                    // Celdas vacías para las columnas restantes.
                    for _ in 1..visible_cols.len() {
                        ui.label("");
                    }
                    ui.end_row();
                }

                // Filas de la vista. Si la vista quedó vacía POR un filtro activo,
                // mostrar el aviso "sin coincidencias" en lugar de filas.
                let mut painted_any = false;
                for (i, entry) in view.iter().enumerate() {
                    if !show_dirs && entry.is_dir() {
                        continue;
                    }
                    painted_any = true;
                    let selected = focused == Some(i);
                    for (ci, col) in visible_cols.iter().enumerate() {
                        if ci == 0 {
                            let key = icon_key_for(entry);
                            let resp = icon_row(ui, icons, key, &entry.name, selected);
                            if resp.clicked() {
                                clicked = Some(i);
                            }
                            if resp.double_clicked() {
                                activated = Some(i);
                            }
                        } else {
                            ui.label(cell_text(entry, col.kind));
                        }
                    }
                    ui.end_row();
                }

                if !painted_any && has_active_filters {
                    ui.weak(i18n.t("table.no_matches"));
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
        if let Some(entry) = view.get(i) {
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

/// Pinta el encabezado de una columna: título + indicadores (orden ▲/▼, filtro ⏷)
/// y un botón `▾` que abre el menú de columna (orden/filtro/columnas). Acumula
/// `TableAction`s. El id del popup incluye el `PaneId` para que dos paneles que
/// muestran la misma columna no compartan el estado de UI.
#[allow(clippy::too_many_arguments)]
fn column_header(
    ui: &mut egui::Ui,
    id: PaneId,
    kind: ColumnKind,
    table: &TableState,
    sort: SortSpec,
    ext_counts: &std::collections::BTreeMap<String, usize>,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<TableAction>,
) {
    ui.horizontal(|ui| {
        let mut title = column_title(kind, i18n);
        // Indicador de orden en la columna activa.
        if sort.key == naygo_core::columns::sort_key_of(kind) {
            title.push(' ');
            title.push(if sort.ascending { '▲' } else { '▼' });
        }
        // Indicador de filtro activo (embudo).
        if table.filters.contains_key(&kind) {
            title.push(' ');
            title.push('⏷');
        }
        ui.label(egui::RichText::new(title).strong());

        // Botón ▾ que alterna el popup del menú de columna.
        let menu_button = ui.add(egui::Button::new("▾").frame(false));
        let popup_id = ui.make_persistent_id(("col_menu", id.0, kind));
        egui::Popup::menu(&menu_button).id(popup_id).show(|ui| {
            crate::column_menu::show_menu(ui, kind, table, sort, ext_counts, i18n, actions);
        });
    });
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

/// Texto de una celda según la columna (Nombre se pinta aparte con su ícono).
fn cell_text(entry: &Entry, kind: ColumnKind) -> String {
    match kind {
        ColumnKind::Name => entry.name.clone(),
        ColumnKind::Extension => naygo_core::filter::entry_extension(entry),
        ColumnKind::Size => format_size(entry),
        ColumnKind::Modified => format_time(entry.modified),
        ColumnKind::Created => format_time(entry.created),
    }
}

/// Título traducido de una columna (mapea a las claves `col.*`).
fn column_title(kind: ColumnKind, i18n: &naygo_core::i18n::I18n) -> String {
    let key = match kind {
        ColumnKind::Name => "col.name",
        ColumnKind::Extension => "col.extension",
        ColumnKind::Size => "col.size",
        ColumnKind::Modified => "col.modified",
        ColumnKind::Created => "col.created",
    };
    i18n.t(key).to_string()
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

/// PROVISIONAL: segundos epoch hasta tener formato de fecha (fase 2C). Reutilizado
/// para Modified y Created.
fn format_time(opt: Option<SystemTime>) -> String {
    use std::time::UNIX_EPOCH;
    match opt.and_then(|t| t.duration_since(UNIX_EPOCH).ok()) {
        Some(d) => format!("{}", d.as_secs()),
        None => String::new(),
    }
}
