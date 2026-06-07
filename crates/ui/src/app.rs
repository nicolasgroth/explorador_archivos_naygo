// Naygo — estado raíz de la aplicación y loop de egui (multi-panel).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `NaygoApp` mantiene un `Workspace` de N paneles independientes. Cada panel
//! `Files` lista en su propio worker (un `PaneListing`); el hilo de UI drena
//! todos los canales sin bloquear. El teclado y los botones del mouse actúan
//! sobre el panel activo. El layout y las carpetas se persisten vía `config`.

use crate::icons::IconProvider;
use crate::input::{map_key, map_mouse_extra, Action, Key as NaygoKey, MouseExtra};
use crate::settings_window::SettingsSection;
use eframe::CreationContext;
use egui_dock::DockState;
use naygo_core::cancel::CancellationToken;
use naygo_core::config::{self, Settings};
use naygo_core::i18n::{pick_default_language, I18n, LangId};
use naygo_core::listing::{spawn_listing, spawn_listing_filtered, ListingFilter, ListingMsg};
use naygo_core::sort::sort_entries;
use naygo_core::tree::DirTree;
use naygo_core::workspace::template::LayoutTemplate;
use naygo_core::workspace::{FilePaneState, PaneId, PanePurpose, Workspace};
use naygo_core::NodeOutcome;
use naygo_core::TemplateStore;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

/// El worker de listing activo de un panel `Files`.
pub struct PaneListing {
    pub rx: Option<Receiver<ListingMsg>>,
    pub token: CancellationToken,
}

/// Un worker de listado solo-directorios para una rama del árbol.
struct TreeListing {
    rx: Option<Receiver<ListingMsg>>,
    token: CancellationToken,
}

/// Estado raíz de la app.
pub struct NaygoApp {
    pub workspace: Workspace,
    pub dock_state: DockState<PaneId>,
    listings: HashMap<PaneId, PaneListing>,
    /// Estado del árbol por cada panel Tree (creado perezosamente).
    trees: HashMap<PaneId, DirTree>,
    /// Workers solo-directorios del árbol, por (panel, carpeta) expandida.
    tree_listings: HashMap<(PaneId, PathBuf), TreeListing>,
    pub settings: Settings,
    pub templates: TemplateStore,
    config_dir: PathBuf,
    pub status: String,
    typeahead_buf: String,
    icons: IconProvider,
    i18n: I18n,
    pub settings_open: bool,
    pub settings_section: SettingsSection,
}

