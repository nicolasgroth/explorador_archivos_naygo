// Naygo — estado raíz de la aplicación y loop de egui (multi-panel).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `NaygoApp` mantiene un `Workspace` de N paneles independientes. Cada panel
//! `Files` lista en su propio worker (un `PaneListing`); el hilo de UI drena
//! todos los canales sin bloquear. El teclado y los botones del mouse actúan
//! sobre el panel activo. El layout y las carpetas se persisten vía `config`.

use crate::icons::IconProvider;
use crate::input::{map_key, map_mouse_extra, Action, Key as NaygoKey, MouseExtra};
use crate::ops_dialogs::{ConflictChoice, NameResult};
use crate::settings_window::SettingsSection;
use crate::theme_apply::{self, ActiveTheme};
use eframe::CreationContext;
use egui_dock::DockState;
use naygo_core::cancel::CancellationToken;
use naygo_core::config::{self, Settings};
use naygo_core::i18n::{pick_default_language, I18n, LangId};
use naygo_core::listing::{spawn_listing, spawn_listing_filtered, ListingFilter, ListingMsg};
use naygo_core::ops::journal::{self, JournalWriter, OpJournal};
use naygo_core::ops::{
    self, ConflictDecision, ConflictPolicy, OpKind, OpMsg, OpOutcome, OpPlan, OpProgress,
    OpRequest, OpSummary,
};
use naygo_core::sort::sort_entries;
use naygo_core::theme::pack::PackCatalog;
use naygo_core::theme::ThemeCatalog;
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

/// Una operación de archivo en curso (o terminada, mostrándose en el panel).
pub struct ActiveOp {
    pub rx: Option<Receiver<OpMsg>>,
    pub token: CancellationToken,
    pub label: String, // p. ej. "Copiar → D:\backup"
    pub progress: Option<OpProgress>,
    pub summary: Option<OpSummary>,
    pub started: bool, // false = en cola, true = corriendo
    /// Plan+kind+conflict pendientes de lanzar (modo cola). None si ya se lanzó.
    pub pending: Option<(OpPlan, OpKind, ConflictPolicy)>,
    /// Id del journal de esta op (None = no journaleada, p. ej. papelera).
    pub journal_id: Option<String>,
}

/// Trabajo de escritura de un archivo pegado (corre en un worker corto).
enum WriteJob {
    Text {
        path: std::path::PathBuf,
        body: String,
    },
    Image {
        path: std::path::PathBuf,
        fmt: naygo_core::clipboard::ImageFmt,
        img: naygo_core::clipboard::ClipboardImage,
        quality: u8,
    },
}

/// Carga del diálogo de confirmación de pegado (modo B): lo que se escribirá una
/// vez que el usuario confirme nombre (y, para imagen, formato).
pub(crate) enum PastePreviewKind {
    Text {
        body: String,
    },
    Image {
        img: naygo_core::clipboard::ClipboardImage,
        fmt: naygo_core::clipboard::ImageFmt,
        quality: u8,
    },
}

/// Separa el nombre de archivo en (stem sin extensión, extensión sin punto).
fn split_stem_ext(path: &std::path::Path) -> (String, String) {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    (stem, ext)
}

/// Resumen del archivo escrito (para el status/toast).
enum PasteOk {
    Text {
        bytes: u64,
        chars: usize,
        lines: usize,
    },
    Image {
        w: u32,
        h: u32,
        fmt: &'static str,
        bytes: u64,
    },
}

impl WriteJob {
    fn path(&self) -> &std::path::Path {
        match self {
            WriteJob::Text { path, .. } | WriteJob::Image { path, .. } => path,
        }
    }

    fn run(self) -> Result<PasteOk, String> {
        match self {
            WriteJob::Text { path, body } => {
                let chars = body.chars().count();
                let lines = if body.is_empty() {
                    0
                } else {
                    body.lines().count().max(1)
                };
                std::fs::write(&path, &body).map_err(|e| e.to_string())?;
                Ok(PasteOk::Text {
                    bytes: body.len() as u64,
                    chars,
                    lines,
                })
            }
            WriteJob::Image {
                path,
                fmt,
                img,
                quality,
            } => {
                let (w, h) = (img.width, img.height);
                let bytes = naygo_core::clipboard::encode::encode_image(&img, fmt, quality)
                    .map_err(|e| format!("{e:?}"))?;
                let len = bytes.len() as u64;
                std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
                Ok(PasteOk::Image {
                    w,
                    h,
                    fmt: fmt.ext(),
                    bytes: len,
                })
            }
        }
    }
}

/// Diálogo modal pendiente de confirmación. Lleva la `OpRequest` ya armada; al
/// confirmar se ajusta su política de conflicto / nombre y se llama a `start_op`.
/// Resolver el conflicto AQUÍ (antes de lanzar) garantiza que el motor nunca
/// reciba `ConflictPolicy::Ask` (su canal de conflicto se descarta en ops-A).
pub enum PendingDialog {
    /// Confirmar borrado (papelera si `permanent==false`, irreversible si true).
    ConfirmDelete {
        req: OpRequest,
        label: String,
        permanent: bool,
        count: usize,
    },
    /// Resolver un conflicto de nombre antes de copiar/mover.
    Conflict {
        req: OpRequest,
        label: String,
        /// Nombre del primer choque (para el cuerpo del modal).
        name: String,
    },
    /// Renombrar el elemento `source`: pide el nombre nuevo.
    Rename {
        source: std::path::PathBuf,
        buf: String,
    },
    /// Crear archivo/carpeta en `dir`: pide el nombre.
    Create {
        dir: std::path::PathBuf,
        is_dir: bool,
        buf: String,
    },
    /// Confirmar nombre/formato antes de crear un archivo pegado (modo B).
    PastePreview {
        /// Carpeta destino (el nombre final = dir/<name>.<ext>).
        dir: std::path::PathBuf,
        /// Nombre editable (SIN extensión).
        name_buf: String,
        /// Extensión actual (sin punto) — para texto fija; para imagen sigue al formato.
        ext: String,
        /// Contenido a escribir.
        kind: PastePreviewKind,
    },
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
    theme_catalog: ThemeCatalog,
    pack_catalog: PackCatalog,
    active_theme: ActiveTheme,
    pub settings_open: bool,
    pub settings_section: SettingsSection,
    /// Operaciones de archivo en curso/terminadas (panel de progreso).
    active_ops: Vec<ActiveOp>,
    /// Diálogo modal pendiente (confirmación/conflicto/nombre), si hay uno.
    pending_dialog: Option<PendingDialog>,
    /// Si el panel de operaciones muestra el detalle expandido.
    ops_panel_expanded: bool,
    /// Ops interrumpidas detectadas al arrancar, pendientes de decisión (modal).
    pending_resume: Vec<OpJournal>,
    /// Escritura de un archivo pegado (texto/imagen) en curso en un worker. El hilo de
    /// UI NO bloquea: se drena el resultado por frame. `dir` = carpeta a refrescar.
    pending_paste_write: Option<PendingPasteWrite>,
}

