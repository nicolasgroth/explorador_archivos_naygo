// Naygo — mapeo de teclas de Slint a Chord del keymap del core (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use naygo_core::keymap::{Chord, KeyCode};

/// Convierte la tecla recibida de Slint (su texto unicode + modificadores) en un `Chord`
/// del keymap del core. Las teclas especiales de Slint (flechas, Enter, etc.) llegan como
/// chars de `slint::platform::Key`; las normales como su carácter. Devuelve `None` si la
/// tecla no la modela el keymap (se ignora).
pub fn chord_from(text: &str, ctrl: bool, shift: bool, alt: bool) -> Option<Chord> {
    let key = keycode_from(text)?;
    Some(Chord {
        key,
        ctrl,
        shift,
        alt,
    })
}

/// El char unicode de una tecla especial de Slint (su representación interna).
fn special(k: slint::platform::Key) -> char {
    let s: slint::SharedString = k.into();
    s.chars().next().unwrap_or('\0')
}

/// El char que Slint usa para la tecla Escape (para detectarla sin pasar por el keymap,
/// p. ej. cancelar el selector de panel).
pub fn escape_char() -> char {
    special(slint::platform::Key::Escape)
}

/// Mapea el texto de la tecla a un `KeyCode`. Compara contra los chars de las teclas
/// especiales de Slint y, si no, toma el primer carácter como letra/dígito.
fn keycode_from(text: &str) -> Option<KeyCode> {
    use slint::platform::Key;
    let first = text.chars().next()?;
    let kc = if first == special(Key::UpArrow) {
        KeyCode::ArrowUp
    } else if first == special(Key::DownArrow) {
        KeyCode::ArrowDown
    } else if first == special(Key::LeftArrow) {
        KeyCode::ArrowLeft
    } else if first == special(Key::RightArrow) {
        KeyCode::ArrowRight
    } else if first == special(Key::Return) {
        KeyCode::Enter
    } else if first == special(Key::Backspace) {
        KeyCode::Backspace
    } else if first == special(Key::Tab) {
        KeyCode::Tab
    } else if first == special(Key::Escape) {
        KeyCode::Escape
    } else if first == special(Key::Delete) {
        KeyCode::Delete
    } else if first == special(Key::PageUp) {
        KeyCode::PageUp
    } else if first == special(Key::PageDown) {
        KeyCode::PageDown
    } else if first == special(Key::Home) {
        KeyCode::Home
    } else if first == special(Key::End) {
        KeyCode::End
    } else if first == special(Key::F2) {
        KeyCode::F2
    } else if first == special(Key::F3) {
        KeyCode::F3
    } else if first == special(Key::F4) {
        KeyCode::F4
    } else if first == special(Key::F5) {
        KeyCode::F5
    } else if first == special(Key::F6) {
        KeyCode::F6
    } else if first == special(Key::Space) || first == ' ' {
        KeyCode::Space
    } else if first.is_alphanumeric() {
        // Letra o dígito: normalizar a minúscula (el keymap usa Char minúscula).
        KeyCode::Char(first.to_ascii_lowercase())
    } else {
        return None;
    };
    Some(kc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::keymap::{Chord, KeyCode};

    /// Helper: el char unicode de una tecla especial de Slint, como String.
    fn key_char(k: slint::platform::Key) -> String {
        let s: slint::SharedString = k.into();
        s.to_string()
    }

    #[test]
    fn flechas_y_especiales() {
        assert_eq!(
            chord_from(
                &key_char(slint::platform::Key::UpArrow),
                false,
                false,
                false
            ),
            Some(Chord::plain(KeyCode::ArrowUp))
        );
        assert_eq!(
            chord_from(&key_char(slint::platform::Key::Return), false, false, false),
            Some(Chord::plain(KeyCode::Enter))
        );
        assert_eq!(
            chord_from(
                &key_char(slint::platform::Key::Backspace),
                false,
                false,
                false
            ),
            Some(Chord::plain(KeyCode::Backspace))
        );
        assert_eq!(
            chord_from(&key_char(slint::platform::Key::Home), false, false, false),
            Some(Chord::plain(KeyCode::Home))
        );
        // Teclas de función (F2 = rename, etc.): antes no se mapeaban, así que F2 nunca
        // disparaba el rename en la UI Slint. Ahora sí. (6D)
        assert_eq!(
            chord_from(&key_char(slint::platform::Key::F2), false, false, false),
            Some(Chord::plain(KeyCode::F2))
        );
        assert_eq!(
            chord_from(&key_char(slint::platform::Key::F5), false, false, false),
            Some(Chord::plain(KeyCode::F5))
        );
    }

    #[test]
    fn letras_y_modificadores() {
        assert_eq!(
            chord_from("c", true, false, false),
            Some(Chord::ctrl(KeyCode::Char('c')))
        );
        assert_eq!(
            chord_from("A", false, false, false),
            Some(Chord::plain(KeyCode::Char('a')))
        );
        let c = chord_from(
            &key_char(slint::platform::Key::DownArrow),
            false,
            true,
            false,
        )
        .unwrap();
        assert!(c.shift && c.key == KeyCode::ArrowDown);
    }

    #[test]
    fn texto_vacio_o_desconocido_es_none() {
        assert_eq!(chord_from("", false, false, false), None);
    }
}
