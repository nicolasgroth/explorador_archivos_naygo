// Naygo — espacio libre/total de una unidad, aislado por plataforma.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `read_space` consulta el espacio TOTAL y LIBRE del volumen que contiene una ruta.
//! Puede tardar en discos de red/ópticos, así que se llama DESDE un worker (nunca en
//! el hilo de UI). Tolerante: `None` si la unidad no responde. El cálculo de % usado
//! vive en `naygo_core::disk::DiskUsage`.

use std::path::Path;

/// Devuelve `(total_bytes, free_bytes)` de la unidad que contiene `root`, o `None`
/// si la consulta falla (unidad caída, óptico vacío, ruta inválida).
#[cfg(windows)]
pub fn read_space(root: &Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let root_w: Vec<u16> = root
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free_to_caller: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;
    // SAFETY: PCWSTR a cadena NUL-terminada; los out-params son punteros a u64
    // válidos en la pila que la API rellena durante la llamada.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(root_w.as_ptr()),
            Some(&mut free_to_caller as *mut u64),
            Some(&mut total as *mut u64),
            Some(&mut total_free as *mut u64),
        )
    };
    match ok {
        Ok(()) => Some((total, total_free)),
        Err(_) => None,
    }
}

/// Linux: statvfs consulta directamente el sistema de archivos de la ruta dada.
/// Recibe la carpeta actual del panel, no la raíz `/`, para funcionar también en
/// volúmenes montados. Se ejecuta en el worker del footer; una ruta desmontada o
/// inaccesible devuelve `None` y no afecta la navegación.
#[cfg(target_os = "linux")]
pub fn read_space(root: &Path) -> Option<(u64, u64)> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let path = CString::new(root.as_os_str().as_bytes()).ok()?;
    // SAFETY: `path` es NUL-terminada y `stats` apunta a memoria válida para que
    // statvfs escriba el resultado durante la llamada.
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    // SAFETY: statvfs devolvió 0, por lo que inicializó la estructura de salida.
    let stats = unsafe { stats.assume_init() };
    let block_size = if stats.f_frsize == 0 {
        c_ulong_to_u64(stats.f_bsize)
    } else {
        c_ulong_to_u64(stats.f_frsize)
    };
    let total = c_ulong_to_u64(stats.f_blocks).checked_mul(block_size)?;
    // bavail excluye los bloques reservados que no están disponibles para el usuario.
    let available = c_ulong_to_u64(stats.f_bavail).checked_mul(block_size)?;
    Some((total, available.min(total)))
}

/// `c_ulong` mide 32 o 64 bits según la ABI Unix; ambos caben en u64.
#[cfg(target_os = "linux")]
#[allow(clippy::unnecessary_cast)]
fn c_ulong_to_u64(value: libc::c_ulong) -> u64 {
    value as u64
}

/// Stub para plataformas sin consulta de espacio implementada.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn read_space(_root: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // Smoke test manual: consulta el espacio real de C:\. Queda `#[ignore]` para no
    // depender del entorno en CI. Correr con:
    //   cargo test -p naygo-platform read_space_smoke -- --ignored --test-threads=1 --nocapture
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn read_space_smoke() {
        let res = read_space(Path::new("C:\\"));
        let (total, free) = res.expect("read_space devolvió None para C:\\");
        println!("read_space(C:\\) = total {total} bytes, free {free} bytes");
        assert!(total > 0, "el total debe ser > 0");
        assert!(free <= total, "el libre no puede superar el total");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn read_space_linux_informa_espacio_del_volumen() {
        let (total, available) = read_space(Path::new("/")).expect("statvfs de / debe responder");
        assert!(total > 0, "el total debe ser > 0");
        assert!(
            available <= total,
            "el disponible no puede superar el total"
        );
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    #[test]
    fn stub_plataforma_no_implementada_no_informa_espacio() {
        assert_eq!(read_space(Path::new("/")), None);
    }
}
