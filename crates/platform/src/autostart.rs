// Naygo — inicio con Windows (clave Run de HKCU, sin permisos de admin).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Autostart: escribe/borra el valor `Naygo` en
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` con la ruta del exe entre
//! comillas. `is_enabled` refleja el registro REAL (no se duplica en settings.json).
//! Tolerante: los errores se devuelven como `String` y el llamador los loguea; nada
//! de esto puede tirar la app.

#[cfg(windows)]
mod windows_impl {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "Naygo";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// ¿Existe el valor `Naygo` en la clave Run del usuario?
    pub fn is_enabled() -> bool {
        unsafe {
            let mut key = HKEY::default();
            let kw = wide(RUN_KEY);
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(kw.as_ptr()),
                Some(0),
                KEY_QUERY_VALUE,
                &mut key,
            )
            .is_err()
            {
                return false;
            }
            let vw = wide(VALUE_NAME);
            let found = RegQueryValueExW(key, PCWSTR(vw.as_ptr()), None, None, None, None)
                .ok()
                .is_ok();
            let _ = RegCloseKey(key);
            found
        }
    }

    /// Activa/desactiva el inicio con Windows para el exe ACTUAL. `extra_args` se agregan
    /// tal cual (separados por espacio) después de la ruta entre comillas, p.ej. `&["--tray"]`
    /// para que el proceso arranque directo a la bandeja.
    pub fn set_enabled(on: bool, extra_args: &[&str]) -> Result<(), String> {
        unsafe {
            let mut key = HKEY::default();
            let kw = wide(RUN_KEY);
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(kw.as_ptr()),
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE | KEY_QUERY_VALUE,
                None,
                &mut key,
                None,
            )
            .ok()
            .map_err(|e| format!("abrir clave Run: {e}"))?;
            let vw = wide(VALUE_NAME);
            let result = if on {
                let exe = std::env::current_exe()
                    .map_err(|e| format!("ruta del exe: {e}"))?
                    .display()
                    .to_string();
                let mut cmd = format!("\"{exe}\"");
                for a in extra_args {
                    cmd.push(' ');
                    cmd.push_str(a);
                }
                let data_w = wide(&cmd);
                // RegSetValueExW espera los bytes crudos del wide string (incl. el nul).
                let bytes: &[u8] =
                    std::slice::from_raw_parts(data_w.as_ptr() as *const u8, data_w.len() * 2);
                RegSetValueExW(key, PCWSTR(vw.as_ptr()), None, REG_SZ, Some(bytes))
                    .ok()
                    .map_err(|e| format!("escribir valor: {e}"))
            } else {
                let e = RegDeleteValueW(key, PCWSTR(vw.as_ptr()));
                // Borrar un valor que no existe es un no-op exitoso.
                if e.is_ok() || e == ERROR_FILE_NOT_FOUND {
                    Ok(())
                } else {
                    Err(format!("borrar valor: {:?}", e))
                }
            };
            let _ = RegCloseKey(key);
            result
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{is_enabled, set_enabled};

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_enabled(_on: bool, _extra_args: &[&str]) -> Result<(), String> {
    Err("autostart solo está soportado en Windows".to_string())
}
