// Naygo — helpers de ventana Win32 (conversión de coordenadas). Aislado en platform.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Utilidades de ventana que tocan Win32, mantenidas en `platform` para respetar el
//! aislamiento de capas: la UI nunca llama a `windows::Win32::*` directo.
//!
//! Por ahora solo expone [`screen_to_client`]: convierte un punto de PANTALLA (físico) a
//! coordenadas de CLIENTE (físicas) de una ventana. Lo necesita el drag&drop OLE: el SO
//! entrega el punto del drop en coords de pantalla, pero el hit-testing de paneles trabaja
//! en coords de contenido (un sistema anclado al área de cliente). `ScreenToClient` descuenta
//! el marco y la barra de título nativos del SO de un solo golpe; restar manualmente el origen
//! exterior de la ventana (lo que devuelve `window().position()`) es incorrecto porque ese
//! origen incluye el borde/título nativos.

/// Convierte el punto de PANTALLA `(screen_x, screen_y)` (píxeles físicos) a coordenadas de
/// CLIENTE (píxeles físicos) de la ventana `hwnd` mediante `ScreenToClient`. El origen del
/// área de cliente queda bajo el marco y la barra de título nativos del SO, así que el
/// resultado ya no incluye ese desfase.
///
/// Devuelve `None` si `hwnd` es nulo o si la llamada al SO falla (p. ej. handle inválido) —
/// el llamador debe caer a un plan B, nunca paniquear (el filesystem y el SO son hostiles).
/// En no-Windows es un stub que siempre devuelve `None`.
#[cfg(windows)]
pub fn screen_to_client(hwnd: isize, screen_x: i32, screen_y: i32) -> Option<(i32, i32)> {
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::Graphics::Gdi::ScreenToClient;

    if hwnd == 0 {
        return None;
    }
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    let mut pt = POINT {
        x: screen_x,
        y: screen_y,
    };
    // ScreenToClient modifica `pt` in-place y devuelve FALSE si falla.
    let ok = unsafe { ScreenToClient(hwnd, &mut pt).as_bool() };
    if ok {
        Some((pt.x, pt.y))
    } else {
        None
    }
}

/// Stub no-Windows: la conversión de coordenadas de ventana no existe fuera de Windows.
#[cfg(not(windows))]
pub fn screen_to_client(_hwnd: isize, _screen_x: i32, _screen_y: i32) -> Option<(i32, i32)> {
    None
}

/// Trae la ventana `hwnd` al frente y le da el foco del teclado (`SetForegroundWindow` +
/// `BringWindowToTop`). Lo necesita el flujo de drag&drop: tras el bucle modal de
/// `DoDragDrop` (OLE), la ventana de Naygo deja de ser la *foreground window* del SO, así que
/// el primer clic en un modal de operación solo REACTIVA la ventana y no llega al botón.
/// Llamando esto justo después de recibir el drop, la ventana ya está al frente cuando aparece
/// el modal y el primer clic acciona el botón.
///
/// Tolerante: si `hwnd` es nulo no hace nada. `SetForegroundWindow` devuelve un BOOL que el SO
/// puede poner en FALSE si el proceso no tiene derecho a robar el foco (regla anti-robo de
/// foco de Windows); se ignora a propósito — nunca paniquea. En la práctica funciona aquí
/// porque Naygo ERA el foreground antes del arrastre (el usuario arrastró desde su panel), y
/// `BringWindowToTop` ayuda a re-elevar la ventana aun cuando el cambio de foreground sea
/// denegado. En no-Windows es un stub no-op.
#[cfg(windows)]
pub fn bring_to_front(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{BringWindowToTop, SetForegroundWindow};

    if hwnd == 0 {
        return;
    }
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    // Ambas pueden ser ignoradas por el SO; no nos importa el resultado, solo el efecto.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);
    }
}

/// Stub no-Windows: traer la ventana al frente es específico de Win32.
#[cfg(not(windows))]
pub fn bring_to_front(_hwnd: isize) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// HWND nulo → `None` (sin tocar el SO; evita pasar un handle inválido a Win32).
    #[test]
    fn screen_to_client_hwnd_nulo_es_none() {
        assert_eq!(screen_to_client(0, 100, 200), None);
    }
}
