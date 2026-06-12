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
use crate::input::Action;
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
    ops_actions: &mut Vec<Action>,
    native_menu_request: &mut Option<(f32, f32)>,
    new_items_at_end: bool,
    size_partial: &std::collections::HashSet<std::path::PathBuf>,
    inline_rename: &mut Option<crate::app::InlineRename>,
    pathbar: crate::pathbar::PathBarParams<'_>,
    // Si `Some(pos)`, el cuerpo de la tabla hace scroll para que la fila de la posición
    // de vista `pos` quede visible (foco movido por teclado este frame).
    scroll_to: Option<usize>,
    // Salida: filas visibles del cuerpo medidas este frame (alto disponible / ROW_HEIGHT).
    // `NaygoApp` la guarda por panel para el tamaño de página de AvPag/RePag.
    visible_rows_out: &mut Option<usize>,
    // Modo de ancho de columnas: en `Auto` la tabla reparte por contenido (no se persiste
    // el resize manual); en `Fixed` se usan los anchos del `TableState` (resizable a mano).
    width_mode: naygo_core::config::ColumnWidthMode,
) {
    let Some(pane) = workspace.pane(id) else {
        return;
    };
    let Some(f) = pane.files.as_ref() else {
        return;
    };
    let focused = f.focused;
    // Posiciones de vista seleccionadas (Vec pequeño). Se clona antes de la tabla
    // para usarlo dentro del closure de `body.rows` sin conflictos de préstamo,
    // igual que `focused`. Pintamos TODA la selección, no solo el foco.
    let selected_set = f.selected.clone();
    let show_dirs = f.show_dirs;
    let sort = f.sort; // SortSpec es Copy; se lee antes de los closures
    let current_dir = f.current_dir.clone();
    let table = f.table.clone();
    let all_entries: Vec<Entry> = f.entries.clone();
    let highlighted: std::collections::HashSet<std::path::PathBuf> = f.highlighted.clone();

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

    // Modo "al final": agrupar las entries resaltadas (nuevas) al final, estable.
    // `sort_by_key` con bool es estable → primero las no resaltadas (false), luego las
    // resaltadas (true), sin alterar el orden relativo dentro de cada grupo.
    if new_items_at_end && !highlighted.is_empty() {
        view.sort_by_key(|e| highlighted.contains(&e.path));
    }

    // ¿Mostrar la fila ".."? Solo si la opción está activa y hay carpeta padre.
    let parent = if show_parent_entry {
        current_dir.parent().map(|p| p.to_path_buf())
    } else {
        None
    };

    let has_active_filters = !table.filters.is_empty();

    // Path-bar interactiva (breadcrumbs / edición con autocompletado). Sus widgets
    // clicables viven FUERA de la tabla, así que no roban clics de las filas.
    crate::pathbar::show(ui, id, &current_dir, pending, i18n, theme, pathbar);
    ui.separator();

    // Índice de la entry seleccionada se referencia respecto a la VISTA filtrada/
    // ordenada (lo que el usuario ve), no respecto a `f.entries`. El `focused` del
    // panel también se interpreta sobre la vista (consistente con el pintado).
    let mut clicked: Option<(usize, bool, bool)> = None; // (pos en vista, ctrl, shift)
    let mut activated: Option<usize> = None;
    let mut parent_activated = false;
    // Posición de vista cuya celda Nombre empezó a arrastrarse este frame → dispara el
    // arrastre OLE hacia el SO (Naygo → Explorer). Se resuelve a rutas tras la tabla y se
    // difiere a `NaygoApp` (que llama a `platform::dnd::start_drag` fuera del closure egui).
    let mut os_drag_start: Option<usize> = None;
    // Fila sobre la que se abrió el menú contextual (para enfocarla antes de actuar).
    let mut context_focus: Option<usize> = None;
    // Señales del rename inline (F2) emitidas desde la celda Nombre este frame; se
    // procesan tras la tabla (donde vive la `view`). `rename_commit` = confirmar el
    // texto actual; `rename_travel` = además moverse a la fila pos±1; `rename_cancel`
    // = Esc. Solo aplican si `inline_rename` apunta a ESTE panel.
    let mut rename_commit = false;
    let mut rename_travel: Option<i32> = None;
    let mut rename_cancel = false;

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

    // Rectángulos de fila capturados ESTE frame: (posición en la vista, rect de la fila
    // completa), en coordenadas de pantalla. Doble uso: el rubber-band selecciona lo que
    // toque, y un press FUERA de toda fila es el inicio válido del rubber-band.
    let mut row_rects: Vec<(usize, egui::Rect)> = Vec::new();
    // Rect de la fila con el foco/ancla. Dentro del closure de `body.rows` el `ui` ya
    // está movido al `TableBuilder`, así que no podemos pintar ahí; capturamos el rect
    // y dibujamos el borde punteado tras construir la tabla (con el `ui` padre, igual
    // que el rubber-band). Coordenadas de pantalla.
    let mut focus_rect: Option<egui::Rect> = None;

    // Filas visibles del cuerpo: el alto disponible ANTES de armar la tabla (ya restada la
    // path-bar y el separador) menos el encabezado, dividido por el alto de fila. Lo usa
    // AvPag/RePag como tamaño de página. Se mide cada frame y se devuelve por `visible_rows_out`.
    let body_height = (ui.available_height() - HEADER_HEIGHT).max(0.0);
    *visible_rows_out = Some(((body_height / ROW_HEIGHT).floor() as usize).max(1));

    // Si el foco se movió por teclado este frame, traducir la posición de vista al índice
    // de fila de la tabla (que incluye la fila ".." si está) para pedir el scroll.
    let scroll_to_row_idx: Option<usize> = scroll_to.and_then(|pos| {
        rows.iter()
            .position(|r| matches!(r, DisplayRow::Entry(i) if *i == pos))
    });

    // `TableBuilder` gestiona su propio `ScrollArea` (scroll vertical del cuerpo, con
    // el encabezado fijo arriba). NO lo envolvemos en otro `ScrollArea`.
    let mut builder = TableBuilder::new(ui)
        .id_salt(("file_table", id.0))
        .striped(true)
        .resizable(true)
        // La FILA es la ÚNICA superficie de interacción del mouse: sensa clic Y arrastre.
        // Con ambos senses, egui pospone la decisión clic-vs-arrastre hasta superar el
        // umbral de movimiento (interaction.rs: "could be either, so we postpone the
        // decision until we know") → el clic selecciona y el arrastre real inicia el
        // drag&drop OLE, sin pisarse. LECCIONES (saga de clics muertos): (1) NO apilar
        // interacts drag-only sobre las filas — quedan "dragging" desde el press sin
        // umbral y roban el hit-test; (2) NINGÚN widget de celda puede sensar clic
        // (labels seleccionables incluidos) — queda encima de la fila, recibe el clic
        // y lo descarta (ver `icon_row` y `selectable_labels=false` en theme_apply).
        .sense(egui::Sense::click_and_drag())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
    // Foco movido por teclado: hacer scroll para que la fila quede visible. `align = None`
    // desplaza lo MÍNIMO para que la fila entre en vista (no recentra si ya se ve), que es
    // el comportamiento de Explorer tanto para flechas como para AvPag/Inicio/Fin.
    if let Some(row_idx) = scroll_to_row_idx {
        builder = builder.scroll_to_row(row_idx, None);
    }
    let auto_width = matches!(width_mode, naygo_core::config::ColumnWidthMode::Auto);
    for col in &visible_cols {
        let column = if auto_width {
            // Modo Auto: la tabla reparte el ancho según el contenido. NO resizable (el
            // resize manual no aplica) y NO se emite SetColumnWidth (ver más abajo), para
            // no pelear con egui_extras ni persistir anchos que el usuario no fijó.
            Column::auto().at_least(MIN_COLUMN_WIDTH).clip(true)
        } else {
            // Modo Fijo: ancho inicial = el guardado en el modelo; rango = límites de core.
            // `clip` permite encoger por debajo del contenido (si no, el texto largo impide
            // achicar la columna).
            Column::initial(col.width)
                .at_least(MIN_COLUMN_WIDTH)
                .at_most(MAX_COLUMN_WIDTH)
                .clip(true)
                .resizable(true)
        };
        builder = builder.column(column);
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
                                    let _ = icon_row(ui, icons, IconKey::Folder, "..", None);
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
                        // `selected` refleja TODA la multi-selección (no solo el foco):
                        // así todas las filas seleccionadas se pintan resaltadas.
                        let selected = selected_set.contains(&i);
                        // El foco/ancla es la fila que el teclado mueve; se distingue
                        // del resto de la selección con un borde punteado (abajo).
                        let is_focus = focused == Some(i);
                        // Resaltado estilo A: las entries que el watcher marcó como recién
                        // aparecidas se pintan con el fondo teñido del token `highlight` y
                        // el nombre en ese color. La selección tiene prioridad sobre el
                        // resaltado (si la fila está seleccionada, manda la selección y no
                        // se tiñe ni se colorea el nombre), para no confundir ambos estados.
                        let is_new = !selected && highlighted.contains(&entry.path);
                        row.set_selected(selected);
                        // Fondo teñido a baja opacidad (~18%) sobre el color base del tema.
                        let new_tint = if is_new {
                            let base = theme.highlight();
                            Some(egui::Color32::from_rgba_unmultiplied(
                                base.r(),
                                base.g(),
                                base.b(),
                                46,
                            ))
                        } else {
                            None
                        };
                        for (ci, col) in visible_cols.iter().enumerate() {
                            row.col(|ui| {
                                // Teñir el fondo de la celda (cubre el ancho completo de la
                                // columna) DETRÁS del contenido. Se hace por celda porque
                                // `egui_extras` no expone un hook de fondo de fila completa;
                                // la unión de las celdas cubre toda la fila.
                                if let Some(tint) = new_tint {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, tint);
                                }
                                if ci == 0 {
                                    let key = icon_key_for(entry);
                                    // El ícono+nombre se pintan; el clic se captura sobre
                                    // la FILA completa (abajo), no por celda. Si es nuevo,
                                    // el nombre va en el color de resaltado.
                                    let name_color = if is_new {
                                        Some(theme.highlight())
                                    } else {
                                        None
                                    };
                                    // ¿Esta fila está en rename inline (F2)? → TextEdit en
                                    // lugar del label. Las teclas del gesto (F2/↑/↓/Esc) se
                                    // CONSUMEN antes de crear el TextEdit para que no las vea.
                                    let editing = inline_rename
                                        .as_ref()
                                        .is_some_and(|r| r.pane == id && r.path == entry.path);
                                    if editing {
                                        let r =
                                            inline_rename.as_mut().expect("editing implica Some");
                                        let f2 = ui.input_mut(|inp| {
                                            inp.consume_key(egui::Modifiers::NONE, egui::Key::F2)
                                        });
                                        let up = ui.input_mut(|inp| {
                                            inp.consume_key(
                                                egui::Modifiers::NONE,
                                                egui::Key::ArrowUp,
                                            )
                                        });
                                        let down = ui.input_mut(|inp| {
                                            inp.consume_key(
                                                egui::Modifiers::NONE,
                                                egui::Key::ArrowDown,
                                            )
                                        });
                                        let esc = ui.input_mut(|inp| {
                                            inp.consume_key(
                                                egui::Modifiers::NONE,
                                                egui::Key::Escape,
                                            )
                                        });
                                        let tex = icons.texture(key);
                                        ui.add(
                                            egui::Image::new(tex).fit_to_exact_size(egui::vec2(
                                                ICON_SIZE, ICON_SIZE,
                                            )),
                                        );
                                        let te_id = egui::Id::new(("naygo_inline_rename", id.0));
                                        // El F2 que ABRIO el rename (handler global) puede seguir
                                        // en el input de este mismo frame: la celda lo consumiria
                                        // y ciclaria la etapa de mas. Recien creado (focus aun
                                        // pendiente) ese F2 se ignora.
                                        let f2 = f2 && !r.focus_pending;
                                        if f2 {
                                            r.stage = (r.stage + 1) % 3;
                                            r.select_pending = true;
                                        }
                                        // ORDEN CRITICO: la seleccion se aplica ANTES de crear el
                                        // TextEdit (el widget carga su estado al crearse; despues
                                        // llega 1-2 frames tarde y el tipeo usa el rango viejo).
                                        if r.select_pending {
                                            apply_rename_selection(
                                                ui.ctx(),
                                                te_id,
                                                &r.text,
                                                r.stage,
                                            );
                                            r.select_pending = false;
                                        }
                                        let resp = ui.add(
                                            egui::TextEdit::singleline(&mut r.text)
                                                .id(te_id)
                                                .desired_width(ui.available_width().max(80.0))
                                                .margin(egui::vec2(4.0, 0.0)),
                                        );
                                        if r.focus_pending {
                                            resp.request_focus();
                                            r.focus_pending = false;
                                        }
                                        if esc {
                                            rename_cancel = true;
                                        } else if up {
                                            rename_commit = true;
                                            rename_travel = Some(-1);
                                        } else if down {
                                            rename_commit = true;
                                            rename_travel = Some(1);
                                        } else if resp.lost_focus() {
                                            // Enter o clic afuera: ambos confirman (Esc ya
                                            // se consumió arriba y no llega acá).
                                            rename_commit = true;
                                        }
                                    } else {
                                        // El arrastre de archivos lo sensa la FILA completa
                                        // (como Explorer en detalles); acá solo se pinta.
                                        let _ = icon_row(ui, icons, key, &entry.name, name_color);
                                    }
                                } else {
                                    let mut text = cell_text(entry, col.kind);
                                    if col.kind == ColumnKind::Size
                                        && entry.size.is_some()
                                        && size_partial.contains(&entry.path)
                                    {
                                        text.push_str(i18n.t("size.partial_suffix"));
                                    }
                                    ui.label(text);
                                }
                            });
                        }
                        // Fila completa clicable: clic en cualquier celda/zona selecciona;
                        // doble clic navega/activa; arrastre real inicia el drag OLE.
                        let row_resp = row.response();
                        // Capturar rects para el rubber-band (Step 1).
                        row_rects.push((i, row_resp.rect));
                        // Arrastre de archivos: la fila sensa click_and_drag, así que
                        // `drag_started()` SOLO dispara cuando el gesto superó el umbral
                        // clic-vs-arrastre de egui (decisión pospuesta) — un clic simple
                        // jamás llega acá. Es un evento de transición: un disparo por gesto.
                        if row_resp.drag_started() {
                            os_drag_start = Some(i);
                        }
                        // Guardar el rect de la fila con foco para pintar su borde
                        // punteado tras la tabla (el `ui` aquí ya está movido).
                        if is_focus {
                            focus_rect = Some(row_resp.rect);
                        }
                        if row_resp.clicked() {
                            // `ui` ya está movido dentro del `TableBuilder`; leemos los
                            // modificadores desde el `Context` capturado (ctx.input).
                            let (ctrl, shift) = ctx.input(|inp| {
                                (
                                    inp.modifiers.command || inp.modifiers.ctrl,
                                    inp.modifiers.shift,
                                )
                            });
                            clicked = Some((i, ctrl, shift));
                        }
                        if row_resp.double_clicked() {
                            activated = Some(i);
                        }
                        // Clic derecho: enfocar esta fila (para que las acciones del
                        // menú operen sobre ella) y abrir el menú contextual de ops.
                        // Las acciones se difieren a `NaygoApp` (patrón de la fila:
                        // acumular y procesar tras pintar) vía `ops_actions`.
                        if row_resp.secondary_clicked() {
                            context_focus = Some(i);
                        }
                        row_resp.context_menu(|ui| {
                            context_focus = Some(i);
                            // Encabezado deshabilitado con el conteo de selección múltiple.
                            let n = selected_set.len();
                            if n >= 2 {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(format!(
                                        "{n} {}",
                                        i18n.t("menu.selected_count")
                                    )),
                                );
                                ui.separator();
                            }
                            if ui.button(i18n.t("op.open")).clicked() {
                                ops_actions.push(Action::Open);
                                ui.close();
                            }
                            if ui.button(i18n.t("op.open_with")).clicked() {
                                ops_actions.push(Action::OpenWith);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(i18n.t("op.copy")).clicked() {
                                ops_actions.push(Action::Copy);
                                ui.close();
                            }
                            if ui.button(i18n.t("op.cut")).clicked() {
                                ops_actions.push(Action::Cut);
                                ui.close();
                            }
                            if ui.button(i18n.t("op.paste")).clicked() {
                                ops_actions.push(Action::Paste);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(i18n.t("op.rename")).clicked() {
                                ops_actions.push(Action::Rename);
                                ui.close();
                            }
                            if ui.button(i18n.t("op.delete")).clicked() {
                                ops_actions.push(Action::Delete);
                                ui.close();
                            }
                            ui.separator();
                            // Menú contextual NATIVO de Windows para los ítems
                            // seleccionados (shell-B). Se difiere a NaygoApp con las
                            // coords de PANTALLA del clic (TrackPopupMenuEx usa pantalla,
                            // no coords de ventana).
                            if ui
                                .button(i18n.t("op.more_windows"))
                                .on_hover_text(i18n.t("op.more_windows_soon"))
                                .clicked()
                            {
                                context_focus = Some(i);
                                let screen = ui.input(|inp| {
                                    let p = inp.pointer.interact_pos().unwrap_or_default();
                                    let origin = inp
                                        .viewport()
                                        .outer_rect
                                        .map(|r| r.min)
                                        .unwrap_or_default();
                                    (p.x + origin.x, p.y + origin.y)
                                });
                                *native_menu_request = Some(screen);
                                ui.close();
                            }
                        });
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

    // Arrastre OLE hacia el SO: si una fila superó el umbral de arrastre, resolvemos
    // las rutas (multi-selección si la hay; si no, la entry arrastrada) y diferimos el
    // inicio de `DoDragDrop` a `NaygoApp`, FUERA del closure de egui. `view` es la vista
    // pintada (filtrada/ordenada); `selected_set` son posiciones de vista.
    if let Some(pos) = os_drag_start {
        let paths: Vec<std::path::PathBuf> = if !selected_set.is_empty() {
            selected_set
                .iter()
                .filter_map(|&p| view.get(p))
                .map(|e| e.path.clone())
                .collect()
        } else {
            view.get(pos)
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default()
        };
        if !paths.is_empty() {
            pending.push(PaneRequest::StartOsDrag { paths });
        }
    }

    // Rubber-band (selección por rectángulo) SIN widget propio: se lee el puntero crudo.
    // LECCIÓN (3 bugs seguidos): un `ui.interact` de fondo apilado sobre las filas pelea
    // con el hit-test de egui (roba clics, "arrastra" desde el press sin umbral, rompe el
    // hover: al arrastrar egui deja hovered SOLO el widget arrastrado). En cambio: un
    // press que EMPIEZA dentro del cuerpo de la tabla, FUERA de toda fila visible (el
    // espacio vacío) y sin que otro widget capture el arrastre (scrollbar, resize de
    // columna, dnd de encabezados) arma el rectángulo cuando el gesto supera el umbral
    // clic-vs-arrastre. Las filas no participan: su arrastre es el drag OLE de archivos.
    let band_area = ui.min_rect();
    let start_key = egui::Id::new(("naygo_rubberband_start", id.0));
    let active_key = egui::Id::new(("naygo_rubberband_active", id.0));

    if ui.input(|i| i.pointer.primary_pressed()) {
        if let Some(origin) = ui.input(|i| i.pointer.interact_pos()) {
            // Excluir el encabezado (sus gestos son orden/filtro/dnd de columnas).
            let below_header = origin.y > band_area.top() + HEADER_HEIGHT;
            let on_row = row_rects.iter().any(|(_, r)| r.contains(origin));
            if band_area.contains(origin) && below_header && !on_row {
                ui.memory_mut(|m| m.data.insert_temp(start_key, origin));
            }
        }
    }

    // Resultado del rubber-band al soltar: (posiciones tocadas, aditivo con Ctrl).
    let mut rubber_select: Option<(Vec<usize>, bool)> = None;
    if let Some(start) = ui.memory(|m| m.data.get_temp::<egui::Pos2>(start_key)) {
        if ctx.dragged_id().is_some() {
            // Otro widget capturó el arrastre (scrollbar, resize, dnd de columnas):
            // cancelar el band.
            ui.memory_mut(|m| {
                m.data.remove::<egui::Pos2>(start_key);
                m.data.remove::<bool>(active_key);
            });
        } else {
            // El band se ACTIVA recién cuando el gesto supera el umbral; el flag se
            // necesita porque en el frame del soltar `is_decidedly_dragging` puede ya
            // haber vuelto a false.
            if ui.input(|i| i.pointer.primary_down() && i.pointer.is_decidedly_dragging()) {
                ui.memory_mut(|m| m.data.insert_temp(active_key, true));
            }
            let active = ui
                .memory(|m| m.data.get_temp::<bool>(active_key))
                .unwrap_or(false);
            let released = ui.input(|i| i.pointer.primary_released());
            // `interact_pos()` puede ser `None` si el puntero salió de la ventana; en
            // ese caso no se pinta ni selecciona, pero igual se limpia al soltar.
            if let Some(cur) = ui.input(|i| i.pointer.interact_pos()) {
                let band = egui::Rect::from_two_pos(start, cur);
                if active && !released {
                    paint_rubber_band(ui, band, theme.accent());
                }
                if active && released {
                    let hit: Vec<usize> = row_rects
                        .iter()
                        .filter(|(_, r)| r.intersects(band))
                        .map(|(pos, _)| *pos)
                        .collect();
                    let additive = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
                    rubber_select = Some((hit, additive));
                }
            }
            if released {
                ui.memory_mut(|m| {
                    m.data.remove::<egui::Pos2>(start_key);
                    m.data.remove::<bool>(active_key);
                });
            }
        }
    }

    // NOTA: el panel NO es un destino de drop egui-interno. Todo el drag&drop de
    // archivos (intra-app pane→pane y externo) pasa por OLE: `DoDragDrop` corre un
    // bucle modal que captura el mouse, y los drops vuelven por el `IDropTarget` que
    // registra winit → egui `dropped_files` → `NaygoApp::pump_dropped_files`. Un
    // destino egui aquí sería código muerto (OLE siempre gana el arrastre).

    // Rename inline (F2): procesar las señales del frame con la `view` a mano. El
    // estado se ancla por PATH: tras un rename el listado se refresca y la vista
    // puede estar vacía/reordenada unos frames — se tolera con `missing_frames`.
    let rename_mine = inline_rename.as_ref().is_some_and(|r| r.pane == id);
    if rename_mine {
        let (anchor, text) = {
            let r = inline_rename.as_ref().expect("rename_mine implica Some");
            (r.path.clone(), r.text.clone())
        };
        match view.iter().position(|e| e.path == anchor) {
            None => {
                // El path aún no reaparece (listado en vuelo): mantener vivo el
                // estado un rato y repintar; pasado el límite, descartar limpio.
                let r = inline_rename.as_mut().expect("rename_mine implica Some");
                r.missing_frames = r.missing_frames.saturating_add(1);
                if r.missing_frames > 60 {
                    *inline_rename = None;
                } else {
                    ui.ctx().request_repaint();
                }
            }
            Some(_) if rename_cancel => {
                *inline_rename = None;
            }
            Some(pos) if rename_commit || rename_travel.is_some() => {
                let new_name = text.trim().to_string();
                let old = &view[pos];
                let valid = !new_name.is_empty()
                    && !new_name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']);
                if rename_commit && valid && new_name != old.name {
                    pending.push(PaneRequest::CommitRename {
                        source: old.path.clone(),
                        new_name,
                    });
                }
                match rename_travel {
                    Some(d) => {
                        // Viajar a la fila de arriba/abajo y seguir en serie.
                        let len = view.len() as i32;
                        let np = (pos as i32 + d).clamp(0, len - 1) as usize;
                        let next_text = view[np].name.clone();
                        *inline_rename = Some(crate::app::InlineRename {
                            pane: id,
                            path: view[np].path.clone(),
                            text: next_text,
                            stage: 0,
                            focus_pending: true,
                            select_pending: true,
                            missing_frames: 0,
                        });
                        // El foco/selección del panel acompaña el viaje.
                        if let Some(fp) = workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                            fp.select_single(np);
                        }
                    }
                    None => *inline_rename = None,
                }
            }
            Some(r) => {
                let _ = r;
                if let Some(st) = inline_rename.as_mut() {
                    st.missing_frames = 0;
                }
            }
        }
    }

    // Borde punteado del foco/ancla: distingue la fila con foco de teclado del resto
    // de la multi-selección (que ya va resaltada). Se pinta tras la tabla, con el `ui`
    // padre, para que quede por encima del fondo de la fila. Mismo enfoque de
    // `dashed_line` que el rubber-band.
    if let Some(r) = focus_rect {
        dashed_rect_border(ui.painter(), r, theme.accent());
    }

    // Detectar resize: si el ancho medido de una columna difiere del guardado en el
    // modelo, el usuario arrastró el borde → persistir el nuevo ancho. El clamp lo
    // hace `set_width` en core. El umbral evita re-emitir por jitter de float. En modo
    // Auto NO se emite: el ancho lo decide la tabla por contenido, no el modelo, y
    // persistirlo crearía un bucle de realimentación con el reparto automático.
    if !auto_width {
        for (ci, col) in visible_cols.iter().enumerate() {
            if let Some(w) = measured_widths[ci] {
                if (w - col.width).abs() > WIDTH_CHANGE_EPS {
                    table_actions.push(TableAction::SetColumnWidth(col.kind, w));
                }
            }
        }
    }

    if parent_activated {
        if let Some(dir) = parent {
            pending.push(PaneRequest::Activate { id });
            pending.push(PaneRequest::NavigateTo { id, dir });
        }
    }
    // El clic derecho enfoca la fila y activa el panel, para que las acciones del
    // menú contextual operen sobre la entry correcta (apply_action usa el foco si no
    // hay multi-selección). Se aplica antes que `clicked` (clic izquierdo) que es
    // mutuamente excluyente en la práctica.
    if let Some(i) = context_focus {
        if let Some(f) = workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            if f.is_selected(i) {
                // La fila del clic derecho ya está en la multi-selección: se mantiene
                // (el menú opera sobre todos los seleccionados, como Windows). Solo se
                // asegura el foco en la fila clicada.
                f.focused = Some(i);
            } else {
                // Clic derecho sobre una fila FUERA de la selección → se reduce a ese
                // ítem antes de abrir el menú (comportamiento de Windows). `select_single`
                // ya fija foco y ancla.
                f.select_single(i);
            }
        }
        pending.push(PaneRequest::Activate { id });
    }
    // Clic izquierdo: puebla la multi-selección según modificadores. Los métodos
    // puros (`select_*`) fijan el foco ellos mismos, así que NO se setea `focused`
    // a mano aquí; se conserva la activación del panel (el clic da foco al panel).
    if let Some((pos, ctrl, shift)) = clicked {
        if let Some(f) = workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            if shift {
                f.select_range_to(pos);
            } else if ctrl {
                f.select_toggle(pos);
            } else {
                f.select_single(pos);
            }
        }
        pending.push(PaneRequest::Activate { id });
    }
    // Rubber-band (Step 4): aplicar la selección por rectángulo al soltar. Un arrastre
    // no dispara `clicked` (egui distingue clic de arrastre), así que esto no choca con
    // el manejador de `clicked` de arriba.
    if let Some((positions, additive)) = rubber_select {
        if let Some(f) = workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.select_rect(&positions, additive);
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
            } else {
                // Archivo: doble clic = abrir con el programa por defecto del SO
                // (pedido de Nicolás). Mismo camino que Enter: el doble clic ya fijó
                // el foco en esta fila (su `clicked` corrió select_single), así que
                // Action::Open sobre el panel activo abre exactamente este archivo.
                pending.push(PaneRequest::Activate { id });
                ops_actions.push(Action::Open);
            }
        }
    }

    // Activar este panel con CUALQUIER press dentro de su área (no solo el clic en una
    // fila): el panel activo es el destino del árbol, del teclado y de las operaciones,
    // así que debe seguir al mouse como en cualquier explorador.
    // OJO: tiene que ser `max_rect()` (el área ASIGNADA al panel), no `min_rect()` (lo
    // PINTADO): con pocas filas el min_rect termina en la última fila y un clic en el
    // espacio vacío de abajo quedaba fuera → no activaba (bug reportado por Nicolás).
    // El intersect con el clip protege contra rects extendidos fuera de lo visible.
    let pane_rect = ui.max_rect().intersect(ui.clip_rect());
    let pressed_inside = ui.input(|i| {
        (i.pointer.primary_pressed() || i.pointer.secondary_pressed())
            && i.pointer
                .interact_pos()
                .is_some_and(|p| pane_rect.contains(p))
    });
    if pressed_inside {
        pending.push(PaneRequest::Activate { id });
    }

    // Resaltar el panel ACTIVO con un borde del color de acento (pedido de Nicolás:
    // distinguir de un vistazo qué panel recibe teclado/árbol/operaciones).
    if workspace.active_id() == Some(id) {
        ui.painter().rect_stroke(
            pane_rect,
            2.0,
            egui::Stroke::new(1.5, theme.accent()),
            egui::StrokeKind::Inside,
        );
    }
}