impl NaygoApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let config_dir = config::portable_dir();
        let settings = config::load_settings(&config_dir);
        let templates = config::load_templates(&config_dir);
        let home = default_start_dir();

        // i18n: idioma persistido si ya hubo settings; si es el primer arranque,
        // detectar el del SO. Cargamos primero con un idioma provisional para
        // conocer los idiomas disponibles, luego elegimos.
        let settings_exists = config_dir.join("settings.json").exists();
        let provisional = I18n::load(&config_dir, &settings.language);
        let lang = if settings_exists {
            settings.language.clone()
        } else {
            let locale = naygo_platform::locale::os_locale().unwrap_or_default();
            pick_default_language(&locale, provisional.available())
        };
        let mut i18n = provisional;
        i18n.set_language(&lang);
        let mut settings = settings;
        settings.language = lang;

        let workspace = load_or_default_workspace(&config_dir, &home);
        let dock_state = crate::dock_translate::to_dock_state(&workspace.layout);

        let icons = IconProvider::new(&cc.egui_ctx, settings.icon_set);

        let mut app = NaygoApp {
            workspace,
            dock_state,
            listings: HashMap::new(),
            trees: HashMap::new(),
            tree_listings: HashMap::new(),
            settings,
            templates,
            config_dir,
            status: String::new(),
            typeahead_buf: String::new(),
            icons,
            i18n,
            settings_open: false,
            settings_section: SettingsSection::Appearance,
        };
        app.start_all_listings();
        app
    }

    /// Atajo para traducir una clave con el idioma activo.
    pub fn tr(&self, key: &str) -> String {
        self.i18n.t(key).to_string()
    }

    /// Idiomas disponibles (clonados, para la UI sin prestar `self.i18n`).
    pub fn i18n_available(&self) -> Vec<LangId> {
        self.i18n.available().to_vec()
    }

    /// Ruta de la carpeta de config (para la sección Avanzado).
    pub fn config_dir_display(&self) -> String {
        self.config_dir.display().to_string()
    }

    /// Lanza un worker de listing para CADA panel `Files`, en su carpeta.
    fn start_all_listings(&mut self) {
        let files: Vec<(PaneId, PathBuf)> = self
            .workspace
            .panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Files)
            .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.current_dir.clone())))
            .collect();
        for (id, dir) in files {
            self.start_listing(id, dir);
        }
    }

    /// (Re)lanza el listado de un panel: cancela el anterior y arranca otro.
    pub fn start_listing(&mut self, id: PaneId, dir: PathBuf) {
        if let Some(prev) = self.listings.get(&id) {
            prev.token.cancel();
        }
        let token = CancellationToken::new();
        let (rx, _handle) = spawn_listing(dir, token.clone());
        self.listings.insert(
            id,
            PaneListing {
                rx: Some(rx),
                token,
            },
        );
        // Feedback "Listando…" mientras el panel activo carga (se reemplaza por el
        // conteo de elementos al terminar, en pump_one).
        if self.workspace.active_id() == Some(id) {
            self.status = self.i18n.t("app.loading").to_string();
        }
    }

    /// Re-lista un panel sin tocar su historial (refrescar).
    pub fn refresh_pane(&mut self, id: PaneId, dir: PathBuf) {
        if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.entries.clear();
            f.focused = None;
        }
        self.start_listing(id, dir);
    }

    /// Agrega un panel de archivos nuevo en la carpeta del activo (o home) y lo
    /// inserta en el dock.
    pub fn add_files_pane(&mut self) {
        let dir = self
            .workspace
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(default_start_dir);
        let id = self.workspace.add_pane(PanePurpose::Files, dir.clone());
        self.dock_state.main_surface_mut().push_to_focused_leaf(id);
        self.start_listing(id, dir);
    }

    /// Aplica una plantilla: recompone el workspace, reconstruye el dock, registra
    /// el uso en recientes y relanza los listados. `now` es el timestamp (epoch s)
    /// inyectado desde la UI (core no llama a SystemTime::now).
    pub fn apply_template(&mut self, tpl: &LayoutTemplate, now: u64) {
        let home = default_start_dir();
        // Cancelar explícitamente los workers anteriores (no solo soltar sus
        // receptores) para abortar pronto incluso uno colgado en un disco de red.
        for listing in self.listings.values() {
            listing.token.cancel();
        }
        self.workspace = Workspace::from_template(tpl, &home);
        self.dock_state = crate::dock_translate::to_dock_state(&self.workspace.layout);
        self.listings.clear();
        self.templates.record_use(&tpl.name, now);
        self.start_all_listings();
    }

    /// Guarda la disposición actual como una plantilla del usuario con `name`.
    /// (Para 2A guardamos la forma del layout actual con carpetas = Home.)
    pub fn save_current_as_template(&mut self, name: &str) {
        use naygo_core::workspace::template::{LayoutTemplate, TemplateDir, TemplatePane};
        let ids = self.workspace.layout.pane_ids();
        let mut panes = Vec::new();
        let mut index_of = std::collections::HashMap::new();
        for (idx, id) in ids.iter().enumerate() {
            let purpose = self
                .workspace
                .pane(*id)
                .map(|p| p.purpose)
                .unwrap_or(PanePurpose::Files);
            panes.push(TemplatePane {
                purpose,
                dir: TemplateDir::Home,
            });
            index_of.insert(*id, idx);
        }
        let shape = layout_to_shape(&self.workspace.layout, &index_of);
        let tpl = LayoutTemplate {
            name: name.to_string(),
            builtin: false,
            favorite: false,
            panes,
            layout: shape,
        };
        self.templates.add_user(tpl);
    }

    /// Drena los canales de TODOS los paneles, sin bloquear.
    fn pump_all(&mut self) {
        let ids: Vec<PaneId> = self.listings.keys().copied().collect();
        for id in ids {
            self.pump_one(id);
        }
    }

    fn pump_one(&mut self, id: PaneId) {
        let mut finished = false;
        let mut new_entries = Vec::new();
        let mut err = None;
        let mut cancelled = false;
        if let Some(listing) = self.listings.get(&id) {
            if let Some(rx) = &listing.rx {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        ListingMsg::Entry(e) => new_entries.push(e),
                        ListingMsg::Done => finished = true,
                        ListingMsg::Cancelled => {
                            finished = true;
                            cancelled = true;
                        }
                        ListingMsg::Error(e) => {
                            err = Some(e);
                            finished = true;
                        }
                    }
                }
            }
        }
        let mut count_done = None;
        if let Some(pane) = self.workspace.pane_mut(id) {
            if let Some(f) = pane.files.as_mut() {
                f.entries.extend(new_entries);
                if finished {
                    let spec = f.sort;
                    sort_entries(&mut f.entries, &spec);
                    if f.focused.is_none() && !f.entries.is_empty() {
                        f.focused = Some(0);
                    }
                    if err.is_none() && !cancelled {
                        count_done = Some(f.entries.len());
                    }
                }
            }
        }
        if finished {
            if let Some(listing) = self.listings.get_mut(&id) {
                listing.rx = None;
            }
            // El status global refleja el feedback del panel ACTIVO (con N paneles
            // listando en paralelo, mostrar el del activo es lo predecible).
            let is_active = self.workspace.active_id() == Some(id);
            if let Some(e) = err {
                if is_active {
                    self.status = self.i18n.t("status.error").replace("{e}", &e);
                }
            } else if is_active {
                if cancelled {
                    self.status = self.i18n.t("app.cancelled").to_string();
                } else if let Some(n) = count_done {
                    self.status = self
                        .i18n
                        .t("status.elements")
                        .replace("{n}", &n.to_string());
                }
            }
        }
    }

    fn any_listing_active(&self) -> bool {
        self.listings.values().any(|l| l.rx.is_some())
    }

    /// Devuelve el `DirTree` del panel `id`, creándolo (con las unidades) la primera
    /// vez. Útil antes de pintar un panel Tree.
    fn ensure_tree(&mut self, id: PaneId) -> &mut DirTree {
        self.trees.entry(id).or_insert_with(|| {
            let drives = naygo_platform::drives::drives()
                .into_iter()
                .map(|d| (d.path, d.label, d.kind))
                .collect::<Vec<_>>();
            DirTree::from_drives(&drives)
        })
    }

    /// Expande una rama del árbol del panel `id`: marca Loading y lanza el worker
    /// solo-directorios. No-op si ya hay un worker para esa (id, path).
    fn tree_expand(&mut self, id: PaneId, path: PathBuf) {
        // Ya hay un worker en vuelo para esta rama → no relanzar.
        if self.tree_listings.contains_key(&(id, path.clone())) {
            return;
        }
        if let Some(tree) = self.trees.get_mut(&id) {
            // Si la rama YA tiene hijos cargados, solo reabrir visualmente: NO
            // re-listar (conserva los hijos; spec "colapsar no re-lista al reabrir").
            let already_loaded = tree
                .node_at(&path)
                .map(|n| n.children.is_some())
                .unwrap_or(false);
            if already_loaded {
                tree.expand_loaded(&path);
                return;
            }
            tree.begin_loading(&path);
        }
        let token = CancellationToken::new();
        let (rx, _h) = spawn_listing_filtered(path.clone(), token.clone(), ListingFilter::DirsOnly);
        self.tree_listings.insert(
            (id, path),
            TreeListing {
                rx: Some(rx),
                token,
            },
        );
    }

    /// Colapsa una rama (conserva hijos). Cancela su worker si seguía cargando.
    fn tree_collapse(&mut self, id: PaneId, path: PathBuf) {
        if let Some(l) = self.tree_listings.get(&(id, path.clone())) {
            l.token.cancel();
        }
        self.tree_listings.remove(&(id, path.clone()));
        if let Some(tree) = self.trees.get_mut(&id) {
            tree.collapse(&path);
        }
    }

    /// Drena los canales de TODOS los workers del árbol, sin bloquear.
    fn pump_tree(&mut self) {
        let keys: Vec<(PaneId, PathBuf)> = self.tree_listings.keys().cloned().collect();
        for key in keys {
            self.pump_tree_one(key);
        }
    }

    fn pump_tree_one(&mut self, key: (PaneId, PathBuf)) {
        let (id, path) = (key.0, &key.1);
        let mut finished = false;
        let mut err = false;
        let mut new_dirs: Vec<PathBuf> = Vec::new();
        if let Some(listing) = self.tree_listings.get(&key) {
            if let Some(rx) = &listing.rx {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        ListingMsg::Entry(e) => new_dirs.push(e.path),
                        ListingMsg::Done => finished = true,
                        ListingMsg::Cancelled => finished = true,
                        ListingMsg::Error(_) => {
                            err = true;
                            finished = true;
                        }
                    }
                }
            }
        }
        if let Some(tree) = self.trees.get_mut(&id) {
            for d in new_dirs {
                tree.push_child(path, d);
            }
            if finished {
                let outcome = if err {
                    NodeOutcome::Error
                } else {
                    NodeOutcome::Done
                };
                tree.finish_loading(path, outcome);
            }
        }
        if finished {
            // El estado del nodo vive en el DirTree; el worker terminado se elimina
            // (a diferencia del file panel, que conserva la entrada con rx=None).
            // El árbol se re-expande sin re-listar vía expand_loaded.
            self.tree_listings.remove(&key);
        }
    }

    fn any_tree_listing_active(&self) -> bool {
        self.tree_listings.values().any(|l| l.rx.is_some())
    }

    /// Aplica una acción al panel activo.
    pub fn apply_action(&mut self, action: Action) {
        match action {
            Action::MoveUp => self.move_focus(-1),
            Action::MoveDown => self.move_focus(1),
            Action::Activate => self.activate_focused(),
            Action::GoUp => self.nav(|f| f.go_up()),
            Action::GoBack => self.nav(|f| f.go_back()),
            Action::GoForward => self.nav(|f| f.go_forward()),
            Action::CancelListing => {
                if let Some(id) = self.workspace.active_id() {
                    if let Some(l) = self.listings.get(&id) {
                        l.token.cancel();
                    }
                }
            }
            Action::SwitchPane => self.cycle_active_files(),
        }
    }

    /// Ejecuta una navegación sobre el panel activo y, si cambió de carpeta, lanza
    /// el listado nuevo.
    fn nav(&mut self, f: impl FnOnce(&mut FilePaneState) -> Option<PathBuf>) {
        let Some(active) = self.workspace.active_id() else {
            return;
        };
        let moved = self
            .workspace
            .pane_mut(active)
            .and_then(|p| p.files.as_mut())
            .and_then(f);
        if let Some(dir) = moved {
            self.start_listing(active, dir);
        }
    }

    fn move_focus(&mut self, delta: isize) {
        if let Some(f) = self.workspace.active_files_mut() {
            if f.entries.is_empty() {
                return;
            }
            let len = f.entries.len() as isize;
            let cur = f.focused.unwrap_or(0) as isize;
            f.focused = Some((cur + delta).clamp(0, len - 1) as usize);
        }
    }

    fn activate_focused(&mut self) {
        let Some(active) = self.workspace.active_id() else {
            return;
        };
        let entry = self
            .workspace
            .pane(active)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.focused_entry().cloned());
        let Some(entry) = entry else { return };
        if entry.is_dir() {
            if let Some(f) = self
                .workspace
                .pane_mut(active)
                .and_then(|p| p.files.as_mut())
            {
                f.navigate_to(entry.path.clone());
            }
            self.start_listing(active, entry.path);
        } else {
            self.status = self
                .i18n
                .t("status.open_pending")
                .replace("{name}", &entry.name);
        }
    }

    fn cycle_active_files(&mut self) {
        let files: Vec<PaneId> = self
            .workspace
            .panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Files)
            .map(|p| p.id)
            .collect();
        if files.is_empty() {
            return;
        }
        let cur = self.workspace.active_id();
        let idx = files.iter().position(|id| Some(*id) == cur).unwrap_or(0);
        let next = files[(idx + 1) % files.len()];
        self.workspace.set_active(next);
    }

    fn typeahead(&mut self, typed: &str) {
        if typed.is_empty() {
            return;
        }
        self.typeahead_buf.push_str(typed);
        let buf = self.typeahead_buf.clone();
        if let Some(f) = self.workspace.active_files_mut() {
            let names: Vec<String> = f.entries.iter().map(|e| e.name.clone()).collect();
            let start = f.focused.unwrap_or(0);
            if let Some(i) = crate::typeahead::find_match(&names, &buf, start) {
                f.focused = Some(i);
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let keys = [
            (egui::Key::ArrowUp, NaygoKey::ArrowUp),
            (egui::Key::ArrowDown, NaygoKey::ArrowDown),
            (egui::Key::ArrowLeft, NaygoKey::ArrowLeft),
            (egui::Key::Enter, NaygoKey::Enter),
            (egui::Key::Backspace, NaygoKey::Backspace),
            (egui::Key::Tab, NaygoKey::Tab),
            (egui::Key::Escape, NaygoKey::Escape),
        ];
        let mut actions = Vec::new();
        let mut typed = String::new();
        ctx.input(|i| {
            let alt = i.modifiers.alt;
            if alt && i.key_pressed(egui::Key::ArrowLeft) {
                actions.push(Action::GoBack);
            } else if alt && i.key_pressed(egui::Key::ArrowRight) {
                actions.push(Action::GoForward);
            } else {
                for (egui_key, naygo_key) in keys {
                    if i.key_pressed(egui_key) {
                        if let Some(a) = map_key(naygo_key) {
                            actions.push(a);
                        }
                    }
                }
            }
            if i.pointer.button_pressed(egui::PointerButton::Extra1) {
                actions.push(map_mouse_extra(MouseExtra::Back));
            }
            if i.pointer.button_pressed(egui::PointerButton::Extra2) {
                actions.push(map_mouse_extra(MouseExtra::Forward));
            }
            for event in &i.events {
                if let egui::Event::Text(t) = event {
                    typed.push_str(t);
                }
            }
        });

        if !actions.is_empty() {
            self.typeahead_buf.clear();
        }
        for a in actions {
            self.apply_action(a);
        }
        if !typed.is_empty() {
            self.typeahead(&typed);
        }
    }

    /// Guarda el workspace persistible.
    fn save_workspace(&self) {
        let files = self
            .workspace
            .panes()
            .iter()
            .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.to_persist())))
            .collect();
        let purposes = self
            .workspace
            .panes()
            .iter()
            .map(|p| (p.id, p.purpose))
            .collect();
        let persist = config::WorkspacePersist {
            version: 1,
            layout: self.workspace.layout.clone(),
            active: self.workspace.active_id(),
            files,
            purposes,
        };
        config::save_workspace(&self.config_dir, &persist);
    }
}

