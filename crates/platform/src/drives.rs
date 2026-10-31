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

/// Enumeración Linux desde `/proc/self/mountinfo`. La función se invoca por el worker de
/// `workspace_ctrl::drive_io`; nunca debe llamarse desde el hilo de UI. Conserva las rutas en
/// bytes nativos y omite pseudofs del listado de accesos, sin impedir navegar hacia ellas por ruta.
#[cfg(target_os = "linux")]
pub fn drives() -> Vec<DriveInfo> {
    use std::fs::File;
    use std::io::BufReader;

    match File::open("/proc/self/mountinfo")
        .map(BufReader::new)
        .and_then(drives_from_mountinfo)
    {
        Ok(drives) if !drives.is_empty() => drives,
        Ok(_) => root_fallback(),
        Err(error) => {
            tracing::warn!(%error, "no se pudo enumerar mountinfo; se conserva la raíz Linux");
            root_fallback()
        }
    }
}

/// Stub para plataformas distintas de Windows/Linux: devuelve la raíz `/` como única "unidad".
#[cfg(not(any(windows, target_os = "linux")))]
pub fn drives() -> Vec<DriveInfo> {
    root_fallback()
}

#[cfg(not(windows))]
fn root_fallback() -> Vec<DriveInfo> {
    vec![DriveInfo {
        path: PathBuf::from("/"),
        label: "/".into(),
        kind: DriveKind::Fixed,
    }]
}

#[cfg(any(target_os = "linux", test))]
fn is_pseudo_filesystem(filesystem_type: &[u8]) -> bool {
    matches!(
        filesystem_type,
        b"autofs"
            | b"binfmt_misc"
            | b"bpf"
            | b"cgroup"
            | b"cgroup2"
            | b"configfs"
            | b"debugfs"
            | b"devpts"
            | b"devtmpfs"
            | b"efivarfs"
            | b"fusectl"
            | b"hugetlbfs"
            | b"mqueue"
            | b"nsfs"
            | b"proc"
            | b"pstore"
            | b"ramfs"
            | b"securityfs"
            | b"sysfs"
            | b"tracefs"
    )
}

#[cfg(any(target_os = "linux", test))]
fn mount_drive_kind(filesystem_type: &[u8]) -> DriveKind {
    if matches!(
        filesystem_type,
        b"9p" | b"afs" | b"cifs" | b"ceph" | b"nfs" | b"nfs4" | b"smb3"
    ) {
        DriveKind::Network
    } else {
        // Mountinfo alone cannot safely prove that a block device is removable. Keep the
        // generic icon and never offer eject until a platform-specific probe confirms it.
        DriveKind::Unknown
    }
}

#[cfg(target_os = "linux")]
fn drives_from_mountinfo<R: std::io::BufRead>(reader: R) -> std::io::Result<Vec<DriveInfo>> {
    use std::collections::BTreeMap;
    use std::os::unix::ffi::OsStringExt;

    let snapshot = super::mountinfo::parse_mountinfo(reader)?;
    if snapshot.malformed_lines > 0 {
        tracing::warn!(
            malformed_lines = snapshot.malformed_lines,
            "se omitieron registros inválidos de /proc/self/mountinfo"
        );
    }

    let mut by_mount_point = BTreeMap::new();
    for mount in snapshot.entries {
        if is_pseudo_filesystem(&mount.filesystem_type) {
            continue;
        }
        let mount_point = std::path::PathBuf::from(std::ffi::OsString::from_vec(mount.mount_point));
        if !mount_point.is_absolute() {
            continue;
        }
        let kind = mount_drive_kind(&mount.filesystem_type);
        let label = mount_point.to_string_lossy().into_owned();
        // A path may be mounted more than once. The last mountinfo record is the visible top
        // mount, so replacement keeps one toolbar/tree root per path.
        by_mount_point.insert(
            mount_point.clone(),
            DriveInfo {
                path: mount_point,
                label,
                kind,
            },
        );
    }
    Ok(by_mount_point.into_values().collect())
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

    #[test]
    fn linux_mount_policy_hides_pseudofs_and_marks_network_filesystems() {
        assert!(is_pseudo_filesystem(b"proc"));
        assert!(is_pseudo_filesystem(b"sysfs"));
        assert!(!is_pseudo_filesystem(b"ext4"));
        assert!(!is_pseudo_filesystem(b"fuse.sshfs"));
        assert_eq!(mount_drive_kind(b"cifs"), DriveKind::Network);
        assert_eq!(mount_drive_kind(b"nfs4"), DriveKind::Network);
        assert_eq!(mount_drive_kind(b"ext4"), DriveKind::Unknown);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_mount_enumeration_omits_pseudofs_and_keeps_network_mounts() {
        use std::io::Cursor;

        let text = concat!(
            "1 0 8:1 / / rw,relatime - ext4 /dev/sda1 rw\n",
            "2 1 0:1 / /proc rw,nosuid - proc proc rw\n",
            "3 1 0:42 / /mnt/share rw,relatime - cifs //server/share rw\n",
        );
        let mounts = drives_from_mountinfo(Cursor::new(text)).unwrap();
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[0].path, PathBuf::from("/"));
        assert_eq!(mounts[0].kind, DriveKind::Unknown);
        assert_eq!(mounts[1].path, PathBuf::from("/mnt/share"));
        assert_eq!(mounts[1].kind, DriveKind::Network);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_mount_enumeration_preserves_non_utf8_and_deduplicates_stacked_paths() {
        use std::io::Cursor;
        use std::os::unix::ffi::OsStrExt;

        let text = b"1 0 8:1 / /media rw - ext4 /dev/sda1 rw\n\
                     2 1 8:2 / /media rw - btrfs /dev/sdb1 rw\n\
                     3 1 8:3 / /mnt/name\\377 rw - ext4 /dev/sdc1 rw\n";
        let mounts = drives_from_mountinfo(Cursor::new(text)).unwrap();
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[0].path, PathBuf::from("/media"));
        assert_eq!(mounts[1].path.as_os_str().as_bytes(), b"/mnt/name\xff");
    }
}
