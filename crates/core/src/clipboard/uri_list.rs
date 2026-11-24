// Naygo — codificación portable de referencias de archivo text/uri-list.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Codec puro para la parte local de `text/uri-list` usada por escritorios Linux.
//!
//! Solo transforma referencias `file:` en bytes de ruta absoluta. No abre rutas, ejecuta URIs,
//! accede al portapapeles ni decide operaciones. La integración con Nautilus/Dolphin y el
//! transporte X11/Wayland permanecen en `platform` y deben ejecutarse fuera del trabajo de disco.

/// Causa de rechazo de una entrada URI no confiable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UriListErrorKind {
    InputTooLarge,
    TooManyEntries,
    PathTooLong,
    UnsupportedScheme,
    NonLocalAuthority,
    MissingPath,
    NonAbsolutePath,
    InvalidEscape,
    EncodedSeparator,
    NulByte,
    ControlByte,
    QueryOrFragment,
    UnsupportedAction,
}

/// El clipboard es entrada externa: acotar su volumen antes de reservar/decodificar.
pub const MAX_FILE_URI_LIST_BYTES: usize = 16 * 1024 * 1024;
/// Coincide con el orden de magnitud que Naygo prueba para listas grandes, sin permitir listas ilimitadas.
pub const MAX_FILE_URI_ENTRIES: usize = 100_000;
/// Rutas POSIX anormalmente largas no deben inflar filas/modelos a partir del clipboard.
pub const MAX_FILE_URI_PATH_BYTES: usize = 64 * 1024;
const MAX_URI_ENTRY_BYTES: usize = 7 + MAX_FILE_URI_PATH_BYTES * 3;

/// Error con número de línea 1-based. Al codificar, `line` identifica el índice de ruta + 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UriListError {
    pub line: usize,
    pub kind: UriListErrorKind,
}

impl std::fmt::Display for UriListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "entrada {} de text/uri-list: {:?}", self.line, self.kind)
    }
}

impl std::error::Error for UriListError {}

/// Decodifica una lista de URIs a bytes de rutas locales absolutas.
///
/// Se conservan bytes POSIX no UTF-8 representados como `%HH`. Se aceptan únicamente el esquema
/// `file:` con autoridad vacía o `localhost`; se ignoran líneas vacías y comentarios de RFC 2483.
/// URI remotas, esquemas ajenos, escapes incompletos, NUL y `/` percent-encoded se rechazan para
/// evitar que una entrada ambigua se convierta silenciosamente en otra ruta.
pub fn decode_file_uri_list(input: &[u8]) -> Result<Vec<Vec<u8>>, UriListError> {
    if input.len() > MAX_FILE_URI_LIST_BYTES {
        return Err(error(1, UriListErrorKind::InputTooLarge));
    }
    let mut paths = Vec::new();
    for (index, raw_line) in input.split(|byte| *byte == b'\n').enumerate() {
        let line_number = index + 1;
        let line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        if paths.len() == MAX_FILE_URI_ENTRIES {
            return Err(error(line_number, UriListErrorKind::TooManyEntries));
        }
        if line.len() > MAX_URI_ENTRY_BYTES {
            return Err(error(line_number, UriListErrorKind::PathTooLong));
        }
        let path = decode_file_uri(line, line_number)?;
        if path.len() > MAX_FILE_URI_PATH_BYTES {
            return Err(error(line_number, UriListErrorKind::PathTooLong));
        }
        paths.push(path);
    }
    Ok(paths)
}

/// Codifica bytes de rutas absolutas locales como `text/uri-list` con terminadores CRLF.
///
/// Todo byte que no sea ASCII alfanumérico, `-._~` o `/` se escapa como `%HH`, por lo que espacios,
/// Unicode UTF-8 y nombres POSIX no UTF-8 se preservan sin interpretar como delimitadores URI.
pub fn encode_file_uri_list(paths: &[&[u8]]) -> Result<Vec<u8>, UriListError> {
    if paths.len() > MAX_FILE_URI_ENTRIES {
        return Err(error(
            MAX_FILE_URI_ENTRIES + 1,
            UriListErrorKind::TooManyEntries,
        ));
    }
    let mut output = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let line = index + 1;
        if !path.starts_with(b"/") {
            return Err(UriListError {
                line,
                kind: UriListErrorKind::NonAbsolutePath,
            });
        }
        if path.contains(&0) {
            return Err(UriListError {
                line,
                kind: UriListErrorKind::NulByte,
            });
        }
        if path.len() > MAX_FILE_URI_PATH_BYTES {
            return Err(error(line, UriListErrorKind::PathTooLong));
        }

        let encoded_path_len = path.iter().try_fold(0usize, |length, byte| {
            length.checked_add(if is_unreserved(*byte) || *byte == b'/' {
                1
            } else {
                3
            })
        });
        let Some(encoded_path_len) = encoded_path_len else {
            return Err(error(line, UriListErrorKind::InputTooLarge));
        };
        let Some(next_len) = output.len().checked_add(7 + encoded_path_len + 2) else {
            return Err(error(line, UriListErrorKind::InputTooLarge));
        };
        if next_len > MAX_FILE_URI_LIST_BYTES {
            return Err(error(line, UriListErrorKind::InputTooLarge));
        }

        output.extend_from_slice(b"file://");
        for &byte in *path {
            if is_unreserved(byte) || byte == b'/' {
                output.push(byte);
            } else {
                output.push(b'%');
                output.push(hex_digit(byte >> 4));
                output.push(hex_digit(byte & 0x0f));
            }
        }
        output.extend_from_slice(b"\r\n");
    }
    Ok(output)
}

