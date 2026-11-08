// Naygo — sincronización core→UI: `sync_rows` (contenido) y `sync_layout` (estructura).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// `sync_rows` (barato, en cada tick) actualiza el contenido de los modelos ESTABLES in situ
// (los ListView conservan su scroll). `sync_layout` (estructural) reconcilia la lista de
// paneles y splitters cuando cambia la LISTA de paneles o el ÁREA, y luego llama `sync_rows`.
// `apply_device_change` reacciona a unidades USB enchufadas/quitadas. Extraídos de `main.rs`
// sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::models::{purpose_to_int, rects_equal, Models};
use crate::vm_builders::*;
use crate::win_helpers::*;
use crate::workspace_ctrl::WorkspaceCtrl;
use crate::*;
use naygo_core::workspace::layout::{Rect, SplitDir};
use naygo_core::workspace::{PaneId, PanePurpose};
use slint::{Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Los dos closures de sincronización que consume el resto del cableado de la UI.
pub(crate) struct SyncHandles {
    pub sync_rows: Rc<dyn Fn()>,
    pub sync_layout: Rc<dyn Fn()>,
}

#[derive(Clone)]
struct AppliedRowsSource {
    context: u64,
    revision: u64,
    entry_count: usize,
    view_indices: std::sync::Arc<[usize]>,
}

fn applied_rows_source(
    ctrl: &WorkspaceCtrl,
    id: PaneId,
    context: u64,
) -> Option<AppliedRowsSource> {
    Some(AppliedRowsSource {
        context,
        revision: ctrl.entries_revision_of(id)?,
        entry_count: ctrl.row_entry_count(id)?,
        view_indices: ctrl.row_view_indices_shared(id)?,
    })
}

/// `set_vec` SOLO si el contenido cambió. Un `set_vec` incondicional resetea el modelo
/// entero y Slint RE-CREA todos los delegates que lo consumen; el modelo de columnas se
/// usa en el `for col[ci] in root.columns` de CADA fila, así que resetearlo en cada sync
/// re-montaba todas las celdas del panel — incluida la que aloja el editor de rename
/// (blink visible del panel al presionar Shift/teclas que disparan sync, y el caret del
/// editor saltaba al final: el "_" "después de la extensión"). Columnas y menú de
/// columnas rara vez cambian; comparar es barato (≤5 entradas).
fn set_vec_if_changed<T: PartialEq + Clone + 'static>(model: &VecModel<T>, rows: Vec<T>) {
    let dirty =
        model.row_count() != rows.len() || model.iter().zip(rows.iter()).any(|(a, b)| a != *b);
    if dirty {
        model.set_vec(rows);
    }
}

/// Aplica `rows` al modelo de forma INCREMENTAL cuando el largo cambió (alta/baja de
/// archivos): en vez de `set_vec` (que re-crea TODOS los delegates del ListView), calcula
/// el prefijo y el sufijo que ya coinciden y solo quita/inserta el tramo del medio que
/// cambió. Los delegates fuera del tramo sobreviven (hover, scroll y el editor de rename
/// intactos); el trabajo es O(cambio) en vez de O(todo el modelo). Caso típico: un archivo
/// nuevo en una carpeta "viva" (Dropbox) → un insert, no un remontaje completo.
#[cfg(test)]
fn apply_rows_incremental<T: PartialEq + Clone + 'static>(model: &VecModel<T>, rows: Vec<T>) {
    apply_rows_incremental_by(model, rows, |left, right| left == right);
}

fn apply_rows_incremental_by<T: Clone + 'static>(
    model: &VecModel<T>,
    rows: Vec<T>,
    same: impl Fn(&T, &T) -> bool,
) {
    let old_n = model.row_count();
    let new_n = rows.len();
    // Prefijo común (filas idénticas al inicio).
    let mut prefix = 0;
    while prefix < old_n.min(new_n)
        && model
            .row_data(prefix)
            .as_ref()
            .zip(rows.get(prefix))
            .is_some_and(|(old, new)| same(old, new))
    {
        prefix += 1;
    }
    // Sufijo común (filas idénticas al final, sin solapar el prefijo).
    let mut suffix = 0;
    while suffix < (old_n - prefix).min(new_n - prefix)
        && model
            .row_data(old_n - 1 - suffix)
            .as_ref()
            .zip(rows.get(new_n - 1 - suffix))
            .is_some_and(|(old, new)| same(old, new))
    {
        suffix += 1;
    }
    // El tramo del medio se reemplaza: quita lo viejo e inserta lo nuevo en `prefix`.
    for _ in 0..(old_n - prefix - suffix) {
        model.remove(prefix);
    }
    for (k, row) in rows
        .into_iter()
        .skip(prefix)
        .take(new_n - prefix - suffix)
        .enumerate()
    {
        model.insert(prefix + k, row);
    }
}

/// Compara el contenido visible de una fila. Slint representa `cells` como un modelo anidado;
/// cada conversión crea un `VecModel` nuevo y comparar su `ModelRc` por identidad marcaba todas
/// las filas como distintas aunque las celdas no hubieran cambiado. Eso disparaba 100k
/// notificaciones al aplicar una sola alta/modificación en carpetas grandes.
fn row_data_eq(left: &RowData, right: &RowData) -> bool {
    left.name == right.name
        && left.is_dir == right.is_dir
        && left.selected == right.selected
        && left.focused == right.focused
        && left.cut == right.cut
        && left.highlight == right.highlight
        && left.compare_state == right.compare_state
        && left.filter_match == right.filter_match
        && left.match_pre == right.match_pre
        && left.match_mid == right.match_mid
        && left.match_post == right.match_post
        && left.icon == right.icon
        && left.depth == right.depth
        && left.cells.row_count() == right.cells.row_count()
        && (0..left.cells.row_count())
            .all(|index| left.cells.row_data(index) == right.cells.row_data(index))
}

