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
fn apply_rows_incremental<T: PartialEq + Clone + 'static>(model: &VecModel<T>, rows: Vec<T>) {
    let old_n = model.row_count();
    let new_n = rows.len();
    // Prefijo común (filas idénticas al inicio).
    let mut prefix = 0;
    while prefix < old_n.min(new_n) && model.row_data(prefix).as_ref() == rows.get(prefix) {
        prefix += 1;
    }
    // Sufijo común (filas idénticas al final, sin solapar el prefijo).
    let mut suffix = 0;
    while suffix < (old_n - prefix).min(new_n - prefix)
        && model.row_data(old_n - 1 - suffix).as_ref() == rows.get(new_n - 1 - suffix)
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
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            // `borrow_mut` porque `rows_of` necesita mutar el IconCache (decodifica on-demand).
            let mut c = ctrl.borrow_mut();
            // Título de la ventana según el modo configurado (solo app / app+ruta / solo ruta).
            // Se recalcula en cada sync pero solo se escribe si cambió (navegación, cambio de
            // panel activo o de la opción en Configuración).
            let title = c.window_title();
            if ui.get_window_title() != title {
                ui.set_window_title(title.into());
            }
            let active = c.active_id();
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
                        let sig = c.rows_signature(id, hl_secs, hl_now);
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
                            let rows: Vec<RowData> = c
                                .rows_of(id, hl_secs, hl_now)
                                .into_iter()
                                .map(to_row_data)
                                .collect();
                            let pm = m.models_for(id);
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
                                    if pm.rows.row_data(i).as_ref() != Some(&row) {
                                        pm.rows.set_row_data(i, row);
                                    }
                                }
                            } else {
                                // El largo cambió (alta/baja real): aplicación INCREMENTAL
                                // (prefijo/sufijo intactos; solo el tramo del medio se
                                // quita/inserta) en vez de un `set_vec` que re-creaba todos
                                // los delegates — ver `apply_rows_incremental`.
                                apply_rows_incremental(&pm.rows, rows);
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
                        m.models_for(id).basket.set_vec(rows);
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
            }
            ui.set_status(SharedString::from(c.status_line().as_str()));
            ui.set_sync_vm(c.sync_vm());
            ui.set_text_transform_vm(c.text_transform_vm());
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
                        name: r.name.into(),
                        rel_dir: r.rel_dir.into(),
                        detail: r.detail.into(),
                        is_dir: r.is_dir,
                        icon: r.icon,
                    })
                    .collect();
                ui.set_search_vm(SearchVm {
                    active: open,
                    query: query.into(),
                    root_label: root_label.into(),
                    content_query: options.content_query.into(),
                    ignore_case: options.ignore_case,
                    use_wildcards: options.use_wildcards,
                    recursive: options.recursive,
                    running,
                    hits: ModelRc::new(VecModel::from(hits)),
                    status: status.into(),
                });
            }
            // Menú/editor de columna (clic derecho en el header, F2).
            let colmenu = match c.column_menu_snapshot() {
                Some(m) => {
                    let no_ext = ui.global::<Tr>().get_colfilter_no_ext();
                    let exts: Vec<ExtRowVm> = m
                        .exts
                        .into_iter()
                        .map(|e| {
                            let label = if e.ext.is_empty() {
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
            let all_ids: Vec<PaneId> = pane_rects.iter().map(|(id, _)| *id).collect();

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
                        let tabs: Vec<TabVm> = groups
                            .iter()
                            .find(|(mem, a)| mem.get(*a) == Some(id))
                            .map(|(mem, a)| {
                                mem.iter()
                                    .enumerate()
                                    .map(|(i, mid)| TabVm {
                                        id: mid.0 as i32,
                                        label: SharedString::from(c.pane_label(*mid).as_str()),
                                        active: i == *a,
                                    })
                                    .collect()
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
) -> Rc<dyn Fn()> {
    Rc::new(move || {
        if !devices.drives_changed() {
            return;
        }
        // El espacio en disco pudo cambiar (USB conectado/expulsado): invalida la caché del
        // footer para que se relea en el próximo tick, igual que se refresca la tira de discos.
        ctrl.borrow_mut().invalidate_footer_disk_cache();
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
}
