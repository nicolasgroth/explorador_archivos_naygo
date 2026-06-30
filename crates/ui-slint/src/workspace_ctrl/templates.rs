// Naygo — WorkspaceCtrl: plantillas de disposición, renombrado por lotes y reglas de preview.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
    // --- Plantillas de disposición de paneles (Fase 4) ---

    /// Carpeta «home» para las plantillas (`TemplateDir::Home`): la carpeta del Files activo si la
    /// hay; si no, %USERPROFILE%; si no, la raíz del disco del sistema.
    pub(super) fn template_home(&self) -> PathBuf {
        if let Some(f) = self.ws.active_files() {
            return f.current_dir.clone();
        }
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\"))
    }

    /// Lista de plantillas para el menú: (nombre, es_builtin), built-ins primero y luego las del
    /// usuario. El orden de los built-in es el de `LayoutTemplate::builtins`.
    pub fn layout_templates(&self) -> Vec<(String, bool)> {
        let mut out: Vec<(String, bool)> = naygo_core::workspace::LayoutTemplate::builtins()
            .into_iter()
            .map(|t| (t.name, true))
            .collect();
        out.extend(self.templates.user.iter().map(|t| (t.name.clone(), false)));
        out
    }

    /// Busca una plantilla por nombre entre los built-in y las del usuario.
    fn find_template(&self, name: &str) -> Option<naygo_core::workspace::LayoutTemplate> {
        naygo_core::workspace::LayoutTemplate::builtins()
            .into_iter()
            .find(|t| t.name == name)
            .or_else(|| self.templates.user.iter().find(|t| t.name == name).cloned())
    }

    /// Aplica la plantilla `name`: reconstruye el workspace desde ella, relanza el contenido de
    /// cada panel y registra el uso. `now_secs` lo inyecta la UI (core no llama a SystemTime).
    /// No hace nada si el nombre no existe.
    pub fn apply_template(&mut self, name: &str, now_secs: u64) {
        let Some(tpl) = self.find_template(name) else {
            return;
        };
        crate::logging::breadcrumb(&format!("aplicar layout {}", name));
        let home = self.template_home();
        // Cancelar los listados/árboles actuales antes de reemplazar el workspace.
        for l in self.listings.values() {
            l.cancel();
        }
        self.listings.clear();
        self.trees.clear();
        self.tree_listings.clear();
        self.reveal_targets.clear();
        // Soltar el deep job si lo había: el workspace va a ser reemplazado completo.
        if let Some(d) = self.deep_job.take() {
            d.token.cancel();
        }
        self.ws = naygo_core::workspace::Workspace::from_template(&tpl, &home);
        self.relaunch_all_panes();
        self.last_active_files = self.ws.files_panes().first().copied();
        self.templates.record_use(name, now_secs);
        naygo_core::config::save_templates(&self.config.config_dir, &self.templates);
        self.maybe_persist_session();
    }

    /// Aplica la plantilla `name` SOLO para esta sesión: reconstruye el workspace y relanza el
    /// contenido como `apply_template`, pero NO registra el uso ni persiste (ni la lista de
    /// recientes ni la sesión). La usa `main` para el argumento de CLI `--layout`, que dispone
    /// los paneles por una sola ejecución sin tocar templates.json ni la sesión guardada.
    /// Devuelve `true` si la plantilla existe (si no, no hace nada).
    pub fn apply_template_ephemeral(&mut self, name: &str) -> bool {
        let Some(tpl) = self.find_template(name) else {
            return false;
        };
        crate::logging::breadcrumb(&format!("aplicar layout {} (efímero, CLI)", name));
        let home = self.template_home();
        // Cancelar los listados/árboles actuales antes de reemplazar el workspace.
        for l in self.listings.values() {
            l.cancel();
        }
        self.listings.clear();
        self.trees.clear();
        self.tree_listings.clear();
        self.reveal_targets.clear();
        // Soltar el deep job si lo había: el workspace va a ser reemplazado completo.
        if let Some(d) = self.deep_job.take() {
            d.token.cancel();
        }
        self.ws = naygo_core::workspace::Workspace::from_template(&tpl, &home);
        self.relaunch_all_panes();
        self.last_active_files = self.ws.files_panes().first().copied();
        true
    }

    /// Guarda la disposición ACTUAL como plantilla de usuario con `name` (reemplaza si ya existe).
    /// Persiste. Nombre vacío = no hace nada.
    pub fn save_current_template(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let tpl = self.ws.to_template(name);
        self.templates.add_user(tpl);
        naygo_core::config::save_templates(&self.config.config_dir, &self.templates);
    }

    /// Borra una plantilla de USUARIO por nombre (los built-in no se borran). Persiste.
    pub fn delete_template(&mut self, name: &str) {
        self.templates.remove_user(name);
        naygo_core::config::save_templates(&self.config.config_dir, &self.templates);
    }

    /// Relanza el contenido de TODOS los paneles del workspace actual (tras reemplazarlo por una
    /// plantilla): listados de los Files, árboles de los Tree. Mismo patrón que `load_session`.
    pub(super) fn relaunch_all_panes(&mut self) {
        let panes: Vec<(PaneId, PanePurpose, Option<PathBuf>)> = self
            .ws
            .panes()
            .iter()
            .map(|p| {
                (
                    p.id,
                    p.purpose,
                    p.files.as_ref().map(|f| f.current_dir.clone()),
                )
            })
            .collect();
        for (id, purpose, dir) in panes {
            match purpose {
                PanePurpose::Files => {
                    if let Some(dir) = dir {
                        self.push_recent(dir.clone());
                        self.start_listing(id, dir);
                    }
                }
                PanePurpose::Tree => {
                    let mut t = build_tree();
                    if let Some(cur) = self.ws.active_files().map(|f| f.current_dir.clone()) {
                        t.set_active(cur);
                    }
                    self.trees.insert(id, t);
                }
                _ => {}
            }
        }
    }

    // --- Renombrado por lotes (Fase 5) ---

    /// Abre la ventana de batch-rename con la selección del panel activo (o el foco si no hay
    /// selección). Siembra los ítems (ruta + fecha de modificación) y los nombres existentes del
    /// directorio (para detectar colisiones). No hace nada si no hay ningún ítem.
    pub fn batch_open(&mut self) {
        let targets = self.selected_paths();
        if targets.is_empty() {
            return;
        }
        let Some(f) = self.ws.active_files() else {
            return;
        };
        let to_epoch = |t: Option<std::time::SystemTime>| -> Option<u64> {
            t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
        };
        // Ítems: por cada ruta objetivo, su entry (para la fecha). Conserva el orden de la vista.
        let items: Vec<naygo_core::batch_rename::BatchItem> = targets
            .iter()
            .map(|p| {
                let modified = f
                    .entries
                    .iter()
                    .find(|e| &e.path == p)
                    .and_then(|e| to_epoch(e.modified));
                naygo_core::batch_rename::BatchItem {
                    path: p.clone(),
                    modified_epoch_secs: modified,
                }
            })
            .collect();
        // Nombres existentes del directorio: TODOS los entries (incluye los del lote).
        let existing: Vec<String> = f.entries.iter().map(|e| e.name.clone()).collect();
        self.batch = Some(BatchRenameState {
            spec: naygo_core::batch_rename::BatchSpec::default(),
            items,
            existing,
            tz_offset_secs: self.tz_offset_secs(),
        });
    }

    /// Cierra la ventana de batch-rename sin aplicar.
    pub fn batch_close(&mut self) {
        self.batch = None;
    }

    /// El preview actual (Antes→Después + estado) según el spec en edición. Vacío si no hay
    /// ventana abierta.
    pub fn batch_preview(&self) -> Vec<naygo_core::batch_rename::PreviewRow> {
        match &self.batch {
            Some(b) => {
                naygo_core::batch_rename::preview(&b.items, &b.spec, &b.existing, b.tz_offset_secs)
            }
            None => Vec::new(),
        }
    }

    /// Setters del spec (cada uno recalcula el preview en el siguiente render). `with` muta el
    /// spec si la ventana está abierta.
    fn batch_with<F: FnOnce(&mut naygo_core::batch_rename::BatchSpec)>(&mut self, f: F) {
        if let Some(b) = self.batch.as_mut() {
            f(&mut b.spec);
        }
    }
    pub fn batch_set_template(&mut self, t: &str) {
        let t = t.to_string();
        self.batch_with(|s| s.template = t);
    }
    pub fn batch_set_find(&mut self, t: &str) {
        let t = t.to_string();
        self.batch_with(|s| s.find = t);
    }
    pub fn batch_set_replace(&mut self, t: &str) {
        let t = t.to_string();
        self.batch_with(|s| s.replace = t);
    }
    pub fn batch_set_regex(&mut self, v: bool) {
        self.batch_with(|s| s.use_regex = v);
    }
    pub fn batch_set_include_ext(&mut self, v: bool) {
        self.batch_with(|s| s.include_ext = v);
    }
    /// Mayúsculas: 0=ninguna 1=minúsculas 2=MAYÚSCULAS 3=Título.
    pub fn batch_set_case(&mut self, idx: i32) {
        use naygo_core::batch_rename::CaseTransform::*;
        let case = match idx {
            1 => Lower,
            2 => Upper,
            3 => Title,
            _ => None,
        };
        self.batch_with(|s| s.case = case);
    }
    /// Contador: inicio y paso (parseados a i64; vacío/ inválido → se ignora ese campo).
    pub fn batch_set_counter_start(&mut self, t: &str) {
        if let Ok(v) = t.trim().parse::<i64>() {
            self.batch_with(|s| s.counter_start = v);
        }
    }
    pub fn batch_set_counter_step(&mut self, t: &str) {
        if let Ok(v) = t.trim().parse::<i64>() {
            self.batch_with(|s| s.counter_step = v);
        }
    }

    /// `true` si el preview actual se puede aplicar (≥1 cambio, sin inválidos ni colisiones).
    pub fn batch_can_apply(&self) -> bool {
        naygo_core::batch_rename::can_apply(&self.batch_preview())
    }

    /// Aplica el lote: arma la `OpRequest` de BatchRename con los pares (origen, nombre nuevo) de
    /// las filas `Ok`, la lanza como una sola op deshacible, y cierra la ventana. No hace nada si
    /// el preview no es aplicable.
    pub fn batch_apply(&mut self) {
        let rows = self.batch_preview();
        if !naygo_core::batch_rename::can_apply(&rows) {
            return;
        }
        crate::logging::breadcrumb("batch-rename: aplicar");
        // Solo las filas con cambio real (Ok); Unchanged se omite.
        let mut sources = Vec::new();
        let mut new_names = Vec::new();
        for r in &rows {
            if r.status == naygo_core::batch_rename::RowStatus::Ok {
                sources.push(r.path.clone());
                new_names.push(r.new_name.clone());
            }
        }
        if sources.is_empty() {
            return;
        }
        let req = naygo_core::ops::batch_rename(sources, new_names);
        let label = self.config.t("op.batch_rename");
        self.ops.start_op(req, label, true);
        self.batch = None;
    }

    // --- Reglas de previsualización (C3) ---

    /// Reglas de preview actuales (espejo para la UI): (extensión, habilitada, índice de modo de
    /// vista, índice de lenguaje). El modo de vista es 0=Auto 1=Texto 2=Imagen 3=Código; el índice
    /// de lenguaje es la posición en `CodeLang::all()` (0 si el modo no es Código).
    pub fn preview_rules(&self) -> Vec<(String, bool, i32, i32)> {
        use naygo_core::preview::{CodeLang, ViewMode};
        let mut rows: Vec<(String, bool, i32, i32)> = self
            .config
            .settings
            .preview_rules
            .iter()
            .map(|r| {
                let (view_idx, lang_idx) = match &r.view {
                    ViewMode::Auto => (0, 0),
                    ViewMode::Text => (1, 0),
                    ViewMode::Image => (2, 0),
                    ViewMode::Code(l) => {
                        let li = CodeLang::all().iter().position(|c| c == l).unwrap_or(0) as i32;
                        (3, li)
                    }
                };
                (r.ext.clone(), r.enabled, view_idx, lang_idx)
            })
            .collect();
        // Orden alfabético por extensión para que el listado sea cómodo de leer en Configuración.
        // Los handlers (toggle/set_view/remove) buscan por nombre de extensión, no por índice de
        // fila, así que ordenar la vista no afecta la edición.
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows
    }

    /// Alterna si se previsualiza la extensión `ext`. Persiste.
    pub fn preview_rule_toggle(&mut self, ext: &str) {
        if let Some(r) = self
            .config
            .settings
            .preview_rules
            .iter_mut()
            .find(|r| r.ext == ext)
        {
            r.enabled = !r.enabled;
            self.config.save();
        }
    }

    /// Fija el modo de vista de la extensión `ext` por índice (0=Auto 1=Texto 2=Imagen 3=Código).
    /// Al pasar a Código conserva el lenguaje previo si ya lo tenía; si no, parte de XML. Persiste.
    pub fn preview_rule_set_view_mode(&mut self, ext: &str, idx: i32) {
        use naygo_core::preview::{CodeLang, ViewMode};
        // Normaliza igual que `preview_rule_add` (la regla se guarda en minúscula sin punto), para
        // que el alta — que pasa el texto crudo — encuentre la regla recién creada.
        let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if let Some(r) = self
            .config
            .settings
            .preview_rules
            .iter_mut()
            .find(|r| r.ext == ext)
        {
            r.view = match idx {
                1 => ViewMode::Text,
                2 => ViewMode::Image,
                3 => match r.view {
                    // Conserva el lenguaje si ya estaba en Código; si no, XML por defecto.
                    ViewMode::Code(l) => ViewMode::Code(l),
                    _ => ViewMode::Code(CodeLang::Xml),
                },
                _ => ViewMode::Auto,
            };
            self.config.save();
        }
    }

    /// Fija el lenguaje de código de la extensión `ext` por índice en `CodeLang::all()`. Fuerza el
    /// modo a Código. Índice fuera de rango = no-op. Persiste.
    pub fn preview_rule_set_view_lang(&mut self, ext: &str, idx: i32) {
        use naygo_core::preview::{CodeLang, ViewMode};
        let langs = CodeLang::all();
        let Some(lang) = (idx >= 0)
            .then_some(idx as usize)
            .and_then(|i| langs.get(i))
        else {
            return;
        };
        let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if let Some(r) = self
            .config
            .settings
            .preview_rules
            .iter_mut()
            .find(|r| r.ext == ext)
        {
            r.view = ViewMode::Code(*lang);
            self.config.save();
        }
    }

    /// Quita la regla de la extensión `ext`. Persiste.
    pub fn preview_rule_remove(&mut self, ext: &str) {
        self.config.settings.preview_rules.retain(|r| r.ext != ext);
        self.config.save();
    }

    /// Agrega una regla habilitada para `ext` (normaliza a minúscula sin punto). No duplica.
    /// Nombre vacío = no-op. Persiste.
    pub fn preview_rule_add(&mut self, ext: &str) {
        let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if ext.is_empty()
            || self
                .config
                .settings
                .preview_rules
                .iter()
                .any(|r| r.ext == ext)
        {
            return;
        }
        self.config
            .settings
            .preview_rules
            .push(naygo_core::preview::PreviewRule {
                ext,
                enabled: true,
                view: naygo_core::preview::ViewMode::Auto,
            });
        self.config.save();
    }
}
