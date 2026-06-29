// Naygo — espacio libre/total de una unidad (Win32 GetDiskFreeSpaceExW), aislado.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `read_space` consulta el espacio TOTAL y LIBRE de una unidad. Puede tardar en
//! discos de red/ópticos, así que se llama DESDE un worker (nunca en el hilo de UI).
//! Tolerante: `None` si la unidad no responde. El cálculo de % usado vive en
//! `naygo_core::disk::DiskUsage`.

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

/// Stub no-Windows: sin información de espacio.
#[cfg(not(windows))]
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
}
