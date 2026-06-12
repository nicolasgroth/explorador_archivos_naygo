// Naygo — mapeo de teclado a acciones de navegación (lógica pura testeable).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Traduce una pulsación de tecla (egui) a un `Chord`/`KeyCode` puro, y de ahí a una
//! `Action` vía el `KeyMap` configurable. El mapeo tecla→acción vive en el keymap
//! (no aquí); esta capa solo hace el puente egui ↔ tipos puros. Los botones laterales
//! del mouse siguen siendo fijos.

pub use naygo_core::keymap::Action;
use naygo_core::keymap::{Chord, KeyCode};

/// Traduce una `egui::Key` a nuestro `KeyCode`, si la soportamos.
pub fn egui_key_to_code(key: egui::Key) -> Option<KeyCode> {
    Some(match key {
        egui::Key::ArrowUp => KeyCode::ArrowUp,
        egui::Key::ArrowDown => KeyCode::ArrowDown,
        egui::Key::ArrowLeft => KeyCode::ArrowLeft,
        egui::Key::ArrowRight => KeyCode::ArrowRight,
        egui::Key::Enter => KeyCode::Enter,
        egui::Key::Backspace => KeyCode::Backspace,
        egui::Key::Tab => KeyCode::Tab,
        egui::Key::Escape => KeyCode::Escape,
        egui::Key::Delete => KeyCode::Delete,
        egui::Key::F2 => KeyCode::F2,
        egui::Key::F3 => KeyCode::F3,
        egui::Key::F4 => KeyCode::F4,
        egui::Key::F5 => KeyCode::F5,
        egui::Key::F6 => KeyCode::F6,
        egui::Key::PageDown => KeyCode::PageDown,
        egui::Key::PageUp => KeyCode::PageUp,
        egui::Key::Home => KeyCode::Home,
        egui::Key::End => KeyCode::End,
        egui::Key::Space => KeyCode::Space,
        // Dígitos (fila superior y numpad: egui ya los unifica en Num*). Los usan
        // los atajos de favoritos (Ctrl+1..9) y cualquier rebind del usuario.
        egui::Key::Num0 => KeyCode::Char('0'),
        egui::Key::Num1 => KeyCode::Char('1'),
        egui::Key::Num2 => KeyCode::Char('2'),
        egui::Key::Num3 => KeyCode::Char('3'),
        egui::Key::Num4 => KeyCode::Char('4'),
        egui::Key::Num5 => KeyCode::Char('5'),
        egui::Key::Num6 => KeyCode::Char('6'),
        egui::Key::Num7 => KeyCode::Char('7'),
        egui::Key::Num8 => KeyCode::Char('8'),
        egui::Key::Num9 => KeyCode::Char('9'),
        egui::Key::A => KeyCode::Char('a'),
        egui::Key::B => KeyCode::Char('b'),
        egui::Key::C => KeyCode::Char('c'),
        egui::Key::D => KeyCode::Char('d'),
        egui::Key::E => KeyCode::Char('e'),
        egui::Key::F => KeyCode::Char('f'),
        egui::Key::G => KeyCode::Char('g'),
        egui::Key::H => KeyCode::Char('h'),
        egui::Key::I => KeyCode::Char('i'),
        egui::Key::J => KeyCode::Char('j'),
        egui::Key::K => KeyCode::Char('k'),
        egui::Key::L => KeyCode::Char('l'),
        egui::Key::M => KeyCode::Char('m'),
        egui::Key::N => KeyCode::Char('n'),
        egui::Key::O => KeyCode::Char('o'),
        egui::Key::P => KeyCode::Char('p'),
        egui::Key::Q => KeyCode::Char('q'),
        egui::Key::R => KeyCode::Char('r'),
        egui::Key::S => KeyCode::Char('s'),
        egui::Key::T => KeyCode::Char('t'),
        egui::Key::U => KeyCode::Char('u'),
        egui::Key::V => KeyCode::Char('v'),
        egui::Key::W => KeyCode::Char('w'),
        egui::Key::X => KeyCode::Char('x'),
        egui::Key::Y => KeyCode::Char('y'),
        egui::Key::Z => KeyCode::Char('z'),
        _ => return None,
    })
}

