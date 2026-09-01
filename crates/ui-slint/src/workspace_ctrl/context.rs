// Naygo — WorkspaceCtrl: menú contextual, carpeta nueva y terminal.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
    // --- Menú contextual (clic derecho) ---

    /// Abre el menú contextual en (x,y) sobre la fila `pos` del panel `id`. Si la fila no
    /// estaba seleccionada, la selecciona (Explorer hace lo mismo). El objetivo del menú es
    /// la selección actual.
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
            x,
            y,
            targets,
            folder_mode: false,
            target_is_folder,
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
        if let Some(dir) = dir.filter(|d| d.is_dir()) {
            self.context_menu = Some(ContextMenuState {
                x,
                y,
                targets: vec![dir],
                folder_mode: true,
                target_is_folder: true,
                show_open_here: false,
            });
        }
    }

    /// Abre el menú de carpeta para un destino explícito de breadcrumb o árbol. Esos modelos ya
    /// entregan directorios reales, por lo que no se hace I/O adicional para validar la ruta.
    pub fn open_path_folder_context_menu(&mut self, id: PaneId, dir: PathBuf, x: f32, y: f32) {
        self.set_active(id);
        self.context_menu = Some(ContextMenuState {
            x,
            y,
            targets: vec![dir],
            folder_mode: true,
            target_is_folder: true,
            show_open_here: true,
        });
    }

    /// Abrir el Explorador de Windows en la carpeta objetivo del menú (modo carpeta).
    pub fn ctx_open_explorer(&mut self) {
        if let Some(dir) = self.terminal_dir() {
            let result = naygo_platform::open::open_default(&dir);
            self.report_shell_result(result);
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
        if let Some(dir) = self.terminal_dir() {
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
        if let Some(dir) = self
            .ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .filter(|d| d.is_dir())
        {
            self.new_folder = Some(NewFolderState {
                dir,
                text: String::new(),
            });
        }
    }

    /// Cierra el modal de nuevas carpetas sin crear nada.
    pub fn new_folder_close(&mut self) {
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
    /// `create_dir_all`, así que las anidadas se crean enteras). Cierra el modal y refresca.
    pub fn new_folder_apply(&mut self) {
        let (dir, text) = match self.new_folder.as_ref() {
            Some(s) => (s.dir.clone(), s.text.clone()),
            None => return,
        };
        let specs = naygo_core::ops::parse_new_folders(&text);
        let label = self.config.t("op.new_folder");
        let mut created_any = false;
        for spec in specs {
            if let naygo_core::ops::FolderSpec::Valid(rel) = spec {
                let req = naygo_core::ops::create(dir.clone(), rel, true);
                self.ops.start_op(req, label.clone(), true);
                created_any = true;
            }
        }
        self.new_folder = None;
        if created_any {
            self.refresh_active();
        }
    }

    /// Cierra el menú contextual.
    pub fn close_context_menu(&mut self) {
        self.context_menu = None;
    }

    /// Las rutas objetivo del menú contextual abierto (vacío si no hay).
    pub fn context_targets(&self) -> Vec<PathBuf> {
        self.context_menu
            .as_ref()
            .map(|c| c.targets.clone())
            .unwrap_or_default()
    }

    /// Abrir el primer objetivo con su programa por defecto.
    pub fn ctx_open(&mut self) {
        if let Some(p) = self.context_targets().first().cloned() {
            let result = naygo_platform::open::open_default(&p);
            self.report_shell_result(result);
        }
        self.close_context_menu();
    }

    /// Abrir-con… (diálogo del Shell) sobre el primer objetivo.
    pub fn ctx_open_with(&mut self) {
        if let Some(p) = self.context_targets().first().cloned() {
            let result = naygo_platform::open::open_with_dialog(&p);
            self.report_shell_result(result);
        }
        self.close_context_menu();
    }

    /// Ejecuta el primer objetivo como administrador mediante el verbo Shell `runas`.
    pub fn ctx_run_as_administrator(&mut self) {
        if let Some(p) = self.context_targets().first().cloned() {
            if naygo_platform::open::can_run_as_administrator(&p) {
                let result = naygo_platform::open::run_as_administrator(&p);
                self.report_shell_result(result);
            }
        }
        self.close_context_menu();
    }

    /// Carpeta destino para "abrir terminal aquí": si el primer objetivo del menú es una carpeta,
    /// se usa esa; si no (es un archivo, o no hay objetivo), la carpeta del panel activo.
    pub(super) fn terminal_dir(&self) -> Option<PathBuf> {
        if let Some(p) = self.context_targets().first() {
            if p.is_dir() {
                return Some(p.clone());
            }
        }
        self.ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .filter(|d| d.is_dir())
    }

    /// Abre una terminal (`term_int`: 0=PowerShell, 1=CMD, 2=Windows Terminal) en la carpeta
    /// seleccionada o, si no hay, en la del panel activo. Cierra el menú contextual.
    pub fn ctx_open_terminal(&mut self, term_int: i32) {
        if let Some(dir) = self.terminal_dir() {
            let result = naygo_platform::open::open_terminal(&dir, term_from_int(term_int));
            self.report_shell_result(result);
        }
        self.close_context_menu();
    }

    /// Abre una terminal (`term_int`: 0=PowerShell, 1=CMD, 2=Windows Terminal, 3=WSL) en la
    /// carpeta del panel ACTIVO. Lo usa el combo de terminales de la toolbar.
    pub fn terminal_active(&mut self, term_int: i32) {
        if let Some(dir) = self
            .ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .filter(|d| d.is_dir())
        {
            let result = naygo_platform::open::open_terminal(&dir, term_from_int(term_int));
            self.report_shell_result(result);
        }
    }

    /// `true` si Windows Terminal (`wt.exe`) está disponible, para decidir si ofrecer la entrada
    /// "Abrir Windows Terminal aquí" en el menú contextual.
    pub fn windows_terminal_available(&self) -> bool {
        naygo_platform::open::windows_terminal_available()
    }

    /// `true` si WSL (`wsl.exe`) está disponible, para ofrecer la entrada "Abrir WSL aquí".
    pub fn wsl_available(&self) -> bool {
        naygo_platform::open::wsl_available()
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
            let _ = naygo_platform::clipboard::write_text(&lines.join("\r\n"));
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
            let _ = naygo_platform::clipboard::write_text(&lines.join("\r\n"));
        }
        self.close_context_menu();
    }

    /// Construye el CSV desde el estado YA CARGADO del panel: selección si existe, o toda la
    /// vista filtrada. Usa las columnas visibles en su orden actual y agrega la ruta completa.
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
                        crate::bridge::cell_value(
                            entry,
                            *kind,
                            self.config.settings.size_format,
                            self.config.settings.date_format,
                            crate::logging::tz_offset_secs(),
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

    pub fn ctx_export_listing_clipboard(&mut self) {
        if let Some(csv) = self.export_listing_csv(';') {
            let _ = naygo_platform::clipboard::write_text(&csv);
        }
        self.close_context_menu();
    }

    pub fn ctx_export_listing_clipboard_comma(&mut self) {
        if let Some(csv) = self.export_listing_csv(',') {
            let _ = naygo_platform::clipboard::write_text(&csv);
        }
        self.close_context_menu();
    }

    pub fn export_listing_file_bytes(&self, separator: char) -> Option<Vec<u8>> {
        let csv = self.export_listing_csv(separator)?;
        let mut bytes = Vec::with_capacity(csv.len() + 3);
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        bytes.extend_from_slice(csv.as_bytes());
        Some(bytes)
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
