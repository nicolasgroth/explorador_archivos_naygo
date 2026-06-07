// Naygo — lógica pura del menú de columna (qué acción produce cada interacción).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! El render del desplegable (Tarea siguiente) llama a estas funciones puras para
//! decidir qué `TableAction` emitir. Separar la decisión del dibujo las hace
//! testeables sin egui.

// consumido en la Tarea 7 (render del menú)
#![allow(dead_code)]

use crate::table_actions::TableAction;
use naygo_core::columns::{sort_key_of, ColumnKind};
use naygo_core::filter::ColumnFilter;
use naygo_core::fs_model::SortSpec;

/// Acción al pedir "ordenar" por una columna en una dirección.
pub fn sort_action(kind: ColumnKind, ascending: bool, dirs_first: bool) -> TableAction {
    TableAction::SetSort(SortSpec {
        key: sort_key_of(kind),
        ascending,
        dirs_first,
    })
}

/// Acción al cambiar el texto del filtro de Nombre. Texto vacío → quitar filtro.
pub fn name_filter_action(contains: &str, case_sensitive: bool) -> TableAction {
    if contains.is_empty() {
        TableAction::ClearFilter(ColumnKind::Name)
    } else {
        TableAction::SetFilter(
            ColumnKind::Name,
            ColumnFilter::Text {
                contains: contains.to_string(),
                case_sensitive,
            },
        )
    }
}

/// Acción al cambiar el conjunto de extensiones marcadas. Vacío → quitar filtro.
pub fn extensions_filter_action(selected: std::collections::BTreeSet<String>) -> TableAction {
    if selected.is_empty() {
        TableAction::ClearFilter(ColumnKind::Extension)
    } else {
        TableAction::SetFilter(ColumnKind::Extension, ColumnFilter::Extensions(selected))
    }
}

/// Acción al fijar un rango de tamaño. Ambos None → quitar filtro.
pub fn size_filter_action(min: Option<u64>, max: Option<u64>) -> TableAction {
    if min.is_none() && max.is_none() {
        TableAction::ClearFilter(ColumnKind::Size)
    } else {
        TableAction::SetFilter(ColumnKind::Size, ColumnFilter::SizeRange { min, max })
    }
}

/// Unidad de tamaño para los controles de filtro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeUnit {
    Kb,
    Mb,
    Gb,
}

/// Convierte un valor + unidad (KB/MB/GB) a bytes. Para los controles de tamaño.
pub fn to_bytes(value: f64, unit: SizeUnit) -> u64 {
    let mult = match unit {
        SizeUnit::Kb => 1024.0,
        SizeUnit::Mb => 1024.0 * 1024.0,
        SizeUnit::Gb => 1024.0 * 1024.0 * 1024.0,
    };
    (value * mult).max(0.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::fs_model::SortKey;
    use std::collections::BTreeSet;

    #[test]
    fn sort_action_usa_sortkey_de_la_columna() {
        let a = sort_action(ColumnKind::Extension, true, true);
        match a {
            TableAction::SetSort(spec) => {
                assert_eq!(spec.key, SortKey::Extension);
                assert!(spec.ascending);
                assert!(spec.dirs_first);
            }
            _ => panic!("esperaba SetSort"),
        }
    }

    #[test]
    fn name_filter_vacio_quita_filtro() {
        assert_eq!(name_filter_action("", false), TableAction::ClearFilter(ColumnKind::Name));
    }

    #[test]
    fn name_filter_con_texto_setea() {
        let a = name_filter_action("doc", true);
        assert_eq!(
            a,
            TableAction::SetFilter(
                ColumnKind::Name,
                ColumnFilter::Text { contains: "doc".into(), case_sensitive: true }
            )
        );
    }

    #[test]
    fn extensions_vacio_quita_filtro() {
        assert_eq!(
            extensions_filter_action(BTreeSet::new()),
            TableAction::ClearFilter(ColumnKind::Extension)
        );
    }

    #[test]
    fn size_ambos_none_quita_filtro() {
        assert_eq!(
            size_filter_action(None, None),
            TableAction::ClearFilter(ColumnKind::Size)
        );
    }

    #[test]
    fn to_bytes_convierte_unidades() {
        assert_eq!(to_bytes(1.0, SizeUnit::Kb), 1024);
        assert_eq!(to_bytes(2.0, SizeUnit::Mb), 2 * 1024 * 1024);
        assert_eq!(to_bytes(1.0, SizeUnit::Gb), 1024 * 1024 * 1024);
    }
}
