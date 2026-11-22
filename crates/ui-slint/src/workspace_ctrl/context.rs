// Naygo — WorkspaceCtrl: menú contextual, carpeta nueva y terminal.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

/// Terminales opcionales presentes en el PATH. Detectarlas recorre las carpetas del PATH (I/O
/// de disco, lento con unidades de red), así que se calcula UNA vez en un worker al arrancar y
/// el hilo de UI solo lee este resultado. Antes el menú contextual abierto lo recalculaba en
/// cada tick de `sync_rows`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TerminalTools {
    windows_terminal: bool,
    wsl: bool,
}

static TERMINAL_TOOLS: std::sync::OnceLock<TerminalTools> = std::sync::OnceLock::new();

fn probe_terminal_tools(
    slot: &std::sync::OnceLock<TerminalTools>,
    detect_windows_terminal: impl FnOnce() -> bool,
    detect_wsl: impl FnOnce() -> bool,
) {
    let _ = slot.set(TerminalTools {
        windows_terminal: detect_windows_terminal(),
        wsl: detect_wsl(),
    });
}

/// Lanza el sondeo de terminales en un hilo propio y despierta la UI al terminar, para que el
/// siguiente `sync_rows` publique el resultado. Si el hilo no se puede crear, las entradas
/// quedan ocultas (mismo efecto que no tener las herramientas instaladas).
pub(crate) fn start_terminal_tools_probe(waker: naygo_platform::dir_watch::Waker) {
    let spawned = std::thread::Builder::new()
        .name("naygo-terminal-probe".into())
        .spawn(move || {
            probe_terminal_tools(
                &TERMINAL_TOOLS,
                naygo_platform::open::windows_terminal_available,
                naygo_platform::open::wsl_available,
            );
            waker();
        });
    if let Err(error) = spawned {
        crate::logging::log_line(&format!("no se inició el sondeo de terminales: {error}"));
    }
}

impl WorkspaceCtrl {
    pub(crate) fn take_creation_wait_notice(&mut self) -> bool {
        std::mem::take(&mut self.creation_wait_notice)
    }
    pub(crate) fn take_deep_selection_notice(&mut self) -> bool {
        std::mem::take(&mut self.deep_selection_notice)
    }
    /// Una elección explícita prevalece sobre resultados y avisos tardíos de esa creación.
    pub(super) fn mark_explicit_creation_selection(&mut self, id: PaneId) {
        self.pending_created_activation.remove(&id);
        if let Some(state) = self.creation_selection.get_mut(&id) {
            state.overridden = true;
        }
    }

    pub(super) fn sync_explicit_creation_selection(&mut self, id: PaneId) {
        if self
            .creation_selection
            .get(&id)
            .is_some_and(|state| state.overridden)
        {
            if let Some(files) = self.ws.pane_mut(id).and_then(|pane| pane.files.as_mut()) {
                files.update_pending_context_after_selection();
            }
        }
    }
    /// Bloquea solo activación implícita durante una creación; nunca espera al filesystem.
    pub(super) fn block_pending_created_activation(&mut self, id: PaneId) -> bool {
        let Some(dir) = self.pending_created_activation.get(&id) else {
            return false;
        };
        let current = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .is_some_and(|files| files.current_dir == *dir);
        let blocked = current
            && (self.ops.has_unreconciled_folder_creation(dir)
                || self.listings.contains_key(&id)
                || self.pending_file_listings.contains_key(&id)
                || self
                    .deep_job
                    .as_ref()
                    .is_some_and(|job| job.pane == id && !job.done && !job.cancelled));
        if !blocked {
            self.pending_created_activation.remove(&id);
        }
        blocked
    }
    // --- Menú contextual (clic derecho) ---

