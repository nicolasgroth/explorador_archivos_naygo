// Naygo — WorkspaceCtrl: listado de carpetas, footer, carpeta-no-encontrada y filas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use std::time::Instant;

#[cfg(target_os = "linux")]
const MAX_LINUX_FOOTER_DISK_CACHE_ENTRIES: usize = 128;
#[cfg(target_os = "linux")]
const MAX_LINUX_FOOTER_DISK_QUERIES_IN_FLIGHT: usize = 8;

fn insert_bounded_disk_usage(
    cache: &mut std::collections::HashMap<PathBuf, naygo_core::disk::DiskUsage>,
    root: PathBuf,
    usage: naygo_core::disk::DiskUsage,
    max_entries: usize,
) {
    if !cache.contains_key(&root) && cache.len() >= max_entries {
        if let Some(evicted) = cache.keys().next().cloned() {
            cache.remove(&evicted);
        }
    }
    cache.insert(root, usage);
}

fn footer_disk_query_has_slot(
    pending: &std::collections::HashSet<PathBuf>,
    root: &Path,
    max_pending: usize,
) -> bool {
    pending.contains(root) || pending.len() < max_pending
}

/// Resultado del probe async de "carpeta no encontrada" para UN panel. Lo calcula un hilo
/// worker (el `read_dir` puede colgar SEGUNDOS sobre un share de red caído: jamás en la UI).
/// Guarda la carpeta evaluada para descartar resultados obsoletos (el panel navegó mientras
/// el probe corría).
pub(super) struct MissingProbeHit {
    pane: u64,
    dir: PathBuf,
    revision: u64,
    missing: Option<bool>,
    ancestor: Option<PathBuf>,
}

impl WorkspaceCtrl {
    pub fn has_file_listing_work(&self) -> bool {
        !self.listings.is_empty() || !self.pending_file_listings.is_empty()
    }

