// Naygo — abrir archivos con su programa por defecto (Win32 ShellExecuteW), aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `open_default` abre un archivo con la app asociada (verbo "open"); `open_with_dialog`
//! lanza el diálogo "Abrir con…" de Windows (verbo "openas"). Tolerante: devuelve un
//! `Result` tipado, nunca tumba el proceso. La política de Naygo es abrir con el SO,
//! no reproducir/editar.

use std::path::Path;

/// Error al pedirle al Shell que abra un archivo.
#[derive(Debug)]
pub enum ShellError {
    /// No soportado en esta plataforma.
    NotSupported,
    /// No hay programa asociado a ese tipo de archivo.
    NoAssociation,
    /// Otra falla del Shell (el mensaje describe el código).
    Failed(String),
}

#[cfg(not(windows))]
pub fn open_default(_path: &Path) -> Result<(), ShellError> {
    Err(ShellError::NotSupported)
}

#[cfg(not(windows))]
pub fn open_with_dialog(_path: &Path) -> Result<(), ShellError> {
    Err(ShellError::NotSupported)
}

#[cfg(windows)]
pub fn open_default(path: &Path) -> Result<(), ShellError> {
    windows_impl::shell_execute(path, "open")
}

#[cfg(windows)]
pub fn open_with_dialog(path: &Path) -> Result<(), ShellError> {
    windows_impl::shell_execute(path, "openas")
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    /// Ejecuta `verb` sobre `path`. ShellExecuteW devuelve un HINSTANCE cuyo valor
    /// numérico > 32 indica éxito; <= 32 es un código de error (SE_ERR_*).
    pub fn shell_execute(path: &Path, verb: &str) -> Result<(), ShellError> {
        let verb_w: Vec<u16> = verb.encode_utf16().chain(std::iter::once(0)).collect();
        let path_w: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: punteros válidos a cadenas NUL-terminadas; HWND nulo = sin ventana padre.
        let hinst = unsafe {
            ShellExecuteW(
                Some(HWND(std::ptr::null_mut())),
                PCWSTR(verb_w.as_ptr()),
                PCWSTR(path_w.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        let code = hinst.0 as usize;
        if code > 32 {
            Ok(())
        } else {
            const SE_ERR_NOASSOC: usize = 31;
            const SE_ERR_ASSOCINCOMPLETE: usize = 27;
            if code == SE_ERR_NOASSOC || code == SE_ERR_ASSOCINCOMPLETE {
                Err(ShellError::NoAssociation)
            } else {
                Err(ShellError::Failed(format!("ShellExecuteW devolvió {code}")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Smoke test manual: lanza la app asociada a .ini (normalmente el Bloc de
    // notas), por eso queda `#[ignore]` para no abrir ventanas en CI. Correr con:
    //   cargo test -p naygo-platform open_smoke -- --ignored --test-threads=1
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn open_smoke() {
        let res = open_default(Path::new("C:\\Windows\\win.ini"));
        assert!(res.is_ok(), "open_default falló: {res:?}");
    }
}
