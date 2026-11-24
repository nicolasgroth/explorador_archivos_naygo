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
use crate::persistent_models::{model_rc, preview_vm_eq, update_rows, PersistentModels};
use crate::rows_model::{diff_keys, RowsModel, STALE_CONTENT};
use crate::vm_builders::*;
use crate::win_helpers::*;
use crate::workspace_ctrl::WorkspaceCtrl;
use crate::*;
use naygo_core::workspace::layout::{Rect, SplitDir};
use naygo_core::workspace::{PaneId, PanePurpose};
use slint::{Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
    /// Firma del conjunto de rutas frescas con que se construyeron las filas. Los deltas
    /// (alta al final, filas modificadas) solo reconstruyen sus filas: si además empezó o venció
    /// un resaltado, otras filas cambian y hay que pasar por la reconciliación por claves.
    fresh: u64,
}

/// Instantánea pequeña de presentación; selecciones grandes usan la reconciliación completa.
#[derive(Clone)]
struct DeepPresentation {
    context: u64,
    focused: Option<std::path::PathBuf>,
    selected: HashSet<std::path::PathBuf>,
    row_count: usize,
}

fn deep_presentation(
    ctrl: &WorkspaceCtrl,
    id: PaneId,
    now: std::time::Instant,
) -> Option<DeepPresentation> {
    let job = ctrl.deep_job.as_ref().filter(|job| job.pane == id)?;
    if job.selected_paths.len() > 32 {
        return None;
    }
    Some(DeepPresentation {
        context: ctrl.deep_rows_context_signature(id, now)?,
        focused: job.focused_path.clone(),
        selected: job.selected_paths.clone(),
        row_count: 0,
    })
}

/// Solo actualiza marcas sobre el modelo ya aplicado; cualquier duda vuelve al camino completo.
fn apply_deep_presentation(
    ctrl: &WorkspaceCtrl,
    id: PaneId,
    model: &RowsModel,
    old: &DeepPresentation,
    new: &DeepPresentation,
) -> bool {
    if old.context != new.context || old.row_count != model.row_count() {
        return false;
    }
    let mut changed: HashSet<&std::path::PathBuf> =
        old.selected.symmetric_difference(&new.selected).collect();
    if old.focused != new.focused {
        changed.extend(old.focused.iter());
        changed.extend(new.focused.iter());
    }
    if changed.is_empty() {
        return true;
    }
    // Para cambios amplios conviene la reconciliación completa; para pocos elementos, comparar
    // rutas evita calcular un hash de cada entrada de un listado grande.
    if changed.len() > 8 {
        return false;
    }
    let changed: Vec<_> = changed.into_iter().collect();
    let Some(job) = ctrl.deep_job.as_ref().filter(|job| job.pane == id) else {
        return false;
    };
    let vis = ctrl.visibility_flags();
    let mut updates = Vec::new();
    for (position, (entry, _)) in job
        .items
        .iter()
        .filter(|(entry, _)| {
            naygo_core::filter::is_visible(
                entry,
                vis.show_hidden,
                vis.show_system,
                vis.hide_dotfiles,
            )
        })
        .enumerate()
    {
        if changed.iter().any(|path| **path == entry.path) {
            let Some(mut row) = model.row_data(position) else {
                return false;
            };
            row.selected = new.selected.contains(&entry.path);
            row.focused = new.focused.as_ref() == Some(&entry.path);
            updates.push((position, row));
        }
    }
    for (position, row) in updates {
        model.set_row(position, row);
    }
    true
}

