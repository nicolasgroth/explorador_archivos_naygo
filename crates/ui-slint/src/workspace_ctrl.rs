// Naygo — controlador multi-panel de la UI Slint (Fase 2a). Posee el Workspace (varios
// FilePaneState + layout) y traduce gestos a llamadas del core.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::bridge::{
    favorite_rows, history_rows, inspector_info, recent_rows, rows_from_view, tree_rows, HistRow,
    InspectorInfo, NavRow, PlainRow, TreeRow,
};
use crate::listing::Listing;
use naygo_core::favorites::Favorites;
use naygo_core::fs_model::{EntryKind, SortKey};
use naygo_core::keymap::Action;
use naygo_core::recent_dirs::RecentDirs;
use naygo_core::tree::DirTree;
use naygo_core::workspace::layout::{
    Rect, SerializableDockLayout, SplitDir, SplitHandle, SplitStep,
};
use naygo_core::workspace::{FilePaneState, PaneId, PanePurpose, Workspace};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const PAGE_ROWS: usize = 20;

/// Destino resuelto de una acción «hacia otro panel».
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaneTarget {
    /// Hay exactamente un candidato: se actúa directo.
    Direct(PaneId),
    /// Hay varios: la UI muestra el selector 1..9 (orden visual).
    Pick(Vec<PaneId>),
    /// No hay otro panel Files: primero hay que dividir el actual.
    NeedsSplit,
}

/// Resultado de un intento de expulsión segura de una unidad extraíble. La UI lo
/// mapea a un toast localizado (éxito / en-uso / fallo). NUNCA se fuerza: `InUse`
/// significa que se abortó limpio sin desmontar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EjectOutcome {
    /// Desmontada y expulsada: ya se puede quitar con seguridad.
    Ok,
    /// Hay archivos abiertos (no se pudo bloquear el volumen). No se tocó nada.
    InUse,
    /// Otra falla (el mensaje describe el código).
    Failed(String),
}

/// Una acción multi-panel pendiente de elegir destino (cuando hay 3+ paneles).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaneAction {
    /// Abrir `dir` (subcarpeta) en el panel destino.
    OpenDir(PathBuf),
    /// Intercambiar el origen con el destino.
    Swap,
    /// Clonar la carpeta del origen en el destino.
    Clone,
    /// Apilar el origen como pestaña sobre el destino (agruparlos).
    Stack,
    /// Copiar (`move_files=false`) o mover (`true`) la selección del origen a la carpeta del
    /// destino (F5/F6 estilo Commander). La selección se lee al resolver el destino.
    Transfer { move_files: bool },
}

/// Estado del selector numérico de panel destino (overlay 1..9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanePick {
    pub action: PaneAction,
    pub origin: PaneId,
    /// Candidatos en orden visual; la posición 0 es el número "1".
    pub candidates: Vec<PaneId>,
}

pub struct WorkspaceCtrl {
    pub ws: Workspace,
    /// Configuración de la app (settings + i18n + temas + atajos), cargada del core y
    /// persistida en el directorio portable. El keymap vive aquí (`config.keymap`).
    pub config: crate::config_ctrl::ConfigCtrl,
    /// Un listado en curso por panel (la carpeta de cada panel se lista por separado).
    pub listings: HashMap<PaneId, Listing>,
    /// Un árbol de carpetas por panel Tree (lazy; se llena con workers solo-dirs).
    pub trees: HashMap<PaneId, DirTree>,
    /// Cursor de teclado por árbol (la fila enfocada para ↑↓←→). Si falta, se siembra con la
    /// carpeta activa del árbol o su primera raíz. Distinto de `active_path` (la carpeta navegada):
    /// el cursor se mueve sin navegar; Enter/→ sí navegan.
    pub tree_cursor: HashMap<PaneId, PathBuf>,
    /// Un worker solo-directorios por rama de árbol en vuelo (clave: panel + carpeta).
    pub tree_listings: HashMap<(PaneId, PathBuf), Listing>,
    /// Destino a "revelar" en cada árbol: al navegar el Files activo, el árbol expande
    /// progresivamente los ancestros hasta esta carpeta (reveal). Se limpia al llegar.
    pub reveal_targets: HashMap<PaneId, PathBuf>,
    /// Favoritos (global). Persistencia real diferida a la fase de config (F4); en 2b
    /// vive en memoria de sesión (el modelo core ya serializa).
    pub favorites: Favorites,
    /// Carpetas recientes (global): se empuja al navegar.
    pub recents: RecentDirs,
    /// Controlador de operaciones de archivo (F3): ops en curso, modales, deshacer,
    /// clipboard interno con corte visual. Posee el historial de deshacer.
    pub ops: crate::ops_ctrl::OpsCtrl,
    /// Estado del preview (debounce + worker + último resultado).
    pub preview: crate::preview::PreviewState,
    /// Selector de panel destino en curso (overlay 1..9), si lo hay.
    pub pending_pick: Option<PanePick>,
    /// Última área de contenido conocida (la setea la UI en cada layout) para resolver
    /// destinos por orden visual desde gestos que no traen el área (p. ej. teclado).
    pub last_area: Rect,
    /// Último clic (panel, posición de vista, instante) para detectar el doble-clic en Rust
    /// sin depender del `double-clicked` de Slint (que con el renderizador por software puede
    /// no dispararse si el rastreo de clics se reinicia). Ver `on_row_clicked`.
    pub last_click: Option<(PaneId, usize, std::time::Instant)>,
    /// Instante de la última APERTURA por doble-clic, para que los dos detectores (nativo de
    /// Slint y por tiempo en Rust) no naveguen dos veces sobre el mismo gesto. Quien dispara
    /// primero navega y estampa; el otro ve la estampa reciente y se abstiene.
    pub last_open: Option<std::time::Instant>,
    pub typeahead: String,
    /// Instante del último carácter de typeahead: si pasan >500ms entre teclas, el buffer se
    /// reinicia (escribir "in", pausa, "for" busca "for", no "infor"). Estilo Explorer.
    pub typeahead_at: Option<std::time::Instant>,
    pub ctrl_down: bool,
    pub shift_down: bool,
    /// Menú contextual abierto (clic derecho): posición (x,y en la ventana) y rutas objetivo.
    pub context_menu: Option<ContextMenuState>,
    /// Menú/editor de columna abierto (clic derecho en el header, F2): panel, columna y posición.
    pub column_menu: Option<ColumnMenuState>,
    /// Plantillas de disposición del usuario + recientes (templates.json). Los built-in viven en
    /// código (`LayoutTemplate::builtins`). Fase 4.
    pub templates: naygo_core::workspace::TemplateStore,
    /// Ventana de renombrado por lotes abierta (Fase 5): el spec en edición + los ítems objetivo.
    pub batch: Option<BatchRenameState>,
    /// Modal "nueva(s) carpeta(s)" abierto: carpeta destino + texto multilínea en edición.
    pub new_folder: Option<NewFolderState>,
    /// La ayuda (F1) está abierta.
    pub help_open: bool,
    /// Cálculo de tamaño de carpeta en curso (F3), si lo hay. Un solo job a la vez: un F3 nuevo
    /// cancela y reemplaza el anterior. El resultado se muestra en la barra de estado.
    pub size_job: Option<SizeJob>,
    /// Búsqueda recursiva en curso/terminada (Ctrl+F / lupa), si la hay. Mientras esté presente
    /// la UI muestra el panel de resultados; `None` = sin búsqueda. Ver `SearchJob`.
    pub search_job: Option<SearchJob>,
    /// Vista profunda (listado recursivo) en curso/terminada para un panel, si la hay. Un solo
    /// job a la vez. `None` = ningún panel en modo profundo. Ver `DeepJob`.
    pub deep_job: Option<DeepJob>,
    /// Huella de la última sesión persistida (paneles + carpetas + disposición). El bucle de
    /// UI compara contra `session_fingerprint()` en cada tick y, si cambió, guarda. Así la
    /// sesión en disco queda siempre al día sin depender del evento de cierre de ventana (que
    /// con el render por software de Slint puede no dispararse). Ver `maybe_persist_session`.
    pub last_saved_fingerprint: Option<String>,
    /// Watchers de carpeta por panel (Fase 5A): refrescan el panel sin re-listar y resaltan los
    /// archivos recién aparecidos. La UI los arranca al navegar (tiene el waker de Slint).
    pub watchers: crate::watch::Watchers,
    /// Último panel Files que estuvo activo. Cuando se activa un panel AUXILIAR (Árbol,
    /// Favoritos…) y desde ahí se navega, la navegación va a ESTE panel (el que el usuario venía
    /// usando), no al primer Files cualquiera. Ver `active_files_id`.
    pub last_active_files: Option<PaneId>,
    /// El atajo "editar ruta" (Ctrl+L / F4) pidió editar este panel. La UI lo lee con
    /// `take_edit_path_request` para abrir el editor de la path-bar.
    pub edit_path_requested: Option<PaneId>,
    /// El rename inline (F2 / menú) pidió editar la fila `pos` del panel, con la etapa de
    /// selección del ciclo F2. La UI lo lee con `take_rename_request` para abrir el editor en
    /// la celda Name. (pane, posición de vista, etapa). Ver 6D.
    pub rename_requested: Option<(PaneId, usize, u8)>,
    /// Cache de íconos (PNG → slint::Image, decodificado una vez por set+clave). Lo posee el
    /// controlador para resolver el ícono de cada fila al pintarla. Su set activo lo fija la
    /// configuración (Apariencia → Set de íconos). Ver `crate::icons::IconCache`.
    pub icons: crate::icons::IconCache,
}

/// Estado del menú contextual abierto.
#[derive(Clone, Debug)]
pub struct ContextMenuState {
    pub x: f32,
    pub y: f32,
    pub targets: Vec<PathBuf>,
    /// Modo CARPETA: el menú se abrió en la zona vacía del panel (no sobre un archivo). El
    /// objetivo es la carpeta del panel; el menú muestra Explorador/Nueva carpeta/Pegar en vez
    /// de las acciones de archivo.
    pub folder_mode: bool,
}

/// Modal "nueva(s) carpeta(s)": cada línea del `text` es una subcarpeta a crear dentro de `dir`;
/// `\` (o `/`) la vuelve anidada. La validación vive en core (`ops::parse_new_folders`).
pub struct NewFolderState {
    pub dir: PathBuf,
    pub text: String,
}

/// Mapea el entero de la UI a una terminal: 0=PowerShell, 1=CMD, 2=Windows Terminal, 3=WSL.
fn term_from_int(term_int: i32) -> naygo_platform::open::Terminal {
    use naygo_platform::open::Terminal;
    match term_int {
        1 => Terminal::Cmd,
        2 => Terminal::WindowsTerminal,
        3 => Terminal::Wsl,
        _ => Terminal::PowerShell,
    }
}

/// En qué fase está el menú de columna (clic derecho en el header, F2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnMenuMode {
    /// Lista de acciones (ordenar asc/desc, filtrar…, quitar filtro, ocultar).
    Menu,
    /// Editor de filtro abierto para esta columna (su tipo lo decide el `ColumnKind`).
    Filter,
}

/// Estado del menú/editor de columna abierto: a qué panel y columna pertenece, dónde se pinta
/// y en qué fase está. El editor de filtro lee/escribe los campos de borrador de abajo.
#[derive(Clone, Debug)]
pub struct ColumnMenuState {
    pub pane: PaneId,
    pub kind: naygo_core::columns::ColumnKind,
    pub x: f32,
    pub y: f32,
    pub mode: ColumnMenuMode,
    /// Borrador del filtro de TEXTO (Name): subcadena + sensible a mayúsculas.
    pub text_draft: String,
    pub text_case: bool,
    /// Borrador del filtro de RANGO (Size/fechas) como texto editable; se parsea al aplicar.
    pub min_draft: String,
    pub max_draft: String,
    /// Borrador del filtro de EXTENSIONES: extensiones marcadas ("" = sin extensión).
    pub ext_checked: std::collections::BTreeSet<String>,
}

/// Cálculo de tamaño de carpeta en curso (F3): qué carpeta, el canal del worker, el token para
/// cancelar, y el progreso/resultado acumulado.
pub struct SizeJob {
    pub target: PathBuf,
    pub rx: std::sync::mpsc::Receiver<naygo_core::sizing::SizeMsg>,
    pub token: naygo_core::CancellationToken,
    pub bytes: u64,
    pub done: bool,
    pub partial: bool,
    pub cancelled: bool,
}

/// Una coincidencia de búsqueda lista para la UI: el `Entry` y su ruta relativa a la raíz
/// de la búsqueda (para mostrar dónde está sin repetir la raíz en cada fila).
#[derive(Clone, Debug)]
pub struct SearchHit {
    pub entry: naygo_core::fs_model::Entry,
    /// Ruta de la CARPETA que contiene la coincidencia, relativa a la raíz de búsqueda
    /// (vacía si está en la raíz misma). Solo para mostrar; la acción usa `entry.path`.
    pub rel_dir: String,
}

/// Una fila del panel de resultados, ya formateada y con el ícono resuelto (lo arma el
/// controlador para que main.rs solo la copie al `SearchHitVm` de Slint).
pub struct SearchRow {
    pub name: String,
    pub rel_dir: String,
    /// Tamaño + fecha ya formateados ("12 KB · 2026-06-17").
    pub detail: String,
    pub is_dir: bool,
    pub icon: slint::Image,
}

/// Listado profundo (vista recursiva) en curso/terminado para un panel. Un solo job a la vez.
/// Igual que SearchJob, pero las entradas se vuelcan en las filas NORMALES del panel (no en un
/// overlay): cada una lleva su profundidad.
pub struct DeepJob {
    pub pane: PaneId,
    pub root: PathBuf,
    pub rx: std::sync::mpsc::Receiver<naygo_core::deep_listing::DeepMsg>,
    pub token: naygo_core::CancellationToken,
    /// Entradas acumuladas con su profundidad (orden de descubrimiento).
    pub items: Vec<(naygo_core::fs_model::Entry, u32)>,
    pub dirs_scanned: usize,
    pub done: bool,
    pub cancelled: bool,
    pub partial: bool,
}

/// Búsqueda recursiva en curso/terminada (overlay de resultados). Un solo job a la vez: una
/// búsqueda nueva cancela y reemplaza la anterior. Mientras `open`, la UI muestra el panel de
/// resultados. Modelado igual que `SizeJob`: worker + canal + token + estado acumulado.
pub struct SearchJob {
    /// Carpeta raíz bajo la que se busca (la del panel activo al disparar).
    pub root: PathBuf,
    /// Texto buscado (lo que el usuario tipeó; se muestra en el encabezado).
    pub query: String,
    pub rx: std::sync::mpsc::Receiver<naygo_core::search::SearchMsg>,
    pub token: naygo_core::CancellationToken,
    /// Coincidencias acumuladas (en el orden en que el worker las descubrió).
    pub hits: Vec<SearchHit>,
    /// Cuántas carpetas se han recorrido (para el indicador de avance).
    pub dirs_scanned: usize,
    pub done: bool,
    pub cancelled: bool,
    /// Hubo carpetas ilegibles (permiso/desaparición): resultado parcial.
    pub partial: bool,
    /// Se cortó por alcanzar el tope `MAX_HITS`.
    pub hit_cap: bool,
}

/// Estado de la ventana de renombrado por lotes (Fase 5): el `BatchSpec` que el usuario edita en
/// vivo + los ítems objetivo + los nombres existentes del directorio (para detectar colisiones).
/// El preview se recalcula con `core::batch_rename::preview` cada vez que cambia el spec.
#[derive(Clone, Debug)]
pub struct BatchRenameState {
    pub spec: naygo_core::batch_rename::BatchSpec,
    pub items: Vec<naygo_core::batch_rename::BatchItem>,
    pub existing: Vec<String>,
    pub tz_offset_secs: i64,
}

impl WorkspaceCtrl {
    /// Arranca con UN panel Files en `start` (el usuario agrega más con el botón). Lanza
    /// su listado inicial. La configuración se carga del directorio portable.
    pub fn new(start: std::path::PathBuf) -> WorkspaceCtrl {
        WorkspaceCtrl::new_in(start, naygo_core::config::portable_dir())
    }

    /// Como `new`, pero con el directorio de configuración explícito (para tests: permite
    /// usar un directorio temporal en vez del portable).
    pub fn new_in(start: std::path::PathBuf, config_dir: std::path::PathBuf) -> WorkspaceCtrl {
        let mut ws = Workspace::new();
        let id = ws.add_pane(PanePurpose::Files, start.clone());
        ws.layout = SerializableDockLayout::single(id);
        ws.set_active(id);
        let config = crate::config_ctrl::ConfigCtrl::new(config_dir.clone());
        let icons =
            crate::icons::IconCache::new(config.settings.icon_set.clone(), config_dir.clone());
        let mut c = WorkspaceCtrl {
            ws,
            config,
            listings: HashMap::new(),
            trees: HashMap::new(),
            tree_cursor: HashMap::new(),
            tree_listings: HashMap::new(),
            reveal_targets: HashMap::new(),
            favorites: Favorites::new(),
            recents: RecentDirs::new(),
            ops: crate::ops_ctrl::OpsCtrl::new(config_dir.clone()),
            preview: crate::preview::PreviewState::new(),
            pending_pick: None,
            last_area: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            last_click: None,
            last_open: None,
            typeahead: String::new(),
            typeahead_at: None,
            ctrl_down: false,
            shift_down: false,
            context_menu: None,
            column_menu: None,
            templates: naygo_core::config::load_templates(&config_dir),
            batch: None,
            new_folder: None,
            help_open: false,
            size_job: None,
            search_job: None,
            deep_job: None,
            last_saved_fingerprint: None,
            watchers: crate::watch::Watchers::new(),
            last_active_files: Some(id),
            edit_path_requested: None,
            rename_requested: None,
            icons,
        };
        c.push_recent(start.clone());
        c.start_listing(id, start);
        // Aplicar el modo de operaciones (cola/paralelo) guardado en Settings al motor de ops.
        c.sync_ops_mode();
        c
    }

    /// Propaga el modo de operaciones de Settings (cola/paralelo) al controlador de ops. Antes
    /// el motor de cola existía pero arrancaba SIEMPRE en paralelo (el campo de Settings no se
    /// aplicaba), así que la cola era inalcanzable. Se llama al arrancar y al cambiarlo en config.
    pub fn sync_ops_mode(&mut self) {
        use naygo_core::config::OpsMode as CfgMode;
        self.ops.ops_mode = match self.config.settings.ops_mode {
            CfgMode::Queue => crate::ops_ctrl::OpsMode::Queue,
            CfgMode::Parallel => crate::ops_ctrl::OpsMode::Parallel,
        };
    }

    /// Registra una visita en el historial respetando el límite configurado
    /// (`Settings.recent_limit`, clampeado a 1..=100). Centraliza el límite.
    fn push_recent(&mut self, dir: std::path::PathBuf) {
        let limit = self.config.settings.recent_limit.clamp(1, 100);
        self.recents.push(dir, limit);
    }

    /// Fija el límite de carpetas recientes, persiste y trunca la lista al nuevo tope.
    /// Cableado desde main.rs: on_recent_limit_changed en la ventana de configuración.
    pub fn set_recent_limit(&mut self, n: usize) {
        self.config.settings.recent_limit = n.clamp(1, 100);
        self.config.save();
        let limit = self.config.settings.recent_limit;
        self.recents.truncate_to(limit);
    }