/// Escritura de un archivo pegado en curso (worker + canal de resultado).
struct PendingPasteWrite {
    rx: std::sync::mpsc::Receiver<Result<PasteOk, String>>,
    dir: Option<std::path::PathBuf>,
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

        let theme_catalog = ThemeCatalog::load(&config_dir, &settings.theme);
        let active_theme = {
            let t = theme_catalog.get(&settings.theme).clone();
            ActiveTheme::new(settings.theme.clone(), t)
        };
        theme_apply::apply(&active_theme.theme, &cc.egui_ctx);

        let pack_catalog = PackCatalog::load(&config_dir);

        // Ops interrumpidas de una sesión anterior: se ofrecen retomar al arrancar.
        let pending_resume = journal::scan(&config_dir);

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
            theme_catalog,
            pack_catalog,
            active_theme,
            settings_open: false,
            settings_section: SettingsSection::Appearance,
            active_ops: Vec::new(),
            pending_dialog: None,
            ops_panel_expanded: false,
            pending_resume,
            pending_paste_write: None,
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

    /// Ids + Theme de cada tema disponible (para pintar las tarjetas del selector).
    pub fn theme_cards(&self) -> Vec<(naygo_core::theme::ThemeId, naygo_core::theme::Theme)> {
        self.theme_catalog
            .available()
            .iter()
            .map(|id| (id.clone(), self.theme_catalog.get(id).clone()))
            .collect()
    }

    /// Packs disponibles (catálogo cargado una vez en `new`).
    pub fn packs(&self) -> Vec<naygo_core::theme::pack::Pack> {
        self.pack_catalog.packs().to_vec()
    }

    /// Activa un pack: setea tema + icon set (siguen independientes después).
    pub fn apply_pack(&mut self, pack: &naygo_core::theme::pack::Pack) {
        self.settings.theme = pack.theme.clone();
        self.settings.icon_set = pack.icon_set;
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

    /// Genera un id único para un journal de operación (timestamp en nanos).
    fn next_journal_id(&self) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("op-{now}")
    }

    /// Crea el journal de una op si es journaleable (copy/move/borrado permanente).
    /// Devuelve `(id, writer)` o None. La papelera NUNCA llega aquí (atómica).
    fn make_journal(
        &self,
        kind: &OpKind,
        conflict: ConflictPolicy,
        plan: &OpPlan,
    ) -> Option<(String, JournalWriter)> {
        let journaled = matches!(
            kind,
            OpKind::Copy | OpKind::Move | OpKind::Delete { to_trash: false }
        );
        if !journaled {
            return None;
        }
        let id = self.next_journal_id();
        let j = OpJournal::new(id.clone(), kind.clone(), conflict, plan.clone());
        Some((id, JournalWriter::new(&self.config_dir, j)))
    }

    /// Muestra el modal de retomar si hay ops interrumpidas y procesa la decisión.
    fn process_resume_dialog(&mut self, ctx: &egui::Context) {
        if self.pending_resume.is_empty() {
            return;
        }
        let items: Vec<(String, String, usize, usize)> = self
            .pending_resume
            .iter()
            .map(|j| (j.id.clone(), j.label(), j.done_through, j.plan.steps.len()))
            .collect();
        let Some(choice) = crate::ops_dialogs::resume_dialog(ctx, &self.i18n, &items) else {
            return;
        };
        use crate::ops_dialogs::ResumeChoice;
        // IDs a retomar y a descartar según la elección.
        let (resume_ids, discard_ids): (Vec<String>, Vec<String>) = match choice {
            ResumeChoice::Resume(id) => (vec![id], vec![]),
            ResumeChoice::Discard(id) => (vec![], vec![id]),
            ResumeChoice::ResumeAll => (
                self.pending_resume.iter().map(|j| j.id.clone()).collect(),
                vec![],
            ),
            ResumeChoice::DiscardAll => (
                vec![],
                self.pending_resume.iter().map(|j| j.id.clone()).collect(),
            ),
        };
        // Descartar: borrar journal (se quitan de pending_resume al final).
        for id in &discard_ids {
            journal::remove(&self.config_dir, id);
        }
        // Retomar: tomar el journal, podar el plan, relanzar reusando el id.
        for id in &resume_ids {
            if let Some(pos) = self.pending_resume.iter().position(|j| &j.id == id) {
                let j = self.pending_resume[pos].clone();
                let r = journal::resume_plan(&j);
                if r.plan.steps.is_empty() {
                    // Nada pendiente que retomar: borrar el journal.
                    journal::remove(&self.config_dir, id);
                } else {
                    if !r.skipped_changed.is_empty() {
                        self.status = self
                            .i18n
                            .t("resume.skipped_changed")
                            .replace("{n}", &r.skipped_changed.len().to_string());
                    }
                    self.start_resumed_op(
                        id.clone(),
                        j.kind.clone(),
                        j.conflict,
                        r.plan,
                        j.label(),
                    );
                }
            }
        }
        // Quitar de pending_resume todo lo procesado.
        self.pending_resume
            .retain(|j| !resume_ids.contains(&j.id) && !discard_ids.contains(&j.id));
    }

