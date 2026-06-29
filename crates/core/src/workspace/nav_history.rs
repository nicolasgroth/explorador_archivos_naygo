// Naygo — historial de navegación atrás/adelante de un panel (puro).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

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

    /// Rutas hacia ATRÁS desde el cursor (de la más cercana a la más lejana). Vacío si no hay.
    pub fn back_entries(&self) -> Vec<PathBuf> {
        match self.cursor {
            Some(i) if i > 0 => self.stack[..i].iter().rev().cloned().collect(),
            _ => Vec::new(),
        }
    }

    /// Rutas hacia ADELANTE desde el cursor (de la más cercana a la más lejana). Vacío si no hay.
    pub fn forward_entries(&self) -> Vec<PathBuf> {
        match self.cursor {
            Some(i) if i + 1 < self.stack.len() => self.stack[i + 1..].to_vec(),
            _ => Vec::new(),
        }
    }

    /// La pila completa (de la más vieja a la más nueva) y el índice del cursor.
    /// Para el menú de historial del botón atrás/adelante.
    pub fn stack(&self) -> (&[PathBuf], Option<usize>) {
        (&self.stack, self.cursor)
    }

    /// Salta directo a la posición `index` de la pila (menú de historial) y
    /// devuelve la ruta nueva. `None` si el índice no existe o ya es el actual.
    pub fn jump_to(&mut self, index: usize) -> Option<&Path> {
        if index >= self.stack.len() || Some(index) == self.cursor {
            return None;
        }
        self.cursor = Some(index);
        Some(self.stack[index].as_path())
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
    fn jump_to_salta_directo_y_valida_indices() {
        let mut h = NavHistory::new();
        h.push(p("C:/a"));
        h.push(p("C:/a/b"));
        h.push(p("C:/a/b/c"));
        assert_eq!(h.jump_to(0), Some(p("C:/a").as_path()));
        assert_eq!(h.current(), Some(p("C:/a").as_path()));
        // Adelante sigue disponible (no se truncó la rama).
        assert!(h.can_forward());
        assert_eq!(h.jump_to(2), Some(p("C:/a/b/c").as_path()));
        assert_eq!(h.jump_to(2), None); // ya es el actual
        assert_eq!(h.jump_to(9), None); // fuera de rango
        let (stack, cursor) = h.stack();
        assert_eq!(stack.len(), 3);
        assert_eq!(cursor, Some(2));
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

    #[test]
    fn can_back_forward_reflejan_el_cursor() {
        // Bloquea la semántica de can_back/can_forward que habilita/deshabilita los
        // botones del toolbar: vacío y recién-creado no permiten moverse; tras push hay
        // atrás pero no adelante; tras back se invierte.
        let mut h = NavHistory::new();
        assert!(!h.can_back());
        assert!(!h.can_forward());
        h.push(p("A"));
        assert!(!h.can_back(), "una sola entrada: nada atrás");
        assert!(!h.can_forward());
        h.push(p("B"));
        assert!(h.can_back());
        assert!(!h.can_forward());
        h.back();
        assert!(!h.can_back());
        assert!(h.can_forward());
    }

    #[test]
    fn back_y_forward_entries_parten_la_pila_por_el_cursor() {
        let mut h = NavHistory::new();
        h.push(std::path::PathBuf::from("A"));
        h.push(std::path::PathBuf::from("B"));
        h.push(std::path::PathBuf::from("C")); // cursor en C (índice 2)
        assert_eq!(
            h.back_entries(),
            vec![std::path::PathBuf::from("B"), std::path::PathBuf::from("A")]
        );
        assert!(h.forward_entries().is_empty());
        h.back();
        h.back(); // cursor en A (índice 0)
        assert!(h.back_entries().is_empty());
        assert_eq!(
            h.forward_entries(),
            vec![std::path::PathBuf::from("B"), std::path::PathBuf::from("C")]
        );
    }

    #[test]
    fn respeta_el_tope_de_profundidad() {
        // Empujar más allá del tope descarta las más viejas; el cursor queda en
        // la última y `current` apunta a ella. Protege la lógica de overflow.
        let mut h = NavHistory::new();
        for i in 0..(MAX_DEPTH + 1) {
            h.push(p(&format!("C:/p{i}")));
        }
        assert_eq!(
            h.current(),
            Some(p(&format!("C:/p{}", MAX_DEPTH)).as_path())
        );
        assert!(h.can_back());
        // Yendo atrás hasta el tope, la entrada más vieja ya no es "p0".
        while h.can_back() {
            h.back();
        }
        assert_eq!(h.current(), Some(p("C:/p1").as_path()), "p0 se descartó");
    }
}