impl eframe::App for NaygoApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump_all();
        self.pump_tree();
        self.handle_input(ctx);
        if self.any_listing_active() || self.any_tree_listing_active() {
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.icons.set() != self.settings.icon_set {
            let set = self.settings.icon_set;
            self.icons.reload(ui.ctx(), set);
        }

        // Aplica un cambio de idioma. También sirve a la ventana de Configuración
        // (viewport): un cambio hecho ahí este frame se aplica al inicio del
        // siguiente (la ventana repinta cada frame, así que el relabel es inmediato
        // a la vista).
        if self.i18n.active_lang() != self.settings.language {
            let lang = self.settings.language.clone();
            self.i18n.set_language(&lang);
        }

        crate::toolbar::show(ui, self);

        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let dir = self
                    .workspace
                    .active_files()
                    .map(|f| f.current_dir.display().to_string())
                    .unwrap_or_default();
                ui.label(dir);
                ui.separator();
                ui.label(&self.status);
            });
        });

        // --- Sincronización del árbol con el panel Files activo ---
        // Aseguramos un DirTree por cada panel Tree y lo apuntamos a la carpeta del
        // panel activo, expandiendo la cadena de ancestros necesaria para revelarla.
        let active_dir = self.workspace.active_files().map(|f| f.current_dir.clone());
        let tree_pane_ids: Vec<PaneId> = self
            .workspace
            .panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Tree)
            .map(|p| p.id)
            .collect();
        for id in &tree_pane_ids {
            self.ensure_tree(*id);
        }
        // CAVEAT de casing: reveal_chain/node_at usan starts_with/== que en Windows
        // son sensibles a mayúsculas. Las raíces vienen de GetLogicalDriveStringsW
        // (devuelve "C:\" en mayúscula) y current_dir suele ser canónico, así que en
        // la práctica coinciden. No normalizamos rutas aquí (sería frágil); si la
        // letra de unidad difiriera, el auto-reveal simplemente no encontraría la
        // cadena (degrada sin romper).
        if let Some(dir) = active_dir.clone() {
            for id in &tree_pane_ids {
                // Re-ejecutar cada frame mientras active_path == dir es inofensivo:
                // los niveles ya cargados se saltan (children.is_some()) y los que
                // están en vuelo se saltan por el guard contains_key de tree_expand.
                let needs: Vec<PathBuf> = if let Some(tree) = self.trees.get_mut(id) {
                    if tree.active_path.as_deref() != Some(dir.as_path()) {
                        tree.set_active(dir.clone());
                    }
                    tree.reveal_chain(&dir)
                        .into_iter()
                        .filter(|anc| {
                            tree.node_at(anc)
                                .map(|n| n.children.is_none())
                                .unwrap_or(false)
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                for anc in needs {
                    self.tree_expand(*id, anc);
                }
            }
        }

        let mut pending: Vec<crate::docking::PaneRequest> = Vec::new();
        let mut tree_actions: Vec<(PaneId, crate::tree_actions::TreeAction)> = Vec::new();
        let mut tree_revealed: HashSet<PaneId> = HashSet::new();
        let mut table_actions: Vec<(PaneId, crate::table_actions::TableAction)> = Vec::new();
        {
            let mut viewer = crate::docking::NaygoTabViewer {
                workspace: &mut self.workspace,
                status: &mut self.status,
                pending: &mut pending,
                icons: &self.icons,
                show_parent_entry: self.settings.show_parent_entry,
                i18n: &self.i18n,
                trees: &self.trees,
                tree_actions: &mut tree_actions,
                tree_revealed: &mut tree_revealed,
                table_actions: &mut table_actions,
            };
            egui_dock::DockArea::new(&mut self.dock_state)
                .style(egui_dock::Style::from_egui(ui.style().as_ref()))
                .show_inside(ui, &mut viewer);
        }
        for req in pending {
            match req {
                crate::docking::PaneRequest::Activate { id } => {
                    self.workspace.set_active(id);
                }
                crate::docking::PaneRequest::NavigateTo { id, dir } => {
                    // Solo navegar/listar si el panel es Files: evita lanzar un
                    // worker inútil contra un panel Tree/Inspector si en el futuro
                    // alguno de esos llega a ser el activo.
                    if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                        f.navigate_to(dir.clone());
                        self.start_listing(id, dir);
                    }
                }
            }
        }

        // Acciones del árbol acumuladas durante el pintado.
        for (id, action) in tree_actions {
            match action {
                crate::tree_actions::TreeAction::Expand(path) => self.tree_expand(id, path),
                crate::tree_actions::TreeAction::Collapse(path) => self.tree_collapse(id, path),
                crate::tree_actions::TreeAction::Navigate(path) => {
                    if let Some(active) = self.workspace.active_id() {
                        if let Some(f) = self
                            .workspace
                            .pane_mut(active)
                            .and_then(|p| p.files.as_mut())
                        {
                            f.navigate_to(path.clone());
                            self.start_listing(active, path);
                        }
                    }
                }
            }
        }

        // Acciones de tabla (menú de columna) acumuladas durante el pintado.
        for (id, action) in table_actions {
            if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                match action {
                    crate::table_actions::TableAction::SetSort(spec) => {
                        f.sort = spec;
                    }
                    crate::table_actions::TableAction::SetFilter(kind, filter) => {
                        f.table.set_filter(kind, filter);
                    }
                    crate::table_actions::TableAction::ClearFilter(kind) => {
                        f.table.clear_filter(kind);
                    }
                    crate::table_actions::TableAction::ToggleColumn(kind) => {
                        f.table.toggle_visible(kind);
                    }
                    crate::table_actions::TableAction::MoveColumn(from, to) => {
                        f.table.move_column(from, to);
                    }
                    crate::table_actions::TableAction::SetColumnWidth(kind, w) => {
                        f.table.set_width(kind, w);
                    }
                }
            }
        }

        // Limpiar el reveal SOLO si el nodo objetivo se pintó (y se hizo scroll) este
        // frame. Si el objetivo aún no está cargado/pintado (revelado en cascada),
        // reveal_to persiste hasta que aparezca; el repaint por workers activos
        // garantiza más frames. Así el scroll a una carpeta profunda recién navegada
        // sí ocurre, en vez de perderse el primer frame.
        for id in &tree_pane_ids {
            if tree_revealed.contains(id) {
                if let Some(t) = self.trees.get_mut(id) {
                    t.clear_reveal();
                }
            }
        }

        if self.settings_open {
            let ctx = ui.ctx().clone();
            crate::settings_window::show_settings_viewport(self, &ctx);
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save_workspace();
        config::save_settings(&self.config_dir, &self.settings);
        config::save_templates(&self.config_dir, &self.templates);
    }
}