    /// Retoma una operación desde un plan ya podado, reusando el `id` de journal.
    fn start_resumed_op(
        &mut self,
        id: String,
        kind: OpKind,
        conflict: ConflictPolicy,
        plan: OpPlan,
        label: String,
    ) {
        let token = CancellationToken::new();
        let (_ctx, crx) = std::sync::mpsc::channel::<ConflictDecision>();
        let j = OpJournal::new(id.clone(), kind.clone(), conflict, plan.clone());
        let writer = JournalWriter::new(&self.config_dir, j);
        let (rx, _h) = ops::spawn(plan, kind, conflict, token.clone(), crx, Some(writer));
        self.active_ops.push(ActiveOp {
            rx: Some(rx),
            token,
            label,
            progress: None,
            summary: None,
            started: true,
            pending: None,
            journal_id: Some(id),
        });
    }

    /// Lanza una operación: planifica, spawnea (o encola). Papelera = atómica vía
    /// platform (no pasa por el motor core). `label` se muestra en el panel.
    pub fn start_op(&mut self, req: OpRequest, label: String) {
        // Papelera: caso especial atómico vía platform.
        if let OpKind::Delete { to_trash: true } = &req.kind {
            let _ = naygo_platform::trash::move_to_trash(&req.sources);
            if let (Some(id), Some(dir)) = (
                self.workspace.active_id(),
                self.workspace.active_files().map(|f| f.current_dir.clone()),
            ) {
                self.refresh_pane(id, dir);
            }
            return;
        }
        let plan = match ops::plan(&req) {
            Ok(p) => p,
            Err(_e) => return, // plan inválido: en futuro mostrar error en UI
        };
        let queue = self.settings.ops_mode == naygo_core::config::OpsMode::Queue;
        let any_running = self.active_ops.iter().any(|o| o.started && o.rx.is_some());
        let token = CancellationToken::new();
        if queue && any_running {
            // Encolar: guardar plan+kind+conflict; se lanza en pump_ops al liberarse.
            self.active_ops.push(ActiveOp {
                rx: None,
                token,
                label,
                progress: None,
                summary: None,
                started: false,
                pending: Some((plan, req.kind, req.conflict)),
                journal_id: None, // se journalea al lanzarse en pump_ops
            });
        } else {
            let (_ctx, crx) = std::sync::mpsc::channel::<ConflictDecision>();
            let journal = self.make_journal(&req.kind, req.conflict, &plan);
            let (journal_id, writer) = match journal {
                Some((id, w)) => (Some(id), Some(w)),
                None => (None, None),
            };
            let (rx, _h) = ops::spawn(plan, req.kind, req.conflict, token.clone(), crx, writer);
            self.active_ops.push(ActiveOp {
                rx: Some(rx),
                token,
                label,
                progress: None,
                summary: None,
                started: true,
                pending: None,
                journal_id,
            });
        }
    }

    /// Drena canales de las ops y gestiona la cola (lanza la siguiente al liberarse).
    pub fn pump_ops(&mut self) {
        // Drenar mensajes de las ops corriendo. `just_finished` se marca cuando una
        // op pasa a tener summary este pump → refrescamos el panel activo después.
        let mut just_finished = false;
        // Clon previo: no se puede prestar `self.config_dir` dentro del `&mut self.active_ops`.
        let cfg = self.config_dir.clone();
        for op in &mut self.active_ops {
            // Drenar a un buffer local primero: no podemos asignar `op.rx = None`
            // mientras `rx` sigue prestado dentro del while (E0506).
            let mut msgs = Vec::new();
            if let Some(rx) = &op.rx {
                while let Ok(msg) = rx.try_recv() {
                    msgs.push(msg);
                }
            }
            for msg in msgs {
                match msg {
                    OpMsg::Progress(p) => op.progress = Some(p),
                    OpMsg::Done(s) | OpMsg::Cancelled(s) => {
                        op.summary = Some(s);
                        op.rx = None;
                        just_finished = true;
                        // Op concluida (ok o cancelada): el journal ya no aplica.
                        if let Some(id) = &op.journal_id {
                            journal::remove(&cfg, id);
                        }
                    }
                    OpMsg::Failed(_) => {
                        op.rx = None;
                        // El motor nunca emite Failed hoy (los fallos por paso van en
                        // el summary), pero si lo hiciera, borramos el journal igual
                        // para no dejar una op "interrumpida" fantasma que re-pregunte
                        // al arrancar.
                        if let Some(id) = &op.journal_id {
                            journal::remove(&cfg, id);
                        }
                    }
                    OpMsg::Conflict(_) => {} // ops-A resuelve conflicto antes de spawn
                }
            }
        }
        // Una op acabó de tocar el filesystem → re-listar el panel activo para
        // mostrar el resultado (copia/movida/borrada).
        if just_finished {
            if let (Some(id), Some(dir)) = (
                self.workspace.active_id(),
                self.workspace.active_files().map(|f| f.current_dir.clone()),
            ) {
                self.refresh_pane(id, dir);
            }
        }
        // Lanzar la siguiente pendiente (cola) si no hay ninguna corriendo.
        let none_running_now = !self.active_ops.iter().any(|o| o.started && o.rx.is_some());
        if none_running_now {
            if let Some(idx) = self.active_ops.iter().position(|o| o.pending.is_some()) {
                if let Some((plan, kind, conflict)) = self.active_ops[idx].pending.take() {
                    let token = self.active_ops[idx].token.clone();
                    let (_ctx, crx) = std::sync::mpsc::channel::<ConflictDecision>();
                    let journal = self.make_journal(&kind, conflict, &plan);
                    let (journal_id, writer) = match journal {
                        Some((id, w)) => (Some(id), Some(w)),
                        None => (None, None),
                    };
                    let (rx, _h) = ops::spawn(plan, kind, conflict, token, crx, writer);
                    self.active_ops[idx].rx = Some(rx);
                    self.active_ops[idx].started = true;
                    self.active_ops[idx].journal_id = journal_id;
                }
            }
        }
    }

