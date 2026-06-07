// Naygo — modelo de columnas del file panel (puro, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Define qué columnas existen y el estado de tabla de un panel (qué columnas se
//! ven, en qué orden, su ancho) más los filtros activos. Puro y testeable.

use serde::{Deserialize, Serialize};

/// Qué columna. Extensible: agregar variante + su extractor a futuro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ColumnKind {
    Name,
    Extension,
    Size,
    Modified,
    Created,
}

use crate::filter::ColumnFilter;
use crate::fs_model::SortKey;
use std::collections::BTreeMap;

/// Ancho mínimo/máximo de una columna (px lógicos).
pub const MIN_COLUMN_WIDTH: f32 = 40.0;
pub const MAX_COLUMN_WIDTH: f32 = 1200.0;

/// Una columna de la tabla: qué es, si se ve, su ancho. El ORDEN del Vec en
/// `TableState.columns` es el orden visual.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub kind: ColumnKind,
    pub visible: bool,
    pub width: f32,
}

/// Estado de tabla de un panel: columnas (orden/visibilidad/ancho) + filtros AND.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableState {
    pub columns: Vec<ColumnSpec>,
    pub filters: BTreeMap<ColumnKind, ColumnFilter>,
}

impl Default for TableState {
    fn default() -> Self {
        let col = |kind, visible, width| ColumnSpec { kind, visible, width };
        TableState {
            columns: vec![
                col(ColumnKind::Name, true, 240.0),
                col(ColumnKind::Extension, true, 90.0),
                col(ColumnKind::Size, true, 90.0),
                col(ColumnKind::Modified, true, 140.0),
                col(ColumnKind::Created, false, 140.0),
            ],
            filters: BTreeMap::new(),
        }
    }
}

impl TableState {
    /// Itera las columnas visibles en orden visual.
    pub fn visible_columns(&self) -> impl Iterator<Item = &ColumnSpec> {
        self.columns.iter().filter(|c| c.visible)
    }

    /// Alterna la visibilidad de una columna. Nombre nunca se oculta.
    pub fn toggle_visible(&mut self, kind: ColumnKind) {
        if kind == ColumnKind::Name {
            return;
        }
        if let Some(c) = self.columns.iter_mut().find(|c| c.kind == kind) {
            c.visible = !c.visible;
        }
    }

    /// Mueve la columna del índice `from` al índice `to` (reordena el Vec).
    pub fn move_column(&mut self, from: usize, to: usize) {
        if from >= self.columns.len() || to >= self.columns.len() || from == to {
            return;
        }
        let c = self.columns.remove(from);
        self.columns.insert(to, c);
    }

    /// Fija el ancho de una columna, con clamp a [MIN, MAX].
    pub fn set_width(&mut self, kind: ColumnKind, width: f32) {
        if let Some(c) = self.columns.iter_mut().find(|c| c.kind == kind) {
            c.width = width.clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
        }
    }

    /// Establece (o reemplaza) el filtro de una columna.
    pub fn set_filter(&mut self, kind: ColumnKind, filter: ColumnFilter) {
        self.filters.insert(kind, filter);
    }

    /// Quita el filtro de una columna.
    pub fn clear_filter(&mut self, kind: ColumnKind) {
        self.filters.remove(&kind);
    }
}

/// Mapea una columna a su `SortKey` (1:1).
pub fn sort_key_of(kind: ColumnKind) -> SortKey {
    match kind {
        ColumnKind::Name => SortKey::Name,
        ColumnKind::Extension => SortKey::Extension,
        ColumnKind::Size => SortKey::Size,
        ColumnKind::Modified => SortKey::Modified,
        ColumnKind::Created => SortKey::Created,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tiene_las_cinco_columnas_creacion_oculta() {
        let t = TableState::default();
        assert_eq!(t.columns.len(), 5);
        let visible: Vec<ColumnKind> = t.visible_columns().map(|c| c.kind).collect();
        assert_eq!(
            visible,
            vec![ColumnKind::Name, ColumnKind::Extension, ColumnKind::Size, ColumnKind::Modified]
        );
        assert!(t.filters.is_empty());
    }

    #[test]
    fn toggle_visible_oculta_y_muestra() {
        let mut t = TableState::default();
        t.toggle_visible(ColumnKind::Size);
        assert!(!t.columns.iter().find(|c| c.kind == ColumnKind::Size).unwrap().visible);
        t.toggle_visible(ColumnKind::Size);
        assert!(t.columns.iter().find(|c| c.kind == ColumnKind::Size).unwrap().visible);
    }

    #[test]
    fn nombre_no_se_puede_ocultar() {
        let mut t = TableState::default();
        t.toggle_visible(ColumnKind::Name);
        assert!(
            t.columns.iter().find(|c| c.kind == ColumnKind::Name).unwrap().visible,
            "Nombre siempre visible"
        );
    }

    #[test]
    fn move_column_reordena() {
        let mut t = TableState::default();
        t.move_column(2, 0);
        assert_eq!(t.columns[0].kind, ColumnKind::Size);
    }

    #[test]
    fn set_width_clampa() {
        let mut t = TableState::default();
        t.set_width(ColumnKind::Name, 5.0);
        let w = t.columns.iter().find(|c| c.kind == ColumnKind::Name).unwrap().width;
        assert!(w >= MIN_COLUMN_WIDTH, "se respeta el ancho mínimo");
        t.set_width(ColumnKind::Name, 5000.0);
        let w = t.columns.iter().find(|c| c.kind == ColumnKind::Name).unwrap().width;
        assert!(w <= MAX_COLUMN_WIDTH, "se respeta el ancho máximo");
    }

    #[test]
    fn set_y_clear_filter() {
        use crate::filter::ColumnFilter;
        let mut t = TableState::default();
        t.set_filter(
            ColumnKind::Name,
            ColumnFilter::Text { contains: "x".into(), case_sensitive: false },
        );
        assert!(t.filters.contains_key(&ColumnKind::Name));
        t.clear_filter(ColumnKind::Name);
        assert!(!t.filters.contains_key(&ColumnKind::Name));
    }

    #[test]
    fn sort_key_of_mapea_columna_a_sortkey() {
        use crate::fs_model::SortKey;
        assert_eq!(sort_key_of(ColumnKind::Name), SortKey::Name);
        assert_eq!(sort_key_of(ColumnKind::Extension), SortKey::Extension);
        assert_eq!(sort_key_of(ColumnKind::Size), SortKey::Size);
        assert_eq!(sort_key_of(ColumnKind::Modified), SortKey::Modified);
        assert_eq!(sort_key_of(ColumnKind::Created), SortKey::Created);
    }

    #[test]
    fn round_trip_serde() {
        use crate::filter::ColumnFilter;
        let mut t = TableState::default();
        t.toggle_visible(ColumnKind::Created);
        t.set_filter(
            ColumnKind::Name,
            ColumnFilter::Text { contains: "doc".into(), case_sensitive: false },
        );
        let json = serde_json::to_string(&t).unwrap();
        let back: TableState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }
}
