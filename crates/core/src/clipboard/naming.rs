// Naygo — expansión de plantillas de nombre para el pegado (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Expande plantillas de nombre como "pegado {fecha}" sin depender de `chrono`.
//! `{fecha}` → "YYYY-MM-DD HH-MM" en UTC, derivada de los segundos epoch (determinista
//! y testeable). Otros `{...}` desconocidos se dejan literales.

/// Expande `{fecha}` en `template` por "YYYY-MM-DD HH-MM" (UTC) derivado de
/// `now_secs` (segundos epoch). Tokens `{...}` desconocidos se dejan literales.
pub fn expand_name_template(template: &str, now_secs: u64) -> String {
    let (y, mo, d, h, mi) = civil_from_epoch(now_secs);
    let fecha = format!("{y:04}-{mo:02}-{d:02} {h:02}-{mi:02}");
    template.replace("{fecha}", &fecha)
}

/// Convierte segundos epoch (UTC) a (año, mes, día, hora, minuto). Algoritmo de
/// días-civiles de Howard Hinnant (sin librerías de fecha). Calendario gregoriano.
fn civil_from_epoch(secs: u64) -> (i64, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = (rem / 3_600) as u32;
    let minute = ((rem % 3_600) / 60) as u32;

    // Hinnant civil_from_days: días desde 1970-01-01.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, hour, minute)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expande_fecha_epoch_cero() {
        // 1970-01-01 00:00 UTC
        assert_eq!(
            expand_name_template("pegado {fecha}", 0),
            "pegado 1970-01-01 00-00"
        );
    }

    #[test]
    fn expande_fecha_conocida() {
        // 2021-01-01 00:00:00 UTC = 1609459200
        assert_eq!(
            expand_name_template("x {fecha}", 1_609_459_200),
            "x 2021-01-01 00-00"
        );
    }

    #[test]
    fn expande_fecha_con_hora_minuto() {
        // 2021-01-01 13:45:00 UTC = 1609459200 + 13*3600 + 45*60 = 1609508700
        assert_eq!(
            expand_name_template("{fecha}", 1_609_508_700),
            "2021-01-01 13-45"
        );
    }

    #[test]
    fn sin_token_queda_igual() {
        assert_eq!(expand_name_template("captura", 123), "captura");
    }

    #[test]
    fn token_desconocido_literal() {
        assert_eq!(expand_name_template("a {otro} b", 0), "a {otro} b");
    }
}
