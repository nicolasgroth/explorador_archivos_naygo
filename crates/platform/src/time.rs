// Naygo — zona horaria local (Win32), aislada en la capa platform.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Offset local respecto a UTC, en segundos. Lo consumen los comodines de fecha
//! del batch-rename (core es puro y no conoce la zona horaria: la UI le pasa este
//! valor). Positivo al este de Greenwich (Chile invierno: -4h → -14400).

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
