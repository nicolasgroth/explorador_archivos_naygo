// Naygo — estado raíz de la aplicación y loop de egui (multi-panel).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `NaygoApp` mantiene un `Workspace` de N paneles independientes. Cada panel
//! `Files` lista en su propio worker (un `PaneListing`); el hilo de UI drena
//! todos los canales sin bloquear. El teclado y los botones del mouse actúan
//! sobre el panel activo. El layout y las carpetas se persisten vía `config`.

use crate::icons::IconProvider;
use crate::input::{map_mouse_extra, Action, MouseExtra};
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

/// Un cálculo de tamaño de carpeta en curso.
struct SizeJob {
    rx: std::sync::mpsc::Receiver<naygo_core::sizing::SizeMsg>,
    token: CancellationToken,
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
    /// Request original, para construir el DESHACER al completar. None = no se
    /// registra (ops que ya son un deshacer, o retomadas tras un crash).
    pub request: Option<OpRequest>,
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
    /// Snapshot de los settings tal como quedaron GUARDADOS por última vez. Si al final
    /// de un frame difieren de `settings`, se guarda de inmediato (persistencia real:
    /// eframe sin la feature `persistence` jamás llama a `App::save`).
    last_saved_settings: Settings,
    /// Último autosave del workspace (cada `WORKSPACE_AUTOSAVE` también se guarda, para
    /// tolerar cierres abruptos sin perder más que ese intervalo).
    last_workspace_autosave: std::time::Instant,
    /// La sección Avanzado pidió restaurar valores de fábrica; se procesa en `logic`.
    pub(crate) factory_reset_requested: bool,
    /// Logo para la sección "Acerca de" (lazy; se decodifica al abrirla por 1ª vez).
    pub(crate) about_logo: Option<egui::TextureHandle>,
    /// Easter egg de "Acerca de": clics encadenados sobre el logo.
    pub(crate) egg_clicks: u8,
    /// Momento del último clic del egg (para encadenar dentro de la ventana de 2 s).
    pub(crate) egg_last_click: Option<std::time::Instant>,
    /// El egg está activo hasta este instante (None = inactivo).
    pub(crate) egg_until: Option<std::time::Instant>,
    /// Rename inline en curso (F2). Suspende el input global mientras existe.
    pub(crate) inline_rename: Option<InlineRename>,
    /// Diálogo de batch-rename abierto (F2 con multi-selección). Suspende el input
    /// global mientras existe (mismo trato que `pending_dialog`).
    pub(crate) batch_rename: Option<crate::batch_rename_dialog::BatchRenameState>,
    /// Historial de deshacer de la sesión (más nuevo al final; tope 100).
    pub(crate) undo_history: Vec<naygo_core::ops::undo::UndoEntry>,
    /// Id incremental de las entradas del historial.
    next_undo_id: u64,
    /// Ícono en la bandeja del sistema (None = deshabilitado o falló al crear).
    tray: Option<crate::tray::Tray>,
    /// La creación del tray falló (no reintentar cada frame).
    tray_failed: bool,
    /// "Salir" del menú del tray: cierre REAL aunque `close_to_tray` esté activo.
    quit_requested: bool,
    /// La ventana está oculta en la bandeja (mantiene un latido para drenar eventos).
    hidden_to_tray: bool,
    pub templates: TemplateStore,
    config_dir: PathBuf,
    pub status: String,
    typeahead_buf: String,
    /// Atajos de teclado configurables.
    pub(crate) keymap: naygo_core::keymap::KeyMap,
    /// Acción cuyo nuevo atajo se está capturando en el editor (suspende el input global).
    pub(crate) shortcut_capture: Option<naygo_core::keymap::Action>,
    /// Texto del buscador de acciones del editor de atajos.
    pub(crate) shortcut_search: String,
    /// Mensaje del banner de conflicto tras un bind que robó un atajo (efímero).
    pub(crate) shortcut_conflict: Option<String>,
    pub(crate) icons: IconProvider,
    i18n: I18n,
    theme_catalog: ThemeCatalog,
    pack_catalog: PackCatalog,
    pub(crate) active_theme: ActiveTheme,
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
    /// Espacio por unidad (root → uso), rellenado async por un worker; lo pinta el árbol.
    disk_usage: std::collections::HashMap<std::path::PathBuf, naygo_core::disk::DiskUsage>,
    /// Canal del worker de espacio en curso (None si no hay escaneo activo).
    disk_rx: Option<std::sync::mpsc::Receiver<(std::path::PathBuf, naygo_core::disk::DiskUsage)>>,
    /// Frames desde el último escaneo (re-escaneo periódico sin reloj).
    disk_scan_ticks: u32,
    /// Últimas unidades vistas por el escaneo de discos (para el strip del toolbar).
    /// Se refresca al inicio de cada escaneo; barato (drives() no toca espacio).
    pub(crate) drives_cache: Vec<naygo_platform::drives::DriveInfo>,
    /// Watcher de carpeta por panel (vigila la carpeta visible de cada FilePane).
    watchers: std::collections::HashMap<PaneId, naygo_platform::dir_watch::WatchHandle>,
    /// Receptores de eventos de carpeta por panel.
    watch_rx: std::collections::HashMap<
        PaneId,
        std::sync::mpsc::Receiver<Vec<naygo_core::listing::DirEvent>>,
    >,
    /// Instante en que se resaltó algo por última vez en cada panel. Solo se usa con
    /// `HighlightDuration::FadeSeconds`: cuando transcurre el plazo, se limpia el
    /// resaltado del panel (estado efímero, no se persiste).
    highlight_since: std::collections::HashMap<PaneId, std::time::Instant>,
    /// Watcher de dispositivos (pendrives) — ventana message-only Win32.
    device_watch: Option<naygo_platform::device_watch::DeviceWatchHandle>,
    /// Receptor de eventos de dispositivos.
    device_rx: Option<std::sync::mpsc::Receiver<naygo_platform::device_watch::DeviceEvent>>,
    /// Cálculos de tamaño en curso, por (panel, carpeta).
    size_jobs: std::collections::HashMap<(PaneId, std::path::PathBuf), SizeJob>,
    /// Paths con tamaño calculado (para recalcular en F5), por panel.
    sized_paths: std::collections::HashMap<PaneId, std::collections::HashSet<std::path::PathBuf>>,
    /// Paths cuyo tamaño calculado es PARCIAL (accesos denegados) — para el render.
    size_partial: std::collections::HashSet<std::path::PathBuf>,
    /// Splash de arranque (solo release): pinta el logo un instante al iniciar y se
    /// limpia (None) al expirar o al primer input. En debug siempre es None.
    splash: Option<crate::splash::Splash>,
    /// HWND de la ventana para el menú contextual nativo (shell-B). None → ítem deshabilitado.
    hwnd: Option<isize>,
    /// Petición de menú contextual nativo (shell-B): coords de PANTALLA del clic. La procesa NaygoApp fuera del closure de egui.
    native_menu_request: Option<(f32, f32)>,
}

/// Escritura de un archivo pegado en curso (worker + canal de resultado).
struct PendingPasteWrite {
    rx: std::sync::mpsc::Receiver<Result<PasteOk, String>>,
    dir: Option<std::path::PathBuf>,
}

/// Estado del rename inline (F2 en serie, R1). `pos` es la posición en la VISTA
/// (filtrada/ordenada) del panel. `stage`: 0 = nombre sin extensión, 1 = extensión,
/// 2 = todo (el ciclo de F2). Los `*_pending` se consumen al pintar la celda.
pub struct InlineRename {
    pub pane: PaneId,
    /// Ancla por PATH (no por posición: el refresh tras un rename reordena/vacía la
    /// vista unos frames y una posición quedaría colgando).
    pub path: std::path::PathBuf,
    pub text: String,
    pub stage: u8,
    pub focus_pending: bool,
    pub select_pending: bool,
    /// Frames seguidos en que el path no aparece en la vista (listado en vuelo).
    pub missing_frames: u8,
}