    /// Estado persistible del workspace (para guardar al cerrar la ventana): disposición,
    /// panel activo, estado de cada panel Files y el tipo de cada panel.
    pub fn session_persist(&self) -> naygo_core::config::WorkspacePersist {
        naygo_core::config::WorkspacePersist {
            version: 1,
            layout: self.ws.layout.clone(),
            active: self.ws.active_id(),
            files: self
                .ws
                .panes()
                .iter()
                .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.to_persist())))
                .collect(),
            purposes: self.ws.panes().iter().map(|p| (p.id, p.purpose)).collect(),
        }
    }

    /// Guarda la sesión actual en disco (la llama la UI al cerrar la ventana).
    pub fn save_session(&self) {
        naygo_core::config::save_workspace(&self.config.config_dir, &self.session_persist());
    }

    /// Intenta restaurar la sesión guardada (al arrancar). Si hay un workspace.json válido,
    /// reemplaza el workspace de arranque por el restaurado y relanza el listado de cada
    /// panel Files + el árbol de cada panel Tree. Si no hay sesión guardada (o el layout es
    /// vacío/corrupto), no hace nada y se conserva el arranque por defecto.
    ///
    /// Devuelve `true` si restauró una sesión, `false` si no había ninguna (primera
    /// ejecución). El llamador usa ese flag para, en primera ejecución, aplicar la
    /// disposición clásica en vez del panel único de arranque.
    pub fn load_session(&mut self) -> bool {
        let (persist, _recovered) =
            naygo_core::config::load_workspace_flagged(&self.config.config_dir);
        let Some(persist) = persist else {
            return false;
        };
        let Some(restored) = Workspace::from_persist(&persist) else {
            return false;
        };
        // Cancelar los listados del arranque por defecto antes de reemplazar el workspace.
        for l in self.listings.values() {
            l.cancel();
        }
        self.listings.clear();
        self.trees.clear();
        // Soltar el deep job si lo había: el workspace va a ser reemplazado completo.
        if let Some(d) = self.deep_job.take() {
            d.token.cancel();
        }
        self.ws = restored;
        // Relanzar el contenido de cada panel restaurado.
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
        // La sesión recién cargada ES el estado en disco: sembrar la huella para no
        // reescribir el mismo workspace.json en el primer tick.
        self.last_saved_fingerprint = Some(self.session_fingerprint());
        // Sembrar el último Files activo con el activo restaurado (o el primer Files).
        self.last_active_files = self
            .ws
            .active_id()
            .filter(|a| self.ws.pane(*a).map(|p| p.purpose) == Some(PanePurpose::Files))
            .or_else(|| self.ws.files_panes().first().copied());
        // Arrancar el REVEAL del árbol hasta la carpeta activa restaurada: antes solo se hacía
        // `set_active` en cada árbol (fijaba el destino) pero no se sembraba `reveal_targets` ni
        // se arrancaba `pump_reveal`, así que al iniciar el árbol no expandía hasta la carpeta.
        if let Some(active_dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
            self.sync_trees_active(active_dir);
        }
        true
    }

    /// Aplica la disposición CLÁSICA de primera ejecución: árbol + dos paneles de
    /// archivos + Propiedades (Inspector) + Vista previa (Preview). Reemplaza el
    /// workspace de arranque (un solo panel) y relanza el contenido de cada panel,
    /// reusando la misma maquinaria que `apply_template`. La llama `main` SOLO cuando
    /// no hay sesión guardada (primera ejecución); las sesiones guardadas se respetan.
    /// A diferencia de `apply_template`, NO registra un uso en la lista de plantillas
    /// recientes (no es una elección del usuario en el menú).
    pub fn apply_first_run_layout(&mut self) {
        let tpl = naygo_core::workspace::LayoutTemplate::primera_ejecucion();
        crate::logging::breadcrumb("aplicar layout clásico (primera ejecución)");
        let home = self.template_home();
        // Cancelar los listados/árboles del arranque por defecto antes de reemplazar el workspace.
        for l in self.listings.values() {
            l.cancel();
        }
        self.listings.clear();
        self.trees.clear();
        self.tree_listings.clear();
        self.reveal_targets.clear();
        if let Some(d) = self.deep_job.take() {
            d.token.cancel();
        }
        self.ws = naygo_core::workspace::Workspace::from_template(&tpl, &home);
        self.relaunch_all_panes();
        self.last_active_files = self.ws.files_panes().first().copied();
    }

    /// Huella barata del estado persistible (lo que cambia entre sesiones que vale la pena
    /// guardar): por cada panel, su id + tipo + carpeta; más la disposición y el activo. NO
    /// incluye scroll ni selección (no se persisten). Si cambia, hay que volver a guardar.
    pub fn session_fingerprint(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        for p in self.ws.panes() {
            let dir = p
                .files
                .as_ref()
                .map(|f| f.current_dir.to_string_lossy().into_owned())
                .unwrap_or_default();
            let _ = write!(s, "{}:{:?}:{}|", p.id.0, p.purpose, dir);
        }
        let _ = write!(
            s,
            "active={:?};layout={:?}",
            self.ws.active_id().map(|a| a.0),
            self.ws
                .layout
                .pane_ids()
                .iter()
                .map(|i| i.0)
                .collect::<Vec<_>>()
        );
        s
    }

    /// Persiste la sesión SOLO si cambió respecto de la última guardada (compara huellas).
    /// La UI la llama en cada tick: barato cuando no hay cambios (solo construye la huella),
    /// y garantiza que la sesión en disco quede al día tras agregar/cerrar/navegar paneles,
    /// sin depender del evento de cierre de ventana.
    pub fn maybe_persist_session(&mut self) {
        let fp = self.session_fingerprint();
        if self.last_saved_fingerprint.as_deref() != Some(fp.as_str()) {
            self.save_session();
            self.last_saved_fingerprint = Some(fp);
        }
    }

    /// Asegura que cada panel Files tenga un watcher sobre su carpeta actual: arranca uno si
    /// falta o si la carpeta cambió, y quita los watchers de paneles que ya no existen. La UI lo
    /// llama en el tick (tiene el `waker` de Slint). Es barato cuando nada cambió.
    pub fn reconcile_watchers(&mut self, waker: naygo_platform::dir_watch::Waker) {
        // Paneles Files actuales y su carpeta.
        let files: Vec<(PaneId, std::path::PathBuf)> = self
            .ws
            .panes()
            .iter()
            .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.current_dir.clone())))
            .collect();
        let vivos: std::collections::HashSet<u64> = files.iter().map(|(id, _)| id.0).collect();
        // Quitar watchers de paneles que ya no existen.
        for pane in self.watchers.watched_panes() {
            if !vivos.contains(&pane) {
                self.watchers.unwatch(pane);
            }
        }
        // Arrancar/re-arrancar el watcher de cada panel cuya carpeta cambió.
        for (id, dir) in files {
            let needs = self.watchers.current_dir(id.0) != Some(dir.as_path());
            if needs {
                self.watchers.watch(id.0, dir, waker.clone());
            }
        }
    }

    /// Aplica un lote de eventos del watcher al panel `pane`: muta sus entries SIN re-listar,
    /// re-ordena, y devuelve las rutas NUEVAS (para resaltar). No-op si el panel no es Files.
    pub fn apply_watch_events(
        &mut self,
        pane: PaneId,
        events: &[naygo_core::listing::DirEvent],
    ) -> Vec<std::path::PathBuf> {
        let Some(f) = self.ws.pane_mut(pane).and_then(|p| p.files.as_mut()) else {
            return Vec::new();
        };
        // `read_entry` debe devolver `Option<Entry>`: leemos metadata (puede fallar si la ruta
        // ya desapareció) y armamos el Entry.
        let nuevas = naygo_core::listing::apply_dir_events(&mut f.entries, events, &|p| {
            std::fs::metadata(p)
                .ok()
                .map(|m| naygo_core::listing::entry_from_path(p, Some(&m)))
        });
        let spec = f.sort;
        naygo_core::sort::sort_entries(&mut f.entries, &spec);
        nuevas
    }

    /// Tras un cambio de unidades (USB enchufado/quitado), reubica a `home` los paneles Files
    /// cuya carpeta ya no existe (p. ej. el USB se sacó). Devuelve los panes reubicados para
    /// que la UI re-liste su contenido. No crashea: "el filesystem es hostil".
    pub fn relocate_orphans(&mut self, _home: &std::path::Path) -> Vec<PaneId> {
        // Ya NO reubica ni abre un popup global: cada panel cuya carpeta desapareció muestra el
        // aviso "carpeta no encontrada" IN-PLACE (lo decide `pane_dir_missing` en el builder del
        // PaneVm). Acá no hay nada que hacer salvo no mandar el panel a HOME en silencio.
        Vec::new()
    }

    /// ¿Se puede navegar a `dir`? Existe Y se puede abrir para listar (permiso). `read_dir` es la
    /// prueba real: `exists()` puede mentir en rutas de red/permiso. Si no es navegable, el panel
    /// muestra el aviso in-place tras navegar ahí.
    fn dir_is_navigable(dir: &std::path::Path) -> bool {
        std::fs::read_dir(dir).is_ok()
    }

    /// El ancestro existente más cercano de `path` (sube hasta encontrar uno que exista; si
    /// ninguno —p. ej. la unidad entera se fue—, devuelve None).
    fn nearest_existing_ancestor(path: &std::path::Path) -> Option<PathBuf> {
        let mut cur = path.parent();
        while let Some(p) = cur {
            if p.exists() {
                return Some(p.to_path_buf());
            }
            cur = p.parent();
        }
        None
    }

    /// La carpeta actual del panel `id` (para el aviso in-place de "carpeta no encontrada").
    fn pane_current_dir(&self, id: PaneId) -> Option<PathBuf> {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
    }

    /// ¿La carpeta del panel `id` dejó de existir / es ilegible? Lo consulta el builder del
    /// PaneVm para mostrar el aviso DENTRO de ese panel (in-place), con sus opciones.
    pub fn pane_dir_missing(&self, id: PaneId) -> bool {
        match self.pane_current_dir(id) {
            Some(dir) => std::fs::read_dir(&dir).is_err(),
            None => false,
        }
    }

    /// ¿El panel `id` (en estado "carpeta no encontrada") tiene un ancestro existente real al
    /// que subir? Falso si la unidad entera se desconectó (no hay a dónde subir con sentido); en
    /// ese caso el aviso oculta el botón "subir un nivel".
    pub fn pane_has_existing_ancestor(&self, id: PaneId) -> bool {
        self.pane_current_dir(id)
            .and_then(|d| Self::nearest_existing_ancestor(&d))
            .is_some()
    }

    /// Reintentar en el panel `id`: si su carpeta volvió a existir (USB reconectado), re-listar; si
    /// sigue sin existir, no hace nada (el aviso permanece). El aviso desaparece solo cuando el
    /// re-listado puebla la carpeta (la detección es por `current_dir.exists()`).
    pub fn missing_folder_retry(&mut self, id: PaneId) {
        crate::logging::log_line(&format!("carpeta no encontrada: reintentar panel {}", id.0));
        if let Some(dir) = self.pane_current_dir(id) {
            if std::fs::read_dir(&dir).is_ok() {
                self.cancel_deep_if_navigating(id);
                self.start_listing(id, dir.clone());
                self.sync_trees_active(dir);
            }
        }
    }

    /// Subir al ancestro existente más cercano del panel `id` (o al HOME si la unidad entera se
    /// fue). Navega ese panel y re-lista.
    pub fn missing_folder_go_ancestor(&mut self, id: PaneId) {
        {
            let dir = self
                .pane_current_dir(id)
                .map(|d| d.display().to_string())
                .unwrap_or_default();
            crate::logging::log_line(&format!(
                "carpeta no encontrada: subir del panel {} ({})",
                id.0, dir
            ));
        }
        let Some(lost) = self.pane_current_dir(id) else {
            return;
        };
        let dest = Self::nearest_existing_ancestor(&lost).unwrap_or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("C:/"))
        });
        self.cancel_deep_if_navigating(id);
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dest.clone());
        }
        self.start_listing(id, dest.clone());
        self.sync_trees_active(dest);
    }

    /// Navegar el panel `id` a `dir` (elegido en el selector nativo) y re-listar.
    pub fn missing_folder_choose(&mut self, id: PaneId, dir: PathBuf) {
        self.cancel_deep_if_navigating(id);
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dir.clone());
        }
        self.start_listing(id, dir.clone());
        self.sync_trees_active(dir);
    }

    /// Cerrar el panel `id` (si se puede; si es el último, lo manda al HOME en su lugar).
    pub fn missing_folder_close_pane(&mut self, id: PaneId) {
        if self.can_close_pane(id) {
            self.close_pane(id);
        } else {
            // Es el último panel: no se puede cerrar; lo mandamos al HOME para que sea usable.
            let home = std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("C:/"));
            self.cancel_deep_if_navigating(id);
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.navigate_to(home.clone());
            }
            self.start_listing(id, home);
        }
    }

    /// Arranca el listado del panel `id` en `dir` (cancela el suyo anterior).
    pub fn start_listing(&mut self, id: PaneId, dir: std::path::PathBuf) {
        if let Some(l) = self.listings.get(&id) {
            l.cancel();
        }
        self.listings.insert(id, Listing::start(dir));
    }

    /// Drena los lotes de TODOS los listados activos. Devuelve true si TODOS terminaron
    /// (para apagar el timer). Quita del mapa los que terminan.
    pub fn pump_listings(&mut self) -> bool {
        let ids: Vec<PaneId> = self.listings.keys().copied().collect();
        for id in ids {
            let (batch, done) = match self.listings.get(&id) {
                Some(l) => l.poll(),
                None => continue,
            };
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                if !batch.is_empty() {
                    f.entries.extend(batch);
                }
                if done {
                    let spec = f.sort;
                    naygo_core::sort::sort_entries(&mut f.entries, &spec);
                    if f.focused.is_none() && !f.entries.is_empty() {
                        f.focused = Some(0);
                    }
                }
            }
            if done {
                self.listings.remove(&id);
            }
        }
        self.listings.is_empty()
    }

    /// Rects de los paneles (id, rect) dado el área de contenido.
    pub fn pane_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        self.ws.layout.pane_rects(area)
    }

    /// Recuerda el área de contenido actual (la UI la setea en cada layout) para resolver
    /// destinos por orden visual desde gestos sin área (teclado).
    pub fn set_area(&mut self, area: Rect) {
        self.last_area = area;
    }

    /// Handles de splitter (para pintarlos y arrastrarlos).
    pub fn split_handles(&self, area: Rect) -> Vec<SplitHandle> {
        self.ws.layout.split_handles(area)
    }

    /// Ajusta la fracción de un split (drag de splitter).
    pub fn set_fraction(&mut self, path: &[SplitStep], fraction: f32) {
        self.ws.layout.set_fraction(path, fraction);
    }

    /// Fracción + rect de la barra-fantasma para un split dado el puntero (vista previa del drag).
    pub fn fraction_at(
        &self,
        path: &[SplitStep],
        area: Rect,
        px: f32,
        py: f32,
    ) -> Option<(f32, Rect)> {
        self.ws.layout.fraction_at(path, area, px, py)
    }

    /// Segundos que dura el resaltado de archivos nuevos, según el ajuste. `FadeSeconds(n)`→n;
    /// `UntilInteract`/`UntilRefresh` se aproximan con un tope generoso (el resaltado es una
    /// pista transitoria; modelar "hasta interactuar" exactamente no aporta y complica).
    pub fn highlight_secs(&self) -> u64 {
        use naygo_core::config::HighlightDuration::*;
        match self.config.settings.highlight_duration {
            FadeSeconds(n) => n as u64,
            UntilInteract | UntilRefresh => 8,
        }
    }

    /// Filas a pintar del panel `id` (marca las cortadas para atenuarlas y las recién
    /// aparecidas para resaltarlas durante `highlight_secs` segundos).
    pub fn rows_of(
        &mut self,
        id: PaneId,
        highlight_secs: u64,
        now: std::time::Instant,
    ) -> Vec<PlainRow> {
        let date_format = self.config.settings.date_format;
        let size_format = self.config.settings.size_format;
        let tz = naygo_platform::time::local_utc_offset_secs();

        // Vista profunda activa: construir filas desde deep_items con depth real.
        // Las columnas visibles (orden/formato) se leen del FilePaneState normal.
        if self.is_deep_active(id) {
            // Extraer los datos que necesitamos antes de los préstamos disjuntos.
            let cell_kinds: Vec<naygo_core::columns::ColumnKind> = self
                .ws
                .pane(id)
                .and_then(|p| p.files.as_ref())
                .map(|f| f.table.visible_columns().map(|c| c.kind).collect())
                .unwrap_or_default();

            // Clonar los items del job profundo (evita préstamos solapados con icons/ops).
            let deep_items: Vec<(naygo_core::fs_model::Entry, u32)> = self
                .deep_job
                .as_ref()
                .map(|d| d.items.clone())
                .unwrap_or_default();

            let WorkspaceCtrl { ops, icons, .. } = self;
            return deep_items
                .iter()
                .map(|(e, depth)| {
                    let cells = cell_kinds
                        .iter()
                        .map(|k| crate::bridge::cell_value(e, *k, size_format, date_format, tz))
                        .collect();
                    PlainRow {
                        name: e.name.clone(),
                        cells,
                        is_dir: e.kind == naygo_core::fs_model::EntryKind::Directory,
                        selected: false,
                        focused: false,
                        cut: ops.is_cut(&e.path),
                        highlight: false,
                        icon: icons.get(naygo_core::icon_kind::icon_key_for(e)),
                        depth: *depth,
                    }
                })
                .collect();
        }

        // Vista normal: préstamos disjuntos para ops/watchers/icons.
        let WorkspaceCtrl {
            ws,
            ops,
            watchers,
            icons,
            ..
        } = self;
        match ws.pane(id).and_then(|p| p.files.as_ref()) {
            Some(f) => rows_from_view(
                f,
                &|p| ops.is_cut(p),
                &|p| watchers.is_fresh_ro(id.0, p, highlight_secs, now),
                &mut |e| icons.get(naygo_core::icon_kind::icon_key_for(e)),
                size_format,
                date_format,
                tz,
            ),
            None => Vec::new(),
        }
    }

    /// Etiqueta i18n de una columna (clave `col.*`).
    fn column_label(&self, kind: naygo_core::columns::ColumnKind) -> String {
        use naygo_core::columns::ColumnKind::*;
        let key = match kind {
            Name => "col.name",
            Extension => "col.extension",
            Size => "col.size",
            Modified => "col.modified",
            Created => "col.created",
        };
        self.config.t(key)
    }

    /// Columnas visibles del panel `id` (orden/etiqueta/ancho/alineación) para pintar el header
    /// y repartir las celdas. Vacío si el panel no es Files. La etiqueta sale del i18n.
    pub fn columns_of(&self, id: PaneId) -> Vec<crate::bridge::ColumnInfo> {
        let label_of = |kind| self.column_label(kind);
        match self.ws.pane(id).and_then(|p| p.files.as_ref()) {
            Some(f) => crate::bridge::columns_info(f, &label_of),
            None => Vec::new(),
        }
    }

    /// TODAS las columnas del panel `id` (con su visibilidad) para el menú "Columnas…".
    pub fn column_toggles_of(&self, id: PaneId) -> Vec<crate::bridge::ColumnToggle> {
        let label_of = |kind| self.column_label(kind);
        match self.ws.pane(id).and_then(|p| p.files.as_ref()) {
            Some(f) => crate::bridge::column_toggles(f, &label_of),
            None => Vec::new(),
        }
    }

    /// Alterna la visibilidad de una columna del panel `id` (Name nunca se oculta). Persiste.
    pub fn column_toggle(&mut self, id: PaneId, kind_int: i32) {
        let kind = crate::bridge::column_kind_from_int(kind_int);
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.table.toggle_visible(kind);
        }
        self.maybe_persist_session();
    }

    /// Reordena la columna `from`→`to` (índices en el orden COMPLETO de columnas) del panel `id`.
    pub fn column_move(&mut self, id: PaneId, from: i32, to: i32) {
        if from < 0 || to < 0 {
            return;
        }
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.table.move_column(from as usize, to as usize);
        }
        self.maybe_persist_session();
    }

    /// Fija el ancho de una columna del panel `id` (clamp a MIN/MAX). Persiste.
    pub fn column_resize(&mut self, id: PaneId, kind_int: i32, width: f32) {
        let kind = crate::bridge::column_kind_from_int(kind_int);
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.table.set_width(kind, width);
        }
        self.maybe_persist_session();
    }

    /// Huso local en segundos (para formatear/parsear fechas del filtro de fecha).
    fn tz_offset_secs(&self) -> i64 {
        naygo_platform::time::local_utc_offset_secs()
    }

    // --- Menú/editor de columna (clic derecho en el header, F2) ---

    /// Abre el menú de columna en (x,y) para la columna `kind_int` del panel `id`. Siembra los
    /// borradores del editor de filtro con el filtro YA activo de esa columna (si lo hay).
    pub fn column_menu_open(&mut self, id: PaneId, kind_int: i32, x: f32, y: f32) {
        use naygo_core::filter::ColumnFilter;
        let kind = crate::bridge::column_kind_from_int(kind_int);
        // Cerrar otros overlays para no apilarlos.
        self.context_menu = None;
        let mut st = ColumnMenuState {
            pane: id,
            kind,
            x,
            y,
            mode: ColumnMenuMode::Menu,
            text_draft: String::new(),
            text_case: false,
            min_draft: String::new(),
            max_draft: String::new(),
            ext_checked: std::collections::BTreeSet::new(),
        };
        // Sembrar desde el filtro activo (si existe) para editar en vez de empezar de cero.
        if let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) {
            if let Some(filter) = f.table.filters.get(&kind) {
                match filter {
                    ColumnFilter::Text {
                        contains,
                        case_sensitive,
                    } => {
                        st.text_draft = contains.clone();
                        st.text_case = *case_sensitive;
                    }
                    ColumnFilter::Extensions(set) => st.ext_checked = set.clone(),
                    ColumnFilter::SizeRange { min, max } => {
                        if let Some(m) = min {
                            st.min_draft = m.to_string();
                        }
                        if let Some(m) = max {
                            st.max_draft = m.to_string();
                        }
                    }
                    ColumnFilter::DateRange { from, to } => {
                        if let Some(t) = from {
                            st.min_draft = fmt_date_ymd(*t, self.tz_offset_secs());
                        }
                        if let Some(t) = to {
                            st.max_draft = fmt_date_ymd(*t, self.tz_offset_secs());
                        }
                    }
                }
            }
        }
        self.column_menu = Some(st);
    }

    /// Cierra el menú/editor de columna.
    pub fn column_menu_close(&mut self) {
        self.column_menu = None;
    }

    /// Instantánea del menú de columna para la UI (espejo de `ColumnMenuVm`). `None` si no hay
    /// menú abierto. Incluye etiqueta, modo, si la columna tiene filtro, si se puede ocultar, los
    /// borradores y —para Extension— las extensiones marcables con su conteo y estado.
    pub fn column_menu_snapshot(&self) -> Option<crate::bridge::ColumnMenuInfo> {
        let st = self.column_menu.as_ref()?;
        let has_filter = self
            .ws
            .pane(st.pane)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.table.filters.contains_key(&st.kind))
            .unwrap_or(false);
        let exts = if st.kind == naygo_core::columns::ColumnKind::Extension {
            self.column_filter_ext_counts()
                .into_iter()
                .map(|(ext, count)| crate::bridge::ExtRowInfo {
                    checked: st.ext_checked.contains(&ext),
                    ext,
                    count,
                })
                .collect()
        } else {
            Vec::new()
        };
        Some(crate::bridge::ColumnMenuInfo {
            x: st.x,
            y: st.y,
            kind: crate::bridge::column_kind_to_int(st.kind),
            label: self.column_label(st.kind),
            mode: if st.mode == ColumnMenuMode::Filter {
                1
            } else {
                0
            },
            has_filter,
            can_hide: st.kind != naygo_core::columns::ColumnKind::Name,
            text_draft: st.text_draft.clone(),
            text_case: st.text_case,
            min_draft: st.min_draft.clone(),
            max_draft: st.max_draft.clone(),
            exts,
        })
    }

    /// Pasa el menú de columna a modo editor de filtro (siembra ya hecha al abrir).
    pub fn column_menu_to_filter(&mut self) {
        if let Some(st) = self.column_menu.as_mut() {
            st.mode = ColumnMenuMode::Filter;
        }
    }

    /// Ordena por la columna del menú en una dirección explícita (true = ascendente). Cierra.
    pub fn column_menu_sort(&mut self, ascending: bool) {
        let Some(st) = self.column_menu.clone() else {
            return;
        };
        let key = naygo_core::columns::sort_key_of(st.kind);
        if let Some(f) = self.ws.pane_mut(st.pane).and_then(|p| p.files.as_mut()) {
            f.sort.key = key;
            f.sort.ascending = ascending;
            let spec = f.sort;
            naygo_core::sort::sort_entries(&mut f.entries, &spec);
        }
        self.column_menu = None;
    }

    /// Quita el filtro de la columna del menú. Cierra.
    pub fn column_menu_clear_filter(&mut self) {
        let Some(st) = self.column_menu.clone() else {
            return;
        };
        if let Some(f) = self.ws.pane_mut(st.pane).and_then(|p| p.files.as_mut()) {
            f.table.clear_filter(st.kind);
        }
        self.column_menu = None;
        self.maybe_persist_session();
    }

    /// Mueve la columna del menú una posición a la izquierda (dir=-1) o derecha (dir=+1) en el
    /// orden visual COMPLETO. Cierra el menú. Clampa en los extremos (no hace nada si no cabe).
    pub fn column_menu_move(&mut self, dir: i32) {
        let Some(st) = self.column_menu.clone() else {
            return;
        };
        if let Some(f) = self.ws.pane_mut(st.pane).and_then(|p| p.files.as_mut()) {
            if let Some(from) = f.table.columns.iter().position(|c| c.kind == st.kind) {
                let to = from as i32 + dir;
                if to >= 0 && (to as usize) < f.table.columns.len() {
                    f.table.move_column(from, to as usize);
                }
            }
        }
        self.column_menu = None;
        self.maybe_persist_session();
    }

    /// Oculta la columna del menú (Name no se oculta). Cierra.
    pub fn column_menu_hide(&mut self) {
        let Some(st) = self.column_menu.clone() else {
            return;
        };
        if let Some(f) = self.ws.pane_mut(st.pane).and_then(|p| p.files.as_mut()) {
            f.table.toggle_visible(st.kind);
        }
        self.column_menu = None;
        self.maybe_persist_session();
    }

    /// Editor de filtro: fija el borrador de TEXTO (subcadena del filtro Name/Extension).
    pub fn column_filter_set_text(&mut self, text: &str) {
        if let Some(st) = self.column_menu.as_mut() {
            st.text_draft = text.to_string();
        }
    }

    /// Editor de filtro: alterna sensibilidad a mayúsculas del filtro de texto.
    pub fn column_filter_toggle_case(&mut self) {
        if let Some(st) = self.column_menu.as_mut() {
            st.text_case = !st.text_case;
        }
    }

    /// Editor de filtro: fija el borrador del rango (Size/fecha). `is_max` elige el extremo.
    pub fn column_filter_set_range(&mut self, is_max: bool, text: &str) {
        if let Some(st) = self.column_menu.as_mut() {
            if is_max {
                st.max_draft = text.to_string();
            } else {
                st.min_draft = text.to_string();
            }
        }
    }

    /// Editor de filtro: marca/desmarca una extensión en el filtro de tipos.
    pub fn column_filter_toggle_ext(&mut self, ext: &str) {
        if let Some(st) = self.column_menu.as_mut() {
            if !st.ext_checked.remove(ext) {
                st.ext_checked.insert(ext.to_string());
            }
        }
    }

    /// Cuenta de extensiones de la carpeta activa del panel del menú (para la lista del filtro de
    /// tipos): pares (extensión, conteo) en orden alfabético; "" = sin extensión.
    pub fn column_filter_ext_counts(&self) -> Vec<(String, usize)> {
        let Some(st) = self.column_menu.as_ref() else {
            return Vec::new();
        };
        match self.ws.pane(st.pane).and_then(|p| p.files.as_ref()) {
            Some(f) => naygo_core::filter::extension_counts(&f.entries)
                .into_iter()
                .collect(),
            None => Vec::new(),
        }
    }

    /// Aplica el filtro del editor según el tipo de la columna. Borradores vacíos = sin filtro
    /// (lo quita). Cierra el menú. La vista se refiltra sola (el caché se invalida por la firma).
    pub fn column_filter_apply(&mut self) {
        use naygo_core::filter::ColumnFilter;
        let Some(st) = self.column_menu.clone() else {
            return;
        };
        let filter = match st.kind {
            naygo_core::columns::ColumnKind::Extension => {
                if st.ext_checked.is_empty() {
                    None
                } else {
                    Some(ColumnFilter::Extensions(st.ext_checked.clone()))
                }
            }
            naygo_core::columns::ColumnKind::Size => {
                let min = parse_size(&st.min_draft);
                let max = parse_size(&st.max_draft);
                if min.is_none() && max.is_none() {
                    None
                } else {
                    Some(ColumnFilter::SizeRange { min, max })
                }
            }
            naygo_core::columns::ColumnKind::Modified
            | naygo_core::columns::ColumnKind::Created => {
                let tz = self.tz_offset_secs();
                let from = parse_date_ymd(&st.min_draft, tz, false);
                let to = parse_date_ymd(&st.max_draft, tz, true);
                if from.is_none() && to.is_none() {
                    None
                } else {
                    Some(ColumnFilter::DateRange { from, to })
                }
            }
            // Name (y cualquier otra): filtro de texto sobre el nombre.
            _ => {
                if st.text_draft.is_empty() {
                    None
                } else {
                    Some(ColumnFilter::Text {
                        contains: st.text_draft.clone(),
                        case_sensitive: st.text_case,
                    })
                }
            }
        };
        if let Some(f) = self.ws.pane_mut(st.pane).and_then(|p| p.files.as_mut()) {
            match filter {
                Some(flt) => f.table.set_filter(st.kind, flt),
                None => f.table.clear_filter(st.kind),
            }
        }
        self.column_menu = None;
        self.maybe_persist_session();
    }

    /// `true` si el panel `id` tiene filtros activos pero su vista quedó vacía (aviso "sin
    /// coincidencias"). Distingue "carpeta vacía" (sin entries) de "filtro la vació".
    pub fn no_matches(&self, id: PaneId) -> bool {
        match self.ws.pane(id).and_then(|p| p.files.as_ref()) {
            Some(f) => !f.table.filters.is_empty() && !f.entries.is_empty() && f.view_len() == 0,
            None => false,
        }
    }

    // --- Plantillas de disposición de paneles (Fase 4) ---

    /// Carpeta «home» para las plantillas (`TemplateDir::Home`): la carpeta del Files activo si la
    /// hay; si no, %USERPROFILE%; si no, la raíz del disco del sistema.
    fn template_home(&self) -> PathBuf {
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
    fn relaunch_all_panes(&mut self) {
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
        self.ops
            .start_op(req, "Renombrar por lotes".to_string(), true);
        self.batch = None;
    }

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

    /// Navega el panel `id` a `dir` (clic en un breadcrumb / commit del editor). Reusa la
    /// lógica de navegación del panel activo, pero dirigida a `id`.
    pub fn navigate_pane_to(&mut self, id: PaneId, dir: PathBuf) -> bool {
        if self.ws.pane(id).and_then(|p| p.files.as_ref()).is_none() {
            return false;
        }
        crate::logging::breadcrumb(&format!("navegar panel {} → {}", id.0, dir.display()));
        // Navegar cancela la vista profunda del panel (no es pegajosa).
        self.cancel_deep_if_navigating(id);
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dir.clone());
        }
        self.push_recent(dir.clone());
        self.start_listing(id, dir.clone());
        self.sync_trees_active(dir);
        true
    }

    pub fn active_id(&self) -> Option<PaneId> {
        self.ws.active_id()
    }

    /// El propósito (tipo) del panel `id`, si existe.
    pub fn purpose_of(&self, id: PaneId) -> Option<PanePurpose> {
        self.ws.pane(id).map(|p| p.purpose)
    }

    /// Texto de la barra de estado: carpeta activa + recuento de ítems y de selección.
    pub fn status_line(&self) -> String {
        let Some(f) = self.ws.active_files() else {
            return String::new();
        };
        let total = f.view_indices().len();
        let sel = f.selected.len();
        let dir = f.current_dir.display();
        let base = if sel > 0 {
            format!("{dir}   —   {total} elementos, {sel} seleccionados")
        } else {
            format!("{dir}   —   {total} elementos")
        };
        // Si hay un cálculo de tamaño (F3) en curso/terminado, anexarlo a la derecha.
        match self.size_status() {
            Some(s) => format!("{base}   —   {s}"),
            None => base,
        }
    }

    /// Etiqueta corta del panel `id` para su pestaña: el nombre de la carpeta (Files) o el
    /// nombre del tipo (paneles especiales).
    pub fn pane_label(&self, id: PaneId) -> String {
        let Some(p) = self.ws.pane(id) else {
            return String::new();
        };
        match p.purpose {
            PanePurpose::Files => p
                .files
                .as_ref()
                .map(|f| {
                    f.current_dir
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| f.current_dir.display().to_string())
                })
                .unwrap_or_default(),
            PanePurpose::Tree => "Árbol".to_string(),
            PanePurpose::Inspector => "Propiedades".to_string(),
            PanePurpose::History => "Historial".to_string(),
            PanePurpose::Favorites => "Favoritos".to_string(),
            PanePurpose::Preview => "Vista previa".to_string(),
        }
    }

    pub fn set_active(&mut self, id: PaneId) {
        self.ws.set_active(id);
        // Recordar el último panel Files activo, para que la navegación desde paneles
        // auxiliares (Árbol/Favoritos) vaya al panel que el usuario venía usando.
        if self.ws.pane(id).map(|p| p.purpose) == Some(PanePurpose::Files) {
            self.last_active_files = Some(id);
            // Al ganar foco un panel Files, expandir/resaltar el árbol hasta SU carpeta (antes el
            // árbol se quedaba en la carpeta del panel anterior hasta navegar).
            if let Some(dir) = self
                .ws
                .pane(id)
                .and_then(|p| p.files.as_ref())
                .map(|f| f.current_dir.clone())
            {
                self.sync_trees_active(dir);
            }
        }
    }

    /// Agrega un panel Files dividiendo el leaf activo lado a lado (horizontal, el nuevo a la
    /// derecha). Atajo de la dirección por defecto.
    pub fn add_pane_split(&mut self) {
        crate::logging::breadcrumb("abrir panel (split)");
        self.add_pane_split_dir(SplitDir::Horizontal, false);
    }

    /// Agrega un panel Files dividiendo el leaf activo en la dirección dada. `first=true` pone
    /// el panel NUEVO antes (a la izquierda / arriba); `false`, después (derecha / abajo). Lo
    /// deja activo y arranca su listado en la misma carpeta que el activo (o el home).
    pub fn add_pane_split_dir(&mut self, dir_split: SplitDir, first: bool) {
        let dir = self
            .ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
        let active = self.ws.active_id();
        let new_id = self.ws.add_pane(PanePurpose::Files, dir.clone());
        self.apply_default_table(new_id);
        if let Some(active) = active {
            self.ws.layout.split_leaf(active, dir_split, new_id);
            if first {
                // split_leaf pone el nuevo como `second`; si se pidió antes, intercambiar.
                self.ws.layout.swap_split_children(active, new_id);
            }
        }
        // Vía self.set_active para que `last_active_files` apunte al nuevo panel Files.
        self.set_active(new_id);
        self.start_listing(new_id, dir);
    }

    /// Agrega un panel del `purpose` dado DIVIDIENDO el leaf activo (horizontal). Los
    /// `Files` arrancan listado en la carpeta del activo; los demás no listan. El Tree
    /// inicializa su `DirTree` desde las unidades del sistema.
    pub fn add_pane_of(&mut self, purpose: PanePurpose) {
        crate::logging::breadcrumb(&format!("abrir panel {:?}", purpose));
        if matches!(purpose, PanePurpose::Files) {
            self.add_pane_split();
            return;
        }
        let dir = self
            .ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| PathBuf::from("C:/"));
        let active = self.ws.active_id();
        let new_id = self.ws.add_pane(purpose, dir);
        if matches!(purpose, PanePurpose::Files) {
            self.apply_default_table(new_id);
        }
        if let Some(active) = active {
            self.ws
                .layout
                .split_leaf(active, SplitDir::Horizontal, new_id);
        }
        if matches!(purpose, PanePurpose::Tree) {
            self.trees.insert(new_id, build_tree());
            // Resalta de entrada la carpeta del panel Files activo y arranca el reveal hacia ella
            // (el árbol nuevo aparece ya expandido hasta la carpeta donde está el usuario).
            if let Some(cur) = self.ws.active_files().map(|f| f.current_dir.clone()) {
                if let Some(t) = self.trees.get_mut(&new_id) {
                    t.set_active(cur.clone());
                }
                self.reveal_targets.insert(new_id, cur);
                self.pump_reveal();
            }
        }
        // El panel nuevo queda activo. Vía self.set_active: si es auxiliar (Árbol…),
        // `last_active_files` NO cambia (sigue apuntando al Files que el usuario venía usando),
        // que es justo lo que queremos para que el árbol navegue ese panel.
        self.set_active(new_id);
    }

    /// `true` si el panel `id` se puede cerrar: hay más de uno (nunca dejamos la ventana sin
    /// ningún panel).
    pub fn can_close_pane(&self, id: PaneId) -> bool {
        self.ws.panes().len() > 1 && self.ws.pane(id).is_some()
    }

    /// Cierra (quita) el panel `id`: cancela su listado en vuelo, suelta su árbol, lo saca del
    /// layout y del workspace, y reasigna el activo. No-op si es el último panel. Tras cerrar,
    /// re-sincroniza el árbol con la carpeta del nuevo panel activo.
    pub fn close_pane(&mut self, id: PaneId) {
        if !self.can_close_pane(id) {
            return;
        }
        crate::logging::breadcrumb(&format!("cerrar panel {}", id.0));
        // Cancelar y soltar el listado/árbol del panel que se va (no dejar workers huérfanos).
        if let Some(l) = self.listings.remove(&id) {
            l.cancel();
        }
        self.trees.remove(&id);
        self.reveal_targets.remove(&id);
        // Sacarlo del layout (el split se colapsa en su hermano) y del workspace (reasigna activo).
        self.ws.layout.remove_leaf(id);
        self.ws.remove_pane(id);
        // Si el último Files activo era este, recomputar.
        if self.last_active_files == Some(id) {
            self.last_active_files = self.ws.files_panes().first().copied();
        }
        // Re-resaltar el árbol hacia la carpeta del panel activo resultante.
        if let Some(dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
            self.sync_trees_active(dir);
        }
    }

    // --- Acciones multi-panel (abrir en otro / swap / clonar) + selector 1..9 ---

    /// Candidatos destino (paneles Files distintos de `origin`) en ORDEN VISUAL
    /// (izquierda→derecha, arriba→abajo) según los rects del layout en `area`.
    pub fn target_candidates(&self, origin: PaneId, area: Rect) -> Vec<PaneId> {
        let others: std::collections::HashSet<PaneId> =
            self.ws.other_files_panes(origin).into_iter().collect();
        let mut with_rect: Vec<(PaneId, Rect)> = self
            .pane_rects(area)
            .into_iter()
            .filter(|(id, _)| others.contains(id))
            .collect();
        // Orden visual: por fila (y) y luego por columna (x), con tolerancia.
        with_rect.sort_by(|(_, a), (_, b)| {
            if (a.y - b.y).abs() > 8.0 {
                a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
            }
        });
        with_rect.into_iter().map(|(id, _)| id).collect()
    }

    /// Resuelve el destino de una acción «hacia otro panel» desde `origin`:
    /// 0 candidatos → NeedsSplit; 1 → Direct; 2+ → Pick (a numerar 1..9).
    pub fn resolve_target(&self, origin: PaneId, area: Rect) -> PaneTarget {
        let cands = self.target_candidates(origin, area);
        match cands.len() {
            0 => PaneTarget::NeedsSplit,
            1 => PaneTarget::Direct(cands[0]),
            _ => PaneTarget::Pick(cands),
        }
    }

    /// Abre la carpeta `dir` en el panel `dest` (sin tocar el origen) y arranca su listado.
    /// Si `dir` no existe/ilegible, igual navega ahí: el panel mostrará el aviso "carpeta no
    /// encontrada" IN-PLACE (con sus opciones), en vez de un popup global.
    pub fn open_in_pane(&mut self, dest: PaneId, dir: PathBuf) {
        // Navegar cancela la vista profunda del panel (no es pegajosa).
        self.cancel_deep_if_navigating(dest);
        if let Some(f) = self.ws.pane_mut(dest).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dir.clone());
        }
        self.push_recent(dir.clone());
        self.start_listing(dest, dir);
    }

    /// Intercambia las carpetas del panel `a` y el panel `b` (swap ⇄). Ambos navegan
    /// (queda en sus historiales) y se re-listan. No-op si alguno no es Files.
    pub fn swap_panes(&mut self, a: PaneId, b: PaneId) {
        let dir_a = self
            .ws
            .pane(a)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone());
        let dir_b = self
            .ws
            .pane(b)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone());
        let (Some(dir_a), Some(dir_b)) = (dir_a, dir_b) else {
            return;
        };
        crate::logging::breadcrumb(&format!("intercambiar paneles {} ⇄ {}", a.0, b.0));
        // El swap cambia la carpeta de ambos paneles: cancelar el deep de cada uno.
        self.cancel_deep_if_navigating(a);
        self.cancel_deep_if_navigating(b);
        if let Some(f) = self.ws.pane_mut(a).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dir_b.clone());
        }
        if let Some(f) = self.ws.pane_mut(b).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dir_a.clone());
        }
        self.start_listing(a, dir_b);
        self.start_listing(b, dir_a);
    }

    /// Clona en `dest` la carpeta del panel `origin` (dest navega a donde está origin).
    pub fn clone_into(&mut self, origin: PaneId, dest: PaneId) {
        let Some(dir) = self
            .ws
            .pane(origin)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        else {
            return;
        };
        self.open_in_pane(dest, dir);
    }

    /// Crea un segundo panel Files (split del activo) y devuelve su id, para usarlo como
    /// destino cuando solo hay un panel. Mantiene el foco en el origen.
    pub fn split_for_target(&mut self) -> Option<PaneId> {
        let origin = self.ws.active_id()?;
        let dir = self
            .ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| PathBuf::from("C:/"));
        let new_id = self.ws.add_pane(PanePurpose::Files, dir.clone());
        self.apply_default_table(new_id);
        self.ws
            .layout
            .split_leaf(origin, SplitDir::Horizontal, new_id);
        self.start_listing(new_id, dir);
        // El foco se queda en el origen (estás explorando desde ahí).
        self.ws.set_active(origin);
        Some(new_id)
    }

    // --- Plantilla de tabla por defecto (C4) ---

    /// Si hay una plantilla de tabla por defecto configurada, la aplica al panel `id` recién
    /// creado (columnas visibles, orden y anchos). Si no, el panel conserva `TableState::default`.
    fn apply_default_table(&mut self, id: PaneId) {
        if let Some(tpl) = self.config.settings.default_table.clone() {
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.table = tpl;
            }
        }
    }

    /// Guarda el `TableState` del panel Files activo como plantilla por defecto para los paneles
    /// nuevos (los filtros NO se guardan: la plantilla es columnas/orden/ancho). Persiste.
    pub fn save_default_table_from_active(&mut self) {
        if let Some(f) = self.ws.active_files() {
            let mut table = f.table.clone();
            table.filters.clear(); // la plantilla no arrastra filtros del panel actual
            self.config.settings.default_table = Some(table);
            self.config.save();
        }
    }

    /// Limpia la plantilla de tabla por defecto (los paneles nuevos vuelven a `TableState::default`).
    pub fn clear_default_table(&mut self) {
        self.config.settings.default_table = None;
        self.config.save();
    }

    // --- Reglas de previsualización (C3) ---

    /// Reglas de preview actuales (espejo para la UI): (extensión, habilitada, tratar-como).
    pub fn preview_rules(&self) -> Vec<(String, bool, String)> {
        self.config
            .settings
            .preview_rules
            .iter()
            .map(|r| {
                (
                    r.ext.clone(),
                    r.enabled,
                    r.treat_as.clone().unwrap_or_default(),
                )
            })
            .collect()
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

    /// Fija el alias "tratar como" de la extensión `ext` (vacío = sin alias). Persiste.
    pub fn preview_rule_set_treat_as(&mut self, ext: &str, treat_as: &str) {
        if let Some(r) = self
            .config
            .settings
            .preview_rules
            .iter_mut()
            .find(|r| r.ext == ext)
        {
            let t = treat_as.trim().to_ascii_lowercase();
            r.treat_as = if t.is_empty() { None } else { Some(t) };
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
                treat_as: None,
            });
        self.config.save();
    }

    /// Punto de entrada de una acción multi-panel. Resuelve el destino: si es directo,
    /// actúa; si hay varios, deja un `pending_pick` para que la UI muestre el selector;
    /// si no hay otro panel, divide y usa el nuevo (para OpenDir/Clone; Swap necesita 2).
    /// Devuelve true si arrancó algún listado (para reactivar el timer).
    pub fn request_action(&mut self, action: PaneAction, origin: PaneId, area: Rect) -> bool {
        match self.resolve_target(origin, area) {
            PaneTarget::Direct(dest) => self.apply_action(action, origin, dest),
            PaneTarget::Pick(candidates) => {
                self.pending_pick = Some(PanePick {
                    action,
                    origin,
                    candidates,
                });
                false
            }
            PaneTarget::NeedsSplit => {
                if matches!(action, PaneAction::Swap | PaneAction::Stack) {
                    // Swap/apilar con un solo panel no tiene sentido: no-op.
                    return false;
                }
                if let Some(dest) = self.split_for_target() {
                    self.apply_action(action, origin, dest)
                } else {
                    false
                }
            }
        }
    }

    /// Aplica una acción ya resuelta a un destino concreto. Devuelve true si arrancó listado.
    fn apply_action(&mut self, action: PaneAction, origin: PaneId, dest: PaneId) -> bool {
        match action {
            PaneAction::OpenDir(dir) => {
                self.open_in_pane(dest, dir);
                true
            }
            PaneAction::Swap => {
                self.swap_panes(origin, dest);
                true
            }
            PaneAction::Clone => {
                self.clone_into(origin, dest);
                true
            }
            PaneAction::Stack => {
                self.stack_into(origin, dest);
                false
            }
            PaneAction::Transfer { move_files } => {
                self.transfer_to(origin, dest, move_files);
                false
            }
        }
    }

    /// Copia/mueve la selección del panel `origin` a la carpeta del panel `dest`. Lanza una op
    /// (deshacible) por el motor; el conflicto se resuelve por ítem si choca.
    fn transfer_to(&mut self, _origin: PaneId, dest: PaneId, move_files: bool) {
        let sources = self.selected_paths();
        if sources.is_empty() {
            return;
        }
        let Some(dest_dir) = self
            .ws
            .pane(dest)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        else {
            return;
        };
        let req = naygo_core::ops::transfer(move_files, sources, dest_dir);
        let label = if move_files { "Mover" } else { "Copiar" };
        self.ops.start_op(req, label.to_string(), true);
    }

    /// Copiar/mover la selección al OTRO panel (F-keys estilo Commander). Resuelve el destino
    /// con el selector si hay varios; divide si no hay otro panel. Usa `last_area` (la última
    /// área conocida del contenido) porque el atajo de teclado no la trae.
    pub fn op_to_other(&mut self, move_files: bool) -> bool {
        let accion = if move_files { "mover" } else { "copiar" };
        crate::logging::breadcrumb(&format!("{} ítems al otro panel", accion));
        let Some(origin) = self.active_files_id() else {
            return false;
        };
        self.request_action(PaneAction::Transfer { move_files }, origin, self.last_area)
    }

    /// Apila el panel `origin` como pestaña sobre el grupo/hoja de `dest` (los agrupa). El
    /// origen pasa a compartir el rect del destino y queda como pestaña activa.
    pub fn stack_into(&mut self, origin: PaneId, dest: PaneId) {
        if origin == dest {
            return;
        }
        // Sacar el origen de su posición actual en el layout y apilarlo sobre el destino.
        self.ws.layout.remove_leaf(origin);
        self.ws.layout.stack_onto(dest, origin);
        self.ws.set_active(origin);
    }

    /// Cambia la pestaña activa de un grupo al miembro `member` y lo deja activo.
    pub fn set_active_tab(&mut self, member: PaneId) {
        self.ws.layout.set_active_tab(member);
        self.ws.set_active(member);
    }

    /// Cierra la pestaña `member`: la quita del layout y del workspace. Si era la única del
    /// grupo, el grupo desaparece (su rect lo absorbe el hermano del split).
    pub fn close_tab(&mut self, member: PaneId) {
        self.ws.layout.remove_leaf(member);
        self.ws.remove_pane(member);
        self.listings.remove(&member);
        self.trees.remove(&member);
    }

    /// Los grupos de pestañas actuales: (miembros, índice activo). Para que la UI pinte las
    /// barras de pestañas y sepa cuál panel mostrar de cada grupo.
    pub fn tab_groups(&self) -> Vec<(Vec<PaneId>, usize)> {
        self.ws.layout.tab_groups()
    }

    /// El rect de la zona de drop bajo el punto `(px, py)` (para resaltarla durante el
    /// arrastre) y si esa zona es el CENTRO (apilar como pestaña, que la UI pinta distinto).
    /// `None` si el punto no cae sobre un panel o sobre el propio arrastrado.
    pub fn drop_preview(
        &self,
        dragged: PaneId,
        px: f32,
        py: f32,
        area: Rect,
    ) -> Option<(Rect, bool)> {
        use naygo_core::workspace::layout::{drop_hit, drop_zones, DropZone};
        let panes = self.pane_rects(area);
        let (target, zone) = drop_hit(&panes, px, py)?;
        if target == dragged {
            return None;
        }
        let target_rect = panes
            .iter()
            .find(|(id, _)| *id == target)
            .map(|(_, r)| *r)?;
        let is_center = zone == DropZone::Center;
        drop_zones(target_rect)
            .into_iter()
            .find(|(z, _)| *z == zone)
            .map(|(_, r)| (r, is_center))
    }

    /// Reacomoda por arrastre: suelta el panel `dragged` en el punto `(px, py)` del área
    /// `area`. Según la zona del panel destino: centro → apila como pestaña; borde →
    /// divide (el arrastrado queda en ese lado). No-op si se suelta sobre sí mismo o fuera
    /// de todo panel. Devuelve true si reacomodó.
    pub fn perform_drop(&mut self, dragged: PaneId, px: f32, py: f32, area: Rect) -> bool {
        use naygo_core::workspace::layout::{drop_hit, DropZone};
        let panes = self.pane_rects(area);
        let Some((target, zone)) = drop_hit(&panes, px, py) else {
            return false;
        };
        if target == dragged {
            return false;
        }
        match zone {
            DropZone::Center => {
                self.stack_into(dragged, target);
            }
            DropZone::Left | DropZone::Right | DropZone::Top | DropZone::Bottom => {
                let dir = match zone {
                    DropZone::Left | DropZone::Right => SplitDir::Horizontal,
                    _ => SplitDir::Vertical,
                };
                // Sacar el arrastrado de su lugar y dividir el destino con él.
                self.ws.layout.remove_leaf(dragged);
                self.ws.layout.split_leaf(target, dir, dragged);
                // Para Left/Top el arrastrado debe quedar PRIMERO: split_leaf lo pone
                // segundo, así que para esos casos intercambiamos las fracciones via swap
                // del orden (el split nuevo arranca 50/50, simétrico, así que basta con
                // dejar al arrastrado del lado correcto: reordenamos si es Left/Top).
                if matches!(zone, DropZone::Left | DropZone::Top) {
                    self.ws.layout.swap_split_children(target, dragged);
                }
                self.ws.set_active(dragged);
            }
        }
        true
    }

    /// El usuario eligió el panel número `n` (1..9) del selector. Aplica la acción y cierra
    /// el selector. Devuelve true si arrancó listado. No-op si `n` está fuera de rango.
    pub fn pick_resolve(&mut self, n: usize) -> bool {
        let Some(pick) = self.pending_pick.take() else {
            return false;
        };
        let Some(&dest) = pick.candidates.get(n.wrapping_sub(1)) else {
            // Índice inválido: cancela el selector sin actuar.
            return false;
        };
        self.apply_action(pick.action, pick.origin, dest)
    }

    /// Cancela el selector de panel (Esc).
    pub fn pick_cancel(&mut self) {
        self.pending_pick = None;
    }

    // --- Lectura para los paneles especiales (consumen los bridges puros) ---

    /// Info del inspector para el panel `id` (lee el Files ACTIVO, no el `id`: el
    /// inspector refleja el panel de archivos activo, sea cual sea su posición).
    pub fn inspector_info(&self) -> InspectorInfo {
        inspector_info(
            self.ws.active_files(),
            self.config.settings.date_format,
            naygo_platform::time::local_utc_offset_secs(),
        )
    }

    /// Filas de favoritos (orden de usuario). Llevan el ícono de carpeta del set activo.
    pub fn favorite_rows(&mut self) -> Vec<NavRow> {
        let folder = self.icons.get(naygo_core::icon_kind::IconKey::Folder);
        favorite_rows(&self.favorites, &folder)
    }

    /// Filas de recientes (más nueva primero). Llevan el ícono de carpeta del set activo.
    pub fn recent_rows(&mut self) -> Vec<NavRow> {
        let folder = self.icons.get(naygo_core::icon_kind::IconKey::Folder);
        recent_rows(&self.recents, &folder)
    }

    /// Tira de unidades de disco para la toolbar (paridad con egui): una entrada por unidad,
    /// etiqueta = letra (p. ej. "C:"), ícono de disco cacheado, ruta = raíz. Clic navega el
    /// panel Files activo a la raíz. Se reconsulta al cambiar los dispositivos (USB).
    pub fn drive_strip(&mut self) -> Vec<NavRow> {
        let drives = naygo_platform::drives::drives();
        drives
            .into_iter()
            .map(|d| {
                let icon = self
                    .icons
                    .get(naygo_core::icon_kind::IconKey::Drive(d.kind));
                NavRow {
                    // Letra compacta de la unidad, sin la barra final ("C:\\" → "C:").
                    label: d.label.trim_end_matches(['\\', '/']).to_string(),
                    path: d.path.display().to_string(),
                    icon,
                    // Marca las extraíbles (USB) para ofrecer la expulsión segura.
                    removable: d.kind == naygo_core::icon_kind::DriveKind::Removable,
                }
            })
            .collect()
    }

    /// ¿La unidad cuya raíz es `root` es extraíble (USB)? Se usa para mostrar el
    /// botón/menú de expulsión solo en unidades extraíbles. Consulta el tipo de
    /// la unidad en caliente (barato: una llamada a GetDriveTypeW por unidad).
    pub fn is_removable(&self, root: &Path) -> bool {
        naygo_platform::drives::drives()
            .into_iter()
            .any(|d| d.path == root && d.kind == naygo_core::icon_kind::DriveKind::Removable)
    }

    /// Expulsa de forma segura la unidad extraíble cuya raíz es `root`. Devuelve
    /// `Ok(())` si se desmontó y expulsó sin forzar. Si hay archivos abiertos
    /// (volumen bloqueado) NO fuerza: devuelve [`EjectOutcome::InUse`]. La UI
    /// mapea el resultado a un toast localizado.
    pub fn eject_drive(&self, root: PathBuf) -> EjectOutcome {
        crate::logging::breadcrumb(&format!("expulsar USB {}", root.display()));
        // Guardia de seguridad: solo se expulsan unidades extraíbles. Nunca se debe
        // intentar desmontar una fija/de red aunque la UI lo pidiera por error.
        if !self.is_removable(&root) {
            let msg = format!("expulsar USB {}: no es extraíble", root.display());
            crate::logging::log_line(&msg);
            return EjectOutcome::Failed("not a removable drive".into());
        }
        match naygo_platform::eject::eject_drive(&root) {
            Ok(()) => EjectOutcome::Ok,
            Err(naygo_platform::eject::EjectError::InUse) => {
                crate::logging::log_line(&format!("expulsar USB {}: en uso", root.display()));
                EjectOutcome::InUse
            }
            Err(e) => {
                let msg = format!("expulsar USB {}: fallo — {}", root.display(), e);
                crate::logging::log_line(&msg);
                EjectOutcome::Failed(e.to_string())
            }
        }
    }

    /// Filas del historial de deshacer (validadas contra el disco).
    pub fn history_rows(&self) -> Vec<HistRow> {
        history_rows(&self.ops.undo_history)
    }

    /// Filas del árbol del panel `id` (aplanadas con sangría). Vacío si el panel no tiene
    /// árbol (no es un Tree). El uso de disco se consulta por unidad.
    pub fn tree_rows(&mut self, id: PaneId) -> Vec<TreeRow> {
        // Préstamos disjuntos: `trees` (lectura del árbol) e `icons` (mutable para cachear).
        let WorkspaceCtrl { trees, icons, .. } = self;
        match trees.get(&id) {
            Some(t) => tree_rows(t, &disk_usage, &mut |key| icons.get(key)),
            None => Vec::new(),
        }
    }

    // --- Acciones de los paneles especiales ---

    /// Navega el panel Files activo a `dir` (desde favoritos/recientes/árbol) y arranca su
    /// listado. Registra la visita en recientes. Devuelve true si navegó.
    pub fn navigate_active_to(&mut self, dir: PathBuf) -> bool {
        let Some(active_files_id) = self.active_files_id() else {
            return false;
        };
        crate::logging::breadcrumb(&format!(
            "navegar panel {} → {}",
            active_files_id.0,
            dir.display()
        ));
        // Navegar cancela la vista profunda del panel (no es pegajosa).
        self.cancel_deep_if_navigating(active_files_id);
        // Si la carpeta no existe/ilegible (favorito viejo, red caída, ruta tipeada), igual
        // navegamos: el panel mostrará el aviso "carpeta no encontrada" IN-PLACE con sus
        // opciones (reintentar/subir/elegir/cerrar), en vez de un popup global. No la metemos
        // en recientes si no es navegable (evita ensuciar la lista con rutas muertas).
        let navigable = Self::dir_is_navigable(&dir);
        if let Some(f) = self
            .ws
            .pane_mut(active_files_id)
            .and_then(|p| p.files.as_mut())
        {
            f.navigate_to(dir.clone());
        }
        if navigable {
            self.push_recent(dir.clone());
        }
        self.start_listing(active_files_id, dir.clone());
        self.sync_trees_active(dir);
        true
    }

    /// Quita un favorito por ruta.
    pub fn remove_favorite(&mut self, path: &Path) {
        self.favorites.remove(path);
    }

    /// Alterna el favorito de la carpeta del panel Files activo (estrella).
    pub fn toggle_favorite_active(&mut self) {
        if let Some(dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
            self.favorites.toggle(&dir);
        }
    }

    /// ¿La carpeta del panel `id` está en favoritos? (para pintar la estrella llena/vacía).
    pub fn is_pane_dir_favorite(&self, id: PaneId) -> bool {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| self.favorites.contains(&f.current_dir))
            .unwrap_or(false)
    }

    /// Alterna el favorito de la carpeta del panel `id` (botón ★ de la path-bar de ese panel).
    pub fn toggle_favorite_dir(&mut self, id: PaneId) {
        if let Some(dir) = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        {
            self.favorites.toggle(&dir);
        }
    }

    /// Copia la ruta de la carpeta del panel `id` al portapapeles (botón 📋).
    pub fn copy_pane_path(&self, id: PaneId) {
        if let Some(dir) = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        {
            let _ = naygo_platform::clipboard::write_text(&dir.display().to_string());
        }
    }

    /// El id del panel Files activo (o el primer Files), para dirigir navegaciones desde
    /// paneles auxiliares (un Tree/Favoritos activo no es un Files).
    fn active_files_id(&self) -> Option<PaneId> {
        let active = self.ws.active_id();
        if let Some(a) = active {
            if self.ws.pane(a).map(|p| p.purpose) == Some(PanePurpose::Files) {
                return Some(a);
            }
        }
        // El activo no es un panel Files (p. ej. el Árbol): usar el ÚLTIMO Files activo (el que
        // el usuario venía usando), si todavía existe; si no, el primer Files que haya.
        if let Some(last) = self.last_active_files {
            if self.ws.pane(last).map(|p| p.purpose) == Some(PanePurpose::Files) {
                return Some(last);
            }
        }
        self.ws.files_panes().first().copied()
    }

    /// Carpeta del panel Files activo (destino de pegar/nuevo). None si no hay Files.
    pub fn active_dir(&self) -> Option<PathBuf> {
        self.ws.active_files().map(|f| f.current_dir.clone())
    }

    /// Rutas reales de los ítems SELECCIONADOS del panel Files activo (o, si no hay
    /// selección, el ítem enfocado). Vacío si no hay nada. Para las operaciones de archivo.
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        let Some(f) = self.ws.active_files() else {
            return Vec::new();
        };
        let view = f.view_indices();
        let mut out: Vec<PathBuf> = f
            .selected
            .iter()
            .filter_map(|&pos| view.get(pos).and_then(|&real| f.entries.get(real)))
            .map(|e| e.path.clone())
            .collect();
        if out.is_empty() {
            if let Some(e) = f.focused_view_entry() {
                out.push(e.path.clone());
            }
        }
        out
    }

    // --- Gestos de operaciones de archivo (delegan en OpsCtrl) ---

    /// Copiar la selección al portapapeles (limpia el corte).
    pub fn op_copy(&mut self) {
        let paths = self.selected_paths();
        if !paths.is_empty() {
            self.ops.set_copy(&paths);
        }
    }

    /// Cortar la selección (marca corte visual).
    pub fn op_cut(&mut self) {
        let paths = self.selected_paths();
        if !paths.is_empty() {
            self.ops.set_cut(&paths);
        }
    }

    /// Pegar en la carpeta activa los archivos del portapapeles. Devuelve true si arrancó
    /// una operación (para reactivar el timer). El pegado de texto/imagen se cablea con el
    /// modal PastePreview (fase de diálogos); aquí solo archivos.
    /// Pega el contenido del portapapeles en la carpeta activa. Reusa `core::clipboard::
    /// decide_paste`, que decide según el contenido + Settings: archivos → transferencia;
    /// TEXTO → crea un .txt (nombre/extensión configurables); IMAGEN → crea un .png/.jpg
    /// (nombre/formato configurables). Antes solo manejaba archivos (el texto/imagen no hacían
    /// nada). Por ahora escribe directo (sin el modal de confirmación de nombre de egui).
    pub fn op_paste(&mut self) -> bool {
        let Some(dir) = self.active_dir() else {
            return false;
        };
        let content = naygo_platform::clipboard::read();
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let exists = |p: &std::path::Path| p.exists();
        let plan = naygo_core::clipboard::decide_paste(
            &content,
            &dir,
            &self.config.settings,
            now_secs,
            &exists,
        );
        use naygo_core::clipboard::PastePlan;
        match plan {
            PastePlan::Transfer { paths, cut } => {
                if paths.is_empty() {
                    return false;
                }
                let label = if cut { "Mover" } else { "Copiar" };
                let req = naygo_core::ops::transfer(cut, paths, dir);
                self.ops.start_op(req, label.to_string(), true);
                self.ops.clear_cut();
                true
            }
            PastePlan::CreateText { path, body } => {
                self.paste_write_or_confirm(&dir, &path, body.into_bytes())
            }
            PastePlan::CreateImage { path, fmt, img } => {
                match naygo_core::clipboard::encode::encode_image(
                    &img,
                    fmt,
                    self.config.settings.paste_jpg_quality,
                ) {
                    Ok(bytes) => self.paste_write_or_confirm(&dir, &path, bytes),
                    Err(_) => false,
                }
            }
            PastePlan::Nothing => false,
        }
    }

    /// Escribe el archivo pegado en `path`, o —si `Settings.paste_confirm` está activo— abre el
    /// modal de confirmación de nombre (NameInput con purpose Paste) con el nombre propuesto
    /// editable; al confirmar, `name_confirm` escribe los `bytes` con el nombre elegido.
    fn paste_write_or_confirm(
        &mut self,
        dir: &std::path::Path,
        path: &std::path::Path,
        bytes: Vec<u8>,
    ) -> bool {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if self.config.settings.paste_confirm {
            self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::NameInput {
                purpose: crate::ops_ctrl::NamePurpose::Paste { ext, bytes },
                dir: dir.to_path_buf(),
                buf: stem,
            });
            true
        } else {
            std::fs::write(path, &bytes).is_ok()
        }
    }

    /// Recibe rutas externas soltadas (drag&drop OLE, Fase 5D) sobre el panel `dest`: copia
    /// (o mueve si `move_`) a su carpeta, reusando el engine de operaciones de F3 (con sus
    /// diálogos de conflicto, panel de progreso y cancelación). No-op si el panel no es Files o
    /// no hay rutas. Devuelve true si arrancó la operación.
    pub fn drop_external(
        &mut self,
        dest: PaneId,
        sources: Vec<std::path::PathBuf>,
        move_: bool,
    ) -> bool {
        if sources.is_empty() {
            return false;
        }
        let Some(dir) = self
            .ws
            .pane(dest)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        else {
            return false;
        };
        let label = if move_ { "Mover" } else { "Copiar" };
        let req = naygo_core::ops::transfer(move_, sources, dir);
        self.ops.start_op(req, label.to_string(), true);
        true
    }

    /// Eliminar la selección: abre el modal de confirmación.
    pub fn op_delete(&mut self, permanent: bool) {
        {
            let n = self.selected_paths().len();
            let modo = if permanent { "permanente" } else { "papelera" };
            crate::logging::breadcrumb(&format!("eliminar {} ítem(s) ({})", n, modo));
        }
        let paths = self.selected_paths();
        if !paths.is_empty() {
            self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
                sources: paths,
                permanent,
            });
        }
    }

    /// Nuevo archivo/carpeta en la carpeta activa: abre el modal de nombre.
    pub fn op_new(&mut self, is_dir: bool) {
        let Some(dir) = self.active_dir() else {
            return;
        };
        let purpose = if is_dir {
            crate::ops_ctrl::NamePurpose::NewDir
        } else {
            crate::ops_ctrl::NamePurpose::NewFile
        };
        self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::NameInput {
            purpose,
            dir,
            buf: String::new(),
        });
    }

    /// Renombrar el ítem enfocado: abre el modal de nombre con el nombre actual.
    /// Rename inline (F2 / menú): pide a la UI abrir el editor en la celda Name de la fila
    /// enfocada del panel Files activo, con la etapa 0 del ciclo (nombre sin extensión). En vez
    /// del modal, marca `rename_requested`; la UI lo consume con `take_rename_request`. (6D)
    pub fn op_rename(&mut self) {
        let Some(id) = self.active_files_id() else {
            return;
        };
        let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) else {
            return;
        };
        let Some(pos) = f.focused else {
            return;
        };
        self.rename_requested = Some((id, pos, 0));
    }

    /// La UI consume el pedido de rename inline (pane, posición de vista, etapa del ciclo F2).
    pub fn take_rename_request(&mut self) -> Option<(PaneId, usize, u8)> {
        self.rename_requested.take()
    }

    /// Nombre actual de la fila en la posición de vista `pos` del panel `id` (para precargar el
    /// editor inline). Vacío si no existe.
    pub fn rename_name_at(&self, id: PaneId, pos: usize) -> String {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.view_entry_at(pos))
            .map(|e| e.name.clone())
            .unwrap_or_default()
    }

    /// Confirma el rename inline de la fila `pos` del panel `id` al nombre `new_name`. Arma la
    /// op de rename (reusa el engine de F3, con su validación) y la lanza. Devuelve true si
    /// arrancó algo (nombre válido y distinto del actual).
    pub fn rename_commit(&mut self, id: PaneId, pos: usize, new_name: &str) -> bool {
        let new_name = new_name.trim();
        let Some(f) = self.ws.pane(id).and_then(|p| p.files.as_ref()) else {
            return false;
        };
        let Some(e) = f.view_entry_at(pos) else {
            return false;
        };
        // Sin cambio o nombre inválido → no hacer nada (evita una op vacía o un error del engine).
        if new_name.is_empty()
            || new_name == e.name
            || !naygo_core::ops::names::is_valid_name(new_name)
        {
            return false;
        }
        crate::logging::breadcrumb("renombrar");
        let source = e.path.clone();
        let req = naygo_core::ops::rename(source, new_name.to_string());
        self.ops.start_op(req, "Renombrar".to_string(), true);
        true
    }

    /// Rename EN CADENA: confirma el rename actual y pide abrir el editor en la fila anterior
    /// (`dir < 0`) o siguiente (`dir > 0`), seleccionando el nombre sin extensión (etapa 0,
    /// decisión de Nicolás). Devuelve la nueva posición si la hay (clamp a la vista). (6D)
    pub fn rename_chain(
        &mut self,
        id: PaneId,
        pos: usize,
        new_name: &str,
        dir: i32,
    ) -> Option<usize> {
        self.rename_commit(id, pos, new_name);
        let count = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.view_len())?;
        if count == 0 {
            return None;
        }
        let next = (pos as i32 + dir).clamp(0, count as i32 - 1) as usize;
        // Mover el foco/selección a la fila nueva, para que el scroll la acompañe.
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.select_single(next);
        }
        self.rename_requested = Some((id, next, 0));
        Some(next)
    }

    /// Deshace la entrada del historial con `id` (botón "Deshacer" del panel Historial).
    /// Valida, re-emite el inverso y la marca deshecha. Devuelve true si arrancó algo.
    pub fn undo_entry(&mut self, id: u64) -> bool {
        let Some(idx) = self.ops.undo_history.iter().position(|e| e.id == id) else {
            return false;
        };
        if self.ops.undo_history[idx].undone
            || naygo_core::ops::undo::validate(&self.ops.undo_history[idx].actions).is_err()
        {
            return false;
        }
        let reqs = naygo_core::ops::undo::to_requests(&self.ops.undo_history[idx].actions);
        self.ops.undo_history[idx].undone = true;
        for req in reqs {
            self.ops.start_op(req, "Deshacer".to_string(), false);
        }
        true
    }

    /// Deshace la última entrada deshacible del historial. Devuelve true si arrancó algo.
    pub fn op_undo_last(&mut self) -> bool {
        // Buscar la última entrada no-deshecha y deshacible.
        let idx = self
            .ops
            .undo_history
            .iter()
            .rposition(|e| !e.undone && naygo_core::ops::undo::validate(&e.actions).is_ok());
        let Some(idx) = idx else {
            return false;
        };
        let reqs = naygo_core::ops::undo::to_requests(&self.ops.undo_history[idx].actions);
        self.ops.undo_history[idx].undone = true;
        for req in reqs {
            self.ops.start_op(req, "Deshacer".to_string(), false);
        }
        true
    }

    // --- Menú contextual (clic derecho) ---

    /// Abre el menú contextual en (x,y) sobre la fila `pos` del panel `id`. Si la fila no
    /// estaba seleccionada, la selecciona (Explorer hace lo mismo). El objetivo del menú es
    /// la selección actual.
    pub fn open_context_menu(&mut self, x: f32, y: f32) {
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
        let mut created_any = false;
        for spec in specs {
            if let naygo_core::ops::FolderSpec::Valid(rel) = spec {
                let req = naygo_core::ops::create(dir.clone(), rel, true);
                self.ops.start_op(req, "Nueva carpeta".to_string(), true);
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
    fn terminal_dir(&self) -> Option<PathBuf> {
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

    /// Resalta `dir` en todos los árboles (cuando cambia la carpeta del Files activo) y arranca
    /// el REVEAL: cada árbol expandirá progresivamente los ancestros hasta `dir`.
    fn sync_trees_active(&mut self, dir: PathBuf) {
        let ids: Vec<PaneId> = self.trees.keys().copied().collect();
        for id in &ids {
            if let Some(t) = self.trees.get_mut(id) {
                t.set_active(dir.clone());
            }
            self.reveal_targets.insert(*id, dir.clone());
        }
        self.pump_reveal();
    }

    /// Avanza el "reveal" de cada árbol: expande el ancestro más profundo del destino que ya
    /// exista como nodo y aún no esté expandido (los hijos de un nodo recién expandido aparecen
    /// cuando su worker termina, y el siguiente tick vuelve a llamar aquí). Cuando el padre del
    /// destino queda expandido (o el destino es una raíz), se limpia el target. Idempotente.
    pub fn pump_reveal(&mut self) {
        let targets: Vec<(PaneId, PathBuf)> = self
            .reveal_targets
            .iter()
            .map(|(id, p)| (*id, p.clone()))
            .collect();
        for (id, target) in targets {
            let Some(tree) = self.trees.get(&id) else {
                self.reveal_targets.remove(&id);
                continue;
            };
            // Cadena de ancestros root..parent(target). Si está vacía, el destino es una raíz
            // (o fuera del árbol): no hay nada que expandir.
            let chain = tree.reveal_chain(&target);
            if chain.is_empty() {
                self.reveal_targets.remove(&id);
                continue;
            }
            // ¿Ya están todos los ancestros expandidos? Entonces el destino ya es visible: listo.
            let all_expanded = chain.iter().all(|p| {
                tree.node_at(p)
                    .map(|n| n.expanded && n.children.is_some())
                    .unwrap_or(false)
            });
            if all_expanded {
                self.reveal_targets.remove(&id);
                continue;
            }
            // Expandir el PRIMER ancestro (de la raíz hacia abajo) que exista pero no esté
            // expandido/cargado. Los más profundos aún no existen como nodo hasta que su padre
            // cargue; el próximo tick los alcanzará.
            let next = chain.iter().find(|p| {
                tree.node_at(p)
                    .map(|n| !n.expanded || n.children.is_none())
                    .unwrap_or(false)
            });
            if let Some(p) = next.cloned() {
                self.tree_expand(id, p);
            } else {
                // Ningún ancestro pendiente existe aún (esperando que cargue un padre): no-op.
            }
        }
    }

    // --- Árbol: expandir / colapsar / drenar workers ---

    /// Expande la rama `path` del árbol del panel `id`: si ya tiene hijos, solo reabre
    /// (sin re-listar); si no, marca Loading y arranca un worker solo-directorios.
    pub fn tree_expand(&mut self, id: PaneId, path: PathBuf) {
        if self.tree_listings.contains_key(&(id, path.clone())) {
            return;
        }
        let already_loaded = self
            .trees
            .get(&id)
            .and_then(|t| t.node_at(&path))
            .map(|n| n.children.is_some())
            .unwrap_or(false);
        if let Some(t) = self.trees.get_mut(&id) {
            if already_loaded {
                t.expand_loaded(&path);
                return;
            }
            t.begin_loading(&path);
        }
        self.tree_listings
            .insert((id, path.clone()), Listing::start_dirs_only(path));
    }

    /// Colapsa la rama `path` del panel `id`: cancela su worker si está en vuelo.
    pub fn tree_collapse(&mut self, id: PaneId, path: PathBuf) {
        if let Some(l) = self.tree_listings.remove(&(id, path.clone())) {
            l.cancel();
        }
        if let Some(t) = self.trees.get_mut(&id) {
            t.collapse(&path);
        }
    }

    /// Alterna expand/collapse de una rama del árbol del panel `id`.
    pub fn tree_toggle(&mut self, id: PaneId, path: PathBuf) {
        let expanded = self
            .trees
            .get(&id)
            .and_then(|t| t.node_at(&path))
            .map(|n| n.expanded)
            .unwrap_or(false);
        if expanded {
            self.tree_collapse(id, path);
        } else {
            self.tree_expand(id, path);
        }
    }

    /// El cursor de teclado del árbol `id` (para resaltar la fila enfocada). Si no hay cursor
    /// fijado, cae a la carpeta activa del árbol, o a su primera raíz.
    pub fn tree_cursor_of(&self, id: PaneId) -> Option<PathBuf> {
        if let Some(p) = self.tree_cursor.get(&id) {
            return Some(p.clone());
        }
        let t = self.trees.get(&id)?;
        t.active_path
            .clone()
            .or_else(|| t.roots.first().map(|r| r.path.clone()))
    }

    /// Navegación por teclado dentro del árbol `id`. `key`: "up"/"down"/"left"/"right"/"enter".
    /// ↑/↓ mueven el cursor por las filas visibles; → expande (o baja al primer hijo si ya está
    /// expandido); ← colapsa (o sube al padre si ya está colapsado); Enter navega el panel Files
    /// activo a la carpeta del cursor. Devuelve true si navegó (para reactivar el timer).
    pub fn tree_key(&mut self, id: PaneId, key: &str) -> bool {
        // Asegurar que el panel del árbol quede activo (para que las teclas le lleguen).
        self.set_active(id);
        let Some(cursor) = self.tree_cursor_of(id) else {
            return false;
        };
        let Some(t) = self.trees.get(&id) else {
            return false;
        };
        let flat = t.flat_paths();
        let pos = flat.iter().position(|p| p == &cursor).unwrap_or(0);
        match key {
            "up" | "down" => {
                let next = if key == "down" {
                    (pos + 1).min(flat.len().saturating_sub(1))
                } else {
                    pos.saturating_sub(1)
                };
                if let Some(p) = flat.get(next).cloned() {
                    self.set_tree_cursor(id, p);
                }
                false
            }
            "right" => {
                let node_expanded = t.node_at(&cursor).map(|n| n.expanded).unwrap_or(false);
                let has_children = t
                    .node_at(&cursor)
                    .map(|n| {
                        !matches!(
                            n.state,
                            naygo_core::tree::NodeState::Empty | naygo_core::tree::NodeState::Error
                        )
                    })
                    .unwrap_or(false);
                if !node_expanded && has_children {
                    self.tree_expand(id, cursor); // expandir
                } else if node_expanded {
                    // Ya expandido: bajar al primer hijo (siguiente fila del flat).
                    if let Some(p) = flat.get(pos + 1).cloned() {
                        self.set_tree_cursor(id, p);
                    }
                }
                false
            }
            "left" => {
                let node_expanded = t.node_at(&cursor).map(|n| n.expanded).unwrap_or(false);
                if node_expanded {
                    self.tree_collapse(id, cursor); // colapsar
                } else if let Some(parent) = t.parent_of(&cursor) {
                    self.set_tree_cursor(id, parent); // subir al padre
                }
                false
            }
            "enter" => self.navigate_active_to(cursor),
            _ => false,
        }
    }

    /// Fija el cursor del árbol y resalta esa fila (sin navegar el panel Files).
    fn set_tree_cursor(&mut self, id: PaneId, path: PathBuf) {
        self.tree_cursor.insert(id, path.clone());
        if let Some(t) = self.trees.get_mut(&id) {
            // Resaltar la fila del cursor (reusa `active_path` como pista visual) y revelarla.
            t.set_active(path);
        }
    }

    /// Drena los workers de árbol en vuelo (asocia hijos por path). Devuelve true si NO
    /// queda ninguno (para que el timer pueda apagarse). Quita los terminados.
    pub fn pump_tree(&mut self) -> bool {
        let keys: Vec<(PaneId, PathBuf)> = self.tree_listings.keys().cloned().collect();
        for key in keys {
            let (batch, done) = match self.tree_listings.get(&key) {
                Some(l) => l.poll(),
                None => continue,
            };
            let (id, parent) = (key.0, &key.1);
            if let Some(t) = self.trees.get_mut(&id) {
                for e in batch {
                    t.push_child(parent, e.path);
                }
                if done {
                    t.finish_loading(parent, naygo_core::tree::NodeOutcome::Done);
                }
            }
            if done {
                self.tree_listings.remove(&key);
            }
        }
        // Tras drenar, avanzar el reveal: una rama recién cargada habilita expandir la siguiente
        // hacia la carpeta objetivo.
        if !self.reveal_targets.is_empty() {
            self.pump_reveal();
        }
        // No dejar dormir el timer mientras haya workers en vuelo O un reveal pendiente (sus
        // ramas se cargan en ticks sucesivos).
        self.tree_listings.is_empty() && self.reveal_targets.is_empty()
    }

    /// ¿Hay algún panel de un `purpose` dado en el workspace?
    pub fn has_purpose(&self, purpose: PanePurpose) -> bool {
        self.ws.panes().iter().any(|p| p.purpose == purpose)
    }

    /// Actualiza el objetivo del preview con el archivo enfocado del Files activo y arranca
    /// el worker si corresponde (debounce vencido). No-op si no hay panel Preview. Devuelve
    /// true si queda trabajo pendiente (para mantener el timer vivo).
    pub fn drive_preview(&mut self, now: std::time::Instant) -> bool {
        if !self.has_purpose(PanePurpose::Preview) {
            // Si se quitó el panel Preview, soltar cualquier worker en vuelo.
            self.preview.set_wanted(None, now);
            return false;
        }
        let focused_file = self
            .ws
            .active_files()
            .and_then(|f| f.focused_view_entry())
            .filter(|e| e.kind != EntryKind::Directory)
            .map(|e| e.path.clone());
        self.preview.set_wanted(focused_file, now);
        if self.preview.should_start(now) {
            self.preview.start();
        }
        self.preview.busy()
    }

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

    /// Refresca (re-lista) la carpeta del panel activo — estilo navegador (F5). No toca el
    /// historial; solo vuelve a leer el disco. Devuelve true si relanzó un listado.
    pub fn refresh_active(&mut self) -> bool {
        let Some(active) = self.active_files_id() else {
            return false;
        };
        let Some(dir) = self
            .ws
            .pane(active)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        else {
            return false;
        };
        // F5 re-lista en el mismo sitio: cancela el deep para no mezclar filas.
        self.cancel_deep_if_navigating(active);
        self.start_listing(active, dir);
        true
    }

    /// Cancela el listado en curso del panel activo (Esc). Lo deja con lo que alcanzó a listar.
    pub fn cancel_active_listing(&mut self) {
        if let Some(active) = self.active_files_id() {
            if let Some(l) = self.listings.get(&active) {
                l.cancel();
            }
        }
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
    fn size_status(&self) -> Option<String> {
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

    /// Ordena el panel activo por la columna `kind_int` (0..4). Reusa `sort_key_of` para cubrir
    /// las 5 columnas (incluida Creado), a diferencia del `on_sort_by` por string. Alterna
    /// ascendente/descendente si ya estaba ordenado por esa clave.
    pub fn sort_by_kind(&mut self, kind_int: i32) {
        let key = naygo_core::columns::sort_key_of(crate::bridge::column_kind_from_int(kind_int));
        if let Some(f) = self.ws.active_files_mut() {
            if f.sort.key == key {
                f.sort.ascending = !f.sort.ascending;
            } else {
                f.sort.key = key;
                f.sort.ascending = true;
            }
            let spec = f.sort;
            naygo_core::sort::sort_entries(&mut f.entries, &spec);
        }
    }

    pub fn on_sort_by(&mut self, column: &str) {
        let key = match column {
            "name" => SortKey::Name,
            "ext" => SortKey::Extension,
            "size" => SortKey::Size,
            "modified" => SortKey::Modified,
            _ => return,
        };
        if let Some(f) = self.ws.active_files_mut() {
            if f.sort.key == key {
                f.sort.ascending = !f.sort.ascending;
            } else {
                f.sort.key = key;
                f.sort.ascending = true;
            }
            let spec = f.sort;
            naygo_core::sort::sort_entries(&mut f.entries, &spec);
        }
    }

    /// Tecla sobre el panel activo (reusa el keymap). Devuelve true si navegó.
    pub fn on_key(&mut self, text: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.ctrl_down = ctrl;
        self.shift_down = shift;
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
            _ => {}
        }
        false
    }

    /// Navega el panel Files activo al favorito en el índice `idx` (Ctrl+1..9). No-op si no
    /// hay tantos favoritos. Devuelve true si navegó.
    pub fn go_favorite(&mut self, idx: usize) -> bool {
        let Some(fav) = self.favorites.list().get(idx) else {
            return false;
        };
        let path = fav.path.clone();
        self.navigate_active_to(path)
    }

    /// Aplica `op` al panel activo (helper para no repetir el match de préstamos).
    fn with_active(&mut self, op: impl FnOnce(&mut FilePaneState)) {
        if let Some(f) = self.ws.active_files_mut() {
            op(f);
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
                    if e.name.to_lowercase().starts_with(needle.as_str()) {
                        f.select_single(pos);
                        break;
                    }
                }
            }
        }
    }

    // --- Vista profunda (DeepJob) ---

    /// Cancela la vista profunda del panel `id` si está activa. Llamar en CUALQUIER camino
    /// que cambie la carpeta de un panel Files (la vista profunda no es pegajosa). No relista:
    /// el camino que navega ya repuebla el panel con su listado normal.
    fn cancel_deep_if_navigating(&mut self, id: PaneId) {
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

    /// Construye el snapshot de diagnóstico (rutas por panel + tema + idioma). Barato (strings).
    /// Lo consume el log ante un panic.
    pub fn diag_snapshot(&self) -> crate::logging::DiagSnapshot {
        use std::fmt::Write as _;
        let mut panes = String::new();
        for p in self.ws.panes() {
            let dir = p
                .files
                .as_ref()
                .map(|f| f.current_dir.display().to_string())
                .unwrap_or_else(|| format!("{:?}", p.purpose));
            let _ = write!(panes, "[{}] {}  ", p.id.0, dir);
        }
        crate::logging::DiagSnapshot {
            panes: panes.trim_end().to_string(),
            theme: self.config.settings.theme.as_str().to_string(),
            lang: self.config.settings.language.0.clone(),
            last_action: crate::logging::last_breadcrumb(),
        }
    }
}

/// Construye un `DirTree` inicial con una raíz por unidad del sistema.
fn build_tree() -> DirTree {
    let drives: Vec<(PathBuf, String, naygo_core::icon_kind::DriveKind)> =
        naygo_platform::drives::drives()
            .into_iter()
            .map(|d| (d.path, d.label, d.kind))
            .collect();
    DirTree::from_drives(&drives)
}

/// Uso de disco de una unidad (total/libre), o `None` si no se puede leer (red caída,
/// óptico vacío). De ahí salen tanto la barrita (% usado) como el texto de espacio.
fn disk_usage(root: &Path) -> Option<naygo_core::disk::DiskUsage> {
    let (total, free) = naygo_platform::drive_space::read_space(root)?;
    Some(naygo_core::disk::DiskUsage { total, free })
}

// --- Parseo/formato para el editor de filtros de columna (F2) ---

const SECS_PER_DAY: i64 = 86_400;

/// Parsea un tamaño escrito por el usuario a bytes. Acepta dígitos con separadores y un sufijo
/// opcional K/KB/M/MB/G/GB (base 1024, sin distinguir mayúsculas). Vacío o inválido → `None`
/// (= sin límite ese extremo). Ej: "10" → 10, "2 KB" → 2048, "1.5M" → 1572864.
fn parse_size(text: &str) -> Option<u64> {
    let t = text
        .trim()
        .to_ascii_lowercase()
        .replace([',', '_', ' '], "");
    if t.is_empty() {
        return None;
    }
    let (num, mult) = if let Some(n) = t.strip_suffix("gb").or_else(|| t.strip_suffix('g')) {
        (n, 1024u64 * 1024 * 1024)
    } else if let Some(n) = t.strip_suffix("mb").or_else(|| t.strip_suffix('m')) {
        (n, 1024 * 1024)
    } else if let Some(n) = t.strip_suffix("kb").or_else(|| t.strip_suffix('k')) {
        (n, 1024)
    } else {
        (t.as_str(), 1)
    };
    let value: f64 = num.parse().ok()?;
    if value < 0.0 {
        return None;
    }
    Some((value * mult as f64) as u64)
}

/// Parsea una fecha `YYYY-MM-DD` (en hora local, `tz_offset_secs`) a `SystemTime`. Si `end_of_day`,
/// apunta al final del día (23:59:59) para que el extremo "hasta" sea inclusivo. Vacío/ inválido →
/// `None`. Cálculo propio (sin chrono): días desde la época civil (algoritmo de Howard Hinnant).
fn parse_date_ymd(text: &str, tz_offset_secs: i64, end_of_day: bool) -> Option<SystemTime> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let mut parts = t.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, m, d);
    let mut secs_local = days * SECS_PER_DAY;
    if end_of_day {
        secs_local += SECS_PER_DAY - 1;
    }
    // De hora local a UTC: restar el huso.
    let secs_utc = secs_local - tz_offset_secs;
    if secs_utc < 0 {
        return Some(SystemTime::UNIX_EPOCH);
    }
    Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs_utc as u64))
}

