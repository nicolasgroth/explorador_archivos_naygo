// Naygo — mapeo de teclado a acciones de navegación (lógica pura testeable).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Traduce una pulsación de tecla (representada con tipos propios, no egui) a una
//! `Action` de alto nivel. Se aísla aquí para testear el mapeo sin levantar la UI.
//! En la Fase 1 el mapa es fijo (default estilo Windows); los atajos
//! configurables llegan en una fase posterior.

pub use naygo_core::keymap::Action;

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

/// Botones extra del mouse (laterales). Espejo de `egui::PointerButton`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseExtra {
    /// Botón lateral 1 (típicamente "atrás").
    Back,
    /// Botón lateral 2 (típicamente "adelante").
    Forward,
}

/// Mapea un botón lateral del mouse a su acción de navegación.
pub fn map_mouse_extra(button: MouseExtra) -> Action {
    match button {
        MouseExtra::Back => Action::GoBack,
        MouseExtra::Forward => Action::GoForward,
    }
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

    #[test]
    fn botones_laterales_del_mouse_navegan() {
        assert_eq!(map_mouse_extra(MouseExtra::Back), Action::GoBack);
        assert_eq!(map_mouse_extra(MouseExtra::Forward), Action::GoForward);
    }
}
