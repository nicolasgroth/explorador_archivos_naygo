// Naygo — WorkspaceCtrl: geometría, paneles, pestañas, acciones multi-panel y discos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
    /// Rects de los paneles (id, rect) dado el área de contenido.
    pub fn pane_rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        if let Some(id) = self.maximized_pane.filter(|id| self.ws.pane(*id).is_some()) {
            return vec![(id, area)];
        }
        self.ws.layout.pane_rects(area)
    }

    pub fn toggle_maximize(&mut self, id: PaneId) -> bool {
        if self.ws.pane(id).is_none() {
            return false;
        }
        self.maximized_pane = if self.maximized_pane == Some(id) {
            None
        } else {
            Some(id)
        };
        self.set_active(id);
        true
    }

    /// Recuerda el área de contenido actual (la UI la setea en cada layout) para resolver
    /// destinos por orden visual desde gestos sin área (teclado).
    pub fn set_area(&mut self, area: Rect) {
        self.last_area = area;
    }

    /// Panel que acepta archivos bajo el punto `(content_x, content_y)` (coords de contenido, el mismo
    /// sistema que usan `pane_rects`/`drop_hit`/`drop_at`). Reusa el hit-testing del docking. Solo
    /// devuelve paneles Files o Bandeja: si cae sobre otro auxiliar (Árbol/Inspector/Preview/…)
    /// o fuera de todo panel, devuelve `None`. Lo usa la UI para resaltar EN VIVO el panel bajo el
    /// cursor mientras se arrastran archivos (mismo destino que recibiría `drop_at`). No ejecuta
    /// nada ni muta estado: es un puro hit-test.
    pub fn pane_at(&self, content_x: f32, content_y: f32) -> Option<PaneId> {
        use naygo_core::workspace::layout::drop_hit;
        let panes = self.pane_rects(self.last_area);
        let (target, _zone) = drop_hit(&panes, content_x, content_y)?;
        // Filtrar a los dos destinos válidos: Files transfiere y Basket guarda referencias.
        self.ws.pane(target).and_then(|pane| {
            matches!(pane.purpose, PanePurpose::Files | PanePurpose::Basket).then_some(target)
        })
    }

    /// Fija el panel resaltado por arrastre (hover de drop). Conserva esta API pequeña para los
    /// callers que no traen coordenadas (y para las pruebas); el camino OLE usa
    /// `set_drag_over_at` para además informar la fila destino a `FilePanel`.
    pub fn set_drag_over(&mut self, pane: Option<PaneId>) -> bool {
        self.set_drag_over_at(pane, None)
    }

    /// Actualiza el hover OLE y su Y de cliente lógica. Al cambiar de panel o salir, invalida la
    /// fila previamente detectada: una fila de otro panel nunca puede convertirse en destino.
    /// Devuelve `true` cuando Slint debe refrescar los `PaneVm`.
    pub fn set_drag_over_at(&mut self, pane: Option<PaneId>, client_y: Option<f32>) -> bool {
        let changed = self.drag_over_pane != pane || self.drag_over_client_y != client_y;
        if self.drag_over_pane != pane {
            self.drag_over_row = None;
        }
        if pane.is_none() {
            self.drag_over_row = None;
        }
        self.drag_over_pane = pane;
        self.drag_over_client_y = client_y;
        changed
    }

    /// El panel actualmente resaltado por arrastre, si lo hay. Lo lee `sync_rows` para poblar el
    /// `drag-over` de cada `PaneVm`.
    pub fn drag_over_pane(&self) -> Option<PaneId> {
        self.drag_over_pane
    }

    /// Y de cliente que consume el `FilePanel` del panel indicado. Un valor muy negativo es una
    /// coordenada fuera de la ventana y evita que un panel no-hover reporte una fila accidental.
    pub fn drag_over_client_y_for(&self, pane: PaneId) -> f32 {
        if self.drag_over_pane == Some(pane) {
            self.drag_over_client_y.unwrap_or(-10_000.0)
        } else {
            -10_000.0
        }
    }

    /// Recibe la fila de vista detectada por `FilePanel` mientras un `IDropTarget::DragOver` está
    /// activo. La validación final de que siga siendo una carpeta se hace al soltar, contra el
    /// modelo actual (un watcher puede haber refrescado la carpeta entre hover y drop).
    pub fn set_drag_over_row(&mut self, pane: PaneId, row: i32) {
        self.drag_over_row =
            (self.drag_over_pane == Some(pane) && row >= 0).then_some((pane, row as usize));
    }

    /// Handles de splitter (para pintarlos y arrastrarlos).
    pub fn split_handles(&self, area: Rect) -> Vec<SplitHandle> {
        if self.maximized_pane.is_some() {
            return Vec::new();
        }
        self.ws.layout.split_handles(area)
    }

    /// Mueve un divisor de un split (drag de splitter): transfiere peso solo entre sus vecinos.
    pub fn set_divider(&mut self, path: &[SplitStep], divider: usize, frac_local: f32) {
        self.ws.layout.set_divider(path, divider, frac_local);
    }

    /// Fracción local + rect de la barra-fantasma para un divisor dado el puntero (vista previa).
    pub fn divider_at(
        &self,
        path: &[SplitStep],
        divider: usize,
        area: Rect,
        px: f32,
        py: f32,
    ) -> Option<(f32, Rect)> {
        self.ws.layout.divider_at(path, divider, area, px, py)
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
        self.sync_trees_for_files(id, dir);
        true
    }

    pub fn active_id(&self) -> Option<PaneId> {
        self.ws.active_id()
    }

    /// El propósito (tipo) del panel `id`, si existe.
    pub fn purpose_of(&self, id: PaneId) -> Option<PanePurpose> {
        self.ws.pane(id).map(|p| p.purpose)
    }

    pub fn is_link_member(&self, id: PaneId) -> bool {
        self.ws.is_link_member(id)
    }

    pub fn tree_is_linked(&self, id: PaneId) -> bool {
        self.ws.linked_files(id).is_some()
    }

    /// Título de la ventana principal según `Settings.window_title_mode`: solo la app,
    /// la app con la ruta del panel activo, o solo la ruta. Sin panel Files activo
    /// (p. ej. solo árbol/ayuda), cae a "Naygo" para no quedar vacío.
    pub fn window_title(&self) -> String {
        let path = self
            .ws
            .active_files()
            .map(|f| f.current_dir.display().to_string());
        match self.config.settings.window_title_mode {
            naygo_core::WindowTitleMode::AppOnly => "Naygo".to_string(),
            naygo_core::WindowTitleMode::AppAndPath => match path {
                Some(p) => format!("Naygo — {p}"),
                None => "Naygo".to_string(),
            },
            naygo_core::WindowTitleMode::PathOnly => path.unwrap_or_else(|| "Naygo".to_string()),
        }
    }

    /// Texto de la barra de estado: carpeta activa + recuento de ítems y de selección.
    pub fn status_line(&self) -> String {
        let Some(f) = self.ws.active_files() else {
            return String::new();
        };
        let total = f.view_indices().len();
        let sel = f.selected.len();
        let dir = f.current_dir.display();
        let elementos = self
            .config
            .t("status.elements")
            .replace("{n}", &total.to_string());
        let base = if sel > 0 {
            let sel_suffix = self.config.t("status.selected_suffix");
            format!("{dir}   —   {elementos}, {sel} {sel_suffix}")
        } else {
            format!("{dir}   —   {elementos}")
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
            PanePurpose::Basket => self.config.t("basket.title"),
            PanePurpose::Search => self.config.t("pane.search.title"),
            PanePurpose::Recents => self.config.t("slint.fav.recents"),
        }
    }

    pub fn set_active(&mut self, id: PaneId) {
        if self.maximized_pane.is_some_and(|solo| solo != id) {
            self.maximized_pane = None;
        }
        // El filtro de tipeo pertenece al panel con foco. Si se estaba ocultando filas, restaurar
        // el panel que se deja antes de cambiar y aplicar el mismo buffer al nuevo panel.
        if self.filter_hide_nonmatches && self.ws.active_id() != Some(id) {
            if let Some(files) = self.ws.active_files_mut() {
                files.set_visual_filter(None);
            }
        }
        self.ws.set_active(id);
        self.sync_visual_filter();
        if !matches!(
            self.ws.pane(id).map(|p| p.purpose),
            Some(PanePurpose::Search | PanePurpose::Preview | PanePurpose::Inspector)
        ) {
            self.search_context = false;
        } else if self.ws.pane(id).map(|p| p.purpose) == Some(PanePurpose::Search) {
            self.search_context = true;
        }
        // La selección de bandeja debe sobrevivir al consultar Preview/Propiedades, pero no
        // competir con la selección normal cuando el usuario vuelve a un panel Files u otro
        // panel de navegación.
        if !matches!(
            self.ws.pane(id).map(|pane| pane.purpose),
            Some(PanePurpose::Basket | PanePurpose::Preview | PanePurpose::Inspector)
        ) {
            self.basket_context = false;
        } else if self.ws.pane(id).map(|pane| pane.purpose) == Some(PanePurpose::Basket) {
            self.basket_context = true;
        }
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
                self.sync_trees_for_files(id, dir);
            }
        }
    }

    /// Agrega un panel Files dividiendo el leaf activo por su LADO MÁS LARGO (columnas si es
    /// ancho, filas si es alto), usando `area` (el área de contenido actual) para medir el rect
    /// del panel activo. El nuevo panel queda después (derecha/abajo).
    pub fn add_pane_split(&mut self, area: Rect) {
        crate::logging::breadcrumb("abrir panel (split)");
        let dir = self
            .ws
            .active_id()
            .and_then(|id| {
                self.pane_rects(area)
                    .into_iter()
                    .find(|(pid, _)| *pid == id)
                    .map(|(_, r)| r)
            })
            .map(naygo_core::workspace::layout::pick_split_dir)
            .unwrap_or(SplitDir::Horizontal);
        self.add_pane_split_dir(dir, false);
    }

    /// Abre `dir` SIEMPRE en un panel NUEVO (divide el leaf activo por su lado más largo, vía
    /// `add_pane_split`) y navega ese panel nuevo a `dir`. Lo usan el clic-medio sobre una fila
    /// (siempre split, a diferencia de Shift+Enter/Ctrl+doble-clic que reusan otro panel si ya
    /// hay uno) y `ctx_open_new_pane` del menú contextual.
    pub fn open_dir_in_new_pane(&mut self, dir: PathBuf, area: Rect) -> bool {
        self.add_pane_split(area);
        let Some(dest) = self.active_id() else {
            return false;
        };
        self.open_in_pane(dest, dir);
        true
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
    pub fn add_pane_of(&mut self, purpose: PanePurpose, area: Rect) {
        crate::logging::breadcrumb(&format!("abrir panel {:?}", purpose));
        if matches!(purpose, PanePurpose::Search) {
            self.open_search_pane(area);
            return;
        }
        if matches!(purpose, PanePurpose::Files) {
            self.add_pane_split(area);
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

    /// Abre (o enfoca) el buscador como un panel dockable. Conserva un único panel de búsqueda:
    /// así F3 no llena el workspace de resultados duplicados y la búsqueda en curso sigue siendo
    /// cancelable desde su propio panel.
    pub fn open_search_pane(&mut self, _area: Rect) {
        if let Some(existing) = self
            .ws
            .panes()
            .iter()
            .find(|p| p.purpose == PanePurpose::Search)
            .map(|p| p.id)
        {
            self.set_active(existing);
            if !self.search_open() {
                self.open_empty_search();
            }
            return;
        }

        // Sembrar primero el job vacío mientras aún está activo el Files de origen; luego el
        // panel Search puede ganar foco sin que la raíz caiga en otro Files arbitrario.
        self.open_empty_search();
        let anchor = self.ws.active_id();
        let root = self
            .last_active_files
            .and_then(|id| self.ws.pane(id))
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| PathBuf::from("C:/"));
        let id = self.ws.add_pane(PanePurpose::Search, root);
        if let Some(anchor) = anchor {
            self.ws.layout.split_leaf(anchor, SplitDir::Horizontal, id);
        }
        self.set_active(id);
    }

    /// Crea un explorador enlazado como unidad: árbol angosto a la izquierda y Files a la
    /// derecha. El split interno se mantiene anidado para que la pareja se perciba y se mueva
    /// como un bloque visual incluso junto a otros paneles horizontales.
    pub fn add_linked_browser(&mut self) {
        crate::logging::breadcrumb("abrir explorador enlazado");
        let dir = self
            .last_active_files
            .and_then(|id| self.ws.pane(id))
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
            .unwrap_or_else(|| PathBuf::from("C:/"));
        let anchor = self.ws.active_id();
        let files = self.ws.add_pane(PanePurpose::Files, dir.clone());
        self.apply_default_table(files);
        if let Some(anchor) = anchor {
            self.ws
                .layout
                .split_leaf(anchor, SplitDir::Horizontal, files);
        } else {
            self.ws.layout = naygo_core::workspace::SerializableDockLayout::single(files);
        }
        let tree = self.ws.add_pane(PanePurpose::Tree, PathBuf::new());
        self.ws
            .layout
            .split_leaf_grouped(files, SplitDir::Horizontal, tree, 0.72);
        self.ws.layout.swap_split_children(files, tree);
        self.ws.link_tree(tree, files);

        let mut model = build_tree();
        model.set_active(dir.clone());
        self.trees.insert(tree, model);
        self.reveal_targets.insert(tree, dir.clone());
        self.set_active(files);
        self.start_listing(files, dir);
        self.pump_reveal();
    }

    /// Asegura que exista un panel de Operaciones en el layout; si ya hay uno, no-op. Se llama
    /// al iniciar una operación larga para que el panel rico de progreso "aparezca solo" sin que
    /// el usuario tenga que abrirlo. A diferencia de `add_pane_of`, NO roba el foco: el panel
    /// Files activo sigue activo (el usuario estaba operando ahí). El usuario puede cerrarlo.
    ///
    /// El split se decide por el lado más largo DEL panel donde nació la operación, igual que al
    /// abrir un panel Files. Antes se forzaba Horizontal: en layouts verticales partía el panel de
    /// trabajo en una dirección inesperada y el progreso aparecía encima de lo que se estaba usando.
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
        let split = active
            .and_then(|id| {
                self.pane_rects(self.last_area)
                    .into_iter()
                    .find(|(pane, _)| *pane == id)
                    .map(|(_, rect)| naygo_core::workspace::layout::pick_split_dir(rect))
            })
            .unwrap_or(SplitDir::Horizontal);
        let new_id = self.ws.add_pane(PanePurpose::Operations, dir);
        if let Some(active) = active {
            self.ws.layout.split_leaf(active, split, new_id);
        }
        // Restaurar el activo previo (el panel de Operaciones no toma el foco).
        if let Some(prev) = prev_active {
            self.set_active(prev);
        }
    }

    /// Los paneles que se cierran junto con `id`. Un explorador enlazado Árbol+Files es una
    /// unidad visual: cerrar cualquiera de sus miembros cierra ambos, sin dejar un árbol o
    /// listado dedicado huérfano.
    fn close_targets(&self, id: PaneId) -> Vec<PaneId> {
        if self.ws.pane(id).is_none() {
            return Vec::new();
        }
        match self.ws.linked_partner(id) {
            Some(partner) if self.ws.pane(partner).is_some() => vec![id, partner],
            _ => vec![id],
        }
    }

    /// Cancela y suelta los recursos que pertenecen a un panel antes de retirarlo del
    /// workspace. Se comparte entre el cierre normal y el de una pestaña para que el cierre
    /// de una pareja enlazada no deje workers ni estado visual huérfanos.
    fn release_pane_resources(&mut self, id: PaneId) {
        if self.ws.pane(id).map(|p| p.purpose) == Some(PanePurpose::Search) {
            self.close_search();
        }
        if let Some(l) = self.listings.remove(&id) {
            l.cancel();
        }
        self.trees.remove(&id);
        self.reveal_targets.remove(&id);
        // Purgar los listados de subcarpetas del árbol de este panel, cancelando sus workers.
        self.tree_listings.retain(|(pane, _), l| {
            if *pane == id {
                l.cancel();
                false
            } else {
                true
            }
        });
    }

    /// `true` si el panel `id` se puede cerrar. Los miembros de un explorador enlazado cuentan
    /// como una pareja indivisible y nunca dejamos la ventana sin ningún panel.
    pub fn can_close_pane(&self, id: PaneId) -> bool {
        let targets = self.close_targets(id);
        !targets.is_empty() && self.ws.panes().len() > targets.len()
    }

    /// Cierra el panel `id` o, si pertenece a un explorador enlazado, todo su bloque Árbol+Files.
    /// Cancela sus listados en vuelo, los saca del layout y del workspace, y reasigna el activo.
    /// No-op si el cierre dejaría la ventana sin paneles. Tras cerrar, re-sincroniza el árbol con
    /// la carpeta del nuevo panel activo.
    pub fn close_pane(&mut self, id: PaneId) {
        self.maximized_pane = None;
        let targets = self.close_targets(id);
        if targets.is_empty() || self.ws.panes().len() <= targets.len() {
            return;
        }
        crate::logging::breadcrumb(&format!(
            "cerrar {}",
            targets
                .iter()
                .map(|pane| format!("panel {}", pane.0))
                .collect::<Vec<_>>()
                .join(" + ")
        ));
        // Primero soltar/cancelar todos los recursos; después mutar layout/workspace. Mantener
        // la pareja completa hasta este punto permite calcular los dos objetivos aun cuando el
        // primer `remove_pane` elimina el enlace Tree→Files.
        for target in &targets {
            self.release_pane_resources(*target);
        }
        for target in &targets {
            self.ws.layout.remove_leaf(*target);
        }
        for target in &targets {
            self.ws.remove_pane(*target);
        }
        // Si el último Files activo era parte del bloque cerrado, recomputar.
        if self
            .last_active_files
            .is_some_and(|last| targets.contains(&last))
        {
            self.last_active_files = self.ws.files_panes().first().copied();
        }
        // Re-resaltar el árbol hacia la carpeta del panel activo resultante.
        if let Some(dir) = self.ws.active_files().map(|f| f.current_dir.clone()) {
            if let Some(files) = self.last_active_files {
                self.sync_trees_for_files(files, dir);
            }
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
    #[cfg(test)]
    pub fn split_for_target(&mut self) -> Option<PaneId> {
        let origin = self.active_files_id()?;
        self.split_for_target_from(origin)
    }

    /// Variante explícita para acciones disparadas desde un panel auxiliar (como Search): el
    /// split se ancla al explorador de origen, nunca al panel auxiliar que tiene el foco.
    fn split_for_target_from(&mut self, origin: PaneId) -> Option<PaneId> {
        let dir = self
            .ws
            .pane(origin)
            .filter(|pane| pane.purpose == PanePurpose::Files)
            .and_then(|pane| pane.files.as_ref())
            .map(|files| files.current_dir.clone())?;
        let new_id = self.ws.add_pane(PanePurpose::Files, dir.clone());
        self.apply_default_table(new_id);
        self.ws
            .layout
            .split_leaf(origin, SplitDir::Horizontal, new_id);
        self.start_listing(new_id, dir);
        // El foco se queda en el origen (estás explorando desde ahí).
        self.set_active(origin);
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
                if let Some(dest) = self.split_for_target_from(origin) {
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

    fn transfer_to_dir(&mut self, sources: Vec<PathBuf>, dest_dir: PathBuf, move_files: bool) {
        if sources.is_empty() {
            return;
        }
        let req = naygo_core::ops::transfer(move_files, sources, dest_dir);
        let label = if move_files {
            self.config.t("ops.file_kind_move")
        } else {
            self.config.t("ops.file_kind_copy")
        };
        self.ensure_ops_pane();
        self.ops.start_op(req, label, true);
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
        let origin_dir = self
            .ws
            .pane(origin)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone());
        if self.selected_paths().is_empty() {
            return false;
        }
        use naygo_core::destination_radar::{
            rank, DestinationCandidate as C, DestinationSource as S,
        };
        let open = self
            .pane_rects(self.last_area)
            .into_iter()
            .filter_map(|(id, _)| {
                if id == origin {
                    return None;
                }
                let f = self.ws.pane(id)?.files.as_ref()?;
                Some(C {
                    path: f.current_dir.clone(),
                    label: self.pane_label(id),
                    source: S::OpenPanel,
                })
            })
            .collect();
        let last = self
            .ops
            .last_transfer_destinations(10)
            .into_iter()
            .map(|path| C {
                label: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| path.to_string_lossy().to_string()),
                path,
                source: S::LastOperation,
            })
            .collect();
        let favorites = self
            .favorites
            .list_flat()
            .into_iter()
            .map(|f| C {
                path: f.path,
                label: f.label,
                source: S::Favorite,
            })
            .collect();
        let frequent = self
            .recents
            .most_used(self.config.settings.frequent_dirs_limit)
            .into_iter()
            .map(|f| C {
                label: f
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| f.path.to_string_lossy().to_string()),
                path: f.path,
                source: S::Frequent,
            })
            .collect();
        let recent = self
            .recents
            .list()
            .iter()
            .cloned()
            .map(|path| C {
                label: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| path.to_string_lossy().to_string()),
                path,
                source: S::Recent,
            })
            .collect();
        let candidates: Vec<C> = rank([open, last, favorites, frequent, recent], 99)
            .into_iter()
            .filter(|c| Some(&c.path) != origin_dir.as_ref())
            .collect();
        if candidates.is_empty() {
            return false;
        }
        self.destination_radar = Some(DestinationRadar {
            move_files,
            sources: self.selected_paths(),
            candidates,
            origin: DestinationRadarOrigin::FilesSelection,
        });
        false
    }

    pub fn destination_radar_resolve(&mut self, index: usize) {
        let Some(radar) = self.destination_radar.take() else {
            return;
        };
        let Some(path) = radar
            .candidates
            .get(index)
            .map(|candidate| candidate.path.clone())
        else {
            return;
        };
        self.destination_radar_transfer(radar, path);
    }

    /// Resuelve el radar con una carpeta elegida manualmente desde «Otra carpeta…».
    pub fn destination_radar_resolve_path(&mut self, path: PathBuf) {
        let Some(radar) = self.destination_radar.take() else {
            return;
        };
        self.destination_radar_transfer(radar, path);
    }

    pub fn destination_radar_cancel(&mut self) {
        self.destination_radar = None;
    }

    fn destination_radar_transfer(&mut self, radar: DestinationRadar, path: PathBuf) {
        match radar.origin {
            DestinationRadarOrigin::FilesSelection => {
                self.transfer_to_dir(radar.sources, path, radar.move_files);
            }
            DestinationRadarOrigin::Basket => {
                self.basket_transfer_paths(radar.sources, path, radar.move_files);
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
        self.set_active(origin);
    }

    /// Cambia la pestaña activa de un grupo al miembro `member` y lo deja activo.
    pub fn set_active_tab(&mut self, member: PaneId) {
        self.ws.layout.set_active_tab(member);
        self.set_active(member);
    }

    /// Cierra la pestaña `member`. La ruta comparte el cierre normal para respetar la regla de
    /// que un explorador enlazado Árbol+Files se elimina como bloque. Si era la única del grupo,
    /// este desaparece y su rect lo absorbe el hermano del split.
    pub fn close_tab(&mut self, member: PaneId) {
        self.close_pane(member);
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
                // `split_leaf` deja el arrastrado JUSTO DESPUÉS del destino (sea insertándolo
                // como hermano en la misma fila/columna del mismo eje, o creando un sub-split).
                // En ambos casos el par destino/arrastrado queda con pesos iguales (simétrico),
                // así que para Left/Top —donde el arrastrado debe ir PRIMERO— basta intercambiar
                // sus posiciones (swap intercambia hijos y pesos a la par).
                if matches!(zone, DropZone::Left | DropZone::Top) {
                    self.ws.layout.swap_split_children(target, dragged);
                }
                self.set_active(dragged);
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
    pub fn panes_on_drive(
        &self,
        drive_root: &std::path::Path,
    ) -> Vec<(PaneId, std::path::PathBuf)> {
        self.ws
            .panes()
            .iter()
            .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.current_dir.clone())))
            .filter(|(_, dir)| path_is_on_drive(dir, drive_root))
            .collect()
    }
}
