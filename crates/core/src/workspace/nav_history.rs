// Naygo — historial de navegación atrás/adelante de un panel (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `NavHistory` es la pila de rutas visitadas de un panel, con un cursor. Modela
//! el atrás/adelante de un navegador: `push` a una ruta nueva trunca la rama de
//! "adelante". Puro y testeable; no toca disco.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Tope de profundidad: más allá, se descartan las entradas más viejas.
const MAX_DEPTH: usize = 256;

/// Historial de navegación de un panel: rutas visitadas + cursor a la actual.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct NavHistory {
    /// Rutas visitadas, de la más vieja a la más nueva.
    stack: Vec<PathBuf>,
    /// Índice de la ruta "actual" dentro de `stack`. `None` si está vacío.
    cursor: Option<usize>,
}

impl NavHistory {
    /// Historial vacío.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ruta actual (donde está parado el cursor), si hay alguna.
    pub fn current(&self) -> Option<&Path> {
        self.cursor.map(|i| self.stack[i].as_path())
    }

    /// Navega a una ruta nueva: la agrega tras la actual y trunca la rama de
    /// "adelante" (todo lo que estaba después del cursor). El cursor pasa a la nueva.
    pub fn push(&mut self, path: PathBuf) {
        // Truncar la rama de adelante.
        if let Some(i) = self.cursor {
            self.stack.truncate(i + 1);
        } else {
            self.stack.clear();
        }
        self.stack.push(path);
        self.cursor = Some(self.stack.len() - 1);

        // Respetar el tope de profundidad descartando las más viejas.
        if self.stack.len() > MAX_DEPTH {
            let overflow = self.stack.len() - MAX_DEPTH;
            self.stack.drain(0..overflow);
            self.cursor = Some(self.stack.len() - 1);
        }
    }

    /// `true` si hay a dónde ir atrás.
    pub fn can_back(&self) -> bool {
        matches!(self.cursor, Some(i) if i > 0)
    }

    /// `true` si hay a dónde ir adelante.
    pub fn can_forward(&self) -> bool {
        matches!(self.cursor, Some(i) if i + 1 < self.stack.len())
    }

    /// Mueve el cursor un paso atrás y devuelve la ruta nueva, o `None` si no se puede.
    pub fn back(&mut self) -> Option<&Path> {
        if self.can_back() {
            let i = self.cursor.unwrap() - 1;
            self.cursor = Some(i);
            Some(self.stack[i].as_path())
        } else {
            None
        }
    }

    /// Mueve el cursor un paso adelante y devuelve la ruta nueva, o `None`.
    pub fn forward(&mut self) -> Option<&Path> {
        if self.can_forward() {
            let i = self.cursor.unwrap() + 1;
            self.cursor = Some(i);
            Some(self.stack[i].as_path())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn historial_vacio_no_tiene_actual_ni_movimiento() {
        let mut h = NavHistory::new();
        assert!(h.current().is_none());
        assert!(!h.can_back());
        assert!(!h.can_forward());
        assert!(h.back().is_none());
        assert!(h.forward().is_none());
    }

    #[test]
    fn push_avanza_la_actual() {
        let mut h = NavHistory::new();
        h.push(p("C:/a"));
        h.push(p("C:/a/b"));
        assert_eq!(h.current(), Some(p("C:/a/b").as_path()));
        assert!(h.can_back());
        assert!(!h.can_forward());
    }

    #[test]
    fn back_y_forward_mueven_el_cursor() {
        let mut h = NavHistory::new();
        h.push(p("C:/a"));
        h.push(p("C:/a/b"));
        h.push(p("C:/a/b/c"));
        assert_eq!(h.back(), Some(p("C:/a/b").as_path()));
        assert_eq!(h.back(), Some(p("C:/a").as_path()));
        assert!(!h.can_back());
        assert_eq!(h.forward(), Some(p("C:/a/b").as_path()));
        assert_eq!(h.current(), Some(p("C:/a/b").as_path()));
    }

    #[test]
    fn push_trunca_la_rama_de_adelante() {
        let mut h = NavHistory::new();
        h.push(p("C:/a"));
        h.push(p("C:/a/b"));
        h.push(p("C:/a/b/c"));
        h.back(); // estamos en C:/a/b
        h.push(p("C:/a/b/x")); // navegar a algo nuevo trunca "c"
        assert_eq!(h.current(), Some(p("C:/a/b/x").as_path()));
        assert!(!h.can_forward(), "la rama de adelante (c) se truncó");
    }
}
