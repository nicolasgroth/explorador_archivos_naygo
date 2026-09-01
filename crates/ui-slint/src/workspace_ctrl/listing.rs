// Naygo — WorkspaceCtrl: listado de carpetas, footer, carpeta-no-encontrada y filas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

/// Resultado del probe async de "carpeta no encontrada" para UN panel. Lo calcula un hilo
/// worker (el `read_dir` puede colgar SEGUNDOS sobre un share de red caído: jamás en la UI).
/// Guarda la carpeta evaluada para descartar resultados obsoletos (el panel navegó mientras
/// el probe corría).
pub(super) struct MissingProbeHit {
    pane: u64,
    dir: PathBuf,
    missing: bool,
    has_ancestor: bool,
}

impl WorkspaceCtrl {
    /// El ancestro existente más cercano de `path` (sube hasta encontrar uno que exista; si
    /// ninguno —p. ej. la unidad entera se fue—, devuelve None).
    fn nearest_existing_ancestor(path: &std::path::Path) -> Option<PathBuf> {
        let mut cur = path.parent();
        while let Some(p) = cur {
            if p.exists() {
                return Some(p.to_path_buf());
            }
            cur = p.parent();
        }
        None
    }

    /// La carpeta actual del panel `id` (para el aviso in-place de "carpeta no encontrada").
    fn pane_current_dir(&self, id: PaneId) -> Option<PathBuf> {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
    }

