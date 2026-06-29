// Naygo — WorkspaceCtrl: columnas, menú/editor de columna y filtros.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl WorkspaceCtrl {
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
}
