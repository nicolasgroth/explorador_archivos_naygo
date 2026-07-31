// Naygo — proveedor de metadata para audio (duración, bitrate, frecuencia, canales, tags).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::{MetadataField, MetadataProvider};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::Accessor;
use std::path::Path;

/// Metadata de audio usando la crate `lofty` (pura-Rust). Lee solo propiedades y tags: nunca
/// decodifica el audio. Vacío si el archivo no es legible o no tiene datos. Tolerante: cualquier
/// error o campo ausente se ignora y se sigue con lo que haya.
pub struct AudioMeta;

impl MetadataProvider for AudioMeta {
    fn extensions(&self) -> &'static [&'static str] {
        &["mp3", "flac", "ogg", "m4a", "wav"]
    }

    fn read(&self, path: &Path) -> Vec<MetadataField> {
        // `read_from_path` abre y parsea solo cabecera/tags, sin decodificar el audio.
        let Ok(tagged) = lofty::read_from_path(path) else {
            return Vec::new();
        };

        let mut fields = Vec::new();

        // Propiedades técnicas del stream.
        let props = tagged.properties();

        let secs = props.duration().as_secs();
        if secs > 0 {
            fields.push(MetadataField {
                label_key: "meta.duration",
                value: format_duration(secs),
            });
        }
        if let Some(kbps) = props.audio_bitrate() {
            fields.push(MetadataField {
                label_key: "meta.bitrate",
                value: format!("{kbps} kbps"),
            });
        }
        if let Some(hz) = props.sample_rate() {
            fields.push(MetadataField {
                label_key: "meta.sample_rate",
                value: format!("{hz} Hz"),
            });
        }
        if let Some(ch) = props.channels() {
            fields.push(MetadataField {
                label_key: "meta.channels",
                value: format!("{ch}"),
            });
        }

        // Tags (artista/título/álbum/año). Se prefiere el tag primario del contenedor; puede faltar.
        if let Some(tag) = tagged.primary_tag() {
            if let Some(v) = non_empty(tag.artist()) {
                fields.push(MetadataField {
                    label_key: "meta.artist",
                    value: v,
                });
            }
            if let Some(v) = non_empty(tag.title()) {
                fields.push(MetadataField {
                    label_key: "meta.title",
                    value: v,
                });
            }
            if let Some(v) = non_empty(tag.album()) {
                fields.push(MetadataField {
                    label_key: "meta.album",
                    value: v,
                });
            }
            if let Some(year) = tag.year() {
                if year > 0 {
                    fields.push(MetadataField {
                        label_key: "meta.year",
                        value: format!("{year}"),
                    });
                }
            }
        }

        fields
    }
}