/// Carpeta inicial: home del usuario o C:\ como fallback.
fn default_start_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("C:\\"))
}

/// Carga el workspace persistido y lo reconstruye, o cae al Dual-pane default.
fn load_or_default_workspace(dir: &Path, home: &Path) -> Workspace {
    if let Some(persist) = config::load_workspace(dir) {
        if let Some(w) = rebuild_workspace(persist, home) {
            return w;
        }
    }
    Workspace::from_template(&LayoutTemplate::dual_pane(), home)
}

/// Reconstruye un `Workspace` desde lo persistido. `None` si el layout es
/// inconsistente (el llamador cae al default). Tolera carpetas inexistentes: el
/// panel se queda con su ruta y el listado mostrará el error.
fn rebuild_workspace(persist: config::WorkspacePersist, _home: &Path) -> Option<Workspace> {
    let mut w = Workspace::new();
    let files_map: HashMap<PaneId, _> = persist.files.into_iter().collect();
    let layout_ids = persist.layout.pane_ids();
    if layout_ids.is_empty() {
        return None;
    }
    let mut remap: HashMap<PaneId, PaneId> = HashMap::new();
    for old_id in &layout_ids {
        let purpose = persist
            .purposes
            .iter()
            .find(|(pid, _)| pid == old_id)
            .map(|(_, p)| *p)?;
        let new_id = match purpose {
            PanePurpose::Files => {
                let fp = files_map.get(old_id)?;
                let state = FilePaneState::from_persist(fp.clone());
                let dir = state.current_dir.clone();
                let id = w.add_pane(PanePurpose::Files, dir);
                if let Some(p) = w.pane_mut(id) {
                    if let Some(f) = p.files.as_mut() {
                        f.sort = state.sort;
                        f.view = state.view;
                        f.show_dirs = state.show_dirs;
                    }
                }
                id
            }
            other => w.add_pane(other, PathBuf::new()),
        };
        remap.insert(*old_id, new_id);
    }
    w.layout = remap_layout(&persist.layout, &remap);
    if let Some(old_active) = persist.active {
        if let Some(new_active) = remap.get(&old_active) {
            w.set_active(*new_active);
        }
    }
    Some(w)
}

