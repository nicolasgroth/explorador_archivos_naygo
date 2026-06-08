// Naygo — mapeo de teclado a acciones de navegación (lógica pura testeable).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Traduce una pulsación de tecla (representada con tipos propios, no egui) a una
//! `Action` de alto nivel. Se aísla aquí para testear el mapeo sin levantar la UI.
//! En la Fase 1 el mapa es fijo (default estilo Windows); los atajos
//! configurables llegan en una fase posterior.

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
    /// Abrir el elemento enfocado con su app por defecto (menú contextual).
    Open,
    /// Abrir el elemento enfocado con… (diálogo nativo de elección de app).
    OpenWith,
    /// Subir un nivel (carpeta padre).
    GoUp,
    /// Ir atrás en el historial del panel activo.
    GoBack,
    /// Ir adelante en el historial del panel activo.
    GoForward,
    /// Cambiar el panel de archivos activo.
    SwitchPane,
    /// Cancelar el listado en curso.
    CancelListing,
    /// Copiar la selección al portapapeles del sistema (Ctrl+C).
    Copy,
    /// Cortar la selección al portapapeles del sistema (Ctrl+X).
    Cut,
    /// Pegar el portapapeles del sistema en la carpeta activa, según su tipo (Ctrl+V).
    Paste,
    /// Eliminar la selección a la papelera (Delete).
    Delete,
    /// Eliminar la selección permanentemente (Shift+Delete).
    DeletePermanent,
    /// Renombrar el elemento enfocado (F2).
    Rename,
    /// Crear un archivo nuevo en la carpeta activa (Ctrl+N).
    NewFile,
    /// Crear una carpeta nueva en la carpeta activa (Ctrl+Shift+N).
    NewDir,
    /// Copiar la selección al OTRO panel de archivos (F5).
    CopyToOther,
    /// Mover la selección al OTRO panel de archivos (F6).
    MoveToOther,
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
