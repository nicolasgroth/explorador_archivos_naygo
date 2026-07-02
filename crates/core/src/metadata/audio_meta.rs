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

    #[test]
    fn audio_inexistente_da_vacio() {
        let fields = AudioMeta.read(std::path::Path::new(r"C:\no\existe.mp3"));
        assert!(fields.is_empty());
    }

    #[test]
    fn format_duration_bajo_una_hora() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(5), "0:05");
        assert_eq!(format_duration(65), "1:05");
        assert_eq!(format_duration(599), "9:59");
    }

    #[test]
    fn format_duration_con_horas() {
        assert_eq!(format_duration(3600), "1:00:00");
        assert_eq!(format_duration(3661), "1:01:01");
        assert_eq!(format_duration(7325), "2:02:05");
    }
}
