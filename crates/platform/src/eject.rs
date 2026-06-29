// Naygo — expulsión segura de unidades extraíbles (USB) vía Win32, aislado.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `eject_drive` desmonta y expulsa de forma SEGURA una unidad extraíble (USB).
//!
//! Enfoque (el clásico robusto sobre el handle del volumen, el que usan muchas
//! utilidades de "quitar hardware con seguridad"):
//!   1. `CreateFileW(\\.\X:)` abre el volumen con `FILE_SHARE_READ|WRITE`.
//!   2. `FSCTL_LOCK_VOLUME` toma el bloqueo exclusivo. Si falla, hay archivos
//!      abiertos → devolvemos [`EjectError::InUse`] y NO desmontamos nada.
//!   3. `FSCTL_DISMOUNT_VOLUME` desmonta el sistema de archivos.
//!   4. `IOCTL_STORAGE_MEDIA_REMOVAL` con `PreventMediaRemoval = false` libera el
//!      bloqueo de medio del dispositivo.
//!   5. `IOCTL_STORAGE_EJECT_MEDIA` expulsa.
//!   6. `CloseHandle` (suelta el lock del volumen) — siempre, vía guard.
//!
//! Decisión de diseño: NO usamos la ruta CfgMgr (`CM_Get_Parent` +
//! `CM_Request_Device_Eject`). Esa es más "completa" (apaga el árbol de
//! dispositivos del padre, ideal para "Quitar hardware con seguridad"), pero
//! exige resolver el *device instance* del volumen vía SetupAPI, bastante más
//! código y superficie de error. La ruta lock+dismount+eject sobre el handle del
//! volumen es suficiente para unidades extraíbles, es segura (nunca fuerza:
//! aborta limpio si el volumen está en uso) y es la que usan herramientas reales.
//!
//! SEGURIDAD: jamás se fuerza la expulsión. Si el bloqueo del volumen no se puede
//! tomar (archivos abiertos), se devuelve [`EjectError::InUse`] sin tocar el
//! sistema de archivos.
//!
//! No bloquea de forma apreciable: son IOCTLs de una sola llamada, sin sondeo.
//! No hay watcher aquí; los cambios de dispositivo ya los detecta `device_watch`.

use std::path::Path;

/// Error al intentar expulsar una unidad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EjectError {
    /// No soportado en esta plataforma.
    NotSupported,
    /// La unidad está en uso (no se pudo bloquear el volumen: hay archivos
    /// abiertos / handles vivos). NO se desmontó ni expulsó nada.
    InUse,
    /// La ruta no parece una raíz de unidad válida (p. ej. no es `X:\`).
    InvalidDrive,
    /// Otra falla del sistema (el mensaje describe el código/HRESULT).
    Failed(String),
}

impl std::fmt::Display for EjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EjectError::NotSupported => write!(f, "eject not supported on this platform"),
            EjectError::InUse => write!(f, "drive is in use (open files prevented locking)"),
            EjectError::InvalidDrive => write!(f, "path is not a valid drive root"),
            EjectError::Failed(msg) => write!(f, "eject failed: {msg}"),
        }
    }
}

impl std::error::Error for EjectError {}

/// Expulsa de forma segura la unidad extraíble cuya raíz es `root` (p. ej. `E:\`).
///
/// Devuelve `Ok(())` solo si el volumen se desmontó y expulsó sin forzar. Si hay
/// archivos abiertos, devuelve [`EjectError::InUse`] sin desmontar.
#[cfg(windows)]
pub fn eject_drive(root: &Path) -> Result<(), EjectError> {
    windows_impl::eject_drive(root)
}

/// Stub no-Windows: la expulsión de hardware no aplica.
#[cfg(not(windows))]
pub fn eject_drive(_root: &Path) -> Result<(), EjectError> {
    Err(EjectError::NotSupported)
}

