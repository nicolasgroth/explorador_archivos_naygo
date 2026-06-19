// Naygo — formateo de tamaños legibles (B/KB/MB/GB/TB). Puro y reutilizable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `human_size` formatea un número de bytes a una cadena legible (1 decimal). Base
//! 1024. Reutilizado por tamaños de archivo, de transferencia y de disco.

const KB: f64 = 1024.0;
const MB: f64 = KB * 1024.0;
const GB: f64 = MB * 1024.0;
const TB: f64 = GB * 1024.0;

/// Formatea `bytes` como "B/KB/MB/GB/TB" con un decimal (salvo bytes crudos).
pub fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= TB {
        format!("{:.1} TB", b / TB)
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Cómo se muestra el tamaño en la columna. Configurable por el usuario.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SizeFormat {
    /// Unidad automática legible (default): 512 B / 1.5 KB / 20.2 MB.
    #[default]
    Auto,
    /// Siempre en bytes, con separadores de miles: "21.250.048".
    Bytes,
    /// Siempre en KB (1 decimal): "20.2 KB".
    Kb,
    /// Siempre en MB (1 decimal): "20.2 MB".
    Mb,
}

/// Formatea `bytes` según `fmt`.
pub fn format_size(bytes: u64, fmt: SizeFormat) -> String {
    match fmt {
        SizeFormat::Auto => human_size(bytes),
        SizeFormat::Bytes => {
            // Separador de miles con punto (convención es-CL).
            let s = bytes.to_string();
            let mut out = String::new();
            for (i, c) in s.chars().rev().enumerate() {
                if i > 0 && i % 3 == 0 {
                    out.push('.');
                }
                out.push(c);
            }
            out.chars().rev().collect()
        }
        SizeFormat::Kb => format!("{:.1} KB", bytes as f64 / KB),
        SizeFormat::Mb => format!("{:.1} MB", bytes as f64 / MB),
    }
}

/// Cómo se muestran las fechas de las columnas (Modificado/Creado). El usuario lo elige en
/// Configuración. Todas son legibles; difieren en precisión/estilo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DateFormat {
    /// "2026-06-15 18:48" (ISO, default — lo prefiere Nicolás).
    #[default]
    IsoMinute,
    /// "2026-06-15" (solo fecha ISO).
    IsoDate,
    /// "15-06-2026 18:48" (día-mes-año + hora).
    DmyMinute,
    /// "15-06-2026" (solo fecha día-mes-año).
    DmyDate,
}

/// Convierte segundos epoch UTC a (año, mes 1..12, día 1..31, hora, minuto). Algoritmo de
/// fecha civil de Howard Hinnant (sin dependencias). Asume días de 86400s.
fn civil_from_epoch(secs: i64) -> (i64, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = (rem / 3600) as u32;
    let minute = ((rem % 3600) / 60) as u32;
    // Hinnant: días desde 1970-01-01 → fecha civil.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, hour, minute)
}

/// Formatea un instante (segundos epoch, ya ajustados al huso local por el llamador) según
/// `fmt`. Cadena vacía si no hay fecha. Puro y sin dependencias.
pub fn format_time(local_epoch_secs: Option<i64>, fmt: DateFormat) -> String {
    let Some(secs) = local_epoch_secs else {
        return String::new();
    };
    let (y, mo, d, h, mi) = civil_from_epoch(secs);
    match fmt {
        DateFormat::IsoMinute => format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}"),
        DateFormat::IsoDate => format!("{y:04}-{mo:02}-{d:02}"),
        DateFormat::DmyMinute => format!("{d:02}-{mo:02}-{y:04} {h:02}:{mi:02}"),
        DateFormat::DmyDate => format!("{d:02}-{mo:02}-{y:04}"),
    }
}

/// Formatea un instante para el log: `"YYYY-MM-DD HH:MM:SS.mmm"` en hora LOCAL.
/// `epoch_ms` son milisegundos desde epoch UTC; `tz_offset_min` es el desplazamiento del
/// huso local en minutos (p. ej. -180 para UTC-3, +60 para UTC+1). Puro, sin dependencias.
pub fn format_log_time(epoch_ms: u64, tz_offset_min: i32) -> String {
    let total_secs = (epoch_ms / 1000) as i64 + (tz_offset_min as i64) * 60;
    let millis = (epoch_ms % 1000) as u32;
    let (y, mo, d, h, mi) = civil_from_epoch(total_secs);
    let sec = total_secs.rem_euclid(60) as u32;
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{sec:02}.{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_format_variantes() {
        assert_eq!(format_size(512, SizeFormat::Auto), "512 B");
        assert_eq!(format_size(1536, SizeFormat::Auto), "1.5 KB");
        assert_eq!(format_size(21_250_048, SizeFormat::Bytes), "21.250.048");
        assert_eq!(format_size(1024 * 1024, SizeFormat::Kb), "1024.0 KB");
        assert_eq!(format_size(1024 * 1024, SizeFormat::Mb), "1.0 MB");
        // Caso chico de bytes: sin separador.
        assert_eq!(format_size(512, SizeFormat::Bytes), "512");
    }

    #[test]
    fn fecha_iso_minuto() {
        // 2021-01-01 00:00:00 UTC = 1609459200.
        assert_eq!(
            format_time(Some(1_609_459_200), DateFormat::IsoMinute),
            "2021-01-01 00:00"
        );
    }

    #[test]
    fn fecha_variantes() {
        // 1781481600 = 2026-06-15 00:00:00 UTC.
        let t = 1_781_481_600;
        assert_eq!(format_time(Some(t), DateFormat::IsoDate), "2026-06-15");
        assert_eq!(format_time(Some(t), DateFormat::DmyDate), "15-06-2026");
        assert_eq!(
            format_time(Some(t), DateFormat::DmyMinute),
            "15-06-2026 00:00"
        );
    }

    #[test]
    fn fecha_vacia_si_none() {
        assert_eq!(format_time(None, DateFormat::IsoMinute), "");
    }

    #[test]
    fn bytes_crudos() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1023), "1023 B");
    }

    #[test]
    fn kilobytes() {
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1536), "1.5 KB");
    }

    #[test]
    fn mega_giga_tera() {
        assert_eq!(human_size(1024 * 1024), "1.0 MB");
        assert_eq!(human_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(human_size(1024u64 * 1024 * 1024 * 1024), "1.0 TB");
    }

    #[test]
    fn format_log_time_basico() {
        // 2026-06-18 00:00:00.000 UTC, offset 0.
        let s = format_log_time(1_781_740_800_000, 0);
        assert_eq!(s, "2026-06-18 00:00:00.000");
    }

    #[test]
    fn format_log_time_con_offset_y_milisegundos() {
        // +123 ms, offset -180 min (UTC-3): la hora local resta 3 h => 2026-06-17 21:00:00.123.
        let s = format_log_time(1_781_740_800_123, -180);
        assert_eq!(s, "2026-06-17 21:00:00.123");
    }

    #[test]
    fn format_log_time_offset_positivo() {
        // offset +60 min: suma 1 hora => 2026-06-18 01:00:00.000.
        let s = format_log_time(1_781_740_800_000, 60);
        assert_eq!(s, "2026-06-18 01:00:00.000");
    }
}