    /// Un clic derecho sobre una fila seleccionada conserva el lote; sobre otra lo reemplaza.
    /// La posición se valida contra el modelo actual sin hacer I/O ni detectar doble clic.
    pub fn open_row_context_menu(&mut self, pane: PaneId, pos: usize, x: f32, y: f32) {
        if self.is_deep_active(pane) {
            let Some(entry) = self.deep_visible_entry(pane, pos).cloned() else {
                return;
            };
            self.close_context_menu();
            self.last_click = None;
            self.set_active(pane);
            let selected = self
                .deep_job
                .as_ref()
                .is_some_and(|job| job.selected_paths.contains(&entry.path));
            if selected {
                self.deep_focus_at(pane, pos);
            } else {
                self.deep_select_at(pane, pos, false, false);
            }
            let Some(targets) = self.deep_complete_selection(pane) else {
                self.deep_selection_notice = true;
                return;
            };
            let target_is_folder = targets.len() == 1 && entry.is_dir();
            let folder_fallback = if targets.len() == 1 {
                entry.path.parent().map(std::path::Path::to_path_buf)
            } else {
                self.ws
                    .pane(pane)
                    .and_then(|p| p.files.as_ref())
                    .map(|f| f.current_dir.clone())
            };
            self.context_menu = Some(ContextMenuState {
                identity: std::rc::Rc::new(()),
                pane,
                x,
                y,
                targets,
                folder_mode: false,
                target_is_folder,
                folder_fallback,
                show_open_here: false,
            });
            return;
        }
        let valid = self
            .ws
            .pane(pane)
            .and_then(|p| p.files.as_ref())
            .is_some_and(|files| files.view_entry_at(pos).is_some());
        if !valid {
            return;
        }
        self.last_click = None;
        self.set_active(pane);
        if let Some(files) = self.ws.pane_mut(pane).and_then(|p| p.files.as_mut()) {
            if files.is_selected(pos) {
                files.focused = Some(pos);
                files.presentation_changed();
            } else {
                files.select_single(pos);
            }
        }
        self.mark_explicit_creation_selection(pane);
        self.sync_explicit_creation_selection(pane);
        self.open_context_menu(pane, x, y);
    }

    /// Abre el menú sobre la selección actual del panel indicado.
    pub fn open_context_menu(&mut self, pane: PaneId, x: f32, y: f32) {
        // Activar el panel del clic derecho: el menú (Copiar/Cortar/…) opera sobre ese panel, no
        // sobre el que estaba activo. Sin esto, clic derecho en un panel inactivo copiaba/cortaba
        // la selección de OTRO panel.
        self.set_active(pane);
        let targets = self.selected_paths();
        if targets.is_empty() {
            return;
        }
        // ¿El objetivo es una única carpeta? Se calcula UNA vez aquí (al abrir), usando el `kind`
        // del entry enfocado (sin tocar disco) cuando hay un solo target; el submenú "Abrir ▸" se
        // ofrece solo en ese caso (no en multi-selección ni sobre un archivo).
        let target_is_folder = targets.len() == 1
            && self
                .ws
                .active_files()
                .and_then(|f| f.focused_view_entry())
                .map(|e| e.kind == naygo_core::fs_model::EntryKind::Directory)
                .unwrap_or(false);
        self.context_menu = Some(ContextMenuState {
            identity: std::rc::Rc::new(()),
            pane,
            x,
            y,
            targets,
            folder_mode: false,
            target_is_folder,
            folder_fallback: self.ws.active_files().map(|f| f.current_dir.clone()),
            show_open_here: false,
        });
    }

