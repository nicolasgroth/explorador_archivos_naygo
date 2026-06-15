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
use naygo_core::keymap::{Action, KeyMap};
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
    pub keymap: KeyMap,
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
    pub typeahead: String,
    pub ctrl_down: bool,
    pub shift_down: bool,
}

impl WorkspaceCtrl {
    /// Arranca con UN panel Files en `start` (el usuario agrega más con el botón). Lanza
    /// su listado inicial.
    pub fn new(start: std::path::PathBuf) -> WorkspaceCtrl {
        let mut ws = Workspace::new();
        let id = ws.add_pane(PanePurpose::Files, start.clone());
        ws.layout = SerializableDockLayout::single(id);
        ws.set_active(id);
        let mut c = WorkspaceCtrl {
            ws,
            keymap: KeyMap::defaults(),
            listings: HashMap::new(),
            trees: HashMap::new(),
            tree_listings: HashMap::new(),
            favorites: Favorites::new(),
            recents: RecentDirs::new(),
            ops: crate::ops_ctrl::OpsCtrl::new(naygo_core::config::portable_dir()),
            preview: crate::preview::PreviewState::new(),
            pending_pick: None,
            last_area: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            last_click: None,
            typeahead: String::new(),
            ctrl_down: false,
            shift_down: false,
        };
        c.recents.push(start.clone());
        c.start_listing(id, start);
        c
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

    /// Filas a pintar del panel `id`.
    pub fn rows_of(&self, id: PaneId) -> Vec<PlainRow> {
        match self.ws.pane(id).and_then(|p| p.files.as_ref()) {
            Some(f) => rows_from_view(f),
            None => Vec::new(),
        }
    }

    /// Carpeta actual del panel `id` (para su path-bar).
    pub fn path_of(&self, id: PaneId) -> String {
        self.ws
            .pane(id)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.display().to_string())
            .unwrap_or_default()
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
    }

    /// Agrega un panel Files DIVIDIENDO el leaf activo (horizontal). Lo deja activo y
    /// arranca su listado en la misma carpeta que el activo (o el home).
    pub fn add_pane_split(&mut self) {
        let dir = self
            .ws
            .active_files()
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| std::path::PathBuf::from("C:/"));
        let active = self.ws.active_id();
        let new_id = self.ws.add_pane(PanePurpose::Files, dir.clone());
        if let Some(active) = active {
            self.ws
                .layout
                .split_leaf(active, SplitDir::Horizontal, new_id);
        }
        self.ws.set_active(new_id);
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
        // El panel nuevo queda activo, salvo que sea un panel "auxiliar" sin foco propio:
        // mantener el foco en el Files mejora el flujo (el usuario sigue navegando). Pero
        // por simplicidad y consistencia con add_pane_split, lo dejamos activo.
        self.ws.set_active(new_id);
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
    /// arrastre). `None` si el punto no cae sobre un panel o sobre el propio arrastrado.
    pub fn drop_preview(&self, dragged: PaneId, px: f32, py: f32, area: Rect) -> Option<Rect> {
        use naygo_core::workspace::layout::{drop_hit, drop_zones};
        let panes = self.pane_rects(area);
        let (target, zone) = drop_hit(&panes, px, py)?;
        if target == dragged {
            return None;
        }
        let target_rect = panes.iter().find(|(id, _)| *id == target).map(|(_, r)| *r)?;
        drop_zones(target_rect)
            .into_iter()
            .find(|(z, _)| *z == zone)
            .map(|(_, r)| r)
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
        inspector_info(self.ws.active_files())
    }

    /// Filas de favoritos (orden de usuario).
    pub fn favorite_rows(&self) -> Vec<NavRow> {
        favorite_rows(&self.favorites)
    }

    /// Filas de recientes (más nueva primero).
    pub fn recent_rows(&self) -> Vec<NavRow> {
        recent_rows(&self.recents)
    }

    /// Filas del historial de deshacer (validadas contra el disco).
    pub fn history_rows(&self) -> Vec<HistRow> {
        history_rows(&self.ops.undo_history)
    }

    /// Filas del árbol del panel `id` (aplanadas con sangría). Vacío si el panel no tiene
    /// árbol (no es un Tree). El uso de disco se consulta por unidad.
    pub fn tree_rows(&self, id: PaneId) -> Vec<TreeRow> {
        match self.trees.get(&id) {
            Some(t) => tree_rows(t, &disk_percent),
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

    /// El id del panel Files activo (o el primer Files), para dirigir navegaciones desde
    /// paneles auxiliares (un Tree/Favoritos activo no es un Files).
    fn active_files_id(&self) -> Option<PaneId> {
        let active = self.ws.active_id();
        if let Some(a) = active {
            if self.ws.pane(a).map(|p| p.purpose) == Some(PanePurpose::Files) {
                return Some(a);
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
    pub fn op_rename(&mut self) {
        let Some(f) = self.ws.active_files() else {
            return;
        };
        let Some(e) = f.focused_view_entry() else {
            return;
        };
        let dir = f.current_dir.clone();
        self.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::NameInput {
            purpose: crate::ops_ctrl::NamePurpose::Rename(e.path.clone()),
            dir,
            buf: e.name.clone(),
        });
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
        const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);
        let is_double = matches!(
            self.last_click,
            Some((lid, lpos, t)) if lid == id && lpos == pos && now.duration_since(t) <= DOUBLE_CLICK
        );
        if is_double {
            self.last_click = None; // un triple clic no encadena dos navegaciones
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
        let Some(action) = self.keymap.action_for(&chord) else {
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
        let rows = c.rows_of(c.active_id().unwrap());
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
        // Dos clics LENTOS (separados > 400 ms) NO navegan: solo seleccionan.
        assert!(!c.on_row_clicked(id, pos, base));
        assert!(!c.on_row_clicked(id, pos, base + Duration::from_millis(600)));
        assert_eq!(c.path_of(id), tmp.path().display().to_string());

        // Dos clics RÁPIDOS (dentro de 400 ms) SÍ navegan. Siguen la misma línea de tiempo.
        let t1 = base + Duration::from_secs(2);
        assert!(!c.on_row_clicked(id, pos, t1), "1er clic: selecciona");
        assert!(
            c.on_row_clicked(id, pos, t1 + Duration::from_millis(150)),
            "2do clic rápido: doble-clic → navega"
        );
        assert!(drain(&mut c));
        let rows = c.rows_of(c.active_id().unwrap());
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
