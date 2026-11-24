// Naygo — validación léxica de rutas editadas; sin I/O ni dependencias de plataforma.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathInputError {
    Empty,
    Invalid,
    TooLong,
}

/// Aplica reglas de entrada propias del SO sin reinterpretar silenciosamente rutas.
/// Windows retira comillas exteriores de pegado; POSIX conserva espacios, comillas y
/// backslash literalmente. La resolución física pertenece al worker.
pub fn normalize(text: &str) -> Result<String, PathInputError> {
    #[cfg(windows)]
    {
        normalize_windows(text)
    }
    #[cfg(unix)]
    {
        normalize_unix(text)
    }
    #[cfg(not(any(windows, unix)))]
    {
        normalize_portable(text)
    }
}

/// Valida entrada de ruta para Windows sin normalizar separadores, UNC ni prefijos extendidos.
#[cfg(windows)]
fn normalize_windows(text: &str) -> Result<String, PathInputError> {
    let text = text.trim();
    let text = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
        &text[1..text.len() - 1]
    } else {
        text
    };
    if text.is_empty() {
        return Err(PathInputError::Empty);
    }
    if text.chars().any(char::is_control) || text.contains('"') {
        return Err(PathInputError::Invalid);
    }
    if text.encode_utf16().count() > 32767 {
        return Err(PathInputError::TooLong);
    }
    Ok(text.to_owned())
}

#[cfg(unix)]
fn normalize_unix(text: &str) -> Result<String, PathInputError> {
    // En POSIX, espacios, comillas y backslash pueden ser parte literal del nombre.
    // No se usa trim ni se interpretan comillas como sintaxis de pegado: hacerlo
    // convertiría una ruta válida en otra distinta antes de que el worker la resuelva.
    if text.is_empty() {
        return Err(PathInputError::Empty);
    }
    if text.chars().any(char::is_control) {
        return Err(PathInputError::Invalid);
    }
    // PATH_MAX depende del sistema de archivos y se mide en bytes del path nativo;
    // un límite UTF-16 de Windows aquí descartaría nombres POSIX válidos.
    Ok(text.to_owned())
}

#[cfg(not(any(windows, unix)))]
fn normalize_portable(text: &str) -> Result<String, PathInputError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(PathInputError::Empty);
    }
    if text.chars().any(char::is_control) {
        return Err(PathInputError::Invalid);
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn preserves_quoted_unicode_unc_and_extended_paths() {
        for path in [
            r"\\servidor\área común\Nicolás",
            r"\\?\C:\datos\",
            r"..\otra carpeta",
        ] {
            assert_eq!(normalize(&format!("  \"{path}\" ")).unwrap(), path);
        }
        assert_eq!(normalize("\" carpeta \"").unwrap(), " carpeta ");
    }

    #[cfg(windows)]
    #[test]
    fn rejects_empty_control_unmatched_quotes_and_extreme_length() {
        for text in ["", " ", "\"\"", "x\0y", "x\ny", "\"C:\\x", "x\ty"] {
            assert!(normalize(text).is_err());
        }
        assert_eq!(normalize(&"a".repeat(32768)), Err(PathInputError::TooLong));
        assert!(normalize(&"a".repeat(32767)).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn preserves_posix_whitespace_quotes_and_backslashes_literally() {
        for path in [
            " leading",
            "trailing ",
            " both ",
            "\"quoted\"",
            r"back\slash",
        ] {
            assert_eq!(normalize(path).unwrap(), path);
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_empty_or_control_text_without_windows_length_limit() {
        assert_eq!(normalize(""), Err(PathInputError::Empty));
        assert_eq!(normalize("\t"), Err(PathInputError::Invalid));
        assert!(normalize(&"a".repeat(32768)).is_ok());
    }

    #[cfg(not(any(windows, unix)))]
    #[test]
    fn portable_fallback_rejects_empty_and_control_text() {
        assert_eq!(normalize(""), Err(PathInputError::Empty));
        assert_eq!(normalize("x\ny"), Err(PathInputError::Invalid));
    }
}