/// Extrae la letra de unidad de una raíz (`"E:\\"` → `'E'`). `None` si no parece
/// una raíz de unidad con letra. Pública para que el llamador valide antes.
pub fn drive_letter(root: &Path) -> Option<char> {
    let s = root.to_string_lossy();
    let mut chars = s.chars();
    let first = chars.next()?;
    let second = chars.next()?;
    if first.is_ascii_alphabetic() && second == ':' {
        Some(first.to_ascii_uppercase())
    } else {
        None
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{drive_letter, EjectError};
    use std::path::Path;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Ioctl::{
        FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, IOCTL_STORAGE_EJECT_MEDIA,
        IOCTL_STORAGE_MEDIA_REMOVAL, PREVENT_MEDIA_REMOVAL,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    /// Cierra automáticamente un HANDLE al salir de su ámbito (RAII). Cerrar el
    /// handle del volumen también suelta `FSCTL_LOCK_VOLUME`, así que es vital que
    /// se ejecute pase lo que pase.
    struct HandleGuard(HANDLE);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            // SAFETY: `self.0` es un handle válido devuelto por CreateFileW.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    /// Codifica una cadena a UTF-16 NUL-terminada (para los PCWSTR de Win32).
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn eject_drive(root: &Path) -> Result<(), EjectError> {
        let letter = drive_letter(root).ok_or(EjectError::InvalidDrive)?;
        // Ruta del dispositivo del volumen: `\\.\X:` (sin barra final).
        let device = format!(r"\\.\{letter}:");
        let device_w = wide(&device);

        // Abrir el volumen. Necesitamos lectura+escritura para los IOCTLs de
        // desmontaje/expulsión.
        // SAFETY: device_w es una cadena UTF-16 NUL-terminada viva durante la
        // llamada; el resto de argumentos son constantes válidas.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(device_w.as_ptr()),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }
        .map_err(|e| EjectError::Failed(format!("CreateFileW: {e}")))?;

        // A partir de aquí, el guard garantiza el CloseHandle (y por tanto la
        // liberación del lock si lo tomamos) en TODOS los caminos de salida.
        let guard = HandleGuard(handle);
        let h = guard.0;

        // 1) Bloquear el volumen. Si falla, hay handles abiertos → en uso.
        //    NO desmontamos: devolvemos InUse y dejamos todo como estaba.
        // SAFETY: `h` es un handle de volumen válido; este IOCTL no usa buffers.
        let locked = unsafe { DeviceIoControl(h, FSCTL_LOCK_VOLUME, None, 0, None, 0, None, None) };
        if locked.is_err() {
            return Err(EjectError::InUse);
        }

        // 2) Desmontar el sistema de archivos.
        // SAFETY: volumen bloqueado; este IOCTL no usa buffers.
        unsafe { DeviceIoControl(h, FSCTL_DISMOUNT_VOLUME, None, 0, None, 0, None, None) }
            .map_err(|e| EjectError::Failed(format!("FSCTL_DISMOUNT_VOLUME: {e}")))?;

        // 3) Permitir la remoción del medio (PreventMediaRemoval = false).
        let mut pmr = PREVENT_MEDIA_REMOVAL {
            PreventMediaRemoval: false,
        };
        // SAFETY: `pmr` vive durante la llamada; tamaño correcto del buffer de entrada.
        unsafe {
            DeviceIoControl(
                h,
                IOCTL_STORAGE_MEDIA_REMOVAL,
                Some(&mut pmr as *mut _ as *const core::ffi::c_void),
                std::mem::size_of::<PREVENT_MEDIA_REMOVAL>() as u32,
                None,
                0,
                None,
                None,
            )
        }
        .map_err(|e| EjectError::Failed(format!("IOCTL_STORAGE_MEDIA_REMOVAL: {e}")))?;

        // 4) Expulsar el medio.
        // SAFETY: volumen desmontado y remoción permitida; este IOCTL no usa buffers.
        unsafe { DeviceIoControl(h, IOCTL_STORAGE_EJECT_MEDIA, None, 0, None, 0, None, None) }
            .map_err(|e| EjectError::Failed(format!("IOCTL_STORAGE_EJECT_MEDIA: {e}")))?;

        // El guard cierra el handle aquí (suelta el lock). Listo.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn drive_letter_parsea_raiz_con_letra() {
        assert_eq!(drive_letter(Path::new("E:\\")), Some('E'));
        assert_eq!(drive_letter(Path::new("c:\\")), Some('C'));
        assert_eq!(drive_letter(Path::new("Z:")), Some('Z'));
    }

    #[test]
    fn drive_letter_rechaza_rutas_no_unidad() {
        assert_eq!(drive_letter(Path::new("/")), None);
        assert_eq!(drive_letter(Path::new("\\\\server\\share")), None);
        assert_eq!(drive_letter(Path::new("")), None);
    }

    // En no-Windows el stub siempre rechaza con NotSupported.
    #[cfg(not(windows))]
    #[test]
    fn eject_no_soportado_fuera_de_windows() {
        assert_eq!(eject_drive(Path::new("/")), Err(EjectError::NotSupported));
    }

    // En Windows: expulsar una ruta inválida (no es "X:\\") debe dar InvalidDrive
    // SIN tocar ningún dispositivo real.
    #[cfg(windows)]
    #[test]
    fn eject_ruta_invalida_da_invalid_drive() {
        let res = eject_drive(Path::new("\\\\server\\share"));
        assert_eq!(res, Err(EjectError::InvalidDrive));
    }

    #[test]
    fn error_display_no_entra_en_panico() {
        // Ejercita el Display de cada variante (cobertura del tipo de error).
        for e in [
            EjectError::NotSupported,
            EjectError::InUse,
            EjectError::InvalidDrive,
            EjectError::Failed("x".into()),
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