/// Texto legible de un chord para el editor ("Ctrl+C", "F3", "↑").
pub fn chord_text(chord: &Chord) -> String {
    let mut s = String::new();
    if chord.ctrl {
        s.push_str("Ctrl+");
    }
    if chord.shift {
        s.push_str("Shift+");
    }
    if chord.alt {
        s.push_str("Alt+");
    }
    let k = match chord.key {
        KeyCode::ArrowUp => "↑",
        KeyCode::ArrowDown => "↓",
        KeyCode::ArrowLeft => "←",
        KeyCode::ArrowRight => "→",
        KeyCode::Enter => "Enter",
        KeyCode::Backspace => "Backspace",
        KeyCode::Tab => "Tab",
        KeyCode::Escape => "Esc",
        KeyCode::Delete => "Supr",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::PageDown => "AvPág",
        KeyCode::PageUp => "RePág",
        KeyCode::Home => "Inicio",
        KeyCode::End => "Fin",
        KeyCode::Space => "␣",
        KeyCode::Char(c) => {
            s.push(c.to_ascii_uppercase());
            return s;
        }
    };
    s.push_str(k);
    s
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
    fn botones_laterales_del_mouse_navegan() {
        assert_eq!(map_mouse_extra(MouseExtra::Back), Action::GoBack);
        assert_eq!(map_mouse_extra(MouseExtra::Forward), Action::GoForward);
    }

    #[test]
    fn egui_key_a_keycode_letras_y_especiales() {
        assert_eq!(egui_key_to_code(egui::Key::C), Some(KeyCode::Char('c')));
        assert_eq!(egui_key_to_code(egui::Key::N), Some(KeyCode::Char('n')));
        assert_eq!(
            egui_key_to_code(egui::Key::ArrowLeft),
            Some(KeyCode::ArrowLeft)
        );
        assert_eq!(egui_key_to_code(egui::Key::F2), Some(KeyCode::F2));
        assert_eq!(egui_key_to_code(egui::Key::Delete), Some(KeyCode::Delete));
        assert_eq!(egui_key_to_code(egui::Key::Space), Some(KeyCode::Space));
        // Dígitos: los usan los atajos de favoritos (Ctrl+1..9).
        assert_eq!(egui_key_to_code(egui::Key::Num0), Some(KeyCode::Char('0')));
        assert_eq!(egui_key_to_code(egui::Key::Num9), Some(KeyCode::Char('9')));
        assert_eq!(egui_key_to_code(egui::Key::F4), Some(KeyCode::F4));
        // Teclas de navegación por bloques (teclado de lista).
        assert_eq!(
            egui_key_to_code(egui::Key::PageDown),
            Some(KeyCode::PageDown)
        );
        assert_eq!(egui_key_to_code(egui::Key::PageUp), Some(KeyCode::PageUp));
        assert_eq!(egui_key_to_code(egui::Key::Home), Some(KeyCode::Home));
        assert_eq!(egui_key_to_code(egui::Key::End), Some(KeyCode::End));
        // Una tecla que no mapeamos.
        assert_eq!(egui_key_to_code(egui::Key::F12), None);
    }

    #[test]
    fn chord_text_legible() {
        assert_eq!(chord_text(&Chord::ctrl(KeyCode::Char('c'))), "Ctrl+C");
        assert_eq!(
            chord_text(&Chord::ctrl_shift(KeyCode::Char('n'))),
            "Ctrl+Shift+N"
        );
        assert_eq!(chord_text(&Chord::plain(KeyCode::F3)), "F3");
        assert_eq!(chord_text(&Chord::plain(KeyCode::ArrowUp)), "↑");
        assert_eq!(chord_text(&Chord::alt(KeyCode::ArrowLeft)), "Alt+←");
        assert_eq!(chord_text(&Chord::plain(KeyCode::Space)), "␣");
    }
}
