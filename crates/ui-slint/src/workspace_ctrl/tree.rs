// Naygo — WorkspaceCtrl: panel árbol de carpetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
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

    /// Resalta `dir` en todos los árboles (cuando cambia la carpeta del Files activo) y arranca
    /// el REVEAL: cada árbol expandirá progresivamente los ancestros hasta `dir`.
    pub(super) fn sync_trees_active(&mut self, dir: PathBuf) {
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
}
