// Naygo — WorkspaceCtrl: teclado, acciones, paleta de comandos y path-bar.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// Debounce del autocompletado async de la path-bar: tipear rápido NO dispara un
/// `read_dir` por tecla; el worker arranca recién cuando el usuario pausó 120 ms.
pub const AUTOCOMPLETE_DEBOUNCE: Duration = Duration::from_millis(120);

/// Estado del autocompletado ASYNC del editor de ruta (path-bar). Mismo patrón que
/// `PreviewState` (debounce + worker con canal + descarte de resultados obsoletos):
/// `request` solo guarda (buffer, instante); `drive` cumple el debounce y lanza el
/// worker (thread que hace el `read_dir` FUERA del hilo de UI); `poll` drena el
/// resultado descartando los de buffers que ya no son el último pedido. Así, tipear
/// una ruta contra un share de red caído no congela la UI.
#[derive(Default)]
pub struct AutocompleteState {
    /// Última petición del usuario: (buffer tecleado, cuándo). Ancla del debounce.
    requested: Option<(String, Instant)>,
    /// Buffer para el que YA se lanzó worker (no relanzar por la misma petición).
    launched: Option<String>,
    /// Worker en vuelo (envía una vez y termina): (buffer, sugerencias).
    rx: Option<Receiver<(String, Vec<String>)>>,
}

impl AutocompleteState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra el buffer tecleado y reinicia el debounce. NO lanza nada (eso es
    /// trabajo de `drive`, llamado por el tick de la UI).
    pub fn request(&mut self, buffer: String, now: Instant) {
        self.requested = Some((buffer, now));
    }

    /// Vencido el debounce, lanza el worker para el último buffer pedido (si no hay
    /// ya uno para ese buffer). Devuelve true si queda trabajo pendiente (debounce
    /// sin vencer o worker en vuelo) para que el timer de la UI siga vivo.
    pub fn drive(&mut self, now: Instant) -> bool {
        let Some((buffer, since)) = &self.requested else {
            return self.rx.is_some();
        };
        if self.launched.as_deref() == Some(buffer.as_str()) {
            // Ya se lanzó para este buffer: solo resta esperar/drenar el resultado.
            return self.rx.is_some();
        }
        if now.duration_since(*since) < AUTOCOMPLETE_DEBOUNCE {
            return true;
        }
        if self.rx.is_some() {
            // Un worker viejo sigue en vuelo (read_dir lento, p. ej. red): esperar a
            // que drene (su resultado se descartará por buffer obsoleto en `poll`).
            return true;
        }
        let buffer = buffer.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker_buffer = buffer.clone();
        std::thread::spawn(move || {
            let sugg = complete_path(&worker_buffer);
            let _ = tx.send((worker_buffer, sugg));
        });
        self.launched = Some(buffer);
        self.rx = Some(rx);
        true
    }

    /// Drena el worker (sin bloquear). Si el resultado corresponde al ÚLTIMO buffer
    /// pedido, lo devuelve como (buffer, sugerencias); un resultado de un buffer
    /// obsoleto (el usuario siguió tipeando) se descarta. None si nada listo/válido.
    pub fn poll(&mut self) -> Option<(String, Vec<String>)> {
        let rx = self.rx.as_ref()?;
        match rx.try_recv() {
            Ok((buffer, sugg)) => {
                self.rx = None;
                if self.requested.as_ref().map(|(b, _)| b) == Some(&buffer) {
                    Some((buffer, sugg))
                } else {
                    None
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.rx = None;
                None
            }
        }
    }

    /// Cancela todo: al cerrar el editor (Enter/Esc) nada de lo pendiente aplica.
    /// El worker en vuelo (si lo hay) termina solo; su envío cae en canal cerrado.
    pub fn cancel(&mut self) {
        self.requested = None;
        self.launched = None;
        self.rx = None;
    }
}

/// Lógica PURA de completado, compartida por el path síncrono (`path_autocomplete`)
/// y el worker async (`AutocompleteState::drive`): dado el `buffer` tecleado, lista
/// las subcarpetas de la carpeta padre que matchean el último segmento
/// (case-insensitive). Lista superficial, acotada a 50.
fn complete_path(buffer: &str) -> Vec<String> {
    let (parent, prefix) = naygo_core::path_segments::split_edit_buffer(buffer);
    if parent.is_empty() {
        return Vec::new();
    }
    let mut names: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&parent) {
        for entry in rd.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                names.push(entry.file_name().to_string_lossy().into_owned());
                if names.len() >= 200 {
                    break;
                }
            }
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    naygo_core::path_segments::filter_candidates(&names, &prefix, 50)
}

