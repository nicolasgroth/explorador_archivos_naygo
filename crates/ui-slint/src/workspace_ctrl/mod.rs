// Naygo — controlador multi-panel de la UI Slint (Fase 2a). Posee el Workspace (varios
// FilePaneState + layout) y traduce gestos a llamadas del core.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use crate::bridge::{
    fav_tree_rows, favorite_rows, frequent_dir_rows, history_rows, inspector_info, recent_rows,
    rows_from_view, rows_from_view_positions, str_to_group_id, tree_rows, FavTreeRow, HistRow,
    InspectorInfo, NavRow, PlainRow, TreeRow, VisibilityFlags,
};
use crate::listing::{Listing, PollBudget};
use naygo_core::favorites::{favorites_path, FavNode, Favorites, NodeId};
use naygo_core::fs_model::{EntryKind, SortKey};
use naygo_core::keymap::Action;
use naygo_core::recent_dirs::RecentDirs;
use naygo_core::tree::DirTree;
#[cfg(test)]
use naygo_core::workspace::layout::DockNode;
use naygo_core::workspace::layout::{
    Rect, SerializableDockLayout, SplitDir, SplitHandle, SplitStep,
};
use naygo_core::workspace::{FilePaneState, PaneId, PanePurpose, Workspace};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use std::time::SystemTime;

const PAGE_ROWS: usize = 20;

#[derive(Default)]
struct StartupCliOverrides {
    layout: Option<String>,
    dir: Option<PathBuf>,
    dir_arg_raw: Option<String>,
    theme: Option<String>,
}

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

/// Resultado terminal o estado del worker de preconsulta de conflictos del drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropProbeEvent {
    Idle,
    Waiting,
    Confirmation,
    Started,
    Failed,
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

/// Resultado de expulsión para aplicar en el hilo UI una vez que termine el worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EjectCompletion {
    pub root: PathBuf,
    pub panes: Vec<PaneId>,
    pub outcome: EjectOutcome,
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
}

/// Estado del selector numérico de panel destino (overlay 1..9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanePick {
    pub action: PaneAction,
    pub origin: PaneId,
    /// Candidatos en orden visual; la posición 0 es el número "1".
    pub candidates: Vec<PaneId>,
}

#[derive(Clone, Debug)]
pub struct DestinationRadar {
    pub move_files: bool,
    /// Fuentes congeladas al abrir; un cambio de foco posterior no cambia la operación.
    pub sources: Vec<PathBuf>,
    pub candidates: Vec<naygo_core::destination_radar::DestinationCandidate>,
    /// La operación que se confirma al elegir un destino. El radar se comparte entre la
    /// selección de un panel Files y la bandeja temporal, sin confundir sus rutas de origen.
    pub origin: DestinationRadarOrigin,
    pub drop_staging: Option<naygo_platform::drop_target::StagedDrop>,
    pub drop_copy_forced: bool,
    pub drop_internal_source: bool,
    pub drop_panes: Vec<PaneId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationRadarOrigin {
    FilesSelection,
    Basket,
    ExternalDrop,
}

/// Estado estable de una sesión de renombrado inline.
///
/// `pos` solo decide dónde pintar el editor. La operación usa siempre `source`, porque el índice
/// visual puede cambiar mientras el watcher incorpora, filtra o reordena entradas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameRequest {
    pub session: u32,
    pub pane: PaneId,
    pub pos: usize,
    pub source: PathBuf,
    pub name: String,
    pub stage: u8,
    /// Offsets UTF-8 (bytes), que es la unidad exigida por `LineEdit::set-selection-offsets`.
    pub selection: (usize, usize),
}