impl NaygoApp {
    pub fn new(cc: &CreationContext<'_>, initial_dir: Option<std::path::PathBuf>) -> Self {
        let config_dir = config::portable_dir();
        // Carga con ARRANQUE SEGURO: un archivo corrupto se respalda como .bad y se
        // arranca con defaults; el flag alimenta el aviso en la barra de estado.
        let (settings, settings_recovered) = config::load_settings_flagged(&config_dir);
        let templates = config::load_templates(&config_dir);
        let keymap = config::load_keymap(&config_dir);
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
        // Primer arranque (o recuperación): persistir de inmediato los settings
        // iniciales para que settings.json exista desde ya.
        if !settings_exists {
            config::save_settings(&config_dir, &settings);
        }

        let (workspace, workspace_recovered) = load_or_default_workspace(&config_dir, &home);
        let dock_state = crate::dock_translate::to_dock_state(&workspace.layout);
        // Aviso de recuperación (config corrupta respaldada) en la barra de estado.
        let initial_status = if settings_recovered || workspace_recovered {
            i18n.t("status.config_recovered").to_string()
        } else {
            String::new()
        };

        let icons = IconProvider::new(&cc.egui_ctx, &settings.icon_set, &config_dir);

        let theme_catalog = ThemeCatalog::load(&config_dir, &settings.theme);
        let active_theme = {
            let t = theme_catalog.get(&settings.theme).clone();
            ActiveTheme::new(settings.theme.clone(), t)
        };
        theme_apply::apply(&active_theme.theme, &cc.egui_ctx);

        let pack_catalog = PackCatalog::load(&config_dir);

        // Ops interrumpidas de una sesión anterior: se ofrecen retomar al arrancar.
        let pending_resume = journal::scan(&config_dir);

        let (dev_tx, dev_rx) = std::sync::mpsc::channel();
        let device_watch = Some(naygo_platform::device_watch::watch(dev_tx));

        // Splash de arranque: solo en release. En debug no hay splash (None).
        #[cfg(debug_assertions)]
        let splash = None;
        #[cfg(not(debug_assertions))]
        let splash = crate::splash::Splash::new(&cc.egui_ctx);

        // HWND de la ventana (para el menú contextual nativo de Windows, shell-B). Si no
        // se puede obtener, queda None y el ítem "Más opciones de Windows…" se deshabilita.
        let hwnd: Option<isize> = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            match cc.window_handle() {
                Ok(h) => match h.as_raw() {
                    RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
                    _ => None,
                },
                Err(_) => None,
            }
        };

        // Recibir arrastres del SO (Explorador → Naygo): NO registramos un IDropTarget
        // propio. winit ya registra el suyo y egui-winit nos entrega los archivos soltados
        // en el input crudo (`pump_dropped_files` los lee). Un RegisterDragDrop propio
        // chocaría con el de winit (DRAGDROP_E_ALREADYREGISTERED) y nuestro OleInitialize/
        // OleUninitialize perturbaba el estado OLE del hilo de UI al arrancar.

        let mut app = NaygoApp {
            workspace,
            dock_state,
            listings: HashMap::new(),
            trees: HashMap::new(),
            tree_listings: HashMap::new(),
            last_saved_settings: settings.clone(),
            settings,
            last_workspace_autosave: std::time::Instant::now(),
            factory_reset_requested: false,
            about_logo: None,
            egg_clicks: 0,
            egg_last_click: None,
            egg_until: None,
            inline_rename: None,
            batch_rename: None,
            undo_history: Vec::new(),
            next_undo_id: 1,
            tray: None,
            tray_failed: false,
            quit_requested: false,
            hidden_to_tray: false,
            templates,
            config_dir,
            status: initial_status,
            typeahead_buf: String::new(),
            keymap,
            shortcut_capture: None,
            shortcut_search: String::new(),
            shortcut_conflict: None,
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
            disk_usage: std::collections::HashMap::new(),
            disk_rx: None,
            disk_scan_ticks: 180,
            drives_cache: Vec::new(),
            watchers: std::collections::HashMap::new(),
            watch_rx: std::collections::HashMap::new(),
            highlight_since: std::collections::HashMap::new(),
            device_watch,
            device_rx: Some(dev_rx),
            size_jobs: HashMap::new(),
            sized_paths: HashMap::new(),
            size_partial: std::collections::HashSet::new(),
            splash,
            hwnd,
            native_menu_request: None,
        };
        app.start_all_listings();

