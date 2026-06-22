// Naygo — paleta de comandos: modelo de comandos y fuzzy-match (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA de la paleta de comandos (sin UI ni Windows). La UI arma la lista de
//! `Command` desde sus fuentes (acciones, archivos, recientes, favoritos, temas) y usa
//! `filter_and_rank` para filtrar/ordenar según lo que el usuario escribe. 100% testeable.

use crate::keymap::Action;
use crate::theme::ThemeId;
use std::path::PathBuf;

/// Categoría de un comando (define el ícono y la etiqueta en la UI).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandCategory {
    Action,
    File,
    Recent,
    Favorite,
    Theme,
    Config,
}

/// Qué ejecuta un comando al elegirlo.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandPayload {
    /// Una acción del keymap; se rutea por el dispatcher de teclado existente.
    Action(Action),
    /// Navegar el panel activo a esta ruta (reciente/favorito).
    Navigate(PathBuf),
    /// Enfocar/seleccionar un entry YA cargado en el panel activo, por su índice de VISTA.
    FocusEntry(usize),
    /// Aplicar este tema.
    Theme(ThemeId),
    /// Abrir la ventana de configuración.
    OpenConfig,
}

/// Un comando de la paleta.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    /// Texto a mostrar (ya traducido por la UI).
    pub label: String,
    pub category: CommandCategory,
    /// Atajo legible ("Ctrl+C"); solo acciones con atajo. Vacío si no tiene.
    pub shortcut: String,
    pub payload: CommandPayload,
}

/// Resultado de filtrar: índice del comando + score + posiciones (char-index) que matchearon.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandMatch {
    pub index: usize,
    pub score: i32,
    pub hit_positions: Vec<usize>,
}
