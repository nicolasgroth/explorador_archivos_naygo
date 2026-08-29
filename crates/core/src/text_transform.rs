// Naygo — diagnóstico y transformación segura de archivos de texto.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Conversión pura de bytes más reemplazo transaccional en disco. El trabajo de archivo debe
//! ejecutarse en un worker. Nunca publica un resultado parcial.

use crate::CancellationToken;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Defensive ceiling: normalization temporarily needs both decoded and encoded buffers.
/// Keeping the input below 128 MiB prevents multi-gigabyte transient allocations.
pub const MAX_TEXT_TRANSFORM_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetEncoding {
    Preserve,
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Crlf,
    Lf,
    Cr,
    Mixed,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalNewline {
    Preserve,
    Ensure,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransformOptions {
    pub line_ending: LineEnding,
    pub encoding: TargetEncoding,
    pub final_newline: FinalNewline,
    pub trim_trailing_whitespace: bool,
}

impl Default for TransformOptions {
    fn default() -> Self {
        Self {
            line_ending: LineEnding::Lf,
            encoding: TargetEncoding::Preserve,
            final_newline: FinalNewline::Preserve,
            trim_trailing_whitespace: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDiagnosis {
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
    pub crlf_count: usize,
    pub lf_count: usize,
    pub cr_count: usize,
    pub has_final_newline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformOutcome {
    pub diagnosis: TextDiagnosis,
    pub target_encoding: TextEncoding,
    pub bytes_written: u64,
    /// Respaldo del original para integrar con el historial/deshacer. El llamador decide cuándo
    /// eliminarlo.
    pub backup_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformError {
    Unreadable,
    InvalidText,
    InvalidTarget,
    TooLarge,
    Cancelled,
    WriteFailed,
}

pub fn diagnose_bytes(bytes: &[u8]) -> Result<TextDiagnosis, TransformError> {
    let (text, encoding) = decode(bytes)?;
    let (crlf_count, lf_count, cr_count) = count_endings(&text);
    let kinds = usize::from(crlf_count > 0) + usize::from(lf_count > 0) + usize::from(cr_count > 0);
    let line_ending = match kinds {
        0 => LineEnding::None,
        1 if crlf_count > 0 => LineEnding::Crlf,
        1 if lf_count > 0 => LineEnding::Lf,
        1 => LineEnding::Cr,
        _ => LineEnding::Mixed,
    };
    Ok(TextDiagnosis {
        encoding,
        line_ending,
        crlf_count,
        lf_count,
        cr_count,
        has_final_newline: text.ends_with('\n') || text.ends_with('\r'),
    })
}

pub fn transform_bytes(bytes: &[u8], options: TransformOptions) -> Result<Vec<u8>, TransformError> {
    if matches!(options.line_ending, LineEnding::Mixed | LineEnding::None) {
        return Err(TransformError::InvalidTarget);
    }
    let (text, source_encoding) = decode(bytes)?;
    let had_final = text.ends_with('\n') || text.ends_with('\r');
    let newline = match options.line_ending {
        LineEnding::Crlf => "\r\n",
        LineEnding::Lf => "\n",
        LineEnding::Cr => "\r",
        LineEnding::Mixed | LineEnding::None => return Err(TransformError::InvalidTarget),
    };
    let mut lines = split_lines(&text);
    if options.trim_trailing_whitespace {
        for line in &mut lines {
            *line = line.trim_end_matches([' ', '\t']).to_string();
        }
    }
    let mut normalized = lines.join(newline);
    let wants_final = match options.final_newline {
        FinalNewline::Preserve => had_final,
        FinalNewline::Ensure => true,
        FinalNewline::Remove => false,
    };
    if wants_final && !normalized.is_empty() {
        normalized.push_str(newline);
    }
    let target = match options.encoding {
        TargetEncoding::Preserve => source_encoding,
        TargetEncoding::Utf8 => TextEncoding::Utf8,
        TargetEncoding::Utf8Bom => TextEncoding::Utf8Bom,
        TargetEncoding::Utf16Le => TextEncoding::Utf16Le,
        TargetEncoding::Utf16Be => TextEncoding::Utf16Be,
    };
    Ok(encode(&normalized, target))
}

/// Transforma `path` de manera transaccional. Lee/escribe por bloques y consulta cancelación;
/// la representación Unicode se mantiene en memoria con un límite duro para evitar presión sin
/// control. El original se conserva como respaldo vecino.
pub fn transform_file(
    path: &Path,
    options: TransformOptions,
    token: &CancellationToken,
) -> Result<TransformOutcome, TransformError> {
    let meta = fs::metadata(path).map_err(|_| TransformError::Unreadable)?;
    if !meta.is_file() {
        return Err(TransformError::Unreadable);
    }
    if meta.len() > MAX_TEXT_TRANSFORM_BYTES {
        return Err(TransformError::TooLarge);
    }
    let mut file = File::open(path).map_err(|_| TransformError::Unreadable)?;
    let mut bytes = Vec::with_capacity(meta.len().min(8 * 1024 * 1024) as usize);
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        if token.is_cancelled() {
            return Err(TransformError::Cancelled);
        }
        let count = file
            .read(&mut chunk)
            .map_err(|_| TransformError::Unreadable)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    let diagnosis = diagnose_bytes(&bytes)?;
    let transformed = transform_bytes(&bytes, options)?;
    let target_encoding = detect_encoding(&transformed);
    if token.is_cancelled() {
        return Err(TransformError::Cancelled);
    }

    let parent = path.parent().ok_or(TransformError::WriteFailed)?;
    let name = path
        .file_name()
        .ok_or(TransformError::WriteFailed)?
        .to_string_lossy();
    let nonce = format!("{}-{}", std::process::id(), unique_tick());
    let temp = parent.join(format!(".{name}.naygo-{nonce}.tmp"));
    let backup = parent.join(format!(".{name}.naygo-{nonce}.bak"));
    let write_result = (|| {
        let mut out = File::create(&temp).map_err(|_| TransformError::WriteFailed)?;
        for part in transformed.chunks(64 * 1024) {
            if token.is_cancelled() {
                return Err(TransformError::Cancelled);
            }
            out.write_all(part)
                .map_err(|_| TransformError::WriteFailed)?;
        }
        out.sync_all().map_err(|_| TransformError::WriteFailed)?;
        fs::rename(path, &backup).map_err(|_| TransformError::WriteFailed)?;
        if fs::rename(&temp, path).is_err() {
            let _ = fs::rename(&backup, path);
            return Err(TransformError::WriteFailed);
        }
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    write_result?;
    Ok(TransformOutcome {
        diagnosis,
        target_encoding,
        bytes_written: transformed.len() as u64,
        backup_path: backup,
    })
}

fn decode(bytes: &[u8]) -> Result<(String, TextEncoding), TransformError> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes[3..].to_vec())
            .map(|text| (text, TextEncoding::Utf8Bom))
            .map_err(|_| TransformError::InvalidText);
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true).map(|text| (text, TextEncoding::Utf16Le));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false).map(|text| (text, TextEncoding::Utf16Be));
    }
    if bytes.contains(&0) {
        return Err(TransformError::InvalidText);
    }
    String::from_utf8(bytes.to_vec())
        .map(|text| (text, TextEncoding::Utf8))
        .map_err(|_| TransformError::InvalidText)
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, TransformError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(TransformError::InvalidText);
    }
    let units = bytes.chunks_exact(2).map(|pair| {
        if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        }
    });
    std::char::decode_utf16(units)
        .collect::<Result<String, _>>()
        .map_err(|_| TransformError::InvalidText)
}

fn encode(text: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => [b"\xEF\xBB\xBF".as_slice(), text.as_bytes()].concat(),
        TextEncoding::Utf16Le | TextEncoding::Utf16Be => {
            let mut out = if encoding == TextEncoding::Utf16Le {
                vec![0xFF, 0xFE]
            } else {
                vec![0xFE, 0xFF]
            };
            for unit in text.encode_utf16() {
                let pair = if encoding == TextEncoding::Utf16Le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                out.extend_from_slice(&pair);
            }
            out
        }
    }
}

fn detect_encoding(bytes: &[u8]) -> TextEncoding {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        TextEncoding::Utf8Bom
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        TextEncoding::Utf16Le
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        TextEncoding::Utf16Be
    } else {
        TextEncoding::Utf8
    }
}