/// Convierte un total de segundos a un texto legible: "H:MM:SS" si hay horas, si no "M:SS".
fn format_duration(total_secs: u64) -> String {
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Recorta espacios y descarta valores vacíos: `Some(texto)` solo si queda contenido útil.
fn non_empty(value: Option<std::borrow::Cow<'_, str>>) -> Option<String> {
    let trimmed = value?.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Genera un WAV PCM 8-bit mono con `secs` segundos de silencio (fixture mínimo real).
    /// `id3`: tag ID3v2 ya serializado (ver `id3v2_tag`), se embebe en un chunk "ID3 ".
    /// NOTA: lofty considera el tag PRIMARIO de un WAV el ID3v2, no el LIST-INFO
    /// (FileType::Wav → TagType::Id3v2), por eso el fixture de tags usa este chunk.
    fn write_wav(path: &Path, sample_rate: u32, secs: u32, id3: Option<&[u8]>) {
        let data_size = sample_rate * secs; // 1 byte por muestra (8-bit mono)
        let mut body: Vec<u8> = Vec::new();
        // Chunk fmt: PCM, mono, 8 bits.
        body.extend_from_slice(b"fmt ");
        body.extend_from_slice(&16u32.to_le_bytes());
        body.extend_from_slice(&1u16.to_le_bytes()); // PCM
        body.extend_from_slice(&1u16.to_le_bytes()); // canales
        body.extend_from_slice(&sample_rate.to_le_bytes());
        body.extend_from_slice(&sample_rate.to_le_bytes()); // byte_rate = rate * 1 * 1
        body.extend_from_slice(&1u16.to_le_bytes()); // block_align
        body.extend_from_slice(&8u16.to_le_bytes()); // bits por muestra
                                                     // Chunk "ID3 " opcional con el tag embebido.
        if let Some(tag) = id3 {
            body.extend_from_slice(b"ID3 ");
            body.extend_from_slice(&(tag.len() as u32).to_le_bytes());
            body.extend_from_slice(tag);
            if tag.len() % 2 == 1 {
                body.push(0); // padding a par
            }
        }
        // Chunk data (silencio: valor medio 128 en PCM 8-bit).
        body.extend_from_slice(b"data");
        body.extend_from_slice(&data_size.to_le_bytes());
        body.extend(std::iter::repeat_n(128u8, data_size as usize));

        let mut out: Vec<u8> = b"RIFF".to_vec();
        out.extend_from_slice(&(body.len() as u32 + 4).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(&body);
        std::fs::write(path, out).unwrap();
    }

    /// Serializa un tag ID3v2.3 mínimo con frames de texto (encoding 0 = Latin-1).
    /// `frames`: pares (id de frame de 4 chars, texto), p. ej. ("TPE1", "Artista").
    fn id3v2_tag(frames: &[(&str, &str)]) -> Vec<u8> {
        let mut body: Vec<u8> = Vec::new();
        for (id, text) in frames {
            let mut data = vec![0u8]; // encoding byte: ISO-8859-1
            data.extend_from_slice(text.as_bytes());
            body.extend_from_slice(id.as_bytes());
            body.extend_from_slice(&(data.len() as u32).to_be_bytes()); // v2.3: tamaño BE plano
            body.extend_from_slice(&[0, 0]); // flags
            body.extend_from_slice(&data);
        }
        let mut tag: Vec<u8> = b"ID3".to_vec();
        tag.extend_from_slice(&[3, 0, 0]); // v2.3, sin flags
                                           // Tamaño del cuerpo en syncsafe (7 bits por byte).
        let n = body.len();
        tag.extend_from_slice(&[
            ((n >> 21) & 0x7F) as u8,
            ((n >> 14) & 0x7F) as u8,
            ((n >> 7) & 0x7F) as u8,
            (n & 0x7F) as u8,
        ]);
        tag.extend_from_slice(&body);
        tag
    }

    #[test]
    fn audio_inexistente_da_vacio() {
        let fields = AudioMeta.read(std::path::Path::new(r"C:\no\existe.mp3"));
        assert!(fields.is_empty());
    }

    #[test]
    fn wav_valido_reporta_propiedades() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tono.wav");
        write_wav(&p, 8000, 2, None);
        let fields = AudioMeta.read(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.duration" && f.value == "0:02"),
            "debe reportar 2s de duración: {fields:?}"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.sample_rate" && f.value == "8000 Hz"),
            "debe reportar 8000 Hz: {fields:?}"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.channels" && f.value == "1"),
            "debe reportar 1 canal: {fields:?}"
        );
    }

    #[test]
    fn wav_con_id3_reporta_tags() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tags.wav");
        let tag = id3v2_tag(&[("TPE1", "Artista Prueba"), ("TIT2", "Tema Prueba")]);
        write_wav(&p, 8000, 1, Some(&tag));
        let fields = AudioMeta.read(&p);
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.artist" && f.value == "Artista Prueba"),
            "debe reportar el artista del ID3: {fields:?}"
        );
        assert!(
            fields
                .iter()
                .any(|f| f.label_key == "meta.title" && f.value == "Tema Prueba"),
            "debe reportar el título del ID3: {fields:?}"
        );
    }

    #[test]
    fn archivo_que_no_es_audio_da_vacio_sin_panic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("texto.mp3");
        std::fs::write(&p, b"esto es solo texto, no audio").unwrap();
        assert!(AudioMeta.read(&p).is_empty());
    }

    #[test]
    fn archivo_vacio_da_vacio_sin_panic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("vacio.flac");
        std::fs::write(&p, b"").unwrap();
        assert!(AudioMeta.read(&p).is_empty());
    }

    #[test]
    fn wav_truncado_no_paniquea() {
        // Un WAV válido cortado a la mitad: cabecera RIFF reconocible pero datos incompletos.
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("ok.wav");
        write_wav(&ok, 8000, 2, None);
        let bytes = std::fs::read(&ok).unwrap();
        let p = dir.path().join("mitad.wav");
        std::fs::write(&p, &bytes[..bytes.len() / 2]).unwrap();
        // No panic; lofty puede rechazarlo o leer propiedades parciales, ambos son válidos.
        let _ = AudioMeta.read(&p);
    }

    #[test]
    fn non_empty_recorta_y_descarta_vacios() {
        assert_eq!(non_empty(Some("  hola  ".into())), Some("hola".to_string()));
        assert_eq!(non_empty(Some("   ".into())), None);
        assert_eq!(non_empty(Some("".into())), None);
        assert_eq!(non_empty(None), None);
    }

    #[test]
    fn extensiones_declaradas() {
        let exts = AudioMeta.extensions();
        for e in ["mp3", "flac", "ogg", "m4a", "wav"] {
            assert!(exts.contains(&e), "falta la extensión {e}");
        }
    }

    #[test]
    fn format_duration_bajo_una_hora() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(5), "0:05");
        assert_eq!(format_duration(65), "1:05");
        assert_eq!(format_duration(599), "9:59");
        assert_eq!(format_duration(3599), "59:59");
    }

    #[test]
    fn format_duration_con_horas() {
        assert_eq!(format_duration(3600), "1:00:00");
        assert_eq!(format_duration(3661), "1:01:01");
        assert_eq!(format_duration(7325), "2:02:05");
    }
}