/// Reescribe los PaneId de un layout según el mapa old→new.
fn remap_layout(
    layout: &naygo_core::workspace::layout::SerializableDockLayout,
    remap: &HashMap<PaneId, PaneId>,
) -> naygo_core::workspace::layout::SerializableDockLayout {
    use naygo_core::workspace::layout::{DockNode, SerializableDockLayout};
    fn go(node: &DockNode, remap: &HashMap<PaneId, PaneId>) -> DockNode {
        match node {
            DockNode::Leaf(id) => DockNode::Leaf(*remap.get(id).unwrap_or(id)),
            DockNode::Split {
                dir,
                fraction,
                first,
                second,
            } => DockNode::Split {
                dir: *dir,
                fraction: *fraction,
                first: Box::new(go(first, remap)),
                second: Box::new(go(second, remap)),
            },
        }
    }
    SerializableDockLayout {
        root: layout.root.as_ref().map(|n| go(n, remap)),
    }
}

/// Traduce el SerializableDockLayout actual a un LayoutShape (índices) para
/// guardar como plantilla.
fn layout_to_shape(
    layout: &naygo_core::workspace::layout::SerializableDockLayout,
    index_of: &std::collections::HashMap<PaneId, usize>,
) -> naygo_core::workspace::template::LayoutShape {
    use naygo_core::workspace::layout::DockNode;
    use naygo_core::workspace::template::LayoutShape;
    fn go(node: &DockNode, index_of: &std::collections::HashMap<PaneId, usize>) -> LayoutShape {
        match node {
            DockNode::Leaf(id) => LayoutShape::Leaf(*index_of.get(id).unwrap_or(&0)),
            DockNode::Split {
                dir,
                fraction,
                first,
                second,
            } => LayoutShape::Split {
                dir: *dir,
                fraction: *fraction,
                first: Box::new(go(first, index_of)),
                second: Box::new(go(second, index_of)),
            },
        }
    }
    layout
        .root
        .as_ref()
        .map(|n| go(n, index_of))
        .unwrap_or(LayoutShape::Leaf(0))
}