/// Pinta el rectángulo de selección (rubber-band): relleno tenue del color de acento
/// y borde punteado del acento (estilo clásico de Windows). `dashed_line` devuelve un
/// `Vec<Shape>` en egui 0.34, así que se vuelca con `painter.extend`.
fn paint_rubber_band(ui: &egui::Ui, rect: egui::Rect, accent: egui::Color32) {
    if !is_paintable_rect(rect) {
        return;
    }
    let fill = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 16);
    ui.painter().rect_filled(rect, 0.0, fill);
    dashed_rect_border(ui.painter(), rect, accent);
}

/// ¿El rect es seguro para pintar? Rechaza rects NO FINITOS (egui devuelve
/// `Rect::NOTHING` = `[+∞..-∞]` para widgets aún sin layout, p. ej. en el primer frame)
/// y rects absurdamente grandes. CRÍTICO: `Shape::dashed_line` asigna ~`largo/(dash+gap)`
/// segmentos; un largo infinito/enorme intenta asignar decenas de GB y ABORTA el proceso
/// (causa del cierre tras el splash). Con esta guarda, simplemente no se pinta.
fn is_paintable_rect(r: egui::Rect) -> bool {
    r.is_finite()
        && r.width() >= 0.0
        && r.height() >= 0.0
        && r.width() < 100_000.0
        && r.height() < 100_000.0
}

