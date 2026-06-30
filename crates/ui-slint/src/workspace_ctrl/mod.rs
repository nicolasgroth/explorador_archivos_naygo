// Naygo — controlador multi-panel de la UI Slint (Fase 2a). Posee el Workspace (varios
// FilePaneState + layout) y traduce gestos a llamadas del core.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

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
    /// Paneles cuya carpeta se perdió por una expulsión de disco (no por "carpeta borrada").
    /// Cambia solo el TEXTO del aviso in-place ("disco expulsado" vs "carpeta no encontrada").
    /// Se limpia cuando el panel navega a una carpeta válida.
    pub ejected_panes: std::collections::HashSet<u64>,
    /// Caché del estado "carpeta no encontrada" por panel: `(missing, has_existing_ancestor)`.
    /// Evita el `read_dir`/`exists` SÍNCRONO en el hilo de UI en CADA tick de `sync_rows` (un
    /// share de red caído podía bloquear la UI segundos). Se recalcula solo ante eventos reales
    /// (navegar, reintentar, expulsar, cambio de discos) vía `refresh_missing_cache`.
    missing_cache: std::collections::HashMap<u64, (bool, bool)>,
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
    /// Panel destino (el que estaba bajo el cursor): se activa al confirmar, para que el foco
    /// quede donde el usuario soltó los archivos.
    pub dest_pane: PaneId,
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

/// Mezcla en `h` TODO lo que de un `Entry` determina su `PlainRow` pintada: nombre, ruta (la usan
/// `is_cut`/`is_fresh`), tamaño, modificado, creado, y si es directorio. NO incluye `hidden`/
/// `system` porque la visibilidad ya se hashea aparte y filtra la vista antes de llegar aquí. Sin
/// allocs: hashea por referencia. La usa `rows_signature` (O-1).
fn hash_entry_for_row<H: std::hash::Hasher>(e: &naygo_core::fs_model::Entry, h: &mut H) {
    use std::hash::Hash;
    e.name.hash(h);
    e.path.hash(h);
    // `EntryKind` no deriva Hash en core: su discriminante basta (solo importa Directory vs no).
    std::mem::discriminant(&e.kind).hash(h);
    e.size.hash(h);
    // `SystemTime` implementa Hash; `Option` lo propaga.
    e.modified.hash(h);
    e.created.hash(h);
}

// --- Submódulos por responsabilidad (O-12) ---
// Cada uno aporta su propio `impl WorkspaceCtrl`. Rust permite varios `impl` del
// mismo tipo repartidos en módulos del mismo crate.
mod columns;
mod context;
mod favorites;
mod input;
mod layout_panes;
mod listing;
mod navigation;
mod ops;
mod session;
mod templates;
mod tree;

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
        let mut icons =
            crate::icons::IconCache::new(config.settings.icon_set.clone(), config_dir.clone());
        // Sembrar overrides + tinte al construir, para que la primera pintura ya sea correcta.
        icons.set_overrides(config.settings.icon_overrides.clone());
        {
            let tintable = naygo_core::icon_set::IconSetCatalog::load(&config_dir)
                .is_tintable(&config.settings.icon_set);
            let rgb = crate::theme_text_rgb(&config.settings, &config.themes);
            icons.set_tint(tintable, rgb);
        }
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
            ejected_panes: std::collections::HashSet::new(),
            missing_cache: std::collections::HashMap::new(),
        };
        c.push_recent(start.clone());
        c.start_listing(id, start);
        // Aplicar el modo de operaciones (cola/paralelo) guardado en Settings al motor de ops.
        c.sync_ops_mode();
        c
    }

    /// Aplica `op` al panel activo (helper para no repetir el match de préstamos).
    pub(super) fn with_active(&mut self, op: impl FnOnce(&mut FilePaneState)) {
        if let Some(f) = self.ws.active_files_mut() {
            op(f);
        }
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
mod tests;
#[cfg(test)]
mod tests_simular;