    fn missing_from_probe_error(error: &std::io::Error) -> Option<bool> {
        matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
        )
        .then_some(true)
    }

    /// El ancestro existente más cercano de `path` (sube hasta encontrar uno que exista; si
    /// ninguno —p. ej. la unidad entera se fue—, devuelve None).
    fn nearest_existing_ancestor(
        path: &std::path::Path,
        token: &naygo_core::cancel::CancellationToken,
    ) -> Option<PathBuf> {
        let mut cur = path.parent();
        while let Some(p) = cur {
            if token.is_cancelled() {
                return None;
            }
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
            Some((dir, _, ancestor)) => {
                ancestor.is_some() && self.pane_current_dir(id).as_deref() == Some(dir.as_path())
            }
            None => false,
        }
    }

    /// Recalcula el caché tras cambios globales (por ejemplo, conexión/expulsión de disco).
    /// Navegar usa `refresh_missing_cache_for` para tocar sólo el destino recién pedido.
    /// El probe adquiere una plaza compartida con Files/árbol antes de crear el hilo; una revisión
    /// y un token cancelable descartan resultados obsoletos. Una syscall de red ya bloqueada no
    /// puede interrumpirse; su permiso queda ocupado hasta que el hilo realmente salga.
    pub fn refresh_missing_cache(&mut self) {
        self.refresh_missing_cache_scope(None);
    }

    /// Revisa únicamente el panel que acaba de navegar; evita volver a tocar shares de los demás.
    pub fn refresh_missing_cache_for(&mut self, id: PaneId) {
        self.refresh_missing_cache_scope(Some(id));
    }

    fn refresh_missing_cache_scope(&mut self, pane: Option<PaneId>) {
        self.missing_probe_revision = self.missing_probe_revision.wrapping_add(1);
        let revision = self.missing_probe_revision;
        self.missing_probe_pending_pane = Some(pane);
        if let Some(cancel) = &self.missing_probe_cancel {
            cancel.cancel();
        }
        if self.missing_probe.is_some() {
            self.missing_probe_refresh_pending = true;
            return;
        }

        // Solo paneles Files; una navegación toma una fotografía de un panel, mientras que
        // conexión/expulsión de dispositivos solicita el conjunto completo.
        let ids: Vec<PaneId> = self
            .ws
            .panes()
            .iter()
            .filter(|p| p.files.is_some())
            .filter(|p| pane.is_none_or(|target| target == p.id))
            .map(|p| p.id)
            .collect();
        if pane.is_none() {
            self.missing_cache.retain(|k, _| {
                self.ws
                    .panes()
                    .iter()
                    .any(|p| p.files.is_some() && p.id.0 == *k)
            });
        }
        let targets: Vec<(u64, PathBuf)> = ids
            .iter()
            .filter_map(|id| self.pane_current_dir(*id).map(|dir| (id.0, dir)))
            .collect();
        let Some(permit) = self.listing_admission.try_acquire() else {
            self.missing_probe_refresh_pending = true;
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let token = naygo_core::cancel::CancellationToken::new();
        let worker_token = token.clone();
        let spawn = std::thread::Builder::new()
            .name("naygo-missing-probe".into())
            .spawn(move || {
                let _permit = permit;
                let mut hits = Vec::with_capacity(targets.len());
                for (pane, dir) in targets {
                    if worker_token.is_cancelled() {
                        break;
                    }
                    let missing = match std::fs::read_dir(&dir) {
                        Ok(_) => Some(false),
                        Err(error) if WorkspaceCtrl::missing_from_probe_error(&error).is_some() => {
                            Some(true)
                        }
                        // PermissionDenied, network errors, and other transient failures do not
                        // prove that the folder is absent; retain the prior cached diagnosis.
                        Err(_) => None,
                    };
                    let ancestor = (missing == Some(true))
                        .then(|| WorkspaceCtrl::nearest_existing_ancestor(&dir, &worker_token))
                        .flatten();
                    if worker_token.is_cancelled() {
                        break;
                    }
                    hits.push(MissingProbeHit {
                        pane,
                        dir,
                        revision,
                        missing,
                        ancestor,
                    });
                }
                let _ = tx.send(hits);
            });
        if let Err(error) = spawn {
            crate::logging::log_line(&format!("no se pudo iniciar probe de carpetas: {error}"));
            self.missing_probe_refresh_pending = false;
            self.missing_probe_pending_pane = None;
            return;
        }
        self.missing_probe = Some(rx);
        self.missing_probe_cancel = Some(token);
        self.missing_probe_refresh_pending = false;
    }

    /// Drena el probe async de "carpeta no encontrada" (sin bloquear) y aplica los resultados
    /// al caché. Un resultado se aplica SOLO si el panel sigue en la misma carpeta que se
    /// evaluó (si navegó mientras el probe corría, el resultado es obsoleto y se descarta:
    /// el refresh de ESA navegación trae el suyo). Devuelve true si NO hay probe en vuelo
    /// (para que el timer pueda dormir).
    pub fn pump_missing_probe(&mut self) -> bool {
        let Some(rx) = self.missing_probe.as_ref() else {
            if self.missing_probe_refresh_pending {
                self.missing_probe_refresh_pending = false;
                let pane = self.missing_probe_pending_pane.take().flatten();
                self.refresh_missing_cache_scope(pane);
                return false;
            }
            return true;
        };
        match rx.try_recv() {
            Ok(hits) => {
                for hit in hits {
                    let still_there = self.pane_current_dir(PaneId(hit.pane)).as_deref()
                        == Some(hit.dir.as_path());
                    if still_there && hit.revision == self.missing_probe_revision {
                        if let Some(missing) = hit.missing {
                            self.missing_cache
                                .insert(hit.pane, (hit.dir, missing, hit.ancestor));
                        }
                    }
                }
                self.missing_probe = None;
                self.missing_probe_cancel = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.missing_probe = None;
                self.missing_probe_cancel = None;
            }
        }
        if self.missing_probe.is_none() && self.missing_probe_refresh_pending {
            self.missing_probe_refresh_pending = false;
            let pane = self.missing_probe_pending_pane.take().flatten();
            self.refresh_missing_cache_scope(pane);
            return false;
        }
        if self.missing_probe.is_none() {
            self.missing_probe_pending_pane = None;
        }
        self.missing_probe.is_none()
    }

    /// Reintentar en el panel `id`: si su carpeta volvió a existir (USB reconectado), re-listar; si
    /// sigue sin existir, el listado del worker conserva el aviso. El chequeo de disco nunca se
    /// hace en este callback.
    pub fn missing_folder_retry(&mut self, id: PaneId) {
        crate::logging::log_line(&format!("carpeta no encontrada: reintentar panel {}", id.0));
        if let Some(dir) = self.pane_current_dir(id) {
            self.cancel_deep_if_navigating(id);
            self.start_listing(id, dir.clone());
            self.sync_trees_for_files(id, dir);
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
        let cached_ancestor = self
            .missing_cache
            .get(&id.0)
            .filter(|(probed, _, _)| probed == &lost)
            .and_then(|(_, _, ancestor)| ancestor.clone());
        let dest = cached_ancestor.unwrap_or_else(|| self.configured_home_dir());
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
            let home = self.configured_home_dir();
            self.cancel_deep_if_navigating(id);
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.navigate_to(home.clone());
            }
            self.start_listing(id, home);
        }
    }

    /// Arranca el listado del panel `id` en `dir` (cancela el suyo anterior).
    pub fn start_listing(&mut self, id: PaneId, dir: std::path::PathBuf) {
        if let Some(l) = self.listings.remove(&id) {
            l.cancel();
        }
        self.pending_file_listings.remove(&id);
        // Una comparación describe exactamente los snapshots cargados; navegar o refrescar uno
        // de sus lados la invalida para no dejar marcas obsoletas sobre nombres nuevos.
        if self.comparison.remove(&id).is_some() {
            self.comparison.clear();
            self.comparison_revision = self.comparison_revision.wrapping_add(1);
        }
        let changes_removed = self.visit_changes.remove(&id).is_some();
        let highlights_removed = self.visit_highlights.remove(&id).is_some();
        if changes_removed || highlights_removed {
            self.visit_changes_revision = self.visit_changes_revision.wrapping_add(1);
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
        self.dirty_listings.remove(&id);
        if !cambia_carpeta {
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.preserve_context_for_listing();
            }
        }
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
        // Navegar puede cambiar de unidad. Windows cachea por raíz; Linux consulta la carpeta
        // actual para que statvfs identifique su montaje. Los dispositivos invalidan la caché
        // explícitamente en `invalidate_footer_disk_cache`.
        // Si el panel estaba marcado como "expulsado" y ahora navega a una carpeta válida,
        // limpiar el flag para que el aviso vuelva a ser el genérico si vuelve a desconectarse.
        self.ejected_panes.remove(&id.0);
        match Listing::try_start(
            dir.clone(),
            &self.listing_admission,
            naygo_core::listing::ListingFilter::All,
        ) {
            Ok(listing) => {
                self.listings.insert(id, listing);
            }
            Err(naygo_core::listing::ListingStartError::Capacity) => {
                self.pending_file_listings.insert(id, dir);
            }
            Err(naygo_core::listing::ListingStartError::Spawn(error)) => {
                crate::logging::log_line(&format!("no se pudo iniciar listado: {error}"));
            }
        }
        // Recalcular el estado "carpeta no encontrada" tras navegar (sale del tick de render).
        self.refresh_missing_cache_for(id);
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
                #[cfg(target_os = "linux")]
                let max_entries = MAX_LINUX_FOOTER_DISK_CACHE_ENTRIES;
                #[cfg(not(target_os = "linux"))]
                let max_entries = usize::MAX;
                insert_bounded_disk_usage(&mut self.footer_disk_cache, root, usage, max_entries);
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
        #[cfg(target_os = "linux")]
        let max_pending = MAX_LINUX_FOOTER_DISK_QUERIES_IN_FLIGHT;
        #[cfg(not(target_os = "linux"))]
        let max_pending = usize::MAX;
        if !footer_disk_query_has_slot(&self.footer_disk_pending, root, max_pending) {
            return None;
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
        // Paso 2 (ya sin borrow de `self.ws`): uso de disco cacheado por clave de volumen/ruta. Si
        // falta, se solicita a un worker y este tick conserva una respuesta inmediata.
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
    #[cfg(test)]
    pub fn pump_listings(&mut self) -> bool {
        let mut budget = PollBudget::new();
        let mut allowance = 1_024;
        self.pump_listings_with_budget(&mut budget, &mut allowance)
    }

    pub(crate) fn pump_listings_with_budget(
        &mut self,
        budget: &mut PollBudget,
        allowance: &mut usize,
    ) -> bool {
        self.start_pending_file_listings();
        // Flags de visibilidad globales: se empujan al panel al sembrar sus entries, así un
        // panel recién listado (arranque, navegación, refresh) ya nace con la vista filtrada
        // correcta sin depender de que el usuario abra el menú "ojo".
        let vis = self.visibility_flags();
        // Espejo del setting "archivos nuevos al final": se empuja en el mismo punto, así un panel
        // recién listado refleja la opción aunque el watcher aún no haya disparado.
        let group_new_at_end = self.config.settings.new_items_at_end;
        let mut ids: Vec<PaneId> = self.listings.keys().copied().collect();
        if !ids.is_empty() {
            let start = self.file_listing_cursor % ids.len();
            ids.rotate_left(start);
            self.file_listing_cursor = (start + 1) % ids.len();
        }
        for id in ids {
            if *allowance == 0 || budget.remaining_entries == 0 || Instant::now() >= budget.deadline
            {
                break;
            }
            // El flag `fresh` se reclama solo cuando este poll trae avance real: o llegaron
            // entries, o el listado TERMINÓ (carpeta vacía → done sin lotes). Un tick que poll-ea
            // vacío y sin terminar NO lo consume, así el reemplazo de filas se aplica recién con
            // el primer avance real. `take_fresh()` solo devuelve `true` una vez por listado.
            let (batch, terminal, fresh) = match self.listings.get_mut(&id) {
                Some(l) => {
                    let (b, terminal) = l.poll_with_shared_budget(budget, allowance);
                    let fresh = if b.is_empty() && terminal.is_none() {
                        false
                    } else {
                        l.take_fresh()
                    };
                    (b, terminal, fresh)
                }
                None => continue,
            };
            let done = terminal.is_some();
            let succeeded = matches!(
                terminal,
                Some(naygo_core::listing::ListingOutcome::Complete)
            );
            let failed = matches!(
                terminal,
                Some(
                    naygo_core::listing::ListingOutcome::Failed(_)
                        | naygo_core::listing::ListingOutcome::Partial { .. }
                )
            );
            let batch_was_empty = batch.is_empty();
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.set_visibility(vis);
                f.group_new_at_end = group_new_at_end;
                if !batch_was_empty {
                    // Primer lote de un listado nuevo: REEMPLAZAR (no acumular) las entries que
                    // el panel tuviera de antes. Esto evita que F5 duplique las filas al
                    // re-listar el mismo directorio sobre un panel ya poblado.
                    let append_start = if fresh {
                        f.entries.clear();
                        0
                    } else {
                        f.entries.len()
                    };
                    f.entries.extend(batch);
                    f.entries_appended(append_start);
                }
                if done {
                    // Carpeta que quedó VACÍA tras refrescar: el listado nuevo no emitió ningún
                    // lote, así que `fresh` nunca se reclamó y las entries viejas seguirían ahí.
                    // Al terminar, si aún estaba fresco, vaciar para reflejar la carpeta vacía.
                    let old_rows_belong_here = f
                        .entries
                        .iter()
                        .all(|entry| entry.path.parent() == Some(f.current_dir.as_path()));
                    if fresh && batch_was_empty && (succeeded || !old_rows_belong_here) {
                        f.entries.clear();
                        f.entries_changed();
                    }
                    let spec = f.sort;
                    naygo_core::sort::sort_entries(&mut f.entries, &spec);
                    // El último tick pudo cachear índices del orden físico anterior.
                    f.entries_changed();
                    let restored = f.apply_pending_nav_context();
                    if !restored && f.focused.is_none() && !f.entries.is_empty() {
                        f.focused = Some(0);
                        f.presentation_changed();
                    }
                }
            }
            if done {
                if succeeded {
                    self.record_listing_changes(id);
                } else if failed {
                    if let Some(outcome) = terminal.as_ref() {
                        let detail = match outcome {
                            naygo_core::listing::ListingOutcome::Failed(error) => {
                                error.message.clone()
                            }
                            naygo_core::listing::ListingOutcome::Partial {
                                skipped,
                                first_error,
                            } => format!("{} ({} omitida(s))", first_error.message, skipped),
                            _ => String::new(),
                        };
                        let dir = self.pane_current_dir(id).unwrap_or_default();
                        crate::logging::log_line(&format!(
                            "listado incompleto en {}: {}",
                            dir.display(),
                            detail
                        ));
                        self.pending_listing_error = Some(format!(
                            "{}: {}",
                            self.config.t("listing.incomplete"),
                            detail
                        ));
                    }
                }
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
                if self.dirty_listings.remove(&id) {
                    if let Some(dir) = self.pane_current_dir(id) {
                        self.start_listing(id, dir);
                    }
                }
            }
        }
        self.listings.is_empty() && self.pending_file_listings.is_empty()
    }

    fn start_pending_file_listings(&mut self) {
        let mut ids: Vec<_> = self.pending_file_listings.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        for id in ids {
            let Some(dir) = self.pending_file_listings.remove(&id) else {
                continue;
            };
            match Listing::try_start(
                dir.clone(),
                &self.listing_admission,
                naygo_core::listing::ListingFilter::All,
            ) {
                Ok(listing) => {
                    self.listings.insert(id, listing);
                }
                Err(naygo_core::listing::ListingStartError::Capacity) => {
                    self.pending_file_listings.insert(id, dir);
                    break;
                }
                Err(naygo_core::listing::ListingStartError::Spawn(error)) => {
                    crate::logging::log_line(&format!(
                        "no se pudo iniciar listado pendiente: {error}"
                    ));
                }
            }
        }
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
            .map(|before| naygo_core::listing_changes::compare_snapshot(&before.entries, &entries))
            .unwrap_or_default();
        self.listing_snapshots.insert(dir, &entries);
        let highlights = naygo_core::listing_changes::highlight_paths(&changes);
        if changes.is_empty() {
            self.visit_changes.remove(&id);
            self.visit_highlights.remove(&id);
        } else {
            self.visit_changes.insert(id, changes);
            self.visit_highlights.insert(id, highlights);
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
            let view = f.view_indices_shared();
            for (row, &real) in rows.iter_mut().zip(view.iter()) {
                if let Some(entry) = f.entries.get(real) {
                    row.compare_state = marks.get(&entry.path).copied().unwrap_or(0);
                }
            }
        }
        if let (Some(highlights), Some(files)) = (
            self.visit_highlights.get(&id),
            self.ws.pane(id).and_then(|pane| pane.files.as_ref()),
        ) {
            let view = files.view_indices_shared();
            for (row, &real) in rows.iter_mut().zip(view.iter()) {
                if let Some(entry) = files.entries.get(real) {
                    row.highlight |= highlights.contains(&entry.path);
                }
            }
        }
        if filter_needle.is_some() {
            self.filter_match_count = rows.iter().filter(|r| r.filter_match).count();
        }
        rows
    }

    /// Construye solo las posiciones indicadas de la vista normal. Devuelve `None` cuando la
    /// presentación depende de un estado dinámico/global que requiere reconstruir todo el panel.
    pub(crate) fn rows_at(
        &mut self,
        id: PaneId,
        positions: &[usize],
        highlight_secs: u64,
        now: Instant,
    ) -> Option<Vec<PlainRow>> {
        if self.is_deep_active(id) {
            return None;
        }
        let filter_needle = self.active_filter_needle(id);
        if filter_needle.is_some() {
            return None;
        }
        let size_format = self.config.settings.size_format;
        let date_format = self.config.settings.date_format;
        let tz = naygo_platform::time::local_utc_offset_secs();
        let mut rows = {
            let WorkspaceCtrl {
                ws,
                ops,
                watchers,
                icons,
                ..
            } = self;
            let f = ws.pane(id)?.files.as_ref()?;
            rows_from_view_positions(
                f,
                positions.iter().copied(),
                &|path| ops.is_cut(path),
                &|path| watchers.is_fresh_ro(id.0, path, highlight_secs, now),
                &mut |entry| icons.get_for_entry(entry),
                size_format,
                date_format,
                tz,
                None,
            )
        };
        if let (Some(marks), Some(f)) = (
            self.comparison.get(&id),
            self.ws.pane(id).and_then(|pane| pane.files.as_ref()),
        ) {
            let view = f.view_indices_shared();
            for (row, &position) in rows.iter_mut().zip(positions) {
                if let Some(entry) = view.get(position).and_then(|&real| f.entries.get(real)) {
                    row.compare_state = marks.get(&entry.path).copied().unwrap_or(0);
                }
            }
        }
        if let (Some(highlights), Some(f)) = (
            self.visit_highlights.get(&id),
            self.ws.pane(id).and_then(|pane| pane.files.as_ref()),
        ) {
            let view = f.view_indices_shared();
            for (row, &position) in rows.iter_mut().zip(positions) {
                if let Some(entry) = view.get(position).and_then(|&real| f.entries.get(real)) {
                    row.highlight |= highlights.contains(&entry.path);
                }
            }
        }
        Some(rows)
    }

    /// Traduce un delta de rutas del watcher a posiciones visibles. Solo es seguro si no puede
    /// alterar estructura, orden, filtrado o estado animado; los demás casos vuelven al listado
    /// completo. El barrido compara rutas, sin construir ni convertir filas.
    pub(crate) fn dirty_row_positions(
        &self,
        id: PaneId,
        previous_revision: u64,
        highlight_secs: u64,
        now: Instant,
    ) -> Option<Vec<usize>> {
        if self.is_deep_active(id) || self.active_filter_needle(id).is_some() {
            return None;
        }
        if self.fresh_active_in_pane(id, highlight_secs, now) {
            return None;
        }
        let files = self.ws.pane(id)?.files.as_ref()?;
        if !files.table.filters.is_empty()
            || !matches!(files.sort.key, SortKey::Name | SortKey::Extension)
        {
            return None;
        }
        let changed_paths = files.entries_rows_changed_since(previous_revision)?;
        if changed_paths.is_empty() {
            return None;
        }
        let changed_paths: std::collections::HashSet<&std::path::Path> =
            changed_paths.iter().map(PathBuf::as_path).collect();
        let view = files.view_indices_shared();
        let mut positions = Vec::new();
        for (position, &real) in view.iter().enumerate() {
            if files
                .entries
                .get(real)
                .is_some_and(|entry| changed_paths.contains(entry.path.as_path()))
            {
                positions.push(position);
            }
        }
        (positions.len() == changed_paths.len()).then_some(positions)
    }

    /// Cantidad actual de filas normales sin construirlas ni clonarlas.
    pub(crate) fn row_source_count(&self, id: PaneId) -> Option<usize> {
        if self.is_deep_active(id) {
            return None;
        }
        self.ws
            .pane(id)?
            .files
            .as_ref()
            .map(|files| files.view_indices_shared().len())
    }

    /// Revisión de las entradas usada para correlacionar deltas del watcher con filas visibles.
    pub(crate) fn entries_revision_of(&self, id: PaneId) -> Option<u64> {
        if self.is_deep_active(id) {
            return None;
        }
        self.ws
            .pane(id)?
            .files
            .as_ref()
            .map(|files| files.entries_revision())
    }

    /// Número de entradas fuente antes de visibilidad/filtros. Se guarda junto con la revisión
    /// que ya representa la UI para distinguir un append real de un refresco que reemplazó el
    /// contenido de la carpeta.
    pub(crate) fn row_entry_count(&self, id: PaneId) -> Option<usize> {
        if self.is_deep_active(id) {
            return None;
        }
        self.ws
            .pane(id)?
            .files
            .as_ref()
            .map(|files| files.entries.len())
    }

    /// Índices compartidos de la última vista calculada. Clonar el Arc es O(1); la sincronización
    /// usa el snapshot previo para demostrar que los índices antiguos siguen siendo prefijo.
    pub(crate) fn row_view_indices_shared(&self, id: PaneId) -> Option<std::sync::Arc<[usize]>> {
        if self.is_deep_active(id) {
            return None;
        }
        self.ws
            .pane(id)?
            .files
            .as_ref()
            .map(|files| files.view_indices_shared())
    }

    /// Rango físico de los lotes append-only recibidos desde una revisión.
    pub(crate) fn rows_appended_since(
        &self,
        id: PaneId,
        revision: u64,
        expected_start: usize,
    ) -> Option<usize> {
        if self.is_deep_active(id) {
            return None;
        }
        self.ws
            .pane(id)?
            .files
            .as_ref()?
            .entries_appended_since(revision, expected_start)
    }

    /// Firma del estado y revisiones que determinan las filas pintadas del panel `id`.
    /// Para la vista normal, la revisión de entries detecta cambios en O(1); no se recorren ni se
    /// convierten filas si la firma no cambia. La vista profunda incorpora el contenido de sus
    /// items porque su fuente no comparte las revisiones del listado normal.
    ///
    /// Devuelve `None` cuando NO se debe cachear: el panel tiene resaltado "fresco" vigente, que
    /// se desvanece por diseño cada tick (`is_fresh_ro` depende de `now`). En ese caso el llamador
    /// reconstruye siempre para que el fundido se vea. `None` también para paneles no-Files.
    ///
    /// La firma cubre el estado global de presentación y las revisiones de entries/selección;
    /// para cambios puntuales de metadata el sync usa un delta de rutas del watcher y construye
    /// las posiciones visibles afectadas. Cambios estructurales o contextos no compatibles
    /// conservan la reconciliación completa/incremental.
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
        self.rows_signature_impl(id, highlight_secs, now, true)
    }

    /// Firma global sin la revisión de entradas. El sync la combina con deltas explícitos del
    /// watcher para actualizar solo filas modificadas sin recorrer ni hashear el listado.
    pub(crate) fn rows_context_signature(
        &self,
        id: PaneId,
        highlight_secs: u64,
        now: Instant,
    ) -> Option<u64> {
        if self.is_deep_active(id) || self.active_filter_needle(id).is_some() {
            return None;
        }
        self.rows_signature_impl(id, highlight_secs, now, false)
    }

    fn rows_signature_impl(
        &self,
        id: PaneId,
        highlight_secs: u64,
        now: std::time::Instant,
        include_entries_revision: bool,
    ) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        let f = self.ws.pane(id).and_then(|p| p.files.as_ref())?;

        let mut h = std::collections::hash_map::DefaultHasher::new();

        // La ruta del panel forma parte de la identidad del contenido y evita reaplicar deltas
        // de una carpeta después de navegar a otra.
        f.current_dir.hash(&mut h);

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
            if include_entries_revision {
                f.entries_revision().hash(&mut h);
            }
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
    /// Visibilidad real: ventana no minimizada/oculta y al menos un preview presentado
    /// (no basta que exista detrás de otra pestaña o de un panel maximizado).
    pub fn drive_preview_visible(&mut self, now: std::time::Instant, window_visible: bool) -> bool {
        let groups = self.tab_groups();
        let shown = self.pane_rects(self.last_area).iter().any(|(id, _)| {
            self.ws
                .pane(*id)
                .is_some_and(|p| p.purpose == PanePurpose::Preview)
                && groups.iter().all(|(members, active)| {
                    !members.contains(id) || members.get(*active) == Some(id)
                })
        });
        self.preview.set_visible(window_visible && shown, now);
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
        let focused_file = if self.search_context {
            self.search_selected_entry()
                .filter(|e| e.kind != EntryKind::Directory)
                .map(|e| e.path.clone())
        } else {
            self.basket_selected_path().or_else(|| {
                files_pane
                    .and_then(|f| f.focused_view_entry())
                    .filter(|e| e.kind != EntryKind::Directory)
                    .map(|e| e.path.clone())
            })
        };
        self.preview.set_wanted(focused_file, now);
        self.preview
            .set_economical(self.config.settings.preview_economical, now);
        // El toggle global de auto-resaltado controla al worker: se sincroniza antes de lanzarlo.
        self.preview
            .set_auto_highlight(self.config.settings.auto_highlight_code);
        // Las reglas de preview configuradas por el usuario (extensiones añadidas, "tratar como")
        // también se sincronizan antes de lanzar el worker; sin esto el worker se quedaba con los
        // defaults y una extensión nueva como `.sif` nunca se previsualizaba.
        self.preview
            .set_rules(self.config.settings.preview_rules.clone());
        self.preview
            .set_text_max_bytes(self.config.settings.preview_text_max_bytes);
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
    pub fn copy_preview_text(&mut self, text: &str) {
        if !text.is_empty() {
            let _ = self.ops.write_clipboard_text(text);
        }
    }

    /// Refresca (re-lista) la carpeta del panel activo — estilo navegador (F5). No toca el
    /// historial; solo vuelve a leer el disco. Devuelve true si relanzó un listado.
    pub fn refresh_active(&mut self) -> bool {
        let Some(active) = self.active_files_id() else {
            return false;
        };
        self.refresh_pane(active)
    }

    /// Reconciliar operaciones terminadas incluso si el watcher no avisó (red/registro tardío).
    /// Seleccionar creaciones sólo en paneles que aún muestran su destino; nunca navegar ni
    /// robar foco a otro panel. Todo el I/O queda en los workers de listado.
    pub fn reconcile_completed_operations(&mut self) {
        use naygo_core::ops::OpKind;
        let changes = self.ops.take_completed_changes();
        if changes.is_empty() {
            return;
        }
        let mut dirs = std::collections::HashSet::new();
        let mut created = std::collections::HashMap::<PathBuf, Vec<PathBuf>>::new();
        for (req, success) in changes {
            for source in &req.sources {
                if let Some(parent) = source.parent() {
                    dirs.insert(parent.to_path_buf());
                }
            }
            if let Some(dir) = req.dest_dir {
                if success {
                    if let OpKind::CreateDir { name } = req.kind {
                        // En creación anidada se revela el primer nivel visible en este panel.
                        if let Some(first) = std::path::Path::new(&name).components().next() {
                            created
                                .entry(dir.clone())
                                .or_default()
                                .push(dir.join(first));
                        }
                    }
                }
                dirs.insert(dir);
            }
        }
        let panes: Vec<_> = self
            .ws
            .panes()
            .iter()
            .filter_map(|p| {
                p.files
                    .as_ref()
                    .filter(|f| dirs.contains(&f.current_dir))
                    .map(|f| (p.id, f.current_dir.clone()))
            })
            .collect();
        for (id, dir) in panes {
            if let Some(paths) = created.get(&dir) {
                if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                    f.reveal_after_listing(paths.clone());
                }
            }
            self.start_listing(id, dir.clone());
            self.sync_trees_for_files(id, dir);
        }
        for dir in dirs {
            self.refresh_tree_directory(&dir);
        }
    }

    /// Refrescar sin cambiar foco ni historial; rearmar también un watcher fallido.
    pub fn refresh_pane(&mut self, active: PaneId) -> bool {
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
        self.watchers.unwatch(active.0);
        self.start_listing(active, dir);
        true
    }

    /// Incluye pestañas abiertas y ramas expandidas, nunca recorre todo el disco.
    pub fn refresh_all(&mut self) {
        let ids: Vec<_> = self
            .ws
            .panes()
            .iter()
            .filter(|p| p.files.is_some())
            .map(|p| p.id)
            .collect();
        for id in ids {
            self.refresh_pane(id);
        }
        self.refresh_trees_visibility();
        self.refresh_missing_cache();
    }

    /// Cancela el listado en curso del panel activo (Esc). Lo deja con lo que alcanzó a listar.
    pub fn cancel_active_listing(&mut self) {
        if let Some(active) = self.active_files_id() {
            self.dirty_listings.remove(&active);
            if let Some(l) = self.listings.get(&active) {
                l.cancel();
            }
        }
    }
}

