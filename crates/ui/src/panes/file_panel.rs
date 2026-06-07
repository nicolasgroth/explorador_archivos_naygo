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
use egui_extras::{Column, TableBuilder};
use naygo_core::columns::{ColumnKind, TableState, MAX_COLUMN_WIDTH, MIN_COLUMN_WIDTH};
use naygo_core::fs_model::{Entry, SortSpec};
use naygo_core::icon_kind::{icon_key_for, IconKey};
use naygo_core::workspace::{PaneId, Workspace};
use std::time::SystemTime;

const ICON_SIZE: f32 = 16.0;

/// Alto de fila del cuerpo de la tabla (px lógicos). Constante para poder usar el
/// `TableBody::rows` virtualizado (solo pinta filas visibles), clave para la
/// prioridad del proyecto: navegar carpetas enormes sin congelar ni gastar de más.
const ROW_HEIGHT: f32 = 20.0;
/// Alto de la fila de encabezados.
const HEADER_HEIGHT: f32 = 22.0;
/// Umbral (px) para considerar que el usuario cambió el ancho de una columna. Evita
/// re-emitir `SetColumnWidth` cada frame por jitter de punto flotante (lo que
/// crearía un bucle de realimentación con el clamp de `set_width`).
const WIDTH_CHANGE_EPS: f32 = 0.5;

/// Una fila a pintar en el cuerpo de la tabla. Unifica la fila ".." (UI pura) con
/// las entries de la vista, para poder usar el `TableBody::rows` virtualizado: todas
/// las filas tienen el mismo alto y se indexan por posición.
enum DisplayRow {
    /// La fila ".." (subir al directorio padre). No es una `Entry`.
    Parent,
    /// Una entry de la vista filtrada/ordenada, en el índice dado dentro de `view`.
    Entry(usize),
    /// Aviso "sin coincidencias" (filtro activo dejó la vista vacía).
    NoMatches,
}

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
    theme: &crate::theme_apply::ActiveTheme,
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

    // Construir la lista de filas a pintar (unificada para el cuerpo virtualizado):
    // ".." (si corresponde) → entries de la vista (respetando `show_dirs`) → o el
    // aviso "sin coincidencias" si un filtro activo dejó la vista vacía.
    let mut rows: Vec<DisplayRow> = Vec::with_capacity(view.len() + 1);
    if parent.is_some() {
        rows.push(DisplayRow::Parent);
    }
    let mut painted_any = false;
    for (i, entry) in view.iter().enumerate() {
        if !show_dirs && entry.is_dir() {
            continue;
        }
        painted_any = true;
        rows.push(DisplayRow::Entry(i));
    }
    if !painted_any && has_active_filters {
        rows.push(DisplayRow::NoMatches);
    }

    // Anchos medidos de los encabezados ESTE frame (uno por columna visible). Los
    // capturamos desde el `Response` de cada celda de encabezado (su rect cubre el
    // ancho completo de la columna). Comparándolos con el ancho guardado detectamos
    // que el usuario arrastró el borde y emitimos `SetColumnWidth`. Es la vía pública
    // para leer el ancho de vuelta: `egui_extras` guarda los anchos en su propia
    // memoria (privada), pero el rect de la celda de encabezado los expone.
    let mut measured_widths: Vec<Option<f32>> = vec![None; visible_cols.len()];

    // Para pintar la línea de inserción al arrastrar una columna necesitamos un
    // painter, pero el closure del encabezado no expone uno fácilmente. Capturamos
    // el `Context` (handle Arc, clonado barato) ANTES de mover `ui` dentro del
    // `TableBuilder`, y una capa Foreground para que la línea quede SOBRE el header.
    let ctx = ui.ctx().clone();
    let drop_line_layer = egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new(("col_drop_line", id.0)),
    );

    // `TableBuilder` gestiona su propio `ScrollArea` (scroll vertical del cuerpo, con
    // el encabezado fijo arriba). NO lo envolvemos en otro `ScrollArea`.
    let mut builder = TableBuilder::new(ui)
        .id_salt(("file_table", id.0))
        .striped(true)
        .resizable(true)
        // Las celdas sensan clic para que `row.response()` (unión de las celdas de la
        // fila) registre clics en CUALQUIER celda/zona de la fila, no solo en Nombre.
        // Por defecto las celdas solo sensan hover y la fila completa no sería clicable.
        .sense(egui::Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
    for col in &visible_cols {
        // Ancho inicial = el guardado en el modelo; rango = límites de core. `clip`
        // permite encoger por debajo del contenido (si no, el texto largo impide
        // achicar la columna).
        builder = builder.column(
            Column::initial(col.width)
                .at_least(MIN_COLUMN_WIDTH)
                .at_most(MAX_COLUMN_WIDTH)
                .clip(true)
                .resizable(true),
        );
    }

    builder
        .header(HEADER_HEIGHT, |mut header| {
            // Encabezados: título + indicadores (▲/▼ si es la columna de orden, ≡ si
            // tiene filtro activo) + botón ▾ que abre el menú de columna. Cada
            // encabezado es FUENTE de arrastre (payload = índice REAL en
            // `table.columns`) y DESTINO: soltar A sobre B mueve A a la posición de B
            // (emite `TableAction::MoveColumn`).
            for (ci, col) in visible_cols.iter().enumerate() {
                let to_real = table
                    .columns
                    .iter()
                    .position(|c| c.kind == col.kind)
                    .unwrap();
                let dnd_id = egui::Id::new(("colhdr", id.0, col.kind));
                let (_, cell_resp) = header.col(|ui| {
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
                    // Soltar una columna sobre este encabezado la mueve aquí.
                    if let Some(from_real) = resp.dnd_release_payload::<usize>() {
                        if *from_real != to_real {
                            table_actions.push(TableAction::MoveColumn(*from_real, to_real));
                        }
                    }
                });
                // El rect de la celda de encabezado cubre el ancho completo de la
                // columna: lo guardamos para comparar con el ancho del modelo.
                measured_widths[ci] = Some(cell_resp.rect.width());

                // Indicador de drop: si se está arrastrando una columna y el cursor
                // está sobre este encabezado, pintar una línea de inserción azul en su
                // borde izquierdo ("caería antes de esta columna"). `dnd_hover_payload`
                // solo devuelve `Some` cuando el puntero está sobre la celda Y se
                // arrastra un payload `usize` (nuestro índice de columna), así que ya
                // implica "arrastrando una columna y sobre este header".
                if cell_resp.dnd_hover_payload::<usize>().is_some() {
                    let rect = cell_resp.rect;
                    ctx.layer_painter(drop_line_layer).vline(
                        rect.left(),
                        rect.y_range(),
                        egui::Stroke::new(2.0, theme.accent()),
                    );
                }
            }
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, rows.len(), |mut row| {
                let row_idx = row.index();
                match rows[row_idx] {
                    DisplayRow::Parent => {
                        // ".." se ve como una carpeta normal (estilo Total Commander):
                        // usa el ícono Folder, no uno especial de "subir".
                        for (ci, _col) in visible_cols.iter().enumerate() {
                            row.col(|ui| {
                                if ci == 0 {
                                    let _ = icon_row(ui, icons, IconKey::Folder, "..", false);
                                }
                            });
                        }
                        // Fila completa: ".." sube con un clic (o doble) en cualquier celda.
                        let row_resp = row.response();
                        if row_resp.clicked() || row_resp.double_clicked() {
                            parent_activated = true;
                        }
                    }
                    DisplayRow::Entry(i) => {
                        let entry = &view[i];
                        let selected = focused == Some(i);
                        row.set_selected(selected);
                        for (ci, col) in visible_cols.iter().enumerate() {
                            row.col(|ui| {
                                if ci == 0 {
                                    let key = icon_key_for(entry);
                                    // El ícono+nombre se pintan; el clic se captura sobre
                                    // la FILA completa (abajo), no por celda.
                                    let _ = icon_row(ui, icons, key, &entry.name, false);
                                } else {
                                    ui.label(cell_text(entry, col.kind));
                                }
                            });
                        }
                        // Fila completa clicable: clic en cualquier celda/zona selecciona;
                        // doble clic navega/activa.
                        let row_resp = row.response();
                        if row_resp.clicked() {
                            clicked = Some(i);
                        }
                        if row_resp.double_clicked() {
                            activated = Some(i);
                        }
                    }
                    DisplayRow::NoMatches => {
                        // Aviso en la primera columna; resto vacías.
                        for (ci, _col) in visible_cols.iter().enumerate() {
                            row.col(|ui| {
                                if ci == 0 {
                                    ui.weak(i18n.t("table.no_matches"));
                                }
                            });
                        }
                    }
                }
            });
        });

    // Detectar resize: si el ancho medido de una columna difiere del guardado en el
    // modelo, el usuario arrastró el borde → persistir el nuevo ancho. El clamp lo
    // hace `set_width` en core. El umbral evita re-emitir por jitter de float.
    for (ci, col) in visible_cols.iter().enumerate() {
        if let Some(w) = measured_widths[ci] {
            if (w - col.width).abs() > WIDTH_CHANGE_EPS {
                table_actions.push(TableAction::SetColumnWidth(col.kind, w));
            }
        }
    }

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