/// Formatea un `SystemTime` como `YYYY-MM-DD` en hora local (para sembrar el editor de fecha).
fn fmt_date_ymd(t: SystemTime, tz_offset_secs: i64) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 + tz_offset_secs)
        .unwrap_or(0);
    let days = secs.div_euclid(SECS_PER_DAY);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Días desde 1970-01-01 para una fecha civil (algoritmo de Howard Hinnant, dominio público).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Inverso de `days_from_civil`: (año, mes, día) a partir de días desde la época.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drena los listados hasta que todos terminan (con timeout), simulando los ticks del
    /// Timer. Devuelve true si terminaron.
    fn drain(c: &mut WorkspaceCtrl) -> bool {
        for _ in 0..2000 {
            if c.pump_listings() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        false
    }

    fn active_pos_of(c: &WorkspaceCtrl, name: &str) -> Option<usize> {
        let f = c.ws.active_files()?;
        f.view_indices()
            .iter()
            .position(|&real| f.entries[real].name == name)
    }

    #[test]
    fn parse_size_acepta_sufijos_y_vacio() {
        assert_eq!(parse_size(""), None);
        assert_eq!(parse_size("  "), None);
        assert_eq!(parse_size("10"), Some(10));
        assert_eq!(parse_size("2kb"), Some(2048));
        assert_eq!(parse_size("2 KB"), Some(2048));
        assert_eq!(parse_size("1m"), Some(1024 * 1024));
        assert_eq!(parse_size("1.5M"), Some(1024 * 1024 + 512 * 1024));
        assert_eq!(parse_size("3gb"), Some(3 * 1024 * 1024 * 1024));
        assert_eq!(parse_size("1,024"), Some(1024));
        assert_eq!(parse_size("no"), None);
    }

    #[test]
    fn fecha_ymd_round_trip_en_utc() {
        // En UTC (tz=0): 2026-06-16 al inicio del día, y de vuelta a la misma cadena.
        let t = parse_date_ymd("2026-06-16", 0, false).unwrap();
        assert_eq!(fmt_date_ymd(t, 0), "2026-06-16");
        // Fin del día sigue siendo el mismo día (23:59:59).
        let end = parse_date_ymd("2026-06-16", 0, true).unwrap();
        assert_eq!(fmt_date_ymd(end, 0), "2026-06-16");
        assert!(end > t, "fin de día es posterior al inicio");
        // Inválidas y vacías → None.
        assert_eq!(parse_date_ymd("", 0, false), None);
        assert_eq!(parse_date_ymd("2026-13-01", 0, false), None);
        assert_eq!(parse_date_ymd("nope", 0, false), None);
    }

    /// El menú de columna aplica un filtro de texto sobre Name y la vista se refiltra sola; al
    /// quitarlo, vuelven todas las filas. Cubre column_menu_open → set_text → apply → clear.
    #[test]
    fn menu_de_columna_filtra_y_limpia_por_nombre() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("informe.pdf"), b"x").unwrap();
        std::fs::write(work.path().join("notas.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let id = c.ws.active_id().unwrap();
        assert_eq!(c.ws.active_files().unwrap().view_len(), 2);

        // Abrir menú sobre la columna Name (kind 0), pasar a filtro, escribir y aplicar.
        c.column_menu_open(id, 0, 0.0, 0.0);
        c.column_menu_to_filter();
        c.column_filter_set_text("informe");
        c.column_filter_apply();
        assert!(c.column_menu.is_none(), "aplicar cierra el menú");
        assert_eq!(
            c.ws.active_files().unwrap().view_len(),
            1,
            "el filtro deja solo informe.pdf"
        );
        assert!(!c.no_matches(id), "hay una coincidencia");

        // Quitar el filtro: vuelven las dos filas.
        c.column_menu_open(id, 0, 0.0, 0.0);
        c.column_menu_clear_filter();
        assert_eq!(c.ws.active_files().unwrap().view_len(), 2);
    }

    /// La carpeta destino de "abrir terminal aquí" es la subcarpeta seleccionada si la hay, y si
    /// no, la carpeta del panel activo.
    #[test]
    fn terminal_dir_usa_carpeta_seleccionada_o_actual() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let sub = work.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(work.path().join("a.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));

        // Sin selección de carpeta → la carpeta del panel.
        assert_eq!(c.terminal_dir().as_deref(), Some(work.path()));

        // Seleccionar la subcarpeta → terminal apunta a la subcarpeta.
        let pos = active_pos_of(&c, "sub").unwrap();
        c.ws.active_files_mut().unwrap().select_single(pos);
        // El menú contextual toma la selección como objetivo.
        c.open_context_menu(0.0, 0.0);
        assert_eq!(c.terminal_dir().as_deref(), Some(sub.as_path()));

        // Seleccionar un ARCHIVO → cae a la carpeta del panel (no se abre terminal en un archivo).
        c.column_menu_close();
        let posf = active_pos_of(&c, "a.txt").unwrap();
        c.ws.active_files_mut().unwrap().select_single(posf);
        c.open_context_menu(0.0, 0.0);
        assert_eq!(c.terminal_dir().as_deref(), Some(work.path()));
    }

    /// C3: agregar/alternar/aliasar/quitar reglas de previsualización; persisten.
    #[test]
    fn reglas_de_preview_crud() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        // Agregar una extensión nueva (normaliza el punto y mayúsculas).
        c.preview_rule_add(".SIF");
        assert!(c.preview_rules().iter().any(|(e, on, _)| e == "sif" && *on));
        // No duplica.
        c.preview_rule_add("sif");
        assert_eq!(
            c.preview_rules()
                .iter()
                .filter(|(e, _, _)| e == "sif")
                .count(),
            1
        );
        // Tratar-como.
        c.preview_rule_set_treat_as("sif", "XML");
        assert!(c
            .preview_rules()
            .iter()
            .any(|(e, _, t)| e == "sif" && t == "xml"));
        // Alternar.
        c.preview_rule_toggle("sif");
        assert!(c
            .preview_rules()
            .iter()
            .any(|(e, on, _)| e == "sif" && !*on));
        // Persistió: reabrir y la regla sigue.
        let c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(c2
            .preview_rules()
            .iter()
            .any(|(e, _, t)| e == "sif" && t == "xml"));
        // Quitar.
        c.preview_rule_remove("sif");
        assert!(!c.preview_rules().iter().any(|(e, _, _)| e == "sif"));
    }

    /// C4: guardar la tabla del panel activo como plantilla → un panel Files nuevo nace con esas
    /// columnas (no las default). Limpiar la plantilla restaura el comportamiento por defecto.
    #[test]
    fn plantilla_de_tabla_por_defecto_se_aplica_a_paneles_nuevos() {
        use naygo_core::columns::ColumnKind;
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        // Ocultar "Extensión" en el panel activo y guardarlo como plantilla.
        c.ws.active_files_mut()
            .unwrap()
            .table
            .toggle_visible(ColumnKind::Extension);
        c.save_default_table_from_active();
        assert!(c.config.settings.default_table.is_some());
        // Un panel nuevo (split) hereda la plantilla: Extensión oculta.
        c.add_pane_split();
        let new_id = *c.ws.files_panes().last().unwrap();
        let ext_visible =
            c.ws.pane(new_id)
                .unwrap()
                .files
                .as_ref()
                .unwrap()
                .table
                .columns
                .iter()
                .find(|col| col.kind == ColumnKind::Extension)
                .unwrap()
                .visible;
        assert!(
            !ext_visible,
            "el panel nuevo hereda la plantilla (Extensión oculta)"
        );
        // Limpiar la plantilla.
        c.clear_default_table();
        assert!(c.config.settings.default_table.is_none());
    }

    /// Cerrar un panel: con dos paneles, `close_pane` quita uno y deja el otro; el último panel
    /// NO se puede cerrar (can_close_pane = false) para no dejar la ventana vacía.
    #[test]
    fn cerrar_panel_quita_uno_y_protege_el_ultimo() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        // Un solo panel: no se puede cerrar.
        let first = *c.ws.files_panes().first().unwrap();
        assert!(!c.can_close_pane(first));
        // Agregar un segundo panel y cerrarlo: vuelve a quedar uno.
        c.add_pane_split();
        assert!(drain(&mut c));
        let second = *c.ws.files_panes().last().unwrap();
        assert!(c.can_close_pane(second));
        c.close_pane(second);
        assert_eq!(c.ws.panes().len(), 1, "queda un solo panel tras cerrar");
        assert!(c.ws.pane(second).is_none(), "el panel cerrado ya no existe");
        // El que queda no se puede cerrar.
        let remaining = *c.ws.files_panes().first().unwrap();
        assert!(!c.can_close_pane(remaining));
    }

    /// F3 calcula el tamaño de la carpeta del panel: spawnea el worker, se drena hasta terminar,
    /// y la barra de estado muestra el total.
    #[test]
    fn calcular_tamano_de_carpeta() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("a.bin"), vec![0u8; 1000]).unwrap();
        std::fs::write(work.path().join("b.bin"), vec![0u8; 2000]).unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        // Sin foco en una subcarpeta → calcula la carpeta del panel (work).
        c.compute_size_active();
        assert!(c.size_job.is_some());
        // Drenar el worker hasta que termine.
        for _ in 0..3000 {
            if c.pump_sizes() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let job = c.size_job.as_ref().unwrap();
        assert!(job.done);
        assert_eq!(job.bytes, 3000, "suma de los dos archivos");
        // La barra de estado anexa el resultado (el nombre de la carpeta calculada).
        let name = work
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            c.status_line().contains(&name),
            "el resultado del cálculo aparece en el status: {}",
            c.status_line()
        );
    }

    /// Navegación por teclado del árbol: ↓ mueve el cursor, → expande, Enter navega el panel
    /// Files a la carpeta del cursor.
    #[test]
    fn arbol_teclado_cursor_expande_y_navega() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let sub = work.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        // Crear un panel Árbol cuya raíz sea `work` (insertamos un DirTree controlado).
        let tree_id = c.ws.add_pane(PanePurpose::Tree, std::path::PathBuf::new());
        let mut t = DirTree::from_drives(&[(
            work.path().to_path_buf(),
            "work".into(),
            naygo_core::icon_kind::DriveKind::Fixed,
        )]);
        // Cargar el hijo `sub` bajo la raíz.
        t.begin_loading(work.path());
        t.push_child(work.path(), sub.clone());
        t.finish_loading(work.path(), naygo_core::tree::NodeOutcome::Done);
        t.collapse(work.path()); // arrancar colapsado
        c.trees.insert(tree_id, t);

        // Cursor inicial = primera raíz (work). → expande la raíz.
        c.tree_key(tree_id, "right");
        assert!(
            c.trees
                .get(&tree_id)
                .unwrap()
                .node_at(work.path())
                .unwrap()
                .expanded,
            "→ expande la raíz"
        );
        // ↓ baja el cursor a `sub` (ya visible bajo la raíz expandida).
        c.tree_key(tree_id, "down");
        assert_eq!(c.tree_cursor_of(tree_id).as_deref(), Some(sub.as_path()));
        // Enter navega el panel Files activo a `sub`.
        assert!(c.tree_key(tree_id, "enter"));
        assert!(drain(&mut c));
        assert_eq!(c.ws.active_files().unwrap().current_dir, sub);
    }

    /// La ayuda (F1) lista atajos activos (con chord no vacío) e incluye el propio F1.
    #[test]
    fn ayuda_lista_atajos_activos() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        let rows = c.help_shortcuts();
        assert!(!rows.is_empty(), "hay atajos");
        assert!(
            rows.iter().all(|(_, chord)| !chord.is_empty()),
            "solo acciones con atajo asignado"
        );
        assert!(
            rows.iter().any(|(_, chord)| chord == "F1"),
            "F1 (ayuda) está en la lista"
        );
    }

    /// Atrás/adelante de teclado: navegar a una subcarpeta, volver con go_back, re-avanzar con
    /// go_forward. Replica el historial estilo navegador.
    #[test]
    fn teclado_atras_y_adelante() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let sub = work.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        c.navigate_active_to(sub.clone());
        assert!(drain(&mut c));
        assert_eq!(c.ws.active_files().unwrap().current_dir, sub);
        // Atrás → vuelve a work.
        assert!(c.on_go_back());
        assert!(drain(&mut c));
        assert_eq!(c.ws.active_files().unwrap().current_dir, work.path());
        // Adelante → re-entra a sub.
        assert!(c.on_go_forward());
        assert!(drain(&mut c));
        assert_eq!(c.ws.active_files().unwrap().current_dir, sub);
        // Sin más adelante: no-op.
        assert!(!c.on_go_forward());
    }

    /// "Mover al otro panel" con dos paneles: copia la selección al directorio del otro panel
    /// (una op deshacible). Verifica que el archivo aparezca en el destino.
    #[test]
    fn mover_al_otro_panel_con_dos_paneles() {
        let cfg = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        // Segundo panel apuntando a `b`.
        let origin = c.ws.active_id().unwrap();
        let dest = c.split_for_target().unwrap();
        c.open_in_pane(dest, b.path().to_path_buf());
        assert!(drain(&mut c));
        // Dar un área para que resolve_target tenga rects de ambos paneles.
        c.set_area(Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        });
        c.ws.set_active(origin);
        // Seleccionar el archivo y copiarlo al otro panel (move=false).
        c.ws.active_files_mut().unwrap().select_all();
        c.op_to_other(false);
        for _ in 0..2000 {
            if c.ops.pump_ops() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(b.path().join("doc.txt").exists(), "se copió al otro panel");
        assert!(
            a.path().join("doc.txt").exists(),
            "el original sigue (copia)"
        );
    }

    /// El batch-rename: abrir con la selección, editar el spec (plantilla + contador) y aplicar.
    /// La op renombra los archivos en disco (verificado tras drenar las ops).
    #[test]
    fn batch_rename_abre_edita_y_aplica() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("a.txt"), b"x").unwrap();
        std::fs::write(work.path().join("b.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        // Seleccionar todo y abrir el batch-rename.
        c.ws.active_files_mut().unwrap().select_all();
        c.batch_open();
        assert!(c.batch.is_some(), "se abrió la ventana");

        // Plantilla "foto{n}" con contador desde 1 → foto1.txt, foto2.txt.
        c.batch_set_template("foto{n}");
        let rows = c.batch_preview();
        assert_eq!(rows.len(), 2);
        let nuevos: Vec<&str> = rows.iter().map(|r| r.new_name.as_str()).collect();
        assert!(nuevos.contains(&"foto1.txt") && nuevos.contains(&"foto2.txt"));
        assert!(c.batch_can_apply());

        // Aplicar y drenar la op; los archivos quedan renombrados en disco.
        c.batch_apply();
        assert!(c.batch.is_none(), "aplicar cierra la ventana");
        for _ in 0..2000 {
            if c.ops.pump_ops() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(work.path().join("foto1.txt").exists() || work.path().join("foto2.txt").exists());
        assert!(!work.path().join("a.txt").exists() && !work.path().join("b.txt").exists());
    }

    /// Una plantilla que produce el mismo nombre para todos marca colisión y no deja aplicar.
    #[test]
    fn batch_rename_colision_no_deja_aplicar() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("a.txt"), b"x").unwrap();
        std::fs::write(work.path().join("b.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        c.ws.active_files_mut().unwrap().select_all();
        c.batch_open();
        c.batch_set_template("igual"); // ambos → "igual.txt" → colisión
        assert!(!c.batch_can_apply());
    }

    /// Aplicar una plantilla built-in reconstruye el workspace con sus paneles, y guardar la
    /// disposición actual como plantilla de usuario persiste (se ve en otro controlador).
    #[test]
    fn plantillas_aplicar_guardar_y_borrar() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        assert_eq!(c.ws.panes().len(), 1, "arranca con un panel");

        // Aplicar "Dual-pane" (4 paneles: árbol + 2 files + inspector).
        c.apply_template("Dual-pane", 100);
        assert!(drain(&mut c));
        assert_eq!(c.ws.panes().len(), 4);
        assert_eq!(c.ws.files_panes().len(), 2);
        // Quedó registrado en recientes.
        assert_eq!(
            c.templates.recents.first().map(|r| r.name.as_str()),
            Some("Dual-pane")
        );

        // Guardar la disposición actual como plantilla de usuario.
        c.save_current_template("Mi setup");
        assert!(c
            .layout_templates()
            .iter()
            .any(|(n, builtin)| n == "Mi setup" && !builtin));
        // Persistió: un controlador nuevo (mismo config_dir) la ve.
        let c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(c2.templates.user.iter().any(|t| t.name == "Mi setup"));

        // Borrarla.
        c.delete_template("Mi setup");
        assert!(!c.layout_templates().iter().any(|(n, _)| n == "Mi setup"));
    }

    /// Primera ejecución (sin sesión guardada): `apply_first_run_layout` arma la disposición
    /// clásica: árbol + dos paneles de archivos + propiedades + vista previa (5 paneles).
    #[test]
    fn primera_ejecucion_arma_layout_clasico() {
        use naygo_core::workspace::PanePurpose;
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        assert_eq!(
            c.ws.panes().len(),
            1,
            "new_in sigue arrancando con un panel"
        );

        c.apply_first_run_layout();
        assert!(drain(&mut c));
        assert_eq!(c.ws.panes().len(), 5, "árbol + 2 files + props + preview");
        assert_eq!(c.ws.files_panes().len(), 2, "dos paneles de archivos");
        let has = |p: PanePurpose| c.ws.panes().iter().any(|x| x.purpose == p);
        assert!(has(PanePurpose::Tree), "hay árbol");
        assert!(has(PanePurpose::Inspector), "hay propiedades");
        assert!(has(PanePurpose::Preview), "hay vista previa");
    }

    /// Cuando NO hay sesión guardada, `load_session` devuelve `false` (señal de primera
    /// ejecución); cuando sí la hay, devuelve `true`.
    #[test]
    fn load_session_reporta_si_restauro() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        // Sin sesión previa: false.
        let mut c1 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c1));
        assert!(!c1.load_session(), "sin sesión previa → false");
        // Guardar una sesión y reabrir: true.
        c1.save_session();
        let mut c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(c2.load_session(), "con sesión guardada → true");
    }

    /// "Mover →" desde el menú reordena la columna en el orden visual completo.
    #[test]
    fn menu_de_columna_mueve_la_columna() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let id = c.ws.active_id().unwrap();
        // Orden por defecto: Name, Extension, Size, Modified, Created. Mover Extension (índice 1)
        // a la derecha → queda detrás de Size.
        c.column_menu_open(id, 1, 0.0, 0.0); // kind 1 = Extension
        c.column_menu_move(1);
        let order: Vec<_> =
            c.ws.pane(id)
                .unwrap()
                .files
                .as_ref()
                .unwrap()
                .table
                .columns
                .iter()
                .map(|col| col.kind)
                .collect();
        assert_eq!(order[1], naygo_core::columns::ColumnKind::Size);
        assert_eq!(order[2], naygo_core::columns::ColumnKind::Extension);
        assert!(c.column_menu.is_none(), "mover cierra el menú");
    }

    /// Un filtro que no coincide con nada marca `no_matches` (aviso "sin coincidencias"), sin
    /// confundirlo con una carpeta vacía.
    #[test]
    fn filtro_sin_coincidencias_se_detecta() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("a.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let id = c.ws.active_id().unwrap();
        c.column_menu_open(id, 0, 0.0, 0.0);
        c.column_menu_to_filter();
        c.column_filter_set_text("zzz-no-existe");
        c.column_filter_apply();
        assert_eq!(c.ws.active_files().unwrap().view_len(), 0);
        assert!(c.no_matches(id), "filtro vació la vista → aviso");
    }

    /// F4: la sesión (paneles + carpetas + disposición) se guarda al cerrar y se restaura al
    /// abrir. Dividir en dos paneles, navegar uno a una subcarpeta, guardar, y reconstruir en
    /// un controlador nuevo (mismo config_dir) restaura los dos paneles con sus carpetas.
    #[test]
    fn sesion_guarda_y_restaura_dos_paneles() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let sub = work.path().join("sub");
        std::fs::create_dir(&sub).unwrap();

        // Controlador 1: arranca con un panel en `work`, lo divide (segundo panel), y navega
        // el panel activo (el nuevo) a la subcarpeta.
        let mut c1 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c1));
        c1.add_pane_split();
        assert_eq!(c1.ws.panes().len(), 2, "tras dividir hay dos paneles");
        assert!(drain(&mut c1));
        c1.navigate_active_to(sub.clone());
        assert!(drain(&mut c1));
        // El layout referencia los dos paneles.
        assert_eq!(
            c1.ws.layout.pane_ids().len(),
            2,
            "el layout tiene dos hojas"
        );
        c1.save_session();

        // Controlador 2: mismo config_dir; load_session reemplaza el arranque por la sesión.
        let mut c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        c2.load_session();
        assert_eq!(c2.ws.panes().len(), 2, "se restauran los dos paneles");
        assert_eq!(
            c2.ws.layout.pane_ids().len(),
            2,
            "se restaura la disposición"
        );
        let dirs: Vec<std::path::PathBuf> = c2
            .ws
            .panes()
            .iter()
            .filter_map(|p| p.files.as_ref().map(|f| f.current_dir.clone()))
            .collect();
        assert!(
            dirs.contains(&work.path().to_path_buf()),
            "un panel quedó en la carpeta raíz"
        );
        assert!(
            dirs.contains(&sub),
            "el otro panel quedó en la subcarpeta navegada"
        );
    }

    /// F5A: un evento del watcher (Created) agrega la entrada al panel sin re-listar y la
    /// reporta como nueva (para resaltarla).
    #[test]
    fn watch_events_agregan_entry() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("viejo.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let id = c.active_id().unwrap();
        let nuevo = tmp.path().join("nuevo.txt");
        std::fs::write(&nuevo, b"y").unwrap();
        let nuevas =
            c.apply_watch_events(id, &[naygo_core::listing::DirEvent::Created(nuevo.clone())]);
        assert_eq!(nuevas, vec![nuevo.clone()]);
        assert!(c
            .rows_of(id, 8, std::time::Instant::now())
            .iter()
            .any(|r| r.name == "nuevo.txt"));
    }

    /// 6D: el rename inline. `op_rename` marca el pedido sobre la fila enfocada; `rename_commit`
    /// renombra el archivo en disco; `rename_chain` confirma y avanza a la fila siguiente.
    #[test]
    fn rename_inline_y_en_cadena() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"y").unwrap();
        let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let id = c.active_id().unwrap();
        // Enfocar "a.txt" y pedir rename (F2): debe marcar el pedido en esa posición.
        let pos = active_pos_of(&c, "a.txt").expect("a.txt visible");
        c.ws.active_files_mut().unwrap().select_single(pos);
        c.op_rename();
        let req = c
            .take_rename_request()
            .expect("F2 marcó el pedido de rename");
        assert_eq!(req.0, id);
        assert_eq!(req.1, pos);
        assert_eq!(req.2, 0, "etapa inicial = nombre sin extensión");
        // Confirmar el rename y bombear la op hasta completarla (mismo patrón que ops_ctrl).
        assert!(c.rename_commit(id, pos, "renombrado.txt"));
        for _ in 0..4000 {
            let done = c.ops.pump_ops();
            if done && !c.ops.active_ops.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            tmp.path().join("renombrado.txt").exists(),
            "el archivo se renombró en disco"
        );
        assert!(!tmp.path().join("a.txt").exists());
        // Rename en cadena hacia abajo desde "b.txt": confirma (sin cambio) y devuelve la
        // posición avanzada con un nuevo pedido en etapa 0.
        // Re-listar para reflejar el rename antes de seguir.
        c.start_listing(id, tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let pos_b = active_pos_of(&c, "b.txt").expect("b.txt visible");
        let next = c.rename_chain(id, pos_b, "b.txt", 1);
        assert!(next.is_some(), "encadena a una fila válida");
        let req2 = c.take_rename_request().expect("chain reabre el editor");
        assert_eq!(req2.2, 0, "el chain selecciona el nombre sin extensión");
    }

    /// 6F: rubber-band. select_rect_range selecciona el rango inclusivo de filas; aditivo (Ctrl)
    /// suma a lo ya seleccionado.
    #[test]
    fn rubber_band_selecciona_rango() {
        let tmp = tempfile::tempdir().unwrap();
        for n in ["a.txt", "b.txt", "c.txt", "d.txt"] {
            std::fs::write(tmp.path().join(n), b"x").unwrap();
        }
        let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let id = c.active_id().unwrap();
        // Rectángulo de la fila 0 a la 2 (inclusive) → 3 seleccionadas.
        c.select_rect_range(id, 0, 2, false);
        let sel = c.ws.active_files().unwrap().selected.len();
        assert_eq!(sel, 3);
        // Aditivo: sumar la fila 3 conserva las anteriores → 4.
        c.select_rect_range(id, 3, 3, true);
        assert_eq!(c.ws.active_files().unwrap().selected.len(), 4);
        // No-aditivo: reemplaza con una sola.
        c.select_rect_range(id, 1, 1, false);
        assert_eq!(c.ws.active_files().unwrap().selected.len(), 1);
    }

    /// La path-bar: breadcrumbs de la carpeta del panel + autocompletado del editor.
    #[test]
    fn pathbar_segmentos_y_autocompletado() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("Alpha")).unwrap();
        std::fs::create_dir(tmp.path().join("alfajor")).unwrap();
        std::fs::create_dir(tmp.path().join("Beta")).unwrap();
        let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let id = c.active_id().unwrap();
        // Breadcrumbs: el último segmento es el nombre de la carpeta actual.
        let segs = c.path_segments_of(id);
        assert!(!segs.is_empty());
        assert_eq!(segs.last().unwrap().1, tmp.path().display().to_string());
        // Autocompletado: tecleando "<tmp>\al" matchea Alpha y alfajor (case-insensitive).
        let buffer = format!("{}\\al", tmp.path().display());
        let sugg = c.path_autocomplete(&buffer);
        assert!(sugg.iter().any(|s| s == "Alpha"));
        assert!(sugg.iter().any(|s| s == "alfajor"));
        assert!(!sugg.iter().any(|s| s == "Beta"));
    }

    /// La navegación desde un panel auxiliar (Árbol) va al ÚLTIMO panel Files activo, no al
    /// primero. Regresión del bug "clic en disco cambia el primer panel".
    #[test]
    fn navega_al_ultimo_files_activo_no_al_primero() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let first = c.active_id().unwrap(); // primer Files
        c.add_pane_split(); // segundo Files, queda activo
        let second = c.active_id().unwrap();
        assert_ne!(first, second);
        // Agregar un Árbol y activarlo (simula clic en el panel Carpetas).
        c.add_pane_of(PanePurpose::Tree);
        let tree = c.active_id().unwrap();
        c.set_active(tree);
        // Navegar desde el árbol → debe ir al SEGUNDO Files (el último activo), no al primero.
        c.navigate_active_to(sub.clone());
        assert_eq!(
            c.ws.pane(second)
                .and_then(|p| p.files.as_ref())
                .map(|f| f.current_dir.clone()),
            Some(sub.clone())
        );
        assert_ne!(
            c.ws.pane(first)
                .and_then(|p| p.files.as_ref())
                .map(|f| f.current_dir.clone()),
            Some(sub)
        );
    }

    /// F5B: si un panel quedó parado en una carpeta que desapareció (p. ej. se sacó el USB), el
    /// panel marca su carpeta como "perdida" (aviso IN-PLACE, sin popup global). Elegir "subir al
    /// ancestro existente" lo reubica a la carpeta superior que exista y limpia el aviso.
    #[test]
    fn carpeta_perdida_se_detecta_por_panel_y_sube_al_ancestro() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("usb");
        std::fs::create_dir(&sub).unwrap();
        let mut c = WorkspaceCtrl::new_in(sub.clone(), tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let id = c.ws.active_id().unwrap();
        assert!(!c.pane_dir_missing(id), "al inicio la carpeta existe");
        std::fs::remove_dir_all(&sub).unwrap(); // "sacar el USB"
        assert!(
            c.pane_dir_missing(id),
            "el panel detecta su carpeta perdida"
        );
        assert_eq!(c.ws.active_files().unwrap().current_dir, sub);
        // "Subir al ancestro existente": el panel queda en tmp (el padre que sigue vivo).
        c.missing_folder_go_ancestor(id);
        assert!(!c.pane_dir_missing(id), "ya no está perdida tras subir");
        assert_eq!(c.ws.active_files().unwrap().current_dir, tmp.path());
    }

    /// REVEAL: al fijar un destino, el árbol expande progresivamente los ancestros hasta él.
    #[test]
    fn arbol_revela_la_carpeta_objetivo() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = a.join("b");
        std::fs::create_dir_all(&b).unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        // Árbol con raíz manual en tmp (no dependemos de las unidades reales).
        let tree_id = c.ws.add_pane(PanePurpose::Tree, std::path::PathBuf::new());
        let mut t = DirTree::default();
        t.roots
            .push(naygo_core::tree::TreeNode::folder(tmp.path().to_path_buf()));
        c.trees.insert(tree_id, t);
        // Pedir revelar tmp/a/b: debe expandir tmp (raíz) y luego tmp/a.
        c.reveal_targets.insert(tree_id, b.clone());
        c.pump_reveal();
        // Drenar los workers de árbol + avanzar el reveal hasta que no quede target.
        for _ in 0..4000 {
            let done = c.pump_tree();
            if done {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        // tmp/a debe haber quedado expandido (su hijo "b" es el destino, queda visible).
        let a_expanded = c
            .trees
            .get(&tree_id)
            .and_then(|t| t.node_at(&a))
            .map(|n| n.expanded && n.children.is_some())
            .unwrap_or(false);
        assert!(a_expanded, "el ancestro tmp/a quedó expandido (revela b)");
        assert!(
            c.reveal_targets.is_empty(),
            "el target se limpió al completar el reveal"
        );
    }

    /// REGRESIÓN (heredada de F1): navegar a una carpeta repuebla la vista del panel
    /// activo (el listado de la carpeta nueva se arranca al navegar).
    #[test]
    fn navegar_repuebla_la_vista() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("dentro.txt"), b"x").unwrap();

        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c), "listado inicial termina");
        let id = c.active_id().unwrap();
        let pos = active_pos_of(&c, "sub").expect("'sub' visible");
        assert!(c.on_row_double_clicked(id, pos), "doble clic navega");
        assert!(drain(&mut c), "listado de sub termina");
        let rows = c.rows_of(c.active_id().unwrap(), 8, std::time::Instant::now());
        assert!(
            rows.iter().any(|r| r.name == "dentro.txt"),
            "la vista refleja la carpeta nueva (no vacía)"
        );
    }

    /// El doble-clic detectado en Rust (dos on_row_clicked rápidos en la misma fila) navega;
    /// dos clics LENTOS no.
    #[test]
    fn doble_clic_en_rust_navega() {
        use std::time::{Duration, Instant};
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("dentro.txt"), b"x").unwrap();

        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let id = c.active_id().unwrap();
        let pos = active_pos_of(&c, "sub").expect("'sub' visible");

        // Una sola línea de tiempo sintética (mezclar Instant::now() con offsets daría
        // instantes incoherentes). Base + offsets crecientes.
        let base = Instant::now();
        // Dos clics LENTOS (separados > 700 ms) NO navegan: solo seleccionan.
        assert!(!c.on_row_clicked(id, pos, base));
        assert!(!c.on_row_clicked(id, pos, base + Duration::from_millis(900)));
        assert_eq!(c.path_of(id), tmp.path().display().to_string());

        // Dos clics RÁPIDOS (dentro de 700 ms) SÍ navegan. Siguen la misma línea de tiempo.
        let t1 = base + Duration::from_secs(5);
        assert!(!c.on_row_clicked(id, pos, t1), "1er clic: selecciona");
        assert!(
            c.on_row_clicked(id, pos, t1 + Duration::from_millis(150)),
            "2do clic rápido: doble-clic → navega"
        );
        assert!(drain(&mut c));
        let rows = c.rows_of(c.active_id().unwrap(), 8, std::time::Instant::now());
        assert!(rows.iter().any(|r| r.name == "dentro.txt"));
    }

    /// Agregar un panel divide el layout y deja DOS paneles Files; el nuevo queda activo.
    #[test]
    fn agregar_panel_divide_y_deja_dos() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let first = c.active_id().unwrap();
        c.add_pane_split();
        assert!(drain(&mut c));
        // Dos paneles Files en el layout, y el activo es el nuevo (distinto del primero).
        assert_eq!(c.ws.files_panes().len(), 2);
        assert_ne!(c.active_id(), Some(first), "el panel nuevo queda activo");
        // El área se reparte en dos rects no vacíos.
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        let rects = c.pane_rects(area);
        assert_eq!(rects.len(), 2);
        assert!(rects.iter().all(|(_, r)| r.w > 1.0 && r.h > 1.0));
        // Y hay un splitter entre ellos.
        assert_eq!(c.split_handles(area).len(), 1);
    }

    /// Agregar un panel especial (no-Files) crea el purpose correcto y NO arranca un
    /// listado de archivos (los auxiliares no listan). El Tree inicializa su DirTree.
    #[test]
    fn agregar_panel_especial_no_lista_archivos() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let files_listings_antes = c.listings.len();
        c.add_pane_of(PanePurpose::Tree);
        // Se agregó un panel Tree y no aumentaron los listados de archivos.
        assert!(c.ws.panes().iter().any(|p| p.purpose == PanePurpose::Tree));
        assert_eq!(
            c.listings.len(),
            files_listings_antes,
            "un panel Tree no arranca listado de archivos"
        );
        // El Tree tiene su DirTree (con al menos una raíz, si el sistema tiene unidades).
        let tree_id =
            c.ws.panes()
                .iter()
                .find(|p| p.purpose == PanePurpose::Tree)
                .unwrap()
                .id;
        assert!(c.trees.contains_key(&tree_id));
    }

    /// El inspector refleja el ítem enfocado del panel Files activo, aunque el panel activo
    /// sea un panel especial.
    #[test]
    fn inspector_lee_el_files_activo() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("dato.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        // Enfocar la primera fila de la vista.
        if let Some(f) = c.ws.active_files_mut() {
            f.select_single(0);
        }
        let info = c.inspector_info();
        assert!(info.present, "hay un ítem enfocado");
    }

    /// Navegar desde un favorito mueve el panel Files activo y lo registra en recientes.
    #[test]
    fn navegar_desde_favorito() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        c.favorites.toggle(&sub);
        assert!(c.go_favorite(0), "navega al favorito 0");
        assert!(drain(&mut c));
        assert_eq!(
            c.ws.active_files().map(|f| f.current_dir.clone()),
            Some(sub.clone())
        );
        // La carpeta nueva quedó en recientes (al frente).
        assert_eq!(c.recents.list().first(), Some(&sub));
    }

    /// Expandir una rama del árbol la marca expandida y, tras drenar, puebla sus hijos;
    /// colapsar la vuelve a cerrar.
    #[test]
    fn arbol_expande_colapsa_y_puebla() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("rama");
        std::fs::create_dir(&sub).unwrap();
        std::fs::create_dir(sub.join("hoja")).unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        // Crear un panel Tree con una raíz manual apuntando a tmp (no dependemos de las
        // unidades reales del sistema para el test).
        let tree_id = c.ws.add_pane(PanePurpose::Tree, std::path::PathBuf::new());
        let mut t = DirTree::default();
        t.roots
            .push(naygo_core::tree::TreeNode::folder(tmp.path().to_path_buf()));
        c.trees.insert(tree_id, t);

        c.tree_expand(tree_id, tmp.path().to_path_buf());
        // Drenar el worker del árbol.
        for _ in 0..2000 {
            if c.pump_tree() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let rows = c.tree_rows(tree_id);
        assert!(
            rows.iter().any(|r| r.name == "rama"),
            "la rama aparece como hijo"
        );
        // Colapsar: la raíz deja de estar expandida.
        c.tree_collapse(tree_id, tmp.path().to_path_buf());
        let node_expanded = c
            .trees
            .get(&tree_id)
            .and_then(|t| t.node_at(tmp.path()))
            .map(|n| n.expanded)
            .unwrap();
        assert!(!node_expanded, "la raíz quedó colapsada");
    }

    fn area() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        }
    }

    /// resolve_target: 1 panel → NeedsSplit; 2 → Direct(el otro); 3+ → Pick.
    #[test]
    fn resolve_target_segun_cantidad_de_paneles() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let a = c.active_id().unwrap();
        // Un solo panel → hay que dividir.
        assert_eq!(c.resolve_target(a, area()), PaneTarget::NeedsSplit);
        // Dos paneles → destino directo (el otro).
        c.add_pane_split();
        assert!(drain(&mut c));
        let b = c.active_id().unwrap();
        assert_eq!(c.resolve_target(b, area()), PaneTarget::Direct(a));
        // Tres paneles → selector (Pick con 2 candidatos).
        c.add_pane_split();
        assert!(drain(&mut c));
        let third = c.active_id().unwrap();
        match c.resolve_target(third, area()) {
            PaneTarget::Pick(cands) => assert_eq!(cands.len(), 2),
            other => panic!("esperaba Pick, fue {other:?}"),
        }
    }

    /// Swap intercambia las carpetas de dos paneles.
    #[test]
    fn swap_intercambia_carpetas() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("otra");
        std::fs::create_dir(&sub).unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let a = c.active_id().unwrap();
        c.add_pane_split();
        assert!(drain(&mut c));
        let b = c.active_id().unwrap();
        // Mandar b a la subcarpeta.
        c.open_in_pane(b, sub.clone());
        assert!(drain(&mut c));
        let dir_a_antes = c.path_of(a);
        let dir_b_antes = c.path_of(b);
        c.swap_panes(a, b);
        assert_eq!(c.path_of(a), dir_b_antes);
        assert_eq!(c.path_of(b), dir_a_antes);
    }

    /// Con 3+ paneles, una acción deja un pending_pick; elegir el número lo aplica.
    #[test]
    fn selector_pendiente_y_resolucion() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("dest");
        std::fs::create_dir(&sub).unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        c.add_pane_split();
        assert!(drain(&mut c));
        c.add_pane_split();
        assert!(drain(&mut c));
        let origin = c.active_id().unwrap();
        // Clonar desde el origen: 3 paneles → queda pendiente el selector con 2 candidatos.
        let acted = c.request_action(PaneAction::Clone, origin, area());
        assert!(!acted, "no actúa de inmediato: espera la elección");
        assert!(c.pending_pick.is_some());
        let candidates = c.pending_pick.as_ref().unwrap().candidates.clone();
        assert_eq!(candidates.len(), 2);
        // Elegir el panel 1: clona la carpeta del origen ahí.
        assert!(c.pick_resolve(1));
        assert!(drain(&mut c));
        assert!(c.pending_pick.is_none(), "el selector se cerró");
        assert_eq!(c.path_of(candidates[0]), c.path_of(origin));
    }

    /// Apilar el origen sobre otro panel los agrupa en pestañas; el origen queda activo.
    #[test]
    fn apilar_crea_un_grupo_de_pestanas() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let a = c.active_id().unwrap();
        c.add_pane_split();
        assert!(drain(&mut c));
        let b = c.active_id().unwrap();
        // Apilar b sobre a: quedan en un grupo de 2.
        c.stack_into(b, a);
        let groups = c.tab_groups();
        assert_eq!(groups.len(), 1);
        let (members, active) = &groups[0];
        assert_eq!(members.len(), 2);
        assert!(members.contains(&a) && members.contains(&b));
        // El miembro activo es el apilado (b).
        assert_eq!(members[*active], b);
    }

    /// set_active_tab cambia la pestaña visible del grupo.
    #[test]
    fn cambiar_pestana_activa() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let a = c.active_id().unwrap();
        c.add_pane_split();
        assert!(drain(&mut c));
        let b = c.active_id().unwrap();
        c.stack_into(b, a);
        // Activar a: pasa a ser la pestaña visible.
        c.set_active_tab(a);
        let (members, active) = c.tab_groups()[0].clone();
        assert_eq!(members[active], a);
        assert_eq!(c.active_id(), Some(a));
    }

    /// Cerrar una pestaña la quita; con una sola restante el grupo se colapsa a hoja.
    #[test]
    fn cerrar_pestana_colapsa_el_grupo() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let a = c.active_id().unwrap();
        c.add_pane_split();
        assert!(drain(&mut c));
        let b = c.active_id().unwrap();
        c.stack_into(b, a);
        assert_eq!(c.tab_groups().len(), 1);
        // Cerrar b: queda solo a, el grupo desaparece.
        c.close_tab(b);
        assert!(c.tab_groups().is_empty());
        assert!(c.ws.pane(b).is_none(), "el panel cerrado ya no existe");
        assert!(c.ws.pane(a).is_some(), "el otro sigue");
    }

    /// Soltar un panel en el CENTRO de otro los apila como pestañas.
    #[test]
    fn drop_en_el_centro_apila() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let a = c.active_id().unwrap();
        c.add_pane_split();
        assert!(drain(&mut c));
        let b = c.active_id().unwrap();
        // Layout: [a | b] en 800x600. Soltar a en el centro de b.
        let ar = area();
        let rects = c.pane_rects(ar);
        let b_rect = rects.iter().find(|(id, _)| *id == b).unwrap().1;
        let (cx, cy) = (b_rect.x + b_rect.w / 2.0, b_rect.y + b_rect.h / 2.0);
        assert!(c.perform_drop(a, cx, cy, ar));
        // Quedan agrupados.
        let groups = c.tab_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0.len(), 2);
    }

    /// Soltar un panel sobre sí mismo no hace nada.
    #[test]
    fn drop_sobre_si_mismo_es_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let a = c.active_id().unwrap();
        let ar = area();
        let r = c.pane_rects(ar)[0].1;
        assert!(!c.perform_drop(a, r.x + r.w / 2.0, r.y + r.h / 2.0, ar));
        assert!(c.tab_groups().is_empty());
    }

    /// Soltar en un borde divide; el panel arrastrado queda en el lado correspondiente.
    #[test]
    fn drop_en_borde_divide() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        let a = c.active_id().unwrap();
        c.add_pane_split();
        assert!(drain(&mut c));
        let b = c.active_id().unwrap();
        // Apilar primero a+b para tener un grupo, luego sacar 'a' soltándolo en un borde.
        c.stack_into(a, b);
        assert_eq!(c.tab_groups().len(), 1);
        let ar = area();
        let rects = c.pane_rects(ar);
        // El grupo ocupa todo; soltar 'a' en el borde derecho lo separa en un split.
        let r = rects[0].1;
        let (px, py) = (r.x + r.w - 5.0, r.y + r.h / 2.0);
        assert!(c.perform_drop(a, px, py, ar));
        // Ya no hay grupo (a salió); hay dos paneles en un split.
        assert!(c.tab_groups().is_empty());
        assert_eq!(c.pane_rects(ar).len(), 2);
    }

    /// Esc (vía pick_cancel) cierra el selector sin actuar.
    #[test]
    fn cancelar_selector() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
        assert!(drain(&mut c));
        c.add_pane_split();
        assert!(drain(&mut c));
        c.add_pane_split();
        assert!(drain(&mut c));
        let origin = c.active_id().unwrap();
        c.request_action(PaneAction::Clone, origin, area());
        assert!(c.pending_pick.is_some());
        c.pick_cancel();
        assert!(c.pending_pick.is_none());
    }

    /// Vista profunda: activar sobre un árbol temporal, drenar hasta completar y cancelar.
    /// Verifica el ciclo completo: deep_start → is_deep_active → deep_poll → deep_items →
    /// deep_cancel → ya no activo ni con ítems.
    #[test]
    fn vista_profunda_activa_acumula_y_cancela() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("a.txt"), b"x").unwrap();
        fs::write(root.join("sub/b.txt"), b"y").unwrap();

        let cfg = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(root.to_path_buf(), cfg.path().to_path_buf());
        let id = c.ws.active_id().unwrap();

        c.deep_start(id);
        assert!(c.is_deep_active(id));
        let mut tries = 0;
        while !c
            .deep_job
            .as_ref()
            .map(|d| d.done || d.cancelled)
            .unwrap_or(true)
            && tries < 2000
        {
            c.deep_poll();
            std::thread::sleep(std::time::Duration::from_millis(2));
            tries += 1;
        }
        c.deep_poll();
        // El árbol tiene 3 entradas: a.txt, sub, sub/b.txt
        assert_eq!(
            c.deep_items().len(),
            3,
            "deben llegar exactamente 3 entradas (a.txt, sub, sub/b.txt)"
        );
        c.deep_cancel();
        assert!(!c.is_deep_active(id));
        assert!(c.deep_items().is_empty());
    }
}
