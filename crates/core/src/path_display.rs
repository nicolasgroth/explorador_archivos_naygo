// Naygo — representación visible y no ambigua de componentes nativos de ruta.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Convierte un componente `OsStr` en texto para filas y consultas de búsqueda.
//! En Unix escapa separadores de la notación y bytes que no sean UTF-8 para que
//! dos nombres nativos distintos no colapsen a U+FFFD. El resultado es sólo de
//! presentación: nunca reconstruir una ruta operativa desde esta cadena.

use std::ffi::OsStr;

/// Presenta un componente de ruta sin perder su identidad visible.
pub fn display_os_str(value: &OsStr) -> String {
    #[cfg(unix)]
    {
        use std::fmt::Write as _;
        use std::os::unix::ffi::OsStrExt;

        fn append_valid(output: &mut String, text: &str) {
            for character in text.chars() {
                if character == '\\' {
                    output.push_str("\\\\");
                } else if character.is_control() {
                    output.extend(character.escape_default());
                } else {
                    output.push(character);
                }
            }
        }

        let mut output = String::new();
        let mut remaining = value.as_bytes();
        while !remaining.is_empty() {
            match std::str::from_utf8(remaining) {
                Ok(text) => {
                    append_valid(&mut output, text);
                    break;
                }
                Err(error) => {
                    let valid_len = error.valid_up_to();
                    if valid_len > 0 {
                        let valid = std::str::from_utf8(&remaining[..valid_len])
                            .expect("valid_up_to must be a UTF-8 boundary");
                        append_valid(&mut output, valid);
                    }
                    let invalid_len = error.error_len().unwrap_or(remaining.len() - valid_len);
                    for byte in &remaining[valid_len..valid_len + invalid_len] {
                        let _ = write!(output, "\\x{byte:02X}");
                    }
                    remaining = &remaining[valid_len + invalid_len..];
                }
            }
        }
        output
    }
    #[cfg(not(unix))]
    {
        value.to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::*;

    #[cfg(unix)]
    #[test]
    fn posix_display_preserves_unicode_and_escapes_bytes_and_backslashes_distinctly() {
        use std::os::unix::ffi::OsStrExt;

        assert_eq!(display_os_str(OsStr::new("foto ñ.jpg")), "foto ñ.jpg");
        assert_eq!(display_os_str(OsStr::new(r"foto\b.jpg")), r"foto\\b.jpg");
        assert_eq!(
            display_os_str(OsStr::from_bytes(b"foto-\xff.jpg")),
            r"foto-\xFF.jpg"
        );
        assert_eq!(
            display_os_str(OsStr::from_bytes(b"foto-\\xFF.jpg")),
            r"foto-\\xFF.jpg"
        );
        assert_ne!(
            display_os_str(OsStr::from_bytes(b"foto-\xff.jpg")),
            display_os_str(OsStr::from_bytes(b"foto-\\xFF.jpg"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn posix_display_escapes_control_characters() {
        use std::os::unix::ffi::OsStrExt;

        assert_eq!(
            display_os_str(OsStr::from_bytes(b"line\nfeed")),
            r"line\nfeed"
        );
    }
}
