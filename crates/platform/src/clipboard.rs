// Naygo — portapapeles del SO (Win32: CF_HDROP, CF_DIB, CF_UNICODETEXT), aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lee y escribe el portapapeles del sistema. La lógica de QUÉ hacer con el contenido
//! vive en `core::clipboard`; aquí solo está la frontera Win32. Tolerante: cualquier
//! fallo de lectura → `Empty`; la escritura devuelve `Result`. No tumba el proceso.

use naygo_core::clipboard::ClipboardContent;
use std::path::PathBuf;

/// Error al escribir el portapapeles.
#[derive(Debug)]
pub enum ClipboardError {
    NotSupported,
    Failed(String),
}

#[cfg(not(windows))]
pub fn read() -> ClipboardContent {
    ClipboardContent::Empty
}

#[cfg(not(windows))]
pub fn write_files(_paths: &[PathBuf], _cut: bool) -> Result<(), ClipboardError> {
    Err(ClipboardError::NotSupported)
}

#[cfg(windows)]
pub fn read() -> ClipboardContent {
    windows_impl::read()
}

#[cfg(windows)]
pub fn write_files(paths: &[PathBuf], cut: bool) -> Result<(), ClipboardError> {
    windows_impl::write_files(paths, cut)
}

#[cfg(windows)]
pub(crate) mod windows_impl {
    use super::ClipboardError;
    use naygo_core::clipboard::{ClipboardContent, ClipboardImage, MAX_IMAGE_PIXELS};
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;