#[cfg(test)]
mod listing_probe_tests {
    use super::{footer_disk_query_has_slot, insert_bounded_disk_usage, WorkspaceCtrl};
    use std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
    };

    #[test]
    fn cache_de_espacio_no_supera_el_limite_y_acepta_la_ruta_actualizada() {
        let mut cache = HashMap::new();
        let usage = naygo_core::disk::DiskUsage {
            total: 100,
            free: 50,
        };
        insert_bounded_disk_usage(&mut cache, PathBuf::from("/a"), usage, 2);
        insert_bounded_disk_usage(&mut cache, PathBuf::from("/b"), usage, 2);
        insert_bounded_disk_usage(&mut cache, PathBuf::from("/c"), usage, 2);
        assert_eq!(cache.len(), 2);
        insert_bounded_disk_usage(
            &mut cache,
            PathBuf::from("/c"),
            naygo_core::disk::DiskUsage {
                total: 200,
                free: 75,
            },
            2,
        );
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&PathBuf::from("/c")).unwrap().free, 75);
    }

    #[test]
    fn consultas_de_espacio_respetan_tope_sin_bloquear_ruta_ya_pendiente() {
        let pending = HashSet::from([PathBuf::from("/a"), PathBuf::from("/b")]);
        assert!(!footer_disk_query_has_slot(
            &pending,
            std::path::Path::new("/c"),
            2
        ));
        assert!(footer_disk_query_has_slot(
            &pending,
            std::path::Path::new("/a"),
            2
        ));
        assert!(footer_disk_query_has_slot(
            &pending,
            std::path::Path::new("/c"),
            3
        ));
    }

    #[test]
    fn permiso_denegado_no_se_diagnostica_como_carpeta_ausente() {
        let error = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(WorkspaceCtrl::missing_from_probe_error(&error), None);
    }

    #[test]
    fn ruta_inexistente_se_diagnostica_como_carpeta_ausente() {
        let error = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert_eq!(WorkspaceCtrl::missing_from_probe_error(&error), Some(true));
    }
}
