// Naygo — WorkspaceCtrl: teclado, acciones, paleta de comandos y path-bar.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

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
    pub fn path_autocomplete(&self, buffer: &str) -> Vec<String> {
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
    }

    /// Tecla sobre el panel activo (reusa el keymap). Devuelve true si navegó.
    pub fn on_key(&mut self, text: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.ctrl_down = ctrl;
        self.shift_down = shift;
        // Si hay un modal de operaciones abierto (confirmar borrado, conflicto, pedir nombre,
        // pegar, retomar), el teclado lo controla el modal Slint (Enter confirma, Esc cancela);
        // aquí suspendemos las acciones globales para que un Enter NO abra el archivo
        // seleccionado por debajo del modal. Mismo criterio que con el selector de panel.
        if self.ops.pending_dialog.is_some() {
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
            self.typeahead(text);
            return false;
        };
        let Some(action) = self.config.keymap.action_for(&chord) else {
            self.typeahead(text);
            return false;
        };
        self.typeahead.clear();
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
        let active = self.ws.active_id();
        match action {
            Action::MoveUp => self.with_active(|f| f.move_focus_extend(-1, false)),
            Action::MoveDown => self.with_active(|f| f.move_focus_extend(1, false)),
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
                        self.ws.set_active(next);
                    }
                }
            }
            Action::GoUp => return self.on_go_up(),
            Action::GoBack => return self.on_go_back(),
            Action::GoForward => return self.on_go_forward(),
            Action::GoHome => return self.on_go_home(),
            Action::Refresh => return self.refresh_active(),
            // Ctrl+F: alterna el panel de búsqueda recursiva (abre vacío / cierra).
            Action::Find => {
                if self.search_open() {
                    self.close_search();
                } else {
                    self.open_empty_search();
                }
            }
            Action::ComputeSize => self.compute_size_active(),
            Action::CancelListing => {
                // Esc cierra primero el panel de búsqueda si está abierto (caso más común).
                if self.search_open() {
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
            Action::Cut => self.op_cut(),
            Action::Paste => return self.op_paste(),
            Action::Delete => self.op_delete(false),
            Action::DeletePermanent => self.op_delete(true),
            Action::NewFile => self.op_new(false),
            Action::NewDir => self.op_new(true),
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
            // --- Atajos de botones de la toolbar (configurables) ---
            // Terminal (Ctrl+T): abre PowerShell directo en la carpeta del panel activo (acción
            // directa, no el combo de terminales). term_int 0 = PowerShell, ver `term_from_int`.
            Action::OpenTerminal => self.ctx_open_terminal(0),
            // Dividir (Ctrl+Shift+T): agrega un panel de archivos (la opción más común del menú "+").
            Action::SplitPanel => self.add_pane_split(),
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

        // 1) Acciones CURADAS: las más útiles, en orden de presentación. (Se omiten las de
        // micro-navegación —mover foco, extender selección— que no tienen sentido en una paleta.)
        const CURATED: &[Action] = &[
            Action::Copy,
            Action::Cut,
            Action::Paste,
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
            CommandPayload::OpenConfig => {
                self.open_config_requested = true;
                true
            }
        }
    }

    fn typeahead(&mut self, text: &str) {
        let Some(ch) = text.chars().next().filter(|c| !c.is_control()) else {
            return;
        };
        // Reiniciar el buffer si pasaron más de 500ms desde la última tecla (salto por tipeo
        // estilo Explorer: una pausa empieza una búsqueda nueva).
        let now = std::time::Instant::now();
        if let Some(last) = self.typeahead_at {
            if now.duration_since(last) > std::time::Duration::from_millis(500) {
                self.typeahead.clear();
            }
        }
        self.typeahead_at = Some(now);
        self.typeahead.push(ch.to_ascii_lowercase());
        let needle = self.typeahead.clone();
        if let Some(f) = self.ws.active_files_mut() {
            let view = f.view_indices();
            for (pos, &real) in view.iter().enumerate() {
                if let Some(e) = f.entries.get(real) {
                    // `needle` ya viene en minúsculas ASCII (se arma plegando cada char con
                    // `to_ascii_lowercase`). Comparar el prefijo del nombre en minúsculas ASCII
                    // SIN alocar (antes `e.name.to_lowercase()` alocaba un String Unicode por
                    // entry en cada pulsación, sobre toda la vista de la carpeta).
                    if name_starts_with_ascii_ci(&e.name, &needle) {
                        f.select_single(pos);
                        break;
                    }
                }
            }
        }
    }
}

/// ¿`name` empieza por `needle` comparando case-insensitive SOLO en ASCII, sin alocar?
/// `needle` ya viene plegado a minúsculas ASCII (los chars no-ASCII pasan tal cual, igual
/// que hace `char::to_ascii_lowercase`), así que basta plegar cada char de `name` del mismo
/// modo y comparar carácter a carácter mientras dure `needle`. Coincide con el comportamiento
/// previo para el tipeo ASCII normal (Explorer-style), pero sin construir un String por entry.
fn name_starts_with_ascii_ci(name: &str, needle: &str) -> bool {
    let mut nc = name.chars();
    for need in needle.chars() {
        match nc.next() {
            Some(c) if c.to_ascii_lowercase() == need => {}
            _ => return false,
        }
    }
    true
}
