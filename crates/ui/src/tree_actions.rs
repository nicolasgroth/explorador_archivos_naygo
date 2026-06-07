// Naygo — acciones que el árbol acumula durante el pintado y se ejecutan después.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Igual que `PaneRequest` en el file panel: el render del árbol no muta el estado
//! ni hace I/O directamente; acumula `TreeAction`s que `NaygoApp` procesa tras
//! pintar, evitando préstamos conflictivos.

use std::path::PathBuf;

/// Una acción pedida desde el árbol durante el pintado.
#[derive(Clone, Debug, PartialEq, Eq)]
// consumido en Tareas 8-9 (estado del árbol + render)
#[allow(dead_code)]
pub enum TreeAction {
    /// Expandir (y listar) el nodo en `path`.
    Expand(PathBuf),
    /// Colapsar el nodo en `path`.
    Collapse(PathBuf),
    /// Navegar el panel activo a `path` (clic en el nombre de una carpeta).
    Navigate(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_action_es_comparable() {
        let a = TreeAction::Expand(PathBuf::from("C:\\"));
        let b = TreeAction::Expand(PathBuf::from("C:\\"));
        assert_eq!(a, b);
        assert_ne!(a, TreeAction::Collapse(PathBuf::from("C:\\")));
    }
}