/// Pinta el encabezado de una columna: título + indicadores (orden ▲/▼, filtro ≡)
/// y un botón `▾` que abre el menú de columna (orden/filtro/columnas). El clic
/// derecho sobre el encabezado abre ese mismo menú. Acumula `TableAction`s. El id
/// del popup incluye el `PaneId` para que dos paneles que muestran la misma columna
/// no compartan el estado de UI.
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
    // El id del popup se calcula primero: lo usan tanto `Popup::menu(...).id(...)`
    // como la apertura por id al hacer clic derecho sobre el encabezado.
    let popup_id = ui.make_persistent_id(("col_menu", id.0, kind));
    let header_resp = ui
        .horizontal(|ui| {
            let mut title = column_title(kind, i18n);
            // Indicador de orden en la columna activa.
            if sort.key == naygo_core::columns::sort_key_of(kind) {
                title.push(' ');
                title.push(if sort.ascending { '▲' } else { '▼' });
            }
            // Indicador de filtro activo.
            if table.filters.contains_key(&kind) {
                title.push(' ');
                title.push('≡');
            }
            ui.label(egui::RichText::new(title).strong());

            // Botón ▾ que alterna el popup del menú de columna.
            let menu_button = ui.add(egui::Button::new("▾").frame(false));
            egui::Popup::menu(&menu_button).id(popup_id).show(|ui| {
                crate::column_menu::show_menu(ui, kind, table, sort, ext_counts, i18n, actions);
            });
        })
        .response;

    // Clic derecho en cualquier parte del encabezado abre el MISMO menú que el ▾.
    // Solo marca el popup como abierto en memoria; `Popup::menu(...).show` lo pinta
    // el frame siguiente. No re-ejecuta el contenido aquí, así que `actions` (que
    // ya dejó de estar prestado al cerrar el closure de arriba) no se necesita.
    if header_resp.secondary_clicked() {
        egui::Popup::open_id(ui.ctx(), popup_id);
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
