// Naygo — keymap configurable: combinaciones de teclas → acciones (puro, serde).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Modelo PURO de atajos de teclado: `KeyCode`/`Chord`/`Action`/`KeyMap`. Sin egui ni
//! Windows. `action_for` resuelve qué acción dispara una combinación (lo consume la UI);
//! el editor usa `chords_for`/`bind`/`unbind`/`reset_*`. Serde tolerante: un json viejo
//! o incompleto se mergea con los defaults.

use serde::{Deserialize, Serialize};

/// Tecla lógica (espejo serializable de las teclas que la app usa). Sin egui.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyCode {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Enter,
    Backspace,
    Tab,
    Escape,
    Delete,
    F2,
    F3,
    F5,
    F6,
    /// Letra o dígito; normalizada a minúscula al construirla.
    Char(char),
}

/// Una combinación: tecla + modificadores.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chord {
    pub key: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    /// Combinación sin modificadores.
    pub fn plain(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }
    /// Con Ctrl.
    pub fn ctrl(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: true,
            shift: false,
            alt: false,
        }
    }
    /// Con Ctrl+Shift.
    pub fn ctrl_shift(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: true,
            shift: true,
            alt: false,
        }
    }
    /// Con Shift.
    pub fn shift(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: false,
            shift: true,
            alt: false,
        }
    }
    /// Con Alt.
    pub fn alt(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: false,
            shift: false,
            alt: true,
        }
    }
}

/// Acción de alto nivel (movida desde `ui::input`). Enum puro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    MoveUp,
    MoveDown,
    Activate,
    Open,
    OpenWith,
    GoUp,
    GoBack,
    GoForward,
    SwitchPane,
    CancelListing,
    Copy,
    Cut,
    Paste,
    Delete,
    DeletePermanent,
    Rename,
    NewFile,
    NewDir,
    CopyToOther,
    MoveToOther,
    /// Calcular el tamaño de la carpeta enfocada/seleccionada (fase sizing).
    ComputeSize,
}

impl Action {
    /// Todas las acciones, en orden de presentación para el editor.
    pub fn all() -> &'static [Action] {
        use Action::*;
        &[
            MoveUp,
            MoveDown,
            Activate,
            Open,
            OpenWith,
            GoUp,
            GoBack,
            GoForward,
            SwitchPane,
            CancelListing,
            Copy,
            Cut,
            Paste,
            Delete,
            DeletePermanent,
            Rename,
            NewFile,
            NewDir,
            CopyToOther,
            MoveToOther,
            ComputeSize,
        ]
    }

    /// Clave i18n del nombre legible de la acción.
    pub fn i18n_key(self) -> &'static str {
        use Action::*;
        match self {
            MoveUp => "action.move_up",
            MoveDown => "action.move_down",
            Activate => "action.activate",
            Open => "action.open",
            OpenWith => "action.open_with",
            GoUp => "action.go_up",
            GoBack => "action.go_back",
            GoForward => "action.go_forward",
            SwitchPane => "action.switch_pane",
            CancelListing => "action.cancel_listing",
            Copy => "action.copy",
            Cut => "action.cut",
            Paste => "action.paste",
            Delete => "action.delete",
            DeletePermanent => "action.delete_permanent",
            Rename => "action.rename",
            NewFile => "action.new_file",
            NewDir => "action.new_dir",
            CopyToOther => "action.copy_to_other",
            MoveToOther => "action.move_to_other",
            ComputeSize => "action.compute_size",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tiene_21_acciones_con_clave_i18n_unica() {
        let all = Action::all();
        assert_eq!(all.len(), 21);
        let mut keys: Vec<&str> = all.iter().map(|a| a.i18n_key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), 21, "cada acción tiene una clave i18n única");
    }

    #[test]
    fn chord_constructores() {
        assert_eq!(
            Chord::plain(KeyCode::F3),
            Chord {
                key: KeyCode::F3,
                ctrl: false,
                shift: false,
                alt: false
            }
        );
        assert!(Chord::ctrl(KeyCode::Char('c')).ctrl);
        assert!(Chord::ctrl_shift(KeyCode::Char('n')).shift);
    }
}