fn applied_rows_source(
    ctrl: &WorkspaceCtrl,
    id: PaneId,
    context: u64,
    fresh: u64,
) -> Option<AppliedRowsSource> {
    Some(AppliedRowsSource {
        context,
        revision: ctrl.entries_revision_of(id)?,
        entry_count: ctrl.row_entry_count(id)?,
        view_indices: ctrl.row_view_indices_shared(id)?,
        fresh,
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

/// Compara el contenido visible de una fila. Slint representa `cells` como un modelo anidado;
/// cada conversión crea un `VecModel` nuevo y comparar su `ModelRc` por identidad marcaba todas
/// las filas como distintas aunque las celdas no hubieran cambiado. Eso disparaba 100k
/// notificaciones al aplicar una sola alta/modificación en carpetas grandes.
fn row_data_eq(left: &RowData, right: &RowData) -> bool {
    left.name == right.name
        && left.is_dir == right.is_dir
        && left.drop_folder == right.drop_folder
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
        let deep_presentations = Rc::new(RefCell::new(HashMap::<u64, DeepPresentation>::new()));
        let deferred_rename_rows = Rc::new(RefCell::new(HashSet::<u64>::new()));
        // Modelos de nivel ventana creados UNA vez: reemplazarlos en cada tick obligaba a Slint a
        // re-crear delegates y repintar overlays de ventana completa (ver `persistent_models`).
        let window_models = RefCell::new(PersistentModels::new());
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
            // El cierre del panel invalida la sesión en el controlador. Retirar el editor
            // huérfano antes de prestar `ctrl` evita que su callback tardío choque con el sync
            // y permite iniciar otra edición normalmente.
            let stale_rename = ui.get_rename_pane() >= 0 && ctrl.borrow().rename_active.is_none();
            if stale_rename {
                ui.set_rename_pane(-1);
                ui.set_rename_pos(-1);
            }
            // La fila editada puede haber diferido una actualización de metadatos. Al
            // terminar o cambiar de sesión, invalidar su firma una vez para reconciliarla.
            let active_rename_pane = ctrl.borrow().rename_active.as_ref().map(|r| r.pane.0);
            let finished: Vec<u64> = {
                let mut pending = deferred_rename_rows.borrow_mut();
                let done: Vec<_> = pending
                    .iter()
                    .copied()
                    .filter(|id| Some(*id) != active_rename_pane)
                    .collect();
                pending.retain(|id| Some(*id) == active_rename_pane);
                done
            };
            for id in finished {
                last_row_sig.borrow_mut().remove(&id);
                row_sources.borrow_mut().remove(&id);
                deep_presentations.borrow_mut().remove(&id);
            }
            // El índice es presentación, no identidad. Antes de mutar el modelo, resolver
            // la ruta anclada en la vista actual. Si un listado parcial aún no la contiene,
            // conservar las filas existentes hasta que aparezca o termine la edición.
            let rename_view = {
                let c = ctrl.borrow();
                let m = models.borrow();
                c.rename_active.as_ref().and_then(|rename| {
                    if ui.get_rename_pane() != rename.pane.0 as i32
                        || !m.pane_ids.contains(&rename.pane)
                    {
                        return None;
                    }
                    let displayed_count = m.per_pane.get(&rename.pane)?.rows.row_count();
                    if c.is_deep_active(rename.pane) {
                        return Some((
                            rename.pane,
                            rename.pos,
                            c.deep_position_of(rename.pane, &rename.source),
                            c.deep_visible_count(rename.pane),
                            displayed_count,
                        ));
                    }
                    let files = c.ws.pane(rename.pane)?.files.as_ref()?;
                    let pos = if files
                        .view_entry_at(rename.pos)
                        .is_some_and(|entry| entry.path == rename.source)
                    {
                        Some(rename.pos)
                    } else {
                        files.view_indices_shared().iter().position(|&index| {
                            files
                                .entries
                                .get(index)
                                .is_some_and(|entry| entry.path == rename.source)
                        })
                    };
                    Some((
                        rename.pane,
                        rename.pos,
                        pos,
                        files.view_len(),
                        displayed_count,
                    ))
                })
            };
            let mut hold_rename_rows = None;
            if let Some((pane, old_pos, pos, current_count, displayed_count)) = rename_view {
                if let Some(pos) = pos {
                    if pos != old_pos || current_count != displayed_count {
                        // Invalidar el editor anterior antes de que Slint lo desmonte; el
                        // guard de UI también evita callbacks mientras `ctrl` está prestado.
                        let session = ctrl.borrow_mut().reposition_rename(pos);
                        if let Some(session) = session {
                            ui.set_rename_repositioning(true);
                            ui.set_rename_session(session as i32);
                            ui.set_rename_pos(pos as i32);
                        }
                    }
                } else {
                    hold_rename_rows = Some(pane);
                }
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
            let mut wm = window_models.borrow_mut();
            update_rows(&wm.inspector_meta, meta_fields(&c));
            inspector.meta = model_rc(&wm.inspector_meta);
            inspector.meta_loading = c.meta_loading();
            inspector.has_provenance = c.has_provenance();

            // Props a nivel de ventana para el menú ▾ de favoritos del toolbar (el árbol jerárquico)
            // y para el submenú "Mover a…" del panel (lista de grupos destino).
            update_rows(&wm.fav_tree, fav_tree.clone());
            ui.set_fav_tree(model_rc(&wm.fav_tree));
            update_rows(&wm.fav_group_options, fav_group_options);
            ui.set_fav_group_options(model_rc(&wm.fav_group_options));

            let mut m = models.borrow_mut();
            // La vista previa es la misma para todos los paneles Preview: se arma una vez.
            let preview_vm = c
                .has_purpose(PanePurpose::Preview)
                .then(|| m.preview_vm.vm(&c));
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
                        let rows_unchanged = if hold_rename_rows == Some(id) {
                            sigs.remove(&id.0);
                            row_sources.borrow_mut().remove(&id.0);
                            deep_presentations.borrow_mut().remove(&id.0);
                            true
                        } else {
                            match sig {
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
                            let next_deep = if c
                                .rename_active
                                .as_ref()
                                .is_none_or(|rename| rename.pane != id)
                            {
                                deep_presentation(&c, id, hl_now)
                            } else {
                                None
                            };
                            let deep_updated = next_deep.as_ref().is_some_and(|next| {
                                deep_presentations.borrow().get(&id.0).is_some_and(|old| {
                                    apply_deep_presentation(&c, id, &pm.rows, old, next)
                                })
                            });
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
                            let current_fresh = c.fresh_signature_of(id, hl_secs, hl_now);
                            let previous_source = match (row_context, current_revision) {
                                (Some(context), Some(_)) => row_sources
                                    .borrow()
                                    .get(&id.0)
                                    .filter(|source| {
                                        source.context == context
                                            && source.fresh == current_fresh
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
                            // distinto cae en la reconciliación por claves.
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
                                Some((view, positions, plain_rows))
                            });
                            let changed_positions = previous_revision.and_then(|revision| {
                                c.dirty_row_positions(id, revision)
                                    .filter(|positions| positions.len() <= pm.rows.row_count() / 4)
                            });
                            // Fila bajo edición inline (rename) en este panel, si la hay. Un
                            // `set_row` sobre ella RE-MONTA el delegate y destruye el `LineEdit`
                            // (se llevaba el texto tipeado: bug del "_" que borraba todo). Se
                            // congela mientras se edita (el editor la tapa) y se reconcilia al
                            // terminar vía `deferred_rename_rows`.
                            let rename_row = c
                                .rename_active
                                .as_ref()
                                .filter(|r| r.pane == id)
                                .map(|r| r.pos);
                            let keys_known = pm.rows.keys().is_some();

                            if deep_updated {
                                // Ya se actualizaron solo las marcas afectadas.
                            } else if let Some((view, positions, plain_rows)) = appended_rows {
                                let keys = if keys_known {
                                    c.row_keys(id, Some(&positions), hl_secs, hl_now)
                                } else {
                                    None
                                };
                                let rows: Vec<RowData> = {
                                    let _metric = crate::metrics::span("ui.rows_convert");
                                    plain_rows.into_iter().map(to_row_data).collect()
                                };
                                {
                                    let _metric = crate::metrics::span("ui.rows_model_apply");
                                    // Un solo aviso `row_added(i, n)` para todo el lote.
                                    pm.rows.append(rows, keys);
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
                                                fresh: current_fresh,
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
                                    let keys = if keys_known {
                                        c.row_keys(id, Some(positions), hl_secs, hl_now)
                                    } else {
                                        None
                                    };
                                    if let Some(plain_rows) = plain_rows {
                                        rows_built = true;
                                        let rows: Vec<RowData> = {
                                            let _metric = crate::metrics::span("ui.rows_convert");
                                            plain_rows.into_iter().map(to_row_data).collect()
                                        };
                                        let _metric = crate::metrics::span("ui.rows_model_apply");
                                        for (k, (position, row)) in
                                            positions.iter().copied().zip(rows).enumerate()
                                        {
                                            let key = keys.as_ref().and_then(|keys| keys.get(k));
                                            if rename_row == Some(position) {
                                                if let Some(key) = key {
                                                    pm.rows.set_key(
                                                        position,
                                                        crate::rows_model::RowKey {
                                                            id: key.id,
                                                            content: STALE_CONTENT,
                                                        },
                                                    );
                                                }
                                                deferred_rename_rows.borrow_mut().insert(id.0);
                                                continue;
                                            }
                                            if pm
                                                .rows
                                                .with_row(position, |old| !row_data_eq(old, &row))
                                                .unwrap_or(false)
                                            {
                                                pm.rows.set_row(position, row);
                                            }
                                            if let Some(key) = key {
                                                pm.rows.set_key(position, *key);
                                            }
                                        }
                                        if keys_known && keys.is_none() {
                                            pm.rows.invalidate_keys();
                                        }
                                    }
                                }
                                if rows_built {
                                    // El estado se mantiene bajo el préstamo de `ctrl` durante la
                                    // traducción y la conversión; se puede avanzar la revisión.
                                    if current_revision.is_some() {
                                        if let Some(context) = row_context {
                                            if let Some(source) =
                                                applied_rows_source(&c, id, context, current_fresh)
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
                                // Reconciliación por claves: identidad + contenido de cada fila
                                // (enteros, sin cadenas). Solo se construyen (con su `VecModel` de
                                // celdas) las filas que entran o cambian; altas y bajas se avisan
                                // por tramo. Con 100k filas un alta en el medio, el vencimiento de
                                // un resaltado o un cambio de selección cuestan O(n) en enteros +
                                // O(filas cambiadas) en construcción, en vez de construir y
                                // comparar las n filas.
                                let applied_by_keys = 'keys: {
                                    if !keys_known {
                                        break 'keys false;
                                    }
                                    let Some(new_keys) = ({
                                        let _metric = crate::metrics::span("ui.rows_keys");
                                        c.row_keys(id, None, hl_secs, hl_now)
                                    }) else {
                                        break 'keys false;
                                    };
                                    let diff = {
                                        let old = pm.rows.keys();
                                        let Some(old) = old.as_ref() else {
                                            break 'keys false;
                                        };
                                        diff_keys(old, &new_keys)
                                    };
                                    if diff.is_noop() {
                                        // Mismas claves: nada que avisar (p. ej. vencido un
                                        // resaltado de una ruta que no está en la vista).
                                        break 'keys true;
                                    }
                                    let mut positions = diff.positions_to_build();
                                    if let Some(rename_pos) = rename_row {
                                        // La fila editada conserva su identidad (misma ruta) si
                                        // figura entre los cambios: no se reconstruye.
                                        if diff.updates.contains(&rename_pos) {
                                            positions.retain(|&p| p != rename_pos);
                                            deferred_rename_rows.borrow_mut().insert(id.0);
                                        }
                                    }
                                    let built: Vec<(usize, RowData)> = if positions.is_empty() {
                                        Vec::new()
                                    } else {
                                        let Some(plain_rows) = ({
                                            let _metric = crate::metrics::span("ui.rows_build");
                                            c.rows_at(id, &positions, hl_secs, hl_now)
                                        }) else {
                                            break 'keys false;
                                        };
                                        if plain_rows.len() != positions.len() {
                                            break 'keys false;
                                        }
                                        let _metric = crate::metrics::span("ui.rows_convert");
                                        positions
                                            .iter()
                                            .copied()
                                            .zip(plain_rows.into_iter().map(to_row_data))
                                            .collect()
                                    };
                                    let _metric = crate::metrics::span("ui.rows_model_apply");
                                    pm.rows.apply_diff(&diff, built, new_keys);
                                    true
                                };
                                if !applied_by_keys {
                                    let plain_rows = {
                                        let _metric = crate::metrics::span("ui.rows_build");
                                        c.rows_of(id, hl_secs, hl_now)
                                    };
                                    let rows: Vec<RowData> = {
                                        let _metric = crate::metrics::span("ui.rows_convert");
                                        plain_rows.into_iter().map(to_row_data).collect()
                                    };
                                    // Actualización POR FILA cuando el largo no cambió. `set_vec`
                                    // resetea el modelo ENTERO y el ListView re-crea todas sus
                                    // filas; en una carpeta "en movimiento" (Dropbox) la cadena de
                                    // hover del puntero se invalidaba y la RUEDA dejaba de llegar al
                                    // panel. Con `set_row` solo se notifican las filas que de
                                    // verdad cambiaron. Si el LARGO cambió, se reconcilia por tramo
                                    // (prefijo/sufijo intactos, avisos `row_added/removed(i, n)`).
                                    let _metric = crate::metrics::span("ui.rows_model_apply");
                                    let mut frozen = None;
                                    if pm.rows.row_count() == rows.len() {
                                        for (i, row) in rows.into_iter().enumerate() {
                                            // Solo congelar si sigue siendo la misma fila: tras
                                            // un reordenamiento con igual longitud, la posición
                                            // nueva puede contener todavía otro nombre.
                                            if rename_row == Some(i)
                                                && pm
                                                    .rows
                                                    .with_row(i, |old| old.name == row.name)
                                                    .unwrap_or(false)
                                            {
                                                if !pm
                                                    .rows
                                                    .with_row(i, |old| row_data_eq(old, &row))
                                                    .unwrap_or(true)
                                                {
                                                    deferred_rename_rows.borrow_mut().insert(id.0);
                                                }
                                                frozen = Some(i);
                                                continue;
                                            }
                                            if pm
                                                .rows
                                                .with_row(i, |old| !row_data_eq(old, &row))
                                                .unwrap_or(false)
                                            {
                                                pm.rows.set_row(i, row);
                                            }
                                        }
                                    } else {
                                        pm.rows.reconcile_by(rows, row_data_eq);
                                    }
                                    // Claves del estado recién aplicado (bajo el mismo préstamo de
                                    // `ctrl`): habilitan la reconciliación por claves desde el
                                    // próximo cambio. `None` en vista profunda o con filtro.
                                    let keys = {
                                        let _metric = crate::metrics::span("ui.rows_keys");
                                        c.row_keys(id, None, hl_secs, hl_now)
                                    };
                                    pm.rows.set_keys(keys);
                                    if let Some(i) = frozen {
                                        // Leer la clave ANTES de ramificar: en edición 2021 el `Ref`
                                        // de `keys()` en la condición de un `if let` vive todo el
                                        // cuerpo y el `borrow_mut` de `set_key` reventaría.
                                        let id_key = pm
                                            .rows
                                            .keys()
                                            .as_ref()
                                            .and_then(|keys| keys.get(i).map(|k| k.id));
                                        if let Some(id_key) = id_key {
                                            pm.rows.set_key(
                                                i,
                                                crate::rows_model::RowKey {
                                                    id: id_key,
                                                    content: STALE_CONTENT,
                                                },
                                            );
                                        }
                                    }
                                }
                                if let (Some(context), Some(count)) = (row_context, row_count) {
                                    if count == pm.rows.row_count() {
                                        if let Some(source) =
                                            applied_rows_source(&c, id, context, current_fresh)
                                        {
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
                            if let Some(mut next) = next_deep {
                                next.row_count = pm.rows.row_count();
                                deep_presentations.borrow_mut().insert(id.0, next);
                            } else {
                                deep_presentations.borrow_mut().remove(&id.0);
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
                        m.models_for(id).tree.reconcile(rows);
                    }
                    // Favoritos, recientes e historial: antes `set_vec` en cada tick (re-creaba
                    // todas las filas del panel visible y mataba el hover); ahora solo las filas
                    // que cambiaron.
                    Some(PanePurpose::Favorites) => {
                        update_rows(&m.models_for(id).favs, favs.clone());
                        update_rows(&m.models_for(id).fav_tree, fav_tree.clone());
                    }
                    Some(PanePurpose::Recents) => {
                        update_rows(&m.models_for(id).recents, recents.clone());
                        update_rows(&m.models_for(id).frequent_dirs, frequent_dirs.clone());
                    }
                    Some(PanePurpose::History) => {
                        update_rows(&m.models_for(id).hist, hist.clone());
                    }
                    Some(PanePurpose::Basket) => {
                        // set_vec recreaba los TouchArea incluso al activar el panel:
                        // el mouse-up ya no encontraba el mouse-down de esa misma fila.
                        update_rows(&m.models_for(id).basket, c.basket_rows());
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
                    // Antes se marcaban como cambiados en CADA tick (con un modelo de metadata
                    // nuevo): `set_row_data` del panel y repintado completo de Propiedades y
                    // Vista previa cada 30 ms. Ahora solo si el VM cambió de verdad.
                    if purpose == Some(PanePurpose::Inspector) && pv.inspector != inspector {
                        pv.inspector = inspector.clone();
                        changed = true;
                    }
                    if let (Some(PanePurpose::Preview), Some(preview)) =
                        (purpose, preview_vm.as_ref())
                    {
                        if !preview_vm_eq(&pv.preview, preview) {
                            pv.preview = preview.clone();
                            changed = true;
                        }
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
            // El asistente de sincronización trae hasta 5000 filas: se arman solo cuando cambia
            // su huella (plan, marcas o idioma) y se vuelcan en el modelo persistente, en vez de
            // construirlas y compararlas en cada tick con el modal abierto.
            let mut sync_vm = c.sync_vm_header();
            let sync_fingerprint = c.sync_rows_fingerprint();
            if wm.sync_rows_fingerprint != sync_fingerprint {
                update_rows(&wm.sync_rows, c.sync_rows_vm());
                wm.sync_rows_fingerprint = sync_fingerprint;
            }
            sync_vm.rows = model_rc(&wm.sync_rows);
            ui.set_sync_vm(sync_vm);
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
                let points_model = &wm.points;
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
                ui.set_points_rows(model_rc(points_model));
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
            update_rows(&wm.spaces_recents, recents);
            ui.set_spaces_recents(model_rc(&wm.spaces_recents));
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
            update_rows(&wm.op_running, running);
            update_rows(&wm.op_queued, queued);
            update_rows(&wm.op_history, history);
            update_rows(&wm.op_planning, planning);
            ui.set_op_running_rows(model_rc(&wm.op_running));
            ui.set_op_queued_rows(model_rc(&wm.op_queued));
            ui.set_op_history_rows(model_rc(&wm.op_history));
            ui.set_op_planning_rows(model_rc(&wm.op_planning));
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
            update_rows(&wm.resume, resume_rows);
            ui.set_resume_rows(model_rc(&wm.resume));
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
            update_rows(&wm.clipboard_history, clipboard_rows);
            ui.set_clipboard_history_rows(model_rc(&wm.clipboard_history));
            // Terminales opcionales: el sondeo del PATH corre en un worker al arrancar; aquí solo se
            // lee el resultado cacheado (sin I/O en el hilo de UI).
            ui.set_has_wt(c.windows_terminal_available());
            ui.set_has_wsl(c.wsl_available());
            // Menú contextual: posición + si hay menú nativo disponible (hay HWND).
            let ctx = match &c.context_menu {
                Some(cm) if c.active_id() == Some(cm.pane) => ContextMenuVm {
                    active: true,
                    deep_mode: c.active_id().is_some_and(|id| c.is_deep_active(id)),
                    single_target: cm.targets.len() == 1,
                    x: cm.x,
                    y: cm.y,
                    has_native: naygo_hwnd(&ui).is_some(),
                    has_wt: c.windows_terminal_available(),
                    folder_mode: cm.folder_mode,
                    is_single_zip: c.sel_is_single_zip(),
                    has_selection: if c.active_id().is_some_and(|id| c.is_deep_active(id)) {
                        !cm.targets.is_empty()
                    } else {
                        !c.selected_paths().is_empty()
                    },
                    // Carpeta objetivo (habilita el submenú "Abrir ▸"): flag YA cacheado al abrir
                    // el menú (evita un `stat` por tick, costoso en shares de red lentos).
                    target_is_folder: cm.target_is_folder,
                    show_open_here: cm.show_open_here,
                    target_is_executable: cm.targets.len() == 1
                        && cm
                            .targets
                            .first()
                            .is_some_and(|p| naygo_platform::open::can_run_as_administrator(p)),
                },
                _ => ContextMenuVm {
                    active: false,
                    deep_mode: false,
                    single_target: false,
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
                let search_model = &wm.search_hits;
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
                    hits: model_rc(search_model),
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
                        exts: {
                            update_rows(&wm.column_exts, exts);
                            model_rc(&wm.column_exts)
                        },
                    }
                }
                // `ColumnMenu` es un overlay de ventana completa: con un modelo `exts` nuevo en
                // cada tick su VM cambiaba y el renderer repintaba toda la ventana.
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
                    exts: {
                        update_rows(&wm.column_exts, Vec::new());
                        model_rc(&wm.column_exts)
                    },
                },
            };
            ui.set_column_menu(colmenu);

            // Ventana de renombrado por lotes (F5): espejo del estado + preview en vivo.
            let batch = match &c.batch {
                Some(b) => {
                    use naygo_core::batch_rename::{CaseTransform, RowStatus};
                    let rows_src = c.batch_preview();
                    let first_parent = b
                        .items
                        .first()
                        .and_then(|item| item.path.parent())
                        .map(naygo_core::path_identity::key);
                    let show_parent = b.items.iter().any(|item| {
                        item.path.parent().map(naygo_core::path_identity::key) != first_parent
                    });
                    let rows: Vec<BatchRowVm> = rows_src
                        .iter()
                        .map(|r| BatchRowVm {
                            old_name: SharedString::from(r.old_name.as_str()),
                            new_name: SharedString::from(r.new_name.as_str()),
                            parent: if show_parent {
                                let parent = r.path.parent().unwrap_or(&b.base_dir);
                                let relative = parent.strip_prefix(&b.base_dir).unwrap_or(parent);
                                SharedString::from(if relative.as_os_str().is_empty() {
                                    ".".to_string()
                                } else {
                                    relative.display().to_string()
                                })
                            } else {
                                SharedString::new()
                            },
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
                        show_parent,
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
                        rows: {
                            update_rows(&wm.batch_rows, rows);
                            model_rc(&wm.batch_rows)
                        },
                    }
                }
                None => BatchRenameVm {
                    active: false,
                    show_parent: false,
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
                    // Overlay de ventana completa, igual que `ColumnMenu`: mismo modelo siempre.
                    rows: {
                        update_rows(&wm.batch_rows, Vec::new());
                        model_rc(&wm.batch_rows)
                    },
                },
            };
            ui.set_batch(batch);

            // Ayuda (F1): estado + atajos activos (leídos del keymap en vivo).
            // La lista solo se ve con la ayuda abierta: se arma al abrirla o al cambiar el idioma,
            // no en cada tick (traducir todo el keymap cada 30 ms era trabajo perdido).
            ui.set_help_open(c.help_open);
            if wm.help_needs_rebuild(c.help_open, &c.config.settings.language) {
                let help_rows: Vec<HelpRowVm> = c
                    .help_shortcuts()
                    .into_iter()
                    .map(|(label, chord)| HelpRowVm {
                        label: SharedString::from(label.as_str()),
                        chord: SharedString::from(chord.as_str()),
                    })
                    .collect();
                update_rows(&wm.help, help_rows);
            }
            ui.set_help_shortcuts(model_rc(&wm.help));
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
                set_vec_if_changed(&m.splits, splits);

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
                set_vec_if_changed(&m.splits, splits);
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
            set_vec_if_changed(&m.picks, picks);
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
            set_vec_if_changed(&m.destination_radar, radar_rows);

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

    #[test]
    #[ignore = "medición sintética manual; no es una prueba de rendimiento en CI"]
    fn benchmark_marcas_parciales_vista_profunda() {
        use naygo_core::fs_model::{Entry, EntryKind};
        use naygo_core::keymap::Action;
        let root = tempfile::tempdir().unwrap();
        let cfg = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
        let id = c.active_id().unwrap();
        c.deep_start(id);
        let job = c.deep_job.as_mut().unwrap();
        *job.items = (0..100_000)
            .map(|i| {
                let name = format!("archivo-{i:06}.txt");
                (
                    Entry {
                        path: root.path().join(&name),
                        name,
                        kind: EntryKind::File,
                        size: Some(64),
                        modified: None,
                        created: None,
                        hidden: false,
                        system: false,
                    },
                    0,
                )
            })
            .collect();
        let path = job.items[99_950].0.path.clone();
        job.focused_path = Some(path.clone());
        job.selected_paths.insert(path);
        job.selection_revision += 1;
        let rows = c
            .rows_of(id, 0, std::time::Instant::now())
            .into_iter()
            .map(to_row_data)
            .collect::<Vec<_>>();
        let model = RowsModel::default();
        model.append(rows, None);
        let mut previous = deep_presentation(&c, id, std::time::Instant::now()).unwrap();
        previous.row_count = model.row_count();
        let mut samples = Vec::new();
        for _ in 0..21 {
            assert!(c.run_action(Action::MoveDown));
            let mut next = deep_presentation(&c, id, std::time::Instant::now()).unwrap();
            next.row_count = model.row_count();
            let t = std::time::Instant::now();
            assert!(apply_deep_presentation(&c, id, &model, &previous, &next));
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
            previous = next;
        }
        samples.remove(0);
        samples.sort_by(f64::total_cmp);
        println!("deep-partial n=100000 apply_ms={samples:?}");
        assert_eq!(model.iter().filter(|row| row.selected).count(), 1);
    }

    #[test]
    fn conflict_during_inline_rename_survives_real_sync_and_controller() {
        use naygo_core::fs_model::{Entry, EntryKind};
        use naygo_core::ops::ConflictAction;
        use naygo_core::workspace::layout::Rect;
        use slint::platform::{
            software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
            Key, Platform, WindowAdapter, WindowEvent,
        };
        use std::time::{Duration, SystemTime};

        struct Headless(Rc<MinimalSoftwareWindow>);
        impl Platform for Headless {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                Ok(self.0.clone())
            }
        }
        fn pump(adapter: &MinimalSoftwareWindow) {
            for _ in 0..5 {
                slint::platform::update_timers_and_animations();
                adapter.draw_if_needed(|renderer| {
                    let mut pixels = vec![slint::Rgb8Pixel::default(); 1280 * 800];
                    renderer.render(&mut pixels, 1280);
                });
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        std::fs::create_dir(&root).unwrap();
        let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Headless(adapter.clone()))).unwrap();
        let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new_in(
            root.clone(),
            temp.path().join("settings"),
        )));
        let models = Rc::new(RefCell::new(Models::new()));
        let ui = AppWindow::new().unwrap();
        ui.set_panes(ModelRc::from(models.borrow().panes.clone()));
        ui.set_splits(ModelRc::from(models.borrow().splits.clone()));
        ui.set_settings_vm(crate::vm_builders::build_settings_vm(&ctrl.borrow().config));
        let area_of: Rc<dyn Fn() -> Rect> = Rc::new(|| Rect {
            x: 0.0,
            y: 0.0,
            w: 1280.0,
            h: 800.0,
        });
        let sync = build_sync(
            ui.as_weak(),
            ctrl.clone(),
            models.clone(),
            Rc::new(RefCell::new(HashMap::new())),
            area_of.clone(),
        );
        let ctx = crate::wire::WireCtx {
            ctrl: ctrl.clone(),
            sync_rows: sync.sync_rows.clone(),
            sync_layout: sync.sync_layout.clone(),
            start_timer: Rc::new(|| {}),
            area_of,
            palette_cmds: Rc::new(RefCell::new(Vec::new())),
            palette_cmd_indices: Rc::new(RefCell::new(Vec::new())),
            pending_eject: Rc::new(RefCell::new(None)),
            pending_eject_panes: Rc::new(RefCell::new(Vec::new())),
            pending_undo: Rc::new(RefCell::new(None)),
        };
        crate::callbacks_rename::wire_rename(&ui, &ctx);
        (sync.sync_layout)();
        let id = ctrl.borrow().active_id().unwrap();
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.active_files_mut().unwrap();
            files.entries = vec![Entry {
                name: "file.txt".into(),
                path: root.join("file.txt"),
                kind: EntryKind::File,
                size: Some(1),
                modified: Some(SystemTime::UNIX_EPOCH),
                created: None,
                hidden: false,
                system: false,
            }];
            files.entries_changed();
            files.select_single(0);
        }
        (sync.sync_rows)();
        ui.show().unwrap();
        adapter.set_size(slint::PhysicalSize::new(1280, 800));
        pump(&adapter);
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            let request = c.take_rename_request().unwrap();
            crate::vm_builders::show_rename_editor(&ui, &request);
        }
        pump(&adapter);
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "x.txt");
        // F2 cicla la selección, pero no puede reemplazar el borrador por el nombre original.
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::F2.into(),
        });
        pump(&adapter);
        assert_eq!(
            ui.get_rename_draft().as_str(),
            "x.txt",
            "F2 conserva el nombre editado"
        );
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "log".into() });
        pump(&adapter);
        assert_eq!(
            ui.get_rename_draft().as_str(),
            "x.log",
            "F2 selecciona la extensión del borrador"
        );
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Home.into(),
        });
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::RightArrow.into(),
        });
        pump(&adapter);
        // Ejecutar el worker real sobre datos temporales, mientras el editor está activo.
        let source_dir = temp.path().join("worker-source");
        let dest_dir = temp.path().join("worker-dest");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::create_dir(&dest_dir).unwrap();
        std::fs::write(source_dir.join("conflict.txt"), b"incoming").unwrap();
        std::fs::write(dest_dir.join("conflict.txt"), b"existing").unwrap();
        ctrl.borrow_mut().ops.start_op(
            naygo_core::ops::transfer(
                false,
                vec![source_dir.join("conflict.txt")],
                dest_dir.clone(),
            ),
            "test".into(),
            false,
        );
        let mut conflict_id = None;
        for _ in 0..5000 {
            let mut c = ctrl.borrow_mut();
            c.ops.pump_ops();
            if let Some(crate::ops_ctrl::OpDialog::Conflict { op_id, .. }) =
                c.ops.pending_dialog.as_ref()
            {
                conflict_id = Some(*op_id);
                break;
            }
            drop(c);
            std::thread::sleep(Duration::from_millis(1));
        }
        let conflict_id = conflict_id.expect("el worker debe producir un conflicto real");
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(
            ui.get_op_dialog().kind,
            2,
            "el conflicto del worker llega a la ventana"
        );
        assert!(
            ctrl.borrow().rename_active.is_some(),
            "el conflicto no confirma la sesión"
        );
        assert_eq!(ui.get_rename_draft().as_str(), "x.log");
        ctrl.borrow_mut()
            .ops
            .resolve_conflict(conflict_id, ConflictAction::Skip, false);
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(ui.get_op_dialog().kind, 0);
        for _ in 0..5000 {
            let mut c = ctrl.borrow_mut();
            c.ops.pump_ops();
            if c.ops.active_ops.iter().all(|op| op.summary.is_some()) {
                break;
            }
            drop(c);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            ctrl.borrow()
                .ops
                .active_ops
                .iter()
                .all(|op| op.summary.is_some()),
            "el worker debe terminar tras omitir el conflicto"
        );
        assert_eq!(
            std::fs::read(dest_dir.join("conflict.txt")).unwrap(),
            b"existing"
        );
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "y".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "xy.log");
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        pump(&adapter);
        assert!(
            ctrl.borrow().rename_active.is_none(),
            "Escape cancela la sesión real"
        );
        assert_eq!(ui.get_rename_pane(), -1);
        assert!(ctrl.borrow().ws.pane(id).is_some());

        // Un cierre desde el controlador invalida primero la sesión; el sync debe retirar
        // las propiedades de UI sin que el callback de pérdida de foco vuelva a prestar ctrl.
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            let request = c.take_rename_request().unwrap();
            crate::vm_builders::show_rename_editor(&ui, &request);
        }
        pump(&adapter);
        ctrl.borrow_mut().rename_cancel();
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(ui.get_rename_pane(), -1);
        assert_eq!(ui.get_rename_pos(), -1);

        // Un cambio de panel mientras el conflicto tiene el foco conserva F2 suspendido.
        // La ventana real debe llevar las flechas al segundo panel y F2 de vuelta al borrador.
        crate::callbacks_nav::wire_nav_keys(&ui, &ctx);
        let second;
        {
            let mut c = ctrl.borrow_mut();
            c.add_pane_split_dir(naygo_core::workspace::layout::SplitDir::Horizontal, false);
            second = c.active_id().unwrap();
            let files = c.ws.active_files_mut().unwrap();
            files.entries = ["a.txt", "b.txt"]
                .into_iter()
                .map(|name| Entry {
                    name: name.into(),
                    path: root.join(name),
                    kind: EntryKind::File,
                    size: Some(1),
                    modified: Some(SystemTime::UNIX_EPOCH),
                    created: None,
                    hidden: false,
                    system: false,
                })
                .collect();
            files.entries_changed();
            files.select_single(0);
            c.set_active(id);
        }
        (sync.sync_layout)();
        pump(&adapter);
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            let request = c.take_rename_request().unwrap();
            crate::vm_builders::show_rename_editor(&ui, &request);
        }
        pump(&adapter);
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "x.txt");
        ctrl.borrow_mut().ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
            sources: vec![],
            permanent: false,
        });
        (sync.sync_rows)();
        pump(&adapter);
        ctrl.borrow_mut().set_active(second);
        (sync.sync_rows)();
        pump(&adapter);
        ctrl.borrow_mut().ops.pending_dialog = None;
        (sync.sync_rows)();
        pump(&adapter);
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::DownArrow.into(),
        });
        pump(&adapter);
        assert_eq!(ctrl.borrow().ws.active_files().unwrap().focused, Some(1));
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::F2.into(),
        });
        pump(&adapter);
        assert_eq!(ctrl.borrow().active_id(), Some(id));
        assert_eq!(ui.get_rename_draft().as_str(), "x.txt");
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "y".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "xy.txt");
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        pump(&adapter);
        assert!(ctrl.borrow().rename_active.is_none());

        // Al ocultar y volver a crear una pestaña, el estado de selección pertenece a la
        // sesión y no al FilePanel desmontado: no se debe seleccionar el borrador otra vez.
        {
            let mut c = ctrl.borrow_mut();
            c.stack_into(second, id);
            c.set_active_tab(id);
        }
        (sync.sync_layout)();
        pump(&adapter);
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            let request = c.take_rename_request().unwrap();
            crate::vm_builders::show_rename_editor(&ui, &request);
        }
        pump(&adapter);
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "x.txt");
        ctrl.borrow_mut().ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
            sources: vec![],
            permanent: false,
        });
        (sync.sync_rows)();
        pump(&adapter);
        ctrl.borrow_mut().set_active_tab(second);
        (sync.sync_layout)();
        pump(&adapter);
        ctrl.borrow_mut().ops.pending_dialog = None;
        (sync.sync_rows)();
        pump(&adapter);
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::F2.into(),
        });
        pump(&adapter);
        assert_eq!(ctrl.borrow().active_id(), Some(id));
        assert!(ctrl
            .borrow()
            .tab_groups()
            .iter()
            .any(|(members, active)| members.get(*active) == Some(&id)));
        assert_eq!(ui.get_rename_draft().as_str(), "x.txt");
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "y".into() });
        pump(&adapter);
        assert_eq!(
            ui.get_rename_draft().as_str(),
            "x.txty",
            "la selección inicial no reemplaza el borrador al recrear la pestaña"
        );
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        pump(&adapter);
        assert!(ctrl.borrow().rename_active.is_none());

        // Una llegada que se ordena antes del archivo editado no debe mover el editor
        // a esa nueva fila ni perder su borrador.
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            let request = c.take_rename_request().unwrap();
            crate::vm_builders::show_rename_editor(&ui, &request);
        }
        pump(&adapter);
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
        pump(&adapter);
        let old_session = ui.get_rename_session();
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
            files.entries.push(Entry {
                name: "a.txt".into(),
                path: root.join("a.txt"),
                kind: EntryKind::File,
                size: Some(1),
                modified: Some(SystemTime::UNIX_EPOCH),
                created: None,
                hidden: false,
                system: false,
            });
            files.entries_changed();
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(
            ui.get_rename_pos(),
            1,
            "el editor sigue al archivo original"
        );
        assert_eq!(ui.get_rename_draft().as_str(), "x.txt");
        assert_ne!(ui.get_rename_session(), old_session);
        ui.invoke_rename_commit(id.0 as i32, old_session, 0, "wrong.txt".into());
        assert!(
            ctrl.borrow().rename_active.is_some(),
            "un editor viejo no cierra la sesión"
        );
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "y".into() });
        pump(&adapter);
        // El alta se avisa como `row_added` de UNA fila: el delegate del archivo editado
        // sobrevive (solo se desplaza), así que el editor conserva también el cursor (antes se
        // re-creaba y el cursor saltaba al final: "x.txty").
        assert_eq!(ui.get_rename_draft().as_str(), "xy.txt");

        // Durante un listado parcial sin la fuente se conserva la vista previa; al volver
        // la ruta, se reconcilian los cambios y el editor vuelve a seguirla.
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
            files.entries.retain(|entry| entry.name != "file.txt");
            files.entries_changed();
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(
            models.borrow().per_pane.get(&id).unwrap().rows.row_count(),
            2
        );
        assert_eq!(ui.get_rename_pos(), 1);
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "z".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "xyz.txt");
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
            for name in ["b.txt", "file.txt"] {
                files.entries.push(Entry {
                    name: name.into(),
                    path: root.join(name),
                    kind: EntryKind::File,
                    size: Some(1),
                    modified: Some(SystemTime::UNIX_EPOCH),
                    created: None,
                    hidden: false,
                    system: false,
                });
            }
            files.entries_changed();
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(ui.get_rename_pos(), 2);
        assert_eq!(ui.get_rename_draft().as_str(), "xyz.txt");
        assert_eq!(
            models.borrow().per_pane.get(&id).unwrap().rows.row_count(),
            3
        );

        // Igual longitud: el destino del editor tenía otra fila en el modelo anterior.
        // Debe actualizarse la identidad subyacente sin descartar el borrador.
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
            files.entries.retain(|entry| entry.name == "file.txt");
            for name in ["y.txt", "z.txt"] {
                files.entries.push(Entry {
                    name: name.into(),
                    path: root.join(name),
                    kind: EntryKind::File,
                    size: Some(1),
                    modified: Some(SystemTime::UNIX_EPOCH),
                    created: None,
                    hidden: false,
                    system: false,
                });
            }
            files.entries_changed();
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(ui.get_rename_pos(), 0);
        assert_eq!(
            models
                .borrow()
                .per_pane
                .get(&id)
                .unwrap()
                .rows
                .row_data(0)
                .unwrap()
                .name,
            "file.txt"
        );
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "w".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "xyzw.txt");
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
            files.entries.retain(|entry| entry.name == "file.txt");
            for name in ["a.txt", "b.txt"] {
                files.entries.push(Entry {
                    name: name.into(),
                    path: root.join(name),
                    kind: EntryKind::File,
                    size: Some(1),
                    modified: Some(SystemTime::UNIX_EPOCH),
                    created: None,
                    hidden: false,
                    system: false,
                });
            }
            files.entries_changed();
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(ui.get_rename_pos(), 2);
        assert_eq!(
            models
                .borrow()
                .per_pane
                .get(&id)
                .unwrap()
                .rows
                .row_data(2)
                .unwrap()
                .name,
            "file.txt"
        );
        assert_eq!(ui.get_rename_draft().as_str(), "xyzw.txt");
        let cells_before: Vec<String> = models
            .borrow()
            .per_pane
            .get(&id)
            .unwrap()
            .rows
            .row_data(2)
            .unwrap()
            .cells
            .iter()
            .map(|s| s.to_string())
            .collect();
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
            files
                .entries
                .iter_mut()
                .find(|e| e.name == "file.txt")
                .unwrap()
                .size = Some(99999);
            files.entries_changed();
        }
        (sync.sync_rows)();
        pump(&adapter);
        let cells_during: Vec<String> = models
            .borrow()
            .per_pane
            .get(&id)
            .unwrap()
            .rows
            .row_data(2)
            .unwrap()
            .cells
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            cells_during, cells_before,
            "la fila editada conserva su delegate"
        );
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        pump(&adapter);
        let cells_after: Vec<String> = models
            .borrow()
            .per_pane
            .get(&id)
            .unwrap()
            .rows
            .row_data(2)
            .unwrap()
            .cells
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_ne!(
            cells_after, cells_before,
            "se actualizan los metadatos diferidos al cancelar"
        );
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
            files.select_single(2);
        }

        // Cancelar mientras falta la fuente libera la vista retenida inmediatamente.
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            let request = c.take_rename_request().unwrap();
            crate::vm_builders::show_rename_editor(&ui, &request);
        }
        pump(&adapter);
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
            files.entries.retain(|entry| entry.name != "file.txt");
            files.entries_changed();
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(
            models.borrow().per_pane.get(&id).unwrap().rows.row_count(),
            3
        );
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        pump(&adapter);
        assert_eq!(
            models.borrow().per_pane.get(&id).unwrap().rows.row_count(),
            2
        );
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
            files.entries.push(Entry {
                name: "file.txt".into(),
                path: root.join("file.txt"),
                kind: EntryKind::File,
                size: Some(1),
                modified: Some(SystemTime::UNIX_EPOCH),
                created: None,
                hidden: false,
                system: false,
            });
            files.entries_changed();
            files.select_single(2);
        }
        (sync.sync_rows)();
        pump(&adapter);

        // Una navegación externa del panel invalida el editor antes de sincronizar filas.
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            let request = c.take_rename_request().unwrap();
            crate::vm_builders::show_rename_editor(&ui, &request);
        }
        pump(&adapter);
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "z".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "z.txt");
        let dest = root.join("dest");
        std::fs::create_dir(&dest).unwrap();
        ctrl.borrow_mut().open_in_pane(id, dest);
        (sync.sync_rows)();
        pump(&adapter);
        assert!(ctrl.borrow().rename_active.is_none());
        assert_eq!(ui.get_rename_pane(), -1);
        assert_eq!(ui.get_rename_pos(), -1);

        // El editor profundo sigue la ruta si llegan filas y conserva el borrador si la fuente
        // desaparece transitoriamente. El modelo y los callbacks son los de la ventana real.
        {
            let mut c = ctrl.borrow_mut();
            let current =
                c.ws.pane(id)
                    .unwrap()
                    .files
                    .as_ref()
                    .unwrap()
                    .current_dir
                    .clone();
            c.deep_start(id);
            *c.deep_job.as_mut().unwrap().items = vec![(
                Entry {
                    name: "profundo.txt".into(),
                    path: current.join("profundo.txt"),
                    kind: EntryKind::File,
                    size: Some(1),
                    modified: None,
                    created: None,
                    hidden: false,
                    system: false,
                },
                1,
            )];
            c.run_action(naygo_core::keymap::Action::MoveDown);
        }
        (sync.sync_rows)();
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            let request = c.take_rename_request().expect("editor profundo");
            crate::vm_builders::show_rename_editor(&ui, &request);
        }
        pump(&adapter);
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: "borrador".into(),
        });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "borrador.txt");
        let old_session = ui.get_rename_session();
        {
            let mut c = ctrl.borrow_mut();
            let job = c.deep_job.as_mut().unwrap();
            let mut new_entry = job.items[0].0.clone();
            new_entry.name = "antes.txt".into();
            new_entry.path = new_entry.path.with_file_name("antes.txt");
            job.items.insert(0, (new_entry, 1));
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(ui.get_rename_pos(), 1);
        assert_ne!(ui.get_rename_session(), old_session);
        assert_eq!(ui.get_rename_draft().as_str(), "borrador.txt");
        {
            let mut c = ctrl.borrow_mut();
            c.deep_job.as_mut().unwrap().items.remove(1);
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(
            models.borrow().per_pane.get(&id).unwrap().rows.row_count(),
            2
        );
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        pump(&adapter);
        assert!(ctrl.borrow().rename_active.is_none());
        assert_eq!(
            models.borrow().per_pane.get(&id).unwrap().rows.row_count(),
            1
        );
        {
            let mut c = ctrl.borrow_mut();
            let job = c.deep_job.as_mut().unwrap();
            let mut second = job.items[0].0.clone();
            second.name = "otro.txt".into();
            second.path = second.path.with_file_name("otro.txt");
            job.items.push((second, 1));
            assert!(c.run_action(naygo_core::keymap::Action::SelectAll));
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert!(
            models
                .borrow()
                .per_pane
                .get(&id)
                .unwrap()
                .rows
                .row_data(0)
                .unwrap()
                .selected
        );
        assert!(
            models
                .borrow()
                .per_pane
                .get(&id)
                .unwrap()
                .rows
                .row_data(1)
                .unwrap()
                .selected
        );
        ctrl.borrow_mut().open_row_context_menu(id, 0, 20.0, 20.0);
        (sync.sync_rows)();
        let menu = ui.get_ctx_menu();
        assert!(menu.active && menu.deep_mode && !menu.single_target && menu.has_selection);
        assert_eq!(ctrl.borrow().context_targets().len(), 2);
        // Con el mismo recorrido, una flecha solo cambia marcas; si cambia el orden,
        // la ruta parcial debe rechazarse para que la sincronización reconstruya las filas.
        ctrl.borrow_mut().close_context_menu();
        let mut previous =
            deep_presentation(&ctrl.borrow(), id, std::time::Instant::now()).unwrap();
        previous.row_count = models.borrow().per_pane.get(&id).unwrap().rows.row_count();
        let mut broad = previous.clone();
        for n in 0..9 {
            broad.selected.insert(root.join(format!("distinto-{n}")));
        }
        assert!(
            !apply_deep_presentation(
                &ctrl.borrow(),
                id,
                &models.borrow().per_pane.get(&id).unwrap().rows,
                &previous,
                &broad
            ),
            "un cambio amplio usa reconciliación completa"
        );
        assert!(ctrl
            .borrow_mut()
            .run_action(naygo_core::keymap::Action::MoveUp));
        let current = deep_presentation(&ctrl.borrow(), id, std::time::Instant::now()).unwrap();
        {
            let m = models.borrow();
            let rows = &m.per_pane.get(&id).unwrap().rows;
            assert!(apply_deep_presentation(
                &ctrl.borrow(),
                id,
                rows,
                &previous,
                &current
            ));
            assert_eq!(rows.iter().filter(|row| row.selected).count(), 1);
            assert_eq!(rows.iter().filter(|row| row.focused).count(), 1);
        }
        ctrl.borrow_mut()
            .deep_job
            .as_mut()
            .unwrap()
            .items
            .swap(0, 1);
        let changed = deep_presentation(&ctrl.borrow(), id, std::time::Instant::now()).unwrap();
        assert!(!apply_deep_presentation(
            &ctrl.borrow(),
            id,
            &models.borrow().per_pane.get(&id).unwrap().rows,
            &previous,
            &changed
        ));
        (sync.sync_rows)();
        {
            let mut c = ctrl.borrow_mut();
            c.close_context_menu();
            let job = c.deep_job.as_mut().unwrap();
            let root = job.root.clone();
            job.items[0].0.name = "igual.txt".into();
            job.items[0].0.path = root.join("igual.txt");
            job.items[1].0.name = "igual.txt".into();
            job.items[1].0.path = root.join("carpeta").join("igual.txt");
            job.done = true;
            job.partial = false;
            job.cancelled = false;
            assert!(c.run_action(naygo_core::keymap::Action::SelectAll));
            c.batch_open();
        }
        (sync.sync_rows)();
        pump(&adapter);
        let batch = ui.get_batch();
        assert!(batch.show_parent);
        assert_eq!(batch.rows.row_data(0).unwrap().parent.as_str(), ".");
        assert_eq!(batch.rows.row_data(1).unwrap().parent.as_str(), "carpeta");
    }

    /// Modelo de filas con un nombre por fila (las reconciliaciones comparan por nombre).
    fn named_rows(names: &[i32]) -> Vec<RowData> {
        names
            .iter()
            .map(|n| RowData {
                name: SharedString::from(n.to_string()),
                ..Default::default()
            })
            .collect()
    }

    fn model_of(names: &[i32]) -> RowsModel {
        let model = RowsModel::default();
        model.append(named_rows(names), None);
        model
    }

    fn reconcile(model: &RowsModel, names: &[i32]) {
        model.reconcile_by(named_rows(names), |a, b| a.name == b.name);
    }

    fn to_vec(model: &RowsModel) -> Vec<i32> {
        model.iter().map(|r| r.name.parse().unwrap()).collect()
    }

    #[test]
    fn incremental_agrega_al_final_sin_tocar_prefijo() {
        let m = model_of(&[1, 2, 3]);
        reconcile(&m, &[1, 2, 3, 4]);
        assert_eq!(to_vec(&m), vec![1, 2, 3, 4]);
    }

    #[test]
    fn incremental_inserta_en_el_medio() {
        let m = model_of(&[1, 2, 4, 5]);
        reconcile(&m, &[1, 2, 3, 4, 5]);
        assert_eq!(to_vec(&m), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn incremental_quita_en_el_medio() {
        let m = model_of(&[1, 2, 3, 4, 5]);
        reconcile(&m, &[1, 2, 4, 5]);
        assert_eq!(to_vec(&m), vec![1, 2, 4, 5]);
    }

    #[test]
    fn incremental_quita_al_final() {
        let m = model_of(&[1, 2, 3, 4]);
        reconcile(&m, &[1, 2]);
        assert_eq!(to_vec(&m), vec![1, 2]);
    }

    #[test]
    fn incremental_reemplazo_total() {
        let m = model_of(&[1, 2, 3]);
        reconcile(&m, &[7, 8, 9, 10]);
        assert_eq!(to_vec(&m), vec![7, 8, 9, 10]);
    }

    #[test]
    fn incremental_de_vacio_y_a_vacio() {
        let m = model_of(&[]);
        reconcile(&m, &[1, 2]);
        assert_eq!(to_vec(&m), vec![1, 2]);
        reconcile(&m, &[]);
        assert_eq!(to_vec(&m), Vec::<i32>::new());
    }

    #[test]
    fn incremental_mismo_contenido_es_noop() {
        let m = model_of(&[1, 2, 3]);
        reconcile(&m, &[1, 2, 3]);
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
            drop_folder: SharedString::new(),
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
        let mut folder = make_row("Carpeta");
        folder.is_dir = true;
        folder.drop_folder = "C:/a/destino".into();
        let mut other = folder.clone();
        other.drop_folder = "C:/b/destino".into();
        assert!(!row_data_eq(&folder, &other));
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

    /// Regresión: con un rename inline abierto, salir del filtro por tipeo (o de la vista
    /// profunda) a igual largo pasa por el camino completo con claves desconocidas y vuelve a
    /// sembrarlas, congelando la fila editada. Antes el `Ref` de `keys()` seguía vivo dentro del
    /// `if let` y `set_key` reventaba con "already borrowed".
    #[test]
    fn rename_abierto_al_salir_del_filtro_siembra_claves_sin_panic() {
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
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.active_files_mut().unwrap();
            files.entries = (0..20)
                .map(|i| Entry {
                    name: format!("f{i:02}.txt"),
                    path: root.join(format!("f{i:02}.txt")),
                    kind: EntryKind::File,
                    size: Some(1),
                    modified: Some(SystemTime::UNIX_EPOCH),
                    created: None,
                    hidden: false,
                    system: false,
                })
                .collect();
            files.entries_changed();
            files.select_single(3);
        }
        (sync.sync_rows)();
        let rows = models.borrow().per_pane[&id].rows.clone();
        assert!(rows.keys().is_some(), "vista normal: claves conocidas");
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            assert!(c.rename_active.as_ref().is_some_and(|r| r.pos == 3));
            // Filtro visual por tipeo (tiñe, no oculta): el largo no cambia.
            c.typeahead = "f0".into();
        }
        (sync.sync_rows)();
        assert!(rows.keys().is_none(), "con filtro, sin claves");
        assert_eq!(rows.row_count(), 20);
        ctrl.borrow_mut().typeahead.clear();
        // Camino completo + `set_keys` + fila congelada: no debe reventar.
        (sync.sync_rows)();
        let keys = rows.keys();
        let keys = keys
            .as_ref()
            .expect("al salir del filtro se siembran las claves");
        assert_eq!(keys.len(), 20);
        assert_eq!(
            keys[3].content, STALE_CONTENT,
            "la fila en rename queda pendiente de reconciliar"
        );
        assert_ne!(keys[4].content, STALE_CONTENT);
    }

    /// Altas al final y en el medio con un rename inline abierto: el modelo avisa UN tramo por
    /// alta (sin `reset` ni bajas), el editor conserva su borrador y su cursor (el delegate no
    /// se re-crea) y la fila seleccionada sigue seleccionada.
    #[test]
    fn rename_y_seleccion_sobreviven_a_altas_avisadas_por_tramo() {
        use naygo_core::fs_model::{Entry, EntryKind};
        use naygo_core::workspace::layout::Rect;
        use slint::platform::{
            software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
            Platform, WindowAdapter, WindowEvent,
        };
        use slint::private_unstable_api::re_exports::{
            ModelChangeListener, ModelChangeListenerContainer,
        };
        use std::pin::Pin;
        use std::time::{Duration, SystemTime};

        struct Headless(Rc<MinimalSoftwareWindow>);
        impl Platform for Headless {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                Ok(self.0.clone())
            }
        }
        fn pump(adapter: &MinimalSoftwareWindow) {
            for _ in 0..5 {
                slint::platform::update_timers_and_animations();
                adapter.draw_if_needed(|renderer| {
                    let mut pixels = vec![slint::Rgb8Pixel::default(); 1280 * 800];
                    renderer.render(&mut pixels, 1280);
                });
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        #[derive(Default)]
        struct Recorder(RefCell<Vec<String>>);
        impl ModelChangeListener for Recorder {
            fn row_changed(self: Pin<&Self>, row: usize) {
                self.0.borrow_mut().push(format!("changed {row}"));
            }
            fn row_added(self: Pin<&Self>, index: usize, count: usize) {
                self.0.borrow_mut().push(format!("added {index}+{count}"));
            }
            fn row_removed(self: Pin<&Self>, index: usize, count: usize) {
                self.0.borrow_mut().push(format!("removed {index}+{count}"));
            }
            fn reset(self: Pin<&Self>) {
                self.0.borrow_mut().push("reset".into());
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
        let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Headless(adapter.clone()))).unwrap();
        let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new_in(
            root.clone(),
            temp.path().join("settings"),
        )));
        let models = Rc::new(RefCell::new(Models::new()));
        let ui = AppWindow::new().unwrap();
        ui.set_panes(ModelRc::from(models.borrow().panes.clone()));
        ui.set_splits(ModelRc::from(models.borrow().splits.clone()));
        ui.set_settings_vm(crate::vm_builders::build_settings_vm(&ctrl.borrow().config));
        let area_of: Rc<dyn Fn() -> Rect> = Rc::new(|| Rect {
            x: 0.0,
            y: 0.0,
            w: 1280.0,
            h: 800.0,
        });
        let sync = build_sync(
            ui.as_weak(),
            ctrl.clone(),
            models.clone(),
            Rc::new(RefCell::new(HashMap::new())),
            area_of.clone(),
        );
        let ctx = crate::wire::WireCtx {
            ctrl: ctrl.clone(),
            sync_rows: sync.sync_rows.clone(),
            sync_layout: sync.sync_layout.clone(),
            start_timer: Rc::new(|| {}),
            area_of,
            palette_cmds: Rc::new(RefCell::new(Vec::new())),
            palette_cmd_indices: Rc::new(RefCell::new(Vec::new())),
            pending_eject: Rc::new(RefCell::new(None)),
            pending_eject_panes: Rc::new(RefCell::new(Vec::new())),
            pending_undo: Rc::new(RefCell::new(None)),
        };
        crate::callbacks_rename::wire_rename(&ui, &ctx);
        (sync.sync_layout)();
        let id = ctrl.borrow().active_id().unwrap();
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.active_files_mut().unwrap();
            files.entries = (0..100)
                .map(|i| entry(&root, &format!("f{i:03}.txt")))
                .collect();
            files.entries_changed();
            files.select_single(3);
        }
        (sync.sync_rows)();
        ui.show().unwrap();
        adapter.set_size(slint::PhysicalSize::new(1280, 800));
        pump(&adapter);
        {
            let mut c = ctrl.borrow_mut();
            c.op_rename();
            let request = c.take_rename_request().unwrap();
            crate::vm_builders::show_rename_editor(&ui, &request);
        }
        pump(&adapter);
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "x".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "x.txt");
        assert_eq!(ui.get_rename_pos(), 3);

        let rows = models.borrow().per_pane[&id].rows.clone();
        let listener = Box::pin(ModelChangeListenerContainer::new(Recorder::default()));
        rows.model_tracker()
            .attach_peer(listener.as_ref().model_peer());

        // (1) Alta al final (lote de listado): un solo `row_added` del tramo.
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.active_files_mut().unwrap();
            files.entries.push(entry(&root, "g000.txt"));
            files.entries.push(entry(&root, "g001.txt"));
            files.entries_appended(100);
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(*listener.0.borrow(), ["added 100+2"]);
        assert_eq!(ui.get_rename_pos(), 3);
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "y".into() });
        pump(&adapter);
        assert_eq!(
            ui.get_rename_draft().as_str(),
            "xy.txt",
            "el editor conserva borrador y cursor"
        );
        assert!(rows.row_data(3).unwrap().selected);

        // (2) Alta en el medio, DESPUÉS de la fila editada: un `row_added` de una fila; nada
        // antes de ella se toca.
        listener.0.borrow_mut().clear();
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.active_files_mut().unwrap();
            files.entries.push(entry(&root, "f050a.txt"));
            files.entries_appended(102);
        }
        (sync.sync_rows)();
        pump(&adapter);
        assert_eq!(*listener.0.borrow(), ["added 51+1"]);
        assert_eq!(rows.row_count(), 103);
        assert_eq!(rows.row_data(51).unwrap().name, "f050a.txt");
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "z".into() });
        pump(&adapter);
        assert_eq!(ui.get_rename_draft().as_str(), "xyz.txt");
        let row = rows.row_data(3).unwrap();
        assert!(row.selected && row.name == "f003.txt");
    }

    /// Dos `sync_rows` seguidos sin cambios de estado deben dejar los MISMOS modelos (mismo
    /// puntero) y no emitir ninguna notificación: un modelo nuevo por tick hacía que Slint
    /// re-creara delegates y repintara overlays de ventana completa (100 % de CPU en idle).
    #[test]
    fn sync_rows_sin_cambios_conserva_modelos_y_no_notifica() {
        use naygo_core::workspace::layout::Rect;
        use slint::platform::{software_renderer::MinimalSoftwareWindow, Platform, WindowAdapter};
        use slint::private_unstable_api::re_exports::{
            ModelChangeListener, ModelChangeListenerContainer,
        };
        use std::cell::Cell;
        use std::pin::Pin;

        struct Headless(Rc<MinimalSoftwareWindow>);
        impl Platform for Headless {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                Ok(self.0.clone())
            }
        }

        #[derive(Default)]
        struct Counter(Cell<usize>);
        impl ModelChangeListener for Counter {
            fn row_changed(self: Pin<&Self>, _row: usize) {
                self.0.set(self.0.get() + 1);
            }
            fn row_added(self: Pin<&Self>, _index: usize, _count: usize) {
                self.0.set(self.0.get() + 1);
            }
            fn row_removed(self: Pin<&Self>, _index: usize, _count: usize) {
                self.0.set(self.0.get() + 1);
            }
            fn reset(self: Pin<&Self>) {
                self.0.set(self.0.get() + 1);
            }
        }
        type Listener = Pin<Box<ModelChangeListenerContainer<Counter>>>;
        fn listen<T>(model: &ModelRc<T>) -> Listener {
            let listener = Box::pin(ModelChangeListenerContainer::new(Counter::default()));
            model
                .model_tracker()
                .attach_peer(listener.as_ref().model_peer());
            listener
        }

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("files");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.txt"), "hola").unwrap();
        let adapter = MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::NewBuffer,
        );
        slint::platform::set_platform(Box::new(Headless(adapter))).unwrap();
        let ctrl = Rc::new(RefCell::new(WorkspaceCtrl::new_in(
            root.clone(),
            temp.path().join("settings"),
        )));
        // Disposición clásica: árbol + dos Files + Propiedades + Vista previa (los paneles que
        // antes se re-asignaban en cada tick).
        ctrl.borrow_mut().apply_first_run_layout();
        // El resaltado "fresco" vence por reloj (8 s por defecto): bajo carga el test podía
        // cruzar ese umbral entre dos syncs y repintar la fila. Con una duración enorme el
        // vencimiento deja de ser una variable del test.
        ctrl.borrow_mut().config.settings.highlight_duration =
            naygo_core::config::HighlightDuration::FadeSeconds(3_600);
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
        (sync.sync_rows)();
        assert!(ctrl.borrow().has_purpose(PanePurpose::Inspector));
        assert!(ctrl.borrow().has_purpose(PanePurpose::Preview));

        // Resaltado fresco VIGENTE en el panel activo: mientras dura no debe haber trabajo por
        // fila (antes la firma era `None` y cada tick reconstruía todas las filas).
        let active = ctrl.borrow().active_id().unwrap();
        {
            let mut c = ctrl.borrow_mut();
            let files = c.ws.pane_mut(active).unwrap().files.as_mut().unwrap();
            for name in ["b.txt", "fresco.txt"] {
                files.entries.push(naygo_core::fs_model::Entry {
                    name: name.into(),
                    path: root.join(name),
                    kind: naygo_core::fs_model::EntryKind::File,
                    size: Some(1),
                    modified: None,
                    created: None,
                    hidden: false,
                    system: false,
                });
            }
            files.entries_changed();
            c.watchers.mark_fresh(
                active.0,
                vec![root.join("fresco.txt")],
                std::time::Instant::now(),
            );
        }
        (sync.sync_rows)();
        let file_rows: Vec<(PaneId, ModelRc<RowData>)> = {
            let c = ctrl.borrow();
            let m = models.borrow();
            m.pane_ids
                .iter()
                .filter(|id| c.purpose_of(**id) == Some(PanePurpose::Files))
                .map(|id| (*id, ModelRc::from(m.per_pane[id].rows.clone())))
                .collect()
        };
        assert!(
            file_rows
                .iter()
                .find(|(id, _)| *id == active)
                .unwrap()
                .1
                .iter()
                .any(|row| row.name == "fresco.txt" && row.highlight),
            "la fila fresca se pinta resaltada"
        );
        // Fuentes asíncronas que cambian el `PaneVm` por su cuenta (cambios reales, no trabajo por
        // tick) y que, bajo la carga de la suite completa, podían llegar ENTRE los dos syncs
        // medidos si solo se esperaba "un sync sin avisos":
        //  - la metadata del archivo enfocado (Propiedades pide su lectura a un worker);
        //  - el espacio libre del disco en el footer (un hilo consulta el volumen y su resultado
        //    se publica en un sync posterior, cuando `pump_footer_disk_results` lo drena).
        // Se espera por CONDICIÓN EXPLÍCITA (ambas fuentes sin trabajo en vuelo) más un sync
        // quieto, sin depender de la duración de un sleep. Si cada sync re-asignara paneles, el
        // bucle se agota y la aserción de abajo falla igual.
        {
            let settle = listen(&ModelRc::from(models.borrow().panes.clone()));
            let mut settled = false;
            for _ in 0..2_000 {
                settle.0.set(0);
                (sync.sync_rows)();
                let idle = {
                    let c = ctrl.borrow();
                    !c.meta_loading() && !c.footer_disk_in_flight()
                };
                if settle.0.get() == 0 && idle {
                    settled = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(settled, "el estado asíncrono no se asentó en 10 s");
        }
        let row_listeners: Vec<_> = file_rows.iter().map(|(_, rows)| listen(rows)).collect();

        macro_rules! snapshot {
            () => {
                (
                    ui.get_fav_tree(),
                    ui.get_fav_group_options(),
                    ui.get_op_running_rows(),
                    ui.get_op_queued_rows(),
                    ui.get_op_history_rows(),
                    ui.get_op_planning_rows(),
                    ui.get_resume_rows(),
                    ui.get_clipboard_history_rows(),
                    ui.get_help_shortcuts(),
                    ui.get_spaces_recents(),
                    ui.get_column_menu().exts,
                    ui.get_batch().rows,
                    ui.get_sync_vm().rows,
                    ui.get_search_vm().hits,
                    ui.get_points_rows(),
                )
            };
        }
        let before = snapshot!();
        let panes_before: Vec<PaneVm> = models.borrow().panes.iter().collect();
        let pane_listener = listen(&ModelRc::from(models.borrow().panes.clone()));
        let listeners = vec![
            ("fav_tree", listen(&before.0)),
            ("fav_group_options", listen(&before.1)),
            ("op_running", listen(&before.2)),
            ("op_queued", listen(&before.3)),
            ("op_history", listen(&before.4)),
            ("op_planning", listen(&before.5)),
            ("resume", listen(&before.6)),
            ("clipboard_history", listen(&before.7)),
            ("help", listen(&before.8)),
            ("spaces_recents", listen(&before.9)),
            ("column_exts", listen(&before.10)),
            ("batch_rows", listen(&before.11)),
            ("sync_rows", listen(&before.12)),
            ("search_hits", listen(&before.13)),
            ("points", listen(&before.14)),
        ];

        (sync.sync_rows)();
        (sync.sync_rows)();

        let after = snapshot!();
        macro_rules! same_models {
            ($($index:tt $name:literal),+) => {
                $(assert!(
                    before.$index == after.$index,
                    "sync_rows sin cambios reemplazó el modelo {}",
                    $name
                );)+
            };
        }
        same_models!(
            0 "fav_tree", 1 "fav_group_options", 2 "op_running", 3 "op_queued", 4 "op_history",
            5 "op_planning", 6 "resume", 7 "clipboard_history", 8 "help", 9 "spaces_recents",
            10 "column_exts", 11 "batch_rows", 12 "sync_rows", 13 "search_hits", 14 "points"
        );
        for (name, listener) in &listeners {
            assert_eq!(
                listener.0.get(),
                0,
                "el modelo {name} emitió notificaciones"
            );
        }
        assert_eq!(
            pane_listener.0.get(),
            0,
            "sync_rows sin cambios re-asignó filas del modelo de paneles"
        );
        for listener in &row_listeners {
            assert_eq!(
                listener.0.get(),
                0,
                "con un resaltado vigente, sync_rows sin cambios notificó filas de un panel"
            );
        }
        let panes_after: Vec<PaneVm> = models.borrow().panes.iter().collect();
        assert_eq!(panes_before.len(), panes_after.len());
        for (left, right) in panes_before.iter().zip(&panes_after) {
            assert!(left.inspector.meta == right.inspector.meta);
            assert!(left.preview.hl_lines == right.preview.hl_lines);
            assert!(left.preview.meta == right.preview.meta);
            assert!(left.rows == right.rows);
            assert!(left.segments == right.segments);
            assert!(left.tabs == right.tabs);
        }
    }

    /// Contraparte del test anterior: con modelos persistentes, un cambio REAL debe seguir
    /// llegando a la UI. (a) una vista previa nueva (nueva revisión) se publica en el panel
    /// Preview; (b) con la ayuda abierta, cambiar el idioma reconstruye sus filas.
    #[test]
    fn sync_rows_propaga_cambios_reales_de_preview_y_ayuda() {
        use naygo_core::workspace::layout::Rect;
        use slint::platform::{software_renderer::MinimalSoftwareWindow, Platform, WindowAdapter};

        struct Headless(Rc<MinimalSoftwareWindow>);
        impl Platform for Headless {
            fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
                Ok(self.0.clone())
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
        ctrl.borrow_mut().apply_first_run_layout();
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
        (sync.sync_rows)();

        // (a) Vista previa: nueva revisión de la vista → el PaneVm del panel Preview cambia.
        let preview_index = {
            let c = ctrl.borrow();
            let m = models.borrow();
            m.pane_ids
                .iter()
                .position(|id| c.purpose_of(*id) == Some(PanePurpose::Preview))
                .expect("la disposición clásica trae un panel Vista previa")
        };
        let preview_of = |models: &Rc<RefCell<Models>>| {
            models
                .borrow()
                .panes
                .row_data(preview_index)
                .unwrap()
                .preview
        };
        let before = preview_of(&models);
        let revision = ctrl.borrow().preview.view_revision();
        ctrl.borrow_mut()
            .preview
            .show_message_for_test("vista nueva");
        assert_ne!(ctrl.borrow().preview.view_revision(), revision);
        (sync.sync_rows)();
        let after = preview_of(&models);
        assert_eq!(after.mode, 3, "la vista de mensaje debe llegar al panel");
        assert_eq!(after.message.as_str(), "vista nueva");
        assert!(
            before.hl_lines != after.hl_lines,
            "la base nueva debe tener su propio modelo de líneas"
        );

        // (b) Ayuda abierta + cambio de idioma → filas reconstruidas en el idioma nuevo.
        ctrl.borrow_mut().help_open = true;
        (sync.sync_rows)();
        let labels = |ui: &AppWindow| {
            ui.get_help_shortcuts()
                .iter()
                .map(|row| row.label.to_string())
                .collect::<Vec<_>>()
        };
        let first = labels(&ui);
        assert!(!first.is_empty(), "al abrir la ayuda se arman sus filas");
        let current = ctrl.borrow().config.settings.language.0.clone();
        let target = if current == "en" { "es" } else { "en" };
        assert!(ctrl
            .borrow_mut()
            .config
            .set_language(naygo_core::i18n::LangId(target.into())));
        (sync.sync_rows)();
        let second = labels(&ui);
        assert_eq!(first.len(), second.len());
        assert_ne!(
            first, second,
            "con la ayuda abierta, cambiar el idioma debe reconstruir sus filas"
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
                // Sin resaltados de la pasada anterior: cada tamaño parte del mismo estado.
                workspace.watchers.prune(0, Instant::now());
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

            // Alta que el orden por nombre intercala en el medio: reconciliación por claves.
            let middle_name = format!("entry-{:06}a.dat", count / 2);
            {
                let mut workspace = ctrl.borrow_mut();
                let files = workspace.ws.active_files_mut().unwrap();
                files.entries.push(Entry {
                    name: middle_name.clone(),
                    path: root.join(&middle_name),
                    kind: EntryKind::File,
                    size: Some(1),
                    modified: Some(SystemTime::UNIX_EPOCH),
                    created: None,
                    hidden: false,
                    system: false,
                });
                files.entries_appended(count + 1);
            }
            let start = Instant::now();
            (sync.sync_rows)();
            let middle_insert_ms = start.elapsed().as_secs_f64() * 1_000.0;

            // Empieza un resaltado fresco (lo que hace el watcher tras el alta) y luego ticks
            // sin cambios mientras dura: antes cada uno reconstruía las n filas.
            {
                let mut workspace = ctrl.borrow_mut();
                let id = workspace.active_id().unwrap();
                workspace
                    .watchers
                    .mark_fresh(id.0, vec![root.join(&middle_name)], Instant::now());
            }
            let start = Instant::now();
            (sync.sync_rows)();
            let fresh_start_ms = start.elapsed().as_secs_f64() * 1_000.0;
            let mut fresh_noop_samples = Vec::with_capacity(5);
            for _ in 0..5 {
                let start = Instant::now();
                (sync.sync_rows)();
                fresh_noop_samples.push(start.elapsed().as_secs_f64() * 1_000.0);
            }

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
                "W-07/W-10C software rows={count} initial_ms={initial_ms:.3} no_op_median_ms={:.3} one_row_update_ms={one_row_update_ms:.3} append_ms={append_ms:.3} middle_insert_ms={middle_insert_ms:.3} fresh_start_ms={fresh_start_ms:.3} fresh_noop_median_ms={:.3} render_median_ms={render_median_ms:.3}",
                median_ms(no_op_samples),
                median_ms(fresh_noop_samples),
            );
        }
        ui.hide().unwrap();
    }
}