/// Construye `sync_rows` y `sync_layout` sobre los modelos estables compartidos.
pub(crate) fn build_sync(
    ui_weak: slint::Weak<AppWindow>,
    ctrl: Rc<RefCell<WorkspaceCtrl>>,
    models: Rc<RefCell<Models>>,
    last_row_sig: Rc<RefCell<HashMap<u64, u64>>>,
    area_of: Rc<dyn Fn() -> Rect>,
) -> SyncHandles {
    let sync_rows: Rc<dyn Fn()> = Rc::new({
        let ui_weak = ui_weak.clone();
        let ctrl = ctrl.clone();
        let models = models.clone();
        let last_row_sig = last_row_sig.clone();
        let row_sources = Rc::new(RefCell::new(HashMap::<u64, AppliedRowsSource>::new()));
        let row_sources = row_sources.clone();
        let search_model = Rc::new(VecModel::<SearchHitVm>::default());
        let points_model = Rc::new(VecModel::<PointRowVm>::default());
        move || {
            let _sync_metric = crate::metrics::span("ui.sync_rows");
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            {
                let c = ctrl.borrow();
                ui.set_has_favorites_pane(
                    c.has_purpose(naygo_core::workspace::PanePurpose::Favorites),
                );
                ui.set_has_preview_pane(c.has_purpose(naygo_core::workspace::PanePurpose::Preview));
                ui.set_has_operations_pane(
                    c.has_purpose(naygo_core::workspace::PanePurpose::Operations),
                );
                ui.set_has_basket_pane(c.has_purpose(naygo_core::workspace::PanePurpose::Basket));
            }
            // El borrador pertenece a la carpeta/panel capturados al abrirlo. Si se cierra
            // ese panel o navega por otra vía, cancelar ANTES de tomar el préstamo mutable:
            // el callback cancela también validaciones/sugerencias y vuelve a prestar ctrl.
            let edit_id = ui.get_edit_pane();
            let stale_edit = edit_id >= 0
                && ctrl.borrow().path_of(PaneId(edit_id as u64)) != ui.get_edit_base().as_str();
            if stale_edit {
                ui.invoke_path_edit_cancel(edit_id);
            }
            // `borrow_mut` porque `rows_of` necesita mutar el IconCache (decodifica on-demand).
            let mut c = ctrl.borrow_mut();
            ui.set_maximized_pane(c.maximized_pane.map_or(-1, |id| id.0 as i32));
            ui.set_basket_selected_count(c.basket_selection.count() as i32);
            use naygo_core::icon_kind::{ActionIcon, FileCategory, IconKey};
            ui.set_basket_copy_icon(c.icons.get_control(IconKey::Action(ActionIcon::Copy)));
            ui.set_basket_move_icon(c.icons.get_control(IconKey::Action(ActionIcon::Forward)));
            ui.set_basket_delete_icon(c.icons.get_control(IconKey::Action(ActionIcon::Delete)));
            ui.set_basket_open_icon(c.icons.get_control(IconKey::Folder));
            ui.set_basket_delivery_icon(c.icons.get_control(IconKey::File(FileCategory::Archive)));
            ui.set_basket_focused_row(
                c.basket_selection
                    .focused
                    .as_ref()
                    .and_then(|path| c.basket.items().iter().position(|p| p == path))
                    .map_or(-1, |i| i as i32),
            );
            // Título de la ventana según el modo configurado (solo app / app+ruta / solo ruta).
            // Se recalcula en cada sync pero solo se escribe si cambió (navegación, cambio de
            // panel activo o de la opción en Configuración).
            let title = c.window_title();
            if ui.get_window_title() != title {
                ui.set_window_title(title.into());
            }
            let active = c.active_id();
            ui.set_destination_radar_open(c.destination_radar.is_some());
            ui.set_destination_radar_explicit(c.destination_radar.as_ref().is_some_and(|radar| {
                radar.origin == crate::workspace_ctrl::DestinationRadarOrigin::ExternalDrop
            }));
            ui.set_destination_radar_move(
                c.destination_radar
                    .as_ref()
                    .is_some_and(|radar| radar.move_files),
            );
            // Panel resaltado por arrastre (hover de drop): se refleja en `PaneVm.drag-over`.
            let drag_over = c.drag_over_pane();
            let hl_secs = c.highlight_secs();
            let hl_now = std::time::Instant::now();
            // Datos compartidos (no dependen del panel concreto): favoritos, recientes,
            // historial, inspector, preview se derivan del estado global / panel activo.
            let favs: Vec<NavRow> = c.favorite_rows().into_iter().map(to_nav_row).collect();
            let fav_tree: Vec<FavTreeRow> =
                c.fav_tree_rows().into_iter().map(to_fav_tree_row).collect();
            // Grupos para el submenú "Mover a…" del panel y para que el menú ▾ del toolbar exista.
            let fav_group_options: Vec<PathSeg> = c
                .fav_group_options()
                .into_iter()
                .map(|(label, gid)| PathSeg {
                    label: SharedString::from(label),
                    path: SharedString::from(gid),
                })
                .collect();
            let recents: Vec<NavRow> = c.recent_rows().into_iter().map(to_nav_row).collect();
            let frequent_dirs: Vec<NavRow> =
                c.frequent_dir_rows().into_iter().map(to_nav_row).collect();
            let hist: Vec<HistRow> = c.history_rows().into_iter().map(to_hist_row).collect();
            let info = c.inspector_info();
            let mut inspector = to_inspector_vm(info.clone());
            // Carpeta: el tamaño no viene del listado, se pide con el botón "Calcular" (F3 hace lo
            // mismo). Solo se refleja el resultado si el cálculo corresponde a ESTA carpeta (la
            // enfocada, `info.path`): si el usuario cambió de foco a otra carpeta, el resultado viejo
            // NO se pega y el Inspector vuelve a mostrar el botón «Calcular» (size_status_for → None).
            if inspector.is_dir {
                if let Some(txt) = c.size_status_for(std::path::Path::new(info.path.as_str())) {
                    inspector.size_calc = SharedString::from(txt);
                }
            }
            // Metadata por tipo del archivo a mostrar. Se sigue el MISMO archivo que la Vista
            // previa (último Files activo → ítem enfocado, solo archivos), no el panel activo a
            // secas: así coincide con el preview aun con varios paneles Files, y clicar en el
            // Preview/Inspector no la vacía. Si es carpeta o no hay nada, se limpia el job. El
            // worker no relanza si ya es la del mismo archivo. Las etiquetas se traducen con
            // `config.t` en `meta_fields_model`.
            match c.metadata_target() {
                Some(path) => c.request_metadata(path),
                None => c.clear_metadata(),
            }
            inspector.meta = meta_fields_model(&c);
            inspector.meta_loading = c.meta_loading();
            inspector.has_provenance = c.has_provenance();

            // Props a nivel de ventana para el menú ▾ de favoritos del toolbar (el árbol jerárquico)
            // y para el submenú "Mover a…" del panel (lista de grupos destino).
            ui.set_fav_tree(ModelRc::from(Rc::new(VecModel::from(fav_tree.clone()))));
            ui.set_fav_group_options(ModelRc::from(Rc::new(VecModel::from(
                fav_group_options.clone(),
            ))));

            let mut m = models.borrow_mut();
            for (i, &id) in m.pane_ids.clone().iter().enumerate() {
                let purpose = c.purpose_of(id);
                // Actualiza los modelos de lista que apliquen al tipo, in situ.
                match purpose {
                    Some(PanePurpose::Files) => {
                        // O-1: firma barata del estado que determina las filas. Si no cambió desde
                        // el tick anterior, `rows_of` daría las MISMAS filas → saltamos su
                        // construcción (O(n) con allocs de String) y el `set_vec`. `None` = no
                        // cachear (resaltado fresco vigente: el fundido cambia cada tick).
                        let sig = {
                            let _metric = crate::metrics::span("ui.rows_signature");
                            c.rows_signature(id, hl_secs, hl_now)
                        };
                        let mut sigs = last_row_sig.borrow_mut();
                        let rows_unchanged = match sig {
                            Some(s) => {
                                if sigs.get(&id.0) == Some(&s) {
                                    true // misma firma: filas idénticas, no reconstruir
                                } else {
                                    sigs.insert(id.0, s);
                                    false
                                }
                            }
                            None => {
                                // Sin cache este tick: limpiar la firma guardada para que el
                                // PRÓXIMO tick (ya sin fresh) reconstruya y vuelva a sembrarla.
                                sigs.remove(&id.0);
                                row_sources.borrow_mut().remove(&id.0);
                                false
                            }
                        };
                        drop(sigs);
                        // Columnas y menú de columnas son baratos (≤5 entradas): se refrescan
                        // siempre, no entran al cache de filas.
                        let cols: Vec<ColumnVm> =
                            c.columns_of(id).into_iter().map(to_column_vm).collect();
                        let col_menu: Vec<ColumnToggleVm> = c
                            .column_toggles_of(id)
                            .into_iter()
                            .map(to_column_toggle_vm)
                            .collect();
                        if !rows_unchanged {
                            let pm = m.models_for(id);
                            // Los eventos Modified estables pueden recalcular solo las filas
                            // afectadas. Altas, bajas, reordenamientos y demás cambios mantienen
                            // la reconciliación completa/incremental de abajo.
                            let row_context = if sig.is_some()
                                && c.rename_active
                                    .as_ref()
                                    .is_none_or(|rename| rename.pane != id)
                            {
                                c.rows_context_signature(id, hl_secs, hl_now)
                            } else {
                                row_sources.borrow_mut().remove(&id.0);
                                None
                            };
                            let row_count = c.row_source_count(id);
                            let current_revision = c.entries_revision_of(id);
                            let previous_source = match (row_context, current_revision) {
                                (Some(context), Some(_)) => row_sources
                                    .borrow()
                                    .get(&id.0)
                                    .filter(|source| {
                                        source.context == context
                                            && pm.rows.row_count() == source.view_indices.len()
                                    })
                                    .cloned(),
                                _ => None,
                            };
                            let previous_revision = previous_source.as_ref().and_then(|source| {
                                (row_count == Some(pm.rows.row_count())).then_some(source.revision)
                            });
                            // El core solo marca los lotes que fueron agregados físicamente al
                            // final. Además exigimos que su revisión/rango coincidan y que los
                            // índices visibles ya aplicados sigan siendo un prefijo exacto. Así
                            // un sort que intercale archivos, un refresh o un filtro/contexto
                            // distinto cae en la reconciliación normal.
                            let appended_rows = previous_source.as_ref().and_then(|source| {
                                let end =
                                    c.rows_appended_since(id, source.revision, source.entry_count)?;
                                if c.row_entry_count(id)? != end {
                                    return None;
                                }
                                let view = c.row_view_indices_shared(id)?;
                                if view.len() < source.view_indices.len()
                                    || view[..source.view_indices.len()]
                                        != *source.view_indices.as_ref()
                                {
                                    return None;
                                }
                                let positions: Vec<usize> =
                                    (source.view_indices.len()..view.len()).collect();
                                let plain_rows = {
                                    let _metric = crate::metrics::span("ui.rows_build");
                                    c.rows_at(id, &positions, hl_secs, hl_now)?
                                };
                                Some((view, plain_rows))
                            });
                            let changed_positions = previous_revision.and_then(|revision| {
                                c.dirty_row_positions(id, revision, hl_secs, hl_now)
                                    .filter(|positions| positions.len() <= pm.rows.row_count() / 4)
                            });

                            if let Some((view, plain_rows)) = appended_rows {
                                let rows: Vec<RowData> = {
                                    let _metric = crate::metrics::span("ui.rows_convert");
                                    plain_rows.into_iter().map(to_row_data).collect()
                                };
                                {
                                    let _metric = crate::metrics::span("ui.rows_model_apply");
                                    for row in rows {
                                        pm.rows.push(row);
                                    }
                                }
                                if let (Some(context), Some(entry_count)) =
                                    (row_context, c.row_entry_count(id))
                                {
                                    if let Some(revision) = current_revision {
                                        row_sources.borrow_mut().insert(
                                            id.0,
                                            AppliedRowsSource {
                                                context,
                                                revision,
                                                entry_count,
                                                view_indices: view,
                                            },
                                        );
                                    }
                                }
                            } else if let Some(positions) = changed_positions.as_ref() {
                                let mut rows_built = positions.is_empty();
                                if !positions.is_empty() {
                                    let plain_rows = {
                                        let _metric = crate::metrics::span("ui.rows_build");
                                        c.rows_at(id, positions, hl_secs, hl_now)
                                    };
                                    if let Some(plain_rows) = plain_rows {
                                        rows_built = true;
                                        let rows: Vec<RowData> = {
                                            let _metric = crate::metrics::span("ui.rows_convert");
                                            plain_rows.into_iter().map(to_row_data).collect()
                                        };
                                        let _metric = crate::metrics::span("ui.rows_model_apply");
                                        for (position, row) in positions.iter().copied().zip(rows) {
                                            if c.rename_active.as_ref().is_some_and(|rename| {
                                                rename.pane == id && rename.pos == position
                                            }) {
                                                continue;
                                            }
                                            if pm
                                                .rows
                                                .row_data(position)
                                                .as_ref()
                                                .is_none_or(|old| !row_data_eq(old, &row))
                                            {
                                                pm.rows.set_row_data(position, row);
                                            }
                                        }
                                    }
                                }
                                if rows_built {
                                    // El estado se mantiene bajo el préstamo de `ctrl` durante la
                                    // traducción y la conversión; se puede avanzar la revisión.
                                    if current_revision.is_some() {
                                        if let Some(context) = row_context {
                                            if let Some(source) =
                                                applied_rows_source(&c, id, context)
                                            {
                                                row_sources.borrow_mut().insert(id.0, source);
                                            } else {
                                                row_sources.borrow_mut().remove(&id.0);
                                            }
                                        }
                                    }
                                } else {
                                    // Si el delta dejó de ser aplicable, el siguiente tick debe
                                    // reconciliar el modelo completo.
                                    row_sources.borrow_mut().remove(&id.0);
                                }
                            } else {
                                let plain_rows = {
                                    let _metric = crate::metrics::span("ui.rows_build");
                                    c.rows_of(id, hl_secs, hl_now)
                                };
                                let rows: Vec<RowData> = {
                                    let _metric = crate::metrics::span("ui.rows_convert");
                                    plain_rows.into_iter().map(to_row_data).collect()
                                };
                                // Actualización POR FILA cuando el largo no cambió. `set_vec`
                                // resetea el modelo ENTERO y el ListView re-crea todas sus filas;
                                // en una carpeta "en movimiento" (Dropbox sincronizando → eventos
                                // del watcher → resaltado fresco activo → la firma no cachea) eso
                                // ocurría en CADA tick de 30 ms: la cadena de hover del puntero se
                                // invalidaba constantemente y los eventos de RUEDA dejaban de
                                // llegar al panel (scroll "muerto" justo en esas carpetas), además
                                // de reconstruir cientos de ítems por tick. Con `set_row_data`
                                // solo se notifican las filas que de verdad cambiaron: los ítems
                                // del ListView sobreviven, el hover queda válido y la rueda sigue
                                // viva. Si el LARGO cambió (alta/baja real de archivos), sí se
                                // resetea entero — es el caso raro y ahí sí cambia la estructura.
                                let _metric = crate::metrics::span("ui.rows_model_apply");
                                if pm.rows.row_count() == rows.len() {
                                    // Fila bajo edición inline (rename): NO tocarla aunque cambien
                                    // sus datos (p. ej. expira su resaltado "fresco" o llega un
                                    // evento del watcher). Un `set_row_data` sobre ella RE-MONTA el
                                    // delegate y destruye el `LineEdit` del rename (se llevaba el
                                    // texto tipeado: bug del "_" que borraba todo). La fila queda
                                    // visualmente "congelada" mientras se edita, lo cual es invisible
                                    // porque el editor la tapa. Con el draft en `rename-draft` el
                                    // texto sobrevive incluso al `set_vec` (cambio de largo).
                                    let rename_row = c
                                        .rename_active
                                        .as_ref()
                                        .filter(|r| r.pane == id)
                                        .map(|r| r.pos);
                                    for (i, row) in rows.into_iter().enumerate() {
                                        if rename_row == Some(i) {
                                            continue;
                                        }
                                        if pm
                                            .rows
                                            .row_data(i)
                                            .as_ref()
                                            .is_none_or(|old| !row_data_eq(old, &row))
                                        {
                                            pm.rows.set_row_data(i, row);
                                        }
                                    }
                                } else {
                                    // El largo cambió (alta/baja real): aplicación INCREMENTAL
                                    // (prefijo/sufijo intactos; solo el tramo del medio se
                                    // quita/inserta) en vez de un `set_vec` que re-creaba todos
                                    // los delegates — ver `apply_rows_incremental`.
                                    apply_rows_incremental_by(&pm.rows, rows, row_data_eq);
                                }
                                if let (Some(context), Some(count)) = (row_context, row_count) {
                                    if count == pm.rows.row_count() {
                                        if let Some(source) = applied_rows_source(&c, id, context) {
                                            row_sources.borrow_mut().insert(id.0, source);
                                        } else {
                                            row_sources.borrow_mut().remove(&id.0);
                                        }
                                    } else {
                                        row_sources.borrow_mut().remove(&id.0);
                                    }
                                } else {
                                    row_sources.borrow_mut().remove(&id.0);
                                }
                            }
                            set_vec_if_changed(&pm.columns, cols);
                            set_vec_if_changed(&pm.col_menu, col_menu);
                        } else {
                            let pm = m.models_for(id);
                            set_vec_if_changed(&pm.columns, cols);
                            set_vec_if_changed(&pm.col_menu, col_menu);
                        }
                    }
                    Some(PanePurpose::Tree) => {
                        let rows: Vec<TreeRow> =
                            c.tree_rows(id).into_iter().map(to_tree_row).collect();
                        m.models_for(id).tree.set_vec(rows);
                    }
                    Some(PanePurpose::Favorites) => {
                        m.models_for(id).favs.set_vec(favs.clone());
                        m.models_for(id).fav_tree.set_vec(fav_tree.clone());
                    }
                    Some(PanePurpose::Recents) => {
                        m.models_for(id).recents.set_vec(recents.clone());
                        m.models_for(id)
                            .frequent_dirs
                            .set_vec(frequent_dirs.clone());
                    }
                    Some(PanePurpose::History) => {
                        m.models_for(id).hist.set_vec(hist.clone());
                    }
                    Some(PanePurpose::Basket) => {
                        let rows = c.basket_rows();
                        let model = &m.models_for(id).basket;
                        // set_vec recreaba los TouchArea incluso al activar el panel:
                        // el mouse-up ya no encontraba el mouse-down de esa misma fila.
                        if model.row_count() != rows.len() {
                            model.set_vec(rows);
                        } else {
                            for (index, row) in rows.into_iter().enumerate() {
                                if model.row_data(index).as_ref() != Some(&row) {
                                    model.set_row_data(index, row);
                                }
                            }
                        }
                    }
                    _ => {}
                }
                // Actualiza los campos del PaneVm sin recrear el elemento.
                if let Some(mut pv) = m.panes.row_data(i) {
                    let is_active = Some(id) == active;
                    let path = SharedString::from(c.path_of(id).as_str());
                    let mut changed = false;
                    if pv.active != is_active {
                        pv.active = is_active;
                        changed = true;
                    }
                    // Resaltado de arrastre: true solo para el panel bajo el cursor durante el drag.
                    let is_drag_over = drag_over == Some(id);
                    if pv.drag_over != is_drag_over {
                        pv.drag_over = is_drag_over;
                        changed = true;
                    }
                    let drag_client_y = c.drag_over_client_y_for(id);
                    if pv.drag_client_y != drag_client_y {
                        pv.drag_client_y = drag_client_y;
                        changed = true;
                    }
                    let comparison_active = !c.comparison.is_empty();
                    if pv.comparison_active != comparison_active {
                        pv.comparison_active = comparison_active;
                        changed = true;
                    }
                    let comparison_link_available = c
                        .comparison_pair
                        .is_some_and(|(left, right)| id == left || id == right);
                    if pv.comparison_link_available != comparison_link_available {
                        pv.comparison_link_available = comparison_link_available;
                        changed = true;
                    }
                    if pv.comparison_link_active != c.comparison_link_enabled {
                        pv.comparison_link_active = c.comparison_link_enabled;
                        changed = true;
                    }
                    if pv.path != path {
                        pv.path = path;
                        // Los breadcrumbs y el título dependen de la carpeta: hay que
                        // reconstruirlos al navegar (antes solo se armaban en sync_layout, así
                        // que el contenido cambiaba pero los segmentos quedaban viejos).
                        let segs: Vec<PathSeg> = c
                            .path_segments_of(id)
                            .into_iter()
                            .map(|(label, path)| PathSeg {
                                label: SharedString::from(label.as_str()),
                                path: SharedString::from(path.as_str()),
                            })
                            .collect();
                        pv.segments = ModelRc::from(Rc::new(VecModel::from(segs)));
                        pv.title = SharedString::from(c.pane_label(id).as_str());
                        changed = true;
                    }
                    if purpose == Some(PanePurpose::Tree) {
                        let reveal_row = c.tree_reveal_row(id);
                        let reveal_key = SharedString::from(c.tree_reveal_key(id));
                        if pv.tree_reveal_row != reveal_row {
                            pv.tree_reveal_row = reveal_row;
                            changed = true;
                        }
                        if pv.tree_reveal_key != reveal_key {
                            pv.tree_reveal_key = reveal_key;
                            changed = true;
                        }
                    }
                    // La estrella de favorito puede cambiar al navegar o al togglear.
                    if purpose == Some(PanePurpose::Files) {
                        let fav = c.is_pane_dir_favorite(id);
                        if pv.is_favorite != fav {
                            pv.is_favorite = fav;
                            changed = true;
                        }
                        // Aviso "sin coincidencias": filtro activo que vació la vista (F2).
                        let nm = c.no_matches(id);
                        if pv.no_matches != nm {
                            pv.no_matches = nm;
                            changed = true;
                        }
                        // Aviso "carpeta no encontrada": hay que refrescarlo en cada tick (no solo
                        // en sync_layout). Sin esto, al "subir nivel" / "elegir otra" / "reintentar"
                        // el panel navegaba a una carpeta válida pero el campo `missing` quedaba
                        // pegado en true y el aviso seguía tapando el listado.
                        let miss = c.pane_dir_missing(id);
                        if pv.missing != miss {
                            pv.missing = miss;
                            changed = true;
                        }
                        let miss_path = SharedString::from(c.path_of(id).as_str());
                        if pv.missing_path != miss_path {
                            pv.missing_path = miss_path;
                            changed = true;
                        }
                        let miss_anc = c.pane_has_existing_ancestor(id);
                        if pv.missing_has_ancestor != miss_anc {
                            pv.missing_has_ancestor = miss_anc;
                            changed = true;
                        }
                        let miss_ejected = c.pane_was_ejected(id);
                        if pv.missing_ejected != miss_ejected {
                            pv.missing_ejected = miss_ejected;
                            changed = true;
                        }
                        // Estado del botón de vista profunda: on/off según el job activo.
                        let deep = c.is_deep_active(id);
                        if pv.deep_active != deep {
                            pv.deep_active = deep;
                            changed = true;
                        }
                        // Fila enfocada (índice de vista): al navegar por teclado cambia `f.focused`
                        // y la UI (changed focused-row) arrastra el scroll para revelarla (C1). Va
                        // en el refresco central para cubrir TODO lo que mueve el foco (↑↓, Re/Av
                        // Pág, Inicio/Fin, typeahead, saltar desde la paleta), no solo un atajo.
                        let focused = c.focused_view_of(id);
                        if pv.focused_row != focused {
                            pv.focused_row = focused;
                            changed = true;
                        }
                        let restored_scroll = c.restored_scroll_row_of(id);
                        if pv.restore_scroll_row != restored_scroll {
                            pv.restore_scroll_row = restored_scroll;
                            changed = true;
                        }
                        let can_back = c.can_go_back_for(id);
                        if pv.can_go_back != can_back {
                            pv.can_go_back = can_back;
                            changed = true;
                        }
                        let can_forward = c.can_go_forward_for(id);
                        if pv.can_go_forward != can_forward {
                            pv.can_go_forward = can_forward;
                            changed = true;
                        }
                        // Footer (barra inferior): selección + disco. Vacío si está deshabilitado.
                        // El disco se cachea por unidad dentro del controlador (no pega a WinAPI
                        // en cada tick).
                        let footer = SharedString::from(c.footer_text_of(id).as_str());
                        if pv.footer_text != footer {
                            pv.footer_text = footer;
                            changed = true;
                        }
                        let footer_disk = SharedString::from(c.footer_disk_text_of(id).as_str());
                        if pv.footer_disk_text != footer_disk {
                            pv.footer_disk_text = footer_disk;
                            changed = true;
                        }
                        // Mini-barra del filtro visual por tipeo (vacío = oculta).
                        let filter_label = SharedString::from(c.filter_label_of(id).as_str());
                        if pv.filter_label != filter_label {
                            pv.filter_label = filter_label;
                            changed = true;
                        }
                        let filter_hiding = c.filter_hide_nonmatches;
                        if pv.filter_hiding != filter_hiding {
                            pv.filter_hiding = filter_hiding;
                            changed = true;
                        }
                    }
                    // Inspector/Preview son structs sueltas: se setean según el tipo.
                    if purpose == Some(PanePurpose::Inspector) {
                        pv.inspector = inspector.clone();
                        changed = true;
                    }
                    if purpose == Some(PanePurpose::Preview) {
                        pv.preview = current_preview_vm(&c);
                        changed = true;
                    }
                    if changed {
                        m.panes.set_row_data(i, pv);
                    }
                }
            }
            if let Some(id) = active {
                ui.set_active_path(SharedString::from(c.path_of(id).as_str()));
                ui.set_active_path_pane(id.0 as i32);
            } else {
                ui.set_active_path("".into());
                ui.set_active_path_pane(-1);
            }
            let path_target = if ui.get_edit_toolbar() && ui.get_edit_pane() >= 0 {
                Some(PaneId(ui.get_edit_pane() as u64))
            } else {
                active
            };
            ui.set_toolbar_path_favorite(path_target.is_some_and(|id| c.is_pane_dir_favorite(id)));
            ui.set_status(SharedString::from(c.status_line().as_str()));
            ui.set_sync_vm(c.sync_vm());
            ui.set_text_transform_vm(c.text_transform_vm());
            ui.set_delivery_visible(c.delivery.open);
            if c.task_spaces.open && !ui.get_spaces_visible() {
                ui.set_spaces_root("".into());
                ui.set_spaces_own_favorites(c.task_spaces.create_private_favorites);
            }
            ui.set_spaces_visible(c.task_spaces.open);
            if c.saved_queries.open
                && (!ui.get_query_visible() || ui.get_query_revision() != c.saved_queries.revision)
            {
                ui.set_query_draft(crate::callbacks_queries::draft_vm(&c.saved_queries.draft));
                ui.set_query_revision(c.saved_queries.revision);
            }
            ui.set_query_visible(c.saved_queries.open);
            let recipes = &c.recipes;
            if ui.get_recipe_revision() != recipes.revision {
                ui.set_recipe_query(crate::callbacks_queries::draft_vm(
                    &recipes.draft.query(recipes.parameters.roots.clone()),
                ));
                ui.set_recipe_source(
                    if recipes.draft.source == naygo_core::recipe::Source::Selection {
                        0
                    } else {
                        1
                    },
                );
                ui.set_recipe_layout(match recipes.draft.layout {
                    naygo_core::recipe::Layout::Flat => 0,
                    naygo_core::recipe::Layout::Grouped => 1,
                    naygo_core::recipe::Layout::Relative => 2,
                });
                ui.set_recipe_output_name(recipes.draft.output_name.clone().into());
                ui.set_recipe_parent(recipes.parameters.parent.display().to_string().into());
                ui.set_recipe_zip(recipes.draft.output == naygo_core::delivery::Output::Zip);
                ui.set_recipe_hashes(recipes.draft.hashes);
                ui.set_recipe_number_duplicates(recipes.draft.number_duplicates);
                ui.set_recipe_revision(recipes.revision);
            }
            ui.set_recipe_visible(recipes.open);
            ui.set_recipe_busy(recipes.busy());
            ui.set_recipe_ready(recipes.ready());
            ui.set_recipe_can_update(recipes.can_update());
            ui.set_recipe_report(recipes.report.clone().into());
            ui.set_recipe_progress(recipes.progress().to_string().into());
            ui.set_recipe_waiting(
                c.ops.ops_mode == crate::ops_ctrl::OpsMode::Queue && c.ops.any_running(),
            );
            let points = &c.comparison_points;
            if ui.get_points_revision() != points.revision {
                ui.set_points_root(points.root.clone().into());
                ui.set_points_exclusions(points.exclusions.clone().into());
                ui.set_points_recursive(points.recursive);
                ui.set_points_hashed(points.hashed);
                while points_model.row_count() > points.rows.len() {
                    points_model.remove(points_model.row_count() - 1);
                }
                for (index, row) in points.rows.iter().enumerate() {
                    if index >= points_model.row_count() {
                        points_model.push(row.clone());
                    } else if points_model.row_data(index).as_ref() != Some(row) {
                        points_model.set_row_data(index, row.clone());
                    }
                }
                ui.set_points_rows(ModelRc::from(points_model.clone()));
                ui.set_points_revision(points.revision);
            }
            ui.set_points_visible(points.open);
            ui.set_points_busy(points.busy());
            ui.set_points_loaded(points.loaded());
            ui.set_points_confirm_delete(points.confirm_delete);
            ui.set_points_has_selection(points.selected());
            ui.set_points_report(points.report.clone().into());
            ui.set_points_progress(points.progress().to_string().into());
            ui.set_query_busy(c.saved_queries.busy());
            ui.set_query_can_update(c.saved_queries.can_update());
            ui.set_query_report(c.saved_queries.report.clone().into());
            ui.set_spaces_busy(c.task_spaces.busy());
            ui.set_retry_visible(c.retry.open);
            ui.set_retry_busy(c.retry.busy());
            ui.set_retry_ready(c.retry.ready());
            if ui.get_retry_report().as_str() != c.retry.report {
                ui.set_retry_report(c.retry.report.clone().into());
            }
            let recents: Vec<SharedString> = c
                .task_spaces
                .recents
                .iter()
                .map(|p| SharedString::from(p.to_string_lossy().as_ref()))
                .collect();
            // No recrear botones en cada tick: conservar foco y navegación por teclado.
            if ui.get_spaces_recents().iter().ne(recents.iter().cloned()) {
                ui.set_spaces_recents(ModelRc::from(Rc::new(VecModel::from(recents))));
            }
            ui.set_spaces_can_update(c.task_spaces.can_update());
            ui.set_spaces_pending(c.task_spaces.has_pending());
            ui.set_spaces_name(c.task_spaces.name().into());
            if ui.get_spaces_report().as_str() != c.task_spaces.report {
                ui.set_spaces_report(c.task_spaces.report.clone().into());
            }
            ui.set_delivery_busy(c.delivery.rx.is_some());
            ui.set_delivery_ready(c.delivery.plan.is_some());
            ui.set_delivery_group_defaults(c.delivery.group_names.clone().into());
            ui.set_delivery_result_visible(c.delivery_results.latest.is_some());
            ui.set_delivery_result_success(c.delivery_result_location(false).is_some());
            if let Some(receipt) = &c.delivery_results.latest {
                if ui.get_delivery_result_report().as_str() != receipt.message {
                    ui.set_delivery_result_report(receipt.message.clone().into());
                }
            }
            ui.set_delivery_waiting(
                c.ops.ops_mode == crate::ops_ctrl::OpsMode::Queue && c.ops.any_running(),
            );
            if ui.get_delivery_report().as_str() != c.delivery.report {
                ui.set_delivery_report(c.delivery.report.clone().into());
            }
            // Botones Atrás/Adelante del toolbar: habilitados según el historial del panel activo.
            // Va aquí (refresco central) para que se actualicen tras cualquier navegación —teclado,
            // mouse, doble-clic, breadcrumbs— no solo al pulsar los botones.
            ui.set_can_go_back(c.can_go_back());
            ui.set_can_go_forward(c.can_go_forward());
            // Casillas del menú del "ojo" (visibilidad): reflejan los settings. Refresco central
            // para que queden al día tras alternar, factory-reset o recarga de config.
            ui.set_show_hidden(c.config.settings.show_hidden);
            ui.set_show_system(c.config.settings.show_system);
            ui.set_hide_dotfiles(c.config.settings.hide_dotfiles);
            // Operaciones de archivo (F3): modal activo + filas de progreso + retomar.
            ui.set_op_dialog(to_op_dialog_vm(c.ops.dialog_vm()));
            let op_rows: Vec<OpRowVm> = c
                .ops
                .op_rows(c.config.settings.date_format)
                .into_iter()
                .map(to_op_row_vm)
                .collect();
            // El panel rico de operaciones consume modelos separados por zona (kind: 0=en curso
            // 1=en cola 2=historial 3=calculando). Separarlos en Rust evita filas-fantasma en Slint.
            let running: Vec<OpRowVm> = op_rows.iter().filter(|r| r.kind == 0).cloned().collect();
            let queued: Vec<OpRowVm> = op_rows.iter().filter(|r| r.kind == 1).cloned().collect();
            let history: Vec<OpRowVm> = op_rows.iter().filter(|r| r.kind == 2).cloned().collect();
            let planning: Vec<OpRowVm> = op_rows.iter().filter(|r| r.kind == 3).cloned().collect();
            ui.set_op_running_count(running.len() as i32);
            ui.set_op_queued_count(queued.len() as i32);
            ui.set_op_history_count(history.len() as i32);
            ui.set_op_planning_count(planning.len() as i32);
            ui.set_op_running_rows(ModelRc::from(Rc::new(VecModel::from(running))));
            ui.set_op_queued_rows(ModelRc::from(Rc::new(VecModel::from(queued))));
            ui.set_op_history_rows(ModelRc::from(Rc::new(VecModel::from(history))));
            ui.set_op_planning_rows(ModelRc::from(Rc::new(VecModel::from(planning))));
            // Brillo animado de la barra del panel de ops: solo si el usuario lo activó (default false).
            ui.set_op_animations_enabled(c.config.animations_enabled());
            let resume_rows: Vec<ResumeRowVm> = c
                .ops
                .resume_rows()
                .into_iter()
                .map(|(id, label)| ResumeRowVm {
                    id: SharedString::from(id.as_str()),
                    label: SharedString::from(label.as_str()),
                })
                .collect();
            ui.set_resume_rows(ModelRc::from(Rc::new(VecModel::from(resume_rows))));
            let clipboard_rows: Vec<ClipboardHistoryRowVm> = c
                .ops
                .clipboard_history()
                .iter()
                .map(|entry| {
                    let label = entry
                        .paths
                        .first()
                        .and_then(|path| path.file_name())
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let parent = entry.paths[0]
                        .parent()
                        .map(|path| path.display().to_string())
                        .unwrap_or_default();
                    let detail = if entry.paths.len() == 1 {
                        parent
                    } else {
                        format!("{parent}  +{}", entry.paths.len() - 1)
                    };
                    ClipboardHistoryRowVm {
                        label: label.into(),
                        detail: detail.into(),
                        cut: entry.cut,
                    }
                })
                .collect();
            ui.set_clipboard_history_rows(ModelRc::from(Rc::new(VecModel::from(clipboard_rows))));
            // Menú contextual: posición + si hay menú nativo disponible (hay HWND).
            let ctx = match &c.context_menu {
                Some(cm) => ContextMenuVm {
                    active: true,
                    x: cm.x,
                    y: cm.y,
                    has_native: naygo_hwnd(&ui).is_some(),
                    has_wt: c.windows_terminal_available(),
                    folder_mode: cm.folder_mode,
                    is_single_zip: c.sel_is_single_zip(),
                    has_selection: !c.selected_paths().is_empty(),
                    // Carpeta objetivo (habilita el submenú "Abrir ▸"): flag YA cacheado al abrir
                    // el menú (evita un `stat` por tick, costoso en shares de red lentos).
                    target_is_folder: cm.target_is_folder,
                    show_open_here: cm.show_open_here,
                    target_is_executable: cm
                        .targets
                        .first()
                        .is_some_and(|p| naygo_platform::open::can_run_as_administrator(p)),
                },
                None => ContextMenuVm {
                    active: false,
                    x: 0.0,
                    y: 0.0,
                    has_native: false,
                    has_wt: false,
                    folder_mode: false,
                    is_single_zip: false,
                    has_selection: false,
                    target_is_folder: false,
                    show_open_here: false,
                    target_is_executable: false,
                },
            };
            ui.set_ctx_menu(ctx);
            // Modal "nueva(s) carpeta(s)".
            let (nf_valid, _nf_invalid) = c.new_folder_counts();
            ui.set_new_folder_vm(NewFolderVm {
                active: c.new_folder_open(),
                dir: c.new_folder_dir().into(),
                text: c.new_folder_text().into(),
                status: c.new_folder_status().into(),
                can_create: nf_valid > 0,
            });
            // (El aviso "carpeta no encontrada" es ahora IN-PLACE por panel: se arma en el PaneVm
            //  con `missing`/`missing-path`; ya no hay un VM de modal global.)
            // Panel de búsqueda recursiva (F3 / lupa).
            {
                let open = c.search_open();
                let (status, running) = c.search_status_text();
                let root_label = c.search_root_label();
                let query = c.search_query();
                let options = c.search_options();
                let hits: Vec<SearchHitVm> = c
                    .search_rows()
                    .into_iter()
                    .map(|r| SearchHitVm {
                        selected: r.selected,
                        name: r.name.into(),
                        rel_dir: r.rel_dir.into(),
                        detail: r.detail.into(),
                        is_dir: r.is_dir,
                        icon: r.icon,
                    })
                    .collect();
                while search_model.row_count() > hits.len() {
                    search_model.remove(search_model.row_count() - 1);
                }
                for (index, hit) in hits.into_iter().enumerate() {
                    if index >= search_model.row_count() {
                        search_model.push(hit);
                    } else if search_model.row_data(index).as_ref() != Some(&hit) {
                        search_model.set_row_data(index, hit);
                    }
                }
                ui.set_search_vm(SearchVm {
                    advanced: c
                        .search_job
                        .as_ref()
                        .is_some_and(|j| j.details.advanced.is_some()),
                    focused_row: c.search_focused_index(),
                    marked_count: c.search_marked_count() as i32,
                    active: open,
                    query: query.into(),
                    root_label: root_label.into(),
                    content_query: options.content_query.into(),
                    ignore_case: options.ignore_case,
                    use_wildcards: options.use_wildcards,
                    recursive: options.recursive,
                    running,
                    hits: ModelRc::from(search_model.clone()),
                    status: status.into(),
                });
            }
            // Menú/editor de columna (clic derecho en el header, F2).
            let colmenu = match c.column_menu_snapshot() {
                Some(m) => {
                    let no_ext = ui.global::<Tr>().get_colfilter_no_ext();
                    let folder = ui.global::<Tr>().get_dlg_kind_folder();
                    let exts: Vec<ExtRowVm> = m
                        .exts
                        .into_iter()
                        .map(|e| {
                            let label = if e.ext == naygo_core::filter::DIRECTORY_TYPE_FILTER_KEY {
                                folder.clone()
                            } else if e.ext.is_empty() {
                                no_ext.clone()
                            } else {
                                SharedString::from(e.ext.as_str())
                            };
                            ExtRowVm {
                                ext: SharedString::from(e.ext.as_str()),
                                label,
                                count: e.count as i32,
                                checked: e.checked,
                            }
                        })
                        .collect();
                    ColumnMenuVm {
                        active: true,
                        x: m.x,
                        y: m.y,
                        kind: m.kind,
                        label: SharedString::from(m.label.as_str()),
                        mode: m.mode,
                        has_filter: m.has_filter,
                        can_hide: m.can_hide,
                        text_draft: SharedString::from(m.text_draft.as_str()),
                        text_case: m.text_case,
                        min_draft: SharedString::from(m.min_draft.as_str()),
                        max_draft: SharedString::from(m.max_draft.as_str()),
                        exts: ModelRc::from(Rc::new(VecModel::from(exts))),
                    }
                }
                None => ColumnMenuVm {
                    active: false,
                    x: 0.0,
                    y: 0.0,
                    kind: 0,
                    label: SharedString::new(),
                    mode: 0,
                    has_filter: false,
                    can_hide: false,
                    text_draft: SharedString::new(),
                    text_case: false,
                    min_draft: SharedString::new(),
                    max_draft: SharedString::new(),
                    exts: ModelRc::from(Rc::new(VecModel::<ExtRowVm>::default())),
                },
            };
            ui.set_column_menu(colmenu);

            // Ventana de renombrado por lotes (F5): espejo del estado + preview en vivo.
            let batch = match &c.batch {
                Some(b) => {
                    use naygo_core::batch_rename::{CaseTransform, RowStatus};
                    let rows_src = c.batch_preview();
                    let rows: Vec<BatchRowVm> = rows_src
                        .iter()
                        .map(|r| BatchRowVm {
                            old_name: SharedString::from(r.old_name.as_str()),
                            new_name: SharedString::from(r.new_name.as_str()),
                            status: match r.status {
                                RowStatus::Ok => 0,
                                RowStatus::Unchanged => 1,
                                RowStatus::Invalid(_) => 2,
                                RowStatus::Collision => 3,
                                RowStatus::Unrepresentable => 4,
                            },
                        })
                        .collect();
                    BatchRenameVm {
                        active: true,
                        template: SharedString::from(b.spec.template.as_str()),
                        find: SharedString::from(b.spec.find.as_str()),
                        replace: SharedString::from(b.spec.replace.as_str()),
                        use_regex: b.spec.use_regex,
                        include_ext: b.spec.include_ext,
                        case: match b.spec.case {
                            CaseTransform::None => 0,
                            CaseTransform::Lower => 1,
                            CaseTransform::Upper => 2,
                            CaseTransform::Title => 3,
                        },
                        counter_start: SharedString::from(b.spec.counter_start.to_string()),
                        counter_step: SharedString::from(b.spec.counter_step.to_string()),
                        count: b.items.len() as i32,
                        can_apply: c.batch_can_apply(),
                        rows: ModelRc::from(Rc::new(VecModel::from(rows))),
                    }
                }
                None => BatchRenameVm {
                    active: false,
                    template: SharedString::new(),
                    find: SharedString::new(),
                    replace: SharedString::new(),
                    use_regex: false,
                    include_ext: false,
                    case: 0,
                    counter_start: SharedString::new(),
                    counter_step: SharedString::new(),
                    count: 0,
                    can_apply: false,
                    rows: ModelRc::from(Rc::new(VecModel::<BatchRowVm>::default())),
                },
            };
            ui.set_batch(batch);

            // Ayuda (F1): estado + atajos activos (leídos del keymap en vivo).
            ui.set_help_open(c.help_open);
            let help_rows: Vec<HelpRowVm> = c
                .help_shortcuts()
                .into_iter()
                .map(|(label, chord)| HelpRowVm {
                    label: SharedString::from(label.as_str()),
                    chord: SharedString::from(chord.as_str()),
                })
                .collect();
            ui.set_help_shortcuts(ModelRc::from(Rc::new(VecModel::from(help_rows))));
            crate::logging::set_diag_snapshot(c.diag_snapshot());
        }
    });

    // Reconcilia la ESTRUCTURA (paneles + splitters) con el estado del core. Solo
    // reconstruye cuando cambia la lista de IDs o el área. Tras reestructurar, sincroniza.
    let sync_layout: Rc<dyn Fn()> = {
        let ctrl = ctrl.clone();
        let models = models.clone();
        let area_of = area_of.clone();
        let sync_rows = sync_rows.clone();
        Rc::new(move || {
            let _sync_metric = crate::metrics::span("ui.sync_layout");
            let area = area_of();
            ctrl.borrow_mut().set_area(area);
            let pane_rects = ctrl.borrow().pane_rects(area);
            let split_handles = ctrl.borrow().split_handles(area);
            // Grupos de pestañas: solo se PINTA la pestaña activa de cada grupo (todas
            // comparten rect). Los miembros ocultos se filtran; al activo se le adjunta la
            // lista de pestañas para que pinte la barra.
            let groups = ctrl.borrow().tab_groups();
            let grouped: std::collections::HashSet<PaneId> =
                groups.iter().flat_map(|(m, _)| m.iter().copied()).collect();
            let active_members: std::collections::HashSet<PaneId> = groups
                .iter()
                .filter_map(|(m, a)| m.get(*a).copied())
                .collect();
            // Rects visibles: panel no agrupado, o la pestaña activa de su grupo.
            let visible: Vec<(PaneId, Rect)> = pane_rects
                .iter()
                .filter(|(id, _)| !grouped.contains(id) || active_members.contains(id))
                .copied()
                .collect();
            let new_ids: Vec<PaneId> = visible.iter().map(|(id, _)| *id).collect();
            // Todos los ids del layout (visibles + ocultos) para conservar sus modelos.
            let all_ids: Vec<PaneId> = ctrl
                .borrow()
                .ws
                .layout
                .pane_rects(area)
                .iter()
                .map(|(id, _)| *id)
                .collect();

            let mut m = models.borrow_mut();
            // La estructura cambió si cambió la lista visible, el área, o algún grupo
            // (apilar/activar pestaña no cambia los ids visibles pero sí las barras).
            let structure_changed =
                new_ids != m.pane_ids || !rects_equal(area, m.area) || groups != m.groups;

            if structure_changed {
                let active = ctrl.borrow().active_id();
                let panes: Vec<PaneVm> = visible
                    .iter()
                    .map(|(id, r)| {
                        let c = ctrl.borrow();
                        let purpose = c.purpose_of(*id).map(purpose_to_int).unwrap_or(0);
                        // Si este id es la pestaña activa de un grupo, armar su barra.
                        let (tabs, active_tab_index): (Vec<TabVm>, i32) = groups
                            .iter()
                            .find(|(mem, a)| mem.get(*a) == Some(id))
                            .map(|(mem, a)| {
                                (
                                    mem.iter()
                                        .enumerate()
                                        .map(|(i, mid)| TabVm {
                                            id: mid.0 as i32,
                                            label: SharedString::from(c.pane_label(*mid).as_str()),
                                            active: i == *a,
                                        })
                                        .collect(),
                                    *a as i32,
                                )
                            })
                            .unwrap_or_default();
                        drop(c);
                        let pm = m.models_for(*id);
                        PaneVm {
                            id: id.0 as i32,
                            x: r.x,
                            y: r.y,
                            w: r.w,
                            h: r.h,
                            path: SharedString::from(ctrl.borrow().path_of(*id).as_str()),
                            active: Some(*id) == active,
                            // Resaltado de arrastre: true solo para el panel bajo el cursor mientras
                            // se arrastran archivos encima. `drag_over_pane()` es transitorio (lo
                            // setea el drenado del canal de hover en el tick).
                            drag_over: ctrl.borrow().drag_over_pane() == Some(*id),
                            drag_client_y: ctrl.borrow().drag_over_client_y_for(*id),
                            comparison_active: !ctrl.borrow().comparison.is_empty(),
                            comparison_link_available: ctrl
                                .borrow()
                                .comparison_pair
                                .is_some_and(|(left, right)| *id == left || *id == right),
                            comparison_link_active: ctrl.borrow().comparison_link_enabled,
                            purpose,
                            title: SharedString::from(ctrl.borrow().pane_label(*id).as_str()),
                            link_member: ctrl.borrow().is_link_member(*id),
                            tree_linked: ctrl.borrow().tree_is_linked(*id),
                            tree_link_label: SharedString::from(
                                ctrl.borrow().tree_link_label(*id).as_str(),
                            ),
                            tree_reveal_row: ctrl.borrow().tree_reveal_row(*id),
                            tree_reveal_key: SharedString::from(ctrl.borrow().tree_reveal_key(*id)),
                            rows: ModelRc::from(pm.rows.clone()),
                            columns: ModelRc::from(pm.columns.clone()),
                            col_menu: ModelRc::from(pm.col_menu.clone()),
                            is_favorite: ctrl.borrow().is_pane_dir_favorite(*id),
                            no_matches: ctrl.borrow().no_matches(*id),
                            // Carpeta no encontrada / ilegible: aviso in-place con opciones.
                            missing: ctrl.borrow().pane_dir_missing(*id),
                            missing_path: SharedString::from(ctrl.borrow().path_of(*id).as_str()),
                            // "Subir un nivel" solo tiene sentido si hay un ancestro existente real.
                            missing_has_ancestor: ctrl.borrow().pane_has_existing_ancestor(*id),
                            // ¿La pérdida fue por expulsión de disco? Cambia el texto del aviso.
                            missing_ejected: ctrl.borrow().pane_was_ejected(*id),
                            // Fila enfocada (índice de vista) para el auto-scroll por teclado (C1).
                            focused_row: ctrl.borrow().focused_view_of(*id),
                            restore_scroll_row: ctrl.borrow().restored_scroll_row_of(*id),
                            deep_active: ctrl.borrow().is_deep_active(*id),
                            // El footer se llena en el primer `sync_rows` (necesita `&mut` por la
                            // caché de disco). Aquí nace vacío.
                            footer_text: SharedString::new(),
                            footer_disk_text: SharedString::new(),
                            can_go_back: ctrl.borrow().can_go_back_for(*id),
                            can_go_forward: ctrl.borrow().can_go_forward_for(*id),
                            // La mini-barra del filtro igual nace vacía (la llena sync_rows).
                            filter_label: SharedString::new(),
                            filter_hiding: false,
                            segments: {
                                let segs: Vec<PathSeg> = ctrl
                                    .borrow()
                                    .path_segments_of(*id)
                                    .into_iter()
                                    .map(|(label, path)| PathSeg {
                                        label: SharedString::from(label.as_str()),
                                        path: SharedString::from(path.as_str()),
                                    })
                                    .collect();
                                ModelRc::from(Rc::new(VecModel::from(segs)))
                            },
                            tree_rows: ModelRc::from(pm.tree.clone()),
                            favs: ModelRc::from(pm.favs.clone()),
                            recents: ModelRc::from(pm.recents.clone()),
                            frequent_dirs: ModelRc::from(pm.frequent_dirs.clone()),
                            fav_tree: ModelRc::from(pm.fav_tree.clone()),
                            hist_rows: ModelRc::from(pm.hist.clone()),
                            inspector: InspectorVm::default(),
                            preview: PreviewVm::default(),
                            basket_rows: ModelRc::from(pm.basket.clone()),
                            active_tab_index,
                            tabs: ModelRc::from(Rc::new(VecModel::from(tabs))),
                        }
                    })
                    .collect();
                m.panes.set_vec(panes);

                let splits: Vec<SplitVm> = split_handles
                    .iter()
                    .enumerate()
                    .map(|(i, h)| SplitVm {
                        index: i as i32,
                        divider: h.divider as i32,
                        x: h.rect.x,
                        y: h.rect.y,
                        w: h.rect.w,
                        h: h.rect.h,
                        horizontal: matches!(h.dir, SplitDir::Horizontal),
                    })
                    .collect();
                m.splits.set_vec(splits);

                m.per_pane.retain(|id, _| all_ids.contains(id));
                m.pane_ids = new_ids;
                m.groups = groups;
                m.area = area;
            } else {
                // La estructura NO cambió (mismos paneles, misma área, mismos grupos), pero los
                // RECTS pueden haber cambiado al arrastrar un splitter (cambia la fraction, no los
                // ids). Antes esto quedaba fuera del rebuild y el resize "no hacía nada". Ahora se
                // actualizan los x/y/w/h de cada PaneVm IN SITU (sin recrear modelos → conserva el
                // scroll) y se reposicionan las barras de splitter.
                for (id, r) in &visible {
                    if let Some(i) = m.pane_ids.iter().position(|p| p == id) {
                        if let Some(mut pv) = m.panes.row_data(i) {
                            let link_member = ctrl.borrow().is_link_member(*id);
                            let tree_linked = ctrl.borrow().tree_is_linked(*id);
                            let tree_link_label =
                                SharedString::from(ctrl.borrow().tree_link_label(*id).as_str());
                            let link_changed = pv.link_member != link_member
                                || pv.tree_linked != tree_linked
                                || pv.tree_link_label != tree_link_label;
                            let geometry_changed =
                                pv.x != r.x || pv.y != r.y || pv.w != r.w || pv.h != r.h;
                            if geometry_changed {
                                pv.x = r.x;
                                pv.y = r.y;
                                pv.w = r.w;
                                pv.h = r.h;
                            }
                            if link_changed {
                                pv.link_member = link_member;
                                pv.tree_linked = tree_linked;
                                pv.tree_link_label = tree_link_label;
                            }
                            if link_changed || geometry_changed {
                                m.panes.set_row_data(i, pv);
                            }
                        }
                    }
                }
                let splits: Vec<SplitVm> = split_handles
                    .iter()
                    .enumerate()
                    .map(|(i, h)| SplitVm {
                        index: i as i32,
                        divider: h.divider as i32,
                        x: h.rect.x,
                        y: h.rect.y,
                        w: h.rect.w,
                        h: h.rect.h,
                        horizontal: matches!(h.dir, SplitDir::Horizontal),
                    })
                    .collect();
                m.splits.set_vec(splits);
            }

            // Selector de panel destino: rect de cada candidato (orden visual) + su número.
            // Se reconstruye siempre (puede aparecer/desaparecer sin cambio de estructura).
            let picks: Vec<PickVm> = {
                let c = ctrl.borrow();
                match &c.pending_pick {
                    Some(pick) => {
                        let rects: std::collections::HashMap<PaneId, Rect> =
                            c.pane_rects(area).into_iter().collect();
                        pick.candidates
                            .iter()
                            .enumerate()
                            .filter_map(|(i, id)| {
                                rects.get(id).map(|r| PickVm {
                                    x: r.x,
                                    y: r.y,
                                    w: r.w,
                                    h: r.h,
                                    number: (i + 1) as i32,
                                })
                            })
                            .collect()
                    }
                    None => Vec::new(),
                }
            };
            m.picks.set_vec(picks);
            let radar_rows: Vec<DestinationRadarRowVm> = ctrl
                .borrow()
                .destination_radar
                .as_ref()
                .map(|radar| {
                    radar
                        .candidates
                        .iter()
                        .enumerate()
                        .map(|(i, candidate)| DestinationRadarRowVm {
                            number: (i + 1) as i32,
                            label: SharedString::from(candidate.label.as_str()),
                            path: SharedString::from(candidate.path.to_string_lossy().as_ref()),
                        })
                        .collect()
                })
                .unwrap_or_default();
            m.destination_radar.set_vec(radar_rows);

            drop(m);
            sync_rows();
        })
    };
    SyncHandles {
        sync_rows,
        sync_layout,
    }
}

