// Naygo — estado de un panel de archivos (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `FilePaneState` es el estado de un panel de archivos: dónde está parado, qué
//! lista, su historial de navegación, su filtro de carpetas. No toca disco: la UI
//! le inyecta las entradas (vía el motor de `listing`) y le pide navegar.

use crate::columns::{ColumnKind, TableState};
use crate::filter::matches as filter_matches;
use crate::filter::ColumnFilter;
use crate::fs_model::{Entry, SortSpec, ViewMode};
use crate::workspace::nav_history::NavHistory;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Estado de un panel de archivos. Lo serializable se persiste; `entries` no
/// (se re-lista al abrir) y `history` tampoco (arranca limpio cada sesión).
#[derive(Debug)]
pub struct FilePaneState {
    pub current_dir: PathBuf,
    pub entries: Vec<Entry>,
    pub sort: SortSpec,
    pub view: ViewMode,
    pub focused: Option<usize>,
    pub selected: Vec<usize>,
    /// Ancla de la selección por rango (Shift). Efímero, NO se persiste.
    pub anchor: Option<usize>,
    pub history: NavHistory,
    /// Si es `false`, el panel oculta las carpetas (muestra solo archivos).
    pub show_dirs: bool,
    /// Estado de tabla: columnas (orden/visibilidad/ancho) + filtros por columna.
    pub table: TableState,
    /// Rutas resaltadas como "recién aparecidas" (estado de presentación efímero; NO se
    /// persiste). El render las tiñe; la interacción/refresh las limpia.
    pub highlighted: std::collections::HashSet<std::path::PathBuf>,
    /// Espejo runtime del setting "archivos nuevos al final" (lo setea la UI). NO se
    /// persiste (vive en settings.json); existe para que `view_indices` agrupe IGUAL
    /// que el render y los índices nunca se desalineen.
    pub group_new_at_end: bool,
    /// Caché de los índices de vista (filtrados+ordenados). Recompute PEREZOSO bajo
    /// `&self` vía `RefCell`; se invalida comparando una firma O(1) de los inputs. NO se
    /// clona (cada panel reconstruye el suyo) ni se persiste. Efímero de presentación.
    view_cache: std::cell::RefCell<Option<ViewCache>>,
    /// Contador de recomputes de la vista (solo para tests; mide aciertos del caché).
    #[cfg(test)]
    view_recomputes: std::cell::Cell<u32>,
}

/// Caché de la vista: la firma de los inputs con que se calculó + los índices resultantes.
#[derive(Debug)]
struct ViewCache {
    signature: u64,
    indices: Vec<usize>,
}

impl Clone for FilePaneState {
    fn clone(&self) -> Self {
        FilePaneState {
            current_dir: self.current_dir.clone(),
            entries: self.entries.clone(),
            sort: self.sort,
            view: self.view,
            focused: self.focused,
            selected: self.selected.clone(),
            anchor: self.anchor,
            history: self.history.clone(),
            show_dirs: self.show_dirs,
            table: self.table.clone(),
            highlighted: self.highlighted.clone(),
            group_new_at_end: self.group_new_at_end,
            // El caché NO se arrastra: el clon lo reconstruye a demanda.
            view_cache: std::cell::RefCell::new(None),
            #[cfg(test)]
            view_recomputes: std::cell::Cell::new(0),
        }
    }
}

/// Lo que se persiste de un panel de archivos (sin entries ni history).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilePanePersist {
    pub current_dir: PathBuf,
    pub sort: SortSpec,
    pub view: ViewMode,
    pub show_dirs: bool,
    /// DEPRECADO: filtro de texto plano (fases previas). Solo se LEE para migrar a
    /// `table`. Ya no se escribe.
    #[serde(default)]
    pub text_filter: Option<String>,
    /// Estado de tabla. `None` en persists viejos → se migra desde `text_filter`.
    #[serde(default)]
    pub table: Option<TableState>,
}

