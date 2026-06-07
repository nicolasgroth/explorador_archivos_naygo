// Naygo — lógica pura del ordenamiento por clic en encabezado de columna.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Dado el `SortSpec` actual y la columna (SortKey) que el usuario clicó, devuelve
//! el nuevo `SortSpec`: si clicó la columna ya activa, invierte la dirección; si
//! clicó otra, la activa en ascendente. `dirs_first` se preserva. Puro, testeable.

use naygo_core::fs_model::{SortKey, SortSpec};

/// Calcula el nuevo `SortSpec` al clicar el encabezado de `clicked`.
// El orden hoy se pide desde el menú de columna (TableAction::SetSort); esta
// lógica pura se conserva (con sus tests) por si vuelve el clic directo al header.
#[allow(dead_code)]
pub fn next_sort_on_header_click(current: SortSpec, clicked: SortKey) -> SortSpec {
    if current.key == clicked {
        SortSpec {
            ascending: !current.ascending,
            ..current
        }
    } else {
        SortSpec {
            key: clicked,
            ascending: true,
            dirs_first: current.dirs_first,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clic_en_columna_activa_invierte_direccion() {
        let cur = SortSpec {
            key: SortKey::Name,
            ascending: true,
            dirs_first: true,
        };
        let next = next_sort_on_header_click(cur, SortKey::Name);
        assert_eq!(next.key, SortKey::Name);
        assert!(!next.ascending, "invierte a descendente");
        assert!(next.dirs_first, "preserva dirs_first");
    }

    #[test]
    fn clic_en_columna_activa_descendente_vuelve_a_ascendente() {
        let cur = SortSpec {
            key: SortKey::Size,
            ascending: false,
            dirs_first: false,
        };
        let next = next_sort_on_header_click(cur, SortKey::Size);
        assert!(next.ascending);
    }

    #[test]
    fn clic_en_otra_columna_la_activa_ascendente() {
        let cur = SortSpec {
            key: SortKey::Name,
            ascending: false,
            dirs_first: true,
        };
        let next = next_sort_on_header_click(cur, SortKey::Modified);
        assert_eq!(next.key, SortKey::Modified);
        assert!(next.ascending, "nueva columna arranca ascendente");
        assert!(next.dirs_first, "preserva dirs_first");
    }

    #[test]
    fn clic_en_otra_columna_preserva_dirs_first_false() {
        // Protege que el brazo "otra columna" lea current.dirs_first y no lo
        // hardcodee a true (los otros tests parten de dirs_first: true).
        let cur = SortSpec {
            key: SortKey::Name,
            ascending: true,
            dirs_first: false,
        };
        let next = next_sort_on_header_click(cur, SortKey::Size);
        assert_eq!(next.key, SortKey::Size);
        assert!(!next.dirs_first, "preserva dirs_first=false");
    }
}