/// Construye el closure que aplica un cambio de unidades (USB enchufado/quitado): drena el
/// watcher, reubica paneles huérfanos y RECONSTRUYE la tira de discos de la toolbar. Se llama
/// desde el timer Y desde `on_wake` para que un USB recién conectado aparezca EN VIVO aunque el
/// timer esté dormido (antes el refresh solo ocurría en el tick, que se apaga en reposo, y el
/// USB no salía hasta interactuar/reabrir). Idempotente: si no hubo cambios reales, no hace nada.
pub(crate) fn build_apply_device_change(
    ctrl: Rc<RefCell<WorkspaceCtrl>>,
    devices: Rc<crate::devices::Devices>,
    home: Rc<std::path::PathBuf>,
    ui_weak: slint::Weak<AppWindow>,
    waker: naygo_platform::dir_watch::Waker,
) -> Rc<dyn Fn()> {
    let waker = waker.clone();
    Rc::new(move || {
        if !devices.drives_changed() {
            return;
        }
        // El espacio en disco pudo cambiar (USB conectado/expulsado): invalida la caché del
        // footer para que se relea en el próximo tick, igual que se refresca la tira de discos.
        ctrl.borrow_mut().invalidate_footer_disk_cache();
        ctrl.borrow_mut().request_drive_refresh(waker.clone());
        // Un disco que se va/llega puede cambiar el estado "carpeta no encontrada" de paneles
        // abiertos en él: recalcular el caché (fuera del tick de render).
        ctrl.borrow_mut().refresh_missing_cache();
        let moved = ctrl.borrow_mut().relocate_orphans(&home);
        for id in moved {
            let dir = ctrl
                .borrow()
                .ws
                .pane(id)
                .and_then(|p| p.files.as_ref())
                .map(|f| f.current_dir.clone());
            if let Some(dir) = dir {
                ctrl.borrow_mut().start_listing(id, dir);
            }
        }
        if let Some(ui) = ui_weak.upgrade() {
            let drives: Vec<NavRow> = ctrl
                .borrow_mut()
                .drive_strip()
                .into_iter()
                .map(to_nav_row)
                .collect();
            ui.set_drives(ModelRc::from(Rc::new(VecModel::from(drives))));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_vec(model: &VecModel<i32>) -> Vec<i32> {
        model.iter().collect()
    }

    #[test]
    fn incremental_agrega_al_final_sin_tocar_prefijo() {
        let m = VecModel::from(vec![1, 2, 3]);
        apply_rows_incremental(&m, vec![1, 2, 3, 4]);
        assert_eq!(to_vec(&m), vec![1, 2, 3, 4]);
    }

    #[test]
    fn incremental_inserta_en_el_medio() {
        let m = VecModel::from(vec![1, 2, 4, 5]);
        apply_rows_incremental(&m, vec![1, 2, 3, 4, 5]);
        assert_eq!(to_vec(&m), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn incremental_quita_en_el_medio() {
        let m = VecModel::from(vec![1, 2, 3, 4, 5]);
        apply_rows_incremental(&m, vec![1, 2, 4, 5]);
        assert_eq!(to_vec(&m), vec![1, 2, 4, 5]);
    }

    #[test]
    fn incremental_quita_al_final() {
        let m = VecModel::from(vec![1, 2, 3, 4]);
        apply_rows_incremental(&m, vec![1, 2]);
        assert_eq!(to_vec(&m), vec![1, 2]);
    }

    #[test]
    fn incremental_reemplazo_total() {
        let m = VecModel::from(vec![1, 2, 3]);
        apply_rows_incremental(&m, vec![7, 8, 9, 10]);
        assert_eq!(to_vec(&m), vec![7, 8, 9, 10]);
    }

    #[test]
    fn incremental_de_vacio_y_a_vacio() {
        let m = VecModel::from(Vec::<i32>::new());
        apply_rows_incremental(&m, vec![1, 2]);
        assert_eq!(to_vec(&m), vec![1, 2]);
        apply_rows_incremental(&m, vec![]);
        assert_eq!(to_vec(&m), Vec::<i32>::new());
    }

    #[test]
    fn incremental_mismo_contenido_es_noop() {
        let m = VecModel::from(vec![1, 2, 3]);
        apply_rows_incremental(&m, vec![1, 2, 3]);
        assert_eq!(to_vec(&m), vec![1, 2, 3]);
    }

    #[test]
    fn filas_compara_el_contenido_de_las_celdas_no_su_identidad() {
        let icon =
            slint::Image::from_rgba8(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(1, 1));
        let make_row = |cell: &str| RowData {
            name: SharedString::from("archivo.txt"),
            cells: ModelRc::from(Rc::new(VecModel::from(vec![SharedString::from(cell)]))),
            is_dir: false,
            selected: false,
            focused: false,
            cut: false,
            highlight: false,
            compare_state: 0,
            filter_match: false,
            match_pre: SharedString::new(),
            match_mid: SharedString::new(),
            match_post: SharedString::new(),
            icon: icon.clone(),
            depth: 0,
        };

        assert!(row_data_eq(&make_row("1 KiB"), &make_row("1 KiB")));
        assert!(!row_data_eq(&make_row("1 KiB"), &make_row("2 KiB")));
    }

    #[test]
    fn sync_rows_aplica_append_visible_y_reconcilia_append_que_intercala_el_sort() {
        use naygo_core::fs_model::{Entry, EntryKind};
        use naygo_core::workspace::layout::Rect;
        use slint::platform::{software_renderer::MinimalSoftwareWindow, Platform, WindowAdapter};
        use std::time::SystemTime;

        struct Headless(Rc<MinimalSoftwareWindow>);
        impl Platform for Headless {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                Ok(self.0.clone())
            }
        }

        fn entry(root: &std::path::Path, name: &str) -> Entry {
            Entry {
                name: name.to_owned(),
                path: root.join(name),
                kind: EntryKind::File,
                size: Some(1),
                modified: Some(SystemTime::UNIX_EPOCH),
                created: None,
                hidden: false,
                system: false,
            }
        }

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        std::fs::create_dir(&root).unwrap();
        let adapter = MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::NewBuffer,
        );
        slint::platform::set_platform(Box::new(Headless(adapter))).unwrap();
        let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new_in(
            root.clone(),
            temp.path().join("settings"),
        )));
        let models = Rc::new(RefCell::new(Models::new()));
        let ui = AppWindow::new().unwrap();
        let sync = build_sync(
            ui.as_weak(),
            ctrl.clone(),
            models.clone(),
            Rc::new(RefCell::new(HashMap::new())),
            Rc::new(|| Rect {
                x: 0.0,
                y: 0.0,
                w: 1280.0,
                h: 800.0,
            }),
        );
        (sync.sync_layout)();
        let id = ctrl.borrow().active_id().unwrap();
        let rows = models.borrow_mut().models_for(id).rows.clone();
        {
            let mut workspace = ctrl.borrow_mut();
            let files = workspace.ws.active_files_mut().unwrap();
            files.entries = vec![entry(&root, "a.txt"), entry(&root, "z.txt")];
            files.entries_changed();
        }
        (sync.sync_rows)();
        assert_eq!(
            rows.iter()
                .map(|row| row.name.to_string())
                .collect::<Vec<_>>(),
            ["a.txt", "z.txt"]
        );

        {
            let mut workspace = ctrl.borrow_mut();
            let files = workspace.ws.active_files_mut().unwrap();
            files.entries.push(entry(&root, "zz.txt"));
            files.entries_appended(2);
        }
        (sync.sync_rows)();
        assert_eq!(
            rows.iter()
                .map(|row| row.name.to_string())
                .collect::<Vec<_>>(),
            ["a.txt", "z.txt", "zz.txt"]
        );

        {
            let mut workspace = ctrl.borrow_mut();
            let files = workspace.ws.active_files_mut().unwrap();
            files.entries.push(entry(&root, "m.txt"));
            files.entries_appended(3);
        }
        (sync.sync_rows)();
        assert_eq!(
            rows.iter()
                .map(|row| row.name.to_string())
                .collect::<Vec<_>>(),
            ["a.txt", "m.txt", "z.txt", "zz.txt"],
            "un append que intercalaría el orden debe reconciliar el listado"
        );
    }

    /// Invoca el callback de teclado generado por Slint, conectado en `wire_nav_keys`.
    /// Es headless: mide el callback de producción, no el dispatch del SO ni Winit.
    #[test]
    #[ignore = "medición manual W-07 del callback Slint headless; usar el supervisor de release"]
    fn w07_headless_keyboard_callback_measurement() {
        use crate::wire::WireCtx;
        use crate::workspace_ctrl::WorkspaceCtrl;
        use naygo_core::workspace::layout::Rect;
        use slint::platform::{software_renderer::MinimalSoftwareWindow, Platform, WindowAdapter};
        use std::time::{Duration, Instant};

        const EVENT_COUNT: usize = 256;

        struct Headless(Rc<MinimalSoftwareWindow>);
        impl Platform for Headless {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                Ok(self.0.clone())
            }
        }

        fn median_us(mut samples: Vec<f64>) -> f64 {
            samples.sort_by(f64::total_cmp);
            samples[samples.len() / 2]
        }

        let temp = tempfile::tempdir().unwrap();
        let metrics_enabled =
            std::env::var_os("NAYGO_PERF_METRICS").as_deref() == Some(std::ffi::OsStr::new("1"));
        if metrics_enabled {
            crate::logging::init(temp.path().join("logs"));
            crate::metrics::init();
        }

        let adapter = MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::NewBuffer,
        );
        slint::platform::set_platform(Box::new(Headless(adapter.clone()))).unwrap();
        let root = temp.path().join("files");
        std::fs::create_dir(&root).unwrap();
        let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new_in(
            root,
            temp.path().join("settings"),
        )));
        let ui = AppWindow::new().unwrap();
        let ctx = WireCtx {
            ctrl,
            sync_rows: Rc::new(|| {}),
            sync_layout: Rc::new(|| {}),
            start_timer: Rc::new(|| {}),
            area_of: Rc::new(|| Rect {
                x: 0.0,
                y: 0.0,
                w: 1280.0,
                h: 800.0,
            }),
            palette_cmds: Rc::new(RefCell::new(Vec::new())),
            palette_cmd_indices: Rc::new(RefCell::new(Vec::new())),
            pending_eject: Rc::new(RefCell::new(None)),
            pending_eject_panes: Rc::new(RefCell::new(Vec::new())),
            pending_undo: Rc::new(RefCell::new(None)),
        };
        crate::callbacks_nav::wire_nav_keys(&ui, &ctx);
        let mut callback_samples = Vec::with_capacity(EVENT_COUNT);
        for _ in 0..EVENT_COUNT {
            let started = Instant::now();
            ui.invoke_key("DownArrow".into(), false, false, false);
            callback_samples.push(started.elapsed().as_secs_f64() * 1_000_000.0);
        }

        if metrics_enabled {
            crate::metrics::shutdown();
            assert!(crate::logging::flush(Duration::from_secs(2)));
            let log_path = crate::logging::log_path();
            let log = std::fs::read_to_string(&log_path).unwrap();
            assert!(
                log.contains(&format!(
                    "[perf] kind=ui.input.keyboard_callback count={EVENT_COUNT}"
                )),
                "Slint dispatch did not reach the production keyboard callback; log:\n{log}"
            );
        }
        eprintln!(
            "W-07 Slint callback samples={EVENT_COUNT} callback_p50_us={:.3} callback_max_us={:.3} metrics_enabled={metrics_enabled}",
            median_us(callback_samples.clone()),
            callback_samples.iter().copied().fold(0.0, f64::max),
        );
    }

    /// Medición manual de `sync_rows` y el renderer software de Slint para W-07/W-10C. Se excluye del gate
    /// porque crea 100k entradas y su rasterizado headless no representa interacción/presentación.
    #[test]
    #[ignore = "medición manual W-07/W-10C; correr explícitamente con --ignored --nocapture"]
    fn w10c_headless_sync_measurement() {
        use naygo_core::fs_model::{Entry, EntryKind};
        use naygo_core::workspace::layout::Rect;
        use slint::platform::{
            software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
            Platform, WindowAdapter,
        };
        use std::time::{Duration, Instant, SystemTime};

        struct Headless(Rc<MinimalSoftwareWindow>);
        impl Platform for Headless {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                Ok(self.0.clone())
            }
        }

        fn median_ms(mut samples: Vec<f64>) -> f64 {
            samples.sort_by(f64::total_cmp);
            samples[samples.len() / 2]
        }

        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        std::fs::create_dir(&root).unwrap();
        let ui = AppWindow::new().unwrap();
        let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new_in(
            root.clone(),
            temp.path().join("settings"),
        )));
        let models = Rc::new(RefCell::new(Models::new()));
        let sync = build_sync(
            ui.as_weak(),
            ctrl.clone(),
            models,
            Rc::new(RefCell::new(HashMap::new())),
            Rc::new(|| Rect {
                x: 0.0,
                y: 0.0,
                w: 1280.0,
                h: 800.0,
            }),
        );
        (sync.sync_layout)();
        ui.show().unwrap();
        window.set_size(slint::PhysicalSize::new(1280, 800));
        let mut pixels = vec![slint::Rgb8Pixel::default(); 1280 * 800];

        for count in [10_000usize, 100_000] {
            {
                let mut workspace = ctrl.borrow_mut();
                let files = workspace.ws.active_files_mut().unwrap();
                files.entries.clear();
                files.entries_changed();
            }
            (sync.sync_rows)();

            let entries: Vec<Entry> = (0..count)
                .map(|index| Entry {
                    name: format!("entry-{index:06}.dat"),
                    path: root.join(format!("entry-{index:06}.dat")),
                    kind: EntryKind::File,
                    size: Some(index as u64),
                    modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(index as u64)),
                    created: None,
                    hidden: false,
                    system: false,
                })
                .collect();
            {
                let mut workspace = ctrl.borrow_mut();
                let files = workspace.ws.active_files_mut().unwrap();
                files.entries = entries;
                files.entries_changed();
            }

            let start = Instant::now();
            (sync.sync_rows)();
            let initial_ms = start.elapsed().as_secs_f64() * 1_000.0;

            let mut no_op_samples = Vec::with_capacity(5);
            for _ in 0..5 {
                let start = Instant::now();
                (sync.sync_rows)();
                no_op_samples.push(start.elapsed().as_secs_f64() * 1_000.0);
            }

            {
                let mut workspace = ctrl.borrow_mut();
                let files = workspace.ws.active_files_mut().unwrap();
                files.entries[count - 1].size = Some(u64::MAX);
                let changed_path = files.entries[count - 1].path.clone();
                files.entries_rows_changed([changed_path]);
            }
            let start = Instant::now();
            (sync.sync_rows)();
            let one_row_update_ms = start.elapsed().as_secs_f64() * 1_000.0;

            {
                let mut workspace = ctrl.borrow_mut();
                let files = workspace.ws.active_files_mut().unwrap();
                files.entries.push(Entry {
                    name: format!("entry-{count:06}.dat"),
                    path: root.join(format!("entry-{count:06}.dat")),
                    kind: EntryKind::File,
                    size: Some(count as u64),
                    modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(count as u64)),
                    created: None,
                    hidden: false,
                    system: false,
                });
                files.entries_appended(count);
            }
            let start = Instant::now();
            (sync.sync_rows)();
            let append_ms = start.elapsed().as_secs_f64() * 1_000.0;

            // Pide un frame explícito en cada pasada para separar rasterizado software del
            // tiempo de sincronización de modelos medido arriba. El buffer/tamaño y la ventana
            // de prueba son iguales para 10k y 100k; la UI virtualiza las filas fuera del viewport.
            let mut render_samples = Vec::with_capacity(5);
            for _ in 0..5 {
                slint::platform::update_timers_and_animations();
                window.request_redraw();
                let start = Instant::now();
                window.draw_if_needed(|renderer| {
                    renderer.render(&mut pixels, 1280);
                });
                render_samples.push(start.elapsed().as_secs_f64() * 1_000.0);
            }
            let render_median_ms = median_ms(render_samples);

            println!(
                "W-07/W-10C software rows={count} initial_ms={initial_ms:.3} no_op_median_ms={:.3} one_row_update_ms={one_row_update_ms:.3} append_ms={append_ms:.3} render_median_ms={render_median_ms:.3}",
                median_ms(no_op_samples),
            );
        }
        ui.hide().unwrap();
    }
}
