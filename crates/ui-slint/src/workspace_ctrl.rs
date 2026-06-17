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

pub struct WorkspaceCtrl {
    pub ws: Workspace,
    /// Configuración de la app (settings + i18n + temas + atajos), cargada del core y
    /// persistida en el directorio portable. El keymap vive aquí (`config.keymap`).
    pub config: crate::config_ctrl::ConfigCtrl,
    /// Un listado en curso por panel (la carpeta de cada panel se lista por separado).
    pub listings: HashMap<PaneId, Listing>,
    /// Un árbol de carpetas por panel Tree (lazy; se llena con workers solo-dirs).
    pub trees: HashMap<PaneId, DirTree>,
    /// Un worker solo-directorios por rama de árbol en vuelo (clave: panel + carpeta).
    pub tree_listings: HashMap<(PaneId, PathBuf), Listing>,
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
    pub ctrl_down: bool,
    pub shift_down: bool,
    /// Menú contextual abierto (clic derecho): posición (x,y en la ventana) y rutas objetivo.
    pub context_menu: Option<ContextMenuState>,
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
            tree_listings: HashMap::new(),
            favorites: Favorites::new(),
            recents: RecentDirs::new(),
            ops: crate::ops_ctrl::OpsCtrl::new(config_dir),
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
            ctrl_down: false,
            shift_down: false,
            context_menu: None,
            last_saved_fingerprint: None,
            watchers: crate::watch::Watchers::new(),
            last_active_files: Some(id),
            edit_path_requested: None,
            rename_requested: None,
            icons,
        };
        c.recents.push(start.clone());
        c.start_listing(id, start);
        c
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
    pub fn load_session(&mut self) {
        let (persist, _recovered) =
            naygo_core::config::load_workspace_flagged(&self.config.config_dir);
        let Some(persist) = persist else {
            return;
        };
        let Some(restored) = Workspace::from_persist(&persist) else {
            return;
        };
        // Cancelar los listados del arranque por defecto antes de reemplazar el workspace.
        for l in self.listings.values() {
            l.cancel();
        }
        self.listings.clear();
        self.trees.clear();
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
                        self.recents.push(dir.clone());
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
    pub fn relocate_orphans(&mut self, home: &std::path::Path) -> Vec<PaneId> {
        let mut moved = Vec::new();
        let ids: Vec<PaneId> = self.ws.files_panes();
        for id in ids {
            let gone = self
                .ws
                .pane(id)
                .and_then(|p| p.files.as_ref())
                .map(|f| !f.current_dir.exists())
                .unwrap_or(false);
            if gone {
                if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                    f.navigate_to(home.to_path_buf());
                }
                moved.push(id);
            }
        }
        moved
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
        // Préstamos disjuntos: `ops`/`watchers` (lectura) y `icons` (mutable para cachear) son
        // campos distintos; al destructurar `self` el borrow checker los separa.
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
        if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dir.clone());
        }
        self.recents.push(dir.clone());
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
        if sel > 0 {
            format!("{dir}   —   {total} elementos, {sel} seleccionados")
        } else {
            format!("{dir}   —   {total} elementos")
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
        }
    }

    /// Agrega un panel Files dividiendo el leaf activo lado a lado (horizontal, el nuevo a la
    /// derecha). Atajo de la dirección por defecto.
    pub fn add_pane_split(&mut self) {
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
        if let Some(active) = active {
            self.ws
                .layout
                .split_leaf(active, SplitDir::Horizontal, new_id);
        }
        if matches!(purpose, PanePurpose::Tree) {
            self.trees.insert(new_id, build_tree());
            // Resalta de entrada la carpeta del panel Files activo, si la hay.
            if let Some(cur) = self.ws.active_files().map(|f| f.current_dir.clone()) {
                if let Some(t) = self.trees.get_mut(&new_id) {
                    t.set_active(cur);
                }
            }
        }
        // El panel nuevo queda activo. Vía self.set_active: si es auxiliar (Árbol…),
        // `last_active_files` NO cambia (sigue apuntando al Files que el usuario venía usando),
        // que es justo lo que queremos para que el árbol navegue ese panel.
        self.set_active(new_id);
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
    pub fn open_in_pane(&mut self, dest: PaneId, dir: PathBuf) {
        if let Some(f) = self.ws.pane_mut(dest).and_then(|p| p.files.as_mut()) {
            f.navigate_to(dir.clone());
        }
        self.recents.push(dir.clone());
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
        self.ws
            .layout
            .split_leaf(origin, SplitDir::Horizontal, new_id);
        self.start_listing(new_id, dir);
        // El foco se queda en el origen (estás explorando desde ahí).
        self.ws.set_active(origin);
        Some(new_id)
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
        }
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
                }
            })
            .collect()
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
            Some(t) => tree_rows(t, &disk_percent, &mut |key| icons.get(key)),
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
        if let Some(f) = self
            .ws
            .pane_mut(active_files_id)
            .and_then(|p| p.files.as_mut())
        {
            f.navigate_to(dir.clone());
        }
        self.recents.push(dir.clone());
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
    pub fn op_paste(&mut self) -> bool {
        let Some(dir) = self.active_dir() else {
            return false;
        };
        let content = naygo_platform::clipboard::read();
        if let naygo_core::clipboard::ClipboardContent::Files { paths, cut } = content {
            if paths.is_empty() {
                return false;
            }
            let label = if cut { "Mover" } else { "Copiar" };
            let req = naygo_core::ops::transfer(cut, paths, dir);
            self.ops.start_op(req, label.to_string(), true);
            self.ops.clear_cut();
            return true;
        }
        false
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
        self.context_menu = Some(ContextMenuState { x, y, targets });
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

    /// Resalta `dir` en todos los árboles (cuando cambia la carpeta del Files activo).
    fn sync_trees_active(&mut self, dir: PathBuf) {
        for t in self.trees.values_mut() {
            t.set_active(dir.clone());
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
        self.tree_listings.is_empty()
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
    pub fn on_row_double_clicked(&mut self, id: PaneId, pos: usize) -> bool {
        self.ws.set_active(id);
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
            if let Some(f) = self.ws.pane_mut(id).and_then(|p| p.files.as_mut()) {
                f.navigate_to(e.path.clone());
            }
            self.recents.push(e.path.clone());
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
                self.recents.push(dir.clone());
                self.start_listing(active, dir.clone());
                self.sync_trees_active(dir);
                true
            }
            None => false,
        }
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
            Action::Undo => return self.op_undo_last(),
            // Editar la ruta del panel activo (Ctrl+L / F4): la UI abre el editor de la path-bar.
            Action::EditPath => {
                self.edit_path_requested = self.active_files_id();
            }
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

/// Porcentaje de uso de disco de una unidad (0..100), o `None` si no se puede leer.
fn disk_percent(root: &Path) -> Option<u8> {
    let (total, free) = naygo_platform::drive_space::read_space(root)?;
    let usage = naygo_core::disk::DiskUsage { total, free };
    Some(usage.percent_used())
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

    /// F5B: si un panel quedó parado en una carpeta que desapareció (p. ej. se sacó el USB),
    /// relocate_orphans lo reubica a HOME y lo reporta como reubicado.
    #[test]
    fn relocate_reubica_panel_huerfano() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("usb");
        std::fs::create_dir(&sub).unwrap();
        let mut c = WorkspaceCtrl::new_in(sub.clone(), tmp.path().to_path_buf());
        assert!(drain(&mut c));
        std::fs::remove_dir_all(&sub).unwrap(); // "sacar el USB"
        let moved = c.relocate_orphans(tmp.path());
        assert_eq!(moved.len(), 1);
        assert_eq!(c.ws.active_files().unwrap().current_dir, tmp.path());
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
}
