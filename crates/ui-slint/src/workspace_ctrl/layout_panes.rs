// Naygo — WorkspaceCtrl: geometría, paneles, pestañas, acciones multi-panel y discos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
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
            // Rótulos traducidos (antes eran literales en español → la pestaña del panel salía en
            // español aunque la app estuviera en otro idioma). Las claves ya existen en los 10 idiomas.
            PanePurpose::Tree => self.config.t("pane.tree.title"),
            PanePurpose::Inspector => self.config.t("pane.inspector.title"),
            PanePurpose::History => self.config.t("pane.history.title"),
            PanePurpose::Favorites => self.config.t("pane.favorites.title"),
            PanePurpose::Preview => self.config.t("pane.preview.title"),
            PanePurpose::Operations => self.config.t("ops.menu_label"),
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
        // Purgar los listados de subcarpetas del árbol de este panel, cancelando sus workers
        // (si no, quedaban dirs-only sin cancelar al cerrar el panel).
        self.tree_listings.retain(|(pane, _), l| {
            if *pane == id {
                l.cancel();
                false
            } else {
                true
            }
        });
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
        // Cancelar el listado en vuelo antes de soltarlo (no dejar workers huérfanos),
        // igual que en `close_pane`.
        if let Some(l) = self.listings.remove(&member) {
            l.cancel();
        }
        self.trees.remove(&member);
        // Purgar los listados de subcarpetas del árbol de este panel, cancelando sus workers.
        self.tree_listings.retain(|(pane, _), l| {
            if *pane == member {
                l.cancel();
                false
            } else {
                true
            }
        });
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

    /// Suelta el handle que la app mantiene sobre la carpeta del panel `id`: cierra su watcher.
    /// Se llama justo ANTES de expulsar el disco, para que el "en uso" no lo cause la propia app.
    /// El aviso in-place "elegir carpeta" aparece solo tras expulsar (pane_dir_missing detecta el
    /// read_dir fallido). No-op si el panel no tiene watcher.
    pub fn release_pane_watcher(&mut self, id: PaneId) {
        self.watchers.unwatch(id.0);
    }

    /// ¿Hay algún panel de un `purpose` dado en el workspace?
    pub fn has_purpose(&self, purpose: PanePurpose) -> bool {
        self.ws.panes().iter().any(|p| p.purpose == purpose)
    }

    /// Los paneles Files cuya carpeta actual está en el disco `drive_root` (el que se va a
    /// expulsar). Devuelve `(PaneId, carpeta_actual)`. Puro sobre el estado del workspace.
    pub fn panes_on_drive(&self, drive_root: &std::path::Path) -> Vec<(PaneId, std::path::PathBuf)> {
        self.ws
            .panes()
            .iter()
            .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.current_dir.clone())))
            .filter(|(_, dir)| path_is_on_drive(dir, drive_root))
            .collect()
    }
}
