// Naygo — proveedor de metadata para ejecutables (versión de archivo/producto), Win32.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Lee la versión de archivo embebida en el recurso `VERSIONINFO` de un `.exe`/`.dll` vía la
//! API de versión de Win32 (`GetFileVersionInfoSizeW` → `GetFileVersionInfoW` →
//! `VerQueryValueW`). Tolerante: cualquier fallo (archivo sin recurso de versión, ruta
//! inexistente, disco caído) devuelve una lista vacía; nunca panic.

use naygo_core::metadata::{MetadataField, MetadataProvider};
use std::path::Path;

pub struct ExeMeta;

impl MetadataProvider for ExeMeta {
    fn extensions(&self) -> &'static [&'static str] {
        &["exe", "dll"]
    }

    fn read(&self, path: &Path) -> Vec<MetadataField> {
        #[cfg(windows)]
        {
            windows_impl::read_version(path)
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            Vec::new()
        }
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
    };

    /// Convierte una ruta a UTF-16 terminado en NUL, como esperan las funciones de Win32.
    fn wide_path(path: &std::path::Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Firma esperada de `VS_FIXEDFILEINFO` (constante documentada de la API de versión).
    const VS_FFI_SIGNATURE: u32 = 0xFEEF_04BD;

    /// Lee la versión de archivo del recurso `VERSIONINFO` de `path`. Vacío ante cualquier
    /// fallo o si la versión reportada es `0.0.0.0` (no aporta información).
    pub fn read_version(path: &std::path::Path) -> Vec<MetadataField> {
        let wpath = wide_path(path);
        let pwpath = PCWSTR(wpath.as_ptr());

        // 1) Tamaño del bloque de versión. 0 = el archivo no tiene recurso de versión.
        let mut handle: u32 = 0;
        let size = unsafe { GetFileVersionInfoSizeW(pwpath, Some(&mut handle)) };
        if size == 0 {
            return Vec::new();
        }

        // 2) Reserva el buffer y lee el bloque completo.
        let mut buffer: Vec<u8> = vec![0u8; size as usize];
        let ok = unsafe {
            GetFileVersionInfoW(
                pwpath,
                None,
                size,
                buffer.as_mut_ptr() as *mut core::ffi::c_void,
            )
        };
        if ok.is_err() {
            return Vec::new();
        }

        // 3) Pide el bloque raíz, que contiene un VS_FIXEDFILEINFO.
        let root: Vec<u16> = "\\".encode_utf16().chain(std::iter::once(0)).collect();
        let mut info_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut info_len: u32 = 0;
        let ok = unsafe {
            VerQueryValueW(
                buffer.as_ptr() as *const core::ffi::c_void,
                PCWSTR(root.as_ptr()),
                &mut info_ptr,
                &mut info_len,
            )
        };
        if !ok.as_bool() || info_ptr.is_null() {
            return Vec::new();
        }
        if (info_len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>() {
            return Vec::new();
        }

        // 4) Castea y valida la firma antes de confiar en los campos.
        let fixed = unsafe { &*(info_ptr as *const VS_FIXEDFILEINFO) };
        if fixed.dwSignature != VS_FFI_SIGNATURE {
            return Vec::new();
        }

        let major = (fixed.dwFileVersionMS >> 16) & 0xFFFF;
        let minor = fixed.dwFileVersionMS & 0xFFFF;
        let build = (fixed.dwFileVersionLS >> 16) & 0xFFFF;
        let revision = fixed.dwFileVersionLS & 0xFFFF;

        if major == 0 && minor == 0 && build == 0 && revision == 0 {
            return Vec::new();
        }

        vec![MetadataField {
            label_key: "meta.file_version",
            value: format!("{major}.{minor}.{build}.{revision}"),
        }]
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn no_panica_con_archivo_inexistente() {
        let fields = ExeMeta.read(Path::new(r"C:\ruta\que\no\existe.exe"));
        assert!(fields.is_empty());
    }

    #[test]
    fn version_conocida_tiene_formato_valido_si_hay_resultado() {
        // Tolerante: si notepad.exe no existe en este entorno (CI atípico) o no trae recurso
        // de versión, simplemente no hay nada que validar. Lo único que nos importa es que
        // nunca panica y que, si hay resultado, tiene el formato "a.b.c.d".
        let fields = ExeMeta.read(Path::new(r"C:\Windows\System32\notepad.exe"));
        if let Some(f) = fields.iter().find(|f| f.label_key == "meta.file_version") {
            let parts: Vec<&str> = f.value.split('.').collect();
            assert_eq!(parts.len(), 4, "formato esperado a.b.c.d: {}", f.value);
            for p in parts {
                assert!(
                    p.parse::<u32>().is_ok(),
                    "cada parte debe ser numérica: {p}"
                );
            }
        }
    }
}