    use windows::core::w;
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
    use windows::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_BITFIELDS, BI_RGB};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
        OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
    use windows::Win32::UI::Shell::{DragQueryFileW, DROPFILES, HDROP};

    // DROPEFFECT_* viven en System::Ole / System::SystemServices según la versión.
    // Los definimos localmente para no depender del módulo exacto: son valores estables.
    const DROPEFFECT_COPY: u32 = 1;
    const DROPEFFECT_MOVE: u32 = 2;

    /// Guard RAII: cierra el portapapeles en CUALQUIER salida (incluido panic).
    struct ClipboardGuard;
    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            // SAFETY: solo se construye tras un OpenClipboard exitoso; el cierre es
            // idempotente desde el punto de vista de balance abrir/cerrar de este hilo.
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    /// Abre el portapapeles con reintentos (otro proceso puede tenerlo un instante).
    /// Devuelve un guard que lo cerrará al salir del scope, o `None` si no se pudo abrir.
    fn open_clipboard() -> Option<ClipboardGuard> {
        for attempt in 0..5 {
            // SAFETY: OpenClipboard con HWND nulo asocia el portapapeles a este hilo.
            let opened = unsafe { OpenClipboard(Some(HWND::default())) };
            if opened.is_ok() {
                return Some(ClipboardGuard);
            }
            if attempt < 4 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        None
    }

    pub fn read() -> ClipboardContent {
        let _guard = match open_clipboard() {
            Some(g) => g,
            None => return ClipboardContent::Empty,
        };

        // Orden de prioridad: archivos → imagen → texto.
        // SAFETY: el portapapeles está abierto para este hilo mientras viva `_guard`.
        unsafe {
            if IsClipboardFormatAvailable(CF_HDROP.0 as u32).is_ok() {
                if let Some(files) = read_hdrop() {
                    return files;
                }
            }
            if IsClipboardFormatAvailable(CF_DIB.0 as u32).is_ok() {
                if let Some(img) = read_dib() {
                    return img;
                }
            }
            if IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_ok() {
                if let Some(text) = read_text() {
                    return text;
                }
            }
        }
        ClipboardContent::Empty
    }

    /// Lee CF_HDROP (lista de archivos) + el Preferred DropEffect (copy/cut).
    /// SAFETY: el llamador garantiza que CF_HDROP está disponible y el portapapeles abierto.
    unsafe fn read_hdrop() -> Option<ClipboardContent> {
        let handle = GetClipboardData(CF_HDROP.0 as u32).ok()?;
        if handle.0.is_null() {
            return None;
        }
        let hdrop = HDROP(handle.0);

        // 0xFFFFFFFF → número de archivos.
        let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
        let mut paths = Vec::with_capacity(count as usize);
        for i in 0..count {
            // Primera llamada con buffer vacío: devuelve la longitud (sin el NUL).
            let len = DragQueryFileW(hdrop, i, None);
            if len == 0 {
                continue;
            }
            // +1 para el NUL terminador que escribe la API.
            let mut buf = vec![0u16; len as usize + 1];
            let written = DragQueryFileW(hdrop, i, Some(buf.as_mut_slice()));
            if written == 0 {
                continue;
            }
            buf.truncate(written as usize);
            paths.push(PathBuf::from(String::from_utf16_lossy(&buf)));
        }

        if paths.is_empty() {
            return None;
        }

        let cut = read_preferred_drop_effect().unwrap_or(false);
        Some(ClipboardContent::Files { paths, cut })
    }

    /// Lee "Preferred DropEffect" → `true` si es MOVE (cortar).
    /// SAFETY: portapapeles abierto por el llamador.
    unsafe fn read_preferred_drop_effect() -> Option<bool> {
        let fmt = RegisterClipboardFormatW(w!("Preferred DropEffect"));
        if fmt == 0 || IsClipboardFormatAvailable(fmt).is_err() {
            return None;
        }
        let handle = GetClipboardData(fmt).ok()?;
        if handle.0.is_null() {
            return None;
        }
        let hglobal = HGLOBAL(handle.0);
        let ptr = GlobalLock(hglobal) as *const u32;
        if ptr.is_null() {
            return None;
        }
        let effect = std::ptr::read_unaligned(ptr);
        let _ = GlobalUnlock(hglobal);
        Some((effect & DROPEFFECT_MOVE) != 0)
    }

    /// Desplazamiento de bits a la derecha equivalente al primer bit activo de `mask`,
    /// y nº de bits significativos: permite extraer un canal de un píxel empaquetado y
    /// escalarlo a 8 bits. `mask == 0` → canal ausente.
    fn mask_shift_and_width(mask: u32) -> (u32, u32) {
        if mask == 0 {
            return (0, 0);
        }
        let shift = mask.trailing_zeros();
        let width = (mask >> shift).count_ones();
        (shift, width)
    }

    /// Extrae un canal de `value` según `mask` y lo escala al rango 0..=255.
    fn extract_channel(value: u32, mask: u32, shift: u32, bits: u32) -> u8 {
        if mask == 0 || bits == 0 {
            return 0;
        }
        let raw = (value & mask) >> shift;
        let max = (1u32 << bits) - 1;
        // Escalado redondeado a 8 bits.
        ((raw * 255 + max / 2) / max) as u8
    }

    /// Lee CF_DIB y lo convierte a RGBA8. Soporta BI_RGB (24/32 bpp) y BI_BITFIELDS
    /// (32 bpp con máscaras de color, el caso que Windows sintetiza desde 32bpp). El
    /// resto (paletizados, RLE, BITMAPV4/V5 con datos exóticos) → None (cae a texto).
    /// SAFETY: CF_DIB disponible y portapapeles abierto.
    unsafe fn read_dib() -> Option<ClipboardContent> {
        let handle = GetClipboardData(CF_DIB.0 as u32).ok()?;
        if handle.0.is_null() {
            return None;
        }
        let hglobal = HGLOBAL(handle.0);
        let base = GlobalLock(hglobal) as *const u8;
        if base.is_null() {
            return None;
        }
        // Tamaño REAL del bloque del portapapeles. El portapapeles es entrada hostil:
        // un DIB malformado puede declarar dimensiones que excedan el buffer. Validamos
        // contra este tamaño antes de leer un solo píxel (evita lecturas fuera de rango).
        let buf_size = GlobalSize(hglobal);

        // Guard local para garantizar el GlobalUnlock pase lo que pase.
        struct Unlocker(HGLOBAL);
        impl Drop for Unlocker {
            fn drop(&mut self) {
                unsafe {
                    let _ = GlobalUnlock(self.0);
                }
            }
        }
        let _unlock = Unlocker(hglobal);

        let header_size = std::mem::size_of::<BITMAPINFOHEADER>();
        // El buffer debe contener al menos el header antes de leerlo (entrada hostil).
        if buf_size < header_size {
            return None;
        }
        let header = std::ptr::read_unaligned(base as *const BITMAPINFOHEADER);

        // Solo entendemos headers tipo BITMAPINFOHEADER o mayores (biSize >= 40).
        if (header.biSize as usize) < header_size {
            return None;
        }

        let bpp = header.biBitCount as u32;
        let is_rgb = header.biCompression == BI_RGB.0;
        let is_bitfields = header.biCompression == BI_BITFIELDS.0;

        // BI_RGB acepta 24 y 32 bpp; BI_BITFIELDS solo lo tratamos a 32 bpp.
        if !((is_rgb && (bpp == 24 || bpp == 32)) || (is_bitfields && bpp == 32)) {
            return None;
        }

        let width = header.biWidth;
        if width <= 0 {
            return None;
        }
        let height_raw = header.biHeight;
        if height_raw == 0 {
            return None;
        }
        let top_down = height_raw < 0;
        let height = height_raw.unsigned_abs();
        let width_u = width as u32;

        let pixels = width_u as u64 * height as u64;
        if pixels == 0 || pixels > MAX_IMAGE_PIXELS {
            return None;
        }

        // Máscaras de canal. En BI_BITFIELDS vienen 3 u32 (R,G,B) justo tras el header;
        // en BI_RGB usamos las máscaras estándar BGRA en memoria.
        let (mask_r, mask_g, mask_b);
        let mut pixels_offset = header.biSize as usize;
        if is_bitfields {
            // Las 3 máscaras (12 bytes) deben caber en el buffer antes de leerlas.
            if (header.biSize as usize).checked_add(3 * std::mem::size_of::<u32>())? > buf_size {
                return None;
            }
            let masks = base.add(header.biSize as usize) as *const u32;
            mask_r = std::ptr::read_unaligned(masks);
            mask_g = std::ptr::read_unaligned(masks.add(1));
            mask_b = std::ptr::read_unaligned(masks.add(2));
            // Las 3 máscaras (12 bytes) preceden a los píxeles solo si el header es el
            // de 40 bytes; los headers V4/V5 ya las incluyen dentro de biSize.
            if (header.biSize as usize) == header_size {
                pixels_offset += 3 * std::mem::size_of::<u32>();
            }
            if mask_r == 0 || mask_g == 0 || mask_b == 0 {
                return None;
            }
        } else {
            // BI_RGB en memoria es BGR(A): B en bits 0..7, G 8..15, R 16..23.
            mask_b = 0x0000_00FF;
            mask_g = 0x0000_FF00;
            mask_r = 0x00FF_0000;
        }

        let (sh_r, w_r) = mask_shift_and_width(mask_r);
        let (sh_g, w_g) = mask_shift_and_width(mask_g);
        let (sh_b, w_b) = mask_shift_and_width(mask_b);

        let bytes_per_px = (bpp / 8) as usize;
        // Stride alineado a 4 bytes (regla de los DIB).
        let stride = ((width_u as usize * bytes_per_px) + 3) & !3usize;

        // Validación de límites contra el tamaño REAL del buffer (entrada hostil): el
        // bloque debe contener el offset de píxeles + todas las filas declaradas. Si el
        // header miente y el buffer es más corto, abortamos en vez de leer fuera de rango.
        let needed = pixels_offset.checked_add((height as usize).checked_mul(stride)?)?;
        if needed > buf_size {
            return None;
        }

        let mut rgba = vec![0u8; pixels as usize * 4];
        let pix_base = base.add(pixels_offset);

        for y in 0..height {
            // bottom-up (default): la primera fila del buffer es la inferior de la imagen.
            let src_row = if top_down { y } else { height - 1 - y };
            let row = pix_base.add(src_row as usize * stride);
            let dst_row_start = y as usize * width_u as usize * 4;
            for x in 0..width_u {
                let src = row.add(x as usize * bytes_per_px);
                // Leer el píxel como u32 (24bpp: solo 3 bytes).
                let value = if bytes_per_px == 4 {
                    std::ptr::read_unaligned(src as *const u32)
                } else {
                    (*src as u32) | ((*src.add(1) as u32) << 8) | ((*src.add(2) as u32) << 16)
                };
                let r = extract_channel(value, mask_r, sh_r, w_r);
                let g = extract_channel(value, mask_g, sh_g, w_g);
                let b = extract_channel(value, mask_b, sh_b, w_b);
                let dst = dst_row_start + x as usize * 4;
                rgba[dst] = r;
                rgba[dst + 1] = g;
                rgba[dst + 2] = b;
                // No interpretamos alfa: las máscaras de clipboard de 32bpp casi siempre
                // dejan el 4º byte sin definir. Tratamos la imagen como opaca.
                rgba[dst + 3] = 255;
            }
        }

        Some(ClipboardContent::Image(ClipboardImage {
            width: width_u,
            height,
            rgba,
        }))
    }

    /// Lee CF_UNICODETEXT (UTF-16 NUL-terminado).
    /// SAFETY: CF_UNICODETEXT disponible y portapapeles abierto.
    unsafe fn read_text() -> Option<ClipboardContent> {
        let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
        if handle.0.is_null() {
            return None;
        }
        let hglobal = HGLOBAL(handle.0);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            return None;
        }
        // Recorrer hasta el NUL.
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let s = String::from_utf16_lossy(slice);
        let _ = GlobalUnlock(hglobal);
        Some(ClipboardContent::Text(s))
    }

    pub fn write_files(paths: &[PathBuf], cut: bool) -> Result<(), ClipboardError> {
        if paths.is_empty() {
            return Err(ClipboardError::Failed("lista de rutas vacía".into()));
        }

        let _guard = open_clipboard()
            .ok_or_else(|| ClipboardError::Failed("no se pudo abrir el portapapeles".into()))?;

        // SAFETY: portapapeles abierto para este hilo mientras viva `_guard`.
        unsafe {
            EmptyClipboard().map_err(|e| ClipboardError::Failed(format!("EmptyClipboard: {e}")))?;

            // 1) CF_HDROP: DROPFILES + bloque UTF-16 doble-NUL-terminado.
            let hdrop = build_hdrop_global(paths)?;
            match SetClipboardData(CF_HDROP.0 as u32, Some(HANDLE(hdrop.0))) {
                Ok(_) => { /* el sistema es dueño del HGLOBAL; NO liberar. */ }
                Err(e) => {
                    let _ = GlobalFree(Some(hdrop));
                    return Err(ClipboardError::Failed(format!(
                        "SetClipboardData(CF_HDROP): {e}"
                    )));
                }
            }

            // 2) Preferred DropEffect (copy/cut).
            let effect: u32 = if cut {
                DROPEFFECT_MOVE
            } else {
                DROPEFFECT_COPY
            };
            let fmt = RegisterClipboardFormatW(w!("Preferred DropEffect"));
            if fmt != 0 {
                if let Some(hglobal) = alloc_u32(effect) {
                    if SetClipboardData(fmt, Some(HANDLE(hglobal.0))).is_err() {
                        let _ = GlobalFree(Some(hglobal));
                        // No es fatal: los archivos siguen pegables; el destino asumirá copy.
                    }
                }
            }
        }

        Ok(())
    }

    /// Construye el HGLOBAL con la estructura DROPFILES + rutas UTF-16 doble-NUL.
    ///
    /// Es el formato **CF_HDROP**: lo usa tanto el portapapeles (`write_files`) como el
    /// arrastre OLE hacia el SO (`dnd::start_drag`). `pub(crate)` para reutilizarlo desde
    /// `dnd.rs` sin duplicar la construcción de DROPFILES. El llamador es dueño del HGLOBAL
    /// devuelto: debe entregarlo a una API que lo consuma (SetClipboardData / STGMEDIUM con
    /// `pUnkForRelease == null`, que el SO libera con `GlobalFree`) o liberarlo él mismo.
    ///
    /// SAFETY: escribe en memoria recién asignada por GlobalAlloc.
    pub(crate) unsafe fn build_hdrop_global(paths: &[PathBuf]) -> Result<HGLOBAL, ClipboardError> {
        // Bloque de rutas: cada ruta UTF-16 + NUL, y un NUL extra al final.
        let mut block: Vec<u16> = Vec::new();
        for p in paths {
            block.extend(p.as_os_str().encode_wide());
            block.push(0); // NUL por ruta
        }
        block.push(0); // NUL final (lista doble-NUL-terminada)

        let dropfiles_size = std::mem::size_of::<DROPFILES>();
        let block_bytes = block.len() * std::mem::size_of::<u16>();
        let total = dropfiles_size + block_bytes;

        let hglobal = GlobalAlloc(GMEM_MOVEABLE, total)
            .map_err(|e| ClipboardError::Failed(format!("GlobalAlloc: {e}")))?;

        let base = GlobalLock(hglobal) as *mut u8;
        if base.is_null() {
            let _ = GlobalFree(Some(hglobal));
            return Err(ClipboardError::Failed("GlobalLock devolvió null".into()));
        }

        // Header DROPFILES.
        let df = DROPFILES {
            pFiles: dropfiles_size as u32, // offset al primer carácter de la lista
            pt: Default::default(),
            fNC: false.into(),
            fWide: true.into(), // rutas en UTF-16
        };
        std::ptr::write_unaligned(base as *mut DROPFILES, df);

        // Bloque de rutas justo tras el header.
        let dst = base.add(dropfiles_size) as *mut u16;
        std::ptr::copy_nonoverlapping(block.as_ptr(), dst, block.len());

        let _ = GlobalUnlock(hglobal);
        Ok(hglobal)
    }

    /// Asigna un HGLOBAL de 4 bytes con un `u32`. Devuelve None si falla.
    /// SAFETY: escribe en memoria recién asignada.
    unsafe fn alloc_u32(value: u32) -> Option<HGLOBAL> {
        let hglobal = GlobalAlloc(GMEM_MOVEABLE, std::mem::size_of::<u32>()).ok()?;
        let ptr = GlobalLock(hglobal) as *mut u32;
        if ptr.is_null() {
            let _ = GlobalFree(Some(hglobal));
            return None;
        }
        std::ptr::write_unaligned(ptr, value);
        let _ = GlobalUnlock(hglobal);
        Some(hglobal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::clipboard::ClipboardContent;

    // Roundtrip de archivos por el portapapeles del SO. El portapapeles es un recurso
    // GLOBAL compartido por todo el escritorio: este test es inestable bajo ejecución
    // paralela y puede chocar con lo que el usuario tenga copiado. Por eso #[ignore];
    // correrlo explícito y en serie:
    //   cargo test -p naygo-platform clipboard_roundtrip_files -- --ignored --test-threads=1
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn clipboard_roundtrip_files() {
        let dir = std::env::temp_dir().join(format!("naygo_clip_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("pegame.txt");
        std::fs::write(&f, b"x").unwrap();

        write_files(std::slice::from_ref(&f), false).expect("write_files falló");

        match read() {
            ClipboardContent::Files { paths, cut } => {
                assert!(!cut, "esperaba copy, no cut");
                assert!(
                    paths.iter().any(|p| p == &f),
                    "el portapapeles no contiene {f:?}; trae {paths:?}"
                );
            }
            other => panic!("esperaba Files, vino {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
