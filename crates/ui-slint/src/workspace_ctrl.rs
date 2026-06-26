// Naygo — controlador multi-panel de la UI Slint (Fase 2a). Posee el Workspace (varios
// FilePaneState + layout) y traduce gestos a llamadas del core.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::bridge::{
    fav_tree_rows, favorite_rows, history_rows, inspector_info, recent_rows, rows_from_view,
    str_to_group_id, tree_rows, FavTreeRow, HistRow, InspectorInfo, NavRow, PlainRow, TreeRow,
    VisibilityFlags,
};
use crate::listing::Listing;
use naygo_core::favorites::{favorites_path, FavNode, Favorites, NodeId};
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
    /// Favoritos (global), ahora un ÁRBOL de grupos anidados. Se carga de
    /// `<config>/favorites.json` al arrancar y se persiste tras cada cambio (anclar/quitar,
    /// nuevo grupo, renombrar, eliminar, mover).
    pub favorites: Favorites,
    /// Grupos EXPANDIDOS del panel de favoritos, por su "ruta de nombres" ("Trabajo/Sub").
    /// Es una clave estable que sobrevive al reordenamiento de índices (a diferencia del
    /// `GroupId` numérico). Estado de UI: no se persiste.
    pub fav_expanded: std::collections::HashSet<String>,
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
    /// Panel Files que está BAJO el cursor mientras se ARRASTRAN archivos sobre la ventana
    /// (resaltado en vivo: borde + título). Lo actualiza la UI desde los eventos de hover del
    /// `IDropTarget` (`DragHover`), y se limpia al salir/soltar. `None` = no se está arrastrando
    /// sobre ningún panel Files. Estado de UI transitorio: no se persiste. La UI lo refleja en
    /// `PaneVm.drag-over` (true solo para el panel cuyo id coincide).
    pub drag_over_pane: Option<PaneId>,
    /// Drop intra-app (entre paneles) en ESPERA de confirmación del usuario (decisión de Nicolás:
    /// "confirmar al soltar"). `drop_at` ya validó (destino Files, no es la propia carpeta) y guardó
    /// aquí la operación; la UI muestra un modal "¿Copiar/Mover N a «destino»?" y, al confirmar,
    /// llama a `confirm_pending_drop` que arranca la op de verdad. Al cancelar, `cancel_pending_drop`
    /// lo descarta. `None` = no hay drop pendiente. Estado transitorio: no se persiste.
    pub pending_drop: Option<PendingDrop>,
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
    /// La paleta de comandos (Ctrl+P) pidió abrirse. La UI lo lee con `take_open_palette_request`
    /// para mostrar el overlay. Se setea desde `run_action(Action::CommandPalette)`. (Task 6/7)
    pub open_palette_requested: bool,
    /// La paleta eligió cambiar a este tema. El controlador ya lo aplicó+persistió en los
    /// settings (`config.set_theme`), pero re-pintar las ventanas Slint es cosa de la UI: la UI
    /// lo lee con `take_palette_theme_request` y llama a `theme_apply::apply`. (Task 6/7)
    pub palette_theme_requested: Option<naygo_core::theme::ThemeId>,
    /// La paleta eligió "abrir configuración". La UI lo lee con `take_open_config_request` y
    /// muestra la ventana de config (igual que el botón/menú de la toolbar). (Task 6/7)
    pub open_config_requested: bool,
    /// Un atajo de teclado pidió ABRIR un menú flotante de la toolbar (Favoritos / Disposiciones).
    /// `run_action` no puede tocar props de la UI (viven en la AppWindow), así que deja la petición
    /// aquí y la UI la consume con `take_toolbar_menu_request` para abrir el menú correspondiente.
    /// `Refresh-drives` también va por acá (refresca la tira). Ver `ToolbarMenuRequest`.
    pub toolbar_menu_requested: Option<ToolbarMenuRequest>,
    /// Cache de íconos (PNG → slint::Image, decodificado una vez por set+clave). Lo posee el
    /// controlador para resolver el ícono de cada fila al pintarla. Su set activo lo fija la
    /// configuración (Apariencia → Set de íconos). Ver `crate::icons::IconCache`.
    pub icons: crate::icons::IconCache,
    /// Caché del uso de disco por raíz de unidad (p. ej. `C:\`), para el footer de cada panel.
    /// Evita pegarle a WinAPI en CADA tick de `sync_rows`: se lee una vez por unidad y se reusa.
    /// Se vacía al navegar (vía `start_listing`) para que el espacio libre se refresque al
    /// entrar a otra carpeta/unidad. Ver `footer_text_for`.
    footer_disk_cache: std::collections::HashMap<std::path::PathBuf, naygo_core::disk::DiskUsage>,
}

/// Petición de un atajo de teclado que necesita que la UI abra un menú/acción de la toolbar cuyos
/// props viven en la AppWindow (no en el controlador). La UI la consume con
/// `take_toolbar_menu_request` tras procesar la tecla. Ver `Action::FavoritesMenu`/`LayoutsMenu`/
/// `RefreshDrives` en `run_action`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolbarMenuRequest {
    /// Abrir el menú flotante de favoritos (estrella ▾).
    Favorites,
    /// Abrir el menú flotante de disposiciones (grilla 2x2).
    Layouts,
    /// Refrescar la tira de unidades de disco (útil para unidades de red).
    RefreshDrives,
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

/// Un drop intra-app (entre paneles) ya validado y a la espera de que el usuario CONFIRME la
/// copia/mueve en un modal. Guarda todo lo necesario para arrancar la op al confirmar, sin volver
/// a hit-testear (el destino ya se resolvió). Ver `WorkspaceCtrl::pending_drop`.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingDrop {
    /// Archivos a copiar/mover (las rutas de origen del OLE).
    pub paths: Vec<PathBuf>,
    /// Carpeta destino resuelta (el panel bajo el cursor al soltar).
    pub dest_dir: PathBuf,
    /// `true` = mover, `false` = copiar (ya decidido por modificadores + mismo disco).
    pub is_move: bool,
    /// Cuántos elementos (para el texto del modal; == `paths.len()`).
    pub count: usize,
}

/// Cuántos nombres se listan explícitamente antes de truncar con "y N más" (umbral "pocos").
const DROP_NAMES_MAX_LISTED: usize = 4;

