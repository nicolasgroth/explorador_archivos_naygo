// Naygo — mapeo de teclado a acciones de navegación (lógica pura testeable).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Traduce una pulsación de tecla (representada con tipos propios, no egui) a una
//! `Action` de alto nivel. Se aísla aquí para testear el mapeo sin levantar la UI.
//! En la Fase 1 el mapa es fijo (default estilo Windows); los atajos
//! configurables llegan en una fase posterior.

// La API pública de este módulo (`Key`, `Action`, `map_key`) será consumida por
// `app.rs` en la Tarea 8. Hasta entonces, en un build normal (no-test) el
// compilador la ve como código muerto; silenciamos el aviso a nivel de módulo.
#![allow(dead_code)]

/// Teclas que nos interesan en la Fase 1. Espejo reducido de `egui::Key`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    Enter,
    Backspace,
    Tab,
    Escape,
}

/// Acción de alto nivel resultante de una tecla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    MoveUp,
    MoveDown,
    /// Entrar a la carpeta enfocada / abrir el archivo enfocado.
    Activate,
    /// Subir un nivel (carpeta padre).
    GoUp,
    /// Cambiar el panel de archivos activo.
    SwitchPane,
    /// Cancelar el listado en curso.
    CancelListing,
}

/// Mapea una tecla a su acción, si tiene una asignada en la Fase 1.
pub fn map_key(key: Key) -> Option<Action> {
    Some(match key {
        Key::ArrowUp => Action::MoveUp,
        Key::ArrowDown => Action::MoveDown,
        Key::Enter => Action::Activate,
        Key::Backspace | Key::ArrowLeft => Action::GoUp,
        Key::Tab => Action::SwitchPane,
        Key::Escape => Action::CancelListing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flechas_mueven_seleccion() {
        assert_eq!(map_key(Key::ArrowUp), Some(Action::MoveUp));
        assert_eq!(map_key(Key::ArrowDown), Some(Action::MoveDown));
    }

    #[test]
    fn backspace_y_flecha_izquierda_suben_nivel() {
        assert_eq!(map_key(Key::Backspace), Some(Action::GoUp));
        assert_eq!(map_key(Key::ArrowLeft), Some(Action::GoUp));
    }

    #[test]
    fn enter_activa_y_escape_cancela() {
        assert_eq!(map_key(Key::Enter), Some(Action::Activate));
        assert_eq!(map_key(Key::Escape), Some(Action::CancelListing));
    }
}