impl FilePaneState {
    /// Crea un panel parado en `dir`, con su historial ya apuntando a `dir`.
    pub fn new(dir: PathBuf) -> Self {
        let mut history = NavHistory::new();
        history.push(dir.clone());
        FilePaneState {
            current_dir: dir,
            entries: Vec::new(),
            sort: SortSpec::default(),
            view: ViewMode::default(),
            focused: None,
            selected: Vec::new(),
            anchor: None,
            history,
            show_dirs: true,
            table: TableState::default(),
            highlighted: std::collections::HashSet::new(),
            group_new_at_end: false,
            view_cache: std::cell::RefCell::new(None),
            #[cfg(test)]
            view_recomputes: std::cell::Cell::new(0),
        }
    }

    /// ¿Está esta ruta resaltada como nueva?
    pub fn is_highlighted(&self, path: &Path) -> bool {
        self.highlighted.contains(path)
    }

    /// Limpia todo el resaltado (al interactuar o re-listar).
    pub fn clear_highlight(&mut self) {
        self.highlighted.clear();
    }

    /// Firma O(1) de los inputs que determinan la vista. Si no cambia entre llamadas, el
    /// caché es válido. Captura: nº de entries, sort, filtros (su hash), agrupar-al-final,
    /// y nº de resaltadas (solo afecta el orden si `group_new_at_end`). No itera entries.
    fn view_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.entries.len().hash(&mut h);
        // SortSpec deriva Hash: cubre key, ascending y dirs_first.
        self.sort.hash(&mut h);
        // Filtros: el BTreeMap es ordenado, así que el recorrido (y el hash) es estable.
        for (k, v) in &self.table.filters {
            k.hash(&mut h);
            v.hash(&mut h);
        }
        self.group_new_at_end.hash(&mut h);
        // El conjunto de resaltadas solo cambia el orden si se agrupan al final.
        if self.group_new_at_end {
            self.highlighted.len().hash(&mut h);
        }
        h.finish()
    }

    /// Las entries VISIBLES (índices en `entries`): filtradas y ordenadas. CACHEADO: si la
    /// firma de los inputs no cambió desde el último cálculo, devuelve el caché (clon del
    /// Vec, barato). Antes se recalculaba en CADA llamada (cada frame desde el panel), lo
    /// que en carpetas grandes saturaba la CPU bajo render por software. Es el único
    /// espacio de índices que usan foco/selección/teclado/activación/render.
    pub fn view_indices(&self) -> Vec<usize> {
        let sig = self.view_signature();
        // ¿Caché válido? (misma firma). Si sí, servirlo.
        if let Some(c) = self.view_cache.borrow().as_ref() {
            if c.signature == sig {
                return c.indices.clone();
            }
        }
        // Recalcular, guardar y devolver.
        let indices = self.compute_view_indices(self.group_new_at_end);
        #[cfg(test)]
        self.view_recomputes.set(self.view_recomputes.get() + 1);
        *self.view_cache.borrow_mut() = Some(ViewCache {
            signature: sig,
            indices: indices.clone(),
        });
        indices
    }

    /// Acceso de TEST al contador de recomputes (cuántas veces se recalculó la vista).
    #[cfg(test)]
    pub fn view_recomputes_for_test(&self) -> u32 {
        self.view_recomputes.get()
    }

    /// Cálculo CRUDO de los índices de vista (filtrar → ordenar → agrupar-al-final).
    /// No cachea; lo invoca `view_indices`. Si `new_items_at_end`, las filas resaltadas
    /// van al final de forma ESTABLE (conservando su orden relativo).
    fn compute_view_indices(&self, new_items_at_end: bool) -> Vec<usize> {
        let mut idx: Vec<usize> = if self.table.filters.is_empty() {
            (0..self.entries.len()).collect()
        } else {
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, e)| filter_matches(e, &self.table.filters))
                .map(|(i, _)| i)
                .collect()
        };
        let sort = self.sort;
        idx.sort_by(|&a, &b| crate::sort::cmp_entries(&self.entries[a], &self.entries[b], &sort));
        if new_items_at_end && !self.highlighted.is_empty() {
            let hl = &self.highlighted;
            idx.sort_by_key(|&i| hl.contains(&self.entries[i].path));
        }
        idx
    }

    /// La entrada con foco, donde `focused` es una posición en la VISTA (no en
    /// `entries`). Devuelve la entry real correspondiente.
    pub fn focused_view_entry(&self) -> Option<&Entry> {
        let view = self.view_indices();
        let pos = self.focused?;
        view.get(pos).and_then(|&real| self.entries.get(real))
    }

    /// La entrada en la posición de VISTA `pos` (filtrada+ordenada). `None` si está fuera de
    /// rango. Reusado por el rename inline para resolver la fila a renombrar.
    pub fn view_entry_at(&self, pos: usize) -> Option<&Entry> {
        let view = self.view_indices();
        view.get(pos).and_then(|&real| self.entries.get(real))
    }

    /// Cuántas filas tiene la vista actual (filtrada). Para acotar el avance del rename en cadena.
    pub fn view_len(&self) -> usize {
        self.view_indices().len()
    }

    /// Navega a una carpeta nueva: registra en el historial y limpia entries/foco.
    /// (La UI lanzará el listado de `dir` tras llamar esto.)
    pub fn navigate_to(&mut self, dir: PathBuf) {
        self.history.push(dir.clone());
        self.enter(dir);
    }

    /// Va atrás en el historial. Devuelve la nueva carpeta si se movió.
    pub fn go_back(&mut self) -> Option<PathBuf> {
        let path = self.history.back().map(Path::to_path_buf)?;
        self.enter(path.clone());
        Some(path)
    }

    /// Va adelante en el historial. Devuelve la nueva carpeta si se movió.
    pub fn go_forward(&mut self) -> Option<PathBuf> {
        let path = self.history.forward().map(Path::to_path_buf)?;
        self.enter(path.clone());
        Some(path)
    }

    /// Salta directo a la posición `index` del historial (menú del botón atrás/
    /// adelante). Devuelve la nueva carpeta si se movió.
    pub fn go_to_history(&mut self, index: usize) -> Option<PathBuf> {
        let path = self.history.jump_to(index).map(Path::to_path_buf)?;
        self.enter(path.clone());
        Some(path)
    }

    /// ¿Hay una carpeta anterior en el historial? (para habilitar el botón Atrás).
    pub fn can_go_back(&self) -> bool {
        self.history.can_back()
    }

    /// ¿Hay una carpeta posterior en el historial? (para habilitar el botón Adelante).
    pub fn can_go_forward(&self) -> bool {
        self.history.can_forward()
    }

    /// Sube al directorio padre (entra al historial). Devuelve el padre si existe.
    pub fn go_up(&mut self) -> Option<PathBuf> {
        let parent = self.current_dir.parent()?.to_path_buf();
        self.navigate_to(parent.clone());
        Some(parent)
    }

    /// Reemplaza la carpeta actual sin tocar el historial (uso interno).
    fn enter(&mut self, dir: PathBuf) {
        self.current_dir = dir;
        self.entries.clear();
        self.focused = None;
        self.selected.clear();
        self.anchor = None;
    }

    /// Posición válida en la vista (clamp a [0, len-1]); None si la vista está vacía.
    fn clamp_pos(&self, pos: usize) -> Option<usize> {
        let len = self.view_indices().len();
        if len == 0 {
            None
        } else {
            Some(pos.min(len - 1))
        }
    }

    /// Selección simple (clic): solo `pos`; fija foco y ancla ahí.
    pub fn select_single(&mut self, pos: usize) {
        if let Some(p) = self.clamp_pos(pos) {
            self.selected = vec![p];
            self.focused = Some(p);
            self.anchor = Some(p);
        }
    }

    /// Toggle (Ctrl+clic): agrega/quita `pos`; mueve foco y ancla a `pos`.
    pub fn select_toggle(&mut self, pos: usize) {
        if let Some(p) = self.clamp_pos(pos) {
            if let Some(idx) = self.selected.iter().position(|&x| x == p) {
                self.selected.remove(idx);
            } else {
                self.selected.push(p);
            }
            self.focused = Some(p);
            self.anchor = Some(p);
        }
    }

    /// Rango (Shift+clic): selecciona desde el ancla hasta `pos` (el ancla NO cambia).
    /// Sin ancla previa equivale a `select_single`.
    pub fn select_range_to(&mut self, pos: usize) {
        let Some(p) = self.clamp_pos(pos) else {
            return;
        };
        let Some(anchor) = self.anchor else {
            self.select_single(p);
            return;
        };
        let anchor = anchor.min(self.view_indices().len().saturating_sub(1));
        let (lo, hi) = if anchor <= p {
            (anchor, p)
        } else {
            (p, anchor)
        };
        self.selected = (lo..=hi).collect();
        self.focused = Some(p);
        // anchor se mantiene
    }

    /// Rectángulo (rubber-band): reemplaza con `positions`, o suma si `additive`.
    pub fn select_rect(&mut self, positions: &[usize], additive: bool) {
        let len = self.view_indices().len();
        let valid: Vec<usize> = positions.iter().copied().filter(|&p| p < len).collect();
        if additive {
            for p in &valid {
                if !self.selected.contains(p) {
                    self.selected.push(*p);
                }
            }
        } else {
            self.selected = valid.clone();
        }
        if let Some(&last) = valid.last() {
            self.focused = Some(last);
            self.anchor = Some(last);
        }
    }

    /// Selecciona toda la vista.
    pub fn select_all(&mut self) {
        let len = self.view_indices().len();
        self.selected = (0..len).collect();
        if len > 0 {
            self.focused = Some(len - 1);
        }
    }

    /// Limpia la selección y el ancla (p. ej. al cambiar el filtro u orden: las
    /// posiciones de vista dejan de ser válidas). El foco se conserva si sigue en rango;
    /// si no, el clamp natural de la navegación por teclado lo corrige.
    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// Mueve el foco `delta` (teclado). Con `extend` (Shift) extiende el rango desde el
    /// ancla; sin extend es selección simple del nuevo foco.
    pub fn move_focus_extend(&mut self, delta: isize, extend: bool) {
        let len = self.view_indices().len();
        if len == 0 {
            return;
        }
        let cur = self.focused.unwrap_or(0) as isize;
        let new = (cur + delta).clamp(0, len as isize - 1) as usize;
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(cur.max(0) as usize);
            }
            self.select_range_to(new);
        } else {
            self.select_single(new);
        }
    }

    /// Lleva el foco a la posición de vista `new` (ya en rango lógico; se clampa). Con
    /// `extend` (Shift) extiende el rango desde el ancla; sin extend, selección simple
    /// del nuevo foco. Helper común de la navegación por bloques (página/inicio/fin).
    fn focus_to(&mut self, new: usize, extend: bool) {
        let len = self.view_indices().len();
        if len == 0 {
            return;
        }
        let new = new.min(len - 1);
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.focused.unwrap_or(0).min(len - 1));
            }
            self.select_range_to(new);
        } else {
            self.select_single(new);
        }
    }

    /// Mueve el foco `delta_pages` páginas (cada página = `rows_per_page` filas). Con
    /// `extend` (Shift+AvPag/RePag) extiende el rango desde el ancla. Clampa a la vista.
    pub fn focus_page(&mut self, delta_pages: isize, rows_per_page: usize, extend: bool) {
        let len = self.view_indices().len();
        if len == 0 {
            return;
        }
        let step = (rows_per_page.max(1) as isize) * delta_pages;
        let cur = self.focused.unwrap_or(0) as isize;
        let new = (cur + step).clamp(0, len as isize - 1) as usize;
        self.focus_to(new, extend);
    }

    /// Foco al primer ítem de la vista (Inicio/Home). Con `extend`, extiende desde el ancla.
    pub fn focus_home(&mut self, extend: bool) {
        self.focus_to(0, extend);
    }

    /// Foco al último ítem de la vista (Fin/End). Con `extend`, extiende desde el ancla.
    pub fn focus_end(&mut self, extend: bool) {
        let len = self.view_indices().len();
        if len > 0 {
            self.focus_to(len - 1, extend);
        }
    }

    /// Mueve el foco `delta` SIN tocar la selección ni el ancla (Ctrl+↑/↓, estilo
    /// Explorer): solo viaja el cursor punteado por la vista, clampado.
    pub fn move_focus_keep(&mut self, delta: isize) {
        let len = self.view_indices().len();
        if len == 0 {
            return;
        }
        let cur = self.focused.unwrap_or(0) as isize;
        let new = (cur + delta).clamp(0, len as isize - 1) as usize;
        self.focused = Some(new);
    }

    /// ¿La posición de vista `pos` está seleccionada?
    pub fn is_selected(&self, pos: usize) -> bool {
        self.selected.contains(&pos)
    }

    /// Cuántos ítems seleccionados.
    pub fn selection_count(&self) -> usize {
        self.selected.len()
    }

    /// Estado persistible (sin entries ni history).
    pub fn to_persist(&self) -> FilePanePersist {
        FilePanePersist {
            current_dir: self.current_dir.clone(),
            sort: self.sort,
            view: self.view,
            show_dirs: self.show_dirs,
            text_filter: None,
            table: Some(self.table.clone()),
        }
    }

    /// Reconstruye desde lo persistido (historial nuevo apuntando a la carpeta).
    pub fn from_persist(p: FilePanePersist) -> Self {
        let mut s = FilePaneState::new(p.current_dir);
        s.sort = p.sort;
        s.view = p.view;
        s.show_dirs = p.show_dirs;
        s.table = match p.table {
            Some(t) => t,
            None => {
                let mut t = TableState::default();
                if let Some(text) = p.text_filter {
                    if !text.is_empty() {
                        t.set_filter(
                            ColumnKind::Name,
                            ColumnFilter::Text {
                                contains: text,
                                case_sensitive: false,
                            },
                        );
                    }
                }
                t
            }
        };
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn nuevo_apunta_su_historial_a_la_carpeta() {
        let s = FilePaneState::new(p("C:/a"));
        assert_eq!(s.current_dir, p("C:/a"));
        assert_eq!(s.history.current(), Some(p("C:/a").as_path()));
        assert!(s.show_dirs);
        assert!(s.table.filters.is_empty());
    }

    #[test]
    fn navigate_y_back_actualizan_carpeta_e_historial() {
        let mut s = FilePaneState::new(p("C:/a"));
        s.navigate_to(p("C:/a/b"));
        assert_eq!(s.current_dir, p("C:/a/b"));
        let back = s.go_back();
        assert_eq!(back, Some(p("C:/a")));
        assert_eq!(s.current_dir, p("C:/a"));
        let fwd = s.go_forward();
        assert_eq!(fwd, Some(p("C:/a/b")));
    }

    #[test]
    fn navegar_limpia_entries_y_foco() {
        let mut s = FilePaneState::new(p("C:/a"));
        s.focused = Some(3);
        s.selected = vec![1, 2];
        s.navigate_to(p("C:/a/b"));
        assert!(s.entries.is_empty());
        assert!(s.focused.is_none());
        assert!(s.selected.is_empty());
    }

    #[test]
    fn migracion_text_filter_a_table_name_filter() {
        use crate::columns::ColumnKind;
        use crate::filter::ColumnFilter;
        let persist = FilePanePersist {
            current_dir: p("C:/a"),
            sort: SortSpec::default(),
            view: ViewMode::default(),
            show_dirs: true,
            text_filter: Some("informe".into()),
            table: None,
        };
        let s = FilePaneState::from_persist(persist);
        let f = s
            .table
            .filters
            .get(&ColumnKind::Name)
            .expect("filtro de nombre migrado");
        assert_eq!(
            *f,
            ColumnFilter::Text {
                contains: "informe".into(),
                case_sensitive: false
            }
        );
    }

    #[test]
    fn persist_nuevo_usa_table_directamente() {
        use crate::columns::{ColumnKind, TableState};
        let mut table = TableState::default();
        table.toggle_visible(ColumnKind::Created);
        let persist = FilePanePersist {
            current_dir: p("C:/a"),
            sort: SortSpec::default(),
            view: ViewMode::default(),
            show_dirs: true,
            text_filter: None,
            table: Some(table.clone()),
        };
        let s = FilePaneState::from_persist(persist);
        assert_eq!(s.table, table);
    }

    #[test]
    fn focused_view_entry_respeta_filtro() {
        use crate::columns::ColumnKind;
        use crate::filter::ColumnFilter;
        use crate::fs_model::{Entry, EntryKind};
        use std::collections::BTreeSet;
        let mut s = FilePaneState::new(p("C:/a"));
        let mk = |name: &str| Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind: EntryKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
        };
        s.entries = vec![mk("a.txt"), mk("b.pdf"), mk("c.txt")];
        // Filtro: solo .txt → vista = [a.txt (idx0), c.txt (idx2)].
        let mut set = BTreeSet::new();
        set.insert("txt".to_string());
        s.table
            .set_filter(ColumnKind::Extension, ColumnFilter::Extensions(set));
        assert_eq!(s.view_indices(), vec![0, 2]);
        // focused = posición 1 en la VISTA → c.txt (no b.pdf).
        s.focused = Some(1);
        assert_eq!(
            s.focused_view_entry().map(|e| e.name.as_str()),
            Some("c.txt")
        );
        // foco fuera de la vista → None.
        s.focused = Some(5);
        assert!(s.focused_view_entry().is_none());
    }

    #[test]
    fn highlighted_set_y_clear() {
        let mut s = FilePaneState::new(p("D:/x"));
        let path = p("D:/x/a.txt");
        s.highlighted.insert(path.clone());
        assert!(s.is_highlighted(&path));
        s.clear_highlight();
        assert!(!s.is_highlighted(&path));
    }

    #[test]
    fn view_al_final_mueve_resaltadas_al_fondo() {
        use crate::fs_model::{Entry, EntryKind};
        let mut s = FilePaneState::new(p("D:/x"));
        let mk = |name: &str| Entry {
            name: name.into(),
            path: PathBuf::from(format!("D:/x/{name}")),
            kind: EntryKind::File,
            size: None,
            modified: None,
            created: None,
            hidden: false,
        };
        s.entries = vec![mk("a"), mk("b"), mk("c")];
        s.highlighted.insert(p("D:/x/b"));
        // Sin flag: orden natural.
        assert_eq!(s.view_indices(), vec![0, 1, 2]);
        // Con flag: b (idx 1) al final, estable. (Cálculo crudo, sin pasar por el caché.)
        assert_eq!(s.compute_view_indices(true), vec![0, 2, 1]);
    }

    #[test]
    fn view_indices_sin_filtro_es_identidad() {
        use crate::fs_model::{Entry, EntryKind};
        let mut s = FilePaneState::new(p("C:/a"));
        let mk = |name: &str| Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind: EntryKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
        };
        s.entries = vec![mk("a"), mk("b")];
        assert_eq!(s.view_indices(), vec![0, 1]);
    }

    #[test]
    fn view_cache_reusa_y_se_invalida() {
        let mut p = pane_n(50);
        // 1ª llamada: calcula. 2ª sin mutar: mismo resultado y NO recalcula.
        let a = p.view_indices();
        assert_eq!(a.len(), 50);
        assert_eq!(p.view_recomputes_for_test(), 1, "una sola vez");
        let _b = p.view_indices();
        assert_eq!(
            p.view_recomputes_for_test(),
            1,
            "2ª llamada sirve del caché"
        );

        // Cambiar el sort invalida (la firma incluye el sort).
        p.sort = crate::fs_model::SortSpec {
            key: crate::fs_model::SortKey::Name,
            ascending: false,
            dirs_first: true,
        };
        let _c = p.view_indices();
        assert_eq!(
            p.view_recomputes_for_test(),
            2,
            "el cambio de sort recalcula"
        );

        // Agregar una entry (cambia el len) invalida.
        p.entries.push(crate::fs_model::Entry {
            name: "zzz.txt".into(),
            path: std::path::PathBuf::from("C:/zzz.txt"),
            kind: crate::fs_model::EntryKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
        });
        let d = p.view_indices();
        assert_eq!(d.len(), 51);
        assert_eq!(p.view_recomputes_for_test(), 3, "agregar entry recalcula");
    }

    #[test]
    fn view_cache_invalida_por_filtro_y_group_flag() {
        use crate::columns::ColumnKind;
        use crate::filter::ColumnFilter;
        let mut p = pane_n(10);
        let _ = p.view_indices();
        let base = p.view_recomputes_for_test();
        // Cambiar un filtro invalida.
        p.table.set_filter(
            ColumnKind::Name,
            ColumnFilter::Text {
                contains: "f1".into(),
                case_sensitive: false,
            },
        );
        let _ = p.view_indices();
        assert_eq!(p.view_recomputes_for_test(), base + 1);
        // Toggle group_new_at_end invalida.
        p.group_new_at_end = !p.group_new_at_end;
        let _ = p.view_indices();
        assert_eq!(p.view_recomputes_for_test(), base + 2);
    }

    #[test]
    fn clone_no_arrastra_cache_viejo() {
        let p = pane_n(5);
        let _ = p.view_indices(); // llena el caché del original
        let mut c = p.clone();
        // Mutar el clon y pedir su vista: debe reflejar SU estado, no el caché del original.
        c.entries.clear();
        assert_eq!(c.view_indices().len(), 0);
        // El original sigue intacto.
        assert_eq!(p.view_indices().len(), 5);
    }

    fn pane_n(n: usize) -> FilePaneState {
        use crate::fs_model::{Entry, EntryKind};
        let mk = |name: &str| Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind: EntryKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
        };
        let mut p = FilePaneState::new(PathBuf::from("C:/"));
        p.entries = (0..n).map(|i| mk(&format!("f{i}.txt"))).collect();
        p
    }

    #[test]
    fn sel_single_fija_ancla() {
        let mut p = pane_n(5);
        p.select_single(2);
        assert_eq!(p.selected, vec![2]);
        assert_eq!(p.focused, Some(2));
        assert_eq!(p.anchor, Some(2));
        p.select_single(4);
        assert_eq!(p.selected, vec![4]);
        assert_eq!(p.anchor, Some(4));
    }

    #[test]
    fn sel_toggle_agrega_y_quita() {
        let mut p = pane_n(5);
        p.select_single(1);
        p.select_toggle(3);
        let mut s = p.selected.clone();
        s.sort_unstable();
        assert_eq!(s, vec![1, 3]);
        assert_eq!(p.focused, Some(3));
        p.select_toggle(1);
        assert_eq!(p.selected, vec![3]);
    }

    #[test]
    fn sel_range_normaliza_y_no_mueve_ancla() {
        let mut p = pane_n(6);
        p.select_single(4);
        p.select_range_to(1);
        let mut s = p.selected.clone();
        s.sort_unstable();
        assert_eq!(s, vec![1, 2, 3, 4]);
        assert_eq!(p.anchor, Some(4));
        assert_eq!(p.focused, Some(1));
    }

    #[test]
    fn sel_range_sin_ancla_es_single() {
        let mut p = pane_n(5);
        // sin ancla previa
        p.select_range_to(2);
        assert_eq!(p.selected, vec![2]);
        assert_eq!(p.anchor, Some(2));
    }

    #[test]
    fn sel_rect_reemplaza_o_suma() {
        let mut p = pane_n(6);
        p.select_single(0);
        p.select_rect(&[2, 3], false);
        let mut s = p.selected.clone();
        s.sort_unstable();
        assert_eq!(s, vec![2, 3]);
        p.select_rect(&[5], true);
        let mut s = p.selected.clone();
        s.sort_unstable();
        assert_eq!(s, vec![2, 3, 5]);
    }

    #[test]
    fn sel_all_toma_la_vista() {
        let mut p = pane_n(4);
        p.select_all();
        let mut s = p.selected.clone();
        s.sort_unstable();
        assert_eq!(s, vec![0, 1, 2, 3]);
    }

    #[test]
    fn clear_selection_vacia_seleccion_y_ancla() {
        let mut p = pane_n(5);
        p.select_single(2);
        assert_eq!(p.selected, vec![2]);
        assert_eq!(p.anchor, Some(2));
        p.clear_selection();
        assert!(p.selected.is_empty());
        assert_eq!(p.anchor, None);
    }

    #[test]
    fn move_focus_extend_shift_extiende_desde_ancla() {
        let mut p = pane_n(6);
        p.select_single(2);
        p.move_focus_extend(1, true);
        let mut s = p.selected.clone();
        s.sort_unstable();
        assert_eq!(s, vec![2, 3]);
        p.move_focus_extend(1, true);
        let mut s = p.selected.clone();
        s.sort_unstable();
        assert_eq!(s, vec![2, 3, 4]);
        assert_eq!(p.anchor, Some(2));
    }

    #[test]
    fn move_focus_extend_sin_shift_es_single() {
        let mut p = pane_n(6);
        p.select_single(2);
        p.move_focus_extend(1, false);
        assert_eq!(p.selected, vec![3]);
        assert_eq!(p.anchor, Some(3));
    }

    #[test]
    fn focus_page_avanza_y_clampa() {
        let mut p = pane_n(100);
        p.select_single(0);
        // PageDown (1 página de 20) desde 0 → 20.
        p.focus_page(1, 20, false);
        assert_eq!(p.focused, Some(20));
        assert_eq!(p.selected, vec![20]);
        // Otra página → 40.
        p.focus_page(1, 20, false);
        assert_eq!(p.focused, Some(40));
        // PageUp dos páginas → 0.
        p.focus_page(-2, 20, false);
        assert_eq!(p.focused, Some(0));
        // PageUp más allá del inicio: clampa a 0.
        p.focus_page(-5, 20, false);
        assert_eq!(p.focused, Some(0));
        // PageDown enorme: clampa al último.
        p.focus_page(10, 20, false);
        assert_eq!(p.focused, Some(99));
    }

    #[test]
    fn focus_home_y_end() {
        let mut p = pane_n(100);
        p.select_single(40);
        p.focus_end(false);
        assert_eq!(p.focused, Some(99));
        assert_eq!(p.selected, vec![99]);
        p.focus_home(false);
        assert_eq!(p.focused, Some(0));
        assert_eq!(p.selected, vec![0]);
    }

    #[test]
    fn shift_end_extiende_desde_ancla() {
        let mut p = pane_n(100);
        // Ancla en 5.
        p.select_single(5);
        // Shift+End → selección 5..=99 (50 entradas… en realidad 95).
        p.focus_end(true);
        let mut s = p.selected.clone();
        s.sort_unstable();
        assert_eq!(s.first(), Some(&5));
        assert_eq!(s.last(), Some(&99));
        assert_eq!(s.len(), 95);
        assert_eq!(p.anchor, Some(5));
        assert_eq!(p.focused, Some(99));
    }

    #[test]
    fn shift_page_extiende_por_bloques() {
        let mut p = pane_n(100);
        p.select_single(10);
        // Shift+PageDown (página=20) → ancla 10, foco 30, selección 10..=30.
        p.focus_page(1, 20, true);
        let mut s = p.selected.clone();
        s.sort_unstable();
        assert_eq!(s.first(), Some(&10));
        assert_eq!(s.last(), Some(&30));
        assert_eq!(p.anchor, Some(10));
    }

    #[test]
    fn ctrl_flecha_mueve_foco_sin_tocar_seleccion() {
        let mut p = pane_n(100);
        p.select_single(40);
        assert_eq!(p.selected, vec![40]);
        // Ctrl+↓: el foco baja pero la selección NO cambia.
        p.move_focus_keep(1);
        assert_eq!(p.focused, Some(41));
        assert_eq!(p.selected, vec![40], "Ctrl+↓ no toca la selección");
        assert_eq!(p.anchor, Some(40), "ni el ancla");
        // Ctrl+↑ por debajo de 0 clampa.
        p.move_focus_keep(-100);
        assert_eq!(p.focused, Some(0));
        assert_eq!(p.selected, vec![40]);
    }

    #[test]
    fn ops_clampean_a_la_vista() {
        let mut p = pane_n(3);
        p.select_single(99);
        assert_eq!(p.focused, Some(2));
        assert_eq!(p.selected, vec![2]);
    }

    #[test]
    fn ancla_se_limpia_al_navegar() {
        let mut p = pane_n(3);
        p.select_single(1);
        assert_eq!(p.anchor, Some(1));
        p.navigate_to(PathBuf::from("C:/otra"));
        assert_eq!(p.anchor, None);
        assert!(p.selected.is_empty());
    }

    #[test]
    fn persist_round_trip_conserva_lo_serializable() {
        let mut s = FilePaneState::new(p("C:/a"));
        s.show_dirs = false;
        let restored = FilePaneState::from_persist(s.to_persist());
        assert_eq!(restored.current_dir, p("C:/a"));
        assert!(!restored.show_dirs);
        // El historial se reinicia apuntando a la carpeta.
        assert_eq!(restored.history.current(), Some(p("C:/a").as_path()));
    }
}