impl WorkspaceCtrl {
    /// Carpeta actual del panel `id` (para su path-bar).
    pub fn path_of(&self, id: PaneId) -> String {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.display().to_string())
            .unwrap_or_default()
    }

    /// Segmentos clicables (breadcrumbs) de la carpeta del panel `id`: (etiqueta, ruta).
    pub fn path_segments_of(&self, id: PaneId) -> Vec<(String, String)> {
        let Some(dir) = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        else {
            return Vec::new();
        };
        naygo_core::path_segments::split_segments(&dir)
            .into_iter()
            .map(|(label, path)| (label, path.display().to_string()))
            .collect()
    }

    /// Autocompletado del editor de ruta: dado el `buffer` tecleado, lista las subcarpetas de la
    /// carpeta padre que matchean el último segmento (case-insensitive). Lista superficial,
    /// acotada a 50, en el hilo de UI (un read_dir somero es barato).
    ///
    /// Versión SÍNCRONA, solo para llamados puntuales (abrir el editor, clic en sugerencia):
    /// la vía por-tecla es ASYNC (`request_path_autocomplete` + tick), para no bloquear la UI
    /// contra un share de red caído. Ambas comparten `complete_path`.
    pub fn path_autocomplete(&self, buffer: &str) -> Vec<String> {
        complete_path(buffer)
    }

    /// Pide el autocompletado ASYNC del buffer tecleado (por-tecla): solo guarda la petición
    /// y reinicia el debounce; el tick la materializa con `drive_autocomplete` y entrega el
    /// resultado con `poll_autocomplete`.
    pub fn request_path_autocomplete(&mut self, buffer: String, now: Instant) {
        self.autocomplete.request(buffer, now);
    }

    /// Vencido el debounce, lanza el worker de autocompletado para el último buffer pedido.
    /// Devuelve true si queda trabajo pendiente (el timer de la UI debe seguir vivo).
    pub fn drive_autocomplete(&mut self, now: Instant) -> bool {
        self.autocomplete.drive(now)
    }

    /// Drena el resultado del worker (sin bloquear): (buffer, sugerencias) si corresponde al
    /// último buffer pedido; los de buffers obsoletos se descartan. La UI lo aplica SOLO si
    /// el editor sigue abierto con ese mismo buffer.
    pub fn poll_autocomplete(&mut self) -> Option<(String, Vec<String>)> {
        self.autocomplete.poll()
    }

    /// Cancela el autocompletado pendiente/en vuelo (al cerrar el editor con Enter/Esc).
    pub fn cancel_path_autocomplete(&mut self) {
        self.autocomplete.cancel();
    }

    /// Pide autocompletado para «Buscar desde», usando un worker separado de la path-bar.
    pub fn request_search_path_autocomplete(&mut self, buffer: String, now: Instant) {
        self.search_autocomplete.request(buffer, now);
    }

    /// Impulsa el debounce/worker de autocompletado del panel Search.
    pub fn drive_search_autocomplete(&mut self, now: Instant) -> bool {
        self.search_autocomplete.drive(now)
    }

    /// Recoge sugerencias válidas de la raíz de búsqueda, sin bloquear el hilo de UI.
    pub fn poll_search_autocomplete(&mut self) -> Option<(String, Vec<String>)> {
        self.search_autocomplete.poll()
    }

    /// Cancela sugerencias pendientes al cerrar el panel Search.
    pub fn cancel_search_autocomplete(&mut self) {
        self.search_autocomplete.cancel();
    }

    /// Consume la petición de "editar ruta" (Ctrl+L / F4), si la hay. La UI la llama tras
    /// procesar una tecla para abrir el editor de la path-bar del panel devuelto.
    pub fn take_edit_path_request(&mut self) -> Option<PaneId> {
        self.edit_path_requested.take()
    }

    /// Consume la petición de "abrir la paleta de comandos" (Ctrl+P), si la hay. La UI la
    /// llama tras procesar una tecla para mostrar el overlay de la paleta. (Task 6/7)
    pub fn take_open_palette_request(&mut self) -> bool {
        std::mem::take(&mut self.open_palette_requested)
    }

    /// Consume la petición de "re-aplicar tema" tras elegir un tema en la paleta, si la hay.
    /// Devuelve el id elegido para que la UI llame a `theme_apply::apply`. (Task 6/7)
    pub fn take_palette_theme_request(&mut self) -> Option<naygo_core::theme::ThemeId> {
        self.palette_theme_requested.take()
    }

    /// Consume la petición de "abrir configuración" desde la paleta, si la hay. (Task 6/7)
    pub fn take_open_config_request(&mut self) -> bool {
        std::mem::take(&mut self.open_config_requested)
    }

    /// Consume la petición de abrir un menú/acción de la toolbar disparada por un atajo de teclado
    /// (Favoritos / Disposiciones / Refrescar unidades), si la hay. La UI la aplica sobre los props
    /// de la AppWindow. Ver `ToolbarMenuRequest`.
    pub fn take_toolbar_menu_request(&mut self) -> Option<ToolbarMenuRequest> {
        self.toolbar_menu_requested.take()
    }

    /// ¿Hay ALGÚN overlay/modal de la app abierto que dependa del controlador?
    ///
    /// Lo usa el bucle de UI para NO dormir el timer mientras un modal está en pantalla. Con el
    /// render por software y el modo bajo consumo, el timer se detiene cuando todo está en reposo;
    /// pero un modal recién abierto necesita que el event loop siga procesando eventos de mouse
    /// (hover/move) para que sus botones respondan al instante, sin esperar un clic "de despertar".
    /// Mientras este predicado sea `true`, el timer se mantiene vivo; al cerrarse el modal vuelve a
    /// dormirse como antes (el reposo normal NO se ve afectado).
    ///
    /// Cubre los modales/overlays cuyo estado vive en el controlador. Los que viven en la UI
    /// (MessageModal `MessageVm.kind != 0` y la paleta de comandos `palette_open`) los suma el
    /// bucle de UI por separado, porque este método no conoce la `AppWindow`.
    pub fn any_modal_open(&self) -> bool {
        self.ops.pending_dialog.is_some() // conflicto / confirmar borrado / pedir nombre / carpeta
            || self.pending_pick.is_some() // selector de panel destino (overlay 1..9)
            || self.batch.is_some() // ventana de renombrado por lotes
            || self.new_folder.is_some() // modal "nueva(s) carpeta(s)"
            || self.help_open // ayuda (F1)
            || self.context_menu.is_some() // menú contextual (clic derecho)
            || self.column_menu.is_some() // menú/editor de columna (clic derecho en header)
            || self.sync_assistant.is_some() // asistente de sincronización
            || self.text_transform.is_some() // transformación de texto
            || self.delivery.open
            || self.task_spaces.open
            || self.saved_queries.open
            || self.comparison_points.open
            || self.recipes.open
    }

    /// Tecla sobre el panel activo (reusa el keymap). Devuelve true si navegó.
    pub fn on_key(&mut self, text: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.ctrl_down = ctrl;
        self.shift_down = shift;
        // Un menú contextual es una capa transitoria: Esc debe cerrarlo antes de que la misma
        // tecla cancele un listado, limpie typeahead o active otra acción por debajo. Incluye el
        // menú contextual de archivos/árbol/breadcrumbs y el menú del encabezado de columnas.
        if text.starts_with(crate::keys::escape_char())
            && (self.context_menu.is_some() || self.column_menu.is_some())
        {
            self.close_context_menu();
            self.column_menu_close();
            return true;
        }
        // Si hay un modal de operaciones abierto (confirmar borrado, conflicto, pedir nombre,
        // pegar, retomar), el teclado lo controla el modal Slint (Enter confirma, Esc cancela);
        // aquí suspendemos las acciones globales para que un Enter NO abra el archivo
        // seleccionado por debajo del modal. Mismo criterio que con el selector de panel.
        if self.ops.pending_dialog.is_some()
            || self.delivery.open
            || self.task_spaces.open
            || self.saved_queries.open
            || self.comparison_points.open
            || self.recipes.open
        {
            return false;
        }
        // Con el editor de rename inline abierto, las teclas son del editor (Enter/Esc/flechas
        // las maneja el .slint; el resto es tipeo). El FilePanel ya NO reenvía teclas durante
        // el rename (file-panel.slint, handler `key-pressed`); esta guarda es la red de
        // seguridad en Rust por si aparece otro camino de entrada: sin ella, una tecla filtrada
        // gatillaría typeahead/atajos con el editor abierto (ese fue el bug del "_" que
        // reseteaba el texto: la tecla filtrada movía la selección y el sync re-montaba el
        // editor con el nombre original).
        if self.rename_active.is_some() {
            return false;
        }
        // Si el selector de panel está activo, el teclado lo controla: 1..9 elige, Esc
        // cancela; cualquier otra tecla se ignora (input suspendido como en un modal).
        if self.pending_pick.is_some() {
            if let Some(d) = text.chars().next().and_then(|c| c.to_digit(10)) {
                if d >= 1 {
                    return self.pick_resolve(d as usize);
                }
            }
            if text.starts_with(crate::keys::escape_char()) {
                self.pick_cancel();
            }
            return false;
        }
        let Some(chord) = crate::keys::chord_from(text, ctrl, shift, alt) else {
            // Typeahead/filtro visual: SÍ hubo cambios (buffer, salto de foco, tinte) → true,
            // para que el callback despierte el timer y el sync empuje la selección y el
            // auto-scroll (`focused-row`) a la primera coincidencia.
            return self.typeahead(text);
        };
        let Some(action) = self.config.keymap.action_for(&chord) else {
            return self.typeahead(text);
        };
        // OJO: aquí YA NO se limpia el typeahead. El filtro visual persiste hasta Esc o
        // navegar (decisión de diseño); las acciones por atajo (flechas, copiar, etc.)
        // conviven con el marcado activo. La limpieza ocurre en `clear_filter` (Esc) y en
        // `start_listing` (toda navegación pasa por ahí).
        // Breadcrumb de la acción RESUELTA (tecla → comando del keymap): si la app crashea,
        // el "Última acción" del log dice qué atajo lo gatilló. Solo se loguea cuando la tecla
        // efectivamente dispara una acción (no el tipeo libre de typeahead, por privacidad).
        crate::logging::breadcrumb(&format!(
            "tecla {}{}{}{:?} → {:?}",
            if ctrl { "Ctrl+" } else { "" },
            if shift { "Shift+" } else { "" },
            if alt { "Alt+" } else { "" },
            chord.key,
            action
        ));
        // Tab conserva el recorrido Commander entre paneles de archivos. Ctrl+Tab, en
        // cambio, es el recorrido global del workspace: incluye Árbol, Favoritos,
        // Operaciones, vista previa, búsqueda, etc. Se decide desde el chord (y no como
        // una segunda Action) para mantener retrocompatibilidad con los keybindings ya
        // guardados, donde ambos chords pertenecen a `SwitchPane`.
        if action == Action::SwitchPane
            && chord == naygo_core::keymap::Chord::ctrl(naygo_core::keymap::KeyCode::Tab)
        {
            return self.switch_all_panes();
        }
        self.run_action(action)
    }

    /// Soltado de tecla: refresca el estado de los modificadores con el que reporta el evento.
    /// `on_key` sólo SETEA `ctrl_down`/`shift_down` en cada keydown y nunca los baja; sin este
    /// reset quedaban pegados en `true` tras, por ejemplo, un Ctrl+C, y el siguiente doble-clic en
    /// una carpeta entraba por la rama "abrir en otro panel" (Ctrl+doble-clic) en vez de navegar.
    /// Slint entrega en `ctrl`/`shift` el estado YA vigente tras el release (al soltar Ctrl llega
    /// `ctrl=false`), así que basta con copiarlo: es la fuente más fiable.
    pub fn on_key_release(&mut self, ctrl: bool, shift: bool, _alt: bool) {
        self.ctrl_down = ctrl;
        self.shift_down = shift;
    }

    /// Baja ambos modificadores. Red de seguridad para cuando un overlay/modal roba el foco
    /// (config, paleta de comandos, diálogos de operaciones): el `key-released` de la tecla puede
    /// no llegar al panel, así que limpiamos al abrirlos para no dejar Ctrl/Shift pegados.
    pub fn clear_modifiers(&mut self) {
        self.ctrl_down = false;
        self.shift_down = false;
    }

    /// Ejecuta una `Action` de alto nivel: el cuerpo del `match` que antes vivía dentro de
    /// `on_key`. Se extrajo para que la paleta de comandos (Ctrl+P) pueda disparar la MISMA
    /// acción que el teclado sin duplicar el ruteo (ver `execute_palette_command`). Devuelve
    /// `true` si algo cambió y la UI debe refrescar (igual semántica que `on_key`).
    pub fn run_action(&mut self, action: Action) -> bool {
        if self
            .ws
            .active_id()
            .and_then(|id| self.ws.pane(id))
            .is_some_and(|p| p.purpose == PanePurpose::Search)
            && !matches!(
                action,
                Action::SwitchPane
                    | Action::CommandPalette
                    | Action::Help
                    | Action::OpenConfig
                    | Action::Find
                    | Action::ToggleMaximizePane
                    | Action::CancelListing
            )
        {
            return self.search_handle_action(action);
        }
        let active = self.ws.active_id();
        if active
            .and_then(|id| self.ws.pane(id))
            .is_some_and(|p| p.purpose == PanePurpose::Basket)
        {
            match action {
                Action::SelectAll => return self.basket_select_all(),
                Action::ExtendUp => return self.basket_move_modified(-1, false, true),
                Action::ExtendDown => return self.basket_move_modified(1, false, true),
                Action::ExtendPageUp => {
                    return self.basket_move_modified(-(PAGE_ROWS as isize), false, true)
                }
                Action::ExtendPageDown => {
                    return self.basket_move_modified(PAGE_ROWS as isize, false, true)
                }
                Action::ExtendHome => return self.basket_select_modified(0, false, true),
                Action::ExtendEnd => {
                    return self.basket_select_modified(
                        self.basket.len().saturating_sub(1),
                        false,
                        true,
                    )
                }
                Action::FocusUpKeep => return self.basket_move_modified(-1, true, false),
                Action::FocusDownKeep => return self.basket_move_modified(1, true, false),
                Action::ToggleSelect | Action::ToggleFocused => {
                    if let Some(index) = self
                        .basket_selection
                        .focused
                        .as_ref()
                        .and_then(|p| self.basket.items().iter().position(|item| item == p))
                    {
                        return self.basket_select_modified(index, true, false);
                    }
                    return false;
                }
                Action::Delete => return self.basket_delete(),
                Action::DeletePermanent => return self.basket_delete_kind(true),
                Action::CancelListing => {
                    self.basket_selection.clear_marks();
                    return true;
                }
                Action::Refresh => {
                    self.clear_metadata();
                    return true;
                }
                Action::Paste => {
                    if let naygo_core::clipboard::ClipboardContent::Files { paths, .. } =
                        naygo_platform::clipboard::read()
                    {
                        return self.basket.add(paths) > 0;
                    }
                    return false;
                }
                Action::FocusHome => return self.basket_select(0),
                Action::FocusEnd => return self.basket_select(self.basket.len().saturating_sub(1)),
                Action::FocusPageUp => return self.basket_move_selection(-(PAGE_ROWS as isize)),
                Action::FocusPageDown => return self.basket_move_selection(PAGE_ROWS as isize),
                Action::CopyToOther => return self.basket_request_transfer(false),
                Action::MoveToOther => return self.basket_request_transfer(true),
                Action::Activate => {
                    if let Some(path) = self.basket_selected_path() {
                        let result = naygo_platform::open::open_default(&path);
                        self.report_shell_result(result);
                    }
                    return true;
                }
                // Las acciones que necesitan una tabla Files no deben usar un panel anterior.
                Action::Rename
                | Action::PasteHistory
                | Action::BatchRename
                | Action::ComputeSize
                | Action::FilterPrevMatch
                | Action::GoUp
                | Action::GoBack
                | Action::GoForward
                | Action::GoHome
                | Action::RunAsAdministrator
                | Action::EditPath
                | Action::OpenTerminal
                | Action::NewFile
                | Action::NewDir
                | Action::OpenFocusedOtherPane => return false,
                _ => {}
            }
        }
        match action {
            Action::ToggleMaximizePane => {
                return active.is_some_and(|id| self.toggle_maximize(id));
            }
            // Con el filtro visual por tipeo activo, ↑/↓ NO se mueven de a una fila: saltan a
            // la coincidencia anterior/siguiente (pedido del usuario; Tab sigue para paneles).
            Action::MoveUp => {
                if self
                    .ws
                    .active_id()
                    .and_then(|id| self.ws.pane(id))
                    .is_some_and(|p| p.purpose == PanePurpose::Basket)
                {
                    return self.basket_move_selection(-1);
                }
                if self.filter_active() {
                    self.jump_filter_match(-1);
                } else {
                    self.with_active(|f| f.move_focus_extend(-1, false));
                }
            }
            Action::MoveDown => {
                if self
                    .ws
                    .active_id()
                    .and_then(|id| self.ws.pane(id))
                    .is_some_and(|p| p.purpose == PanePurpose::Basket)
                {
                    return self.basket_move_selection(1);
                }
                if self.filter_active() {
                    self.jump_filter_match(1);
                } else {
                    self.with_active(|f| f.move_focus_extend(1, false));
                }
            }
            Action::ExtendUp => self.with_active(|f| f.move_focus_extend(-1, true)),
            Action::ExtendDown => self.with_active(|f| f.move_focus_extend(1, true)),
            Action::FocusPageUp => self.with_active(|f| f.focus_page(-1, PAGE_ROWS, false)),
            Action::FocusPageDown => self.with_active(|f| f.focus_page(1, PAGE_ROWS, false)),
            Action::ExtendPageUp => self.with_active(|f| f.focus_page(-1, PAGE_ROWS, true)),
            Action::ExtendPageDown => self.with_active(|f| f.focus_page(1, PAGE_ROWS, true)),
            Action::FocusHome => self.with_active(|f| f.focus_home(false)),
            Action::FocusEnd => self.with_active(|f| f.focus_end(false)),
            Action::ExtendHome => self.with_active(|f| f.focus_home(true)),
            Action::ExtendEnd => self.with_active(|f| f.focus_end(true)),
            Action::FocusUpKeep => self.with_active(|f| f.move_focus_keep(-1)),
            Action::FocusDownKeep => self.with_active(|f| f.move_focus_keep(1)),
            Action::ToggleSelect | Action::ToggleFocused => self.with_active(|f| {
                if let Some(p) = f.focused {
                    f.select_toggle(p);
                }
            }),
            Action::SelectAll => self.with_active(|f| f.select_all()),
            Action::SwitchPane => {
                // Tab: ciclar el panel activo entre los Files.
                let files = self.ws.files_panes();
                if files.len() > 1 {
                    if let Some(cur) = active {
                        let i = files.iter().position(|&p| p == cur).unwrap_or(0);
                        let next = files[(i + 1) % files.len()];
                        self.set_active(next);
                    }
                }
            }
            Action::GoUp => return self.on_go_up(),
            Action::GoBack => return self.on_go_back(),
            Action::GoForward => return self.on_go_forward(),
            Action::GoHome => return self.on_go_home(),
            Action::Refresh => return self.refresh_active(),
            // F3 (y Ctrl+F): abre o enfoca el panel de búsqueda recursiva.
            Action::Find => {
                self.open_search_pane(self.last_area);
            }
            Action::ComputeSize => {
                // Acción configurable sin atajo de fábrica: conserva el cálculo de tamaño para
                // quien lo asigne. Con filtro activo mantiene el salto a la siguiente coincidencia.
                if self.filter_active() {
                    self.jump_filter_match(1);
                } else {
                    self.compute_size_active();
                }
            }
            // Shift+F3: coincidencia anterior del filtro (no-op sin filtro).
            Action::FilterPrevMatch => {
                if self.filter_active() {
                    self.jump_filter_match(-1);
                }
            }
            Action::CancelListing => {
                // Esc limpia PRIMERO el filtro visual por tipeo si está activo (lo más
                // superficial: es lo último que el usuario "abrió"). Refrescar para
                // quitar el tinte de las filas.
                if self.clear_filter() {
                    return true;
                }
                // Esc cierra primero el trabajo de búsqueda cuando el foco está en su panel;
                // una búsqueda que queda de fondo no debe interceptar Esc en otro explorador.
                if self.ws.active_id().is_some_and(|id| {
                    self.ws.pane(id).map(|p| p.purpose) == Some(PanePurpose::Search)
                }) && self.search_open()
                {
                    self.close_search();
                    return false;
                }
                self.cancel_active_listing();
                // Esc también cancela un cálculo de tamaño en curso.
                if let Some(job) = self.size_job.as_ref() {
                    if !job.done {
                        job.token.cancel();
                    }
                }
            }
            Action::CopyToOther => return self.op_to_other(false),
            Action::MoveToOther => return self.op_to_other(true),
            Action::Activate => {
                if let (Some(id), Some(pos)) =
                    (active, self.ws.active_files().and_then(|f| f.focused))
                {
                    return self.on_row_double_clicked(id, pos);
                }
            }
            // Shift+Enter: abre la carpeta ENFOCADA en OTRO panel; si es un ejecutable, lo
            // lanza elevado. Así conservamos el atajo Commander existente sin sumar otro chord.
            // (`request_action`): 1 otro panel → directo; 2+ → selector; 0 → divide y usa el nuevo.
            Action::OpenFocusedOtherPane => {
                let Some(origin) = active else {
                    return false;
                };
                let target = self
                    .ws
                    .active_files()
                    .and_then(|f| f.focused_view_entry())
                    .cloned();
                let Some(target) = target else {
                    return false;
                };
                if target.kind == EntryKind::Directory {
                    return self.request_action(
                        PaneAction::OpenDir(target.path),
                        origin,
                        self.last_area,
                    );
                }
                if naygo_platform::open::can_run_as_administrator(&target.path) {
                    let result = naygo_platform::open::run_as_administrator(&target.path);
                    self.report_shell_result(result);
                }
                return false;
            }
            Action::RunAsAdministrator => {
                let target = self
                    .ws
                    .active_files()
                    .and_then(|f| f.focused_view_entry())
                    .map(|e| e.path.clone());
                if let Some(path) = target {
                    if naygo_platform::open::can_run_as_administrator(&path) {
                        let result = naygo_platform::open::run_as_administrator(&path);
                        self.report_shell_result(result);
                    }
                }
                return false;
            }
            Action::GoFavorite1 => return self.go_favorite(0),
            Action::GoFavorite2 => return self.go_favorite(1),
            Action::GoFavorite3 => return self.go_favorite(2),
            Action::GoFavorite4 => return self.go_favorite(3),
            Action::GoFavorite5 => return self.go_favorite(4),
            Action::GoFavorite6 => return self.go_favorite(5),
            Action::GoFavorite7 => return self.go_favorite(6),
            Action::GoFavorite8 => return self.go_favorite(7),
            Action::GoFavorite9 => return self.go_favorite(8),
            // --- Operaciones de archivo (F3) ---
            Action::Copy => self.op_copy(),
            Action::Duplicate => return self.op_duplicate(),
            Action::Cut => self.op_cut(),
            Action::Paste => return self.op_paste(),
            Action::PasteHistory => return self.op_paste_history(),
            Action::Delete => self.op_delete(false),
            Action::DeletePermanent => self.op_delete(true),
            Action::NewFile => self.op_new(false),
            // Ctrl+N debe abrir el mismo editor multilínea que el botón y el menú contextual.
            // Antes conservaba el diálogo histórico de un nombre único: además de ocultar la
            // creación simultánea, mostraba "Nombre no válido" para un buffer vacío, que es una
            // validación correcta pero una explicación equivocada para el estado inicial.
            Action::NewDir => self.new_folder_open_active(),
            Action::Rename => self.op_rename(),
            Action::BatchRename => self.batch_open(),
            Action::Undo => return self.op_undo_last(),
            // Editar la ruta del panel activo (Ctrl+L / F4): la UI abre el editor de la path-bar.
            Action::EditPath => {
                self.edit_path_requested = self.active_files_id();
            }
            Action::Help => self.help_open = !self.help_open,
            // Ctrl+P: ABRE la paleta de comandos (no ejecuta nada). La UI lee el flag con
            // `take_open_palette_request` y muestra el overlay (Task 6/7).
            Action::CommandPalette => {
                self.open_palette_requested = true;
                return true;
            }
            Action::ComparePanels => return self.toggle_compare_panels(),
            // --- Atajos de botones de la toolbar (configurables) ---
            // Terminal (Ctrl+T): abre PowerShell directo en la carpeta del panel activo (acción
            // directa, no el combo de terminales). term_int 0 = PowerShell, ver `term_from_int`.
            Action::OpenTerminal => self.ctx_open_terminal(0),
            // Dividir (Ctrl+Shift+T): agrega un panel de archivos (la opción más común del menú "+").
            // `run_action` no recibe el área de contenido (viene de teclado/paleta): usa la última
            // área conocida (`self.last_area`, que la UI mantiene al día vía `set_area`).
            Action::SplitPanel => self.add_pane_split(self.last_area),
            // Mostrar/ocultar ocultos (Ctrl+H): togglea el flag, re-arma los árboles filtrados y deja
            // que el `sync_rows` posterior refiltre los paneles. Mismo efecto que la casilla del ojo.
            Action::ToggleHidden => {
                let v = self.config.settings.show_hidden;
                self.config.set_show_hidden(!v);
                self.refresh_trees_visibility();
            }
            // Refrescar unidades / abrir menú de favoritos / abrir menú de disposiciones: tocan props
            // de la AppWindow, así que dejan una petición que la UI consume tras procesar la tecla.
            Action::RefreshDrives => {
                self.toolbar_menu_requested = Some(ToolbarMenuRequest::RefreshDrives);
                return true;
            }
            Action::FavoritesMenu => {
                self.toolbar_menu_requested = Some(ToolbarMenuRequest::Favorites);
                return true;
            }
            Action::LayoutsMenu => {
                self.toolbar_menu_requested = Some(ToolbarMenuRequest::Layouts);
                return true;
            }
            // Abrir configuración (Ctrl+Shift+O): reusa la misma petición que la paleta; la UI la
            // consume con `take_open_config_request` e invoca el handler del engranaje.
            Action::OpenConfig => {
                self.open_config_requested = true;
                return true;
            }
            _ => {}
        }
        false
    }

    /// Avanza por TODOS los paneles del layout, no solo por los de archivos. Se usa con
    /// Ctrl+Tab: los paneles especiales también reciben el foco visual y el siguiente Ctrl+Tab
    /// continúa desde ellos. El orden es el del árbol de docking, por lo que respeta la
    /// disposición que ve el usuario; los miembros ocultos de una pila de pestañas se activan
    /// mediante `set_active_tab` para hacerse visibles antes de recibir el foco.
    fn switch_all_panes(&mut self) -> bool {
        let panes: Vec<PaneId> = self
            .ws
            .layout
            .pane_ids()
            .into_iter()
            .filter(|id| self.ws.pane(*id).is_some())
            .collect();
        if panes.len() < 2 {
            return false;
        }
        let index = self
            .ws
            .active_id()
            .and_then(|id| panes.iter().position(|candidate| *candidate == id))
            .unwrap_or(0);
        let next = panes[(index + 1) % panes.len()];
        self.set_active_tab(next);
        true
    }

    /// Navega el panel Files activo al favorito en el índice `idx` (Ctrl+1..9). No-op si no
    /// hay tantos favoritos. Devuelve true si navegó.
    pub fn go_favorite(&mut self, idx: usize) -> bool {
        let flat = self.favorites.list_flat();
        let Some(fav) = flat.get(idx) else {
            return false;
        };
        let path = fav.path.clone();
        self.navigate_active_to(path)
    }

    /// Construye la lista de comandos de la paleta (Ctrl+P) desde las fuentes vivas: acciones
    /// curadas, archivos del panel activo, recientes, favoritos, temas y "abrir configuración".
    /// La UI filtra/ordena con `naygo_core::palette::filter_and_rank` según lo que se escribe, y
    /// ejecuta con `execute_palette_command(&commands, index)`. Lo consume la UI de la paleta
    /// (Task 6/7).
    pub fn build_palette_commands(&self) -> Vec<naygo_core::palette::Command> {
        use naygo_core::keymap::Action;
        use naygo_core::palette::{Command, CommandCategory, CommandPayload};
        let mut out: Vec<Command> = Vec::new();
        out.push(Command {
            label: self.config.t("recipes.title"),
            category: CommandCategory::Action,
            shortcut: String::new(),
            payload: CommandPayload::Recipes,
        });
        for path in &self.recipes.references {
            out.push(Command {
                label: format!(
                    "{}: {}",
                    self.config.t("recipes.open"),
                    path.file_stem().unwrap_or_default().to_string_lossy()
                ),
                category: CommandCategory::Action,
                shortcut: String::new(),
                payload: CommandPayload::LoadRecipe(path.clone()),
            });
        }
        out.push(Command {
            label: self.config.t("points.title"),
            category: CommandCategory::Action,
            shortcut: String::new(),
            payload: CommandPayload::ComparisonPoints,
        });
        out.push(Command {
            label: self.config.t("queries.title"),
            category: CommandCategory::Action,
            shortcut: String::new(),
            payload: CommandPayload::SavedQueries,
        });
        for path in &self.saved_queries.references {
            out.push(Command {
                label: format!(
                    "{}: {}",
                    self.config.t("queries.load"),
                    path.file_stem().unwrap_or_default().to_string_lossy()
                ),
                category: CommandCategory::Action,
                shortcut: String::new(),
                payload: CommandPayload::LoadQuery(path.clone()),
            });
        }
        out.push(Command {
            label: self.config.t("spaces.title"),
            category: CommandCategory::Action,
            shortcut: String::new(),
            payload: CommandPayload::TaskSpaces,
        });
        out.push(Command {
            label: self.config.t("delivery.title"),
            category: CommandCategory::Action,
            shortcut: String::new(),
            payload: CommandPayload::PrepareDelivery,
        });

        // 1) Acciones CURADAS: las más útiles, en orden de presentación. (Se omiten las de
        // micro-navegación —mover foco, extender selección— que no tienen sentido en una paleta.)
        const CURATED: &[Action] = &[
            Action::RunAsAdministrator,
            Action::ComparePanels,
            Action::Copy,
            Action::Cut,
            Action::Paste,
            Action::PasteHistory,
            Action::Rename,
            Action::BatchRename,
            Action::NewFile,
            Action::NewDir,
            Action::ComputeSize,
            Action::Refresh,
            Action::Find,
            Action::Undo,
            Action::GoUp,
            Action::GoBack,
            Action::GoForward,
            Action::GoHome,
            Action::SwitchPane,
            Action::ToggleMaximizePane,
            Action::CopyToOther,
            Action::MoveToOther,
            Action::SelectAll,
            Action::Help,
            Action::EditPath,
        ];
        for &a in CURATED {
            out.push(Command {
                label: self.config.t(a.i18n_key()),
                category: CommandCategory::Action,
                shortcut: self.config.chord_text_for(a),
                payload: CommandPayload::Action(a),
            });
        }

        // 2) Archivos del panel activo (entries de la VISTA actual) → FocusEntry(view_idx). El
        // índice que se guarda es la posición en la VISTA (0-based en view_indices), que es lo que
        // consumen el foco/selección/scroll; no el índice crudo en `entries`.
        if let Some(f) = self.ws.active_files() {
            for (view_idx, &real) in f.view_indices().iter().enumerate() {
                if let Some(e) = f.entries.get(real) {
                    out.push(Command {
                        label: e.name.clone(),
                        category: CommandCategory::File,
                        shortcut: String::new(),
                        payload: CommandPayload::FocusEntry(view_idx),
                    });
                }
            }
        }

        // 3) Recientes → Navigate(path). El nombre de la carpeta es la etiqueta; la ruta completa
        // viaja en el payload.
        for p in self.recents.list() {
            let label = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string());
            out.push(Command {
                label,
                category: CommandCategory::Recent,
                shortcut: String::new(),
                payload: CommandPayload::Navigate(p.clone()),
            });
        }

        // 4) Favoritos → Navigate(path). Usa la etiqueta del favorito (editable a futuro).
        // `list_flat` aplana el árbol de grupos en orden de usuario (pre-orden).
        for fav in self.favorites.list_flat() {
            out.push(Command {
                label: fav.label,
                category: CommandCategory::Favorite,
                shortcut: String::new(),
                payload: CommandPayload::Navigate(fav.path),
            });
        }

        // 5) Temas → Theme(id), etiqueta "Tema: <nombre legible del tema>".
        let theme_prefix = self.config.t("slint.palette.theme_prefix");
        for id in self.config.themes.available() {
            let name = self.config.themes.get(id).name.clone();
            out.push(Command {
                label: format!("{theme_prefix}{name}"),
                category: CommandCategory::Theme,
                shortcut: String::new(),
                payload: CommandPayload::Theme(id.clone()),
            });
        }

        // 6) Abrir configuración → OpenConfig.
        out.push(Command {
            label: self.config.t("slint.palette.open_config"),
            category: CommandCategory::Config,
            shortcut: String::new(),
            payload: CommandPayload::OpenConfig,
        });

        out
    }

    /// Compara el panel Files activo con el siguiente panel Files disponible usando solamente
    /// las entradas ya cargadas (cero I/O en el hilo UI). Una segunda ejecución limpia las
    /// marcas. Se compara el nombre sin distinguir mayúsculas; para archivos también tamaño y
    /// fecha de modificación. Las carpetas se comparan por presencia/tipo, sin recorrerlas.
    pub(super) fn toggle_compare_panels(&mut self) -> bool {
        if !self.comparison.is_empty() {
            self.comparison.clear();
            self.comparison_pair = None;
            self.comparison_link_enabled = false;
            self.comparison_revision = self.comparison_revision.wrapping_add(1);
            return true;
        }

        let Some(active) = self.active_files_id() else {
            return false;
        };
        let Some(other) = self
            .ws
            .panes()
            .iter()
            .find(|p| p.id != active && p.files.is_some())
            .map(|p| p.id)
        else {
            return false;
        };

        #[derive(Clone)]
        struct Comparable {
            path: std::path::PathBuf,
            is_dir: bool,
            size: Option<u64>,
            modified: Option<std::time::SystemTime>,
        }

        let snapshot = |id: naygo_core::workspace::PaneId,
                        ws: &naygo_core::workspace::Workspace| {
            ws.pane(id)
                .and_then(|p| p.files.as_ref())
                .map(|f| {
                    f.entries
                        .iter()
                        .map(|e| {
                            (
                                e.name.to_lowercase(),
                                Comparable {
                                    path: e.path.clone(),
                                    is_dir: e.kind == naygo_core::fs_model::EntryKind::Directory,
                                    size: e.size,
                                    modified: e.modified,
                                },
                            )
                        })
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default()
        };
        let left = snapshot(active, &self.ws);
        let right = snapshot(other, &self.ws);
        let mut left_marks = HashMap::new();
        let mut right_marks = HashMap::new();

        for (name, item) in &left {
            match right.get(name) {
                None => {
                    left_marks.insert(item.path.clone(), 2);
                }
                Some(peer)
                    if item.is_dir != peer.is_dir
                        || (!item.is_dir
                            && (item.size != peer.size || item.modified != peer.modified)) =>
                {
                    left_marks.insert(item.path.clone(), 1);
                    right_marks.insert(peer.path.clone(), 1);
                }
                Some(_) => {}
            }
        }
        for (name, item) in &right {
            if !left.contains_key(name) {
                right_marks.insert(item.path.clone(), 2);
            }
        }

        self.comparison.insert(active, left_marks);
        self.comparison.insert(other, right_marks);
        self.comparison_pair = Some((active, other));
        self.comparison_link_enabled = false;
        self.comparison_revision = self.comparison_revision.wrapping_add(1);
        true
    }

    /// Selecciona desde el panel indicado en vez de depender del orden entre callbacks de UI.
    pub fn comparison_select_for(&mut self, id: PaneId, state: i32) -> bool {
        let Some(marks) = self.comparison.get(&id) else {
            return false;
        };
        let Some(f) = self.ws.pane_mut(id).and_then(|pane| pane.files.as_mut()) else {
            return false;
        };
        if state == 3 {
            let hidden = f
                .entries
                .iter()
                .filter(|entry| !marks.contains_key(&entry.path))
                .map(|entry| entry.path.clone())
                .collect();
            f.set_hidden_paths(hidden);
            return true;
        }
        let selected: Vec<usize> = f
            .view_indices()
            .iter()
            .enumerate()
            .filter_map(|(view_pos, real)| {
                let mark = f
                    .entries
                    .get(*real)
                    .and_then(|entry| marks.get(&entry.path))
                    .copied()?;
                (state == 0 || mark as i32 == state).then_some(view_pos)
            })
            .collect();
        if selected.is_empty() {
            return false;
        }
        f.focused = selected.first().copied();
        f.selected = selected;
        f.presentation_changed();
        true
    }

    /// Copia o mueve desde un panel de origen explícito hacia su compañero comparado.
    pub fn comparison_transfer_from(&mut self, origin: PaneId, move_files: bool) -> bool {
        let Some((left, right)) = self.comparison_pair else {
            return false;
        };
        let other = if origin == left {
            right
        } else if origin == right {
            left
        } else {
            return false;
        };
        let Some(dest_dir) = self
            .ws
            .pane(other)
            .and_then(|pane| pane.files.as_ref())
            .map(|files| files.current_dir.clone())
        else {
            return false;
        };
        let sources = self.selected_paths_of(origin);
        if sources.is_empty() {
            return false;
        }
        let label = if move_files {
            self.config.t("ops.file_kind_move")
        } else {
            self.config.t("ops.file_kind_copy")
        };
        self.ensure_ops_pane();
        self.ops.start_op(
            naygo_core::ops::transfer(move_files, sources, dest_dir),
            label,
            true,
        );
        true
    }

    /// Alterna el enlace de navegación relativa para un miembro del par comparado.
    pub fn comparison_toggle_link(&mut self, id: PaneId) -> bool {
        let Some((left, right)) = self.comparison_pair else {
            return false;
        };
        if id != left && id != right {
            return false;
        }
        self.comparison_link_enabled = !self.comparison_link_enabled;
        true
    }

    /// Ejecuta el comando en `index` de la lista que devolvió `build_palette_commands`. Devuelve
    /// `true` si algo cambió (para refrescar). El llamador (la UI) cierra la paleta. Lo consume la
    /// UI de la paleta (Task 6/7).
    pub fn execute_palette_command(
        &mut self,
        commands: &[naygo_core::palette::Command],
        index: usize,
    ) -> bool {
        use naygo_core::palette::CommandPayload;
        let Some(cmd) = commands.get(index) else {
            return false;
        };
        match cmd.payload.clone() {
            // Acción: se rutea por el MISMO dispatcher del teclado (sin duplicar lógica).
            CommandPayload::Action(a) => self.run_action(a),
            // Navegar el panel activo a la ruta (reciente/favorito).
            CommandPayload::Navigate(p) => self.navigate_active_to(p),
            // Enfocar/seleccionar el índice de VISTA en el panel activo (selección simple, que
            // además fija el foco; la UI hace scroll a la fila enfocada al refrescar).
            CommandPayload::FocusEntry(view_idx) => {
                if let Some(f) = self.ws.active_files_mut() {
                    f.select_single(view_idx);
                    true
                } else {
                    false
                }
            }
            // Aplicar tema: persistir en settings + pedir a la UI que re-pinte las ventanas.
            CommandPayload::Theme(id) => {
                self.config.set_theme(id.clone());
                self.palette_theme_requested = Some(id);
                true
            }
            // Abrir configuración: la UI lee el flag y muestra la ventana.
            CommandPayload::PrepareDelivery => self.delivery_open(),
            CommandPayload::TaskSpaces => self.spaces_open(),
            CommandPayload::SavedQueries => self.query_open(None),
            CommandPayload::ComparisonPoints => self.points_open(),
            CommandPayload::Recipes => self.recipe_open(),
            CommandPayload::LoadRecipe(path) => {
                if self.recipe_open() {
                    self.recipe_read(path.clone(), false);
                    true
                } else {
                    false
                }
            }
            CommandPayload::LoadQuery(path) => {
                if self.query_open(None) {
                    self.query_read(path.clone());
                    true
                } else {
                    false
                }
            }
            CommandPayload::OpenConfig => {
                self.open_config_requested = true;
                true
            }
        }
    }

    /// ¿Hay un filtro visual por tipeo activo? (buffer no vacío). El filtro PERSISTE
    /// hasta Esc o navegar (decisión de diseño): el timeout de 500ms solo agrupa letras
    /// en el buffer, no apaga el marcado.
    pub fn filter_active(&self) -> bool {
        !self.typeahead.is_empty()
    }

    /// Limpia el filtro visual por tipeo (buffer + conteo). Devuelve true si había algo
    /// que limpiar (la UI debe refrescar para quitar el tinte de las filas).
    pub fn clear_filter(&mut self) -> bool {
        let had = self.filter_active();
        self.typeahead.clear();
        self.typeahead_at = None;
        self.filter_match_count = 0;
        self.sync_visual_filter();
        had
    }

    /// Alterna entre marcar coincidencias y dejar solamente esas filas en la vista. No toca los
    /// filtros persistentes por columna; solo acompaña el buffer efímero de typeahead.
    pub fn toggle_filter_hide_nonmatches(&mut self) -> bool {
        self.filter_hide_nonmatches = !self.filter_hide_nonmatches;
        self.sync_visual_filter();
        self.filter_active()
    }

    /// Empuja el typeahead a la vista del panel activo solamente cuando corresponde ocultar.
    /// Los otros paneles mantienen su listado completo y, al activarse, `set_active` aplica el
    /// mismo buffer vigente de forma consistente.
    pub(crate) fn sync_visual_filter(&mut self) {
        let needle = (self.filter_hide_nonmatches && self.filter_active())
            .then(|| naygo_core::text_match::fold_for_match(&self.typeahead));
        if let Some(files) = self.ws.active_files_mut() {
            files.set_visual_filter(needle);
        }
    }

    /// Aguja del filtro YA PLEGADA (`text_match::fold_for_match`) para pintar las filas del
    /// panel `id`, o None si no hay filtro activo. El filtro es global al tipeo pero se
    /// aplica SOLO al panel ACTIVO (es donde el usuario está escribiendo); los demás
    /// paneles no se tiñen.
    pub(crate) fn active_filter_needle(&self, id: PaneId) -> Option<String> {
        if self.ws.active_id() == Some(id) && self.filter_active() {
            Some(naygo_core::text_match::fold_for_match(&self.typeahead))
        } else {
            None
        }
    }

    /// Con el filtro visual activo, ↑/↓ saltan a la coincidencia anterior/siguiente del foco
    /// (con wrap-around: pasar el final vuelve al inicio). Devuelve true si saltó (la UI
    /// refresca); false si no hay matches (no hay a dónde saltar).
    fn jump_filter_match(&mut self, dir: i32) -> bool {
        let needle = naygo_core::text_match::fold_for_match(&self.typeahead);
        let Some(f) = self.ws.active_files_mut() else {
            return false;
        };
        let view = f.view_indices();
        let n = view.len() as i32;
        if n == 0 {
            return false;
        }
        // Sin foco previo: ↓ parte desde antes del primero, ↑ desde después del último.
        let start = f
            .focused
            .map(|p| p as i32)
            .unwrap_or(if dir > 0 { -1 } else { n });
        let mut pos = start;
        for _ in 0..n {
            pos = (pos + dir).rem_euclid(n);
            let Some(e) = view.get(pos as usize).and_then(|&real| f.entries.get(real)) else {
                continue;
            };
            if naygo_core::text_match::contains_folded(&e.name, &needle) {
                f.select_single(pos as usize);
                return true;
            }
        }
        false
    }

    /// Typeahead/filtro visual: agrega el caracter al buffer y salta el foco a la primera
    /// aparición. Devuelve true si la tecla se aceptó (hay que refrescar: tinte del filtro,
    /// selección, auto-scroll a la coincidencia); false si era un caracter de control.
    fn typeahead(&mut self, text: &str) -> bool {
        if self
            .ws
            .active_id()
            .and_then(|id| self.ws.pane(id))
            .is_some_and(|p| p.purpose != PanePurpose::Files)
        {
            return false;
        }
        let Some(ch) = text.chars().next().filter(|c| !c.is_control()) else {
            return false;
        };
        // Reiniciar el buffer si pasaron más de 500ms desde la última tecla (salto por tipeo
        // estilo Explorer: una pausa empieza una búsqueda nueva). OJO: esto SOLO agrupa el
        // tipeo en búsquedas; el MARCADO visual del filtro no se apaga por pausa (persiste
        // hasta Esc o navegar).
        let now = std::time::Instant::now();
        if let Some(last) = self.typeahead_at {
            if now.duration_since(last) > std::time::Duration::from_millis(500) {
                self.typeahead.clear();
            }
        }
        self.typeahead_at = Some(now);
        self.typeahead.push(ch);
        self.sync_visual_filter();
        // Salto de foco: a la PRIMERA aparición en orden de vista que CONTIENE la aguja
        // (prefijo incluido: es un "contiene" al inicio). Pedido del usuario: si los matches
        // están más abajo, el foco va a la primera aparición. Las coincidencias siguientes
        // se recorren con ↓/↑ (ver `jump_filter_match`). Case/acento-insensible.
        if let Some(f) = self.ws.active_files_mut() {
            let needle = naygo_core::text_match::fold_for_match(&self.typeahead);
            let view = f.view_indices();
            let target = (0..view.len()).find(|&pos| {
                view.get(pos)
                    .and_then(|&real| f.entries.get(real))
                    .map(|e| naygo_core::text_match::contains_folded(&e.name, &needle))
                    .unwrap_or(false)
            });
            if let Some(pos) = target {
                f.select_single(pos);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Espera (acotada) a que el worker en vuelo drene, llamando `poll` hasta que el
    /// canal quede consumido. Devuelve el resultado entregado, si hubo uno válido.
    fn drenar_worker(s: &mut AutocompleteState) -> Option<(String, Vec<String>)> {
        let mut entregado = None;
        for _ in 0..2000 {
            if let Some(r) = s.poll() {
                entregado = Some(r);
            }
            if s.rx.is_none() {
                return entregado;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("el worker de autocompletado no terminó en 2 s");
    }

    #[test]
    fn complete_path_matchea_subcarpetas_case_insensitive() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("Alpha")).unwrap();
        std::fs::create_dir(tmp.path().join("alfajor")).unwrap();
        std::fs::create_dir(tmp.path().join("Beta")).unwrap();
        let buffer = format!("{}\\al", tmp.path().display());
        let sugg = complete_path(&buffer);
        assert!(sugg.iter().any(|s| s == "Alpha"), "matchea Alpha: {sugg:?}");
        assert!(
            sugg.iter().any(|s| s == "alfajor"),
            "matchea alfajor (case-insensitive): {sugg:?}"
        );
        assert!(
            !sugg.iter().any(|s| s == "Beta"),
            "no matchea Beta: {sugg:?}"
        );
    }

    #[test]
    fn debounce_no_lanza_worker_antes_del_plazo() {
        let mut s = AutocompleteState::new();
        let t0 = Instant::now();
        s.request("C:\\x".to_string(), t0);
        // Recién pedido: el debounce no venció → busy (pendiente) pero SIN worker.
        assert!(s.drive(t0));
        assert!(s.rx.is_none(), "no debe lanzar worker dentro del debounce");
        // Vencido el plazo, drive lanza el worker.
        assert!(s.drive(t0 + AUTOCOMPLETE_DEBOUNCE));
        assert!(s.rx.is_some(), "vencido el debounce debe lanzar worker");
    }

    #[test]
    fn resultado_de_buffer_obsoleto_se_descarta() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("Alpha")).unwrap();
        let buf_a = format!("{}\\a", tmp.path().display());
        let buf_b = format!("{}\\b", tmp.path().display());
        let mut s = AutocompleteState::new();
        let t0 = Instant::now();
        // Pedir A y lanzar su worker; antes de que llegue, el usuario siguió tipeando (B).
        s.request(buf_a, t0);
        s.drive(t0 + AUTOCOMPLETE_DEBOUNCE);
        assert!(s.rx.is_some());
        s.request(buf_b, t0 + AUTOCOMPLETE_DEBOUNCE);
        // El resultado del buffer A llega obsoleto: se drena pero NO se entrega.
        let entregado = drenar_worker(&mut s);
        assert!(
            entregado.is_none(),
            "un resultado de un buffer viejo no debe entregarse: {entregado:?}"
        );
    }

    #[test]
    fn entrega_sugerencias_del_buffer_vigente_y_deja_de_estar_busy() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("Alpha")).unwrap();
        let buffer = format!("{}\\al", tmp.path().display());
        let mut s = AutocompleteState::new();
        let t0 = Instant::now();
        s.request(buffer.clone(), t0);
        assert!(
            s.drive(t0 + AUTOCOMPLETE_DEBOUNCE),
            "worker en vuelo = busy"
        );
        let Some((buf, sugg)) = drenar_worker(&mut s) else {
            panic!("el resultado del buffer vigente debe entregarse");
        };
        assert_eq!(buf, buffer);
        assert!(sugg.iter().any(|x| x == "Alpha"), "sugerencias: {sugg:?}");
        // Entregado el resultado, ya no queda trabajo pendiente.
        assert!(
            !s.drive(Instant::now()),
            "sin pendientes no debe estar busy"
        );
    }
}
