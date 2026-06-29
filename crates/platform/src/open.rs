// Naygo — abrir archivos con su programa por defecto (Win32 ShellExecuteW), aislado.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

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

/// Qué terminal abrir en una carpeta. `WindowsTerminal` solo está disponible si `wt.exe` está
/// instalado (es opcional en Windows 10); usar [`windows_terminal_available`] para decidir si
/// ofrecerlo en el menú.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terminal {
    PowerShell,
    Cmd,
    WindowsTerminal,
    Wsl,
}

impl Terminal {
    /// El ejecutable que lanza esta terminal.
    fn exe(self) -> &'static str {
        match self {
            Terminal::PowerShell => "powershell.exe",
            Terminal::Cmd => "cmd.exe",
            Terminal::WindowsTerminal => "wt.exe",
            Terminal::Wsl => "wsl.exe",
        }
    }
}

#[cfg(not(windows))]
pub fn open_default(_path: &Path) -> Result<(), ShellError> {
    Err(ShellError::NotSupported)
}

#[cfg(not(windows))]
pub fn open_with_dialog(_path: &Path) -> Result<(), ShellError> {
    Err(ShellError::NotSupported)
}

#[cfg(not(windows))]
pub fn open_terminal(_dir: &Path, _term: Terminal) -> Result<(), ShellError> {
    Err(ShellError::NotSupported)
}

#[cfg(not(windows))]
pub fn windows_terminal_available() -> bool {
    false
}

#[cfg(not(windows))]
pub fn wsl_available() -> bool {
    false
}

#[cfg(windows)]
pub fn open_default(path: &Path) -> Result<(), ShellError> {
    windows_impl::shell_execute(path, "open")
}

#[cfg(windows)]
pub fn open_with_dialog(path: &Path) -> Result<(), ShellError> {
    windows_impl::shell_execute(path, "openas")
}

/// Abre `term` con la carpeta de trabajo en `dir`. Para Windows Terminal se pasa `-d <dir>`
/// porque `wt.exe` ignora el directorio de trabajo del Shell; PowerShell y CMD heredan el
/// `lpDirectory`. No tumba el proceso: devuelve un `Result` tipado.
#[cfg(windows)]
pub fn open_terminal(dir: &Path, term: Terminal) -> Result<(), ShellError> {
    let params = match term {
        // `wt -d <dir>` abre una pestaña ya posicionada en la carpeta.
        Terminal::WindowsTerminal => Some(format!("-d \"{}\"", dir.display())),
        // PowerShell/CMD/WSL arrancan en el `lpDirectory` que pasamos a ShellExecuteW (WSL mapea
        // ese cwd de Windows a su `/mnt/...` automáticamente).
        Terminal::PowerShell | Terminal::Cmd | Terminal::Wsl => None,
    };
    windows_impl::shell_execute_in(term.exe(), params.as_deref(), Some(dir))
}

/// `true` si `wt.exe` (Windows Terminal) parece estar disponible en el PATH del usuario.
#[cfg(windows)]
pub fn windows_terminal_available() -> bool {
    windows_impl::exe_in_path("wt.exe")
}

/// `true` si `wsl.exe` (Subsistema de Windows para Linux) parece estar en el PATH del usuario.
#[cfg(windows)]
pub fn wsl_available() -> bool {
    windows_impl::exe_in_path("wsl.exe")
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    /// Codifica una cadena a UTF-16 NUL-terminada (para los PCWSTR de Win32).
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn wide_path(p: &Path) -> Vec<u16> {
        p.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Ejecuta `verb` sobre `path`. ShellExecuteW devuelve un HINSTANCE cuyo valor
    /// numérico > 32 indica éxito; <= 32 es un código de error (SE_ERR_*).
    pub fn shell_execute(path: &Path, verb: &str) -> Result<(), ShellError> {
        let verb_w = wide(verb);
        let path_w = wide_path(path);
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
        check(hinst.0 as usize)
    }

    /// Ejecuta `file` (un ejecutable o documento) con verbo "open", parámetros opcionales y un
    /// directorio de trabajo opcional. Usado para lanzar terminales en una carpeta.
    pub fn shell_execute_in(
        file: &str,
        params: Option<&str>,
        dir: Option<&Path>,
    ) -> Result<(), ShellError> {
        let verb_w = wide("open");
        let file_w = wide(file);
        let params_w = params.map(wide);
        let dir_w = dir.map(wide_path);
        let params_ptr = params_w
            .as_ref()
            .map(|v| PCWSTR(v.as_ptr()))
            .unwrap_or(PCWSTR::null());
        let dir_ptr = dir_w
            .as_ref()
            .map(|v| PCWSTR(v.as_ptr()))
            .unwrap_or(PCWSTR::null());
        // SAFETY: todos los punteros refieren cadenas vivas hasta el fin de la llamada.
        let hinst = unsafe {
            ShellExecuteW(
                Some(HWND(std::ptr::null_mut())),
                PCWSTR(verb_w.as_ptr()),
                PCWSTR(file_w.as_ptr()),
                params_ptr,
                dir_ptr,
                SW_SHOWNORMAL,
            )
        };
        check(hinst.0 as usize)
    }

    /// Traduce el código de retorno de ShellExecuteW a `Result`.
    fn check(code: usize) -> Result<(), ShellError> {
        if code > 32 {
            return Ok(());
        }
        const SE_ERR_NOASSOC: usize = 31;
        const SE_ERR_ASSOCINCOMPLETE: usize = 27;
        if code == SE_ERR_NOASSOC || code == SE_ERR_ASSOCINCOMPLETE {
            Err(ShellError::NoAssociation)
        } else {
            Err(ShellError::Failed(format!("ShellExecuteW devolvió {code}")))
        }
    }

    /// `true` si `exe` se encuentra en alguna carpeta del PATH del usuario.
    pub fn exe_in_path(exe: &str) -> bool {
        let Some(paths) = std::env::var_os("PATH") else {
            return false;
        };
        std::env::split_paths(&paths).any(|dir| dir.join(exe).is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn terminal_exe_mapea_cada_variante() {
        assert_eq!(Terminal::PowerShell.exe(), "powershell.exe");
        assert_eq!(Terminal::Cmd.exe(), "cmd.exe");
        assert_eq!(Terminal::WindowsTerminal.exe(), "wt.exe");
    }

    // Smoke test manual: abre PowerShell en C:\Windows. `#[ignore]` para no abrir ventanas en CI.
    //   cargo test -p naygo-platform terminal_smoke -- --ignored --test-threads=1
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn terminal_smoke() {
        let res = open_terminal(Path::new("C:\\Windows"), Terminal::PowerShell);
        assert!(res.is_ok(), "open_terminal falló: {res:?}");
    }

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