    /// Abre el menú contextual de la CARPETA del panel `id` en (x,y): clic derecho en la zona
    /// vacía del panel. Marca `id` como activo (para que terminal/Explorer usen su carpeta) y
    /// fija el objetivo en la carpeta actual de ese panel.
    pub fn open_folder_context_menu(&mut self, id: PaneId, x: f32, y: f32) {
        self.set_active(id);
        let dir = self
            .ws
            .pane(id)
            .filter(|p| p.purpose == PanePurpose::Files)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone());
        // La ruta pertenece al estado del panel; consultarla aquí puede congelar la UI en
        // unidades remotas. Cada acción es responsable de manejar una carpeta desaparecida.
        if let Some(dir) = dir {
            self.context_menu = Some(ContextMenuState {
                identity: std::rc::Rc::new(()),
                pane: id,
                x,
                y,
                targets: vec![dir],
                folder_mode: true,
                target_is_folder: true,
                folder_fallback: None,
                show_open_here: false,
            });
        }
    }

    /// Abre el menú de carpeta para un destino explícito de breadcrumb o árbol. Esos modelos ya
    /// entregan directorios reales, por lo que no se hace I/O adicional para validar la ruta.
    pub fn open_path_folder_context_menu(&mut self, id: PaneId, dir: PathBuf, x: f32, y: f32) {
        self.set_active(id);
        self.context_menu = Some(ContextMenuState {
            identity: std::rc::Rc::new(()),
            pane: id,
            x,
            y,
            targets: vec![dir],
            folder_mode: true,
            target_is_folder: true,
            folder_fallback: None,
            show_open_here: true,
        });
    }

    /// Abrir el Explorador de Windows en la carpeta objetivo del menú (modo carpeta).
    pub fn ctx_open_explorer(&mut self) {
        if let Some(dir) = self.terminal_dir() {
            self.start_folder_shell(dir, naygo_platform::open::FolderAction::Explorer);
        }
        self.close_context_menu();
    }

    /// Submenú "Abrir ▸" → "Abrir": navega el panel ACTIVO a la carpeta objetivo del menú
    /// (equivalente al doble-clic sobre esa fila). Devuelve `true` si navegó, para que el
    /// llamador rearme el timer de listado. Cierra el menú siempre.
    pub fn ctx_open_here(&mut self) -> bool {
        let dir = self
            .context_menu
            .as_ref()
            .and_then(|s| s.targets.first().cloned());
        let navigated = match (dir, self.active_files_id()) {
            (Some(dir), Some(active)) => self.navigate_pane_to(active, dir),
            _ => false,
        };
        self.close_context_menu();
        navigated
    }

    /// Desde el menú contextual de carpeta: abrir el modal "nueva(s) carpeta(s)" en la carpeta
    /// objetivo. Cierra el menú.
    pub fn ctx_new_folder(&mut self) {
        // El editor modal toma el foco y puede tragarse el key-release que debería limpiar Ctrl.
        // El siguiente doble clic debe decidirse por el modificador de su propio gesto.
        self.clear_modifiers();
        // El menú en modo carpeta ya capturó su destino (árbol, breadcrumb o panel). No usar
        // terminal_dir(): consultaba disco en UI y podía caer silenciosamente a otro panel.
        let dir = self
            .context_menu
            .as_ref()
            .filter(|menu| menu.folder_mode)
            .and_then(|menu| menu.targets.first())
            .cloned();
        if let Some(dir) = dir {
            self.new_folder = Some(NewFolderState {
                dir,
                text: String::new(),
            });
        }
        self.close_context_menu();
    }

    // --- Modal "nueva(s) carpeta(s)" (multilínea, `\` anidado) ---

    /// Abre el modal de nuevas carpetas en la carpeta del panel activo (p. ej. desde la toolbar).
    pub fn new_folder_open_active(&mut self) {
        // Igual que los demás overlays, no heredar un Ctrl/Shift cuyo release pudo perderse.
        self.clear_modifiers();
        // El árbol usa su cursor, que puede diferir de la carpeta del último panel Files.
        // La existencia se valida en el worker de operaciones, sin consultar disco en UI.
        if let Some(tree) = self.ws.active_id().filter(|id| {
            self.ws
                .pane(*id)
                .is_some_and(|p| p.purpose == PanePurpose::Tree)
        }) {
            if let Some(dir) = self.tree_cursor_of(tree) {
                self.new_folder = Some(NewFolderState {
                    dir,
                    text: String::new(),
                });
            }
            return;
        }
        if let Some(dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
            self.new_folder = Some(NewFolderState {
                dir,
                text: String::new(),
            });
        }
    }

    /// Cierra el modal de nuevas carpetas sin crear nada.
    pub fn new_folder_close(&mut self) {
        self.clear_modifiers();
        self.new_folder = None;
    }

    /// `true` si el modal de nuevas carpetas está abierto.
    pub fn new_folder_open(&self) -> bool {
        self.new_folder.is_some()
    }

    /// Texto multilínea en edición del modal.
    pub fn new_folder_text(&self) -> String {
        self.new_folder
            .as_ref()
            .map(|s| s.text.clone())
            .unwrap_or_default()
    }

    /// Carpeta destino del modal (para mostrarla en el encabezado).
    pub fn new_folder_dir(&self) -> String {
        self.new_folder
            .as_ref()
            .map(|s| s.dir.display().to_string())
            .unwrap_or_default()
    }

    /// Actualiza el texto del modal mientras el usuario escribe.
    pub fn new_folder_set_text(&mut self, text: &str) {
        if let Some(s) = self.new_folder.as_mut() {
            s.text = text.to_string();
        }
    }

    /// Resumen del texto actual: (válidas, inválidas). Para el contador y el estado del botón.
    pub fn new_folder_counts(&self) -> (usize, usize) {
        let specs = naygo_core::ops::parse_new_folders(&self.new_folder_text());
        let valid = specs
            .iter()
            .filter(|s| matches!(s, naygo_core::ops::FolderSpec::Valid(_)))
            .count();
        (valid, specs.len() - valid)
    }

    /// Mensaje de estado del modal: cuántas se crearán y cuántas líneas se ignorarán por inválidas.
    pub fn new_folder_status(&self) -> String {
        let (valid, invalid) = self.new_folder_counts();
        let t = |k: &str| self.config.t(k);
        if valid == 0 && invalid == 0 {
            return t("slint.newfolder.empty");
        }
        let mut parts = Vec::new();
        if valid > 0 {
            parts.push(t("slint.newfolder.will_create").replace("{n}", &valid.to_string()));
        }
        if invalid > 0 {
            parts.push(t("slint.newfolder.invalid").replace("{n}", &invalid.to_string()));
        }
        parts.join(" · ")
    }

    /// Crea las carpetas válidas dentro de la carpeta destino (las inválidas se ignoran, ya
    /// avisadas en el estado). Cada línea válida es una `OpRequest` de `CreateDir` (el motor usa
    /// `create_dir_all`, así que las anidadas se crean enteras). Cierra el modal;
    /// la notificación terminal confirma el resultado y pide selección/refresco.
    pub fn new_folder_apply(&mut self) {
        let (dir, text) = match self.new_folder.as_ref() {
            Some(s) => (s.dir.clone(), s.text.clone()),
            None => return,
        };
        // Ctrl+Enter cierra el modal antes de que el panel reciba key-release.
        self.clear_modifiers();
        let specs = naygo_core::ops::parse_new_folders(&text);
        let has_valid_spec = specs
            .iter()
            .any(|spec| matches!(spec, naygo_core::ops::FolderSpec::Valid(_)));
        // Acotar la memoria extra de seguimiento incluso ante un modal con miles de líneas.
        // Si se supera el límite, conservar la elección ante cualquier llegada de esa carpeta.
        const MAX_TRACKED_TARGETS: usize = 4096;
        let mut targets = HashSet::new();
        let mut overflow = false;
        for spec in &specs {
            if let naygo_core::ops::FolderSpec::Valid(rel) = spec {
                if let Some(first) = std::path::Path::new(rel).components().next() {
                    let target = dir.join(first);
                    if targets.len() < MAX_TRACKED_TARGETS || targets.contains(&target) {
                        targets.insert(target);
                    } else {
                        overflow = true;
                    }
                }
            }
        }
        let origin = self.ws.active_id().filter(|id| {
            self.ws
                .pane(*id)
                .and_then(|pane| pane.files.as_ref())
                .is_some_and(|files| files.current_dir == dir)
        });
        let label = self.config.t("op.new_folder");
        for spec in specs {
            if let naygo_core::ops::FolderSpec::Valid(rel) = spec {
                let req = naygo_core::ops::create(dir.clone(), rel, true);
                self.ops.start_op(req, label.clone(), true);
            }
        }
        // El relist o un watcher puede restaurar el foco anterior antes de la confirmación.
        // Proteger Enter hasta reconciliar y terminar el listado, solo si se inició una op.
        if let Some(id) =
            origin.filter(|_| has_valid_spec && self.ops.has_unreconciled_folder_creation(&dir))
        {
            self.pending_created_activation.insert(id, dir.clone());
            self.creation_selection.insert(
                id,
                CreationSelection {
                    dir,
                    targets,
                    overflow,
                    overridden: false,
                },
            );
            if let Some(files) = self.ws.pane_mut(id).and_then(|pane| pane.files.as_mut()) {
                files.focused = None;
                files.clear_selection();
            }
            if let Some(job) = self.deep_job.as_mut().filter(|job| job.pane == id) {
                job.focused_path = None;
                job.selected_paths.clear();
                job.selection_anchor = None;
                job.selection_revision = job.selection_revision.wrapping_add(1);
            }
        }
        self.new_folder = None;
        // El resultado terminal pide el listado; refrescar aquí corría antes de crear.
    }

    /// Cierra el menú contextual.
    pub fn close_context_menu(&mut self) {
        self.context_menu = None;
    }

    /// Las rutas objetivo del menú contextual abierto (vacío si no hay).
    pub fn context_targets(&self) -> Vec<PathBuf> {
        self.context_menu
            .as_ref()
            .filter(|menu| self.ws.active_id() == Some(menu.pane))
            .map(|c| c.targets.clone())
            .unwrap_or_default()
    }

    /// Abrir el primer objetivo con su programa por defecto.
    pub fn ctx_open(&mut self) {
        if let Some(p) = self.context_targets().first().cloned() {
            self.submit_shell(naygo_platform::open::ShellJob::Open(p));
        }
        self.close_context_menu();
    }

    /// Abrir-con… (diálogo del Shell) sobre el primer objetivo.
    pub fn ctx_open_with(&mut self) {
        if let Some(p) = self.context_targets().first().cloned() {
            self.submit_shell(naygo_platform::open::ShellJob::OpenWith(p));
        }
        self.close_context_menu();
    }

    /// Ejecuta el primer objetivo como administrador mediante el verbo Shell `runas`.
    pub fn ctx_run_as_administrator(&mut self) {
        if let Some(p) = self.context_targets().first().cloned() {
            if naygo_platform::open::can_run_as_administrator(&p) {
                self.submit_shell(naygo_platform::open::ShellJob::RunAsAdministrator(p));
            }
        }
        self.close_context_menu();
    }

    /// Resuelve exclusivamente desde el estado capturado, sin filesystem ni fallback cruzado.
    pub(super) fn terminal_dir(&self) -> Option<PathBuf> {
        if let Some(menu) = &self.context_menu {
            return if menu.folder_mode || menu.target_is_folder {
                menu.targets.first().cloned()
            } else {
                menu.folder_fallback.clone()
            };
        }
        self.ws.active_files().map(|f| f.current_dir.clone())
    }

    /// Abre una terminal (`term_int`: 0=PowerShell, 1=CMD, 2=Windows Terminal) en la carpeta
    /// seleccionada o, si no hay, en la del panel activo. Cierra el menú contextual.
    pub fn ctx_open_terminal(&mut self, term_int: i32) {
        if let Some(dir) = self.terminal_dir() {
            self.start_folder_shell(
                dir,
                naygo_platform::open::FolderAction::Terminal(term_from_int(term_int)),
            );
        }
        self.close_context_menu();
    }

    /// Abre una terminal (`term_int`: 0=PowerShell, 1=CMD, 2=Windows Terminal, 3=WSL) en la
    /// carpeta del panel ACTIVO. Lo usa el combo de terminales de la toolbar.
    pub fn terminal_active(&mut self, term_int: i32) {
        if let Some(dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
            self.start_folder_shell(
                dir,
                naygo_platform::open::FolderAction::Terminal(term_from_int(term_int)),
            );
        }
    }

    pub(crate) fn set_folder_shell_waker(&mut self, waker: naygo_platform::dir_watch::Waker) {
        self.shell.set_waker(waker);
    }

    fn start_folder_shell(&mut self, dir: PathBuf, action: naygo_platform::open::FolderAction) {
        self.submit_shell(naygo_platform::open::ShellJob::Folder { dir, action });
    }

    /// Recoge los resultados que el worker Shell ya entregó y reporta los errores.
    pub(super) fn pump_folder_shell(&mut self) {
        for outcome in self.shell.drain() {
            if outcome.report {
                self.report_shell_result(outcome.result);
            } else if let Err(error) = outcome.result {
                crate::logging::log_line(&format!("operación Shell falló: {error}"));
            }
        }
    }

    /// `true` si Windows Terminal (`wt.exe`) está disponible, para decidir si ofrecer la entrada
    /// "Abrir Windows Terminal aquí" en el menú contextual. Lee el resultado del sondeo de
    /// arranque (`start_terminal_tools_probe`); mientras no termina, vale `false`.
    pub fn windows_terminal_available(&self) -> bool {
        TERMINAL_TOOLS
            .get()
            .is_some_and(|tools| tools.windows_terminal)
    }

    /// `true` si WSL (`wsl.exe`) está disponible, para ofrecer la entrada "Abrir WSL aquí".
    /// Mismo sondeo cacheado que `windows_terminal_available`.
    pub fn wsl_available(&self) -> bool {
        TERMINAL_TOOLS.get().is_some_and(|tools| tools.wsl)
    }

    /// Copiar la ruta del primer objetivo al portapapeles (como texto).
    /// Copia al portapapeles, como TEXTO, las rutas COMPLETAS de los ítems del menú contextual
    /// (uno por línea). Antes esto pegaba los archivos al clipboard (no copiaba la ruta de
    /// verdad); ahora escribe CF_UNICODETEXT con `clipboard::write_text`.
    pub fn ctx_copy_path(&mut self) {
        let lines: Vec<String> = self
            .context_targets()
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        if !lines.is_empty() {
            let _ = self.ops.write_clipboard_text(&lines.join("\r\n"));
        }
        self.close_context_menu();
    }

    /// Copia al portapapeles, como TEXTO, los NOMBRES de los ítems del menú contextual (uno por
    /// línea, sin ruta).
    pub fn ctx_copy_names(&mut self) {
        let lines: Vec<String> = self
            .context_targets()
            .iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.display().to_string())
            })
            .collect();
        if !lines.is_empty() {
            let _ = self.ops.write_clipboard_text(&lines.join("\r\n"));
        }
        self.close_context_menu();
    }

    /// Construye el CSV desde el estado YA CARGADO del panel: selección si existe, o toda la
    /// vista filtrada. Usa las columnas visibles en su orden actual y agrega la ruta completa.
    #[cfg(test)]
    pub fn export_listing_csv(&self, separator: char) -> Option<String> {
        let pane = self.ws.active_files()?;
        let columns: Vec<naygo_core::columns::ColumnKind> = pane
            .table
            .visible_columns()
            .map(|column| column.kind)
            .collect();
        let mut headers: Vec<String> = columns
            .iter()
            .map(|kind| {
                use naygo_core::columns::ColumnKind::*;
                self.config.t(match kind {
                    Name => "col.name",
                    Extension => "col.extension",
                    Size => "col.size",
                    Modified => "col.modified",
                    Created => "col.created",
                })
            })
            .collect();
        headers.push(self.config.t("export.path"));

        let view = pane.view_indices();
        let positions: Vec<usize> = if pane.selected.is_empty() {
            (0..view.len()).collect()
        } else {
            pane.selected.to_vec()
        };
        let rows: Vec<Vec<String>> = positions
            .into_iter()
            .filter_map(|position| view.get(position).and_then(|real| pane.entries.get(*real)))
            .map(|entry| {
                let mut row: Vec<String> = columns
                    .iter()
                    .map(|kind| {
                        if *kind == naygo_core::columns::ColumnKind::Extension
                            && entry.kind == naygo_core::fs_model::EntryKind::Directory
                        {
                            return self.config.t("icons.obj.folder");
                        }
                        crate::bridge::cell_value(
                            entry,
                            *kind,
                            self.config.settings.size_format,
                            self.config.settings.date_format,
                            self.tz_offset_secs(),
                        )
                    })
                    .collect();
                row.push(entry.path.display().to_string());
                row
            })
            .collect();
        Some(naygo_core::listing_export::to_csv(
            &headers, &rows, separator,
        ))
    }

    /// Abre la carpeta objetivo del menú contextual en OTRO panel. Reusa `request_action`:
    /// 1 otro panel → directo; 2+ → selector 1..9; 0 → crea panel nuevo (split por lado largo).
    /// `area` es el área de contenido (la UI la pasa).
    pub fn ctx_open_other_pane(&mut self, area: Rect) -> bool {
        let Some(dir) = self
            .context_menu
            .as_ref()
            .and_then(|s| s.targets.first().cloned())
        else {
            return false;
        };
        let Some(origin) = self.active_files_id() else {
            return false;
        };
        self.request_action(PaneAction::OpenDir(dir), origin, area)
    }

    /// Abre la carpeta objetivo del menú en un panel NUEVO (split por el lado más largo).
    pub fn ctx_open_new_pane(&mut self, area: Rect) -> bool {
        let Some(dir) = self
            .context_menu
            .as_ref()
            .and_then(|s| s.targets.first().cloned())
        else {
            return false;
        };
        self.open_dir_in_new_pane(dir, area)
    }
}

#[cfg(test)]
mod terminal_tools_tests {
    use super::*;

    #[test]
    fn el_sondeo_guarda_el_resultado_una_sola_vez() {
        let slot = std::sync::OnceLock::new();
        assert!(slot.get().is_none());
        probe_terminal_tools(&slot, || true, || false);
        probe_terminal_tools(&slot, || false, || true);
        assert_eq!(
            slot.get(),
            Some(&TerminalTools {
                windows_terminal: true,
                wsl: false
            })
        );
    }
}
