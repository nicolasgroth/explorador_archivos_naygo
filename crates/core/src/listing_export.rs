// Naygo — exportación pura de listados visibles a CSV o texto tabulado.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

/// Serializa encabezados y filas como CSV RFC-4180 adaptado al separador elegido. Siempre usa
/// CRLF para interoperar bien con Excel en Windows. Las filas cortas se completan con vacío.
pub fn to_csv(headers: &[String], rows: &[Vec<String>], separator: char) -> String {
    let separator = if separator == ',' { ',' } else { ';' };
    let mut out = String::new();
    push_row(&mut out, headers.iter().map(String::as_str), separator);
    for row in rows {
        push_row(&mut out, row.iter().map(String::as_str), separator);
    }
    out
}

fn push_row<'a>(out: &mut String, fields: impl Iterator<Item = &'a str>, separator: char) {
    for (index, field) in fields.enumerate() {
        if index > 0 {
            out.push(separator);
        }
        let quote = field.contains(separator)
            || field.contains('"')
            || field.contains('\r')
            || field.contains('\n');
        if quote {
            out.push('"');
            out.push_str(&field.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(field);
        }
    }
    out.push_str("\r\n");
}

/// Bytes listos para archivo: BOM UTF-8 para que Excel detecte correctamente tildes y alfabetos
/// no latinos, seguido del CSV.
pub fn csv_file_bytes(headers: &[String], rows: &[Vec<String>], separator: char) -> Vec<u8> {
    let csv = to_csv(headers, rows, separator);
    let mut bytes = Vec::with_capacity(csv.len() + 3);
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(csv.as_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escapa_separador_comillas_y_saltos() {
        let headers = vec!["Nombre".into(), "Ruta".into()];
        let rows = vec![vec!["uno;dos".into(), "C:\\a\"b\r\nx".into()]];
        assert_eq!(
            to_csv(&headers, &rows, ';'),
            "Nombre;Ruta\r\n\"uno;dos\";\"C:\\a\"\"b\r\nx\"\r\n"
        );
    }

    #[test]
    fn archivo_lleva_bom_utf8() {
        let bytes = csv_file_bytes(&["Nombre".into()], &[vec!["Ñandú".into()]], ';');
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert_eq!(
            std::str::from_utf8(&bytes[3..]).unwrap(),
            "Nombre\r\nÑandú\r\n"
        );
    }
}