    pub fn any_op_active(&self) -> bool {
        self.active_ops
            .iter()
            .any(|o| o.rx.is_some() || o.pending.is_some())
    }

    /// Quita las ops realmente terminadas y sin nada que mostrar. Regla: una op se
    /// descarta cuando no corre (`rx==None`), no está en cola (`pending==None`) y
    /// NO tiene summary. Eso captura las que `Failed` (rx=None, sin summary). Las
    /// Done/Cancelled conservan su summary → se quedan visibles hasta que el panel
    /// se cierre (no hay ops) o llegue Task 11 con un "limpiar" explícito.
    fn prune_finished_ops(&mut self) {
        // Si el usuario desactivó el resumen, las ops terminadas se descartan en
        // cuanto dejan de correr: no se conserva su summary para mostrarlo.
        let keep_summaries = self.settings.show_op_summary;
        self.active_ops.retain(|o| {
            o.rx.is_some() || o.pending.is_some() || (keep_summaries && o.summary.is_some())
        });
    }

    /// Escribe un reporte de texto del resumen de la op `index` a un archivo en el
    /// directorio del panel activo: `<dir>/naygo-ops-<unix_secs>.txt`. Sin selector
    /// nativo (eso es de la fase platform/shell). El reporte es pequeño, así que la
    /// escritura síncrona es aceptable. Deja la ruta (o el error) en `self.status`.
    fn export_op_summary(&mut self, index: usize) {
        let Some(summary) = self.active_ops.get(index).and_then(|o| o.summary.clone()) else {
            return;
        };

        // Directorio destino: el del panel activo; si no hay, el dir de config.
        let dir = self
            .workspace
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| self.config_dir.clone());

        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("naygo-ops-{secs}.txt"));

        let report = build_summary_report(&summary);

        match std::fs::write(&path, report) {
            Ok(()) => {
                self.status = self
                    .i18n
                    .t("ops.exported")
                    .replace("{path}", &path.display().to_string());
            }
            Err(e) => {
                self.status = self
                    .i18n
                    .t("ops.export_failed")
                    .replace("{e}", &e.to_string());
            }
        }
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
            Action::Open => {
                if let Some((p, n)) = self.focused_file() {
                    self.open_path(&p, &n);
                }
            }
            Action::OpenWith => {
                if let Some((p, n)) = self.focused_file() {
                    self.open_with_path(&p, &n);
                }
            }
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
            Action::Copy => self.clipboard_set(false),
            Action::Cut => self.clipboard_set(true),
            Action::Paste => self.paste(),
            Action::Delete => self.delete_selection(false),
            Action::DeletePermanent => self.delete_selection(true),
            Action::Rename => self.begin_rename(),
            Action::NewFile => self.begin_create(false),
            Action::NewDir => self.begin_create(true),
            Action::CopyToOther => self.transfer_to_other(false),
            Action::MoveToOther => self.transfer_to_other(true),
        }
    }

    /// Rutas seleccionadas en el panel activo (mapeadas vista→entries). Si no hay
    /// selección, usa la entry enfocada. Vacío si no hay nada. Las rutas son las
    /// de las entries reales (no la fila virtual "..").
    fn selected_paths(&self) -> Vec<PathBuf> {
        let Some(f) = self.workspace.active_files() else {
            return Vec::new();
        };
        let view = f.view_indices();
        // NOTA: `f.selected` está RESERVADO para una futura multi-selección y hoy la UI
        // nunca lo puebla (siempre vacío en ops-A) → esta rama es código correcto-por-
        // contrato pero inactivo; en la práctica se usa el fallback al foco de abajo.
        // `selected` y `focused` viven en espacio de VISTA (pos en view_indices()), por
        // eso se mapea pos→real antes de indexar `entries`.
        if !f.selected.is_empty() {
            return f
                .selected
                .iter()
                .filter_map(|&pos| view.get(pos))
                .filter_map(|&real| f.entries.get(real))
                .map(|e| e.path.clone())
                .collect();
        }
        // Sin multi-selección: la entry enfocada.
        f.focused_view_entry()
            .map(|e| vec![e.path.clone()])
            .unwrap_or_default()
    }

    /// Carpeta del panel de archivos activo.
    fn active_dir(&self) -> Option<PathBuf> {
        self.workspace.active_files().map(|f| f.current_dir.clone())
    }

    /// La carpeta del OTRO panel `Files` (para F5/F6). `None` si solo hay uno.
    fn other_files_dir(&self) -> Option<PathBuf> {
        let active = self.workspace.active_id();
        self.workspace
            .panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Files && Some(p.id) != active)
            .find_map(|p| p.files.as_ref().map(|f| f.current_dir.clone()))
    }

    /// Copia/corta la selección al portapapeles del SISTEMA (CF_HDROP + DropEffect),
    /// para interoperar con el Explorador de Windows y otras apps.
    fn clipboard_set(&mut self, cut: bool) {
        let paths = self.selected_paths();
        if paths.is_empty() {
            return;
        }
        if let Err(e) = naygo_platform::clipboard::write_files(&paths, cut) {
            // Detalle técnico al log; al usuario, un mensaje traducible discreto.
            tracing::warn!("write_files al portapapeles falló: {e:?}");
            self.status = self.i18n.t("paste.copy_error").to_string();
        }
    }

    /// ¿Existe ya `dest_dir/name` para alguna de las fuentes de nivel superior?
    /// Devuelve el primer nombre que choca (para el cuerpo del modal de conflicto).
    fn first_collision(sources: &[PathBuf], dest_dir: &Path) -> Option<String> {
        for src in sources {
            if let Some(name) = src.file_name() {
                if dest_dir.join(name).exists() {
                    return Some(name.to_string_lossy().into_owned());
                }
            }
        }
        None
    }

    /// Lanza una transferencia (copia/movida) resolviendo conflictos ANTES de
    /// spawnear. Si hay colisión y la política es Ask, abre el modal de conflicto;
    /// si no, usa `Overwrite` (no habrá conflicto) y arranca.
    fn launch_transfer(&mut self, mut req: OpRequest, label: String) {
        let Some(dest) = req.dest_dir.clone() else {
            return;
        };
        match Self::first_collision(&req.sources, &dest) {
            Some(name) => {
                // Hay choque y la política aún es Ask → preguntar.
                self.pending_dialog = Some(PendingDialog::Conflict { req, label, name });
            }
            None => {
                // Sin choque: Overwrite es inocuo (no se gatilla ningún conflicto).
                req.conflict = ConflictPolicy::Overwrite;
                self.start_op(req, label);
            }
        }
    }

    /// Escribe un archivo pegado (texto o imagen) en un worker corto: el hilo de UI
    /// no hace la E/S. Al terminar, refresca el panel activo y deja un status con la
    /// metadata; en error, status discreto. (No usa el OpPlan: es un único archivo.)
    fn write_pasted_file(&mut self, job: WriteJob) {
        let dir = job.path().parent().map(|p| p.to_path_buf());
        let (tx, rx) = std::sync::mpsc::channel::<Result<PasteOk, String>>();
        // El worker codifica (imagen) y escribe; el hilo de UI no se bloquea: el
        // resultado se drena por frame en `pump_paste_write`. Una imagen grande puede
        // tardar en codificar, así que NO esperamos aquí.
        std::thread::spawn(move || {
            let _ = tx.send(job.run());
        });
        self.pending_paste_write = Some(PendingPasteWrite { rx, dir });
    }

    /// Drena el resultado de una escritura de archivo pegado (si terminó). Deja un
    /// status con la metadata y refresca el panel; en error, status discreto.
    fn pump_paste_write(&mut self) {
        use naygo_core::format::human_size;
        let Some(pending) = &self.pending_paste_write else {
            return;
        };
        let result = match pending.rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return, // aún en curso
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(String::new()),
        };
        // Terminó: consumir el pendiente.
        let dir = self.pending_paste_write.take().and_then(|p| p.dir);
        match result {
            Ok(PasteOk::Text {
                bytes,
                chars,
                lines,
            }) => {
                self.status = self
                    .i18n
                    .t("paste.done_text")
                    .replace("{bytes}", &human_size(bytes))
                    .replace("{chars}", &chars.to_string())
                    .replace("{lines}", &lines.to_string());
                if let (Some(id), Some(d)) = (self.workspace.active_id(), dir) {
                    self.refresh_pane(id, d);
                }
            }
            Ok(PasteOk::Image { w, h, fmt, bytes }) => {
                self.status = self
                    .i18n
                    .t("paste.done_image")
                    .replace("{w}", &w.to_string())
                    .replace("{h}", &h.to_string())
                    .replace("{fmt}", fmt)
                    .replace("{bytes}", &human_size(bytes));
                if let (Some(id), Some(d)) = (self.workspace.active_id(), dir) {
                    self.refresh_pane(id, d);
                }
            }
            Err(_) => self.status = self.i18n.t("paste.error").to_string(),
        }
    }

    /// Pega el portapapeles del SISTEMA en la carpeta activa según su tipo:
    /// archivos → copiar/mover (motor de ops); texto → .txt; imagen → png/jpg.
    fn paste(&mut self) {
        let Some(dest) = self.active_dir() else {
            return;
        };
        let content = naygo_platform::clipboard::read();
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let exists = |p: &std::path::Path| p.exists();
        let plan =
            naygo_core::clipboard::decide_paste(&content, &dest, &self.settings, now_secs, &exists);
        use naygo_core::clipboard::PastePlan;
        match plan {
            PastePlan::Transfer { paths, cut } => {
                let req = crate::ops_actions::transfer(cut, paths, dest.clone());
                let verb = if cut {
                    self.i18n.t("op.cut")
                } else {
                    self.i18n.t("op.paste")
                };
                let label = format!("{verb} → {}", dest.display());
                self.launch_transfer(req, label);
            }
            PastePlan::CreateText { path, body } => {
                if self.settings.paste_confirm {
                    let (stem, ext) = split_stem_ext(&path);
                    self.pending_dialog = Some(PendingDialog::PastePreview {
                        dir: path
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or(dest.clone()),
                        name_buf: stem,
                        ext,
                        kind: PastePreviewKind::Text { body },
                    });
                } else {
                    self.write_pasted_file(WriteJob::Text { path, body });
                }
            }
            PastePlan::CreateImage { path, fmt, img } => {
                if self.settings.paste_confirm {
                    let (stem, _ext) = split_stem_ext(&path);
                    self.pending_dialog = Some(PendingDialog::PastePreview {
                        dir: path
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or(dest.clone()),
                        name_buf: stem,
                        ext: fmt.ext().to_string(),
                        kind: PastePreviewKind::Image {
                            img,
                            fmt,
                            quality: self.settings.paste_jpg_quality,
                        },
                    });
                } else {
                    self.write_pasted_file(WriteJob::Image {
                        path,
                        fmt,
                        img,
                        quality: self.settings.paste_jpg_quality,
                    });
                }
            }
            PastePlan::Nothing => {
                self.status = self.i18n.t("paste.empty").to_string();
            }
        }
    }

    /// Copia/mueve la selección al OTRO panel (F5 copia, F6 mueve).
    fn transfer_to_other(&mut self, move_it: bool) {
        let sources = self.selected_paths();
        if sources.is_empty() {
            return;
        }
        let Some(dest) = self.other_files_dir() else {
            return;
        };
        let req = crate::ops_actions::transfer(move_it, sources, dest.clone());
        let verb = if move_it {
            self.i18n.t("op.cut")
        } else {
            self.i18n.t("op.copy")
        };
        let label = format!("{verb} → {}", dest.display());
        self.launch_transfer(req, label);
    }

    /// Elimina la selección. Papelera: confirma solo si `settings.confirm_trash`.
    /// Permanente: confirma SIEMPRE (irreversible).
    fn delete_selection(&mut self, permanent: bool) {
        let sources = self.selected_paths();
        if sources.is_empty() {
            return;
        }
        let count = sources.len();
        let to_trash = !permanent;
        let req = crate::ops_actions::delete(sources, to_trash);
        let label = if permanent {
            self.i18n.t("op.delete_permanent").to_string()
        } else {
            self.i18n.t("op.delete").to_string()
        };
        let needs_confirm = permanent || self.settings.confirm_trash;
        if needs_confirm {
            self.pending_dialog = Some(PendingDialog::ConfirmDelete {
                req,
                label,
                permanent,
                count,
            });
        } else {
            self.start_op(req, label);
        }
    }

    /// Abre el modal de renombrar para la entry enfocada (precarga su nombre).
    fn begin_rename(&mut self) {
        let entry = self
            .workspace
            .active_files()
            .and_then(|f| f.focused_view_entry())
            .map(|e| (e.path.clone(), e.name.clone()));
        if let Some((source, name)) = entry {
            self.pending_dialog = Some(PendingDialog::Rename { source, buf: name });
        }
    }

    /// Abre el modal de crear archivo/carpeta en la carpeta activa.
    fn begin_create(&mut self, is_dir: bool) {
        if let Some(dir) = self.active_dir() {
            self.pending_dialog = Some(PendingDialog::Create {
                dir,
                is_dir,
                buf: String::new(),
            });
        }
    }

    /// Pinta el diálogo modal pendiente (si hay) y, al recibir una decisión,
    /// finaliza la operación: ajusta la política/nombre del request y la lanza.
    /// Se llama cada frame desde `ui()`.
    fn process_pending_dialog(&mut self, ctx: &egui::Context) {
        // `take` para poder mutar `self` (start_op) sin un préstamo doble; si el
        // modal sigue abierto sin decisión, lo volvemos a colocar al final.
        let Some(dialog) = self.pending_dialog.take() else {
            return;
        };
        match dialog {
            PendingDialog::ConfirmDelete {
                req,
                label,
                permanent,
                count,
            } => match crate::ops_dialogs::confirm_delete(ctx, &self.i18n, count, permanent) {
                Some(true) => self.start_op(req, label),
                Some(false) => {} // cancelado: descartar
                None => {
                    self.pending_dialog = Some(PendingDialog::ConfirmDelete {
                        req,
                        label,
                        permanent,
                        count,
                    });
                }
            },
            PendingDialog::Conflict { req, label, name } => {
                match crate::ops_dialogs::conflict(ctx, &self.i18n, &name) {
                    Some(choice) => self.resolve_conflict(req, label, choice),
                    None => {
                        self.pending_dialog = Some(PendingDialog::Conflict { req, label, name });
                    }
                }
            }
            PendingDialog::Rename { source, mut buf } => {
                match crate::ops_dialogs::name_input(ctx, &self.i18n, "op.rename_title", &mut buf) {
                    Some(NameResult::Confirmed(new_name)) => {
                        let req = crate::ops_actions::rename(source, new_name);
                        let label = self.i18n.t("op.rename").to_string();
                        // Renombrar no genera conflicto vía nuestro flujo de paste;
                        // el motor maneja el choque con la política del request. Para
                        // ops-A usamos Overwrite (decisión simple) — el plan de rename
                        // es un solo paso.
                        let mut req = req;
                        req.conflict = ConflictPolicy::Overwrite;
                        self.start_op(req, label);
                    }
                    Some(NameResult::Cancelled) => {}
                    None => {
                        self.pending_dialog = Some(PendingDialog::Rename { source, buf });
                    }
                }
            }
            PendingDialog::Create {
                dir,
                is_dir,
                mut buf,
            } => {
                let title = if is_dir {
                    "op.new_folder_title"
                } else {
                    "op.new_file_title"
                };
                match crate::ops_dialogs::name_input(ctx, &self.i18n, title, &mut buf) {
                    Some(NameResult::Confirmed(name)) => {
                        let mut req = crate::ops_actions::create(dir, name, is_dir);
                        req.conflict = ConflictPolicy::Overwrite;
                        let label = if is_dir {
                            self.i18n.t("op.new_folder").to_string()
                        } else {
                            self.i18n.t("op.new_file").to_string()
                        };
                        self.start_op(req, label);
                    }
                    Some(NameResult::Cancelled) => {}
                    None => {
                        self.pending_dialog = Some(PendingDialog::Create { dir, is_dir, buf });
                    }
                }
            }
            PendingDialog::PastePreview {
                dir,
                mut name_buf,
                ext,
                kind,
            } => {
                // Datos para el modal: se toman PRESTADOS de `kind` antes de moverlo.
                let is_image = matches!(kind, PastePreviewKind::Image { .. });
                let image_dims = match &kind {
                    PastePreviewKind::Image { img, .. } => Some((img.width, img.height)),
                    _ => None,
                };
                let mut fmt_opt = match &kind {
                    PastePreviewKind::Image { fmt, .. } => Some(*fmt),
                    _ => None,
                };
                match crate::ops_dialogs::paste_preview(
                    ctx,
                    &self.i18n,
                    is_image,
                    &mut name_buf,
                    &ext,
                    image_dims,
                    &mut fmt_opt,
                ) {
                    Some(crate::ops_dialogs::PastePreviewResult::Create { name, fmt }) => {
                        // Extensión final: la imagen la toma del formato elegido; el
                        // texto conserva la suya.
                        let final_ext = match (is_image, fmt) {
                            (true, Some(f)) => f.ext().to_string(),
                            _ => ext.clone(),
                        };
                        let mut path = dir.join(format!("{name}.{final_ext}"));
                        // Deduplica si ya existe (nombre (1), (2), ...).
                        path = naygo_core::ops::dedup_name(&path, &|p| p.exists());
                        let job = match kind {
                            PastePreviewKind::Text { body } => WriteJob::Text { path, body },
                            PastePreviewKind::Image { img, quality, .. } => WriteJob::Image {
                                path,
                                fmt: fmt.unwrap_or(self.settings.paste_image_fmt),
                                img,
                                quality,
                            },
                        };
                        self.write_pasted_file(job);
                    }
                    Some(crate::ops_dialogs::PastePreviewResult::Cancelled) => {}
                    None => {
                        self.pending_dialog = Some(PendingDialog::PastePreview {
                            dir,
                            name_buf,
                            ext,
                            kind,
                        });
                    }
                }
            }
        }
    }

    /// Aplica la elección de conflicto a un request de copia/movida y lo lanza.
    /// Skip cancela la operación (en ops-A el conflicto es a nivel de request).
    fn resolve_conflict(&mut self, mut req: OpRequest, label: String, choice: ConflictChoice) {
        match choice {
            ConflictChoice::Overwrite => req.conflict = ConflictPolicy::Overwrite,
            ConflictChoice::Rename => req.conflict = ConflictPolicy::Rename,
            ConflictChoice::Skip => return, // saltar = no hacer nada
        }
        self.start_op(req, label);
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
            // El foco es una posición en la VISTA (entries que pasan el filtro), no
            // en `entries` crudas: navegar con flechas se mueve por lo que se ve.
            let view_len = f.view_indices().len();
            if view_len == 0 {
                return;
            }
            let cur = f.focused.unwrap_or(0) as isize;
            f.focused = Some((cur + delta).clamp(0, view_len as isize - 1) as usize);
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
            .and_then(|f| f.focused_view_entry().cloned());
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
            let (path, name) = (entry.path.clone(), entry.name.clone());
            self.open_path(&path, &name);
        }
    }

    /// Abre un archivo con su app por defecto; deja status de éxito/error.
    fn open_path(&mut self, path: &std::path::Path, name: &str) {
        match naygo_platform::open::open_default(path) {
            Ok(()) => {
                self.status = self.i18n.t("status.opening").replace("{name}", name);
            }
            Err(naygo_platform::open::ShellError::NoAssociation) => {
                self.status = self.i18n.t("status.no_association").replace("{name}", name);
            }
            Err(_) => {
                self.status = self.i18n.t("status.open_failed").replace("{name}", name);
            }
        }
    }

    /// Abre el diálogo "Abrir con…" del SO para un archivo; status en error.
    fn open_with_path(&mut self, path: &std::path::Path, name: &str) {
        if naygo_platform::open::open_with_dialog(path).is_err() {
            self.status = self.i18n.t("status.open_failed").replace("{name}", name);
        }
    }

    /// Resuelve la entry enfocada del panel activo (ruta + nombre), si es archivo.
    fn focused_file(&self) -> Option<(std::path::PathBuf, String)> {
        let entry = self
            .workspace
            .active_files()
            .and_then(|f| f.focused_view_entry().cloned())?;
        if entry.is_dir() {
            None
        } else {
            Some((entry.path, entry.name))
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
            // Type-ahead opera sobre la VISTA: los nombres y el índice resultante son
            // posiciones en la vista filtrada (consistente con foco/selección).
            let view = f.view_indices();
            let names: Vec<String> = view
                .iter()
                .map(|&real| f.entries[real].name.clone())
                .collect();
            let start = f.focused.unwrap_or(0);
            if let Some(i) = crate::typeahead::find_match(&names, &buf, start) {
                f.focused = Some(i); // i es posición en la VISTA
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // Si hay un diálogo modal abierto, él se queda con el teclado: no
        // procesamos navegación ni disparadores (evita que escribir "C/V/N" en el
        // campo de nombre gatille Copiar/Pegar/Nuevo, o que Delete borre detrás).
        // El modal de retomar (al arrancar) cuenta igual: bloquea hasta decidir.
        if self.pending_dialog.is_some() || !self.pending_resume.is_empty() {
            return;
        }
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
            let ctrl = i.modifiers.ctrl;
            let shift = i.modifiers.shift;
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

            // Disparadores de operaciones (con modificadores; no salen del mapa
            // simple de `input::map_key`). Se evalúan aparte de la navegación.
            if ctrl && i.key_pressed(egui::Key::C) {
                actions.push(Action::Copy);
            }
            if ctrl && i.key_pressed(egui::Key::X) {
                actions.push(Action::Cut);
            }
            if ctrl && i.key_pressed(egui::Key::V) {
                actions.push(Action::Paste);
            }
            if ctrl && shift && i.key_pressed(egui::Key::N) {
                actions.push(Action::NewDir);
            } else if ctrl && i.key_pressed(egui::Key::N) {
                actions.push(Action::NewFile);
            }
            if shift && i.key_pressed(egui::Key::Delete) {
                actions.push(Action::DeletePermanent);
            } else if i.key_pressed(egui::Key::Delete) {
                actions.push(Action::Delete);
            }
            if i.key_pressed(egui::Key::F2) {
                actions.push(Action::Rename);
            }
            if i.key_pressed(egui::Key::F5) {
                actions.push(Action::CopyToOther);
            }
            if i.key_pressed(egui::Key::F6) {
                actions.push(Action::MoveToOther);
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
        self.pump_ops();
        self.pump_paste_write();
        self.handle_input(ctx);
        if self.any_listing_active()
            || self.any_tree_listing_active()
            || self.any_op_active()
            || self.pending_paste_write.is_some()
        {
            ctx.request_repaint();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.icons.set() != self.settings.icon_set {
            let set = self.settings.icon_set;
            self.icons.reload(ui.ctx(), set);
        }

        if self.active_theme.id != self.settings.theme {
            let t = self.theme_catalog.get(&self.settings.theme).clone();
            self.active_theme = ActiveTheme::new(self.settings.theme.clone(), t);
            theme_apply::apply(&self.active_theme.theme, ui.ctx());
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

        // Panel de operaciones (acoplado abajo). Se muestra si hay ops o si el modo
        // de display es "siempre visible". En modo Modal seguimos mostrándolo como
        // panel (un modal de progreso a pantalla completa es de Task 11+); aquí el
        // panel cumple para ops-A. Las ✕ devueltas se cancelan tras pintar.
        let show_panel = !self.active_ops.is_empty()
            || self.settings.ops_display == naygo_core::config::OpsDisplay::AlwaysVisible;
        if show_panel {
            let active_ops = &self.active_ops;
            let i18n = &self.i18n;
            let expanded = &mut self.ops_panel_expanded;
            let mut output = crate::ops_panel::OpsPanelOutput::default();
            egui::Panel::bottom("ops_panel")
                .resizable(true)
                .show_inside(ui, |ui| {
                    output = crate::ops_panel::show(ui, active_ops, i18n, expanded);
                });
            for i in output.cancel {
                if let Some(op) = self.active_ops.get(i) {
                    op.token.cancel();
                }
            }
            if let Some(i) = output.export {
                self.export_op_summary(i);
            }
            self.prune_finished_ops();
        }

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
        let mut ops_actions: Vec<Action> = Vec::new();
        {
            let mut viewer = crate::docking::NaygoTabViewer {
                workspace: &mut self.workspace,
                status: &mut self.status,
                pending: &mut pending,
                icons: &self.icons,
                theme: &self.active_theme,
                show_parent_entry: self.settings.show_parent_entry,
                i18n: &self.i18n,
                trees: &self.trees,
                tree_actions: &mut tree_actions,
                tree_revealed: &mut tree_revealed,
                table_actions: &mut table_actions,
                ops_actions: &mut ops_actions,
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

        // Disparadores de operaciones del menú contextual. Se aplican DESPUÉS del
        // `pending` (que ya enfocó/activó la fila del clic derecho), así las acciones
        // basadas en foco actúan sobre la entry correcta.
        for action in ops_actions {
            self.apply_action(action);
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
                        // Re-ordenar `entries` en sitio (como hace `pump_one`) para que
                        // `view_indices()` (orden de entries) coincida con el orden que
                        // pinta el file_panel. Si no, foco/teclado divergirían de la vista
                        // tras cambiar el orden sin re-listar.
                        sort_entries(&mut f.entries, &spec);
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

        // Diálogo modal pendiente (confirmar/conflicto/nombre). Se pinta al final
        // para quedar sobre todo. Si hay uno abierto, repintamos para que el modal
        // responda con fluidez aunque no haya workers activos.
        if self.pending_dialog.is_some() {
            let ctx = ui.ctx().clone();
            self.process_pending_dialog(&ctx);
            ui.ctx().request_repaint();
        }

        // Modal de retomar ops interrumpidas (detectadas al arrancar). Se pinta al
        // final por la misma razón que el modal anterior.
        if !self.pending_resume.is_empty() {
            let ctx = ui.ctx().clone();
            self.process_resume_dialog(&ctx);
            ui.ctx().request_repaint();
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save_workspace();
        config::save_settings(&self.config_dir, &self.settings);
        config::save_templates(&self.config_dir, &self.templates);
    }
}

/// Carpeta inicial: home del usuario o C:\ como fallback.
/// Construye el reporte de texto plano del resumen de una operación. Una línea de
/// cabecera con conteos + bytes + tiempo, y luego una línea por archivo:
/// `<OUTCOME>\t<ruta>` (con el motivo tras un tab más en los `FAILED`).
fn build_summary_report(summary: &OpSummary) -> String {
    let mut out = String::new();
    out.push_str("Naygo — resumen de operación\n");
    out.push_str(&format!(
        "Hechos: {}  Omitidos: {}  Con error: {}\n",
        summary.count_done(),
        summary.count_skipped(),
        summary.count_failed()
    ));
    out.push_str(&format!(
        "Bytes: {}  Tiempo: {:.1}s\n",
        summary.bytes_done, summary.elapsed_secs
    ));
    out.push_str("---\n");
    for (path, outcome) in &summary.items {
        match outcome {
            OpOutcome::Done => out.push_str(&format!("DONE\t{}\n", path.display())),
            OpOutcome::Skipped => out.push_str(&format!("SKIPPED\t{}\n", path.display())),
            OpOutcome::Failed(reason) => {
                out.push_str(&format!("FAILED\t{}\t{}\n", path.display(), reason))
            }
        }
    }
    out
}

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
                        // Restaurar el estado de tabla persistido (columnas visibles/
                        // orden/ancho + filtros). Sin esto, la persistencia de la Fase
                        // 2E y la migración de text_filter se descartaban al cargar.
                        f.table = state.table;
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
