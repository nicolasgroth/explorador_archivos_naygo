// Naygo — zona horaria local (Win32), aislada en la capa platform.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Offset local respecto a UTC, en segundos. Lo consumen los comodines de fecha
//! del batch-rename (core es puro y no conoce la zona horaria: la UI le pasa este
//! valor). Positivo al este de Greenwich (Chile invierno: -4h → -14400).

/// Resolver en el worker: hoy/semana son límites del calendario local, no restas
/// con el offset actual (puede haber cambiado el DST durante la semana).
pub fn search_date_lower_bound(
    date: naygo_core::saved_search::RelativeDate,
    now: i64,
) -> Option<i64> {
    search_date_in_zone(date, now, &chrono::Local)
}

/// Los parámetros de calendario se congelan al preparar, no al publicar ni importar.
pub fn recipe_time(
    date: naygo_core::saved_search::RelativeDate,
) -> Option<naygo_core::recipe::EvaluationTime> {
    let now = chrono::Utc::now();
    let lower = search_date_lower_bound(date, now.timestamp());
    if date != naygo_core::saved_search::RelativeDate::Any && lower.is_none() {
        return None;
    }
    Some(naygo_core::recipe::EvaluationTime {
        now: now.timestamp(),
        lower,
        date: now
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d")
            .to_string(),
    })
}

fn search_date_in_zone<T: chrono::TimeZone>(
    date: naygo_core::saved_search::RelativeDate,
    now: i64,
    zone: &T,
) -> Option<i64> {
    use chrono::{Datelike, Days, TimeDelta};
    use naygo_core::saved_search::RelativeDate;
    if !matches!(
        date,
        RelativeDate::Today | RelativeDate::ThisWeek | RelativeDate::ThisMonth
    ) {
        return date.lower_bound(now, 0);
    }
    let local = chrono::DateTime::from_timestamp(now, 0)?.with_timezone(zone);
    let mut day = local.date_naive();
    if date == RelativeDate::ThisWeek {
        day = day.checked_sub_days(Days::new(day.weekday().num_days_from_monday() as u64))?;
    }
    if date == RelativeDate::ThisMonth {
        day = day.with_day(1)?;
    }
    let midnight = day.and_hms_opt(0, 0, 0)?;
    // Algunas zonas adelantan el reloj a medianoche: elegir el primer instante válido.
    (0..=180).find_map(|minute| {
        zone.from_local_datetime(&(midnight + TimeDelta::minutes(minute)))
            .earliest()
            .map(|t| t.timestamp())
    })
}

#[cfg(test)]
#[path = "time_query_tests.rs"]
mod query_tests;

/// Offset local vs UTC en segundos (local = UTC + offset). `0` si Windows no pudo
/// informarlo (mejor una hora UTC que ninguna).
#[cfg(windows)]
pub fn local_utc_offset_secs() -> i64 {
    use windows::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
    let mut tzi = TIME_ZONE_INFORMATION::default();
    // SAFETY: pasamos un struct válido que Windows rellena.
    let id = unsafe { GetTimeZoneInformation(&mut tzi) };
    // Bias en MINUTOS y con signo invertido al usual: UTC = local + Bias.
    // Retorno: 0 = desconocido, 1 = horario estándar, 2 = horario de verano.
    let bias = tzi.Bias
        + match id {
            2 => tzi.DaylightBias,
            1 => tzi.StandardBias,
            _ => 0,
        };
    -(bias as i64) * 60
}

/// En no-Windows (mantiene el crate compilable): sin offset.
#[cfg(not(windows))]
pub fn local_utc_offset_secs() -> i64 {
    0
}
