// Naygo — leer/aplicar geometría de la ventana principal (Win32) y enumerar monitores.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Lee/aplica la geometría de la ventana principal vía Win32 sobre su HWND, y enumera los
//! monitores conectados. Slint no da posición de forma portable; el HWND sí. Tolerante: si
//! algo falla, devuelve None y el llamador cae al tamaño por defecto.

/// (width, height, x, y, maximized) en px físicos. Misma forma que core::config::WindowGeometry
/// pero sin dependencia cruzada (platform no depende de core aquí).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

#[cfg(windows)]
mod windows_impl {
    use super::Placement;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, SetWindowPlacement, SW_MAXIMIZE, SW_SHOWNORMAL, WINDOWPLACEMENT,
    };

    /// Lee el placement de la ventana `hwnd`. El `rcNormalPosition` es el rect "restaurado"
    /// (des-maximizado), justo lo que queremos guardar. `showCmd == SW_MAXIMIZE` → maximizada.
    pub fn get(hwnd: isize) -> Option<Placement> {
        unsafe {
            let hwnd = HWND(hwnd as *mut core::ffi::c_void);
            let mut wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            GetWindowPlacement(hwnd, &mut wp).ok()?;
            let r = wp.rcNormalPosition;
            Some(Placement {
                width: (r.right - r.left).max(0) as u32,
                height: (r.bottom - r.top).max(0) as u32,
                x: r.left,
                y: r.top,
                maximized: wp.showCmd == SW_MAXIMIZE.0 as u32,
            })
        }
    }

    /// Aplica un placement a `hwnd`: fija el rect restaurado y, si corresponde, maximiza.
    pub fn set(hwnd: isize, p: Placement) {
        unsafe {
            let hwnd = HWND(hwnd as *mut core::ffi::c_void);
            let wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                showCmd: if p.maximized {
                    SW_MAXIMIZE.0 as u32
                } else {
                    SW_SHOWNORMAL.0 as u32
                },
                rcNormalPosition: RECT {
                    left: p.x,
                    top: p.y,
                    right: p.x + p.width as i32,
                    bottom: p.y + p.height as i32,
                },
                ..Default::default()
            };
            let _ = SetWindowPlacement(hwnd, &wp);
        }
    }

    /// Rects (x,y,w,h) de todos los monitores conectados.
    pub fn monitors() -> Vec<(i32, i32, u32, u32)> {
        unsafe extern "system" fn cb(m: HMONITOR, _dc: HDC, _r: *mut RECT, data: LPARAM) -> BOOL {
            let out = &mut *(data.0 as *mut Vec<(i32, i32, u32, u32)>);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(m, &mut mi).as_bool() {
                let r = mi.rcMonitor;
                out.push((
                    r.left,
                    r.top,
                    (r.right - r.left) as u32,
                    (r.bottom - r.top) as u32,
                ));
            }
            BOOL::from(true)
        }
        let mut out: Vec<(i32, i32, u32, u32)> = Vec::new();
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut out as *mut _ as isize));
        }
        out
    }
}

#[cfg(windows)]
pub use windows_impl::{get, monitors, set};

#[cfg(not(windows))]
pub fn get(_hwnd: isize) -> Option<Placement> {
    None
}
#[cfg(not(windows))]
pub fn set(_hwnd: isize, _p: Placement) {}
#[cfg(not(windows))]
pub fn monitors() -> Vec<(i32, i32, u32, u32)> {
    Vec::new()
}