fn count_endings(text: &str) -> (usize, usize, usize) {
    let bytes = text.as_bytes();
    let mut crlf = 0;
    let mut lf = 0;
    let mut cr = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                crlf += 1;
                index += 2;
            }
            b'\r' => {
                cr += 1;
                index += 1;
            }
            b'\n' => {
                lf += 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    (crlf, lf, cr)
}

fn split_lines(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' || bytes[index] == b'\n' {
            lines.push(text[start..index].to_string());
            if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
    if start < bytes.len() {
        lines.push(text[start..].to_string());
    }
    lines
}

fn unique_tick() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostica_mezcla_sin_contar_crlf_dos_veces() {
        let d = diagnose_bytes(b"a\r\nb\nc\rd").unwrap();
        assert_eq!(d.line_ending, LineEnding::Mixed);
        assert_eq!((d.crlf_count, d.lf_count, d.cr_count), (1, 1, 1));
    }

    #[test]
    fn windows_a_unix_y_limpia_espacio_final() {
        let result = transform_bytes(
            b"uno  \r\ndos\r\n",
            TransformOptions {
                line_ending: LineEnding::Lf,
                encoding: TargetEncoding::Utf8,
                final_newline: FinalNewline::Preserve,
                trim_trailing_whitespace: true,
            },
        )
        .unwrap();
        assert_eq!(result, b"uno\ndos\n");
    }

    #[test]
    fn utf16_round_trip_a_utf8_bom() {
        let source = encode("hola\r\n世界", TextEncoding::Utf16Le);
        let result = transform_bytes(
            &source,
            TransformOptions {
                line_ending: LineEnding::Lf,
                encoding: TargetEncoding::Utf8Bom,
                final_newline: FinalNewline::Remove,
                trim_trailing_whitespace: false,
            },
        )
        .unwrap();
        assert_eq!(&result[..3], &[0xEF, 0xBB, 0xBF]);
        assert_eq!(std::str::from_utf8(&result[3..]).unwrap(), "hola\n世界");
    }

    #[test]
    fn reemplazo_transaccional_deja_respaldo() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.txt");
        fs::write(&path, b"a\r\nb\r\n").unwrap();
        let out = transform_file(
            &path,
            TransformOptions::default(),
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"a\nb\n");
        assert_eq!(fs::read(&out.backup_path).unwrap(), b"a\r\nb\r\n");
    }

    #[test]
    fn binario_con_nul_se_rechaza() {
        assert_eq!(
            diagnose_bytes(b"abc\0def"),
            Err(TransformError::InvalidText)
        );
    }

    #[test]
    fn archivo_sobre_limite_se_rechaza_antes_de_reservar_memoria() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("enorme.txt");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_TEXT_TRANSFORM_BYTES + 1).unwrap();
        assert_eq!(
            transform_file(
                &path,
                TransformOptions::default(),
                &CancellationToken::new()
            ),
            Err(TransformError::TooLarge)
        );
    }

    #[test]
    fn cancelado_antes_de_leer_preserva_original_y_no_deja_temporales() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.txt");
        fs::write(&path, b"a\r\nb\r\n").unwrap();
        let token = CancellationToken::new();
        token.cancel();
        assert_eq!(
            transform_file(&path, TransformOptions::default(), &token),
            Err(TransformError::Cancelled)
        );
        assert_eq!(fs::read(&path).unwrap(), b"a\r\nb\r\n");
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }

    #[test]
    fn destino_invalido_no_toca_el_archivo() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.txt");
        fs::write(&path, b"a\r\nb\r\n").unwrap();
        let options = TransformOptions {
            line_ending: LineEnding::Mixed,
            ..TransformOptions::default()
        };
        assert_eq!(
            transform_file(&path, options, &CancellationToken::new()),
            Err(TransformError::InvalidTarget)
        );
        assert_eq!(fs::read(&path).unwrap(), b"a\r\nb\r\n");
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }

    #[test]
    fn mac_clasico_a_windows_con_salto_final_asegurado() {
        let converted = transform_bytes(
            b"uno\rdos",
            TransformOptions {
                line_ending: LineEnding::Crlf,
                encoding: TargetEncoding::Utf8,
                final_newline: FinalNewline::Ensure,
                trim_trailing_whitespace: false,
            },
        )
        .unwrap();
        assert_eq!(converted, b"uno\r\ndos\r\n");
    }

    #[test]
    fn utf16_truncado_se_rechaza() {
        assert_eq!(
            diagnose_bytes(&[0xFF, 0xFE, 0x41]),
            Err(TransformError::InvalidText)
        );
    }
}
