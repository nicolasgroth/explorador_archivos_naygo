// Naygo — consulta los íconos asociados a tipos de archivo en Windows Shell.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Acceso acotado a los íconos de asociaciones de Windows. Las consultas reciben solo una
//! extensión y usan `SHGFI_USEFILEATTRIBUTES`; nunca inspeccionan una ruta real ni leen su
//! contenido. El llamador debe ejecutarlas desde un worker, ya que Shell puede bloquear.

/// Imagen pequeña del Shell, convertida a RGBA para que la UI no retenga handles Win32.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemFileIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Obtiene el ícono asociado a una extensión en la lista pequeña de Windows Shell.
/// `extension` no lleva punto. Devuelve `None` si la asociación o la conversión GDI falla.
#[cfg(windows)]
pub fn for_extension(extension: &str) -> Option<SystemFileIcon> {
    use std::{mem::size_of, os::windows::ffi::OsStrExt};
    use windows::Win32::{
        Graphics::Gdi::{
            CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
            SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
        },
        Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        UI::{
            Shell::{
                SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON, SHGFI_USEFILEATTRIBUTES,
            },
            WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL},
        },
    };

    const ICON_EDGE: i32 = 16;
    const PIXEL_BYTES: usize = (ICON_EDGE * ICON_EDGE * 4) as usize;

    if extension.is_empty()
        || extension.len() > 128
        || extension
            .chars()
            .any(|ch| ch == '\0' || ch == '/' || ch == '\\' || ch == '.')
    {
        return None;
    }

    let path: Vec<u16> = std::ffi::OsStr::new(&format!(".{extension}"))
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut shell_info = SHFILEINFOW::default();
    let result = unsafe {
        SHGetFileInfoW(
            windows::core::PCWSTR(path.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut shell_info),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES,
        )
    };
    if result == 0 || shell_info.hIcon.is_invalid() {
        return None;
    }

    let screen_dc = unsafe { GetDC(None) };
    if screen_dc.is_invalid() {
        unsafe {
            let _ = DestroyIcon(shell_info.hIcon);
        }
        return None;
    }

    let bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: ICON_EDGE,
            biHeight: -ICON_EDGE,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: PIXEL_BYTES as u32,
            ..BITMAPINFOHEADER::default()
        },
        ..BITMAPINFO::default()
    };
    let mut bits = std::ptr::null_mut();
    let bitmap = unsafe {
        CreateDIBSection(
            Some(screen_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )
    };

    let Ok(bitmap) = bitmap else {
        unsafe {
            let _ = ReleaseDC(None, screen_dc);
            let _ = DestroyIcon(shell_info.hIcon);
        }
        return None;
    };
    if bits.is_null() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = ReleaseDC(None, screen_dc);
            let _ = DestroyIcon(shell_info.hIcon);
        }
        return None;
    }

    let memory_dc = unsafe { CreateCompatibleDC(Some(screen_dc)) };
    if memory_dc.is_invalid() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = ReleaseDC(None, screen_dc);
            let _ = DestroyIcon(shell_info.hIcon);
        }
        return None;
    }
    let old_object = unsafe { SelectObject(memory_dc, HGDIOBJ(bitmap.0)) };
    let draw_result = unsafe {
        std::slice::from_raw_parts_mut(bits.cast::<u8>(), PIXEL_BYTES).fill(0);
        DrawIconEx(
            memory_dc,
            0,
            0,
            shell_info.hIcon,
            ICON_EDGE,
            ICON_EDGE,
            0,
            None,
            DI_NORMAL,
        )
    };
    let rgba = if draw_result.is_ok() {
        let bgra = unsafe { std::slice::from_raw_parts(bits.cast::<u8>(), PIXEL_BYTES) };
        let mut rgba = Vec::with_capacity(PIXEL_BYTES);
        for pixel in bgra.chunks_exact(4) {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
        Some(rgba)
    } else {
        None
    };
    unsafe {
        let _ = SelectObject(memory_dc, old_object);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        let _ = ReleaseDC(None, screen_dc);
        let _ = DestroyIcon(shell_info.hIcon);
    }
    let rgba = rgba?;
    Some(SystemFileIcon {
        width: ICON_EDGE as u32,
        height: ICON_EDGE as u32,
        rgba,
    })
}

/// No hay una asociación de Windows fuera de Windows; la UI oculta este ajuste en otros targets.
#[cfg(not(windows))]
pub fn for_extension(_extension: &str) -> Option<SystemFileIcon> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    use super::for_extension;

    #[test]
    fn gets_small_icon_without_opening_the_file() {
        let icon = for_extension("txt").expect("Windows should provide the .txt association icon");
        assert_eq!((icon.width, icon.height), (16, 16));
        assert_eq!(icon.rgba.len(), 16 * 16 * 4);
        assert!(icon.rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }

    #[test]
    fn rejects_path_syntax_in_extension() {
        assert!(for_extension("..\\secret").is_none());
        assert!(for_extension("").is_none());
    }
}