impl PendingDrop {
    /// Resumen legible de los nombres para el modal de confirmación. Pide nombrar lo que se va a
    /// copiar/mover (pedido de Nicolás), sobre todo cuando es más de uno:
    ///   - 1 elemento: «a.txt»
    ///   - pocos (2..=4): «a.txt», «b.txt», «c.txt»
    ///   - muchos (5+): «a.txt», «b.txt», «c.txt», «d.txt» + sufijo "y N más"
    ///
    /// El sufijo de "y N más" llega ya formateado (i18n, con el número resuelto) para no hardcodear
    /// texto aquí; `more_suffix(n)` recibe cuántos quedan sin nombrar y devuelve el texto a anexar
    /// (p.ej. " y 8 más"). Cada nombre va entre comillas angulares, como el resto de la UI.
    pub fn names_summary(&self, more_suffix: impl Fn(usize) -> String) -> String {
        let names: Vec<String> = self
            .paths
            .iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    // Sin nombre de archivo (raro: raíz de disco) → la ruta completa, no vacío.
                    .unwrap_or_else(|| p.display().to_string())
            })
            .collect();
        let listed = names.len().min(DROP_NAMES_MAX_LISTED);
        let mut out = names
            .iter()
            .take(listed)
            .map(|n| format!("«{}»", n))
            .collect::<Vec<_>>()
            .join(", ");
        let remaining = names.len().saturating_sub(listed);
        if remaining > 0 {
            out.push_str(&more_suffix(remaining));
        }
        out
    }
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
            favorites: load_favorites(&config_dir),
            fav_expanded: std::collections::HashSet::new(),
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
            drag_over_pane: None,
            pending_drop: None,
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
            open_palette_requested: false,
            palette_theme_requested: None,
            open_config_requested: false,
            toolbar_menu_requested: None,
            icons,
            footer_disk_cache: std::collections::HashMap::new(),
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
    /// re-ordena, resalta+selecciona los archivos recién llegados (Created), y devuelve las rutas
    /// NUEVAS (para que el watcher las marque "fresh"). No-op si el panel no es Files.
    ///
    /// Dos comportamientos, ambos sobre los Created (no se distingue origen app vs externo: tanto
    /// una copia/movida dentro de Naygo como un cambio externo se tratan igual, coherente con el
    /// resaltado ámbar):
    /// - Resaltado + selección: los nuevos quedan SELECCIONADOS por su posición de vista, para que
    ///   el usuario vea cuáles llegaron (sobre todo tras copiar). El foco va al último (scroll).
    /// - "Archivos nuevos al final" (setting `new_items_at_end`): se empuja `group_new_at_end` al
    ///   panel, de modo que `view_indices` agrupe las recién aparecidas al final de la vista en vez
    ///   de en su posición ordenada. Al refrescar (F5) o navegar, el resaltado se limpia y todo
    ///   vuelve a su orden normal.
    pub fn apply_watch_events(
        &mut self,
        pane: PaneId,
        events: &[naygo_core::listing::DirEvent],
    ) -> Vec<std::path::PathBuf> {
        // Espejo runtime del setting "agrupar al final" (se lee antes del préstamo mutable).
        let group_new_at_end = self.config.settings.new_items_at_end;
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
        // Empujar el flag ANTES de calcular posiciones: si está activo, la vista pone los nuevos al
        // final y la selección debe apuntar a esas posiciones finales.
        f.group_new_at_end = group_new_at_end;
        // Resaltar + seleccionar los recién llegados (MEJORA 1). Los que estén ocultos por
        // filtro/visibilidad se resaltan pero no se seleccionan (no están en la vista).
        f.select_arrivals(&nuevas);
        nuevas
    }

    /// Sincroniza el conjunto `highlighted` de cada panel Files con el set autoritativo de rutas
    /// "frescas" del watcher (las que siguen vigentes según `highlight_secs`). Lo llama el tick
    /// tras `prune`: cuando a una ruta se le vence el resaltado, se quita de `highlighted` y, si
    /// estaba "agrupada al final", vuelve a su posición ordenada en la próxima vista. Mantiene una
    /// única fuente de verdad (el watcher) entre el resaltado ámbar y el agrupar-al-final.
    pub fn sync_highlighted_from_watchers(&mut self, highlight_secs: u64, now: std::time::Instant) {
        let WorkspaceCtrl { ws, watchers, .. } = self;
        for pane in ws.panes_mut() {
            let id = pane.id.0;
            if let Some(f) = pane.files.as_mut() {
                f.sync_highlighted(|p| watchers.is_fresh_ro(id, p, highlight_secs, now));
            }
        }
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

    /// Índice de VISTA de la fila enfocada del panel `id`, o -1 si no hay foco / no es un panel
    /// Files. Lo consume el builder del PaneVm: la UI lo observa (`changed focused-row`) para que
    /// el scroll del listado siga a la fila enfocada al navegar por teclado (la ListView ya no es
    /// interactiva y no auto-scrollea sola, ver C1).
    pub fn focused_view_of(&self, id: PaneId) -> i32 {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .and_then(|f| f.focused)
            .map(|i| i as i32)
            .unwrap_or(-1)
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
        // Navegar (o refrescar) puede cambiar de unidad: invalida la caché de disco del footer
        // para que el espacio libre/total se relea. Es pequeña; se repuebla a demanda por tick.
        self.footer_disk_cache.clear();
        self.listings.insert(id, Listing::start(dir));
    }

    /// Invalida la caché de disco del footer. La llama main.rs cuando cambian los dispositivos
    /// (USB conectado/expulsado, vía `on_wake`/`drives_changed`), para que el espacio libre del
    /// footer siga esos eventos aunque el panel no haya navegado.
    pub fn invalidate_footer_disk_cache(&mut self) {
        self.footer_disk_cache.clear();
    }

    /// El preset de footer EFECTIVO: si el guardado es `Custom`, usa el template del usuario
    /// (`footer_custom_template`), que es donde vive realmente (la variante puede traer string
    /// vacío). Para el resto, devuelve el preset tal cual.
    fn footer_preset_resolved(&self) -> naygo_core::footer::FooterPreset {
        match &self.config.settings.footer_preset {
            naygo_core::footer::FooterPreset::Custom(_) => {
                naygo_core::footer::FooterPreset::Custom(
                    self.config.settings.footer_custom_template.clone(),
                )
            }
            other => other.clone(),
        }
    }

    /// Texto del footer (barra inferior) para el panel `id`. Vacío si el footer está deshabilitado
    /// en Settings o el panel no es de archivos. Lo invoca `sync_rows` por tick. Toma `&mut self`
    /// por la caché de disco (`footer_disk_cache`): primero lee los datos crudos del panel bajo un
    /// borrow inmutable de `self.ws`, lo suelta, y recién entonces toca la caché y renderiza. Así
    /// no chocan el `&self.ws` y el `&mut self.footer_disk_cache`.
    pub fn footer_text_of(&mut self, id: PaneId) -> String {
        if !self.config.settings.footer_enabled {
            return String::new();
        }
        // Paso 1: extraer datos crudos del panel (borrow inmutable acotado a este bloque).
        let Some((data_no_disk, root)) = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(footer_inputs_of)
        else {
            return String::new();
        };
        // Paso 2 (ya sin borrow de `self.ws`): uso de disco cacheado por raíz de unidad.
        let disk = match root {
            Some(root) => match self.footer_disk_cache.get(&root) {
                Some(d) => Some(*d),
                None => {
                    let d = disk_usage(&root);
                    if let Some(u) = d {
                        self.footer_disk_cache.insert(root, u);
                    }
                    d
                }
            },
            None => None,
        };
        let data = naygo_core::footer::FooterData {
            disk,
            ..data_no_disk
        };
        let preset = self.footer_preset_resolved();
        naygo_core::footer::render(&preset, &data, self.config.settings.size_format)
    }

    /// Drena los lotes de TODOS los listados activos. Devuelve true si TODOS terminaron
    /// (para apagar el timer). Quita del mapa los que terminan.
    pub fn pump_listings(&mut self) -> bool {
        // Flags de visibilidad globales: se empujan al panel al sembrar sus entries, así un
        // panel recién listado (arranque, navegación, refresh) ya nace con la vista filtrada
        // correcta sin depender de que el usuario abra el menú "ojo".
        let vis = self.visibility_flags();
        // Espejo del setting "archivos nuevos al final": se empuja en el mismo punto, así un panel
        // recién listado refleja la opción aunque el watcher aún no haya disparado.
        let group_new_at_end = self.config.settings.new_items_at_end;
        let ids: Vec<PaneId> = self.listings.keys().copied().collect();
        for id in ids {
            // El flag `fresh` se reclama solo cuando este poll trae avance real: o llegaron
            // entries, o el listado TERMINÓ (carpeta vacía → done sin lotes). Un tick que poll-ea
            // vacío y sin terminar NO lo consume, así el reemplazo de filas se aplica recién con
            // el primer avance real. `take_fresh()` solo devuelve `true` una vez por listado.
            let (batch, done, fresh) = match self.listings.get_mut(&id) {
                Some(l) => {
                    let (b, d) = l.poll();
                    let fresh = if b.is_empty() && !d {
                        false
                    } else {
                        l.take_fresh()
                    };
                    (b, d, fresh)
                }
                None => continue,
            };
            let batch_was_empty = batch.is_empty();
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.set_visibility(vis);
                f.group_new_at_end = group_new_at_end;
                if !batch_was_empty {
                    // Primer lote de un listado nuevo: REEMPLAZAR (no acumular) las entries que
                    // el panel tuviera de antes. Esto evita que F5 duplique las filas al
                    // re-listar el mismo directorio sobre un panel ya poblado.
                    if fresh {
                        f.entries.clear();
                    }
                    f.entries.extend(batch);
                }
                if done {
                    // Carpeta que quedó VACÍA tras refrescar: el listado nuevo no emitió ningún
                    // lote, así que `fresh` nunca se reclamó y las entries viejas seguirían ahí.
                    // Al terminar, si aún estaba fresco, vaciar para reflejar la carpeta vacía.
                    if fresh && batch_was_empty {
                        f.entries.clear();
                    }
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

    /// Panel FILES que está bajo el punto `(content_x, content_y)` (coords de contenido, el mismo
    /// sistema que usan `pane_rects`/`drop_hit`/`drop_at`). Reusa el hit-testing del docking. Solo
    /// devuelve paneles Files: si el punto cae sobre un panel auxiliar (Árbol/Inspector/Preview/…)
    /// o fuera de todo panel, devuelve `None`. Lo usa la UI para resaltar EN VIVO el panel bajo el
    /// cursor mientras se arrastran archivos (mismo destino que recibiría `drop_at`). No ejecuta
    /// nada ni muta estado: es un puro hit-test.
    pub fn pane_at(&self, content_x: f32, content_y: f32) -> Option<PaneId> {
        use naygo_core::workspace::layout::drop_hit;
        let panes = self.pane_rects(self.last_area);
        let (target, _zone) = drop_hit(&panes, content_x, content_y)?;
        // Filtrar a paneles Files: el resaltado de drop solo aplica donde se puede soltar.
        self.ws
            .pane(target)
            .and_then(|p| p.files.as_ref())
            .map(|_| target)
    }

    /// Fija el panel resaltado por arrastre (hover de drop). Devuelve `true` si CAMBIÓ respecto del
    /// valor anterior (la UI solo re-pinta cuando cambia, para no inundar con cada `DragOver`).
    pub fn set_drag_over(&mut self, pane: Option<PaneId>) -> bool {
        if self.drag_over_pane != pane {
            self.drag_over_pane = pane;
            true
        } else {
            false
        }
    }

    /// El panel actualmente resaltado por arrastre, si lo hay. Lo lee `sync_rows` para poblar el
    /// `drag-over` de cada `PaneVm`.
    pub fn drag_over_pane(&self) -> Option<PaneId> {
        self.drag_over_pane
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
    /// Los tres flags de visibilidad actuales, leídos de los settings. Los usan el panel
    /// (`rows_of`/`rows_from_view`), la vista profunda y el filtrado del árbol (`pump_tree`)
    /// para llamar a `naygo_core::filter::is_visible`. Globales y persistentes.
    pub fn visibility_flags(&self) -> VisibilityFlags {
        VisibilityFlags {
            show_hidden: self.config.settings.show_hidden,
            show_system: self.config.settings.show_system,
            hide_dotfiles: self.config.settings.hide_dotfiles,
        }
    }

    pub fn rows_of(
        &mut self,
        id: PaneId,
        highlight_secs: u64,
        now: std::time::Instant,
    ) -> Vec<PlainRow> {
        let date_format = self.config.settings.date_format;
        let size_format = self.config.settings.size_format;
        let vis = self.visibility_flags();
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
                .filter(|(e, _)| {
                    naygo_core::filter::is_visible(
                        e,
                        vis.show_hidden,
                        vis.show_system,
                        vis.hide_dotfiles,
                    )
                })
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
            PanePurpose::Operations => "Operaciones".to_string(),
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

    /// Asegura que exista un panel de Operaciones en el layout; si ya hay uno, no-op. Se llama
    /// al iniciar una operación larga para que el panel rico de progreso "aparezca solo" sin que
    /// el usuario tenga que abrirlo. A diferencia de `add_pane_of`, NO roba el foco: el panel
    /// Files activo sigue activo (el usuario estaba operando ahí). El usuario puede cerrarlo.
    pub fn ensure_ops_pane(&mut self) {
        if self.has_purpose(PanePurpose::Operations) {
            return;
        }
        crate::logging::breadcrumb("auto-aparecer panel de operaciones");
        // Recordar el activo para restaurarlo (no robar foco al panel donde se opera).
        let prev_active = self.ws.active_id();
        let dir = self
            .ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| PathBuf::from("C:/"));
        let active = self.ws.active_id();
        let new_id = self.ws.add_pane(PanePurpose::Operations, dir);
        if let Some(active) = active {
            self.ws
                .layout
                .split_leaf(active, SplitDir::Horizontal, new_id);
        }
        // Restaurar el activo previo (el panel de Operaciones no toma el foco).
        if let Some(prev) = prev_active {
            self.set_active(prev);
        }
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

    /// Reglas de preview actuales (espejo para la UI): (extensión, habilitada, índice de modo de
    /// vista, índice de lenguaje). El modo de vista es 0=Auto 1=Texto 2=Imagen 3=Código; el índice
    /// de lenguaje es la posición en `CodeLang::all()` (0 si el modo no es Código).
    pub fn preview_rules(&self) -> Vec<(String, bool, i32, i32)> {
        use naygo_core::preview::{CodeLang, ViewMode};
        self.config
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
        self.ensure_ops_pane();
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

    /// Persiste el árbol de favoritos a disco. Se llama tras CADA mutación (anclar/quitar,
    /// nuevo grupo, renombrar, eliminar, mover) para no perder cambios si la app cae.
    fn persist_favorites(&self) {
        save_favorites(&self.config.config_dir, &self.favorites);
    }

    /// Quita un favorito por ruta (y persiste).
    pub fn remove_favorite(&mut self, path: &Path) {
        self.favorites.remove(path);
        self.persist_favorites();
    }

    /// Alterna el favorito de la carpeta del panel Files activo (estrella; persiste).
    pub fn toggle_favorite_active(&mut self) {
        if let Some(dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
            self.favorites.toggle(&dir);
            self.persist_favorites();
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

    /// Alterna el favorito de la carpeta del panel `id` (botón ★ de la path-bar; persiste).
    pub fn toggle_favorite_dir(&mut self, id: PaneId) {
        if let Some(dir) = self
            .ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        {
            self.favorites.toggle(&dir);
            self.persist_favorites();
        }
    }

    // --- Árbol de favoritos editable (panel + menú ▾ del toolbar) ---

    /// Filas del árbol de favoritos (aplanadas con sangría, grupos colapsables). Lleva el ícono
    /// de carpeta del set activo. Las usa tanto el panel editable como el menú ▾ del toolbar.
    pub fn fav_tree_rows(&mut self) -> Vec<FavTreeRow> {
        let folder = self.icons.get(naygo_core::icon_kind::IconKey::Folder);
        let expanded = &self.fav_expanded;
        fav_tree_rows(&self.favorites, &|np| expanded.contains(np), &folder)
    }

    /// Expande/colapsa un grupo del árbol de favoritos por su ruta de nombres ("Trabajo/Sub").
    /// Estado de UI puro (no persiste): solo cambia qué se muestra.
    pub fn fav_toggle_expand(&mut self, name_path: &str) {
        if !self.fav_expanded.remove(name_path) {
            self.fav_expanded.insert(name_path.to_string());
        }
    }

    /// Crea un grupo nuevo. `parent_group_id` es el GroupId serializado ("0/2") del grupo padre,
    /// o cadena vacía para crearlo en la raíz. El grupo nace con `name` y se persiste. Tras crear,
    /// expande al padre para que el grupo nuevo sea visible.
    pub fn fav_new_group(&mut self, parent_group_id: &str, name: &str) {
        let name = name.trim();
        let name = if name.is_empty() { "Nuevo grupo" } else { name };
        let parent = if parent_group_id.is_empty() {
            None
        } else {
            Some(str_to_group_id(parent_group_id))
        };
        // Si el padre existe, expandirlo (por su ruta de nombres) para revelar el hijo nuevo.
        if let Some(pid) = parent.as_ref() {
            if let Some(np) = self.fav_group_name_path(pid) {
                self.fav_expanded.insert(np);
            }
        }
        self.favorites.new_group(parent.as_ref(), name);
        self.persist_favorites();
    }

    /// Renombra el grupo identificado por su GroupId serializado. La clave de expansión cambia
    /// (es por nombre), así que se re-mapea el estado expandido del subárbol para no "colapsar"
    /// visualmente el grupo renombrado.
    pub fn fav_rename_group(&mut self, group_id: &str, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let id = str_to_group_id(group_id);
        let old_np = self.fav_group_name_path(&id);
        self.favorites.rename_group(&id, name);
        self.persist_favorites();
        // Re-mapear las claves de expansión que colgaban del nombre viejo al nuevo.
        if let (Some(old), Some(new)) = (old_np, self.fav_group_name_path(&id)) {
            if old != new {
                let affected: Vec<String> = self
                    .fav_expanded
                    .iter()
                    .filter(|k| **k == old || k.starts_with(&format!("{old}/")))
                    .cloned()
                    .collect();
                for k in affected {
                    self.fav_expanded.remove(&k);
                    let suffix = &k[old.len()..]; // "" o "/resto"
                    self.fav_expanded.insert(format!("{new}{suffix}"));
                }
            }
        }
    }

    /// Elimina un nodo del árbol de favoritos. `is_group` decide el identificador: grupo por
    /// GroupId serializado, favorito por ruta. Persiste. (Un grupo se borra con todo su subárbol.)
    pub fn fav_delete_node(&mut self, is_group: bool, group_id: &str, path: &str) {
        if is_group {
            let id = str_to_group_id(group_id);
            self.favorites.remove_group(&id);
        } else {
            self.favorites.remove(Path::new(path));
        }
        self.persist_favorites();
    }

    /// Mueve un nodo a un grupo destino. `dest_group_id` vacío = raíz. El nodo origen se
    /// identifica por GroupId (grupo) o ruta (favorito). Persiste.
    pub fn fav_move_node(
        &mut self,
        is_group: bool,
        src_group_id: &str,
        src_path: &str,
        dest_group_id: &str,
    ) {
        let dest = if dest_group_id.is_empty() {
            None
        } else {
            Some(str_to_group_id(dest_group_id))
        };
        let node = if is_group {
            NodeId::group(str_to_group_id(src_group_id))
        } else {
            NodeId::favorite(Path::new(src_path))
        };
        self.favorites.move_node(&node, dest.as_ref());
        self.persist_favorites();
    }

    /// Lista de grupos existentes para el submenú "Mover a…": pares (ruta de nombres, GroupId
    /// serializado). El primer elemento NO es la raíz (la UI agrega "Raíz" aparte). Preorden.
    pub fn fav_group_options(&self) -> Vec<(String, String)> {
        fn walk(
            nodes: &[FavNode],
            parent_id: &[usize],
            parent_np: &str,
            out: &mut Vec<(String, String)>,
        ) {
            for (i, n) in nodes.iter().enumerate() {
                if let FavNode::Group { name, children } = n {
                    let mut id = parent_id.to_vec();
                    id.push(i);
                    let np = if parent_np.is_empty() {
                        name.clone()
                    } else {
                        format!("{parent_np}/{name}")
                    };
                    out.push((np.clone(), crate::bridge::group_id_to_str(&id)));
                    walk(children, &id, &np, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self.favorites.roots(), &[], "", &mut out);
        out
    }

    /// Opciones de destino VÁLIDAS para "Mover a…" del nodo seleccionado. Parte de
    /// `fav_group_options` y, cuando el nodo movido es un GRUPO (`is_group`), excluye los destinos
    /// inválidos: el propio grupo y todos sus descendientes (un grupo no se mueve dentro de sí
    /// mismo). El gid llega serializado ("0/2"); un destino `d` es inválido si `d == origen` o si
    /// `d` es descendiente de `origen` (su ruta de índices empieza con la de `origen`). Para un
    /// favorito (hoja) no hay restricción: cualquier grupo es destino válido.
    pub fn fav_move_targets(&self, is_group: bool, src_gid: &str) -> Vec<(String, String)> {
        let all = self.fav_group_options();
        if !is_group {
            return all;
        }
        let origin = str_to_group_id(src_gid);
        all.into_iter()
            .filter(|(_, gid)| {
                // `starts_with` cubre también la igualdad: el propio grupo y todos sus descendientes
                // (cuya ruta de índices empieza con la del origen) quedan excluidos como destino.
                !str_to_group_id(gid).starts_with(&origin[..])
            })
            .collect()
    }

    /// Ruta de nombres ("Trabajo/Sub") del grupo en `id`, o `None` si no existe / no es grupo.
    /// Sirve para mantener el estado de expansión (que se indexa por nombre, no por índice).
    fn fav_group_name_path(&self, id: &[usize]) -> Option<String> {
        let mut nodes = self.favorites.roots();
        let mut np = String::new();
        for (depth, &idx) in id.iter().enumerate() {
            let node = nodes.get(idx)?;
            match node {
                FavNode::Group { name, children } => {
                    if np.is_empty() {
                        np = name.clone();
                    } else {
                        np = format!("{np}/{name}");
                    }
                    if depth + 1 == id.len() {
                        return Some(np);
                    }
                    nodes = children;
                }
                FavNode::Favorite { .. } => return None,
            }
        }
        None
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
                self.ensure_ops_pane();
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
        // Diagnóstico: este es el camino de FALLBACK (el drop no se pudo enrutar por el punto y
        // cayó al panel activo). Si aquí el destino coincide con el origen, el resultado es un
        // no-op silencioso — clave para diagnosticar drops "que no hacen nada".
        crate::logging::breadcrumb(&format!(
            "drop_external (fallback panel activo): {} {} ítem(s) → {}",
            label,
            sources.len(),
            dir.display(),
        ));
        let req = naygo_core::ops::transfer(move_, sources, dir);
        self.ensure_ops_pane();
        self.ops.start_op(req, label.to_string(), true);
        true
    }

    /// Recibe un drop OLE en el PUNTO `(content_x, content_y)` (coordenadas de contenido, el
    /// mismo sistema que usa `pane_rects`/`drop_hit`): enruta al panel Files que está BAJO el
    /// cursor, no al panel activo. Decide mover/copiar con las reglas del Explorador
    /// (`decide_drop_action`: Shift→mover, Ctrl→copiar, si no según mismo disco); el `move_hint`
    /// del OLE es secundario (Ctrl/Shift + mismo disco mandan, igual que en `drop_external`).
    ///
    /// No-op (devuelve false) si: no hay rutas, el punto no cae sobre ningún panel, el panel
    /// destino no es Files, o el destino ES la misma carpeta de origen de las rutas (soltar
    /// sobre la propia carpeta). Devuelve true si arrancó la operación.
    pub fn drop_at(
        &mut self,
        content_x: f32,
        content_y: f32,
        ctrl: bool,
        shift: bool,
        paths: Vec<std::path::PathBuf>,
        move_hint: bool,
    ) -> bool {
        use naygo_core::dnd::{decide_drop_action, same_drive, DropAction};
        use naygo_core::workspace::layout::drop_hit;
        if paths.is_empty() {
            crate::logging::breadcrumb("drop_at: sin rutas, no-op");
            return false;
        }
        // Panel bajo el cursor, reusando la maquinaria de hit-testing del docking. `last_area`
        // es el área de contenido que la UI mantiene actualizada con `set_area`.
        let panes = self.pane_rects(self.last_area);
        let hit = drop_hit(&panes, content_x, content_y);
        // Diagnóstico: coords (ya en sistema de contenido), nº de rutas, move_ y a qué panel
        // (índice de orden visual en `panes`) acertó el hit-testing — o None si cayó fuera.
        let hit_idx = hit
            .as_ref()
            .and_then(|(id, _)| panes.iter().position(|(pid, _)| pid == id));
        let hit_idx_str = hit_idx
            .map(|i| i.to_string())
            .unwrap_or_else(|| "None".to_string());
        crate::logging::breadcrumb(&format!(
            "drop_at: content=({:.1},{:.1}) rutas={} move_hint={} → panel={}",
            content_x,
            content_y,
            paths.len(),
            move_hint,
            hit_idx_str,
        ));
        let Some((target, _zone)) = hit else {
            return false;
        };
        // El destino debe ser un panel Files con carpeta resoluble.
        let Some(dest_dir) = self
            .ws
            .pane(target)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
        else {
            crate::logging::breadcrumb("drop_at: el panel destino no es Files, no-op");
            return false;
        };
        // Soltar sobre la propia carpeta de origen es no-op: si todas las rutas ya viven en
        // `dest_dir`, no hay nada que copiar/mover. (Comparar el padre de cada ruta con el
        // destino; basta con que alguna venga de otra carpeta para proceder.)
        let all_from_dest = paths.iter().all(|p| {
            p.parent()
                .map(|par| par == dest_dir.as_path())
                .unwrap_or(false)
        });
        if all_from_dest {
            crate::logging::breadcrumb("drop_at: soltado sobre la propia carpeta, no-op");
            return false;
        }
        // Acción según modificadores + mismo disco. CLAVE: el `move_hint` del OLE viene del
        // grfKeyState que Windows entrega al SOLTAR (refleja el Shift REAL en ese instante).
        // Los flags `ctrl`/`shift` de la app NO sirven aquí: durante el bucle modal de
        // DoDragDrop la app no recibe eventos de teclado, así que llegan desactualizados (false)
        // aunque el usuario tenga Shift presionado. Por eso, si el OLE reporta Shift
        // (`move_hint`), MOVEMOS; si no, caemos a decide_drop_action (default por disco + Ctrl).
        let same = same_drive(&paths[0], &dest_dir);
        let is_move =
            move_hint || matches!(decide_drop_action(ctrl, shift, same), DropAction::Move);
        let label = if is_move { "Mover" } else { "Copiar" };
        // CONFIRMAR AL SOLTAR (decisión de Nicolás): NO ejecutamos la op aquí. Guardamos el drop ya
        // validado en `pending_drop` y devolvemos true; la UI abre un modal "¿Copiar/Mover N a
        // «destino»?" y, al confirmar, llama a `confirm_pending_drop` que arranca la op de verdad.
        // Así un arrastre accidental entre paneles no copia/mueve archivos sin que el usuario lo vea.
        let count = paths.len();
        crate::logging::breadcrumb(&format!(
            "drop_at: {} {} ítem(s) → {} (pendiente de confirmar)",
            label,
            count,
            dest_dir.display(),
        ));
        self.pending_drop = Some(PendingDrop {
            paths,
            dest_dir,
            is_move,
            count,
        });
        // Confirmación opcional (decisión de Nicolás): si el ajuste está APAGADO, no abrimos el
        // modal "¿Copiar/Mover…?"; ejecutamos directo reusando `confirm_pending_drop` (que arranca
        // la op y CONSUME `pending_drop`). El modal de CONFLICTO (archivo que ya existe) es
        // independiente y SIEMPRE aparece: lo dispara `pump_ops`, no esta confirmación.
        //
        // Devolvemos `true` en AMBOS casos (el drop fue ENRUTADO y manejado por este panel, así la
        // UI no cae al fallback `drop_external`). Quién decide abrir el modal es el llamador, que
        // lee `pending_drop` DESPUÉS: con la confirmación ON queda `Some` → abre el modal; con la
        // confirmación OFF ya lo consumió `confirm_pending_drop` y queda `None` → no abre nada.
        if !self.config.settings.confirm_drop_between_panes {
            crate::logging::breadcrumb(
                "drop_at: confirmación de drop desactivada → ejecutar directo",
            );
            self.confirm_pending_drop();
        }
        true
    }

    /// El usuario CONFIRMÓ el drop pendiente (botón Copiar/Mover del modal): arranca la op real.
    /// Devuelve true si había un drop pendiente y se lanzó. No-op (false) si no había ninguno.
    pub fn confirm_pending_drop(&mut self) -> bool {
        let Some(pd) = self.pending_drop.take() else {
            return false;
        };
        let label = if pd.is_move { "Mover" } else { "Copiar" };
        crate::logging::breadcrumb(&format!(
            "confirm_pending_drop: {} {} ítem(s) → {}",
            label,
            pd.count,
            pd.dest_dir.display(),
        ));
        let req = naygo_core::ops::transfer(pd.is_move, pd.paths, pd.dest_dir);
        self.ensure_ops_pane();
        self.ops.start_op(req, label.to_string(), true);
        true
    }

    /// El usuario CANCELÓ el drop pendiente (botón Cancelar / Esc / clic fuera): lo descarta sin
    /// copiar ni mover nada.
    pub fn cancel_pending_drop(&mut self) {
        if self.pending_drop.take().is_some() {
            crate::logging::breadcrumb("cancel_pending_drop: drop descartado por el usuario");
        }
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

    /// Propaga los flags de visibilidad actuales (de los settings) a TODOS los paneles Files y
    /// re-arma TODOS los árboles. Lo invocan los toggles del menú "ojo".
    ///
    /// Paneles Files: cada `FilePaneState` guarda su propio espejo de los flags y los aplica
    /// DENTRO de `compute_view_indices`, de modo que la VISTA, la selección, el foco y el
    /// teclado compartan el mismo conjunto filtrado. Por eso hay que empujar los flags aquí (no
    /// basta filtrar al pintar). Si la selección/foco quedan fuera de rango al esconder filas,
    /// `set_visibility` los reacomoda.
    ///
    /// Árboles: las subcarpetas se filtran al LISTARSE (en `pump_tree`), pero las ya cargadas
    /// conservan lo que se listó con los flags anteriores. Aquí re-listamos cada rama
    /// actualmente expandida (con hijos cargados): se cancela su worker si quedara alguno, se
    /// vacía y se vuelve a lanzar el listado solo-dirs. El reveal de la carpeta activa se mantiene.
    pub fn refresh_trees_visibility(&mut self) {
        // Empujar los flags a cada panel Files (el árbol filtra aparte en `pump_tree`).
        let vis = self.visibility_flags();
        for pane in self.ws.panes_mut() {
            if let Some(f) = pane.files.as_mut() {
                f.set_visibility(vis);
            }
        }
        let ids: Vec<PaneId> = self.trees.keys().copied().collect();
        for id in ids {
            // Ramas expandidas y ya cargadas (las que muestran hijos): hay que re-listarlas.
            let to_reload: Vec<PathBuf> = match self.trees.get(&id) {
                Some(t) => t
                    .flat_paths()
                    .into_iter()
                    .filter(|p| {
                        t.node_at(p)
                            .map(|n| n.expanded && n.children.is_some())
                            .unwrap_or(false)
                    })
                    .collect(),
                None => continue,
            };
            for path in to_reload {
                // Cancelar un worker en vuelo de esa rama (si lo hubiera) para no mezclar lotes.
                if let Some(l) = self.tree_listings.remove(&(id, path.clone())) {
                    l.cancel();
                }
                if let Some(t) = self.trees.get_mut(&id) {
                    // Vacía los hijos y marca Loading+expandido: el nuevo lote (ya filtrado en
                    // `pump_tree`) los repuebla.
                    t.begin_loading(&path);
                }
                self.tree_listings
                    .insert((id, path.clone()), Listing::start_dirs_only(path));
            }
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
        // Flags de visibilidad: el árbol esconde las subcarpetas ocultas/sistema/dotfile
        // igual que el panel. Se leen una vez (los `push_child` van detrás de `get_mut`).
        let vis = self.visibility_flags();
        let keys: Vec<(PaneId, PathBuf)> = self.tree_listings.keys().cloned().collect();
        for key in keys {
            let (batch, done) = match self.tree_listings.get(&key) {
                Some(l) => l.poll(),
                None => continue,
            };
            let (id, parent) = (key.0, &key.1);
            if let Some(t) = self.trees.get_mut(&id) {
                for e in batch {
                    // Filtro de visibilidad: una subcarpeta entra al árbol solo si pasa
                    // `is_visible` (oculta/sistema/dotfile según los flags). `TreeNode` no
                    // guarda esos atributos, así que el filtro va aquí, donde aún tenemos el
                    // `Entry` con `hidden`/`system`.
                    if !naygo_core::filter::is_visible(
                        &e,
                        vis.show_hidden,
                        vis.show_system,
                        vis.hide_dotfiles,
                    ) {
                        continue;
                    }
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
        // El preview sigue la selección del ÚLTIMO panel Files activo (no del panel activo a
        // secas): así, hacer clic en el propio panel de Vista previa (o en el Inspector) no
        // vacía la previsualización — el archivo seleccionado se conserva. Fallback al panel
        // activo si todavía no hay un Files recordado.
        let files_pane = self
            .last_active_files
            .and_then(|id| self.ws.pane(id))
            .and_then(|p| p.files.as_ref())
            .or_else(|| self.ws.active_files());
        let focused_file = files_pane
            .and_then(|f| f.focused_view_entry())
            .filter(|e| e.kind != EntryKind::Directory)
            .map(|e| e.path.clone());
        self.preview.set_wanted(focused_file, now);
        // El toggle global de auto-resaltado controla al worker: se sincroniza antes de lanzarlo.
        self.preview
            .set_auto_highlight(self.config.settings.auto_highlight_code);
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

    /// Navega el panel Files activo a la carpeta Home configurada (vacío = perfil del usuario).
    /// Registra en el historial igual que cualquier navegación normal. Devuelve true si navegó.
    pub fn on_go_home(&mut self) -> bool {
        let home = naygo_core::config::resolve_home_dir(&self.config.settings.home_dir);
        crate::logging::breadcrumb(&format!("home → {}", home.display()));
        self.navigate_active_to(home)
    }

    /// ¿El panel activo puede ir Atrás? Lo consumen los botones Atrás/Adelante del toolbar
    /// (main.rs los lee tras cada navegación para habilitarlos/deshabilitarlos).
    pub fn can_go_back(&self) -> bool {
        self.ws
            .active_files()
            .map(|f| f.can_go_back())
            .unwrap_or(false)
    }

    /// ¿El panel activo puede ir Adelante? Mismo consumidor que `can_go_back`.
    pub fn can_go_forward(&self) -> bool {
        self.ws
            .active_files()
            .map(|f| f.can_go_forward())
            .unwrap_or(false)
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

    /// Rutas hacia ATRÁS del panel activo, de la más cercana a la más lejana (para el menú ▾ del
    /// botón Atrás). Vacío si no hay panel Files activo o no hay historial atrás.
    pub fn back_history_entries(&self) -> Vec<std::path::PathBuf> {
        self.ws
            .active_files()
            .map(|f| f.history.back_entries())
            .unwrap_or_default()
    }

    /// Rutas hacia ADELANTE del panel activo, de la más cercana a la más lejana (menú ▾ del botón
    /// Adelante). Vacío si no hay panel Files activo o no hay historial adelante.
    pub fn forward_history_entries(&self) -> Vec<std::path::PathBuf> {
        self.ws
            .active_files()
            .map(|f| f.history.forward_entries())
            .unwrap_or_default()
    }

    /// Salta el panel activo a la entrada `menu_index` del menú ▾ de ATRÁS (0 = la más cercana).
    /// Traduce el índice del menú al índice de la pila usando el cursor actual del NavHistory:
    /// la entrada `i` de `back_entries` está en la posición `cursor - 1 - i` de la pila. Así la UI
    /// no maneja aritmética de índices. Devuelve `true` si navegó.
    pub fn go_back_history(&mut self, menu_index: usize) -> bool {
        let Some(stack_index) = self.stack_index_back(menu_index) else {
            return false;
        };
        self.go_to_history(stack_index)
    }

    /// Salta el panel activo a la entrada `menu_index` del menú ▾ de ADELANTE (0 = la más cercana).
    /// La entrada `i` de `forward_entries` está en la posición `cursor + 1 + i` de la pila.
    /// Devuelve `true` si navegó.
    pub fn go_forward_history(&mut self, menu_index: usize) -> bool {
        let Some(stack_index) = self.stack_index_forward(menu_index) else {
            return false;
        };
        self.go_to_history(stack_index)
    }

    /// Índice en la pila de la entrada `menu_index` del menú de ATRÁS del panel activo. `None` si
    /// no hay panel/cursor o el índice cae fuera de la rama de atrás.
    fn stack_index_back(&self, menu_index: usize) -> Option<usize> {
        let (_, cursor) = self.ws.active_files()?.history.stack();
        let cursor = cursor?;
        // back_entries va de cercano (cursor-1) a lejano (0): entrada i ↦ cursor-1-i.
        cursor.checked_sub(1)?.checked_sub(menu_index)
    }

    /// Índice en la pila de la entrada `menu_index` del menú de ADELANTE del panel activo. `None`
    /// si no hay panel/cursor o el índice cae fuera de la rama de adelante.
    fn stack_index_forward(&self, menu_index: usize) -> Option<usize> {
        let (stack, cursor) = self.ws.active_files()?.history.stack();
        let cursor = cursor?;
        // forward_entries va de cercano (cursor+1) a lejano (len-1): entrada i ↦ cursor+1+i.
        let idx = cursor + 1 + menu_index;
        (idx < stack.len()).then_some(idx)
    }

    /// Salta el panel activo a una entrada de su historial por índice en la pila (menú ▾ de los
    /// botones Atrás/Adelante). Mueve el cursor del NavHistory y navega SIN re-apilar (como
    /// atrás/adelante: `f.go_to_history` usa `history.jump_to`, que solo mueve el cursor).
    /// Devuelve `true` si navegó.
    pub fn go_to_history(&mut self, stack_index: usize) -> bool {
        let Some(active) = self.active_files_id() else {
            return false;
        };
        let moved = self
            .ws
            .pane_mut(active)
            .and_then(|p| p.files.as_mut())
            .and_then(|f| f.go_to_history(stack_index));
        match moved {
            Some(dir) => {
                crate::logging::breadcrumb("historial: salto directo");
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

/// Datos crudos del footer de un panel (todo MENOS el disco) + la raíz de su unidad (para
/// cachear el disco). Puro sobre el `FilePaneState`: lo llama `footer_text_of` bajo un borrow
/// inmutable y acotado, así el `&mut self` de la caché no choca con el `&self.ws`.
fn footer_inputs_of(
    files: &FilePaneState,
) -> (naygo_core::footer::FooterData, Option<std::path::PathBuf>) {
    // Selección: `files.selected` son posiciones de VISTA; se mapean a `entries` vía la vista.
    let view = files.view_indices();
    let sel_count = files.selected.len();
    let total_count = view.len();
    let marked_bytes: u64 = files
        .selected
        .iter()
        .filter_map(|&pos| view.get(pos))
        .filter_map(|&real| files.entries.get(real))
        .filter_map(|e| e.size)
        .sum();
    // Conteo de archivos/carpetas sobre TODO el listado (no solo la vista filtrada).
    let dir_count = files.entries.iter().filter(|e| e.is_dir()).count();
    let file_count = files.entries.len() - dir_count;
    let item_count = file_count + dir_count;
    // Raíz de la unidad de la carpeta actual (p. ej. `C:\`), para cachear el disco por unidad.
    let root = files
        .current_dir
        .ancestors()
        .last()
        .map(std::path::Path::to_path_buf);
    let data = naygo_core::footer::FooterData {
        sel_count,
        total_count,
        marked_bytes,
        disk: None,
        item_count,
        file_count,
        dir_count,
    };
    (data, root)
}

/// Uso de disco de una unidad (total/libre), o `None` si no se puede leer (red caída,
/// óptico vacío). De ahí salen tanto la barrita (% usado) como el texto de espacio.
fn disk_usage(root: &Path) -> Option<naygo_core::disk::DiskUsage> {
    let (total, free) = naygo_platform::drive_space::read_space(root)?;
    Some(naygo_core::disk::DiskUsage { total, free })
}

/// Carga tolerante del árbol de favoritos desde `<config>/favorites.json`. Ausente o corrupto →
/// árbol vacío (nunca cae la app; `Favorites::from_json` ya migra el formato plano antiguo).
fn load_favorites(config_dir: &Path) -> Favorites {
    let path = favorites_path(config_dir);
    match std::fs::read_to_string(&path) {
        Ok(s) => Favorites::from_json(&s),
        Err(_) => Favorites::new(),
    }
}

/// Persiste el árbol de favoritos a `<config>/favorites.json` (pretty, diminuto). Crea la
/// carpeta de config si falta. Un fallo de escritura se registra y se ignora (no es fatal: el
/// estado en memoria sigue siendo válido para la sesión).
fn save_favorites(config_dir: &Path, favorites: &Favorites) {
    let path = favorites_path(config_dir);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, favorites.to_json()) {
        crate::logging::log_line(&format!(
            "guardar favoritos {}: fallo — {e}",
            path.display()
        ));
    }
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
        // Agregar una extensión nueva (normaliza el punto y mayúsculas). Nace en Auto (modo 0).
        c.preview_rule_add(".SIF");
        assert!(c
            .preview_rules()
            .iter()
            .any(|(e, on, v, _)| e == "sif" && *on && *v == 0));
        // No duplica.
        c.preview_rule_add("sif");
        assert_eq!(
            c.preview_rules()
                .iter()
                .filter(|(e, _, _, _)| e == "sif")
                .count(),
            1
        );
        // Forzar el modo a Código + lenguaje XML (índice 0 en CodeLang::all()).
        c.preview_rule_set_view_mode("sif", 3);
        c.preview_rule_set_view_lang("sif", 0);
        assert!(c
            .preview_rules()
            .iter()
            .any(|(e, _, v, l)| e == "sif" && *v == 3 && *l == 0));
        // Alternar.
        c.preview_rule_toggle("sif");
        assert!(c
            .preview_rules()
            .iter()
            .any(|(e, on, _, _)| e == "sif" && !*on));
        // Persistió: reabrir y la regla sigue (modo Código + XML).
        let c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(c2
            .preview_rules()
            .iter()
            .any(|(e, _, v, l)| e == "sif" && *v == 3 && *l == 0));
        // Quitar.
        c.preview_rule_remove("sif");
        assert!(!c.preview_rules().iter().any(|(e, _, _, _)| e == "sif"));
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

    /// El menú ▾ de historial salta a la entrada elegida traduciendo el índice del menú (cercano→
    /// lejano) al índice de la pila. Construye A → B → C, retrocede al medio y verifica que las
    /// listas de atrás/adelante y los saltos por índice de menú caigan en la carpeta correcta.
    #[test]
    fn menu_de_historial_salta_por_indice() {
        let cfg = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        let cc = root.path().join("c");
        for d in [&a, &b, &cc] {
            std::fs::create_dir(d).unwrap();
        }
        // Arranca en root, luego navega a → b → c (cursor en c, índice 3 de la pila).
        let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        for d in [&a, &b, &cc] {
            c.navigate_active_to(d.clone());
            assert!(drain(&mut c));
        }
        assert_eq!(c.ws.active_files().unwrap().current_dir, cc);
        // Atrás: cercano→lejano = [b, a, root]. Adelante: vacío.
        assert_eq!(
            c.back_history_entries(),
            vec![b.clone(), a.clone(), root.path().to_path_buf()]
        );
        assert!(c.forward_history_entries().is_empty());
        // Saltar al ítem 1 del menú de atrás (a). Tras esto hay atrás (root) y adelante (b, c).
        assert!(c.go_back_history(1));
        assert!(drain(&mut c));
        assert_eq!(c.ws.active_files().unwrap().current_dir, a);
        assert_eq!(c.back_history_entries(), vec![root.path().to_path_buf()]);
        assert_eq!(c.forward_history_entries(), vec![b.clone(), cc.clone()]);
        // Saltar al ítem 1 del menú de adelante (c, el más lejano).
        assert!(c.go_forward_history(1));
        assert!(drain(&mut c));
        assert_eq!(c.ws.active_files().unwrap().current_dir, cc);
        // Índice fuera de rango en cualquiera de los dos: no-op.
        assert!(!c.go_forward_history(0));
        assert!(!c.go_back_history(9));
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

    /// Regresión del bug de drop entre paneles: `drop_at` debe enrutar al panel que está BAJO
    /// el punto, NO al panel activo (origen). Construye 2 paneles con área conocida, calcula un
    /// punto que cae claramente dentro del rect del panel destino (el que NO es el origen) y
    /// verifica que la op de copia aterriza en la carpeta de ese panel.
    ///
    /// Cubre el ruteo posterior a la conversión de coordenadas (que sí ocurre con coords ya en
    /// el sistema de contenido). La conversión ScreenToClient en sí necesita un HWND real y se
    /// verifica a mano en la VM; aquí se blinda que coords realistas dentro del destino → ese
    /// panel, y no el fallback.
    #[test]
    fn drop_at_enruta_al_panel_bajo_el_cursor_no_al_activo() {
        let cfg = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        // Segundo panel apuntando a `b` (el destino del drop).
        let origin = c.ws.active_id().unwrap();
        let dest = c.split_for_target().unwrap();
        c.open_in_pane(dest, b.path().to_path_buf());
        assert!(drain(&mut c));
        // Área conocida; el panel activo sigue siendo el origen (`a`).
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        c.set_area(area);
        c.ws.set_active(origin);
        // Localizar el rect del panel destino (`dest`) y apuntar a su CENTRO. Así el punto cae
        // dentro de ese panel (no del origen) y la zona es Center → drop sobre el panel.
        let panes = c.pane_rects(area);
        let (_, dest_rect) = panes
            .iter()
            .find(|(id, _)| *id == dest)
            .copied()
            .expect("el panel destino tiene rect");
        let cx = dest_rect.x + dest_rect.w / 2.0;
        let cy = dest_rect.y + dest_rect.h / 2.0;
        // Sanity: ese punto NO cae dentro del rect del panel origen.
        let (_, origin_rect) = panes
            .iter()
            .find(|(id, _)| *id == origin)
            .copied()
            .expect("el panel origen tiene rect");
        let dentro_origen = cx >= origin_rect.x
            && cx < origin_rect.x + origin_rect.w
            && cy >= origin_rect.y
            && cy < origin_rect.y + origin_rect.h;
        assert!(!dentro_origen, "el punto debe caer fuera del panel origen");
        // Soltar el archivo de `a` sobre el panel destino (`b`). Forzamos COPIA con Ctrl para que
        // el aserto sea determinista sea cual sea el disco: sin modificadores, soltar dentro del
        // mismo disco MUEVE (regla del Explorador), y `a`/`b` viven en el mismo disco temporal.
        // `move_hint=false` (sin Shift del OLE), si no el move_hint forzaría Mover y rompería el
        // aserto de copia. Lo que prueba este test es el RUTEO (que el archivo aterriza en `b`,
        // bajo el cursor), no la elección mover/copiar (eso ya lo cubre dnd::decide_drop_action).
        let routed = c.drop_at(
            cx,
            cy,
            true,  // ctrl → copiar
            false, // shift
            vec![a.path().join("doc.txt")],
            false, // move_hint del OLE (sin Shift al soltar)
        );
        assert!(routed, "drop_at debe enrutar (no caer al fallback)");
        // CONFIRMAR AL SOLTAR: `drop_at` ya no ejecuta; deja el drop pendiente. La op real arranca
        // al confirmar (lo que hace el botón Copiar/Mover del modal). Sanity: nada se copió todavía.
        assert!(
            c.pending_drop.is_some(),
            "el drop queda pendiente de confirmar"
        );
        assert!(
            !b.path().join("doc.txt").exists(),
            "antes de confirmar no se copió nada"
        );
        assert!(c.confirm_pending_drop(), "confirmar arranca la op");
        assert!(c.pending_drop.is_none(), "el pendiente se consumió");
        // Drenar la op y verificar que la copia aterrizó en el panel destino (`b`), no en `a`.
        for _ in 0..2000 {
            if c.ops.pump_ops() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            b.path().join("doc.txt").exists(),
            "se copió al panel destino (b), bajo el cursor"
        );
        assert!(
            a.path().join("doc.txt").exists(),
            "el original sigue (copia)"
        );
    }

    /// `pane_at` resuelve el panel Files bajo un punto de contenido (mismo hit-test que `drop_at`,
    /// para resaltar EN VIVO el panel mientras se arrastra encima). El centro de cada panel cae en
    /// ESE panel; un punto fuera de toda área devuelve `None`. Además `set_drag_over` solo reporta
    /// `true` cuando el valor CAMBIA (la UI re-pinta solo en el cambio, no en cada `DragOver`).
    #[test]
    fn pane_at_resuelve_el_panel_files_bajo_el_cursor() {
        let cfg = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let origin = c.ws.active_id().unwrap();
        let dest = c.split_for_target().unwrap();
        c.open_in_pane(dest, b.path().to_path_buf());
        assert!(drain(&mut c));
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        c.set_area(area);
        let panes = c.pane_rects(area);
        let center = |id| {
            let (_, r) = panes.iter().find(|(p, _)| *p == id).copied().unwrap();
            (r.x + r.w / 2.0, r.y + r.h / 2.0)
        };
        // El centro de cada panel resuelve a ESE panel.
        let (ox, oy) = center(origin);
        let (dx, dy) = center(dest);
        assert_eq!(
            c.pane_at(ox, oy),
            Some(origin),
            "centro del origen → origen"
        );
        assert_eq!(
            c.pane_at(dx, dy),
            Some(dest),
            "centro del destino → destino"
        );
        // Fuera de toda área → None (no se resalta nada).
        assert_eq!(c.pane_at(-50.0, -50.0), None, "fuera de todo panel → None");
        // set_drag_over solo cambia (true) cuando el valor es distinto.
        assert!(c.set_drag_over(Some(dest)), "primera vez: cambia");
        assert!(!c.set_drag_over(Some(dest)), "mismo valor: no cambia");
        assert!(c.set_drag_over(None), "limpiar: cambia");
        assert_eq!(c.drag_over_pane(), None);
    }

    /// El `move_hint` del OLE (Shift presionado al SOLTAR) fuerza MOVER aunque los flags de
    /// teclado de la app lleguen en false (lo que pasa durante el bucle modal de DoDragDrop, que
    /// se traga los eventos de teclado). Regresión del bug "arrastré con Shift y copió en vez de
    /// mover: creó en destino pero no borró el origen".
    #[test]
    fn drop_at_move_hint_del_ole_fuerza_mover() {
        let cfg = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let origin = c.ws.active_id().unwrap();
        let dest = c.split_for_target().unwrap();
        c.open_in_pane(dest, b.path().to_path_buf());
        assert!(drain(&mut c));
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        c.set_area(area);
        c.ws.set_active(origin);
        let panes = c.pane_rects(area);
        let (_, dest_rect) = panes
            .iter()
            .find(|(id, _)| *id == dest)
            .copied()
            .expect("el panel destino tiene rect");
        let cx = dest_rect.x + dest_rect.w / 2.0;
        let cy = dest_rect.y + dest_rect.h / 2.0;
        // ctrl=false, shift=false (estado de la app stale durante el modal), PERO move_hint=true
        // (Shift REAL al soltar, reportado por el OLE). Debe MOVER → el original desaparece.
        let routed = c.drop_at(cx, cy, false, false, vec![a.path().join("doc.txt")], true);
        assert!(routed, "drop_at debe enrutar");
        // El drop queda pendiente: debe ser MOVER (el move_hint del OLE manda).
        assert_eq!(
            c.pending_drop.as_ref().map(|p| p.is_move),
            Some(true),
            "el drop pendiente es Mover por el move_hint"
        );
        assert!(c.confirm_pending_drop(), "confirmar arranca la op");
        for _ in 0..2000 {
            if c.ops.pump_ops() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            b.path().join("doc.txt").exists(),
            "el archivo aterrizó en el destino"
        );
        assert!(
            !a.path().join("doc.txt").exists(),
            "el original se MOVIÓ (ya no está en el origen) por el move_hint del OLE"
        );
    }

    /// Regla del Explorador: soltar SIN modificadores ni move_hint, dentro del MISMO disco, MUEVE
    /// por defecto (no copia). `a` y `b` son tempdirs del mismo volumen del sistema, así que
    /// `same_drive` es true y `decide_drop_action(false, false, true)` = Move. El archivo aterriza
    /// en el destino y desaparece del origen. (El test de ruteo fuerza Ctrl=copia justo para evitar
    /// esta ambigüedad; este test fija el comportamiento por defecto del mismo disco.)
    #[test]
    fn drop_at_mismo_disco_sin_modificadores_mueve_por_defecto() {
        let cfg = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let origin = c.ws.active_id().unwrap();
        let dest = c.split_for_target().unwrap();
        c.open_in_pane(dest, b.path().to_path_buf());
        assert!(drain(&mut c));
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        c.set_area(area);
        c.ws.set_active(origin);
        let panes = c.pane_rects(area);
        let (_, dest_rect) = panes
            .iter()
            .find(|(id, _)| *id == dest)
            .copied()
            .expect("el panel destino tiene rect");
        let cx = dest_rect.x + dest_rect.w / 2.0;
        let cy = dest_rect.y + dest_rect.h / 2.0;
        // ctrl=false, shift=false, move_hint=false → la decisión depende del disco. Mismo disco → Mover.
        let routed = c.drop_at(cx, cy, false, false, vec![a.path().join("doc.txt")], false);
        assert!(routed, "drop_at debe enrutar");
        assert!(c.confirm_pending_drop(), "confirmar arranca la op");
        for _ in 0..2000 {
            if c.ops.pump_ops() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(b.path().join("doc.txt").exists(), "aterrizó en el destino");
        assert!(
            !a.path().join("doc.txt").exists(),
            "mismo disco sin modificadores: se MOVIÓ (el original ya no está)"
        );
    }

    /// CONFIRMAR AL SOLTAR (PUNTO 1b): un drop entre paneles NO ejecuta la operación hasta que el
    /// usuario confirme. `drop_at` solo deja el drop pendiente; CANCELAR lo descarta sin tocar el
    /// disco. Regresión del bug "arrastré sin querer una carpeta a otro panel y empezó a copiarla".
    #[test]
    fn drop_at_no_ejecuta_hasta_confirmar_y_cancelar_descarta() {
        let cfg = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let origin = c.ws.active_id().unwrap();
        let dest = c.split_for_target().unwrap();
        c.open_in_pane(dest, b.path().to_path_buf());
        assert!(drain(&mut c));
        let area = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        c.set_area(area);
        c.ws.set_active(origin);
        let panes = c.pane_rects(area);
        let (_, dest_rect) = panes
            .iter()
            .find(|(id, _)| *id == dest)
            .copied()
            .expect("el panel destino tiene rect");
        let cx = dest_rect.x + dest_rect.w / 2.0;
        let cy = dest_rect.y + dest_rect.h / 2.0;
        // Soltar (Ctrl=copia para que el dato sea determinista).
        let routed = c.drop_at(cx, cy, true, false, vec![a.path().join("doc.txt")], false);
        assert!(routed, "drop_at debe enrutar");
        // NADA se ejecutó todavía: el drop está pendiente y no hay op alguna.
        let pd = c.pending_drop.as_ref().expect("drop pendiente");
        assert_eq!(pd.count, 1);
        assert!(!pd.is_move, "Ctrl → copiar");
        assert_eq!(pd.dest_dir, b.path());
        assert!(
            c.ops.active_ops.is_empty(),
            "antes de confirmar no arrancó ninguna op"
        );
        // CANCELAR descarta sin copiar.
        c.cancel_pending_drop();
        assert!(c.pending_drop.is_none(), "el pendiente se descartó");
        for _ in 0..200 {
            c.ops.pump_ops();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            !b.path().join("doc.txt").exists(),
            "cancelar no copió nada al destino"
        );
        assert!(a.path().join("doc.txt").exists(), "el origen quedó intacto");
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

    /// `ensure_ops_pane` agrega un panel de Operaciones si no hay; es idempotente (no agrega un
    /// segundo) y NO roba el foco al panel Files activo (el usuario estaba operando ahí).
    #[test]
    fn ensure_ops_pane_agrega_una_vez_y_no_roba_foco() {
        use naygo_core::workspace::PanePurpose;
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let files_id = c.ws.active_id();
        assert!(
            !c.has_purpose(PanePurpose::Operations),
            "no hay panel de ops"
        );

        c.ensure_ops_pane();
        assert!(
            c.has_purpose(PanePurpose::Operations),
            "se agregó el panel de ops"
        );
        assert_eq!(
            c.ws.active_id(),
            files_id,
            "el activo sigue siendo el panel Files (no robó foco)"
        );
        let ops_count =
            c.ws.panes()
                .iter()
                .filter(|p| p.purpose == PanePurpose::Operations)
                .count();
        assert_eq!(ops_count, 1, "exactamente un panel de operaciones");

        // Segunda llamada: idempotente, no agrega otro.
        c.ensure_ops_pane();
        let ops_count2 =
            c.ws.panes()
                .iter()
                .filter(|p| p.purpose == PanePurpose::Operations)
                .count();
        assert_eq!(ops_count2, 1, "sigue habiendo exactamente uno");
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
        // MEJORA 1: el archivo recién llegado queda SELECCIONADO (resaltado como selección) y la
        // fila correspondiente lo refleja. Su posición de vista se calculó tras reordenar.
        let f = c.ws.pane(id).and_then(|p| p.files.as_ref()).unwrap();
        let nuevo_pos = f
            .view_indices()
            .iter()
            .position(|&real| f.entries[real].path == nuevo)
            .expect("nuevo.txt está en la vista");
        assert!(
            f.is_selected(nuevo_pos),
            "el archivo recién llegado queda seleccionado"
        );
        assert!(
            c.rows_of(id, 8, std::time::Instant::now())
                .iter()
                .any(|r| r.name == "nuevo.txt" && r.selected),
            "la fila del nuevo se pinta como seleccionada"
        );
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

    /// El árbol editable de favoritos: crear grupo, mover un favorito dentro, expandir/colapsar,
    /// renombrar y eliminar; todo persiste a `favorites.json` (un controlador nuevo lo restaura).
    #[test]
    fn favoritos_arbol_editable_y_persistente() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        let mut c = WorkspaceCtrl::new_in(a.clone(), cfg.clone());
        assert!(drain(&mut c));
        c.favorites.add_favorite(&a);
        c.favorites.add_favorite(&b);

        // Crear grupo en la raíz y mover el favorito `b` dentro.
        c.fav_new_group("", "Trabajo");
        let opts = c.fav_group_options();
        assert_eq!(opts.len(), 1);
        let (np, gid) = opts[0].clone();
        assert_eq!(np, "Trabajo");
        c.fav_move_node(false, "", &b.display().to_string(), &gid);

        // Colapsado por defecto: solo se ven `a` y el grupo (la hoja `b` está oculta).
        let rows = c.fav_tree_rows();
        assert!(rows.iter().any(|r| r.is_group && r.name == "Trabajo"));
        assert!(!rows.iter().any(|r| r.path == b.display().to_string()));

        // Expandir el grupo revela la hoja con sangría.
        c.fav_toggle_expand("Trabajo");
        let rows = c.fav_tree_rows();
        let inner = rows.iter().find(|r| r.path == b.display().to_string());
        assert!(inner.is_some() && inner.unwrap().depth == 1);

        // Renombrar el grupo (re-mapea la expansión: sigue expandido tras renombrar).
        let gid = c
            .fav_group_options()
            .into_iter()
            .find(|(n, _)| n == "Trabajo")
            .unwrap()
            .1;
        c.fav_rename_group(&gid, "Proyectos");
        assert!(c.fav_expanded.contains("Proyectos"));
        let rows = c.fav_tree_rows();
        assert!(rows.iter().any(|r| r.is_group && r.name == "Proyectos"));
        // Sigue expandido: la hoja interna se ve.
        assert!(rows
            .iter()
            .any(|r| r.path == b.display().to_string() && r.depth == 1));

        // Persistió: un controlador nuevo (mismo config_dir) ve el grupo renombrado con su hoja.
        let mut c2 = WorkspaceCtrl::new_in(a.clone(), cfg.clone());
        assert!(drain(&mut c2));
        assert!(c2.favorites.contains(&b));
        assert!(c2
            .favorites
            .roots()
            .iter()
            .any(|n| matches!(n, FavNode::Group { name, .. } if name == "Proyectos")));

        // Eliminar el grupo borra su hoja interna; el favorito de la raíz queda.
        let gid = c2
            .fav_group_options()
            .into_iter()
            .find(|(n, _)| n == "Proyectos")
            .unwrap()
            .1;
        c2.fav_delete_node(true, &gid, "");
        assert!(!c2.favorites.contains(&b));
        assert!(c2.favorites.contains(&a));
    }

    /// "Mover a…" no debe ofrecer mover un grupo dentro de sí mismo ni de sus descendientes.
    #[test]
    fn fav_move_targets_excluye_el_grupo_y_sus_descendientes() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let a = tmp.path().join("a");
        std::fs::create_dir(&a).unwrap();
        let mut c = WorkspaceCtrl::new_in(a.clone(), cfg);
        assert!(drain(&mut c));
        // Estructura: "Trabajo" (root) con subgrupo "Sub"; y "Personal" (root) aparte.
        c.fav_new_group("", "Trabajo");
        let trabajo = c
            .fav_group_options()
            .into_iter()
            .find(|(n, _)| n == "Trabajo")
            .unwrap()
            .1;
        c.fav_new_group(&trabajo, "Sub");
        c.fav_new_group("", "Personal");

        let labels: Vec<String> = c.fav_group_options().into_iter().map(|(n, _)| n).collect();
        assert!(labels.contains(&"Trabajo".to_string()));
        assert!(labels.contains(&"Trabajo/Sub".to_string()));
        assert!(labels.contains(&"Personal".to_string()));

        // Mover el GRUPO "Trabajo": destinos válidos = solo "Personal" (no él mismo, no su "Sub").
        let targets: Vec<String> = c
            .fav_move_targets(true, &trabajo)
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert!(!targets.contains(&"Trabajo".to_string()), "no a sí mismo");
        assert!(
            !targets.contains(&"Trabajo/Sub".to_string()),
            "no a un descendiente"
        );
        assert!(targets.contains(&"Personal".to_string()), "sí a un hermano");

        // Para una HOJA (favorito): sin restricción, cualquier grupo es destino válido.
        let leaf_targets = c.fav_move_targets(false, "");
        assert_eq!(
            leaf_targets.len(),
            3,
            "una hoja puede ir a cualquiera de los 3 grupos"
        );
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

    /// `any_modal_open` (keep-alive del timer): en reposo es false; con un modal/overlay del
    /// controlador abierto es true; al cerrarlo vuelve a false. Es el predicado que mantiene vivo
    /// el bucle de UI mientras hay un popup, para que su hover y primer clic respondan al instante.
    #[test]
    fn any_modal_open_refleja_los_overlays_del_controlador() {
        let cfg = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("a.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));

        // Reposo: sin modales → false (el timer puede dormir como antes; bajo consumo intacto).
        assert!(!c.any_modal_open(), "en reposo no hay modales abiertos");

        // Modal de confirmar borrado (lo reportó el usuario): seleccionar y abrirlo → true.
        let pos = active_pos_of(&c, "a.txt").unwrap();
        c.ws.active_files_mut().unwrap().select_single(pos);
        c.op_delete(false);
        assert!(
            c.ops.pending_dialog.is_some(),
            "op_delete abre el diálogo de confirmación"
        );
        assert!(
            c.any_modal_open(),
            "con un OpDialog abierto, el timer sigue vivo"
        );

        // Cerrar el diálogo → vuelve a reposo.
        c.ops.pending_dialog = None;
        assert!(!c.any_modal_open(), "al cerrar el modal vuelve a dormir");

        // El menú contextual (clic derecho) también cuenta: necesita hover vivo.
        c.open_context_menu(0.0, 0.0);
        assert!(
            c.any_modal_open(),
            "el menú contextual mantiene vivo el timer"
        );
        c.close_context_menu();
        assert!(!c.any_modal_open());

        // La ayuda (F1) cuenta como overlay.
        c.help_open = true;
        assert!(c.any_modal_open(), "la ayuda (F1) mantiene vivo el timer");
        c.help_open = false;
        assert!(!c.any_modal_open());
    }
}

/// SIMULAR USUARIO — tests de integración de gestos de punta a punta.
///
/// Cada test crea archivos/carpetas REALES en un tempdir AISLADO (más un `config_dir`
/// propio), simula al usuario operándolos con ATAJOS DE TECLADO, CLIC/MOUSE y ARRASTRE
/// llamando al MISMO código del controlador (`WorkspaceCtrl` / `OpsCtrl`) que disparan esos
/// gestos en la app real (headless, sin abrir ventana) y verifica el resultado en DISCO.
/// Nada toca archivos del sistema del usuario: todo vive bajo `tempfile::tempdir()` y se borra
/// al caer del scope.
///
/// Estos tests cierran el lazo gesto → controlador → motor de ops → filesystem. La resolución de
/// los modales (confirmar nombre, confirmar borrado, resolver conflicto) se hace por la API de
/// `OpsCtrl`, porque `on_key` SUSPENDE las acciones globales mientras hay un modal abierto (igual
/// que en la app: el teclado lo controla el modal Slint). Por eso el teclado simula el DISPARO del
/// gesto (p. ej. Ctrl+Shift+N abre el modal de carpeta nueva) y el "Aceptar" del modal va por
/// `name_confirm()` / `delete_confirm()` / `resolve_conflict()`, espejando el cableado de `main.rs`.
#[cfg(test)]
mod simular_usuario {
    use super::*;
    use naygo_core::ops::ConflictAction;

    // --- Helpers (copiados del mod `tests` de arriba y de keys.rs: los mods hermanos no se ven
    //     entre sí, así que se replican aquí para que esta suite sea autocontenida) ---

    /// Drena los listados hasta que todos terminan (con timeout), simulando los ticks del Timer.
    fn drain(c: &mut WorkspaceCtrl) -> bool {
        for _ in 0..2000 {
            if c.pump_listings() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        false
    }

    /// Drena las operaciones de archivo en curso hasta que TODAS terminan (summary presente) o se
    /// agota el timeout. Devuelve true si terminaron limpio. NO resuelve modales: para flujos con
    /// conflicto, usar `drain_ops_resolving`.
    fn drain_ops(c: &mut WorkspaceCtrl) -> bool {
        for _ in 0..4000 {
            c.ops.pump_ops();
            if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
            {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        false
    }

    /// Posición de VISTA del ítem llamado `name` en el panel activo (índice contra `view_indices`,
    /// que es el mismo que consumen el clic y el teclado).
    fn active_pos_of(c: &WorkspaceCtrl, name: &str) -> Option<usize> {
        let f = c.ws.active_files()?;
        f.view_indices()
            .iter()
            .position(|&real| f.entries[real].name == name)
    }

    /// UX4: el resumen de nombres para el modal de confirmación de drop. Umbral: hasta 4 nombres
    /// listados; a partir de 5, los primeros 4 + sufijo "y N más" (aquí simulado como " +N").
    #[test]
    fn pending_drop_names_summary_uno_pocos_muchos() {
        let mk = |paths: Vec<&str>| PendingDrop {
            paths: paths.into_iter().map(std::path::PathBuf::from).collect(),
            dest_dir: std::path::PathBuf::from("C:\\dest"),
            is_move: false,
            count: 0,
        };
        // El sufijo de "y N más" lo simulamos con " +N" para no depender de i18n en el test.
        let more = |n: usize| format!(" +{}", n);

        // 1 elemento: solo su nombre entre comillas angulares.
        assert_eq!(mk(vec!["C:\\a\\foo.txt"]).names_summary(more), "«foo.txt»");

        // Pocos (3): todos listados, sin sufijo.
        assert_eq!(
            mk(vec!["C:\\a\\a.txt", "C:\\a\\b.txt", "C:\\a\\c.txt"]).names_summary(more),
            "«a.txt», «b.txt», «c.txt»"
        );

        // Justo en el umbral (4): los 4, sin sufijo.
        assert_eq!(
            mk(vec!["x\\1", "x\\2", "x\\3", "x\\4"]).names_summary(more),
            "«1», «2», «3», «4»"
        );

        // Muchos (6): primeros 4 + " +2" (quedan 2 sin nombrar).
        assert_eq!(
            mk(vec!["x\\1", "x\\2", "x\\3", "x\\4", "x\\5", "x\\6"]).names_summary(more),
            "«1», «2», «3», «4» +2"
        );
    }

    /// UX3: con `confirm_drop_between_panes = false`, soltar entre paneles NO deja un drop pendiente
    /// esperando confirmación: ejecuta la op directo (no abre el modal kind 3). Pero si el archivo
    /// YA EXISTE en el destino, el modal de CONFLICTO sigue apareciendo (son cosas distintas).
    #[test]
    fn drop_con_confirmacion_off_ejecuta_directo_y_sin_conflicto_copia() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
        let (mut c, _cfg) = ctrl_en(a.path());
        // Apagar la confirmación de drop.
        c.config.settings.confirm_drop_between_panes = false;
        let (origin, dest) = split_a(&mut c, b.path());
        let area = area();
        c.set_area(area);
        c.ws.set_active(origin);
        let (cx, cy) = pane_center(&c, area, dest);

        // Soltar (Ctrl = copia, determinista). Con la confirmación OFF NO debe quedar pendiente.
        let routed = c.drop_at(cx, cy, true, false, vec![a.path().join("doc.txt")], false);
        assert!(
            routed,
            "drop_at enruta igual (devuelve true en ambos modos)"
        );
        assert!(
            c.pending_drop.is_none(),
            "confirmación OFF: no queda un drop esperando el modal kind 3"
        );
        assert!(
            !c.ops.active_ops.is_empty(),
            "confirmación OFF: la op arrancó directo"
        );
        // Sin conflicto (nombre libre en el destino): la op termina y el archivo aterriza.
        for _ in 0..4000 {
            if c.ops.pump_ops() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            b.path().join("doc.txt").exists(),
            "confirmación OFF, sin conflicto: la copia se ejecutó directo"
        );
    }

    /// UX3 (parte crítica): aunque la confirmación de drop esté APAGADA, el modal de CONFLICTO
    /// (archivo que ya existe en el destino) DEBE seguir apareciendo. El motor se detiene en el
    /// conflicto y no sobrescribe sin preguntar — confirmación de drop y conflicto son ortogonales.
    #[test]
    fn drop_con_confirmacion_off_pero_archivo_existente_pide_conflicto() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("doc.txt"), b"ORIGEN-nuevo").unwrap();
        std::fs::write(b.path().join("doc.txt"), b"DESTINO-viejo").unwrap();
        let (mut c, _cfg) = ctrl_en(a.path());
        c.config.settings.confirm_drop_between_panes = false;
        let (origin, dest) = split_a(&mut c, b.path());
        let area = area();
        c.set_area(area);
        c.ws.set_active(origin);
        let (cx, cy) = pane_center(&c, area, dest);

        // Soltar con la confirmación OFF → arranca directo, sin modal kind 3.
        assert!(c.drop_at(cx, cy, true, false, vec![a.path().join("doc.txt")], false));
        assert!(c.pending_drop.is_none(), "no hay confirmación de drop");

        // El motor debe DETENERSE en el conflicto, no sobrescribir.
        let mut pidio_conflicto = false;
        for _ in 0..4000 {
            c.ops.pump_ops();
            if matches!(
                c.ops.pending_dialog,
                Some(crate::ops_ctrl::OpDialog::Conflict { .. })
            ) {
                pidio_conflicto = true;
                break;
            }
            if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            pidio_conflicto,
            "confirmación de drop OFF NO debe saltarse el conflicto: el archivo existe → preguntar"
        );
        assert_eq!(
            std::fs::read_to_string(b.path().join("doc.txt")).unwrap(),
            "DESTINO-viejo",
            "el destino no se toca hasta que el usuario resuelva el conflicto"
        );
    }

    /// REGRESIÓN (reportado por Nicolás): arrastrar un archivo a otro panel donde YA EXISTE, y
    /// confirmar el drop, debe DETENERSE en el conflicto (preguntar) — no sobrescribir en silencio.
    /// Reproduce el flujo completo drop_at → confirm_pending_drop → motor con first_collision.
    /// Distingue si el bug está en el MOTOR (este test falla) o solo en la UI/timing (pasa).
    #[test]
    fn drop_sobre_archivo_existente_confirmado_pide_conflicto_no_sobrescribe() {
        let cfg = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        // El MISMO nombre existe en ambos lados, con contenido DISTINTO.
        std::fs::write(a.path().join("doc.txt"), b"ORIGEN-nuevo").unwrap();
        std::fs::write(b.path().join("doc.txt"), b"DESTINO-viejo").unwrap();
        let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let origin = c.ws.active_id().unwrap();
        let dest = c.split_for_target().unwrap();
        c.open_in_pane(dest, b.path().to_path_buf());
        assert!(drain(&mut c));
        let area = area();
        c.set_area(area);
        c.ws.set_active(origin);
        let (cx, cy) = pane_center(&c, area, dest);

        // Soltar el archivo sobre el destino (Ctrl=copia, determinista) y CONFIRMAR.
        assert!(c.drop_at(cx, cy, true, false, vec![a.path().join("doc.txt")], false));
        assert!(c.confirm_pending_drop(), "el drop confirmado arranca la op");

        // Drenar las ops hasta que el motor PIDA el conflicto (pending_dialog = Conflict). Si en
        // vez de eso la op termina (summary) SIN preguntar, es el bug: sobrescribió en silencio.
        let mut pidio_conflicto = false;
        for _ in 0..4000 {
            c.ops.pump_ops();
            if matches!(
                c.ops.pending_dialog,
                Some(crate::ops_ctrl::OpDialog::Conflict { .. })
            ) {
                pidio_conflicto = true;
                break;
            }
            if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
            {
                break; // terminó sin preguntar
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert!(
            pidio_conflicto,
            "copiar sobre un archivo que YA existe debe DETENERSE en el conflicto, no sobrescribir"
        );
        // El destino NO debe haberse pisado mientras se espera la decisión.
        assert_eq!(
            std::fs::read_to_string(b.path().join("doc.txt")).unwrap(),
            "DESTINO-viejo",
            "el destino no se toca hasta que el usuario decida"
        );
    }

    /// El char unicode de una tecla especial de Slint, como String (lo que llega a `on_key`).
    /// Copiado de keys.rs:92.
    fn key_char(k: slint::platform::Key) -> String {
        let s: slint::SharedString = k.into();
        s.to_string()
    }

    /// Área de trabajo conocida y estable para el hit-testing de paneles (clic/arrastre).
    fn area() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        }
    }

    /// Centro (cx, cy) del rect del panel `id` dentro de `a` (para apuntar clic/drop a ESE panel).
    fn pane_center(c: &WorkspaceCtrl, a: Rect, id: PaneId) -> (f32, f32) {
        let (_, r) = c
            .pane_rects(a)
            .into_iter()
            .find(|(p, _)| *p == id)
            .expect("el panel tiene rect");
        (r.x + r.w / 2.0, r.y + r.h / 2.0)
    }

    /// Arranca un controlador AISLADO apuntando a `start`, con un `config_dir` temporal propio, y
    /// drena el primer listado. Devuelve `(ctrl, tmp_cfg)`; el `tmp_cfg` se retiene para que el dir
    /// no se borre antes de tiempo.
    fn ctrl_en(start: &std::path::Path) -> (WorkspaceCtrl, tempfile::TempDir) {
        let cfg = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(start.to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c), "el listado inicial debe terminar");
        (c, cfg)
    }

    /// Abre un SEGUNDO panel apuntando a `dir` (split) y deja el ORIGEN activo. Devuelve
    /// `(origin_id, dest_id)`. Espeja `split_for_target` + `open_in_pane`, el camino de "abrir en
    /// otro panel".
    fn split_a(c: &mut WorkspaceCtrl, dir: &std::path::Path) -> (PaneId, PaneId) {
        let origin = c.ws.active_id().unwrap();
        let dest = c
            .split_for_target()
            .expect("se pudo dividir en dos paneles");
        c.open_in_pane(dest, dir.to_path_buf());
        assert!(drain(c), "el listado del segundo panel debe terminar");
        c.ws.set_active(origin);
        (origin, dest)
    }

    // ============================ 1. Crear carpeta con Ctrl+Shift+N ============================

    /// GESTO: el usuario pulsa Ctrl+Shift+N (atajo de "nueva carpeta"), escribe el nombre y
    /// confirma. RESULTADO: la carpeta existe en disco. Cubre on_key(chord NewDir) → modal
    /// NameInput(NewDir) → name_changed → name_confirm → motor → refresh.
    #[test]
    fn crear_carpeta_con_ctrl_shift_n() {
        let work = tempfile::tempdir().unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());

        // Atajo Ctrl+Shift+N: abre el modal de nombre para NUEVA CARPETA.
        c.on_key("n", true, true, false);
        assert!(
            matches!(
                c.ops.pending_dialog,
                Some(crate::ops_ctrl::OpDialog::NameInput {
                    purpose: crate::ops_ctrl::NamePurpose::NewDir,
                    ..
                })
            ),
            "Ctrl+Shift+N debe abrir el modal de NUEVA CARPETA"
        );

        // El usuario escribe el nombre y confirma (Aceptar / Enter del modal).
        c.ops.name_changed("Documentos".into());
        c.ops.name_confirm();
        assert!(drain_ops(&mut c), "la creación de la carpeta debe terminar");
        c.refresh_active();
        assert!(drain(&mut c));

        let creada = work.path().join("Documentos");
        assert!(creada.is_dir(), "la carpeta nueva debe existir en disco");
        assert!(
            active_pos_of(&c, "Documentos").is_some(),
            "la carpeta nueva aparece en la vista tras refrescar"
        );
    }

    // ===================== 1b. Refrescar (F5) NO duplica las filas =============================

    /// REGRESIÓN (bug reportado por Nicolás): al pulsar F5 sobre un panel ya poblado, las filas
    /// se DUPLICABAN porque `pump_listings` hacía `entries.extend(batch)` sin vaciar primero las
    /// entries previas. RESULTADO esperado: tras refrescar, el panel tiene EXACTAMENTE los mismos
    /// archivos que el disco, sin copias. Cubre el flag `Listing::fresh` + el `clear()` del primer
    /// lote en `pump_listings`.
    #[test]
    fn refrescar_f5_no_duplica_las_filas() {
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("a.txt"), b"a").unwrap();
        std::fs::write(work.path().join("b.txt"), b"b").unwrap();
        std::fs::create_dir(work.path().join("sub")).unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());

        let n0 = c.ws.active_files().unwrap().entries.len();
        assert_eq!(n0, 3, "el listado inicial trae los 3 ítems");

        // Refrescar varias veces (F5): cada refresco debe REEMPLAZAR, no acumular.
        for _ in 0..3 {
            assert!(c.refresh_active(), "F5 inicia el re-listado");
            assert!(drain(&mut c), "el re-listado termina");
            assert_eq!(
                c.ws.active_files().unwrap().entries.len(),
                3,
                "refrescar NO debe duplicar: siguen siendo 3 ítems"
            );
        }
    }

    /// REGRESIÓN: refrescar una carpeta que QUEDÓ VACÍA (todos sus ítems se borraron desde fuera)
    /// debe dejar el panel vacío, no con las filas viejas. Cubre el `clear()` de la rama `done`
    /// cuando el listado nuevo no emitió ningún lote.
    #[test]
    fn refrescar_carpeta_que_quedo_vacia_limpia_las_filas() {
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("a.txt"), b"a").unwrap();
        std::fs::write(work.path().join("b.txt"), b"b").unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());
        assert_eq!(c.ws.active_files().unwrap().entries.len(), 2);

        // Se borran los archivos por fuera y se refresca.
        std::fs::remove_file(work.path().join("a.txt")).unwrap();
        std::fs::remove_file(work.path().join("b.txt")).unwrap();
        assert!(c.refresh_active());
        assert!(drain(&mut c));

        assert_eq!(
            c.ws.active_files().unwrap().entries.len(),
            0,
            "tras refrescar, la carpeta vacía no debe conservar las filas viejas"
        );
    }

    // =============== 1c. Ciclo operación → refresh NO deja la vista inconsistente ===============

    /// REGRESIÓN integral del bug de F5 en su escenario REAL: el usuario opera (crea/copia/borra)
    /// y luego refresca. Crear varios archivos y refrescar varias veces NO debe duplicar ni perder
    /// filas: la vista siempre refleja EXACTAMENTE lo que hay en disco.
    #[test]
    fn crear_y_refrescar_repetido_mantiene_la_vista_consistente() {
        let work = tempfile::tempdir().unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());
        assert_eq!(c.ws.active_files().unwrap().entries.len(), 0);

        // Crear 4 archivos por fuera y refrescar: la vista debe tener exactamente 4.
        for i in 0..4 {
            std::fs::write(work.path().join(format!("f{i}.txt")), b"x").unwrap();
        }
        assert!(c.refresh_active());
        assert!(drain(&mut c));
        assert_eq!(c.ws.active_files().unwrap().entries.len(), 4);

        // Refrescar 5 veces más sin cambios: sigue en 4 (no acumula).
        for _ in 0..5 {
            assert!(c.refresh_active());
            assert!(drain(&mut c));
        }
        assert_eq!(
            c.ws.active_files().unwrap().entries.len(),
            4,
            "refrescos repetidos no deben duplicar ni perder filas"
        );

        // Agregar uno más y refrescar: pasa a 5.
        std::fs::write(work.path().join("f4.txt"), b"x").unwrap();
        assert!(c.refresh_active());
        assert!(drain(&mut c));
        assert_eq!(c.ws.active_files().unwrap().entries.len(), 5);
    }

    /// REGRESIÓN: copiar un archivo al OTRO panel y luego refrescar el panel DESTINO no debe
    /// duplicar la fila recién llegada. (El destino se re-lista tras la operación; este test cierra
    /// el ciclo operación-en-un-panel → refresh-del-otro.)
    #[test]
    fn copiar_al_otro_panel_y_refrescar_destino_no_duplica() {
        let cfg = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
        let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let origin = c.ws.active_id().unwrap();
        let dest = c.split_for_target().unwrap();
        c.open_in_pane(dest, b.path().to_path_buf());
        assert!(drain(&mut c));
        c.set_area(area());
        c.ws.set_active(origin);

        // Copiar al otro panel.
        c.ws.active_files_mut().unwrap().select_all();
        c.op_to_other(false);
        assert!(drain_ops(&mut c));

        // Refrescar el destino dos veces: la fila copiada aparece UNA sola vez.
        c.ws.set_active(dest);
        for _ in 0..2 {
            assert!(c.refresh_active());
            assert!(drain(&mut c));
        }
        let dest_entries = c.ws.active_files().unwrap().entries.len();
        assert_eq!(
            dest_entries, 1,
            "el destino tiene 1 archivo, no duplicado tras refrescar"
        );
    }

    /// REGRESIÓN: eliminar un archivo (permanente, en el tempdir) y refrescar debe BAJAR el conteo,
    /// no dejar la fila fantasma. Cierra el ciclo eliminar → refresh.
    #[test]
    fn eliminar_y_refrescar_baja_el_conteo() {
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("uno.txt"), b"x").unwrap();
        std::fs::write(work.path().join("dos.txt"), b"x").unwrap();
        std::fs::write(work.path().join("tres.txt"), b"x").unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());
        assert_eq!(c.ws.active_files().unwrap().entries.len(), 3);

        // Seleccionar "uno.txt" y eliminarlo permanente (queda en el tempdir, no en la papelera).
        let pos = active_pos_of(&c, "uno.txt").expect("uno.txt está en la vista");
        c.ws.active_files_mut().unwrap().select_single(pos);
        c.op_delete(true);
        c.ops.delete_confirm();
        assert!(drain_ops(&mut c));
        c.refresh_active();
        assert!(drain(&mut c));

        assert_eq!(
            c.ws.active_files().unwrap().entries.len(),
            2,
            "tras eliminar y refrescar, quedan 2 archivos"
        );
        assert!(
            active_pos_of(&c, "uno.txt").is_none(),
            "el archivo eliminado ya no está en la vista"
        );
    }

    // ============================== 2. Crear archivo con Ctrl+N ===============================

    /// GESTO: Ctrl+N (atajo "nuevo archivo"), nombre, confirmar. RESULTADO: el archivo existe.
    #[test]
    fn crear_archivo_con_ctrl_n() {
        let work = tempfile::tempdir().unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());

        c.on_key("n", true, false, false); // Ctrl+N → nuevo archivo
        assert!(
            matches!(
                c.ops.pending_dialog,
                Some(crate::ops_ctrl::OpDialog::NameInput {
                    purpose: crate::ops_ctrl::NamePurpose::NewFile,
                    ..
                })
            ),
            "Ctrl+N debe abrir el modal de NUEVO ARCHIVO"
        );

        c.ops.name_changed("apuntes.txt".into());
        c.ops.name_confirm();
        assert!(drain_ops(&mut c), "la creación del archivo debe terminar");
        c.refresh_active();
        assert!(drain(&mut c));

        let creado = work.path().join("apuntes.txt");
        assert!(creado.is_file(), "el archivo nuevo debe existir en disco");
    }

    // ===================== 3. Crear varias carpetas anidadas (a\b\c) ==========================

    /// GESTO: abrir el cuadro "nueva(s) carpeta(s)" (botón de la toolbar) y escribir varias líneas,
    /// cada una una carpeta SUELTA, y aplicar. RESULTADO: todas existen en disco. Cubre el alta
    /// MULTILÍNEA (varias carpetas de una vez), que es el camino distinto del modal de nombre simple.
    #[test]
    fn crear_varias_carpetas_multilinea() {
        let work = tempfile::tempdir().unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());

        c.new_folder_open_active();
        assert!(
            c.new_folder_open(),
            "el cuadro de nuevas carpetas debe abrir"
        );
        // Tres carpetas en tres líneas (más una línea en blanco, que se ignora).
        c.new_folder_set_text("alfa\nbeta\n\ngamma");
        assert_eq!(
            c.new_folder_counts(),
            (3, 0),
            "tres líneas válidas (la vacía no cuenta)"
        );
        c.new_folder_apply();
        assert!(drain_ops(&mut c), "la creación múltiple debe terminar");

        for n in ["alfa", "beta", "gamma"] {
            assert!(
                work.path().join(n).is_dir(),
                "la carpeta '{n}' de la línea correspondiente debe existir"
            );
        }
    }

    /// GESTO: en el cuadro de nuevas carpetas, escribir una RUTA anidada con separadores
    /// (`a\b\c`) y aplicar. RESULTADO: la cadena anidada existe en disco.
    ///
    /// REGRESIÓN ARREGLADA. Antes `ops::plan()` validaba el nombre completo con
    /// `ops::names::is_valid_name`, que PROHÍBE el `\` (está en FORBIDDEN), así que la ruta
    /// relativa anidada (`nivel1\nivel2\nivel3`) caía en `Err(InvalidName)` y `start_op` la
    /// descartaba en silencio. Ahora el plan de `CreateDir` valida POR COMPONENTE
    /// (`names::relative_components`, que rechaza `.`/`..`/vacío/absoluta) y arma el destino
    /// uniendo cada componente sobre la carpeta destino; el motor lo crea con `create_dir_all`.
    #[test]
    fn crear_carpetas_anidadas_con_separadores() {
        let work = tempfile::tempdir().unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());

        c.new_folder_open_active();
        c.new_folder_set_text("nivel1\\nivel2\\nivel3");
        assert_eq!(
            c.new_folder_counts(),
            (1, 0),
            "la línea anidada se considera válida en el parseo del cuadro"
        );
        c.new_folder_apply();
        assert!(drain_ops(&mut c), "la creación anidada debe terminar");

        let anidada = work.path().join("nivel1").join("nivel2").join("nivel3");
        assert!(
            anidada.is_dir(),
            "la cadena de carpetas anidadas debe existir: {}",
            anidada.display()
        );
    }

    // ===================== 4. Mover archivo entre 2 paneles con F6 ============================

    /// GESTO: con dos paneles, seleccionar un archivo en el origen y pulsar F6 (mover al otro
    /// panel). RESULTADO: el archivo está en el destino y YA NO en el origen.
    #[test]
    fn mover_archivo_al_otro_panel_con_f6() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("informe.txt"), b"contenido").unwrap();
        let (mut c, _cfg) = ctrl_en(src.path());
        let (origin, _dest) = split_a(&mut c, dst.path());
        c.set_area(area()); // F6 resuelve el otro panel con `last_area`: fijarla hace el test determinista

        // Seleccionar el archivo en el panel origen (clic de fila simple).
        c.ws.set_active(origin);
        let pos = active_pos_of(&c, "informe.txt").unwrap();
        c.on_row_clicked(origin, pos, std::time::Instant::now());

        // F6 = MoveToOther. Con dos paneles, el destino se resuelve directo y la op arranca.
        c.on_key(&key_char(slint::platform::Key::F6), false, false, false);
        assert!(drain_ops(&mut c), "el movimiento debe terminar");

        assert!(
            dst.path().join("informe.txt").exists(),
            "el archivo aterrizó en el panel destino"
        );
        assert!(
            !src.path().join("informe.txt").exists(),
            "el archivo se MOVIÓ (ya no está en el origen)"
        );
    }

    // ===================== 5. Copiar archivo entre 2 paneles =================================

    /// GESTO: con dos paneles, seleccionar un archivo y disparar "copiar al otro panel"
    /// (`op_to_other(false)`, la acción CopyToOther). RESULTADO: el archivo está en AMBOS paneles.
    #[test]
    fn copiar_archivo_al_otro_panel() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("foto.txt"), b"data").unwrap();
        let (mut c, _cfg) = ctrl_en(src.path());
        let (origin, _dest) = split_a(&mut c, dst.path());
        c.set_area(area()); // que `op_to_other` resuelva el otro panel por geometría (como en la app)

        c.ws.set_active(origin);
        let pos = active_pos_of(&c, "foto.txt").unwrap();
        c.on_row_clicked(origin, pos, std::time::Instant::now());

        // Copiar al otro panel (CopyToOther no trae atajo por defecto; se dispara por el método,
        // igual que el botón/menú lo cablea en main.rs). `op_to_other` devuelve false para Transfer
        // por diseño (no arranca un LISTADO), así que la op se verifica por el resultado en disco.
        c.op_to_other(false);
        assert!(drain_ops(&mut c), "la copia debe terminar");

        assert!(
            dst.path().join("foto.txt").exists(),
            "la copia aterrizó en el destino"
        );
        assert!(
            src.path().join("foto.txt").exists(),
            "el original sigue en el origen (es COPIA, no movimiento)"
        );
    }

    // ===================== 6. Mover archivo arrastrando (drop con move_hint) ==================

    /// GESTO: arrastrar un archivo del origen y SOLTARLO sobre el centro del panel destino, con el
    /// `move_hint` del OLE (Shift al soltar) → mover. RESULTADO: el archivo está en el destino y no
    /// en el origen. Apunta el drop por hit-testing al rect del panel destino (como en la app).
    #[test]
    fn mover_archivo_arrastrando_al_panel_destino() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("clip.txt"), b"x").unwrap();
        let (mut c, _cfg) = ctrl_en(src.path());
        let (_origin, dest) = split_a(&mut c, dst.path());

        let a = area();
        c.set_area(a);
        let (cx, cy) = pane_center(&c, a, dest);

        // Soltar sobre el panel destino con move_hint=true (Shift real del OLE) → MOVER.
        let routed = c.drop_at(
            cx,
            cy,
            false,
            false,
            vec![src.path().join("clip.txt")],
            true,
        );
        assert!(routed, "el drop debe enrutar al panel destino");
        // CONFIRMAR AL SOLTAR (PUNTO 1b): el drop entre paneles ahora pide confirmación antes de
        // ejecutar; lo confirmamos (equivale a pulsar Mover en el modal).
        assert!(c.confirm_pending_drop(), "confirmar arranca el movimiento");
        assert!(
            drain_ops(&mut c),
            "el movimiento por arrastre debe terminar"
        );

        assert!(
            dst.path().join("clip.txt").exists(),
            "el archivo arrastrado aterrizó en el destino"
        );
        assert!(
            !src.path().join("clip.txt").exists(),
            "el archivo se MOVIÓ por el arrastre"
        );
    }

    // ============== 7. Arrastrar en el mismo disco sin modificadores = mover ==================

    /// GESTO: soltar SIN Ctrl/Shift ni move_hint sobre el panel destino, en el MISMO disco
    /// (tempdirs del mismo volumen). RESULTADO: regla del Explorador → MUEVE por defecto.
    #[test]
    fn arrastrar_mismo_disco_sin_modificadores_mueve() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("nota.txt"), b"x").unwrap();
        let (mut c, _cfg) = ctrl_en(src.path());
        let (_origin, dest) = split_a(&mut c, dst.path());

        let a = area();
        c.set_area(a);
        let (cx, cy) = pane_center(&c, a, dest);

        // ctrl=false, shift=false, move_hint=false → mismo disco → Mover.
        let routed = c.drop_at(
            cx,
            cy,
            false,
            false,
            vec![src.path().join("nota.txt")],
            false,
        );
        assert!(routed, "el drop debe enrutar");
        // CONFIRMAR AL SOLTAR (PUNTO 1b): el drop entre paneles pide confirmación antes de ejecutar.
        assert!(c.confirm_pending_drop(), "confirmar arranca el movimiento");
        assert!(drain_ops(&mut c), "la operación debe terminar");

        assert!(
            dst.path().join("nota.txt").exists(),
            "aterrizó en el destino"
        );
        assert!(
            !src.path().join("nota.txt").exists(),
            "mismo disco sin modificadores: se MOVIÓ"
        );
    }

    // ============== 8. Eliminar permanente con Shift+Supr ====================================

    /// GESTO: seleccionar una fila y pulsar Shift+Supr (borrado PERMANENTE), luego confirmar.
    /// RESULTADO: el archivo ya no existe en disco. Se usa `permanent` para que el borrado quede
    /// DENTRO del tempdir (no toca la papelera real del SO; aislado e irreversible pero seguro).
    #[test]
    fn eliminar_permanente_con_shift_supr() {
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("basura.txt"), b"x").unwrap();
        std::fs::write(work.path().join("queda.txt"), b"x").unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());
        let id = c.ws.active_id().unwrap();

        // Seleccionar la fila a borrar (clic simple).
        let pos = active_pos_of(&c, "basura.txt").unwrap();
        c.on_row_clicked(id, pos, std::time::Instant::now());

        // Shift+Supr → DeletePermanent: abre el modal de confirmación de borrado permanente.
        c.on_key(&key_char(slint::platform::Key::Delete), false, true, false);
        assert!(
            matches!(
                c.ops.pending_dialog,
                Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
                    permanent: true,
                    ..
                })
            ),
            "Shift+Supr debe pedir confirmación de borrado PERMANENTE"
        );

        // Confirmar el borrado (botón Eliminar del modal).
        c.ops.delete_confirm();
        assert!(drain_ops(&mut c), "el borrado debe terminar");

        assert!(
            !work.path().join("basura.txt").exists(),
            "el archivo borrado ya no existe en disco"
        );
        assert!(
            work.path().join("queda.txt").exists(),
            "el otro archivo NO se tocó"
        );
    }

    // ============== 9. Seleccionar todo con Ctrl+A ===========================================

    /// GESTO: Ctrl+A (seleccionar todo). RESULTADO: la selección abarca TODAS las filas de la vista.
    #[test]
    fn seleccionar_todo_con_ctrl_a() {
        let work = tempfile::tempdir().unwrap();
        for n in ["a.txt", "b.txt", "c.txt", "d.txt"] {
            std::fs::write(work.path().join(n), b"x").unwrap();
        }
        let (mut c, _cfg) = ctrl_en(work.path());
        let total = c.ws.active_files().unwrap().view_len();
        assert_eq!(total, 4, "precondición: 4 filas en la vista");

        c.on_key("a", true, false, false); // Ctrl+A → SelectAll

        let f = c.ws.active_files().unwrap();
        assert_eq!(
            f.selection_count(),
            total,
            "Ctrl+A selecciona todas las filas de la vista"
        );
        assert_eq!(
            c.selected_paths().len(),
            total,
            "todas las rutas quedan en la selección efectiva"
        );
    }

    // ============== 10. Selección por rectángulo (rubber-band) ===============================

    /// GESTO: arrastre de selección por rectángulo sobre un rango de filas (`select_rect_range`).
    /// RESULTADO: queda seleccionado exactamente ese rango inclusivo, y nada fuera de él.
    #[test]
    fn seleccion_por_rectangulo_marca_un_rango() {
        let work = tempfile::tempdir().unwrap();
        // Nombres con prefijo numérico para un orden estable y predecible en la vista.
        for n in ["1.txt", "2.txt", "3.txt", "4.txt", "5.txt"] {
            std::fs::write(work.path().join(n), b"x").unwrap();
        }
        let (mut c, _cfg) = ctrl_en(work.path());
        let id = c.ws.active_id().unwrap();
        assert_eq!(c.ws.active_files().unwrap().view_len(), 5);

        // Rubber-band desde la fila 1 hasta la 3 (inclusive), sin Ctrl (reemplaza la selección).
        c.select_rect_range(id, 1, 3, false);

        let f = c.ws.active_files().unwrap();
        assert_eq!(
            f.selection_count(),
            3,
            "el rectángulo abarca 3 filas (1..=3)"
        );
        assert!(!f.is_selected(0), "la fila 0 queda fuera del rango");
        assert!(f.is_selected(1) && f.is_selected(2) && f.is_selected(3));
        assert!(!f.is_selected(4), "la fila 4 queda fuera del rango");
    }

    // ============== 11. Doble clic en carpeta + Backspace para volver ========================

    /// GESTO: doble clic sobre una CARPETA (entra) y luego Backspace (sube/vuelve). RESULTADO: el
    /// panel cambió a la subcarpeta y volvió a la carpeta original. (Doble clic SOLO sobre carpetas
    /// en tests: sobre un archivo abriría el programa del SO.)
    #[test]
    fn navegar_con_doble_clic_y_volver_con_backspace() {
        let work = tempfile::tempdir().unwrap();
        let sub = work.path().join("subcarpeta");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("dentro.txt"), b"x").unwrap();
        let (mut c, _cfg) = ctrl_en(work.path());
        let id = c.ws.active_id().unwrap();
        assert_eq!(c.active_dir().as_deref(), Some(work.path()));

        // Doble clic sobre la carpeta → navega dentro.
        let pos = active_pos_of(&c, "subcarpeta").unwrap();
        c.on_row_double_clicked(id, pos);
        assert!(drain(&mut c), "el listado de la subcarpeta debe terminar");
        assert_eq!(
            c.active_dir().as_deref(),
            Some(sub.as_path()),
            "el doble clic entró a la subcarpeta"
        );

        // Backspace = GoUp → vuelve a la carpeta de arriba.
        c.on_key(
            &key_char(slint::platform::Key::Backspace),
            false,
            false,
            false,
        );
        assert!(drain(&mut c), "el listado al volver debe terminar");
        assert_eq!(
            c.active_dir().as_deref(),
            Some(work.path()),
            "Backspace volvió a la carpeta original"
        );
    }

    // ============== 12. Conflicto al mover: Sobrescribir =====================================

    /// GESTO: mover un archivo a un panel donde YA existe ese nombre → aparece el conflicto por
    /// ítem → el usuario elige "Sobrescribir". RESULTADO: el contenido del origen queda en el
    /// destino (pisó al viejo). Cubre el lazo conflicto → resolve_conflict(Overwrite).
    #[test]
    fn conflicto_al_mover_sobrescribir() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("dato.txt"), b"NUEVO").unwrap();
        std::fs::write(dst.path().join("dato.txt"), b"VIEJO").unwrap();
        let (mut c, _cfg) = ctrl_en(src.path());
        let (origin, _dest) = split_a(&mut c, dst.path());
        c.set_area(area());

        c.ws.set_active(origin);
        let pos = active_pos_of(&c, "dato.txt").unwrap();
        c.on_row_clicked(origin, pos, std::time::Instant::now());

        // Mover al otro panel (F6 / op_to_other(true)): arranca la op, que chocará en el destino.
        // (op_to_other devuelve false para Transfer por diseño; el efecto se ve en el conflicto/disco.)
        c.op_to_other(true);

        // Bucle de drenado que resuelve el conflicto por ítem con Sobrescribir en cuanto aparece.
        let mut resuelto = false;
        let mut termino = false;
        for _ in 0..4000 {
            c.ops.pump_ops();
            if !resuelto {
                if let Some(crate::ops_ctrl::OpDialog::Conflict { op_id, .. }) =
                    c.ops.pending_dialog.clone()
                {
                    c.ops
                        .resolve_conflict(op_id, ConflictAction::Overwrite, false);
                    resuelto = true;
                }
            }
            if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
            {
                termino = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(resuelto, "debe aparecer y resolverse el conflicto");
        assert!(termino, "la operación debe terminar tras resolver");

        assert_eq!(
            std::fs::read_to_string(dst.path().join("dato.txt")).unwrap(),
            "NUEVO",
            "Sobrescribir dejó el contenido del ORIGEN en el destino"
        );
        assert!(
            !src.path().join("dato.txt").exists(),
            "al mover, el origen desaparece tras resolver"
        );
    }

    // ============== 13. Conflicto al mover: Renombrar con nombre nuevo =======================

    /// GESTO: mover un archivo a un panel donde ya existe el nombre → conflicto → el usuario elige
    /// un NOMBRE NUEVO (RenameTo). RESULTADO: existe el nombre nuevo en el destino con el contenido
    /// del origen y el archivo ORIGINAL del destino queda intacto.
    #[test]
    fn conflicto_al_mover_renombrar_con_nombre_nuevo() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("doc.txt"), b"ORIGEN").unwrap();
        std::fs::write(dst.path().join("doc.txt"), b"DESTINO").unwrap();
        let (mut c, _cfg) = ctrl_en(src.path());
        let (origin, _dest) = split_a(&mut c, dst.path());
        c.set_area(area());

        c.ws.set_active(origin);
        let pos = active_pos_of(&c, "doc.txt").unwrap();
        c.on_row_clicked(origin, pos, std::time::Instant::now());
        // op_to_other devuelve false para Transfer por diseño; el efecto se ve en el conflicto/disco.
        c.op_to_other(true);

        let mut resuelto = false;
        let mut termino = false;
        for _ in 0..4000 {
            c.ops.pump_ops();
            if !resuelto {
                if let Some(crate::ops_ctrl::OpDialog::Conflict { op_id, .. }) =
                    c.ops.pending_dialog.clone()
                {
                    c.ops.resolve_conflict(
                        op_id,
                        ConflictAction::RenameTo("doc-copia.txt".into()),
                        false,
                    );
                    resuelto = true;
                }
            }
            if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
            {
                termino = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(resuelto, "debe aparecer y resolverse el conflicto");
        assert!(termino, "la operación debe terminar tras resolver");

        assert_eq!(
            std::fs::read_to_string(dst.path().join("doc-copia.txt")).unwrap(),
            "ORIGEN",
            "el archivo movido quedó con el nombre nuevo y el contenido del origen"
        );
        assert_eq!(
            std::fs::read_to_string(dst.path().join("doc.txt")).unwrap(),
            "DESTINO",
            "el archivo original del destino quedó INTACTO"
        );
    }

    // ============== 14. move_hint fuerza mover aunque ctrl/shift lleguen false ================

    /// REGRESIÓN (bug del Shift): durante el bucle modal del OLE, los flags de teclado de la app
    /// llegan en false porque el modal se traga los eventos. El `move_hint` (Shift REAL al soltar,
    /// que reporta el OLE) DEBE forzar MOVER igualmente. GESTO: drop con ctrl=false, shift=false,
    /// move_hint=true. RESULTADO: movido (creado en destino, borrado del origen).
    #[test]
    fn move_hint_fuerza_mover_con_modificadores_stale() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("reg.txt"), b"x").unwrap();
        let (mut c, _cfg) = ctrl_en(src.path());
        let (_origin, dest) = split_a(&mut c, dst.path());

        let a = area();
        c.set_area(a);
        let (cx, cy) = pane_center(&c, a, dest);

        // Modificadores de la app stale (false), pero move_hint del OLE = true → MOVER.
        let routed = c.drop_at(cx, cy, false, false, vec![src.path().join("reg.txt")], true);
        assert!(routed, "el drop debe enrutar");
        // CONFIRMAR AL SOLTAR (PUNTO 1b): el drop entre paneles pide confirmación antes de ejecutar.
        // El move_hint queda guardado en el pendiente, así que confirmar MUEVE igual.
        assert!(c.confirm_pending_drop(), "confirmar arranca el movimiento");
        assert!(drain_ops(&mut c), "la operación debe terminar");

        assert!(
            dst.path().join("reg.txt").exists(),
            "el archivo aterrizó en el destino"
        );
        assert!(
            !src.path().join("reg.txt").exists(),
            "el move_hint forzó MOVER pese a ctrl/shift en false (regresión del Shift)"
        );
    }
}
