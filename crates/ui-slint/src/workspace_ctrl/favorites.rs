// Naygo — WorkspaceCtrl: favoritos, recientes e historial.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
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

    /// Filas del historial de deshacer (validadas contra el disco).
    pub fn history_rows(&self) -> Vec<HistRow> {
        history_rows(&self.ops.undo_history)
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
}