/// Pinta un borde punteado alrededor de `rect` con el color dado. Tolerante: no hace
/// nada si el rect no es pintable (ver `is_paintable_rect`), evitando que `dashed_line`
/// asigne memoria desproporcionada con coordenadas no finitas.
fn dashed_rect_border(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    if !is_paintable_rect(rect) {
        return;
    }
    let stroke = egui::Stroke::new(1.0, color);
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    for w in corners.windows(2) {
        painter.extend(egui::Shape::dashed_line(&[w[0], w[1]], stroke, 4.0, 3.0));
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

/// Pinta una celda "[ícono] nombre" puramente visual, SIN sense propio. CAUSA RAÍZ
/// de los clics muertos de la tabla (bug reportado por Nicolás): cualquier widget
/// clicable dentro de la celda queda ENCIMA de la fila en el hit-test de egui,
/// recibe el clic (verificado con `InteractionSnapshot`) y su `Response` se descarta
/// — la fila nunca ve `clicked()`. La FILA completa (`row.response()`) es la única
/// superficie de clic de la tabla. Devuelve el `Response` inerte (hover) por si un
/// caller quiere tooltips.
fn icon_row(
    ui: &mut egui::Ui,
    icons: &IconProvider,
    key: IconKey,
    name: &str,
    name_color: Option<egui::Color32>,
) -> egui::Response {
    ui.horizontal(|ui| {
        let tex = icons.texture(key);
        let img = ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(ICON_SIZE, ICON_SIZE)));
        let text = match name_color {
            Some(c) => egui::RichText::new(name).color(c),
            None => egui::RichText::new(name),
        };
        // `Label` plano y NO seleccionable: `selectable_label`/labels seleccionables
        // sensan click(+drag) para el cursor de texto y roban el clic de la fila.
        let label = ui.add(egui::Label::new(text).selectable(false));
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
        Some(bytes) => naygo_core::format::human_size(bytes),
        None => String::new(),
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

/// Aplica al TextEdit del rename inline el rango de selección de la etapa del ciclo
/// F2: 0 = nombre sin extensión, 1 = solo extensión, 2 = todo. Sin extensión (carpetas,
/// dotfiles tipo ".gitignore") todas las etapas seleccionan todo. Rangos en CHARS.
fn apply_rename_selection(ctx: &egui::Context, te_id: egui::Id, text: &str, stage: u8) {
    let total = text.chars().count();
    let split = text
        .rsplit_once('.')
        .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty());
    let (start, end) = match (stage, split) {
        (0, Some((stem, _))) => (0, stem.chars().count()),
        (1, Some((stem, _))) => (stem.chars().count() + 1, total),
        _ => (0, total),
    };
    let mut state = egui::text_edit::TextEditState::load(ctx, te_id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    state.store(ctx, te_id);
}