pub struct WorkspaceCtrl {
    pub ws: Workspace,
    /// Posición anterior del panel de operaciones cuando se cerró.
    pub operations_placement: Option<naygo_core::workspace::layout::ClosedPanePlacement>,
    /// Panel abierto automáticamente, que puede cerrarse al finalizar las operaciones.
    pub automatic_operations_pane: Option<PaneId>,
    /// Operaciones ya terminadas al abrir el panel automático; no afectan su cierre posterior.
    pub automatic_operations_prior: HashSet<u64>,
    /// Presentación temporal; nunca se serializa ni sustituye el layout guardado.
    pub maximized_pane: Option<PaneId>,
    /// Configuración de la app (settings + i18n + temas + atajos), cargada del core y
    /// persistida en el directorio portable. El keymap vive aquí (`config.keymap`).
    pub config: crate::config_ctrl::ConfigCtrl,
    /// Resultado acotado de la carga inicial; sólo se drena desde el event loop.
    startup_load_rx: Option<Receiver<Result<crate::startup_load::StartupData, String>>>,
    startup_async_enabled: bool,
    startup_load_failed: bool,
    startup_session_baseline: Option<String>,
    startup_session_dirty: bool,
    startup_load_applied: bool,
    startup_cli_overrides: Option<StartupCliOverrides>,
    startup_warnings: Vec<String>,
    startup_pending_theme: Option<String>,
    /// Un listado en curso por panel (la carpeta de cada panel se lista por separado).
    pub listings: HashMap<PaneId, Listing>,
    /// Admisión común para listados de Files y árbol. El máximo inicial evita un hilo por cada
    /// expansión rápida; los permisos viven en los workers hasta su salida real.
    pub(crate) listing_admission: naygo_core::listing::ListingAdmission,
    /// Última navegación pendiente por panel cuando no hay plaza; una tecla rápida reemplaza la
    /// ruta pendiente en vez de acumular un worker por cada destino intermedio.
    pub(crate) pending_file_listings: HashMap<PaneId, PathBuf>,
    file_listing_cursor: usize,
    /// Un evento durante el snapshot exige una reconciliación al terminar (sin perderlo).
    pub(super) dirty_listings: std::collections::HashSet<PaneId>,
    /// Un árbol de carpetas por panel Tree (lazy; se llena con workers solo-dirs).
    pub trees: HashMap<PaneId, DirTree>,
    /// Snapshot de unidades entregado por el worker. La UI sólo lee este caché; Win32/USB nunca
    /// se consulta desde callbacks ni al construir/restaurar un árbol.
    pub drive_snapshot: Vec<naygo_platform::drives::DriveInfo>,
    drive_refresh_rx: Option<Receiver<Vec<naygo_platform::drives::DriveInfo>>>,
    drive_refresh_pending: bool,
    /// Expulsión segura con validación fresca y llamadas Win32 fuera del hilo de UI.
    eject_rx: Option<Receiver<EjectCompletion>>,
    /// Cursor de teclado por árbol (la fila enfocada para ↑↓←→). Si falta, se siembra con la
    /// carpeta activa del árbol o su primera raíz. Distinto de `active_path` (la carpeta navegada):
    /// el cursor se mueve sin navegar; Enter/→ sí navegan.
    pub tree_cursor: HashMap<PaneId, PathBuf>,
    /// Un worker solo-directorios por rama de árbol en vuelo (clave: panel + carpeta).
    pub tree_listings: HashMap<(PaneId, PathBuf), Listing>,
    /// Ramas que esperan una plaza de worker, coalescidas por panel y carpeta.
    pub(crate) pending_tree_listings: std::collections::HashSet<(PaneId, PathBuf)>,
    tree_listing_cursor: usize,
    /// Destino a "revelar" en cada árbol: al navegar el Files activo, el árbol expande
    /// progresivamente los ancestros hasta esta carpeta (reveal). Se limpia al llegar.
    pub reveal_targets: HashMap<PaneId, PathBuf>,
    pub tree_refresh_pending: HashMap<PaneId, Vec<PathBuf>>,
    /// Favoritos (global), ahora un ÁRBOL de grupos anidados. Se carga de
    /// `<config>/favorites.json` al arrancar y se persiste tras cada cambio (anclar/quitar,
    /// nuevo grupo, renombrar, eliminar, mover).
    pub favorites: Favorites,
    startup_favorites_dirty: std::cell::Cell<bool>,
    last_favorites_save: Mutex<Option<naygo_core::config::SaveTicket>>,
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
    /// Rutas reunidas desde paneles/carpetas distintas. Su contenido es temporal y no se
    /// serializa, aunque el panel Basket sí puede permanecer en la disposición.
    pub basket: naygo_core::basket::SelectionBasket,
    /// Stagings virtuales referenciados por ítems de la bandeja. Se conservan mientras haya
    /// elementos en ella y se transfieren al motor al copiar/mover/borrar desde la bandeja.
    pub basket_staging: Vec<naygo_platform::drop_target::StagedDrop>,
    /// Lectura async de una bandeja .naygolist elegida por el usuario.
    pub basket_import_rx: Option<std::sync::mpsc::Receiver<Result<basket::BasketImport, String>>>,
    /// Referencias importadas que ya no existen. Se calcula en el worker de importación y se
    /// conserva para que la bandeja las muestre, en vez de descartarlas silenciosamente.
    pub basket_missing: std::collections::HashSet<PathBuf>,
    /// Ítem de bandeja que alimenta Preview/Propiedades. Solo vive mientras el usuario navega
    /// entre Bandeja, Preview e Inspector; al volver a Files se prioriza su selección normal.
    pub basket_selection: naygo_core::basket::BasketSelection,
    pub basket_context: bool,
    pub delivery: delivery::DeliveryState,
    pub task_spaces: task_spaces::TaskSpacesState,
    pub retry: retry::RetryState,
    pub saved_queries: saved_queries::SavedQueriesState,
    pub comparison_points: comparison_points::ComparisonPointsState,
    pub recipes: recipes::RecipesState,
    pub search_context: bool,
    pub delivery_results: delivery::DeliveryResults,
    /// Asistente de sincronización en curso (planificación/preview). Todo el recorrido vive en
    /// su worker y se cancela al cerrar o cambiar opciones.
    pub sync_assistant: Option<sync_assistant::SyncAssistantState>,
    /// Modal/job de transformación de texto. El diagnóstico y la escritura corren en workers.
    pub text_transform: Option<text_transform::TextTransformState>,
    /// Selector de panel destino en curso (overlay 1..9), si lo hay.
    pub pending_pick: Option<PanePick>,
    /// Radar F-6 de destinos no limitado a paneles abiertos.
    pub destination_radar: Option<DestinationRadar>,
    /// Última área de contenido conocida (la setea la UI en cada layout) para resolver
    /// destinos por orden visual desde gestos que no traen el área (p. ej. teclado).
    pub last_area: Rect,
    /// Panel Files que está BAJO el cursor mientras se ARRASTRAN archivos sobre la ventana
    /// (resaltado en vivo: borde + título). Lo actualiza la UI desde los eventos de hover del
    /// `IDropTarget` (`DragHover`), y se limpia al salir/soltar. `None` = no se está arrastrando
    /// sobre ningún panel Files. Estado de UI transitorio: no se persiste. La UI lo refleja en
    /// `PaneVm.drag-over` (true solo para el panel cuyo id coincide).
    pub drag_over_pane: Option<PaneId>,
    /// Posición Y lógica de cliente del cursor durante el último `DragOver`. Se entrega a Slint
    /// para que `FilePanel` determine la fila que está bajo el cursor sin hacer I/O ni aproximar
    /// la geometría interna de la tabla. Estado efímero, solo de la interacción actual.
    pub drag_over_client_y: Option<f32>,
    /// Fila de vista bajo el cursor durante el arrastre OLE. La reporta `FilePanel` mediante su
    /// hit-test de `body-touch`; `drop_at` la transforma a una carpeta destino si corresponde.
    /// `None` representa encabezado, zona vacía o un panel que no es Files.
    pub drag_over_row: Option<(PaneId, usize)>,
    /// Drop intra-app (entre paneles) en espera de la preconsulta de conflicto o de confirmación.
    /// `drop_at` ya validó el destino; el worker decide si corresponde el modal de conflicto o el
    /// modal genérico "¿Copiar/Mover N a «destino»?". Al cancelar, `cancel_pending_drop`
    /// lo descarta. `None` = no hay drop pendiente. Estado transitorio: no se persiste.
    pub pending_drop: Option<PendingDrop>,
    /// Worker de la preconsulta de conflictos de drop. Se admite un único probe en vuelo para
    /// que un share lento no cree hilos ilimitados; los nuevos drops se consumen y notifican.
    pub drop_collision_rx: Option<Receiver<Result<bool, String>>>,
    /// Cancelación cooperativa entre rutas del probe de conflictos. No puede interrumpir una
    /// syscall de filesystem que haya quedado bloqueada dentro del SO.
    pub drop_collision_cancel: Option<naygo_core::CancellationToken>,
    /// Drop libre cuya confirmación genérica ya puede abrirse en UI.
    pub pending_drop_confirmation: bool,
    /// Exige confirmar el destino completo para un drop OLE externo.
    pub pending_drop_external_confirmation: bool,
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
    /// Nº de filas que matchean el filtro visual por tipeo en el panel activo (lo refresca
    /// `rows_of` al reconstruir las filas; el footer lo muestra como "{n} coincidencias").
    pub filter_match_count: usize,
    /// Si está activo, el filtro de tipeo elimina de la vista las filas que no coinciden. Es
    /// efímero: solo controla la búsqueda actual y no se guarda en settings.
    pub filter_hide_nonmatches: bool,
    /// Última carpeta PEDIDA para listar por panel (incluye las que ya terminaron: el mapa
    /// `listings` las olvida al drenar). Es la referencia fiable para saber si un
    /// `start_listing` es NAVEGACIÓN (cambia la carpeta → limpiar el filtro por tipeo) o
    /// REFRESH de la misma (lo conserva). `f.current_dir` no sirve: hay caminos que la
    /// mutan ANTES de llamar a `start_listing` (navigate_pane_to).
    pub last_listing_dirs: HashMap<PaneId, std::path::PathBuf>,
    pub ctrl_down: bool,
    pub shift_down: bool,
    /// Menú contextual abierto (clic derecho): posición (x,y en la ventana) y rutas objetivo.
    pub context_menu: Option<ContextMenuState>,
    /// Menú/editor de columna abierto (clic derecho en el header, F2): panel, columna y posición.
    pub column_menu: Option<ColumnMenuState>,
    /// Plantillas de disposición del usuario + recientes (templates.json). Los built-in viven en
    /// código (`LayoutTemplate::builtins`). Fase 4.
    pub templates: naygo_core::workspace::TemplateStore,
    startup_templates_dirty: std::cell::Cell<bool>,
    last_templates_save: Mutex<Option<naygo_core::config::SaveTicket>>,
    /// Ventana de renombrado por lotes abierta (Fase 5): el spec en edición + los ítems objetivo.
    pub batch: Option<BatchRenameState>,
    /// Modal "nueva(s) carpeta(s)" abierto: carpeta destino + texto multilínea en edición.
    pub new_folder: Option<NewFolderState>,
    /// La ayuda (F1) está abierta.
    pub help_open: bool,
    /// Cálculo de tamaño de carpeta en curso (acción configurable), si lo hay. Un solo job a la
    /// vez cancela y reemplaza el anterior. El resultado se muestra en la barra de estado.
    pub size_job: Option<SizeJob>,
    /// Lectura de metadata por tipo (dimensiones de imagen, versión de exe) del archivo enfocado,
    /// en un hilo worker. Un solo job a la vez: enfocar otro archivo cancela y reemplaza. `None`
    /// = sin metadata (carpeta o nada enfocado). Ver `MetaJob` y `meta.rs`.
    pub meta_job: Option<meta::MetaJob>,
    /// Resultado de quitar Zone.Identifier. La escritura ADS ocurre exclusivamente en un worker;
    /// el tick vuelve a pedir metadata al terminar para refrescar el Inspector.
    pub zone_unblock_rx: Option<std::sync::mpsc::Receiver<(PathBuf, Result<bool, String>)>>,
    /// Búsqueda recursiva en curso/terminada (F3 / Ctrl+F / lupa), si la hay. Mientras esté presente
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
    /// Pedido pendiente de mostrar el editor inline. Se separa de `rename_active`: consumir el
    /// pedido no debe perder la identidad estable del archivo que se está editando.
    pub rename_requested: Option<RenameRequest>,
    /// Sesión de rename actualmente abierta. Conserva la ruta original para que un refresh,
    /// filtro o reordenamiento del watcher nunca haga que el commit opere sobre otra fila.
    pub rename_active: Option<RenameRequest>,
    /// Secuencia monotónica para distinguir callbacks tardíos de editores ya cerrados. Slint
    /// puede emitir `has-focus = false` al destruir un LineEdit; el id evita un segundo commit.
    next_rename_session: u32,
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
    /// Error pendiente de una operación Shell; el tick lo convierte en toast localizado.
    pub pending_shell_error: Option<String>,
    /// Error de listado (puede indicar resultados parciales). El tick lo muestra sin modal.
    pub pending_listing_error: Option<String>,
    /// Marcas persistentes de la última comparación de dos paneles. El valor distingue
    /// metadatos diferentes (1) de un nombre presente solo en ese lado (2).
    pub comparison: HashMap<PaneId, HashMap<PathBuf, u8>>,
    /// Paneles comparados por última vez. Esta relación sobrevive a invalidar las marcas: el
    /// enlace relativo no debe reutilizar un snapshot viejo, pero sí puede seguir activo.
    pub comparison_pair: Option<(PaneId, PaneId)>,
    /// Enlace relativo opcional de `comparison_pair` (F-8).
    pub comparison_link_enabled: bool,
    /// Revisión O(1) que invalida el modelo de filas al comparar o limpiar la comparación.
    pub comparison_revision: u64,
    /// Snapshots completos ya listados, solo durante esta sesión (F-7).
    pub listing_snapshots: naygo_core::listing_snapshot::ListingSnapshotStore,
    /// Cambios de la última visita por panel; los ausentes son solo informativos.
    pub visit_changes: HashMap<PaneId, Vec<naygo_core::listing_changes::ListingChange>>,
    /// Índice de rutas New/Modified del resultado vigente; evita buscar todos los cambios por fila.
    visit_highlights: HashMap<PaneId, std::collections::HashSet<PathBuf>>,
    pub visit_changes_revision: u64,
    /// Cache de íconos (PNG → slint::Image, decodificado una vez por set+clave). Lo posee el
    /// controlador para resolver el ícono de cada fila al pintarla. Su set activo lo fija la
    /// configuración (Apariencia → Set de íconos). Ver `crate::icons::IconCache`.
    pub icons: crate::icons::IconCache,
    /// Caché del uso de disco por raíz de unidad (p. ej. `C:\`), para el footer de cada panel.
    /// Evita pegarle a WinAPI en CADA tick de `sync_rows`: se lee una vez por unidad y se reusa.
    /// Se vacía al navegar (vía `start_listing`) para que el espacio libre se refresque al
    /// entrar a otra carpeta/unidad. Ver `footer_text_for`.
    footer_disk_cache: std::collections::HashMap<std::path::PathBuf, naygo_core::disk::DiskUsage>,
    /// Raíces cuyo espacio se está leyendo en un worker. Impide lanzar un hilo por tick mientras
    /// una unidad de red lenta todavía responde. El resultado llega por `footer_disk_results`.
    footer_disk_pending: std::collections::HashSet<std::path::PathBuf>,
    footer_disk_sender: Sender<(std::path::PathBuf, Option<naygo_core::disk::DiskUsage>)>,
    footer_disk_results: Receiver<(std::path::PathBuf, Option<naygo_core::disk::DiskUsage>)>,
    /// Paneles cuya carpeta se perdió por una expulsión de disco (no por "carpeta borrada").
    /// Cambia solo el TEXTO del aviso in-place ("disco expulsado" vs "carpeta no encontrada").
    /// Se limpia cuando el panel navega a una carpeta válida.
    pub ejected_panes: std::collections::HashSet<u64>,
    /// Caché del estado "carpeta no encontrada" por panel: `(carpeta evaluada, missing,
    /// ancestro existente más cercano)`. Evita `read_dir`/`exists` síncronos en UI en CADA
    /// tick de `sync_rows` (un share de red caído podía bloquear la UI segundos). Se recalcula
    /// solo ante eventos reales (navegar, reintentar, expulsar, cambio de discos) vía
    /// `refresh_missing_cache`, que lanza un worker (el I/O tampoco corre en la UI al navegar).
    /// La carpeta evaluada invalida entradas obsoletas al navegar (ver `pane_dir_missing`).
    missing_cache: std::collections::HashMap<u64, (PathBuf, bool, Option<PathBuf>)>,
    /// Probe async de "carpeta no encontrada" en vuelo (uno a lo sumo; un refresh nuevo lo
    /// reemplaza por una marca de refresh pendiente, sin abrir workers adicionales. El tick lo
    /// drena con `pump_missing_probe`.
    missing_probe: Option<std::sync::mpsc::Receiver<Vec<listing::MissingProbeHit>>>,
    missing_probe_cancel: Option<naygo_core::cancel::CancellationToken>,
    missing_probe_refresh_pending: bool,
    /// Último ámbito solicitado: un panel al navegar o `None` para eventos globales.
    missing_probe_pending_pane: Option<Option<PaneId>>,
    missing_probe_revision: u64,
    /// Decisión de pegado pendiente. Las colisiones de nombres se consultan en un solo worker;
    /// mientras esté ocupado no se lanza otro para evitar acumular threads contra una red lenta.
    paste_probe: Option<(
        PathBuf,
        std::sync::mpsc::Receiver<Result<naygo_core::clipboard::PastePlan, String>>,
    )>,
    /// Error al consultar nombres destino para el pegado, pendiente de mostrarse como toast.
    pending_paste_probe_error: Option<String>,
    /// Lectura externa pendiente al pegar elementos en la Cesta.
    basket_clipboard_rx: Option<std::sync::mpsc::Receiver<naygo_core::clipboard::ClipboardContent>>,
    /// Recientes DIFERIDOS al resultado del listado, por panel. `navigate_active_to` navega sin
    /// chequeo síncrono de navegabilidad (un `read_dir` en la UI congela contra un share caído);
    /// en su lugar registra aquí la carpeta y `pump_listings` la empuja a recientes SOLO si el
    /// listado terminó con éxito.
    pending_recents: HashMap<PaneId, PathBuf>,
    /// Autocompletado async del editor de ruta (path-bar): worker con debounce, para que tipear
    /// no haga un `read_dir` por tecla en el hilo de UI. Ver `input.rs`.
    autocomplete: input::AutocompleteState,
    /// Autocompletado independiente de la raíz del panel Search. Así no se cruzan resultados
    /// si una path-bar y el buscador se editan en paneles distintos.
    search_autocomplete: input::AutocompleteState,
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
    /// El objetivo del menú es una carpeta (habilita el submenú "Abrir ▸"). Se calcula UNA VEZ
    /// al abrir el menú (no en cada tick): `folder_mode`, o clic sobre una única fila que es un
    /// directorio. Cachearlo evita un `stat` por tick que en un share de red lento sería costoso.
    pub target_is_folder: bool,
    /// El menú se abrió sobre un breadcrumb/árbol y su carpeta puede diferir de la actual.
    pub show_open_here: bool,
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
#[derive(Clone, Debug)]
pub struct PendingDrop {
    /// Archivos a copiar/mover (las rutas de origen del OLE).
    pub paths: Vec<PathBuf>,
    /// Carpeta destino resuelta (el panel bajo el cursor al soltar).
    pub dest_dir: PathBuf,
    /// Panel destino (el que estaba bajo el cursor): se activa al confirmar, para que el foco
    /// quede donde el usuario soltó los archivos.
    pub dest_pane: PaneId,
    /// `true` = mover, `false` = copiar (ya decidido por modificadores + mismo disco).
    pub is_move: bool,
    /// Cuántos elementos (para el texto del modal; == `paths.len()`).
    pub count: usize,
    /// Mantiene vivo el staging cuando el origen proviene de 7-Zip/WinRAR u otra fuente virtual.
    pub staging: Option<naygo_platform::drop_target::StagedDrop>,
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
    pub selected: bool,
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
    pub details: saved_queries::SearchDetails,
    /// Carpeta raíz bajo la que se busca (por defecto la del panel activo al disparar).
    pub root: PathBuf,
    /// Texto de nombre/patrón buscado (lo que el usuario tipeó; se muestra en el encabezado).
    pub query: String,
    /// Criterios completos, incluidos contenido y opciones de comparación/recorrido.
    pub options: naygo_core::search::SearchOptions,
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

// --- Submódulos por responsabilidad (O-12) ---
// Cada uno aporta su propio `impl WorkspaceCtrl`. Rust permite varios `impl` del
// mismo tipo repartidos en módulos del mismo crate.
/// Mezcla en `hasher` los campos del Entry que determinan una fila, para la firma de la vista
/// profunda. La vista normal usa revisiones y deltas del watcher.
fn hash_entry_for_row<H: std::hash::Hasher>(e: &naygo_core::fs_model::Entry, hasher: &mut H) {
    use std::hash::Hash;
    e.name.hash(hasher);
    e.path.hash(hasher);
    std::mem::discriminant(&e.kind).hash(hasher);
    e.size.hash(hasher);
    e.modified.hash(hasher);
    e.created.hash(hasher);
}

mod basket;
mod columns;
mod comparison_points;
mod context;
mod delivery;
mod drive_io;
mod favorites;
mod input;
mod layout_panes;
mod listing;
mod meta;
mod navigation;
mod ops;
mod recipes;
mod retry;
mod saved_queries;
mod session;
mod sync_assistant;
mod task_spaces;
mod templates;
mod text_transform;
mod tree;

impl WorkspaceCtrl {
    /// Arranca con UN panel Files en `start` (el usuario agrega más con el botón). Lanza
    /// su listado inicial. La configuración se carga del directorio portable.
    #[cfg(test)]
    pub fn new(start: std::path::PathBuf) -> WorkspaceCtrl {
        WorkspaceCtrl::new_in(start, naygo_core::config::portable_dir())
    }

