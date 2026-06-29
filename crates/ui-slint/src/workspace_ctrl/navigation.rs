// Naygo — WorkspaceCtrl: clics, navegación, tamaño de carpeta, búsqueda, historial y vista profunda.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
    // --- Gestos sobre el panel ACTIVO (reusan la logica de F1) ---

    /// Clic en una fila. Selecciona (respetando Ctrl/Shift) y DETECTA el doble-clic en Rust:
    /// si este clic cae en el mismo panel+fila que el anterior dentro de la ventana de
    /// tiempo, lo trata como doble-clic (navega/abre) y devuelve true. Esto no depende del
    /// `double-clicked` de Slint, que bajo el renderizador por software puede no dispararse.
    pub fn on_row_clicked(&mut self, id: PaneId, pos: usize, now: std::time::Instant) -> bool {
        // Ventana amplia (700 ms): bajo render por software en una VM, el segundo `clicked`
        // puede llegar al hilo de UI con latencia; un umbral chico se pierde el doble-clic.
        const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(700);
        let is_double = !self.opened_recently(now)
            && matches!(
                self.last_click,
                Some((lid, lpos, t)) if lid == id && lpos == pos && now.duration_since(t) <= DOUBLE_CLICK
            );
        if is_double {
            self.last_click = None; // un triple clic no encadena dos navegaciones
            self.last_open = Some(now);
            return self.on_row_double_clicked(id, pos);
        }
        self.last_click = Some((id, pos, now));
        self.ws.set_active(id);
        let (ctrl, shift) = (self.ctrl_down, self.shift_down);
        if let Some(f) = self.ws.active_files_mut() {
            if shift {
                f.select_range_to(pos);
            } else if ctrl {
                f.select_toggle(pos);
            } else {
                f.select_single(pos);
            }
        }
        false
    }

    /// Selección por rectángulo (rubber-band, 6F): selecciona el rango inclusivo de posiciones
    /// de vista [from_pos, to_pos] del panel `id`. `additive` (Ctrl) suma a la selección previa;
    /// si no, la reemplaza. Reusa `FilePaneState::select_rect`. La UI calcula from/to por la `y`
    /// del arrastre (filas de alto fijo).
    pub fn select_rect_range(&mut self, id: PaneId, from_pos: i32, to_pos: i32, additive: bool) {
        self.ws.set_active(id);
        let Some(f) = self.ws.active_files_mut() else {
            return;
        };
        let len = f.view_len() as i32;
        if len == 0 {
            return;
        }
        let lo = from_pos.min(to_pos).clamp(0, len - 1) as usize;
        let hi = from_pos.max(to_pos).clamp(0, len - 1) as usize;
        let positions: Vec<usize> = (lo..=hi).collect();
        f.select_rect(&positions, additive);
    }

    /// ¿Se abrió algo por doble-clic hace muy poco? (Para que los dos detectores no
    /// naveguen dos veces sobre el mismo gesto.)
    fn opened_recently(&self, now: std::time::Instant) -> bool {
        self.last_open
            .map(|t| now.duration_since(t) <= std::time::Duration::from_millis(500))
            .unwrap_or(false)
    }

    /// Doble clic NATIVO de Slint (camino primario, cronometrado por el SO). Si el detector
    /// por tiempo de Rust no se adelantó en este mismo gesto, navega y estampa.
    pub fn on_row_double_clicked_native(&mut self, id: PaneId, pos: usize) -> bool {
        let now = std::time::Instant::now();
        if self.opened_recently(now) {
            return false; // ya navegó el detector por tiempo en este gesto
        }
        self.last_open = Some(now);
        self.last_click = None;
        self.on_row_double_clicked(id, pos)
    }

    /// Doble clic en el panel `id`, posición `pos`. Navega (y arranca listado) o abre. Con
    /// Ctrl presionado y una CARPETA, la abre en OTRO panel (el origen no navega): resuelve
    /// el destino usando el último área conocida (directo / selector 1..9 / dividir).
    /// En modo deep las filas vienen de `deep_items` (mismo orden que `rows_of` devuelve),
    /// así que `pos` mapea directamente contra ese slice.
    pub fn on_row_double_clicked(&mut self, id: PaneId, pos: usize) -> bool {
        self.ws.set_active(id);

        // En modo deep: resolver la entrada desde deep_items (fuente de filas en ese modo).
        if self.is_deep_active(id) {
            let target = self
                .deep_job
                .as_ref()
                .and_then(|d| d.items.get(pos))
                .map(|(e, _depth)| e.clone());
            let Some(e) = target else { return false };
            if e.kind == naygo_core::fs_model::EntryKind::Directory {
                if self.ctrl_down {
                    return self.request_action(PaneAction::OpenDir(e.path), id, self.last_area);
                }
                // Navegar cancela la vista profunda (no es pegajosa).
                self.cancel_deep_if_navigating(id);
                if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                    f.navigate_to(e.path.clone());
                }
                self.push_recent(e.path.clone());
                self.start_listing(id, e.path.clone());
                self.sync_trees_active(e.path);
                return true;
            } else {
                let _ = naygo_platform::open::open_default(&e.path);
                return false;
            }
        }

        let target = {
            let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) else {
                return false;
            };
            let view = f.view_indices();
            let Some(&real) = view.get(pos) else {
                return false;
            };
            f.entries.get(real).cloned()
        };
        let Some(e) = target else { return false };
        if e.kind == EntryKind::Directory {
            // Ctrl+doble-clic en carpeta → abrir en otro panel (el origen no navega).
            if self.ctrl_down {
                return self.request_action(PaneAction::OpenDir(e.path), id, self.last_area);
            }
            // Navegar cancela la vista profunda del panel (no es pegajosa).
            self.cancel_deep_if_navigating(id);
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.navigate_to(e.path.clone());
            }
            self.push_recent(e.path.clone());
            self.start_listing(id, e.path.clone());
            self.sync_trees_active(e.path);
            true
        } else {
            let _ = naygo_platform::open::open_default(&e.path);
            false
        }
    }

    /// Subir al padre en el panel activo (y arranca su listado).
    pub fn on_go_up(&mut self) -> bool {
        let active = match self.ws.active_id() {
            Some(a) => a,
            None => return false,
        };
        let moved = self.ws.active_files_mut().and_then(|f| f.go_up());
        match moved {
            Some(dir) => {
                crate::logging::breadcrumb("subir un nivel");
                // Navegar cancela la vista profunda del panel (no es pegajosa).
                self.cancel_deep_if_navigating(active);
                self.push_recent(dir.clone());
                self.start_listing(active, dir.clone());
                self.sync_trees_active(dir);
                true
            }
            None => false,
        }
    }

    /// Atrás en el historial del panel activo (Alt+← / botón de mouse «atrás»). Devuelve true si
    /// se movió (relanza el listado y resalta en el árbol). Mismo patrón que `on_go_up`.
    pub fn on_go_back(&mut self) -> bool {
        let Some(active) = self.active_files_id() else {
            return false;
        };
        let moved = self
            .ws
            .pane_mut(active)
            .and_then(|p| p.files.as_mut())
            .and_then(|f| f.go_back());
        match moved {
            Some(dir) => {
                crate::logging::breadcrumb("atrás");
                // Navegar cancela la vista profunda del panel (no es pegajosa).
                self.cancel_deep_if_navigating(active);
                self.push_recent(dir.clone());
                self.start_listing(active, dir.clone());
                self.sync_trees_active(dir);
                true
            }
            None => false,
        }
    }

    /// Adelante en el historial del panel activo (Alt+→ / botón de mouse «adelante»).
    pub fn on_go_forward(&mut self) -> bool {
        let Some(active) = self.active_files_id() else {
            return false;
        };
        let moved = self
            .ws
            .pane_mut(active)
            .and_then(|p| p.files.as_mut())
            .and_then(|f| f.go_forward());
        match moved {
            Some(dir) => {
                crate::logging::breadcrumb("adelante");
                // Navegar cancela la vista profunda del panel (no es pegajosa).
                self.cancel_deep_if_navigating(active);
                self.push_recent(dir.clone());
                self.start_listing(active, dir.clone());
                self.sync_trees_active(dir);
                true
            }
            None => false,
        }
    }

    /// Navega el panel Files activo a la carpeta Home configurada (vacío = perfil del usuario).
    /// Registra en el historial igual que cualquier navegación normal. Devuelve true si navegó.
    pub fn on_go_home(&mut self) -> bool {
        let home = naygo_core::config::resolve_home_dir(&self.config.settings.home_dir);
        crate::logging::breadcrumb(&format!("home → {}", home.display()));
        self.navigate_active_to(home)
    }

    /// ¿El panel activo puede ir Atrás? Lo consumen los botones Atrás/Adelante del toolbar
    /// (main.rs los lee tras cada navegación para habilitarlos/deshabilitarlos).
    pub fn can_go_back(&self) -> bool {
        self.ws
            .active_files()
            .map(|f| f.can_go_back())
            .unwrap_or(false)
    }

    /// ¿El panel activo puede ir Adelante? Mismo consumidor que `can_go_back`.
    pub fn can_go_forward(&self) -> bool {
        self.ws
            .active_files()
            .map(|f| f.can_go_forward())
            .unwrap_or(false)
    }

    /// Cierra la ayuda (Esc/clic fuera).
    pub fn help_close(&mut self) {
        self.help_open = false;
    }

    // --- Calcular tamaño de carpeta (F3) ---

    /// Lanza el cálculo del tamaño de la carpeta enfocada/seleccionada del panel activo (o, si lo
    /// enfocado es un archivo, la carpeta del panel). Cancela cualquier cálculo anterior. Respeta
    /// el ajuste "no bajar a subdirectorios". El resultado se ve en la barra de estado.
    pub fn compute_size_active(&mut self) {
        // Carpeta objetivo: el ítem enfocado si es carpeta; si no, la carpeta del panel.
        let target = self
            .ws
            .active_files()
            .and_then(|f| f.focused_view_entry())
            .filter(|e| e.kind == EntryKind::Directory)
            .map(|e| e.path.clone())
            .or_else(|| self.ws.active_files().map(|f| f.current_dir.clone()));
        let Some(target) = target else {
            return;
        };
        // Cancelar el job anterior.
        if let Some(job) = self.size_job.take() {
            job.token.cancel();
        }
        let recursive = !self.config.settings.size_no_subdirs;
        let token = naygo_core::CancellationToken::new();
        let rx = naygo_core::sizing::spawn_dir_size(target.clone(), recursive, token.clone());
        self.size_job = Some(SizeJob {
            target,
            rx,
            token,
            bytes: 0,
            done: false,
            partial: false,
            cancelled: false,
        });
    }

    /// Drena el worker de tamaño en vuelo (si lo hay), actualizando bytes/estado. Devuelve true
    /// si NO queda cálculo activo pendiente (para que el timer pueda apagarse). El job terminado
    /// se conserva para que la barra de estado muestre el resultado hasta el próximo F3/navegar.
    pub fn pump_sizes(&mut self) -> bool {
        let Some(job) = self.size_job.as_mut() else {
            return true;
        };
        if job.done {
            return true;
        }
        use naygo_core::sizing::SizeMsg;
        // Vaciar todo lo que llegó sin bloquear.
        while let Ok(msg) = job.rx.try_recv() {
            match msg {
                SizeMsg::Progress { bytes } => job.bytes = bytes,
                SizeMsg::Done { total, partial } => {
                    job.bytes = total;
                    job.partial = partial;
                    job.done = true;
                }
                SizeMsg::Cancelled { bytes } => {
                    job.bytes = bytes;
                    job.cancelled = true;
                    job.done = true;
                }
            }
        }
        job.done
    }

    /// Fragmento de barra de estado del cálculo de tamaño en curso/terminado (vacío si no hay).
    pub(super) fn size_status(&self) -> Option<String> {
        let job = self.size_job.as_ref()?;
        let name = job
            .target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| job.target.display().to_string());
        let size = naygo_core::format::format_size(job.bytes, self.config.settings.size_format);
        Some(if !job.done {
            format!("Calculando «{name}»… {size}")
        } else if job.cancelled {
            format!("«{name}»: cancelado en {size}")
        } else if job.partial {
            format!("«{name}»: {size} (parcial)")
        } else {
            format!("«{name}»: {size}")
        })
    }

    // ----- Búsqueda recursiva (Ctrl+F / lupa) ----------------------------------------------

    /// Lanza una búsqueda recursiva de `query` bajo la carpeta del panel Files activo. Cancela y
    /// reemplaza cualquier búsqueda anterior. Una `query` vacía no hace nada (no abre el panel).
    /// El panel de resultados queda abierto (`search_job` presente) y se llena en vivo vía
    /// `pump_search`.
    pub fn start_search(&mut self, query: String) {
        let q = query.trim().to_string();
        if q.is_empty() {
            return;
        }
        crate::logging::breadcrumb(&format!("buscar '{}'", q));
        let Some(root) = self.ws.active_files().map(|f| f.current_dir.clone()) else {
            return;
        };
        // Cancelar el job anterior.
        if let Some(job) = self.search_job.take() {
            job.token.cancel();
        }
        let token = naygo_core::CancellationToken::new();
        let (rx, _handle) =
            naygo_core::search::spawn_search(root.clone(), q.clone(), token.clone());
        self.search_job = Some(SearchJob {
            root,
            query: q,
            rx,
            token,
            hits: Vec::new(),
            dirs_scanned: 0,
            done: false,
            cancelled: false,
            partial: false,
            hit_cap: false,
        });
    }

    /// Abre el panel de búsqueda SIN lanzar nada (la lupa de la toolbar): un job vacío, ya
    /// "terminado", con la carpeta activa como raíz. El usuario escribe y pulsa Enter/Buscar para
    /// que `start_search` reemplace este job por uno real. No hace nada si ya hay un job.
    pub fn open_empty_search(&mut self) {
        if self.search_job.is_some() {
            return;
        }
        let Some(root) = self.ws.active_files().map(|f| f.current_dir.clone()) else {
            return;
        };
        // Canal/token muertos (nunca se usan: el job nace `done`). Se descartan al primer Buscar.
        let token = naygo_core::CancellationToken::new();
        let (_tx, rx) = std::sync::mpsc::channel();
        self.search_job = Some(SearchJob {
            root,
            query: String::new(),
            rx,
            token,
            hits: Vec::new(),
            dirs_scanned: 0,
            done: true,
            cancelled: false,
            partial: false,
            hit_cap: false,
        });
    }

    /// ¿Hay un panel de resultados de búsqueda abierto? (la UI muestra/oculta el overlay).
    pub fn search_open(&self) -> bool {
        self.search_job.is_some()
    }

    /// El texto buscado del job actual (para precargar el campo al reabrir), vacío si no hay.
    pub fn search_query(&self) -> String {
        self.search_job
            .as_ref()
            .map(|j| j.query.clone())
            .unwrap_or_default()
    }

    /// Cierra el panel de resultados y cancela el worker en vuelo (si lo hay).
    pub fn close_search(&mut self) {
        if let Some(job) = self.search_job.take() {
            job.token.cancel();
        }
    }

    /// Cancela el worker en vuelo SIN cerrar el panel (Esc dentro del campo o botón Detener): las
    /// coincidencias ya halladas quedan visibles, marcadas como "cancelado".
    pub fn cancel_search(&mut self) {
        if let Some(job) = self.search_job.as_mut() {
            if !job.done {
                job.token.cancel();
            }
        }
    }

    /// Drena el worker de búsqueda en vuelo (si lo hay), acumulando coincidencias y avance.
    /// Devuelve true si NO queda búsqueda activa pendiente (para que el timer pueda apagarse).
    /// El job terminado se conserva (el panel sigue mostrando los resultados hasta cerrarlo).
    pub fn pump_search(&mut self) -> bool {
        let Some(job) = self.search_job.as_mut() else {
            return true;
        };
        if job.done {
            return true;
        }
        use naygo_core::search::SearchMsg;
        let root = job.root.clone();
        while let Ok(msg) = job.rx.try_recv() {
            match msg {
                SearchMsg::Hit(entry) => {
                    let rel_dir = entry
                        .path
                        .parent()
                        .and_then(|p| p.strip_prefix(&root).ok())
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    job.hits.push(SearchHit { entry, rel_dir });
                }
                SearchMsg::Progress { dirs_scanned } => job.dirs_scanned = dirs_scanned,
                SearchMsg::Done { partial, hit_cap } => {
                    job.partial = partial;
                    job.hit_cap = hit_cap;
                    job.done = true;
                }
                SearchMsg::Cancelled => {
                    job.cancelled = true;
                    job.done = true;
                }
            }
        }
        job.done
    }

    /// Abre la coincidencia `idx` del panel de resultados: si es carpeta, navega el panel activo a
    /// ella (y cierra el panel de búsqueda, porque el contexto cambió); si es archivo, lo abre con
    /// su programa por defecto (el panel sigue abierto para abrir más). No falla si `idx` es inválido.
    pub fn open_search_hit(&mut self, idx: usize) {
        let Some(hit) = self
            .search_job
            .as_ref()
            .and_then(|j| j.hits.get(idx))
            .cloned()
        else {
            return;
        };
        if hit.entry.kind == EntryKind::Directory {
            self.navigate_active_to(hit.entry.path);
            self.close_search();
        } else {
            let _ = naygo_platform::open::open_default(&hit.entry.path);
        }
    }

    /// Filas del panel de resultados, ya formateadas y con el ícono resuelto. Vacío si no hay
    /// búsqueda. Mismo patrón que `rows_of`: préstamo disjunto de `icons` (mutable, cachea).
    pub fn search_rows(&mut self) -> Vec<SearchRow> {
        let date_format = self.config.settings.date_format;
        let size_format = self.config.settings.size_format;
        let tz = naygo_platform::time::local_utc_offset_secs();
        let WorkspaceCtrl {
            search_job, icons, ..
        } = self;
        let Some(job) = search_job.as_ref() else {
            return Vec::new();
        };
        job.hits
            .iter()
            .map(|h| {
                let is_dir = h.entry.kind == EntryKind::Directory;
                // Detalle: tamaño (si archivo) + fecha de modificación, separados por "·".
                let mut parts: Vec<String> = Vec::new();
                if let Some(b) = h.entry.size {
                    parts.push(naygo_core::format::format_size(b, size_format));
                }
                if let Some(t) = h.entry.modified {
                    use std::time::UNIX_EPOCH;
                    let local = t
                        .duration_since(UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_secs() as i64 + tz);
                    let s = naygo_core::format::format_time(local, date_format);
                    if !s.is_empty() {
                        parts.push(s);
                    }
                }
                SearchRow {
                    name: h.entry.name.clone(),
                    rel_dir: h.rel_dir.clone(),
                    detail: parts.join(" · "),
                    is_dir,
                    icon: icons.get(naygo_core::icon_kind::icon_key_for(&h.entry)),
                }
            })
            .collect()
    }

    /// Texto de estado del panel de búsqueda: "Buscando… N", "N resultados", con sufijos
    /// "(parcial)" si hubo carpetas ilegibles y "(tope)" si se cortó por `MAX_HITS`. Vacío si no
    /// hay búsqueda. `running` indica si el worker sigue en vuelo.
    pub fn search_status_text(&self) -> (String, bool) {
        let Some(job) = self.search_job.as_ref() else {
            return (String::new(), false);
        };
        // Panel recién abierto (query vacía, sin búsqueda aún): sin estado.
        if job.query.is_empty() {
            return (String::new(), false);
        }
        let n = job.hits.len();
        let running = !job.done;
        let mut s = if running {
            format!("Buscando… {n}")
        } else if job.cancelled {
            format!("Cancelado · {n} resultado(s)")
        } else {
            format!("{n} resultado(s)")
        };
        if job.partial {
            s.push_str(" (parcial)");
        }
        if job.hit_cap {
            s.push_str(&format!(" (tope {})", naygo_core::search::MAX_HITS));
        }
        (s, running)
    }

    /// Etiqueta de la carpeta raíz de la búsqueda (para el encabezado del panel). Vacío si no hay.
    pub fn search_root_label(&self) -> String {
        self.search_job
            .as_ref()
            .map(|j| j.root.display().to_string())
            .unwrap_or_default()
    }

    /// Atajos activos para la ayuda: pares (acción legible, chord) de las acciones que tienen
    /// al menos un atajo asignado, en el orden de presentación del keymap. Lee el keymap EN VIVO,
    /// así refleja lo que el usuario haya reasignado.
    pub fn help_shortcuts(&self) -> Vec<(String, String)> {
        self.config
            .shortcut_list()
            .into_iter()
            .filter(|(_, _, chord)| !chord.is_empty())
            .map(|(_, label, chord)| (label, chord))
            .collect()
    }

    /// Rutas hacia ATRÁS del panel activo, de la más cercana a la más lejana (para el menú ▾ del
    /// botón Atrás). Vacío si no hay panel Files activo o no hay historial atrás.
    pub fn back_history_entries(&self) -> Vec<std::path::PathBuf> {
        self.ws
            .active_files()
            .map(|f| f.history.back_entries())
            .unwrap_or_default()
    }

    /// Rutas hacia ADELANTE del panel activo, de la más cercana a la más lejana (menú ▾ del botón
    /// Adelante). Vacío si no hay panel Files activo o no hay historial adelante.
    pub fn forward_history_entries(&self) -> Vec<std::path::PathBuf> {
        self.ws
            .active_files()
            .map(|f| f.history.forward_entries())
            .unwrap_or_default()
    }

    /// Salta el panel activo a la entrada `menu_index` del menú ▾ de ATRÁS (0 = la más cercana).
    /// Traduce el índice del menú al índice de la pila usando el cursor actual del NavHistory:
    /// la entrada `i` de `back_entries` está en la posición `cursor - 1 - i` de la pila. Así la UI
    /// no maneja aritmética de índices. Devuelve `true` si navegó.
    pub fn go_back_history(&mut self, menu_index: usize) -> bool {
        let Some(stack_index) = self.stack_index_back(menu_index) else {
            return false;
        };
        self.go_to_history(stack_index)
    }

    /// Salta el panel activo a la entrada `menu_index` del menú ▾ de ADELANTE (0 = la más cercana).
    /// La entrada `i` de `forward_entries` está en la posición `cursor + 1 + i` de la pila.
    /// Devuelve `true` si navegó.
    pub fn go_forward_history(&mut self, menu_index: usize) -> bool {
        let Some(stack_index) = self.stack_index_forward(menu_index) else {
            return false;
        };
        self.go_to_history(stack_index)
    }

    /// Índice en la pila de la entrada `menu_index` del menú de ATRÁS del panel activo. `None` si
    /// no hay panel/cursor o el índice cae fuera de la rama de atrás.
    fn stack_index_back(&self, menu_index: usize) -> Option<usize> {
        let (_, cursor) = self.ws.active_files()?.history.stack();
        let cursor = cursor?;
        // back_entries va de cercano (cursor-1) a lejano (0): entrada i ↦ cursor-1-i.
        cursor.checked_sub(1)?.checked_sub(menu_index)
    }

    /// Índice en la pila de la entrada `menu_index` del menú de ADELANTE del panel activo. `None`
    /// si no hay panel/cursor o el índice cae fuera de la rama de adelante.
    fn stack_index_forward(&self, menu_index: usize) -> Option<usize> {
        let (stack, cursor) = self.ws.active_files()?.history.stack();
        let cursor = cursor?;
        // forward_entries va de cercano (cursor+1) a lejano (len-1): entrada i ↦ cursor+1+i.
        let idx = cursor + 1 + menu_index;
        (idx < stack.len()).then_some(idx)
    }

    /// Salta el panel activo a una entrada de su historial por índice en la pila (menú ▾ de los
    /// botones Atrás/Adelante). Mueve el cursor del NavHistory y navega SIN re-apilar (como
    /// atrás/adelante: `f.go_to_history` usa `history.jump_to`, que solo mueve el cursor).
    /// Devuelve `true` si navegó.
    pub fn go_to_history(&mut self, stack_index: usize) -> bool {
        let Some(active) = self.active_files_id() else {
            return false;
        };
        let moved = self
            .ws
            .pane_mut(active)
            .and_then(|p| p.files.as_mut())
            .and_then(|f| f.go_to_history(stack_index));
        match moved {
            Some(dir) => {
                crate::logging::breadcrumb("historial: salto directo");
                // Navegar cancela la vista profunda del panel (no es pegajosa).
                self.cancel_deep_if_navigating(active);
                self.push_recent(dir.clone());
                self.start_listing(active, dir.clone());
                self.sync_trees_active(dir);
                true
            }
            None => false,
        }
    }

    // --- Vista profunda (DeepJob) ---

    /// Cancela la vista profunda del panel `id` si está activa. Llamar en CUALQUIER camino
    /// que cambie la carpeta de un panel Files (la vista profunda no es pegajosa). No relista:
    /// el camino que navega ya repuebla el panel con su listado normal.
    pub(super) fn cancel_deep_if_navigating(&mut self, id: PaneId) {
        if self.is_deep_active(id) {
            if let Some(d) = self.deep_job.take() {
                d.token.cancel();
            }
        }
    }

    /// ¿El panel `id` está en vista profunda ahora mismo?
    pub fn is_deep_active(&self, id: PaneId) -> bool {
        self.deep_job.as_ref().is_some_and(|d| d.pane == id)
    }

    /// Activa la vista profunda en el panel `id` sobre su carpeta actual. Cancela cualquier
    /// job profundo anterior. Si el panel no es Files o no tiene carpeta válida, no hace nada.
    pub fn deep_start(&mut self, id: PaneId) {
        let dir = self
            .ws
            .pane(id)
            .filter(|p| p.purpose == PanePurpose::Files)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
            .filter(|d| d.is_dir());
        let Some(dir) = dir else { return };
        self.deep_cancel();
        let token = naygo_core::CancellationToken::new();
        let (rx, _handle) =
            naygo_core::deep_listing::spawn_deep_listing(dir.clone(), token.clone());
        self.deep_job = Some(DeepJob {
            pane: id,
            root: dir,
            rx,
            token,
            items: Vec::new(),
            dirs_scanned: 0,
            done: false,
            cancelled: false,
            partial: false,
        });
    }

    /// Apaga la vista profunda: cancela el worker y repuebla el panel con su listado normal.
    pub fn deep_cancel(&mut self) {
        if let Some(d) = self.deep_job.take() {
            d.token.cancel();
            self.start_listing(d.pane, d.root);
        }
    }

    /// Alterna la vista profunda en el panel `id` (para el toggle de la barra).
    pub fn deep_toggle(&mut self, id: PaneId) {
        let estado = if self.is_deep_active(id) { "off" } else { "on" };
        crate::logging::breadcrumb(&format!("vista profunda {}", estado));
        if self.is_deep_active(id) {
            self.deep_cancel();
        } else {
            self.deep_start(id);
        }
    }

    /// Drena los mensajes del worker profundo hacia las entradas acumuladas. Devuelve `true`
    /// si hubo cambios (la UI debe re-sincronizar las filas del panel). No bloquea.
    pub fn deep_poll(&mut self) -> bool {
        let Some(d) = self.deep_job.as_mut() else {
            return false;
        };
        let mut changed = false;
        loop {
            match d.rx.try_recv() {
                Ok(naygo_core::deep_listing::DeepMsg::Entry(de)) => {
                    d.items.push((de.entry, de.depth));
                    changed = true;
                }
                Ok(naygo_core::deep_listing::DeepMsg::Progress { dirs_scanned }) => {
                    d.dirs_scanned = dirs_scanned;
                }
                Ok(naygo_core::deep_listing::DeepMsg::Done { partial }) => {
                    d.done = true;
                    d.partial = partial;
                    changed = true;
                    break;
                }
                Ok(naygo_core::deep_listing::DeepMsg::Cancelled) => {
                    d.cancelled = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    d.done = true;
                    break;
                }
            }
        }
        changed
    }

    /// Las entradas profundas acumuladas (entrada + profundidad). En producción las filas se
    /// arman dentro de `rows_of`; este accesor lo usan los tests para verificar el acumulado.
    #[cfg(test)]
    pub fn deep_items(&self) -> &[(naygo_core::fs_model::Entry, u32)] {
        self.deep_job
            .as_ref()
            .map(|d| d.items.as_slice())
            .unwrap_or(&[])
    }
}
