// Naygo — enumeración de unidades de disco del sistema (Win32, aislado).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

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
        DRIVE_FIXED => DriveKind::Fixed,
        DRIVE_REMOVABLE => DriveKind::Removable,
        DRIVE_REMOTE => DriveKind::Network,
        DRIVE_CDROM => DriveKind::Optical,
        DRIVE_RAMDISK => DriveKind::Fixed,
        _ => DriveKind::Unknown,
    }
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