    /// Arranque de producción: recibe el directorio de config resuelto en main sin I/O; evita
    /// explorar catálogos de usuario en el hilo UI, que se hidratan después desde PackIo.
    pub fn new_for_ui(
        start: std::path::PathBuf,
        config_dir: std::path::PathBuf,
        data_dir: std::path::PathBuf,
    ) -> WorkspaceCtrl {
        let mut ctrl = WorkspaceCtrl::new_with_catalog_mode(start, config_dir, data_dir, false);
        ctrl.startup_async_enabled = true;
        ctrl
    }

    /// Como `new`, pero con el directorio de configuración explícito (para tests: permite
    /// usar un directorio temporal en vez del portable).
    #[cfg(test)]
    pub fn new_in(start: std::path::PathBuf, config_dir: std::path::PathBuf) -> WorkspaceCtrl {
        WorkspaceCtrl::new_with_catalog_mode(start, config_dir.clone(), config_dir, true)
    }

    fn new_with_catalog_mode(
        start: std::path::PathBuf,
        config_dir: std::path::PathBuf,
        data_dir: std::path::PathBuf,
        load_external_catalogs: bool,
    ) -> WorkspaceCtrl {
        let mut ws = Workspace::new();
        let id = ws.add_pane(PanePurpose::Files, start.clone());
        ws.layout = SerializableDockLayout::single(id);
        ws.set_active(id);
        let config = if load_external_catalogs {
            crate::config_ctrl::ConfigCtrl::new_with_data_dir(config_dir.clone(), data_dir.clone())
        } else {
            crate::config_ctrl::ConfigCtrl::new_for_ui(config_dir.clone(), data_dir)
        };
        // El uso de disco puede bloquear en un volumen de red. El canal permite que el worker
        // publique el dato sin tocar Slint ni el controlador desde otro hilo.
        let (footer_disk_sender, footer_disk_results) = mpsc::channel();
        let mut icons = crate::icons::IconCache::new_with_legacy(
            config.settings.icon_set.clone(),
            config.icon_data_dir.clone(),
            config_dir.clone(),
        );
        icons.set_system_file_icons(config.settings.use_system_file_icons);
        // Sembrar overrides + tinte al construir, para que la primera pintura ya sea correcta.
        icons.set_overrides(config.settings.icon_overrides.clone());
        {
            let tintable = config.icon_sets.is_tintable(&config.settings.icon_set);
            let rgb = crate::theme_text_rgb(&config.settings, &config.themes);
            icons.set_tint(tintable, rgb);
        }
        let initial_favorites = if load_external_catalogs {
            load_favorites(&config_dir)
        } else {
            Favorites::new()
        };
        let mut task_spaces = task_spaces::TaskSpacesState::default();
        task_spaces.merge_persisted_recents(config.settings.task_space_recents.clone());
        task_spaces.global_favorites = initial_favorites.clone();
        let mut c = WorkspaceCtrl {
            ws,
            operations_placement: None,
            automatic_operations_pane: None,
            automatic_operations_prior: HashSet::new(),
            maximized_pane: None,
            config,
            startup_load_rx: None,
            startup_async_enabled: false,
            startup_load_failed: false,
            startup_session_baseline: None,
            startup_session_dirty: false,
            startup_load_applied: false,
            startup_cli_overrides: None,
            startup_warnings: Vec::new(),
            startup_pending_theme: None,
            listings: HashMap::new(),
            listing_admission: naygo_core::listing::ListingAdmission::new(8),
            pending_file_listings: HashMap::new(),
            file_listing_cursor: 0,
            dirty_listings: std::collections::HashSet::new(),
            trees: HashMap::new(),
            drive_snapshot: Vec::new(),
            drive_refresh_rx: None,
            drive_refresh_pending: false,
            eject_rx: None,
            tree_cursor: HashMap::new(),
            tree_listings: HashMap::new(),
            pending_tree_listings: std::collections::HashSet::new(),
            tree_listing_cursor: 0,
            reveal_targets: HashMap::new(),
            tree_refresh_pending: HashMap::new(),
            favorites: initial_favorites.clone(),
            startup_favorites_dirty: std::cell::Cell::new(false),
            last_favorites_save: Mutex::new(None),
            fav_expanded: std::collections::HashSet::new(),
            recents: RecentDirs::new(),
            ops: crate::ops_ctrl::OpsCtrl::new(config_dir.clone()),
            preview: crate::preview::PreviewState::new(),
            basket: naygo_core::basket::SelectionBasket::new(),
            basket_staging: Vec::new(),
            basket_import_rx: None,
            basket_missing: std::collections::HashSet::new(),
            basket_selection: Default::default(),
            basket_context: false,
            delivery: Default::default(),
            task_spaces,
            retry: Default::default(),
            saved_queries: Default::default(),
            comparison_points: Default::default(),
            recipes: Default::default(),
            search_context: false,
            delivery_results: Default::default(),
            sync_assistant: None,
            text_transform: None,
            pending_pick: None,
            destination_radar: None,
            last_area: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            drag_over_pane: None,
            drag_over_client_y: None,
            drag_over_row: None,
            pending_drop: None,
            drop_collision_rx: None,
            drop_collision_cancel: None,
            pending_drop_confirmation: false,
            pending_drop_external_confirmation: false,
            last_click: None,
            last_open: None,
            typeahead: String::new(),
            typeahead_at: None,
            filter_match_count: 0,
            filter_hide_nonmatches: false,
            last_listing_dirs: HashMap::new(),
            ctrl_down: false,
            shift_down: false,
            context_menu: None,
            column_menu: None,
            templates: if load_external_catalogs {
                naygo_core::config::load_templates(&config_dir)
            } else {
                naygo_core::workspace::TemplateStore::default()
            },
            startup_templates_dirty: std::cell::Cell::new(false),
            last_templates_save: Mutex::new(None),
            batch: None,
            new_folder: None,
            help_open: false,
            size_job: None,
            meta_job: None,
            zone_unblock_rx: None,
            search_job: None,
            deep_job: None,
            last_saved_fingerprint: None,
            watchers: crate::watch::Watchers::new(),
            last_active_files: Some(id),
            edit_path_requested: None,
            rename_requested: None,
            rename_active: None,
            next_rename_session: 1,
            open_palette_requested: false,
            palette_theme_requested: None,
            open_config_requested: false,
            toolbar_menu_requested: None,
            pending_shell_error: None,
            pending_listing_error: None,
            comparison: HashMap::new(),
            comparison_pair: None,
            comparison_link_enabled: false,
            comparison_revision: 0,
            listing_snapshots: naygo_core::listing_snapshot::ListingSnapshotStore::default(),
            visit_changes: HashMap::new(),
            visit_highlights: HashMap::new(),
            visit_changes_revision: 0,
            icons,
            footer_disk_cache: std::collections::HashMap::new(),
            footer_disk_pending: std::collections::HashSet::new(),
            footer_disk_sender,
            footer_disk_results,
            ejected_panes: std::collections::HashSet::new(),
            missing_cache: std::collections::HashMap::new(),
            missing_probe: None,
            missing_probe_cancel: None,
            missing_probe_refresh_pending: false,
            missing_probe_pending_pane: None,
            missing_probe_revision: 0,
            paste_probe: None,
            pending_paste_probe_error: None,
            basket_clipboard_rx: None,
            pending_recents: HashMap::new(),
            autocomplete: input::AutocompleteState::new(),
            search_autocomplete: input::AutocompleteState::new(),
        };
        c.push_recent(start.clone());
        c.start_listing(id, start);
        // Aplicar el modo de operaciones (cola/paralelo) guardado en Settings al motor de ops.
        c.sync_ops_mode();
        c
    }

