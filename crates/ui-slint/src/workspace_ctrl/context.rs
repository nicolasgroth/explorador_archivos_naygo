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
        self.ws.set_active(pane);
        let targets = self.selected_paths();
        if targets.is_empty() {
            return;
        }
        self.context_menu = Some(ContextMenuState {
            x,
            y,
            targets,
            folder_mode: false,
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
            });
        }
    }

    /// Abrir el Explorador de Windows en la carpeta objetivo del menú (modo carpeta).
    pub fn ctx_open_explorer(&mut self) {
        if let Some(dir) = self.terminal_dir() {
            let _ = naygo_platform::open::open_default(&dir);
        }
        self.close_context_menu();
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
        if let Some(p) = self.context_targets().first() {
            let _ = naygo_platform::open::open_default(p);
        }
        self.close_context_menu();
    }

    /// Abrir-con… (diálogo del Shell) sobre el primer objetivo.
    pub fn ctx_open_with(&mut self) {
        if let Some(p) = self.context_targets().first() {
            let _ = naygo_platform::open::open_with_dialog(p);
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
            let _ = naygo_platform::open::open_terminal(&dir, term_from_int(term_int));
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
            let _ = naygo_platform::open::open_terminal(&dir, term_from_int(term_int));
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
}