    /// Índice de VISTA de la fila enfocada del panel `id`, o -1 si no hay foco / no es un panel
    /// Files. Lo consume el builder del PaneVm: la UI lo observa (`changed focused-row`) para que
    /// el scroll del listado siga a la fila enfocada al navegar por teclado (la ListView ya no es
    /// interactiva y no auto-scrollea sola, ver C1).
    pub fn focused_view_of(&self, id: PaneId) -> i32 {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.focused)
            .map(|i| i as i32)
            .unwrap_or(-1)
    }

    /// Fila de vista que era la primera visible en la visita histórica restaurada.
    pub fn restored_scroll_row_of(&self, id: PaneId) -> i32 {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.restored_scroll_row)
            .map(|i| i as i32)
            .unwrap_or(-1)
    }

    /// Conserva el ancla del scroll por ruta; no realiza I/O y se puede llamar desde Slint.
    pub fn set_scroll_top_position(&mut self, id: PaneId, pos: usize) {
        if let Some(files) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            files.set_scroll_top_position(pos);
        }
    }

    /// ¿La carpeta del panel `id` dejó de existir / es ilegible? Lee del CACHÉ (sin I/O), que
    /// se recalcula async en eventos reales (`refresh_missing_cache` + `pump_missing_probe`).
    /// Antes hacía un `read_dir` síncrono en el hilo de UI en cada tick, lo que congelaba la
    /// app sobre un share de red caído. Lo consulta el builder del PaneVm para el aviso in-place.
    /// La entrada cacheada guarda la carpeta evaluada: si el panel ya navegó a otra, la entrada
    /// obsoleta NO cuenta (evita mostrar el aviso sobre una carpeta recién navegada mientras el
    /// probe nuevo está en vuelo).
    pub fn pane_dir_missing(&self, id: PaneId) -> bool {
        match self.missing_cache.get(&id.0) {
            Some((dir, missing, _)) => {
                *missing && self.pane_current_dir(id).as_deref() == Some(dir.as_path())
            }
            None => false,
        }
    }

    /// ¿El panel `id` (en estado "carpeta no encontrada") tiene un ancestro existente real al
    /// que subir? Falso si la unidad entera se desconectó. Lee del CACHÉ (sin I/O), con la misma
    /// invalidación por carpeta obsoleta que `pane_dir_missing`.
    pub fn pane_has_existing_ancestor(&self, id: PaneId) -> bool {
        match self.missing_cache.get(&id.0) {
            Some((dir, _, has_anc)) => {
                *has_anc && self.pane_current_dir(id).as_deref() == Some(dir.as_path())
            }
            None => false,
        }
    }

    /// Recalcula el caché del estado "carpeta no encontrada" de TODOS los paneles Files. Se
    /// llama ante eventos que pueden cambiarlo (navegar, reintentar, expulsar, conectar/
    /// desconectar disco), NUNCA en el tick de render. NO hace I/O en el hilo de UI: solo
    /// fotografía las carpetas actuales y lanza UN worker que hace el `read_dir` + búsqueda de
    /// ancestro por panel; el tick aplica los resultados con `pump_missing_probe`. Un share de
    /// red caído puede colgar el worker segundos sin congelar la app. Un refresh nuevo
    /// reemplaza al anterior (sus resultados se descartan por carpeta obsoleta si llegan).
    pub fn refresh_missing_cache(&mut self) {
        // Solo paneles de archivos (los demás no tienen "carpeta no encontrada").
        let ids: Vec<PaneId> = self
            .ws
            .panes()
            .iter()
            .filter(|p| p.files.is_some())
            .map(|p| p.id)
            .collect();
        self.missing_cache
            .retain(|k, _| ids.iter().any(|id| id.0 == *k));
        let targets: Vec<(u64, PathBuf)> = ids
            .iter()
            .filter_map(|id| self.pane_current_dir(*id).map(|dir| (id.0, dir)))
            .collect();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let hits: Vec<MissingProbeHit> = targets
                .into_iter()
                .map(|(pane, dir)| {
                    let missing = std::fs::read_dir(&dir).is_err();
                    let has_ancestor =
                        missing && WorkspaceCtrl::nearest_existing_ancestor(&dir).is_some();
                    MissingProbeHit {
                        pane,
                        dir,
                        missing,
                        has_ancestor,
                    }
                })
                .collect();
            // Si el receptor se reemplazó (refresh más nuevo) o se cayó, descartar en silencio.
            let _ = tx.send(hits);
        });
        self.missing_probe = Some(rx);
    }

    /// Drena el probe async de "carpeta no encontrada" (sin bloquear) y aplica los resultados
    /// al caché. Un resultado se aplica SOLO si el panel sigue en la misma carpeta que se
    /// evaluó (si navegó mientras el probe corría, el resultado es obsoleto y se descarta:
    /// el refresh de ESA navegación trae el suyo). Devuelve true si NO hay probe en vuelo
    /// (para que el timer pueda dormir).
    pub fn pump_missing_probe(&mut self) -> bool {
        let Some(rx) = self.missing_probe.as_ref() else {
            return true;
        };
        match rx.try_recv() {
            Ok(hits) => {
                for hit in hits {
                    let still_there = self.pane_current_dir(PaneId(hit.pane)).as_deref()
                        == Some(hit.dir.as_path());
                    if still_there {
                        self.missing_cache
                            .insert(hit.pane, (hit.dir, hit.missing, hit.has_ancestor));
                    }
                }
                self.missing_probe = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.missing_probe = None;
            }
        }
        self.missing_probe.is_none()
    }

    /// Reintentar en el panel `id`: si su carpeta volvió a existir (USB reconectado), re-listar; si
    /// sigue sin existir, no hace nada (el aviso permanece). El aviso desaparece solo cuando el
    /// re-listado puebla la carpeta (la detección es por `current_dir.exists()`).
    pub fn missing_folder_retry(&mut self, id: PaneId) {
        crate::logging::log_line(&format!("carpeta no encontrada: reintentar panel {}", id.0));
        if let Some(dir) = self.pane_current_dir(id) {
            if std::fs::read_dir(&dir).is_ok() {
                self.cancel_deep_if_navigating(id);
                self.start_listing(id, dir.clone());
                self.sync_trees_for_files(id, dir);
            }
        }
    }

    /// Subir al ancestro existente más cercano del panel `id` (o al HOME si la unidad entera se
    /// fue). Navega ese panel y re-lista.
    pub fn missing_folder_go_ancestor(&mut self, id: PaneId) {
        {
            let dir = self
                .pane_current_dir(id)
                .map(|d| d.display().to_string())
                .unwrap_or_default();
            crate::logging::log_line(&format!(
                "carpeta no encontrada: subir del panel {} ({})",
                id.0, dir
            ));
        }
        let Some(lost) = self.pane_current_dir(id) else {
            return;
        };
        let dest = Self::nearest_existing_ancestor(&lost).unwrap_or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("C:/"))
        });
        self.cancel_deep_if_navigating(id);
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dest.clone());
        }
        self.start_listing(id, dest.clone());
        self.sync_trees_for_files(id, dest);
    }

    /// Navegar el panel `id` a `dir` (elegido en el selector nativo) y re-listar.
    pub fn missing_folder_choose(&mut self, id: PaneId, dir: PathBuf) {
        self.cancel_deep_if_navigating(id);
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dir.clone());
        }
        self.start_listing(id, dir.clone());
        self.sync_trees_for_files(id, dir);
    }

    /// Cerrar el panel `id` (si se puede; si es el último, lo manda al HOME en su lugar).
    pub fn missing_folder_close_pane(&mut self, id: PaneId) {
        if self.can_close_pane(id) {
            self.close_pane(id);
        } else {
            // Es el último panel: no se puede cerrar; lo mandamos al HOME para que sea usable.
            let home = std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("C:/"));
            self.cancel_deep_if_navigating(id);
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.navigate_to(home.clone());
            }
            self.start_listing(id, home);
        }
    }

    /// Arranca el listado del panel `id` en `dir` (cancela el suyo anterior).
    pub fn start_listing(&mut self, id: PaneId, dir: std::path::PathBuf) {
        if let Some(l) = self.listings.get(&id) {
            l.cancel();
        }
        // Una comparación describe exactamente los snapshots cargados; navegar o refrescar uno
        // de sus lados la invalida para no dejar marcas obsoletas sobre nombres nuevos.
        if self.comparison.remove(&id).is_some() {
            self.comparison.clear();
            self.comparison_revision = self.comparison_revision.wrapping_add(1);
        }
        // "Ocultar iguales" pertenece al mismo snapshot: una navegación o refresh no puede
        // seguir escondiendo rutas basadas en una comparación ya invalidada.
        if let Some(files) = self.ws.pane_mut(id).and_then(|pane| pane.files.as_mut()) {
            files.set_hidden_paths(std::collections::HashSet::new());
        }
        // Navegar a OTRA carpeta limpia el filtro visual por tipeo (decisión de diseño:
        // persiste hasta Esc o navegar). Un refresh de la MISMA carpeta (F5) lo conserva.
        // La referencia es `last_listing_dirs` (lo último PEDIDO para el panel), NO
        // `f.current_dir`: hay caminos que la mutan antes de llamar aquí (navigate_pane_to).
        let cambia_carpeta = self.last_listing_dirs.get(&id) != Some(&dir);
        if cambia_carpeta {
            self.clear_filter();
            // El caché de índices de matching (text_match) guarda nombres de ESTA carpeta;
            // navegar lo invalida para no arrastrar nombres viejos ni crecer sin límite.
            naygo_core::text_match::clear_index_cache();
        }
        self.last_listing_dirs.insert(id, dir.clone());
        // Un listado nuevo reemplaza al anterior: su reciente diferido (si lo había) ya no
        // aplica — el camino que navega decide si registra uno para el listado NUEVO.
        self.pending_recents.remove(&id);
        // Navegar (o refrescar) puede cambiar de unidad. La caché está indexada por raíz, así que
        // no necesitamos borrar datos útiles de los demás paneles; los dispositivos sí la
        // invalidan explícitamente en `invalidate_footer_disk_cache`.
        // Si el panel estaba marcado como "expulsado" y ahora navega a una carpeta válida,
        // limpiar el flag para que el aviso vuelva a ser el genérico si vuelve a desconectarse.
        self.ejected_panes.remove(&id.0);
        self.listings.insert(id, Listing::start(dir));
        // Recalcular el estado "carpeta no encontrada" tras navegar (sale del tick de render).
        self.refresh_missing_cache();
    }

    /// Marca el panel `id` como "soltado por expulsión" (cambia el texto del aviso).
    pub fn mark_pane_ejected(&mut self, id: PaneId) {
        self.ejected_panes.insert(id.0);
    }

    /// ¿El panel `id` fue soltado por una expulsión? (para el texto del aviso).
    pub fn pane_was_ejected(&self, id: PaneId) -> bool {
        self.ejected_panes.contains(&id.0)
    }

    /// Invalida la caché de disco del footer. La llama main.rs cuando cambian los dispositivos
    /// (USB conectado/expulsado, vía `on_wake`/`drives_changed`), para que el espacio libre del
    /// footer siga esos eventos aunque el panel no haya navegado.
    pub fn invalidate_footer_disk_cache(&mut self) {
        self.footer_disk_cache.clear();
    }

    /// Drena las lecturas de espacio terminadas. Corre en el tick de UI pero solo hace `try_recv`:
    /// la llamada WinAPI real vive exclusivamente en el worker lanzado más abajo.
    fn pump_footer_disk_results(&mut self) {
        while let Ok((root, usage)) = self.footer_disk_results.try_recv() {
            self.footer_disk_pending.remove(&root);
            if let Some(usage) = usage {
                self.footer_disk_cache.insert(root, usage);
            }
        }
    }

    /// Devuelve el dato ya cacheado y, si falta, agenda una lectura asíncrona. La primera pintura
    /// muestra "—" brevemente; nunca bloquea la navegación ni el render por un share lento.
    fn cached_disk_usage(&mut self, root: &Path) -> Option<naygo_core::disk::DiskUsage> {
        self.pump_footer_disk_results();
        if let Some(usage) = self.footer_disk_cache.get(root) {
            return Some(*usage);
        }
        let root = root.to_path_buf();
        if self.footer_disk_pending.insert(root.clone()) {
            let sender = self.footer_disk_sender.clone();
            std::thread::spawn(move || {
                let usage = disk_usage(&root);
                let _ = sender.send((root, usage));
            });
        }
        None
    }

    /// El preset de footer EFECTIVO: si el guardado es `Custom`, usa el template del usuario
    /// (`footer_custom_template`), que es donde vive realmente (la variante puede traer string
    /// vacío). Para el resto, devuelve el preset tal cual.
    fn footer_preset_resolved(&self) -> naygo_core::footer::FooterPreset {
        match &self.config.settings.footer_preset {
            naygo_core::footer::FooterPreset::Custom(_) => {
                naygo_core::footer::FooterPreset::Custom(
                    self.config.settings.footer_custom_template.clone(),
                )
            }
            other => other.clone(),
        }
    }

    /// Texto del footer (barra inferior) para el panel `id`. Vacío si el footer está deshabilitado
    /// en Settings o el panel no es de archivos. Lo invoca `sync_rows` por tick. Toma `&mut self`
    /// por la caché de disco (`footer_disk_cache`): primero lee los datos crudos del panel bajo un
    /// borrow inmutable de `self.ws`, lo suelta, y recién entonces toca la caché y renderiza. Así
    /// no chocan el `&self.ws` y el `&mut self.footer_disk_cache`.
    pub fn footer_text_of(&mut self, id: PaneId) -> String {
        if !self.config.settings.footer_enabled {
            return String::new();
        }
        // Paso 1: extraer datos crudos del panel (borrow inmutable acotado a este bloque).
        let Some((data_no_disk, root)) = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(footer_inputs_of)
        else {
            return String::new();
        };
        // Paso 2 (ya sin borrow de `self.ws`): uso de disco cacheado por raíz de unidad. Si falta,
        // se solicita a un worker y este tick conserva una respuesta inmediata.
        let disk = match root {
            Some(root) => self.cached_disk_usage(&root),
            None => None,
        };
        let data = naygo_core::footer::FooterData {
            disk,
            ..data_no_disk
        };
        let preset = self.footer_preset_resolved();
        let base = naygo_core::footer::render(&preset, &data, self.config.settings.size_format);
        // Sufijo del filtro visual por tipeo (solo en el panel ACTIVO, que es donde se
        // tipea): ` · filtro: "texto" · N coincidencias`. Hace el filtro visible (el buffer
        // de tipeo no se ve en ninguna otra parte) y da feedback de cuánto matchea.
        let suffix = self.filter_label_of(id);
        let changes = self
            .visit_changes
            .get(&id)
            .map(|all| {
                use naygo_core::listing_changes::ChangeKind;
                let n = all.iter().filter(|c| c.kind == ChangeKind::New).count();
                let m = all
                    .iter()
                    .filter(|c| c.kind == ChangeKind::Modified)
                    .count();
                let d = all.iter().filter(|c| c.kind == ChangeKind::Missing).count();
                format!("+{n} ~{m} -{d}")
            })
            .unwrap_or_default();
        [base, suffix, changes]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// Texto del volumen para el extremo derecho del footer. El preset Completa/Solo disco ya
    /// contiene estos datos, por lo que no se duplica; Compacta (el valor por defecto) mantiene
    /// selección a la izquierda y capacidad/libre/uso claramente separado a la derecha.
    pub fn footer_disk_text_of(&mut self, id: PaneId) -> String {
        if !self.config.settings.footer_enabled {
            return String::new();
        }
        if matches!(
            self.config.settings.footer_preset,
            naygo_core::footer::FooterPreset::Full
                | naygo_core::footer::FooterPreset::DiskOnly
                | naygo_core::footer::FooterPreset::Custom(_)
        ) {
            return String::new();
        }
        let root = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.current_dir.ancestors().last().map(Path::to_path_buf));
        let Some(usage) = root
            .as_deref()
            .and_then(|root| self.cached_disk_usage(root))
        else {
            return String::new();
        };
        let free = naygo_core::format::format_size(usage.free, self.config.settings.size_format);
        let total = naygo_core::format::format_size(usage.total, self.config.settings.size_format);
        self.config
            .t("disk.usage")
            .replace("{free}", &free)
            .replace("{total}", &total)
            .replace("{pct}", &usage.percent_used().to_string())
    }

    /// Etiqueta del filtro visual por tipeo para el panel `id` (`filtro: "texto" · N
    /// coincidencias`, ya traducida), o vacío si no aplica (sin filtro o panel inactivo).
    /// La usan el footer y la mini-barra del panel (misma composición, una sola fuente).
    pub fn filter_label_of(&self, id: PaneId) -> String {
        if self.ws.active_id() != Some(id) || !self.filter_active() {
            return String::new();
        }
        let label = self.config.t("status.filter_label");
        let matches = self
            .config
            .t("status.filter_matches")
            .replace("{n}", &self.filter_match_count.to_string());
        format!("{label}: \"{}\" · {matches}", self.typeahead)
    }

    /// Drena los lotes de TODOS los listados activos. Devuelve true si TODOS terminaron
    /// (para apagar el timer). Quita del mapa los que terminan.
    pub fn pump_listings(&mut self) -> bool {
        // Flags de visibilidad globales: se empujan al panel al sembrar sus entries, así un
        // panel recién listado (arranque, navegación, refresh) ya nace con la vista filtrada
        // correcta sin depender de que el usuario abra el menú "ojo".
        let vis = self.visibility_flags();
        // Espejo del setting "archivos nuevos al final": se empuja en el mismo punto, así un panel
        // recién listado refleja la opción aunque el watcher aún no haya disparado.
        let group_new_at_end = self.config.settings.new_items_at_end;
        let ids: Vec<PaneId> = self.listings.keys().copied().collect();
        for id in ids {
            // El flag `fresh` se reclama solo cuando este poll trae avance real: o llegaron
            // entries, o el listado TERMINÓ (carpeta vacía → done sin lotes). Un tick que poll-ea
            // vacío y sin terminar NO lo consume, así el reemplazo de filas se aplica recién con
            // el primer avance real. `take_fresh()` solo devuelve `true` una vez por listado.
            let (batch, done, fresh, succeeded) = match self.listings.get_mut(&id) {
                Some(l) => {
                    let (b, d, ok) = l.poll();
                    let fresh = if b.is_empty() && !d {
                        false
                    } else {
                        l.take_fresh()
                    };
                    (b, d, fresh, ok)
                }
                None => continue,
            };
            let batch_was_empty = batch.is_empty();
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.set_visibility(vis);
                f.group_new_at_end = group_new_at_end;
                if !batch_was_empty {
                    // Primer lote de un listado nuevo: REEMPLAZAR (no acumular) las entries que
                    // el panel tuviera de antes. Esto evita que F5 duplique las filas al
                    // re-listar el mismo directorio sobre un panel ya poblado.
                    if fresh {
                        f.entries.clear();
                    }
                    f.entries.extend(batch);
                    f.entries_changed();
                }
                if done {
                    // Carpeta que quedó VACÍA tras refrescar: el listado nuevo no emitió ningún
                    // lote, así que `fresh` nunca se reclamó y las entries viejas seguirían ahí.
                    // Al terminar, si aún estaba fresco, vaciar para reflejar la carpeta vacía.
                    if fresh && batch_was_empty {
                        f.entries.clear();
                        f.entries_changed();
                    }
                    let spec = f.sort;
                    naygo_core::sort::sort_entries(&mut f.entries, &spec);
                    let restored = f.apply_pending_nav_context();
                    if !restored && f.focused.is_none() && !f.entries.is_empty() {
                        f.focused = Some(0);
                        f.presentation_changed();
                    }
                }
            }
            if done {
                self.record_listing_changes(id);
                self.listings.remove(&id);
                // Reciente diferido (navegación sin chequeo síncrono de `dir_is_navigable`):
                // se registra SOLO si el listado terminó con éxito, así una ruta muerta (red
                // caída, favorito viejo) no ensucia recientes — el aviso in-place ya informa.
                if succeeded {
                    if let Some(dir) = self.pending_recents.remove(&id) {
                        self.push_recent(dir);
                    }
                } else {
                    self.pending_recents.remove(&id);
                }
            }
        }
        self.listings.is_empty()
    }

    fn record_listing_changes(&mut self, id: PaneId) {
        let Some((dir, entries)) = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| (f.current_dir.clone(), f.entries.clone()))
        else {
            return;
        };
        let changes = self
            .listing_snapshots
            .get(&dir)
            .map(|before| naygo_core::listing_changes::compare(before, &entries))
            .unwrap_or_default();
        self.listing_snapshots.insert(dir, entries);
        if changes.is_empty() {
            self.visit_changes.remove(&id);
        } else {
            self.visit_changes.insert(id, changes);
        }
        self.visit_changes_revision = self.visit_changes_revision.wrapping_add(1);
    }

    /// Segundos que dura el resaltado de archivos nuevos, según el ajuste. `FadeSeconds(n)`→n;
    /// `UntilInteract`/`UntilRefresh` se aproximan con un tope generoso (el resaltado es una
    /// pista transitoria; modelar "hasta interactuar" exactamente no aporta y complica).
    pub fn highlight_secs(&self) -> u64 {
        use naygo_core::config::HighlightDuration::*;
        match self.config.settings.highlight_duration {
            FadeSeconds(n) => n as u64,
            UntilInteract | UntilRefresh => 8,
        }
    }

    /// Filas a pintar del panel `id` (marca las cortadas para atenuarlas y las recién
    /// aparecidas para resaltarlas durante `highlight_secs` segundos).
    /// Los tres flags de visibilidad actuales, leídos de los settings. Los usan el panel
    /// (`rows_of`/`rows_from_view`), la vista profunda y el filtrado del árbol (`pump_tree`)
    /// para llamar a `naygo_core::filter::is_visible`. Globales y persistentes.
    pub fn visibility_flags(&self) -> VisibilityFlags {
        VisibilityFlags {
            show_hidden: self.config.settings.show_hidden,
            show_system: self.config.settings.show_system,
            hide_dotfiles: self.config.settings.hide_dotfiles,
        }
    }

    pub fn rows_of(
        &mut self,
        id: PaneId,
        highlight_secs: u64,
        now: std::time::Instant,
    ) -> Vec<PlainRow> {
        let date_format = self.config.settings.date_format;
        let size_format = self.config.settings.size_format;
        let vis = self.visibility_flags();
        let tz = naygo_platform::time::local_utc_offset_secs();
        // Aguja del filtro visual por tipeo (ya plegada): SOLO para el panel activo
        // (None para los demás paneles o sin filtro → todo filter_match = false).
        let filter_needle = self.active_filter_needle(id);

        // Vista profunda activa: construir filas desde deep_items con depth real.
        // Las columnas visibles (orden/formato) se leen del FilePaneState normal.
        if self.is_deep_active(id) {
            // Extraer los datos que necesitamos antes de los préstamos disjuntos.
            let cell_kinds: Vec<naygo_core::columns::ColumnKind> = self
                .ws
                .pane(id)
                .and_then(|p| p.files.as_ref())
                .map(|f| f.table.visible_columns().map(|c| c.kind).collect())
                .unwrap_or_default();

            // Iterar los items del job profundo POR REFERENCIA (sin clonar el Vec entero, que
            // en streaming corre muchas veces por segundo). Igual que el camino normal de abajo,
            // se destructura `self` en préstamos disjuntos: `deep_job`, `ops` e `icons` son campos
            // distintos, así que sus `&` no se solapan y el borrow checker los acepta.
            let rows: Vec<PlainRow> = {
                let WorkspaceCtrl {
                    deep_job,
                    ops,
                    icons,
                    ..
                } = self;
                let deep_items: &[(naygo_core::fs_model::Entry, u32)] =
                    deep_job.as_ref().map(|d| d.items.as_slice()).unwrap_or(&[]);
                deep_items
                    .iter()
                    .filter(|(e, _)| {
                        naygo_core::filter::is_visible(
                            e,
                            vis.show_hidden,
                            vis.show_system,
                            vis.hide_dotfiles,
                        )
                    })
                    .map(|(e, depth)| {
                        let cells = cell_kinds
                            .iter()
                            .map(|k| crate::bridge::cell_value(e, *k, size_format, date_format, tz))
                            .collect();
                        // Mismo criterio que la vista normal: el rango del match entrega el
                        // flag y los tramos para pintar (un solo fold por ítem).
                        let (filter_match, match_pre, match_mid, match_post) = match filter_needle
                            .as_deref()
                            .and_then(|n| naygo_core::text_match::match_char_range(&e.name, n))
                        {
                            Some((s, en)) => {
                                let (pre, mid, post) =
                                    naygo_core::text_match::split_at_char_range(&e.name, s, en);
                                (true, pre, mid, post)
                            }
                            None => (false, String::new(), String::new(), String::new()),
                        };
                        PlainRow {
                            name: e.name.clone(),
                            cells,
                            is_dir: e.kind == naygo_core::fs_model::EntryKind::Directory,
                            selected: false,
                            focused: false,
                            cut: ops.is_cut(&e.path),
                            highlight: false,
                            compare_state: 0,
                            filter_match,
                            match_pre,
                            match_mid,
                            match_post,
                            icon: icons.get_for_entry(e),
                            depth: *depth,
                        }
                    })
                    .collect()
            };
            if filter_needle.is_some() {
                self.filter_match_count = rows.iter().filter(|r| r.filter_match).count();
            }
            return rows;
        }

        // Vista normal: préstamos disjuntos para ops/watchers/icons.
        let mut rows = {
            let WorkspaceCtrl {
                ws,
                ops,
                watchers,
                icons,
                ..
            } = self;
            match ws.pane(id).and_then(|p| p.files.as_ref()) {
                Some(f) => rows_from_view(
                    f,
                    &|p| ops.is_cut(p),
                    &|p| watchers.is_fresh_ro(id.0, p, highlight_secs, now),
                    &mut |e| icons.get_for_entry(e),
                    size_format,
                    date_format,
                    tz,
                    filter_needle.as_deref(),
                ),
                None => Vec::new(),
            }
        };
        if let (Some(marks), Some(f)) = (
            self.comparison.get(&id),
            self.ws.pane(id).and_then(|p| p.files.as_ref()),
        ) {
            for (row, &real) in rows.iter_mut().zip(f.view_indices().iter()) {
                if let Some(entry) = f.entries.get(real) {
                    row.compare_state = marks.get(&entry.path).copied().unwrap_or(0);
                }
            }
        }
        if let Some(changes) = self.visit_changes.get(&id) {
            for row in &mut rows {
                row.highlight |= changes.iter().any(|change| {
                    change.kind != naygo_core::listing_changes::ChangeKind::Missing
                        && change.path.file_name().and_then(|name| name.to_str())
                            == Some(row.name.as_str())
                });
            }
        }
        if filter_needle.is_some() {
            self.filter_match_count = rows.iter().filter(|r| r.filter_match).count();
        }
        rows
    }

    /// Firma O(n) SIN allocs de TODO lo que determina las filas pintadas del panel `id` (O-1).
    /// Si entre dos ticks esta firma no cambia, `rows_of` produciría EXACTAMENTE las mismas filas,
    /// así que `sync_rows` puede saltarse la reconstrucción (que sí aloca decenas de miles de
    /// `String` por tick en carpetas grandes bajo render por software).
    ///
    /// Devuelve `None` cuando NO se debe cachear: el panel tiene resaltado "fresco" vigente, que
    /// se desvanece por diseño cada tick (`is_fresh_ro` depende de `now`). En ese caso el llamador
    /// reconstruye siempre para que el fundido se vea. `None` también para paneles no-Files.
    ///
    /// La firma cubre, exhaustivamente, cada cosa que afecta una `PlainRow`:
    ///  - identidad+orden+contenido de las entries VISIBLES (vía `view_indices`): nombre, ruta,
    ///    tamaño, modificado, creado, y si es directorio. Hashear el CONTENIDO (no solo `len`) es
    ///    obligatorio: un `DirEvent::Modified` reemplaza un entry IN SITU sin cambiar `len`, lo que
    ///    cambia la celda de Tamaño/Fecha. Iterar la vista es O(n) pero sin alocar Strings (mucho
    ///    más barato que reconstruir las filas, que es O(n) CON allocs);
    ///  - selección (`selected`) y foco (`focused`);
    ///  - conjunto de columnas visibles, en su orden (qué celdas y en qué orden van);
    ///  - formato de tamaño y de fecha (cambian el texto de las celdas);
    ///  - flags de visibilidad (qué entries entran a la vista; ya implícito en `view_indices`, se
    ///    incluye igual por robustez);
    ///  - conjunto de rutas "cortadas" (atenúa la fila);
    ///  - firma del set de íconos (set activo + tinte + overrides): cambia el ícono de cada fila;
    ///  - vista profunda: flag activo + nº de items del job (otra fuente de filas).
    ///
    /// Lo que NO entra a propósito: `drag_over` (es del PaneVm, no de las filas, y `sync_rows` lo
    /// maneja aparte) y los anchos de columna (no afectan el TEXTO de la fila; van por el modelo
    /// `columns`, que se sigue refrescando con `columns_of`).
    pub fn rows_signature(
        &self,
        id: PaneId,
        highlight_secs: u64,
        now: std::time::Instant,
    ) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        let f = self.ws.pane(id).and_then(|p| p.files.as_ref())?;

        let mut h = std::collections::hash_map::DefaultHasher::new();

        // --- Formato (afecta el texto de celdas) ---
        std::mem::discriminant(&self.config.settings.size_format).hash(&mut h);
        std::mem::discriminant(&self.config.settings.date_format).hash(&mut h);

        // --- Visibilidad (qué entries entran a la vista) ---
        self.visibility_flags().hash(&mut h);

        // --- Columnas visibles, en orden (qué celdas y en qué orden) ---
        for c in f.table.visible_columns() {
            c.kind.hash(&mut h);
        }

        // --- Conjunto cortado (atenúa la fila). Orden-independiente: XOR de hashes por ruta. ---
        let mut cut_acc: u64 = 0;
        for p in &self.ops.cut_set {
            let mut ph = std::collections::hash_map::DefaultHasher::new();
            p.hash(&mut ph);
            cut_acc ^= ph.finish();
        }
        cut_acc.hash(&mut h);

        // --- Íconos (set activo + tinte + overrides): cambia el ícono de cada fila ---
        self.icons.signature().hash(&mut h);
        self.comparison_revision.hash(&mut h);
        self.visit_changes_revision.hash(&mut h);

        // --- Selección/foco: revisión O(1), incluso con "seleccionar todo" en 100k filas. ---
        f.presentation_revision().hash(&mut h);

        // --- Filtro visual por tipeo: cambia el flag `filter_match` de las filas. Se hashea
        // el buffer CRUDO (la aguja plegada se deriva de él); solo el panel activo se tiñe.
        // Al cambiar de panel activo, ambos paneles recomputan (uno gana tinte, otro lo pierde).
        if self.ws.active_id() == Some(id) {
            self.typeahead.hash(&mut h);
        } else {
            0u8.hash(&mut h);
        }
        // --- Vista profunda: otra fuente de filas (deep_items con depth). ---
        if self.is_deep_active(id) {
            1u8.hash(&mut h); // marca "modo profundo" (distinto de la vista normal)
            let items = self
                .deep_job
                .as_ref()
                .map(|d| d.items.as_slice())
                .unwrap_or(&[]);
            items.len().hash(&mut h);
            // Contenido de cada item visible (mismo razonamiento que la vista normal). La
            // visibilidad se aplica aquí igual que en `rows_of`.
            let vis = self.visibility_flags();
            for (e, depth) in items {
                if !naygo_core::filter::is_visible(
                    e,
                    vis.show_hidden,
                    vis.show_system,
                    vis.hide_dotfiles,
                ) {
                    continue;
                }
                depth.hash(&mut h);
                hash_entry_for_row(e, &mut h);
            }
        } else {
            0u8.hash(&mut h); // marca "vista normal"
                              // Contenido de las entries VISIBLES, en orden de vista. Captura inserción/borrado
                              // (cambia la vista), reordenamiento, y mutación IN SITU (Modified).
                              // La revisión la incrementan el listado y los watchers al mutar `entries`; junto a
                              // sort/filtros evita recorrer/clonar toda la vista en cada tick.
            f.entries_revision().hash(&mut h);
            f.sort.hash(&mut h);
            for (kind, filter) in &f.table.filters {
                kind.hash(&mut h);
                filter.hash(&mut h);
            }
            f.group_new_at_end.hash(&mut h);
            if f.group_new_at_end {
                let mut highlighted_acc = 0u64;
                for path in &f.highlighted {
                    let mut ph = std::collections::hash_map::DefaultHasher::new();
                    path.hash(&mut ph);
                    highlighted_acc ^= ph.finish();
                }
                highlighted_acc.hash(&mut h);
            }
        }

        // --- Resaltado "fresco": si hay alguna fila vigente, NO cachear (el fundido cambia cada
        // tick por diseño). Se evalúa AL FINAL para no pagar el costo si total ya rebota.
        if self.fresh_active_in_pane(id, highlight_secs, now) {
            return None;
        }

        Some(h.finish())
    }

    /// ¿El panel `id` tiene alguna ruta VISIBLE todavía "fresca" (resaltada)? Si la hay, la fila
    /// se pinta resaltada y el resaltado se desvanece con el tiempo (`is_fresh_ro` depende de
    /// `now`), así que la firma no debe cachearse: el llamador reconstruye cada tick. Recorre solo
    /// la vista (no todas las entries) y corta al primer acierto.
    fn fresh_active_in_pane(
        &self,
        id: PaneId,
        highlight_secs: u64,
        now: std::time::Instant,
    ) -> bool {
        if highlight_secs == 0 {
            return false;
        }
        let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) else {
            return false;
        };
        if self.is_deep_active(id) {
            let items = self
                .deep_job
                .as_ref()
                .map(|d| d.items.as_slice())
                .unwrap_or(&[]);
            return items.iter().any(|(e, _)| {
                self.watchers
                    .is_fresh_ro(id.0, &e.path, highlight_secs, now)
            });
        }
        f.view_indices().iter().any(|&real| {
            f.entries.get(real).is_some_and(|e| {
                self.watchers
                    .is_fresh_ro(id.0, &e.path, highlight_secs, now)
            })
        })
    }

    /// Huso local en segundos (para formatear/parsear fechas del filtro de fecha).
    pub(super) fn tz_offset_secs(&self) -> i64 {
        naygo_platform::time::local_utc_offset_secs()
    }

    /// Actualiza el objetivo del preview con el archivo enfocado del Files activo y arranca
    /// el worker si corresponde (debounce vencido). No-op si no hay panel Preview. Devuelve
    /// true si queda trabajo pendiente (para mantener el timer vivo).
    pub fn drive_preview(&mut self, now: std::time::Instant) -> bool {
        if !self.has_purpose(PanePurpose::Preview) {
            // Si se quitó el panel Preview, soltar cualquier worker en vuelo.
            self.preview.set_wanted(None, now);
            return false;
        }
        // El preview sigue la selección del ÚLTIMO panel Files activo (no del panel activo a
        // secas): así, hacer clic en el propio panel de Vista previa (o en el Inspector) no
        // vacía la previsualización — el archivo seleccionado se conserva. Fallback al panel
        // activo si todavía no hay un Files recordado.
        let files_pane = self
            .last_active_files
            .and_then(|id| self.ws.pane(id))
            .and_then(|p| p.files.as_ref())
            .or_else(|| self.ws.active_files());
        let focused_file = files_pane
            .and_then(|f| f.focused_view_entry())
            .filter(|e| e.kind != EntryKind::Directory)
            .map(|e| e.path.clone());
        self.preview.set_wanted(focused_file, now);
        // El toggle global de auto-resaltado controla al worker: se sincroniza antes de lanzarlo.
        self.preview
            .set_auto_highlight(self.config.settings.auto_highlight_code);
        // Las reglas de preview configuradas por el usuario (extensiones añadidas, "tratar como")
        // también se sincronizan antes de lanzar el worker; sin esto el worker se quedaba con los
        // defaults y una extensión nueva como `.sif` nunca se previsualizaba.
        self.preview
            .set_rules(self.config.settings.preview_rules.clone());
        if self.preview.should_start(now) {
            self.preview.start();
        }
        self.preview.busy()
    }

    /// Rota el preview STL/3MF actualmente enfocado. Solo actualiza estado y despacha workers;
    /// nunca realiza I/O en el hilo de UI.
    pub fn preview_orbit(&mut self, delta_x: f32, delta_y: f32) -> bool {
        self.preview.orbit_mesh(delta_x, delta_y)
    }

    /// Acerca/aleja el preview STL/3MF con la rueda o touchpad.
    pub fn preview_zoom(&mut self, wheel_delta: f32) -> bool {
        self.preview.zoom_mesh(wheel_delta)
    }

    /// Restaura la orientación y zoom originales del preview STL/3MF sin volver a leer el disco.
    pub fn preview_reset_camera(&mut self) -> bool {
        self.preview.reset_mesh_camera()
    }

    /// Cancela la lectura/rasterización del preview actual. El worker hace I/O fuera de la UI y
    /// observa su token cooperativo; esta llamada solo corta el canal y actualiza el estado.
    pub fn cancel_preview(&mut self) -> bool {
        self.preview.cancel()
    }

    /// Copia el contenido textual ya cargado al portapapeles de Windows. No se pre-copia al
    /// enfocar: copiar debe ser una acción explícita y no debe pisar el portapapeles del usuario.
    pub fn copy_preview_text(&self, text: &str) {
        if !text.is_empty() {
            let _ = naygo_platform::clipboard::write_text(text);
        }
    }

    /// Refresca (re-lista) la carpeta del panel activo — estilo navegador (F5). No toca el
    /// historial; solo vuelve a leer el disco. Devuelve true si relanzó un listado.
    pub fn refresh_active(&mut self) -> bool {
        let Some(active) = self.active_files_id() else {
            return false;
        };
        let Some(dir) = self
            .ws
            .pane(active)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        else {
            return false;
        };
        // F5 re-lista en el mismo sitio: cancela el deep para no mezclar filas.
        self.cancel_deep_if_navigating(active);
        self.start_listing(active, dir);
        true
    }

    /// Cancela el listado en curso del panel activo (Esc). Lo deja con lo que alcanzó a listar.
    pub fn cancel_active_listing(&mut self) {
        if let Some(active) = self.active_files_id() {
            if let Some(l) = self.listings.get(&active) {
                l.cancel();
            }
        }
    }
}