    /// Indica que producción inició sincrónicamente sólo con defaults embebidos y espera
    /// el resultado del worker de carga inicial.
    pub fn startup_loading(&self) -> bool {
        self.startup_async_enabled
    }

    pub(crate) fn start_startup_load(&mut self, waker: naygo_platform::dir_watch::Waker) {
        if self.startup_async_enabled && self.startup_load_rx.is_none() {
            self.startup_load_rx = Some(crate::startup_load::spawn(
                self.config.config_dir.clone(),
                waker,
            ));
        }
    }

    /// Sella el estado inicial de UI después de aplicar el layout de primera ejecución y los
    /// argumentos de CLI. La sesión persistida sólo puede reemplazar ese estado si no cambió.
    pub fn mark_startup_session_baseline(&mut self) {
        self.startup_session_baseline = Some(self.session_fingerprint());
    }

    pub fn startup_load_applied(&self) -> bool {
        self.startup_load_applied
    }

    pub fn additional_persistence_tickets(&self) -> Vec<naygo_core::config::SaveTicket> {
        self.last_favorites_save
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .into_iter()
            .chain(
                self.last_templates_save
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone(),
            )
            .collect()
    }

    pub fn defer_startup_cli_overrides(
        &mut self,
        layout: Option<String>,
        dir: Option<PathBuf>,
        dir_arg_raw: Option<String>,
        theme: Option<String>,
    ) {
        self.startup_cli_overrides = Some(StartupCliOverrides {
            layout,
            dir,
            dir_arg_raw,
            theme,
        });
    }

