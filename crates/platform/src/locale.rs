// Naygo — lectura del locale del SO (Win32), aislada en la capa platform.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Devuelve el nombre de locale del usuario (p. ej. "es-CL") consultando Windows.
//! La elección del idioma a partir de este string la hace `core::pick_default_language`.

/// Locale del usuario del SO, o `None` si no se pudo leer.
#[cfg(windows)]
pub fn os_locale() -> Option<String> {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;
    // LOCALE_NAME_MAX_LENGTH es 85; un buffer holgado basta.
    let mut buf = [0u16; 85];
    // SAFETY: pasamos un buffer válido y su longitud; la función escribe UTF-16.
    let len = unsafe { GetUserDefaultLocaleName(&mut buf) };
    if len > 0 {
        // `len` incluye el terminador nulo; recortarlo.
        let n = (len as usize).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..n]))
    } else {
        None
    }
}

/// En no-Windows (no es el target real, pero mantiene el crate compilable): lee LANG.
#[cfg(not(windows))]
pub fn os_locale() -> Option<String> {
    std::env::var("LANG")
        .ok()
        .map(|l| l.split('.').next().unwrap_or("").to_string())
}