        // Carpeta inicial por línea de comandos (naygo.exe <ruta>): si se pasó una
        // carpeta válida, el panel activo se navega ahí; si no, queda el arranque normal.
        if let Some(dir) = initial_dir {
            app.navigate_active_to(dir);
        }
        app
    }

    /// Atajo para traducir una clave con el idioma activo.
    pub fn tr(&self, key: &str) -> String {
        self.i18n.t(key).to_string()
    }

    /// Guarda el keymap a disco tras una edición.
    pub(crate) fn save_keymap_now(&mut self) {
        naygo_core::config::save_keymap(&self.config_dir, &self.keymap);
    }

    /// Si estamos capturando un atajo, lee la próxima combinación y la asigna. Esc cancela.
    /// Debe llamarse cada frame, ANTES de handle_input. Devuelve `true` si consumió una
    /// tecla este frame (capturó o canceló) — el llamador debe entonces SALTAR handle_input
    /// para que esa misma tecla no dispare además su acción.
    fn process_shortcut_capture(&mut self, ctx: &egui::Context) -> bool {
        let Some(action) = self.shortcut_capture else {
            return false;
        };
        let mut captured: Option<naygo_core::keymap::Chord> = None;
        let mut cancel = false;
        ctx.input(|i| {
            let (ctrl, shift, alt) = (i.modifiers.ctrl, i.modifiers.shift, i.modifiers.alt);
            // Esc sin modificadores cancela la captura.
            if i.key_pressed(egui::Key::Escape) && !ctrl && !shift && !alt {
                cancel = true;
                return;
            }
            // Buscar la primera tecla "real" presionada y armar el chord.
            const KEYS: &[egui::Key] = &[
                egui::Key::ArrowUp,
                egui::Key::ArrowDown,
                egui::Key::ArrowLeft,
                egui::Key::ArrowRight,
                egui::Key::Enter,
                egui::Key::Backspace,
                egui::Key::Tab,
                egui::Key::Delete,
                egui::Key::F2,
                egui::Key::F3,
                egui::Key::F5,
                egui::Key::F6,
                egui::Key::Space,
                egui::Key::A,
                egui::Key::B,
                egui::Key::C,
                egui::Key::D,
                egui::Key::E,
                egui::Key::F,
                egui::Key::G,
                egui::Key::H,
                egui::Key::I,
                egui::Key::J,
                egui::Key::K,
                egui::Key::L,
                egui::Key::M,
                egui::Key::N,
                egui::Key::O,
                egui::Key::P,
                egui::Key::Q,
                egui::Key::R,
                egui::Key::S,
                egui::Key::T,
                egui::Key::U,
                egui::Key::V,
                egui::Key::W,
                egui::Key::X,
                egui::Key::Y,
                egui::Key::Z,
            ];
            for &k in KEYS {
                if i.key_pressed(k) {
                    if let Some(code) = crate::input::egui_key_to_code(k) {
                        captured = Some(naygo_core::keymap::Chord {
                            key: code,
                            ctrl,
                            shift,
                            alt,
                        });
                        break;
                    }
                }
            }
        });
        if cancel {
            self.shortcut_capture = None;
            return true; // consumió el Esc; no dejar que handle_input lo procese
        }
        if let Some(chord) = captured {
            let chord_txt = crate::input::chord_text(&chord);
            if let Some(robbed) = self.keymap.bind(action, chord) {
                self.shortcut_conflict = Some(
                    self.i18n
                        .t("settings.shortcuts.conflict")
                        .replace("{chord}", &chord_txt)
                        .replace("{from}", self.i18n.t(robbed.i18n_key()))
                        .replace("{to}", self.i18n.t(action.i18n_key())),
                );
            } else {
                self.shortcut_conflict = None;
            }
            self.shortcut_capture = None;
            self.save_keymap_now();
            return true; // consumió la tecla recién asignada; no la dispares también
        }
        // Seguimos capturando (solo modificadores / tecla no soportada): el input global
        // ya está suspendido por la guarda de handle_input mientras shortcut_capture es Some.
        false
    }

    /// Idiomas disponibles (clonados, para la UI sin prestar `self.i18n`).
    pub fn i18n_available(&self) -> Vec<LangId> {
        self.i18n.available().to_vec()
    }

    /// Ruta de la carpeta de config (para la sección Avanzado).
    pub fn config_dir_display(&self) -> String {
        self.config_dir.display().to_string()
    }

    /// Carpeta de config (para descubrir packs/sets sueltos desde la UI de Ajustes).
    pub fn config_dir(&self) -> &std::path::Path {
        &self.config_dir
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
        use naygo_core::config::IconSet;
        self.settings.theme = pack.theme.clone();
        // `Pack` aún expone el enum embebido; lo mapeamos al id string del set.
        self.settings.icon_set = match pack.icon_set {
            IconSet::Flat => "flat",
            IconSet::Fluent => "fluent",
            IconSet::Mono => "mono",
        }
        .to_string();
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
        // (Re)vigilar la carpeta recién listada: el watcher del panel pasa a apuntar a
        // `dir`. El insert reemplaza el handle anterior, cuyo Drop detiene el watcher viejo.
        // Se hace antes de mover `dir` al worker de listado.
        self.rewatch_pane(id, &dir);
        let token = CancellationToken::new();
        let (rx, _handle) = spawn_listing(dir, token.clone());
        self.listings.insert(
            id,
            PaneListing {
                rx: Some(rx),
                token,
            },
        );
        // Re-listar la carpeta es un "refresco": el resaltado de aparecidos previo deja
        // de tener sentido (la vista se reconstruye), salvo en UntilInteract donde solo
        // lo limpia la interacción del usuario.
        if self.settings.highlight_duration != naygo_core::config::HighlightDuration::UntilInteract
        {
            if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.clear_highlight();
            }
            self.highlight_since.remove(&id);
        }
        // Re-listar deja obsoletos los tamaños calculados de la carpeta anterior: cancelar
        // los jobs de sizing de ESTE panel y olvidar su registro (si no, acumularían
        // carpetas de directorios ya no visibles y se re-calcularían en cada refresh).
        // `refresh_pane` ya tomó su snapshot de `sized_paths` ANTES de llamar aquí, así que
        // su recálculo no se ve afectado; esto limpia el caso de navegación normal.
        self.size_jobs.retain(|(pane_id, _), job| {
            if *pane_id == id {
                job.token.cancel();
                false
            } else {
                true
            }
        });
        if let Some(paths) = self.sized_paths.remove(&id) {
            for p in paths {
                self.size_partial.remove(&p);
            }
        }
        // Feedback "Listando…" mientras el panel activo carga (se reemplaza por el
        // conteo de elementos al terminar, en pump_one).
        if self.workspace.active_id() == Some(id) {
            self.status = self.i18n.t("app.loading").to_string();
        }
    }

    /// (Re)crea el watcher de carpeta del panel `id` apuntando a `dir`. Soltar el viejo
    /// (vía insert que reemplaza) libera su handle del SO.
    fn rewatch_pane(&mut self, id: PaneId, dir: &std::path::Path) {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = naygo_platform::dir_watch::watch(dir, tx);
        self.watchers.insert(id, handle);
        self.watch_rx.insert(id, rx);
    }

    /// Poda los paneles cerrados por el usuario: cualquier `PaneId` que ya no esté en el
    /// `DockState` se quita del workspace y de TODOS los mapas por-panel (listings, trees,
    /// tree_listings, watchers, watch_rx, highlight_since). Cancela sus workers y suelta
    /// sus handles del SO (el watcher de su carpeta). Evita fugas y mantiene viva la ruta
    /// "idle" (sin esto, watchers nunca se vacía tras cerrar un panel).
    fn prune_closed_panes(&mut self) {
        let live: std::collections::HashSet<PaneId> =
            crate::dock_translate::dock_pane_ids(&self.dock_state)
                .into_iter()
                .collect();
        let closed: Vec<PaneId> = self
            .workspace
            .panes()
            .iter()
            .map(|p| p.id)
            .filter(|id| !live.contains(id))
            .collect();
        for id in closed {
            // Cancelar el listado en curso de ese panel antes de soltarlo.
            if let Some(l) = self.listings.get(&id) {
                l.token.cancel();
            }
            self.listings.remove(&id);
            self.trees.remove(&id);
            self.watchers.remove(&id); // Drop del WatchHandle detiene el watcher de notify.
            self.watch_rx.remove(&id);
            self.highlight_since.remove(&id);
            self.tree_listings.retain(|(pane_id, _), _| *pane_id != id);
            // Cálculos de tamaño del panel cerrado: cancelar sus tokens y quitarlos (si no,
            // su worker sigue caminando el FS y el repaint nunca se apaga).
            self.size_jobs.retain(|(pane_id, _), job| {
                if *pane_id == id {
                    job.token.cancel();
                    false
                } else {
                    true
                }
            });
            self.sized_paths.remove(&id);
            self.workspace.remove_pane(id);
        }
    }

    /// Drena los eventos de carpeta de cada panel y los fusiona al listado en vivo.
    /// Las rutas nuevas se resaltan; con `FadeSeconds`, el resaltado caducado se limpia.
    fn pump_watchers(&mut self) {
        // Recoger (id, eventos) sin mantener prestado self.watch_rx mientras mutamos panes.
        let mut batches: Vec<(PaneId, Vec<naygo_core::listing::DirEvent>)> = Vec::new();
        for (id, rx) in &self.watch_rx {
            let mut events = Vec::new();
            while let Ok(mut batch) = rx.try_recv() {
                events.append(&mut batch);
            }
            if !events.is_empty() {
                batches.push((*id, events));
            }
        }
        // Umbral de ráfaga: un lote enorme (p. ej. extraer un zip de miles de archivos)
        // haría miles de `metadata()` síncronos en el hilo de UI → congelaría el frame
        // (rompe la regla de oro "el hilo de UI no hace I/O de disco"). En ese caso,
        // descartamos el merge incremental y re-listamos la carpeta fuera del hilo de UI
        // (cancelable). Bajo el debounce normal, un lote trae pocos eventos y se fusiona.
        const BURST_THRESHOLD: usize = 256;
        for (id, events) in batches {
            if events.len() > BURST_THRESHOLD {
                // Ráfaga: re-listar (off-thread) en vez de fusionar evento por evento.
                if let Some(dir) = self
                    .workspace
                    .pane(id)
                    .and_then(|p| p.files.as_ref())
                    .map(|f| f.current_dir.clone())
                {
                    // La selección son posiciones de vista; si el watcher cambió las entries,
                    // dejarían de apuntar al archivo correcto → limpiar (evita operar sobre el
                    // archivo equivocado).
                    if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                        f.clear_selection();
                    }
                    self.start_listing(id, dir);
                }
                continue;
            }
            if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                // El cierre lee la metadata de la ruta en el hilo de UI: barato por lote
                // (pocos archivos por debounce de ~300 ms). Devuelve `None` si la ruta ya
                // no existe (p. ej. el evento "from" de un rename visto suelto), para que
                // `apply_dir_events` no inserte una entry fantasma.
                let read = |p: &std::path::Path| {
                    let meta = std::fs::metadata(p).ok()?;
                    Some(naygo_core::listing::entry_from_path(p, Some(&meta)))
                };
                let nuevas = naygo_core::listing::apply_dir_events(&mut f.entries, &events, &read);
                let spec = f.sort;
                naygo_core::sort::sort_entries(&mut f.entries, &spec);
                // La selección son posiciones de vista; si el watcher cambió las entries,
                // dejarían de apuntar al archivo correcto → limpiar (evita operar sobre el
                // archivo equivocado). El lote llega no vacío (filtrado arriba), así que
                // las entries efectivamente cambiaron y/o se reordenaron.
                f.clear_selection();
                if !nuevas.is_empty() {
                    for p in nuevas {
                        f.highlighted.insert(p);
                    }
                    // Reinicia el reloj del fade en cada lote nuevo (el plazo cuenta desde
                    // el último archivo aparecido).
                    self.highlight_since.insert(id, std::time::Instant::now());
                }
                // Podar del set de resaltado lo que ya no está en el listado (un archivo
                // borrado tras resaltarse): mantiene el set acotado y sin rutas fantasma.
                if !f.highlighted.is_empty() {
                    let present: std::collections::HashSet<_> =
                        f.entries.iter().map(|e| e.path.clone()).collect();
                    f.highlighted.retain(|p| present.contains(p));
                }
            }
        }
        // Caducidad por FadeSeconds: limpia el resaltado del panel cuyo plazo expiró.
        if let naygo_core::config::HighlightDuration::FadeSeconds(secs) =
            self.settings.highlight_duration
        {
            let limite = std::time::Duration::from_secs(secs as u64);
            let caducados: Vec<PaneId> = self
                .highlight_since
                .iter()
                .filter(|(_, &t)| t.elapsed() >= limite)
                .map(|(&id, _)| id)
                .collect();
            for id in caducados {
                if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                    f.clear_highlight();
                }
                self.highlight_since.remove(&id);
            }
        }
    }

    /// Drena eventos de dispositivos: ante un cambio de unidades, re-escanea discos ya.
    fn pump_devices(&mut self) {
        let mut changed = false;
        if let Some(rx) = &self.device_rx {
            while let Ok(_ev) = rx.try_recv() {
                changed = true;
            }
        }
        if changed {
            self.start_disk_scan();
        }
    }

    /// Drena los archivos soltados (vía OLE/`IDropTarget`) sobre la ventana de Naygo y los
    /// transfiere al panel activo, honrando el efecto (mover vs copiar) que decidió el
    /// `IDropTarget` a partir de los modificadores (Shift=mover, Ctrl/sin tecla=copiar).
    /// Esto cubre tanto los drops EXTERNOS (Explorador → Naygo) como los INTRA-app entre
    /// paneles: como `DoDragDrop` bloquea y captura el mouse, todo arrastre de archivos sale
    /// y vuelve por OLE.
    ///
    /// El mapeo coords-de-pantalla → panel exacto bajo el cursor sigue pendiente (Task:
    /// screen→pane): mientras el bucle modal de `DoDragDrop` corre, egui está congelado y no
    /// hay un mapeo directo de coordenadas de pantalla a panel; haría falta rastrear el rect
    /// en pantalla de cada panel por frame. Por ahora los archivos caen en la carpeta del
    /// panel ACTIVO, comportamiento aceptable acordado. Lo que SÍ honramos ya es el efecto.
    fn pump_dropped_files(&mut self, ctx: &egui::Context) {
        // Archivos soltados en la ventana desde el SO (Explorer, escritorio, etc.). winit
        // ya registra su propio IDropTarget y egui-winit nos los entrega en el input crudo;
        // NO registramos un IDropTarget propio (chocaría con el de winit —
        // DRAGDROP_E_ALREADYREGISTERED— y perturbaría el estado OLE del hilo de UI).
        let paths: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if paths.is_empty() {
            return;
        }
        let Some(dest) = self.active_dir() else {
            return;
        };
        // Soltar un archivo en su PROPIA carpeta es un no-op (como Windows): sin esta
        // guarda, arrastrar y soltar dentro del mismo panel intentaría copiar el archivo
        // sobre sí mismo y abriría el diálogo de conflicto.
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .filter(|p| p.parent() != Some(dest.as_path()))
            .collect();
        if paths.is_empty() {
            return;
        }
        // egui no expone el efecto (mover/copiar) del drop externo; copiamos por defecto
        // (el comportamiento seguro y habitual al traer archivos de afuera).
        let req = crate::ops_actions::transfer(false, paths, dest.clone());
        let label = format!("{} → {}", self.i18n.t("op.copy"), dest.display());
        self.launch_transfer(req, label);
    }

    /// Re-lista un panel sin tocar su historial (refrescar).
    pub fn refresh_pane(&mut self, id: PaneId, dir: PathBuf) {
        if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.entries.clear();
            f.focused = None;
            // La selección son posiciones de vista; al re-listar la vista se reconstruye →
            // limpiar (paridad con navigate_to/enter; evita operar sobre el archivo
            // equivocado mientras el nuevo listado llega en streaming).
            f.clear_selection();
        }
        // Carpetas que tenían tamaño calculado → recalcular tras re-listar.
        let to_recompute: Vec<PathBuf> = self
            .sized_paths
            .get(&id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        self.sized_paths.remove(&id);
        self.start_listing(id, dir);
        let recursive = !self.settings.size_no_subdirs;
        for p in to_recompute {
            if p.is_dir() {
                self.size_partial.remove(&p);
                let token = CancellationToken::new();
                let rx = naygo_core::sizing::spawn_dir_size(p.clone(), recursive, token.clone());
                self.size_jobs.insert((id, p), SizeJob { rx, token });
            }
        }
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
            request: None,
        });
    }

    /// Lanza una operación: planifica, spawnea (o encola). Papelera = atómica vía
    /// platform (no pasa por el motor core). `label` se muestra en el panel.
    pub fn start_op(&mut self, req: OpRequest, label: String) {
        self.start_op_recorded(req, label, true);
    }

    /// Como `start_op`, con control de si la op REGISTRA su deshacer al completar.
    /// Las ops que YA SON un deshacer van con `false` (v1 sin redo, sin bucles).
    fn start_op_recorded(&mut self, req: OpRequest, label: String, record_undo: bool) {
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
            let request = record_undo.then(|| req.clone());
            self.active_ops.push(ActiveOp {
                rx: None,
                token,
                label,
                progress: None,
                summary: None,
                started: false,
                pending: Some((plan, req.kind, req.conflict)),
                journal_id: None, // se journalea al lanzarse en pump_ops
                request,
            });
        } else {
            let (_ctx, crx) = std::sync::mpsc::channel::<ConflictDecision>();
            let journal = self.make_journal(&req.kind, req.conflict, &plan);
            let (journal_id, writer) = match journal {
                Some((id, w)) => (Some(id), Some(w)),
                None => (None, None),
            };
            let request = record_undo.then(|| req.clone());
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
                request,
            });
        }
    }

    /// Drena canales de las ops y gestiona la cola (lanza la siguiente al liberarse).
    pub fn pump_ops(&mut self) {
        // Drenar mensajes de las ops corriendo. `just_finished` se marca cuando una
        // op pasa a tener summary este pump → refrescamos el panel activo después.
        let mut just_finished = false;
        // Ops terminadas este pump cuyo deshacer hay que registrar (fuera del préstamo).
        let mut finished_for_undo: Vec<(OpRequest, String, OpSummary)> = Vec::new();
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
                        // Registrar el deshacer (también de canceladas: lo parcial
                        // hecho es igualmente reversible — son los Done del summary).
                        if let Some(req) = op.request.take() {
                            finished_for_undo.push((req, op.label.clone(), s.clone()));
                        }
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
        // Registrar los deshacer de las ops recién terminadas (historial acotado).
        for (req, label, s) in finished_for_undo {
            if let Some(actions) = naygo_core::ops::undo::build_undo(&req, &s) {
                let id = self.next_undo_id;
                self.next_undo_id += 1;
                let when_epoch_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                self.undo_history.push(naygo_core::ops::undo::UndoEntry {
                    id,
                    label,
                    when_epoch_secs,
                    actions,
                    undone: false,
                });
                if self.undo_history.len() > 100 {
                    self.undo_history.remove(0);
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

    /// Con `HighlightDuration::UntilInteract`, limpia el resaltado del panel activo
    /// porque el usuario acaba de interactuar con él (navegar, enfocar, activar).
    /// No-op en los demás modos. Centraliza la regla para no repetirla por sitio.
    fn clear_highlight_on_interact(&mut self) {
        if self.settings.highlight_duration == naygo_core::config::HighlightDuration::UntilInteract
        {
            if let Some(id) = self.workspace.active_id() {
                if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                    f.clear_highlight();
                }
                self.highlight_since.remove(&id);
            }
        }
    }

    /// Aplica una acción al panel activo.
    pub fn apply_action(&mut self, action: Action) {
        // Las acciones de navegación/foco cuentan como interacción del usuario: con el
        // modo UntilInteract, eso retira el resaltado de los archivos recién aparecidos.
        if matches!(
            action,
            Action::MoveUp
                | Action::MoveDown
                | Action::Activate
                | Action::GoUp
                | Action::GoBack
                | Action::GoForward
        ) {
            self.clear_highlight_on_interact();
        }
        match action {
            Action::MoveUp => self.move_focus(-1),
            Action::MoveDown => self.move_focus(1),
            Action::Activate => self.activate_focused(),
            Action::Open => {
                // "Abrir" del menú = misma semántica que doble-clic/Enter: una carpeta
                // navega, un archivo se abre con su app. `activate_focused` ya distingue.
                self.activate_focused();
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
                for job in self.size_jobs.values() {
                    job.token.cancel();
                }
            }
            Action::SwitchPane => self.cycle_active_files(),
            Action::Copy => self.clipboard_set(false),
            Action::Cut => self.clipboard_set(true),
            Action::Paste => self.paste(),
            Action::Delete => self.delete_selection(false),
            Action::DeletePermanent => self.delete_selection(true),
            Action::Rename => self.begin_rename(),
            Action::Undo => self.undo_last(),
            Action::NewFile => self.begin_create(false),
            Action::NewDir => self.begin_create(true),
            Action::CopyToOther => self.transfer_to_other(false),
            Action::MoveToOther => self.transfer_to_other(true),
            Action::ComputeSize => self.compute_size(),
            Action::ExtendUp => {
                if let Some(f) = self.workspace.active_files_mut() {
                    f.move_focus_extend(-1, true);
                }
            }
            Action::ExtendDown => {
                if let Some(f) = self.workspace.active_files_mut() {
                    f.move_focus_extend(1, true);
                }
            }
            Action::SelectAll => {
                if let Some(f) = self.workspace.active_files_mut() {
                    f.select_all();
                }
            }
            Action::ToggleSelect => {
                if let Some(f) = self.workspace.active_files_mut() {
                    if let Some(pos) = f.focused {
                        f.select_toggle(pos);
                    }
                }
            }
        }
    }

    /// Lanza el cálculo de tamaño de las carpetas seleccionadas (o la enfocada si es dir).
    fn compute_size(&mut self) {
        let Some(pane) = self.workspace.active_id() else {
            return;
        };
        let dirs: Vec<std::path::PathBuf> = {
            let from_sel: Vec<std::path::PathBuf> = self
                .selected_paths()
                .into_iter()
                .filter(|p| p.is_dir())
                .collect();
            if !from_sel.is_empty() {
                from_sel
            } else if let Some(e) = self
                .workspace
                .active_files()
                .and_then(|f| f.focused_view_entry())
            {
                if e.is_dir() {
                    vec![e.path.clone()]
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        };
        let recursive = !self.settings.size_no_subdirs;
        for dir in dirs {
            if let Some(old) = self.size_jobs.remove(&(pane, dir.clone())) {
                old.token.cancel();
            }
            let token = CancellationToken::new();
            let rx = naygo_core::sizing::spawn_dir_size(dir.clone(), recursive, token.clone());
            self.size_jobs.insert((pane, dir), SizeJob { rx, token });
        }
    }

    /// Drena los cálculos de tamaño y escribe el resultado en el Entry de cada carpeta.
    fn pump_sizing(&mut self) {
        if self.size_jobs.is_empty() {
            return;
        }
        let mut updates: Vec<(PaneId, std::path::PathBuf, u64, Option<bool>)> = Vec::new();
        let mut finished: Vec<(PaneId, std::path::PathBuf)> = Vec::new();
        for ((pane, dir), job) in &self.size_jobs {
            while let Ok(msg) = job.rx.try_recv() {
                match msg {
                    naygo_core::sizing::SizeMsg::Progress { bytes } => {
                        updates.push((*pane, dir.clone(), bytes, None));
                    }
                    naygo_core::sizing::SizeMsg::Done { total, partial } => {
                        updates.push((*pane, dir.clone(), total, Some(partial)));
                        finished.push((*pane, dir.clone()));
                    }
                    naygo_core::sizing::SizeMsg::Cancelled { bytes } => {
                        updates.push((*pane, dir.clone(), bytes, Some(false)));
                        finished.push((*pane, dir.clone()));
                    }
                }
            }
        }
        for (pane, dir, bytes, fin) in updates {
            if let Some(f) = self.workspace.pane_mut(pane).and_then(|p| p.files.as_mut()) {
                if let Some(e) = f.entries.iter_mut().find(|e| e.path == dir) {
                    e.size = Some(bytes);
                }
            }
            if let Some(partial) = fin {
                if partial {
                    self.size_partial.insert(dir.clone());
                }
                self.sized_paths
                    .entry(pane)
                    .or_default()
                    .insert(dir.clone());
            }
        }
        for key in finished {
            self.size_jobs.remove(&key);
        }
    }

    /// Rutas seleccionadas en el panel activo (mapeadas vista→entries). Si no hay
    /// selección, usa la entry enfocada. Vacío si no hay nada. Las rutas son las
    /// de las entries reales (no la fila virtual "..").
    /// Muestra el menú contextual NATIVO de Windows para la selección del panel activo
    /// en las coords de PANTALLA (sx, sy). Tras invocar un comando, re-lista el panel
    /// (consistencia inmediata; un comando puede crear/renombrar/borrar). Tolerante:
    /// sin HWND o sin selección no hace nada; un fallo se reporta discreto en el status.
    fn show_native_menu(&mut self, sx: i32, sy: i32) {
        let Some(hwnd) = self.hwnd else {
            return;
        };
        let paths = self.selected_paths();
        if paths.is_empty() {
            return;
        }
        match naygo_platform::context_menu::show_native_context_menu(hwnd, &paths, sx, sy) {
            Ok(naygo_platform::context_menu::NativeMenuOutcome::Invoked) => {
                if let (Some(id), Some(dir)) = (self.workspace.active_id(), self.active_dir()) {
                    self.refresh_pane(id, dir);
                }
            }
            Ok(naygo_platform::context_menu::NativeMenuOutcome::Cancelled) => {}
            Err(e) => {
                tracing::warn!("menú nativo falló: {e:?}");
                self.status = self.i18n.t("op.more_windows_error").to_string();
            }
        }
    }

    /// Inicia un arrastre OLE de `paths` hacia el SO (Explorer, escritorio, correo…).
    /// BLOQUEANTE: `DoDragDrop` corre su propio bucle modal hasta que el usuario suelta o
    /// cancela. Debe llamarse FUERA del closure de render de egui. Si el resultado es MOVER,
    /// refresca el panel activo (el archivo ya no está en el origen). Tolerante: cualquier
    /// fallo se reporta discreto en logs, nunca tumba la app.
    fn start_os_drag(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        match naygo_platform::dnd::start_drag(&paths) {
            Ok(naygo_platform::dnd::DragOutcome::Moved) => {
                // El destino movió los archivos: el origen quedó obsoleto. Refrescar.
                if let (Some(id), Some(dir)) = (self.workspace.active_id(), self.active_dir()) {
                    self.refresh_pane(id, dir);
                }
            }
            Ok(naygo_platform::dnd::DragOutcome::Copied)
            | Ok(naygo_platform::dnd::DragOutcome::Cancelled) => {
                // Copia: el origen no cambia. Cancelado: nada que hacer.
            }
            Err(e) => {
                tracing::warn!("arrastre OLE al SO falló: {e:?}");
            }
        }
    }

    fn selected_paths(&self) -> Vec<PathBuf> {
        let Some(f) = self.workspace.active_files() else {
            return Vec::new();
        };
        let view = f.view_indices();
        // Multi-selección: `f.selected` son posiciones de VISTA pobladas por la selección
        // (clic/Ctrl/Shift/rectángulo/teclado). Se mapean pos vista→view_indices→entries;
        // las posiciones fuera de rango se descartan (filter_map), nunca indexan mal.
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

    /// Lanza un worker que lee el espacio de cada unidad y lo emite por canal. No
    /// solapa escaneos: si ya hay uno en curso, no hace nada.
    fn start_disk_scan(&mut self) {
        // Refresca el strip del toolbar aunque haya un escaneo en curso (un USB
        // recién conectado debe aparecer; drives() es barato, no lee espacio).
        self.drives_cache = naygo_platform::drives::drives();
        if self.disk_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for d in naygo_platform::drives::drives() {
                if let Some((total, free)) = naygo_platform::drive_space::read_space(&d.path) {
                    let _ = tx.send((d.path.clone(), naygo_core::disk::DiskUsage { total, free }));
                }
            }
        });
        self.disk_rx = Some(rx);
    }

    /// Navega el panel activo a `path` (misma ruta que usa el árbol al navegar).
    pub(crate) fn navigate_active_to(&mut self, path: std::path::PathBuf) {
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

    /// Drena el worker de espacio (por frame) y re-escanea cada ~180 frames (~3s).
    fn pump_disk_usage(&mut self) {
        let mut done = false;
        if let Some(rx) = &self.disk_rx {
            loop {
                match rx.try_recv() {
                    Ok((root, usage)) => {
                        self.disk_usage.insert(root, usage);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                }
            }
        }
        if done {
            self.disk_rx = None;
        }
        self.disk_scan_ticks = self.disk_scan_ticks.wrapping_add(1);
        if self.disk_rx.is_none() && self.disk_scan_ticks >= 180 {
            self.disk_scan_ticks = 0;
            self.start_disk_scan();
        }
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
    /// Deshace la entrada más NUEVA no-deshecha del historial (Ctrl+Z). Valida el
    /// inverso primero; si ya no aplica (rutas movidas/ocupadas), avisa en el status
    /// y NO toca nada. El deshacer corre como ops normales sin registrarse a sí mismo.
    pub(crate) fn undo_last(&mut self) {
        let Some(idx) = self.undo_history.iter().rposition(|e| !e.undone) else {
            self.status = self.i18n.t("undo.nothing").to_string();
            return;
        };
        self.undo_at(idx);
    }

    /// Deshace la entrada con ese id del historial (clic en el panel Historial).
    fn undo_by_id(&mut self, id: u64) {
        if let Some(idx) = self.undo_history.iter().position(|e| e.id == id) {
            self.undo_at(idx);
        }
    }

    /// Valida y lanza el inverso de `undo_history[idx]`; marca `undone` y avisa en el
    /// status. El deshacer corre como ops normales SIN registrarse a sí mismo.
    fn undo_at(&mut self, idx: usize) {
        if self.undo_history[idx].undone {
            return;
        }
        match naygo_core::ops::undo::validate(&self.undo_history[idx].actions) {
            Err(e) => {
                self.status = self.i18n.t("undo.invalid").replace("{e}", &e);
            }
            Ok(()) => {
                let original = self.undo_history[idx].label.clone();
                let reqs = naygo_core::ops::undo::to_requests(&self.undo_history[idx].actions);
                self.undo_history[idx].undone = true;
                let label = self.i18n.t("undo.label").replace("{label}", &original);
                for req in reqs {
                    self.start_op_recorded(req, label.clone(), false);
                }
                self.status = label;
            }
        }
    }

    /// Agrega un panel genérico (Historial/Árbol/Propiedades) al leaf enfocado.
    pub fn add_pane_of(&mut self, purpose: PanePurpose) {
        let id = self.workspace.add_pane(purpose, PathBuf::new());
        self.dock_state.main_surface_mut().push_to_focused_leaf(id);
    }

    /// F2 / Renombrar: con UN ítem abre el rename INLINE (R1, la celda Nombre se
    /// vuelve un TextEdit con el nombre pre-seleccionado); con 2+ seleccionados abre
    /// el diálogo de BATCH-RENAME (R3, preview en vivo + comodines).
    fn begin_rename(&mut self) {
        if self
            .workspace
            .active_files()
            .map(|f| f.selection_count() >= 2)
            .unwrap_or(false)
        {
            self.begin_batch_rename();
            return;
        }
        let Some(pane) = self.workspace.active_id() else {
            return;
        };
        let info = self.workspace.active_files().and_then(|f| {
            let e = f.focused_view_entry()?;
            Some((e.path.clone(), e.name.clone()))
        });
        if let Some((path, name)) = info {
            tracing::debug!(?path, "rename inline: begin");
            self.inline_rename = Some(InlineRename {
                pane,
                path,
                text: name,
                stage: 0,
                focus_pending: true,
                select_pending: true,
                missing_frames: 0,
            });
        }
    }

    /// Abre el diálogo de batch-rename con la selección actual (en orden de vista).
    fn begin_batch_rename(&mut self) {
        let Some(f) = self.workspace.active_files() else {
            return;
        };
        let view = f.view_indices();
        // Posiciones de vista seleccionadas, ORDENADAS como se ven en pantalla (el
        // contador {n} numera en ese orden, lo predecible para el usuario).
        let mut positions: Vec<usize> = f.selected.clone();
        positions.sort_unstable();
        let epoch = |t: Option<std::time::SystemTime>| {
            t.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
        };
        let items: Vec<naygo_core::batch_rename::BatchItem> = positions
            .iter()
            .filter_map(|&pos| view.get(pos))
            .filter_map(|&real| f.entries.get(real))
            .map(|e| naygo_core::batch_rename::BatchItem {
                path: e.path.clone(),
                modified_epoch_secs: epoch(e.modified),
            })
            .collect();
        if items.len() < 2 {
            return;
        }
        let existing_names: Vec<String> = f.entries.iter().map(|e| e.name.clone()).collect();
        self.batch_rename = Some(crate::batch_rename_dialog::BatchRenameState::new(
            items,
            existing_names,
            naygo_platform::time::local_utc_offset_secs(),
        ));
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
            // Sin Shift, mover el foco hace selección simple del nuevo foco (descarta
            // cualquier multi-selección previa).
            f.move_focus_extend(delta, false);
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
        // El editor de atajos, mientras captura una combinación, también se queda
        // con el teclado: no queremos que pulsar el atajo a reasignar dispare su acción.
        if self.pending_dialog.is_some()
            || !self.pending_resume.is_empty()
            || self.shortcut_capture.is_some()
            || self.inline_rename.is_some()
            || self.batch_rename.is_some()
        {
            return;
        }
        // Teclas que la app entiende (espejo de `egui_key_to_code`). Por cada una
        // que se PRESIONE este frame (edge), armamos el `Chord` con los modificadores
        // actuales y dejamos que el keymap resuelva la acción.
        const KEYS: &[egui::Key] = &[
            egui::Key::ArrowUp,
            egui::Key::ArrowDown,
            egui::Key::ArrowLeft,
            egui::Key::ArrowRight,
            egui::Key::Enter,
            egui::Key::Backspace,
            egui::Key::Tab,
            egui::Key::Escape,
            egui::Key::Delete,
            egui::Key::Space,
            egui::Key::F2,
            egui::Key::F3,
            egui::Key::F5,
            egui::Key::F6,
            egui::Key::A,
            egui::Key::B,
            egui::Key::C,
            egui::Key::D,
            egui::Key::E,
            egui::Key::F,
            egui::Key::G,
            egui::Key::H,
            egui::Key::I,
            egui::Key::J,
            egui::Key::K,
            egui::Key::L,
            egui::Key::M,
            egui::Key::N,
            egui::Key::O,
            egui::Key::P,
            egui::Key::Q,
            egui::Key::R,
            egui::Key::S,
            egui::Key::T,
            egui::Key::U,
            egui::Key::V,
            egui::Key::W,
            egui::Key::X,
            egui::Key::Y,
            egui::Key::Z,
        ];
        let mut actions = Vec::new();
        let mut typed = String::new();
        ctx.input(|i| {
            let ctrl = i.modifiers.ctrl;
            let shift = i.modifiers.shift;
            let alt = i.modifiers.alt;
            for &k in KEYS {
                if i.key_pressed(k) {
                    if let Some(code) = crate::input::egui_key_to_code(k) {
                        let chord = naygo_core::keymap::Chord {
                            key: code,
                            ctrl,
                            shift,
                            alt,
                        };
                        if let Some(action) = self.keymap.action_for(&chord) {
                            actions.push(action);
                        }
                    }
                }
            }
            // Botones laterales del mouse (fijos, fuera del keymap).
            if i.pointer.button_pressed(egui::PointerButton::Extra1) {
                actions.push(map_mouse_extra(MouseExtra::Back));
            }
            if i.pointer.button_pressed(egui::PointerButton::Extra2) {
                actions.push(map_mouse_extra(MouseExtra::Forward));
            }
            // Typeahead: texto escrito que no fue un atajo.
            for event in &i.events {
                if let egui::Event::Text(t) = event {
                    typed.push_str(t);
                }
            }
        });

        let fired_action = !actions.is_empty();
        if fired_action {
            self.typeahead_buf.clear();
        }
        for a in actions {
            self.apply_action(a);
        }
        // Si este frame resolvió una acción (incluida una letra simple rebindeada a una
        // acción), NO hacemos typeahead con ese texto: una tecla no debe disparar acción
        // y salto-por-tipeo a la vez.
        if !fired_action && !typed.is_empty() {
            self.typeahead(&typed);
        }
    }

    /// Guarda el workspace persistible.
    /// Restaura los valores de fábrica: settings por defecto (idioma re-detectado del
    /// SO, como un primer arranque), workspace Dual-pane por defecto en el home, y
    /// persiste ambos de inmediato. NO toca plantillas guardadas, keymap ni logs.
    fn apply_factory_reset(&mut self) {
        let locale = naygo_platform::locale::os_locale().unwrap_or_default();
        let s = Settings {
            language: pick_default_language(&locale, self.i18n.available()),
            ..Settings::default()
        };
        self.i18n.set_language(&s.language);
        self.settings = s;
        // El cambio de tema/íconos lo aplican los watchers existentes de `ui()` al
        // detectar la diferencia con el estado activo.
        let home = default_start_dir();
        self.workspace = Workspace::from_template(&LayoutTemplate::dual_pane(), &home);
        self.dock_state = crate::dock_translate::to_dock_state(&self.workspace.layout);
        self.start_all_listings();
        self.save_workspace();
        config::save_settings(&self.config_dir, &self.settings);
        self.last_saved_settings = self.settings.clone();
        self.status = self.i18n.t("status.factory_done").to_string();
        tracing::info!("valores de fábrica restaurados");
    }

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

    /// Resumen de selección múltiple en la barra de estado (N + tamaño conocido sumado).
    /// Las carpetas sin tamaño calculado NO suman (no se dispara cálculo). Solo aplica con
    /// 2+ seleccionados; con 0/1 el status lo maneja el flujo normal.
    fn update_selection_status(&mut self) {
        let Some(f) = self.workspace.active_files() else {
            return;
        };
        let count = f.selection_count();
        if count < 2 {
            return;
        }
        let view = f.view_indices();
        let total: u64 = f
            .selected
            .iter()
            .filter_map(|&pos| view.get(pos))
            .filter_map(|&real| f.entries.get(real))
            .filter_map(|e| e.size)
            .sum();
        let suffix = self.i18n.t("status.selected_suffix").to_string();
        self.status = format!(
            "{count} {suffix} · {}",
            naygo_core::format::human_size(total)
        );
    }
}

impl eframe::App for NaygoApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Mientras el splash está activo no hacemos trabajo pesado ni procesamos input:
        // solo pedimos repaint (el pintado y la decisión de cerrarlo van en `ui`, que sí
        // tiene un `Ui` donde pintar). No se puede pintar en `logic` (regla de eframe).
        // En debug `splash` es None: no aplica.
        if self.splash.is_some() {
            ctx.request_repaint();
            return;
        }
        self.pump_all();
        self.pump_tree();
        self.pump_ops();
        self.pump_disk_usage();
        self.pump_sizing();
        self.pump_watchers();
        self.pump_devices();
        self.pump_dropped_files(ctx);
        self.pump_paste_write();
        // La captura de atajos consume el teclado de este frame. Si capturó o canceló una
        // tecla, NO corremos handle_input este frame: si no, la misma tecla (que sigue
        // `key_pressed` todo el frame) dispararía además su acción recién asignada, y Esc
        // de cancelación también gatillaría CancelListing.
        let capture_consumed = self.process_shortcut_capture(ctx);
        if self.shortcut_capture.is_some() {
            ctx.request_repaint();
        }
        if !capture_consumed {
            self.handle_input(ctx);
        }
        // Resumen de selección múltiple en la barra de estado. Se recomputa cada frame
        // desde la selección actual del panel activo (se actualiza solo al cambiar la
        // selección, sin importar dónde ocurrió el clic). Solo pisa el status cuando hay
        // 2+ seleccionados y no hay ninguna op en curso: así no clobberea el progreso/error
        // efímero de una operación (que debe permanecer visible mientras corre).
        if !self.any_op_active() {
            self.update_selection_status();
        }
        if self.any_listing_active()
            || self.any_tree_listing_active()
            || self.any_op_active()
            || self.pending_paste_write.is_some()
            || self.disk_rx.is_some()
            || !self.size_jobs.is_empty()
        {
            ctx.request_repaint();
        }
        // Los eventos de carpeta llegan por canal sin input del usuario: despertamos la
        // UI ~2 veces/seg para drenarlos. Barato y respeta el bajo consumo (no es un
        // bucle ocupado: si no hay cambios, los pumps no hacen trabajo).
        if !self.watchers.is_empty() || self.device_watch.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        // Fluidez del hover SIN quemar recursos: repintamos solo cuando el puntero se está
        // MOVIENDO sobre la UI (para que la fila bajo el mouse se actualice al instante),
        // no en cada frame. Con el mouse quieto o fuera de la ventana NO se repinta → la
        // app queda idle (bajo consumo, la prioridad del proyecto). egui ya repinta ante
        // clics/teclas por su cuenta, así que la respuesta al clic no depende de esto.
        let pointer_moving =
            ctx.input(|i| i.pointer.is_moving() && i.pointer.interact_pos().is_some());
        if pointer_moving {
            ctx.request_repaint();
        }

        // Espejar el setting "nuevos al final" en cada panel: una sola definición de
        // la vista (core) para render + foco + teclado.
        let group_new = self.settings.new_items_at_end;
        for p in self.workspace.panes_mut() {
            if let Some(f) = p.files.as_mut() {
                f.group_new_at_end = group_new;
            }
        }

        // ── Bandeja del sistema ──
        // Crear/destruir según el setting (cubre también el arranque). `create` corre
        // en el hilo del event loop (requisito de tray-icon). Si falla (p. ej. ícono
        // ilegible), no se reintenta cada frame: queda None y el setting intacto.
        if self.settings.tray_enabled && self.tray.is_none() && !self.tray_failed {
            let open = self.i18n.t("tray.open").to_string();
            let exit = self.i18n.t("tray.exit").to_string();
            self.tray = crate::tray::create(ctx, &open, &exit);
            self.tray_failed = self.tray.is_none();
        } else if !self.settings.tray_enabled && self.tray.is_some() {
            self.tray = None;
        }
        // Drenar los mensajes del tray (los handlers ya despertaron la UI).
        let (mut tray_open, mut tray_exit) = (false, false);
        if let Some(t) = &self.tray {
            while let Ok(msg) = t.rx.try_recv() {
                match msg {
                    crate::tray::TrayMsg::Open => tray_open = true,
                    crate::tray::TrayMsg::Exit => tray_exit = true,
                }
            }
        }
        if tray_open {
            self.hidden_to_tray = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if tray_exit {
            self.quit_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Mientras la ventana está oculta en la bandeja, un latido lento mantiene el
        // drenado de eventos vivo (una ventana oculta puede no recibir redraws; el
        // request_repaint del handler podría no bastar). 3 Hz: costo ínfimo.
        if self.hidden_to_tray {
            ctx.request_repaint_after(std::time::Duration::from_millis(330));
        }

        // ── Persistencia real (eframe sin la feature `persistence` jamás llama a
        // `App::save`, así que el guardado lo dirigimos nosotros) ──
        // Restaurar valores de fábrica (pedido desde Configuración → Avanzado).
        if self.factory_reset_requested {
            self.factory_reset_requested = false;
            self.apply_factory_reset();
        }
        // Settings: guardar de inmediato cuando cambian (struct chica, comparación
        // barata; escribe solo ante un cambio real).
        if self.settings != self.last_saved_settings {
            config::save_settings(&self.config_dir, &self.settings);
            self.last_saved_settings = self.settings.clone();
        }
        // Workspace: autosave periódico (tolera cierres abruptos) + guardado al cerrar.
        const WORKSPACE_AUTOSAVE: std::time::Duration = std::time::Duration::from_secs(60);
        if self.last_workspace_autosave.elapsed() >= WORKSPACE_AUTOSAVE {
            self.save_workspace();
            self.last_workspace_autosave = std::time::Instant::now();
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_workspace();
            config::save_settings(&self.config_dir, &self.settings);
            // Cerrar → ocultar a la bandeja (opt-in). "Salir" del tray fuerza el
            // cierre real (quit_requested).
            if self.settings.close_to_tray && self.tray.is_some() && !self.quit_requested {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.hidden_to_tray = true;
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Splash de arranque (solo release): se pinta a pantalla completa DENTRO de la
        // ventana principal y NO dibujamos la UI real hasta que termina. `show` devuelve
        // `false` al expirar el tiempo o ante el primer input → ahí lo soltamos y el
        // siguiente frame pinta la UI normal.
        if let Some(splash) = &self.splash {
            let keep = splash.show(ui);
            if !keep {
                self.splash = None;
                tracing::info!("splash cerrado; primer frame real de la UI");
                // El primer frame real tras el splash debe renderizar y atender input ya
                // (sin esperar un evento); si no, los primeros clics no toman.
                ui.ctx().request_repaint();
            }
            return;
        }

        if self.icons.set() != self.settings.icon_set {
            let set = self.settings.icon_set.clone();
            self.icons.reload(ui.ctx(), &set, &self.config_dir);
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
        // Petición de menú nativo (shell-B): se declara UNA vez; cada pane que pinte su
        // menú contextual escribe aquí. Solo hay un menú abierto a la vez → last-writer-wins.
        let mut native_menu_request: Option<(f32, f32)> = None;
        // Deshacer pedidos desde el panel Historial este frame (diferidos como pending).
        let mut undo_clicks: Vec<u64> = Vec::new();
        {
            let now_epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let mut viewer = crate::docking::NaygoTabViewer {
                workspace: &mut self.workspace,
                inline_rename: &mut self.inline_rename,
                undo_history: &self.undo_history,
                undo_clicks: &mut undo_clicks,
                now_epoch,
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
                native_menu_request: &mut native_menu_request,
                disk_usage: &self.disk_usage,
                new_items_at_end: self.settings.new_items_at_end,
                size_partial: &self.size_partial,
            };
            let mut dock_style = egui_dock::Style::from_egui(ui.style().as_ref());
            // Ancho mínimo de cada tab del dock para que no se compriman hasta
            // volverse ilegibles cuando hay varios paneles. Solo limita el tab,
            // no la división arrastrable; el mínimo de la ventana (Task 2) cubre
            // ese caso.
            dock_style.tab.minimum_width = Some(150.0);
            egui_dock::DockArea::new(&mut self.dock_state)
                .style(dock_style)
                .show_inside(ui, &mut viewer);
        }
        // Tras pintar el dock: si el usuario cerró un tab, su PaneId ya no está en el
        // DockState. Podamos ese panel de TODOS los mapas por-panel (workspace + workers
        // + watchers + estado) para no filtrar handles del SO ni dejar al watcher
        // vigilando una carpeta de un panel cerrado (consumo). Sin esto, el bucle de
        // repintado de 500 ms nunca se apagaría tras cerrar un panel.
        self.prune_closed_panes();
        // Petición de arrastre OLE hacia el SO (Naygo → Explorer). Se acumula aquí y se
        // procesa DESPUÉS del bucle `pending`, fuera del closure de egui: `DoDragDrop` corre
        // un bucle modal que toma el control del mouse. Mismo patrón que `native_menu`.
        let mut os_drag_request: Option<Vec<PathBuf>> = None;
        for req in pending {
            match req {
                crate::docking::PaneRequest::Activate { id } => {
                    self.workspace.set_active(id);
                    // Un clic en una fila activa el panel: es interacción del usuario, así
                    // que con UntilInteract retira el resaltado del panel ya activo.
                    self.clear_highlight_on_interact();
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
                crate::docking::PaneRequest::StartOsDrag { paths } => {
                    // Solo un arrastre a la vez tiene sentido; last-writer-wins.
                    os_drag_request = Some(paths);
                }
                crate::docking::PaneRequest::CommitRename { source, new_name } => {
                    // Mismo camino que tenía el diálogo de renombrar: rename de un
                    // paso con Overwrite (el motor maneja el choque).
                    let mut req = crate::ops_actions::rename(source, new_name);
                    req.conflict = ConflictPolicy::Overwrite;
                    let label = self.i18n.t("op.rename").to_string();
                    self.start_op(req, label);
                }
            }
        }

        // Deshacer pedidos desde el panel Historial (en el orden emitido).
        for uid in undo_clicks {
            self.undo_by_id(uid);
        }

        // Arrastre OLE hacia el SO: FUERA del closure de egui (DoDragDrop bloquea con su
        // bucle modal mientras el usuario arrastra). Tras soltar, si el efecto fue MOVER,
        // refrescamos el panel activo (el dato ya no está en el origen). El arrastre interno
        // entre paneles de Naygo (Task 2) sigue intacto: si el usuario suelta DENTRO de la
        // ventana, nuestro IDropTarget lo recibe; si suelta fuera, lo recibe el SO.
        if let Some(paths) = os_drag_request.take() {
            self.start_os_drag(paths);
        }

        // Disparadores de operaciones del menú contextual. Se aplican DESPUÉS del
        // `pending` (que ya enfocó/activó la fila del clic derecho), así las acciones
        // basadas en foco actúan sobre la entry correcta.
        for action in ops_actions {
            self.apply_action(action);
        }

        // Petición de menú contextual nativo (shell-B): se almacena para que NaygoApp la
        // procese fuera del closure de egui (la consume Task 5).
        if native_menu_request.is_some() {
            self.native_menu_request = native_menu_request;
        }

        // Menú contextual nativo de Windows (shell-B): se procesa FUERA del closure de
        // egui (COM no debe correr dentro del render del menú). TrackPopupMenuEx bloquea
        // el hilo de UI mientras el menú modal está abierto — interacción explícita del
        // usuario, no I/O de fondo, así que es aceptable.
        if let Some((sx, sy)) = self.native_menu_request.take() {
            self.show_native_menu(sx as i32, sy as i32);
        }

        // Acciones del árbol acumuladas durante el pintado.
        for (id, action) in tree_actions {
            match action {
                crate::tree_actions::TreeAction::Expand(path) => self.tree_expand(id, path),
                crate::tree_actions::TreeAction::Collapse(path) => self.tree_collapse(id, path),
                crate::tree_actions::TreeAction::Navigate(path) => {
                    self.navigate_active_to(path);
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
                        // El orden cambió: las posiciones de vista guardadas ya no apuntan a
                        // las mismas filas. Limpiar selección/ancla para no seleccionar al azar.
                        f.clear_selection();
                    }
                    crate::table_actions::TableAction::SetFilter(kind, filter) => {
                        f.table.set_filter(kind, filter);
                        // El filtro cambió qué filas son visibles → posiciones de vista stale.
                        f.clear_selection();
                    }
                    crate::table_actions::TableAction::ClearFilter(kind) => {
                        f.table.clear_filter(kind);
                        // Idem: cambia la vista → posiciones de vista stale.
                        f.clear_selection();
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

        // Diálogo de batch-rename (R3): editar campos recalcula el preview en vivo;
        // Aplicar lanza UNA op BatchRename (journaled → deshacible del Historial).
        if let Some(mut state) = self.batch_rename.take() {
            let ctx = ui.ctx().clone();
            match crate::batch_rename_dialog::show(&ctx, &self.i18n, &self.active_theme, &mut state)
            {
                crate::batch_rename_dialog::BatchDialogResult::Open => {
                    self.batch_rename = Some(state);
                    ui.ctx().request_repaint();
                }
                crate::batch_rename_dialog::BatchDialogResult::Cancelled => {}
                crate::batch_rename_dialog::BatchDialogResult::Apply(req) => {
                    let label = self.i18n.t("op.batch_rename").to_string();
                    self.start_op(req, label);
                }
            }
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
        // Capturar el layout VIVO del dock antes de persistir: así sobreviven al
        // reinicio los paneles añadidos con ➕ y los reacomodos por arrastre, que
        // mutan `dock_state` pero no `workspace.layout`.
        self.workspace.layout = crate::dock_translate::from_dock_state(&self.dock_state);
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

/// Carga el workspace persistido y lo reconstruye, o cae al Dual-pane default. El
/// bool informa si hubo RECUPERACIÓN (workspace.json corrupto respaldado como .bad).
fn load_or_default_workspace(dir: &Path, home: &Path) -> (Workspace, bool) {
    let (persist, recovered) = config::load_workspace_flagged(dir);
    if let Some(persist) = persist {
        if let Some(w) = rebuild_workspace(persist, home) {
            return (w, recovered);
        }
    }
    (
        Workspace::from_template(&LayoutTemplate::dual_pane(), home),
        recovered,
    )
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
