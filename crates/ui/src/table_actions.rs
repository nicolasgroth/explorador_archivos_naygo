// Naygo — acciones del menú de columna, acumuladas al pintar y aplicadas después.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Igual que `PaneRequest`/`TreeAction`: el render del menú de columna no muta el
//! estado; acumula `TableAction`s que `NaygoApp` procesa tras pintar.

use naygo_core::columns::ColumnKind;
use naygo_core::filter::ColumnFilter;
use naygo_core::fs_model::SortSpec;

/// Una acción pedida desde el menú/encabezado de columna.
#[derive(Clone, Debug, PartialEq)]
pub enum TableAction {
    /// Cambiar el orden del panel.
    SetSort(SortSpec),
    /// Establecer/reemplazar el filtro de una columna.
    SetFilter(ColumnKind, ColumnFilter),
    /// Quitar el filtro de una columna.
    ClearFilter(ColumnKind),
    /// Alternar visibilidad de una columna.
    ToggleColumn(ColumnKind),
    /// Mover una columna del índice `from` al `to`. (Emitido en Tarea 9: drag.)
    MoveColumn(usize, usize),
    /// Fijar el ancho de una columna. (Emitido en Tarea 10: drag de ancho.)
    #[allow(dead_code)]
    SetColumnWidth(ColumnKind, f32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_action_es_comparable() {
        let a = TableAction::ClearFilter(ColumnKind::Name);
        assert_eq!(a, TableAction::ClearFilter(ColumnKind::Name));
        assert_ne!(a, TableAction::ToggleColumn(ColumnKind::Name));
    }
}