fn decode_file_uri(uri: &[u8], line: usize) -> Result<Vec<u8>, UriListError> {
    let Some(colon) = uri.iter().position(|byte| *byte == b':') else {
        return Err(error(line, UriListErrorKind::UnsupportedScheme));
    };
    if !uri[..colon].eq_ignore_ascii_case(b"file") {
        return Err(error(line, UriListErrorKind::UnsupportedScheme));
    }

    let remainder = &uri[colon + 1..];
    let path = if remainder.starts_with(b"//") {
        let authority_and_path = &remainder[2..];
        let Some(slash) = authority_and_path.iter().position(|byte| *byte == b'/') else {
            return Err(error(line, UriListErrorKind::MissingPath));
        };
        let authority = &authority_and_path[..slash];
        if !authority.is_empty() && !authority.eq_ignore_ascii_case(b"localhost") {
            return Err(error(line, UriListErrorKind::NonLocalAuthority));
        }
        &authority_and_path[slash..]
    } else {
        remainder
    };

    if path.is_empty() {
        return Err(error(line, UriListErrorKind::MissingPath));
    }
    if !path.starts_with(b"/") {
        return Err(error(line, UriListErrorKind::NonAbsolutePath));
    }
    if path.iter().any(|byte| matches!(*byte, b'?' | b'#')) {
        return Err(error(line, UriListErrorKind::QueryOrFragment));
    }

    let mut decoded = Vec::with_capacity(path.len());
    let mut index = 0;
    while index < path.len() {
        let byte = path[index];
        match byte {
            b'%' => {
                if index + 2 >= path.len() {
                    return Err(error(line, UriListErrorKind::InvalidEscape));
                }
                let Some(high) = from_hex(path[index + 1]) else {
                    return Err(error(line, UriListErrorKind::InvalidEscape));
                };
                let Some(low) = from_hex(path[index + 2]) else {
                    return Err(error(line, UriListErrorKind::InvalidEscape));
                };
                let decoded_byte = (high << 4) | low;
                if decoded_byte == b'/' {
                    return Err(error(line, UriListErrorKind::EncodedSeparator));
                }
                if decoded_byte == 0 {
                    return Err(error(line, UriListErrorKind::NulByte));
                }
                decoded.push(decoded_byte);
                index += 3;
            }
            0 => return Err(error(line, UriListErrorKind::NulByte)),
            1..=0x1f | 0x7f => return Err(error(line, UriListErrorKind::ControlByte)),
            _ => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    Ok(decoded)
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

fn hex_digit(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        10..=15 => b'A' + nibble - 10,
        _ => unreachable!("nibble is limited to four bits"),
    }
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn error(line: usize, kind: UriListErrorKind) -> UriListError {
    UriListError { line, kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_spaces_unicode_and_uri_delimiters() {
        let paths: [&[u8]; 2] = [
            b"/home/nico/mi carpeta/#foto?.jpg",
            "/home/nico/niño 100%/x.txt".as_bytes(),
        ];
        let encoded = encode_file_uri_list(&paths).unwrap();
        assert_eq!(
            encoded,
            b"file:///home/nico/mi%20carpeta/%23foto%3F.jpg\r\nfile:///home/nico/ni%C3%B1o%20100%25/x.txt\r\n"
        );
        assert_eq!(decode_file_uri_list(&encoded).unwrap(), paths);
    }

    #[test]
    fn preserves_non_utf8_posix_filename_bytes() {
        let path = b"/tmp/name-\xff";
        let encoded = encode_file_uri_list(&[path]).unwrap();
        assert_eq!(encoded, b"file:///tmp/name-%FF\r\n");
        assert_eq!(decode_file_uri_list(&encoded).unwrap(), [path.to_vec()]);
    }

    #[test]
    fn accepts_localhost_authority_and_comments() {
        assert_eq!(
            decode_file_uri_list(b"# comment\r\nFiLe://LOCALHOST/tmp/a%20b\r\n\r\n").unwrap(),
            [b"/tmp/a b".to_vec()]
        );
    }

    #[test]
    fn accepts_uri_without_authority() {
        assert_eq!(
            decode_file_uri_list(b"file:/tmp/a\n").unwrap(),
            [b"/tmp/a".to_vec()]
        );
    }

    #[test]
    fn rejects_non_file_scheme_and_remote_authority() {
        assert_eq!(
            decode_file_uri_list(b"https://example.invalid/file"),
            Err(error(1, UriListErrorKind::UnsupportedScheme))
        );
        assert_eq!(
            decode_file_uri_list(b"file://server/share/a"),
            Err(error(1, UriListErrorKind::NonLocalAuthority))
        );
    }

    #[test]
    fn rejects_invalid_escapes_and_encoded_separator() {
        for input in [
            b"file:///tmp/%".as_slice(),
            b"file:///tmp/%XZ",
            b"file:///tmp/%2",
        ] {
            assert_eq!(
                decode_file_uri_list(input),
                Err(error(1, UriListErrorKind::InvalidEscape))
            );
        }
        assert_eq!(
            decode_file_uri_list(b"file:///tmp/a%2fb"),
            Err(error(1, UriListErrorKind::EncodedSeparator))
        );
    }

    #[test]
    fn rejects_nul_controls_query_and_fragment() {
        assert_eq!(
            decode_file_uri_list(b"file:///tmp/a%00b"),
            Err(error(1, UriListErrorKind::NulByte))
        );
        assert_eq!(
            decode_file_uri_list(b"file:///tmp/a\tb"),
            Err(error(1, UriListErrorKind::ControlByte))
        );
        assert_eq!(
            decode_file_uri_list(b"file:///tmp/a?b"),
            Err(error(1, UriListErrorKind::QueryOrFragment))
        );
        assert_eq!(
            decode_file_uri_list(b"file:///tmp/a#b"),
            Err(error(1, UriListErrorKind::QueryOrFragment))
        );
    }

    #[test]
    fn requires_absolute_paths_when_encoding() {
        assert_eq!(
            encode_file_uri_list(&[b"relative".as_slice()]),
            Err(error(1, UriListErrorKind::NonAbsolutePath))
        );
        assert_eq!(
            encode_file_uri_list(&[b"/tmp/nul\0path".as_slice()]),
            Err(error(1, UriListErrorKind::NulByte))
        );
    }

    #[test]
    fn reports_one_based_line_of_invalid_entry() {
        assert_eq!(
            decode_file_uri_list(b"file:///tmp/ok\r\nhttps://example.invalid/\r\n"),
            Err(error(2, UriListErrorKind::UnsupportedScheme))
        );
    }

    #[test]
    fn rejects_clipboard_payload_over_size_limit() {
        let input = vec![b'x'; MAX_FILE_URI_LIST_BYTES + 1];
        assert_eq!(
            decode_file_uri_list(&input),
            Err(error(1, UriListErrorKind::InputTooLarge))
        );
    }

    #[test]
    fn rejects_too_many_paths_and_oversized_single_path() {
        let too_many = vec![b"/x".as_slice(); MAX_FILE_URI_ENTRIES + 1];
        assert_eq!(
            encode_file_uri_list(&too_many),
            Err(error(
                MAX_FILE_URI_ENTRIES + 1,
                UriListErrorKind::TooManyEntries
            ))
        );
        let long_path = vec![b'a'; MAX_FILE_URI_PATH_BYTES + 1];
        let mut absolute = vec![b'/'];
        absolute.extend(long_path);
        assert_eq!(
            encode_file_uri_list(&[&absolute]),
            Err(error(1, UriListErrorKind::PathTooLong))
        );

        let too_many_uris = b"file:///x\n".repeat(MAX_FILE_URI_ENTRIES + 1);
        assert_eq!(
            decode_file_uri_list(&too_many_uris),
            Err(error(
                MAX_FILE_URI_ENTRIES + 1,
                UriListErrorKind::TooManyEntries
            ))
        );

        let long_uri = format!("file:///{}", "a".repeat(MAX_FILE_URI_PATH_BYTES + 1));
        assert_eq!(
            decode_file_uri_list(long_uri.as_bytes()),
            Err(error(1, UriListErrorKind::PathTooLong))
        );
    }
}
