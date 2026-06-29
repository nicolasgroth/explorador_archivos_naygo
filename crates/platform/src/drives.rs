// Naygo — enumeración de unidades de disco del sistema (Win32, aislado).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `drives()` lista las unidades lógicas del equipo para las raíces del árbol.
//! Tolerante: una unidad que no responde se incluye igual (su expansión dará
//! error en el listado), no aborta la enumeración.

use naygo_core::icon_kind::DriveKind;
use std::path::PathBuf;

/// Una unidad de disco descubierta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriveInfo {
    /// Raíz de la unidad, p. ej. `C:\`.
    pub path: PathBuf,
    /// Etiqueta a mostrar (por ahora la raíz misma, p. ej. `C:\`).
    pub label: String,
    pub kind: DriveKind,
}

#[cfg(windows)]
pub fn drives() -> Vec<DriveInfo> {
    use windows::Win32::Storage::FileSystem::GetLogicalDriveStringsW;
    // Primera llamada con None devuelve el largo necesario (en u16 chars),
    // incluido el NUL final extra del bloque.
    let needed = unsafe { GetLogicalDriveStringsW(None) };
    if needed == 0 {
        return Vec::new();
    }
    // `+1` de holgura defensiva: la semántica exacta de si `needed` incluye el
    // NUL final es históricamente ambigua entre versiones de la API. El parseo no
    // depende de este largo (usa `written` de la 2ª llamada), así que sobra-asignar
    // un u16 es inofensivo.
    let mut buf = vec![0u16; needed as usize + 1];
    let written = unsafe { GetLogicalDriveStringsW(Some(&mut buf)) };
    if written == 0 {
        return Vec::new();
    }
    // El buffer es una lista de cadenas terminadas en NUL, con un NUL final extra.
    let mut out = Vec::new();
    for chunk in buf[..written as usize].split(|&c| c == 0) {
        if chunk.is_empty() {
            continue;
        }
        let root = String::from_utf16_lossy(chunk); // p. ej. "C:\\"
        let kind = drive_kind_of(&root);
        out.push(DriveInfo {
            path: PathBuf::from(&root),
            label: root,
            kind,
        });
    }
    out
}

/// Clasifica el tipo de una unidad por su raíz (p. ej. "C:\\").
///
/// `GetDriveTypeW` sola NO basta para "se puede expulsar": Windows reporta los discos
/// duros externos USB como `DRIVE_FIXED` (solo los pendrives suelen dar `DRIVE_REMOVABLE`).
/// Por eso, cuando el tipo es `Fixed`, además consultamos el BUS del dispositivo
/// (`IOCTL_STORAGE_QUERY_PROPERTY`): si está en bus USB, lo tratamos como `Removable` (el
/// sentido que le importa al usuario: "conviene expulsarlo antes de quitarlo").
#[cfg(windows)]
fn drive_kind_of(root: &str) -> DriveKind {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;
    use windows::Win32::System::WindowsProgramming::{
        DRIVE_CDROM, DRIVE_FIXED, DRIVE_RAMDISK, DRIVE_REMOTE, DRIVE_REMOVABLE,
    };
    let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
    let t = unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) };
    match t {
        // Un "fijo" en bus USB (disco externo) es, para el usuario, extraíble.
        DRIVE_FIXED => {
            if drive_letter_of(root).is_some_and(is_usb_bus) {
                DriveKind::Removable
            } else {
                DriveKind::Fixed
            }
        }
        DRIVE_REMOVABLE => DriveKind::Removable,
        DRIVE_REMOTE => DriveKind::Network,
        DRIVE_CDROM => DriveKind::Optical,
        DRIVE_RAMDISK => DriveKind::Fixed,
        _ => DriveKind::Unknown,
    }
}

/// Letra de una raíz ("C:\\" → 'C'); `None` si no parece raíz con letra.
#[cfg(windows)]
fn drive_letter_of(root: &str) -> Option<char> {
    let mut it = root.chars();
    let a = it.next()?;
    let b = it.next()?;
    (a.is_ascii_alphabetic() && b == ':').then(|| a.to_ascii_uppercase())
}

/// ¿La unidad `letter` está conectada por bus USB? Abre el volumen `\\.\X:` en modo
/// consulta (sin permisos de escritura → no requiere admin) y pide
/// `IOCTL_STORAGE_QUERY_PROPERTY`/`StorageDeviceProperty`; mira `BusType`. Tolerante:
/// cualquier fallo (no se pudo abrir, IOCTL no soportado) devuelve `false` (no es USB
/// hasta que se demuestre lo contrario), sin afectar la enumeración.
#[cfg(windows)]
fn is_usb_bus(letter: char) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        BusTypeUsb, CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows::Win32::System::Ioctl::{
        PropertyStandardQuery, StorageDeviceProperty, IOCTL_STORAGE_QUERY_PROPERTY,
        STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    let device = format!(r"\\.\{letter}:");
    let device_w: Vec<u16> = device.encode_utf16().chain(std::iter::once(0)).collect();
    // Abrir SIN GENERIC_READ/WRITE (acceso 0 = solo metadatos): así no pide admin ni
    // bloquea el volumen. Suficiente para IOCTL_STORAGE_QUERY_PROPERTY.
    // SAFETY: `device_w` es UTF-16 NUL-terminada viva durante la llamada.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(device_w.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    };
    let Ok(h) = handle else { return false };
    // RAII para cerrar el handle pase lo que pase.
    struct Guard(HANDLE);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
    let _guard = Guard(h);

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    // El descriptor es de tamaño variable; con el struct base alcanza para leer `BusType`
    // (está en la cabecera fija). Pedimos llenar lo que entre en el struct.
    let mut desc = STORAGE_DEVICE_DESCRIPTOR::default();
    let mut returned: u32 = 0;
    // SAFETY: query/desc viven durante la llamada; tamaños correctos de entrada/salida.
    let ok = unsafe {
        DeviceIoControl(
            h,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as *const core::ffi::c_void),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(&mut desc as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() as u32,
            Some(&mut returned as *mut u32),
            None,
        )
    };
    ok.is_ok() && desc.BusType == BusTypeUsb
}

/// Stub para plataformas no-Windows: devuelve la raíz `/` como única "unidad".
#[cfg(not(windows))]
pub fn drives() -> Vec<DriveInfo> {
    vec![DriveInfo {
        path: PathBuf::from("/"),
        label: "/".into(),
        kind: DriveKind::Fixed,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drives_devuelve_al_menos_una_unidad() {
        let d = drives();
        assert!(!d.is_empty(), "debe haber al menos una unidad");
        assert!(d.iter().all(|x| !x.path.as_os_str().is_empty()));
    }
}
