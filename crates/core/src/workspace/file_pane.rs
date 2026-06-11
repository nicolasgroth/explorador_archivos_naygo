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
#[derive(Clone, Debug)]
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

    /// Las entries VISIBLES: las que pasan los filtros activos de la tabla, en el
    /// orden actual de `entries` (que `pump_one` mantiene ordenado por `sort`).
    /// Es el ÚNICO espacio de índices que usan foco/selección/teclado/activación.
    pub fn view_indices(&self) -> Vec<usize> {
        self.view_indices_ordered(self.group_new_at_end)
    }

    /// Índices de la vista: filtrada Y ORDENADA por el `sort` del panel (UNA sola
    /// definición del orden — la misma que pinta la UI; foco/selección/teclado y
    /// render no pueden desalinearse). Si `new_items_at_end`, las filas resaltadas
    /// van al final de forma ESTABLE (conservando su orden relativo).
    pub fn view_indices_ordered(&self, new_items_at_end: bool) -> Vec<usize> {
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
        // Con flag: b (idx 1) al final, estable.
        assert_eq!(s.view_indices_ordered(true), vec![0, 2, 1]);
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