    pub fn take_startup_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.startup_warnings)
    }

    pub(crate) fn take_startup_pending_theme(&mut self) -> Option<String> {
        self.startup_pending_theme.take()
    }

    pub(crate) fn startup_catalog_theme(&self) -> Option<String> {
        self.startup_pending_theme.clone()
    }

    /// Drena el único resultado inicial sin esperar. Devuelve true sólo cuando la UI debe
    /// repintar configuración, layout y filas después de aplicar el snapshot.
    pub fn pump_startup_load(&mut self) -> bool {
        let result = match self.startup_load_rx.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return false,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.startup_load_rx = None;
                    self.startup_async_enabled = false;
                    self.startup_load_failed = true;
                    self.config.finish_startup_without_data();
                    crate::logging::log_line("worker de configuración inicial desconectado");
                    return false;
                }
            },
            None => return false,
        };
        self.startup_load_rx = None;
        self.startup_async_enabled = false;
        let data = match result {
            Ok(data) => data,
            Err(error) => {
                self.startup_load_failed = true;
                self.config.finish_startup_without_data();
                crate::logging::log_line(&format!(
                    "falló la carga inicial de configuración: {error}"
                ));
                return false;
            }
        };

        let persisted_space_recents = data.settings.task_space_recents.clone();
        self.config.apply_startup_config(
            data.settings,
            data.i18n,
            data.keymap,
            data.first_launch_save_ticket,
        );
        self.task_spaces
            .merge_persisted_recents(persisted_space_recents);
        if self.task_spaces.recents != self.config.settings.task_space_recents {
            self.persist_task_space_recents();
        }
        if self.startup_favorites_dirty.get() {
            let mut loaded = data.favorites;
            loaded.merge_startup_additions(&self.favorites);
            self.favorites = loaded;
            self.task_spaces.global_favorites = self.favorites.clone();
            self.persist_favorites();
        } else {
            self.favorites = data.favorites;
            self.task_spaces.global_favorites = self.favorites.clone();
        }
        if self.startup_templates_dirty.get() {
            let mut loaded = data.templates;
            loaded.merge_startup_edits(&self.templates);
            self.templates = loaded;
            self.persist_templates();
        } else {
            self.templates = data.templates;
        }
        let baseline_matches = self
            .startup_session_baseline
            .as_ref()
            .is_some_and(|baseline| baseline == &self.session_fingerprint());
        if baseline_matches {
            if let Some(session) = data.workspace {
                let _ = self.apply_loaded_session(session);
            }
            if let Some(cli) = self.startup_cli_overrides.take() {
                if let Some(name) = cli.layout.as_deref() {
                    if !self.apply_template_ephemeral(name) {
                        self.startup_warnings
                            .push(format!("La plantilla \"{name}\" no existe; se ignoró."));
                    }
                }
                if let Some(dir) = cli.dir {
                    self.navigate_active_to(dir);
                } else if let Some(raw) = cli.dir_arg_raw {
                    self.startup_warnings
                        .push(format!("La ruta \"{raw}\" no es una carpeta; se ignoró."));
                }
                if let Some(id) = cli.theme.as_deref() {
                    self.startup_pending_theme = Some(id.to_string());
                }
            }
        } else if self.startup_cli_overrides.take().is_some() {
            self.startup_warnings.push(
                "Se omitieron argumentos de inicio porque la navegación cambió mientras cargaba la configuración.".to_string(),
            );
        }
        if self.startup_session_dirty {
            match self.save_session_async() {
                Ok(_) => self.last_saved_fingerprint = Some(self.session_fingerprint()),
                Err(error) => crate::logging::log_line(&format!(
                    "no se pudo encolar la sesión editada durante el arranque: {error}"
                )),
            }
            self.startup_session_dirty = false;
        }

        self.sync_ops_mode();
        self.icons
            .set_overrides(self.config.settings.icon_overrides.clone());
        let tintable = self
            .config
            .icon_sets
            .is_tintable(&self.config.settings.icon_set);
        let rgb = crate::theme_text_rgb(&self.config.settings, &self.config.themes);
        self.icons.set_tint(tintable, rgb);
        self.icons
            .set_system_file_icons(self.config.settings.use_system_file_icons);
        self.startup_load_applied = true;
        true
    }

    /// Aplica `op` al panel activo (helper para no repetir el match de préstamos).
    pub(super) fn with_active(&mut self, op: impl FnOnce(&mut FilePaneState)) {
        if let Some(f) = self.ws.active_files_mut() {
            op(f);
        }
    }

    pub(super) fn report_shell_result(
        &mut self,
        result: Result<(), naygo_platform::open::ShellError>,
    ) {
        if let Err(error) = result {
            crate::logging::log_line(&format!("operación Shell falló: {error}"));
            self.pending_shell_error = Some(error.to_string());
        }
    }

    pub fn take_shell_error(&mut self) -> Option<String> {
        self.pending_shell_error.take()
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

/// Datos crudos del footer de un panel (todo MENOS el disco) + la ruta que identifica el
/// volumen para consultar/cachear su espacio. Puro sobre el `FilePaneState`: lo llama `footer_text_of` bajo un borrow
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
    // Windows obtiene la raíz de unidad (p. ej. `C:\`). En Linux se pasa la carpeta actual:
    // statvfs resuelve el montaje que la contiene, incluso si no es `/`.
    #[cfg(windows)]
    let root = files
        .current_dir
        .ancestors()
        .last()
        .map(std::path::Path::to_path_buf);
    #[cfg(target_os = "linux")]
    let root = Some(files.current_dir.clone());
    #[cfg(not(any(windows, target_os = "linux")))]
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

/// ¿La ruta `path` está dentro del disco cuya raíz es `drive_root`? Compara por la LETRA
/// de unidad (primer componente con `:`), case-insensitive (Windows). Una ruta UNC
/// (`\\srv\share`) o sin letra de unidad nunca está "en" un disco con letra. Pura y testeable.
fn path_is_on_drive(path: &std::path::Path, drive_root: &std::path::Path) -> bool {
    fn drive_letter(p: &std::path::Path) -> Option<char> {
        let s = p.to_string_lossy();
        let bytes = s.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            Some((bytes[0] as char).to_ascii_uppercase())
        } else {
            None
        }
    }
    match (drive_letter(path), drive_letter(drive_root)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
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
mod tests;
#[cfg(test)]
mod tests_simular;
