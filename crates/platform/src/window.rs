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

/// Trae la ventana `hwnd` al frente y le da el foco del teclado de verdad. Lo necesita el flujo
/// de drag&drop: tras el bucle modal de `DoDragDrop` (OLE), la ventana de Naygo deja de ser la
/// *foreground window* del SO, así que el primer clic en un modal de operación solo REACTIVA la
/// ventana y no llega al botón. Llamando esto justo después de recibir el drop, la ventana ya
/// está activa cuando aparece el modal y el primer clic acciona el botón.
///
/// Windows niega `SetForegroundWindow` a un proceso que no posee el foco (regla anti-robo de
/// foco). El truco estándar para saltarla: ADJUNTAR temporalmente nuestra cola de entrada a la
/// del hilo que SÍ tiene el foreground (`AttachThreadInput`); mientras están adjuntas, el SO nos
/// trata como "el mismo input" y `SetForegroundWindow` sí toma efecto. Luego se desadjunta. Es lo
/// que hacen los gestores de ventanas para activar de verdad una ventana propia.
///
/// Tolerante por diseño (el SO es hostil): si `hwnd` es nulo no hace nada; todos los resultados
/// de las llamadas Win32 se ignoran a propósito — nunca paniquea. Si el foreground es nuestro
/// mismo hilo, o no hay foreground, se omite el attach. En no-Windows es un stub no-op.
#[cfg(windows)]
pub fn bring_to_front(hwnd: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };

    if hwnd == 0 {
        return;
    }
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    unsafe {
        // Hilo de la ventana que actualmente tiene el foreground, y el nuestro (el de la UI).
        let fg = GetForegroundWindow();
        let fg_thread = if fg.0.is_null() {
            0
        } else {
            // Sin necesitar el PID (segundo argumento None): solo queremos el id de hilo.
            GetWindowThreadProcessId(fg, None)
        };
        let our_thread = GetCurrentThreadId();

        // Solo adjuntamos si el foreground pertenece a OTRO hilo (adjuntar un hilo a sí mismo es
        // inválido). `fg_thread == 0` = no hay foreground válido: intentamos activar igual.
        let attach = fg_thread != 0 && fg_thread != our_thread;
        if attach {
            // El resultado (BOOL) se ignora: si el attach falla, el SetForegroundWindow de abajo
            // simplemente puede ser denegado, pero BringWindowToTop igual re-eleva la ventana.
            let _ = AttachThreadInput(our_thread, fg_thread, true);
        }

        let _ = SetForegroundWindow(hwnd);
        let _ = BringWindowToTop(hwnd);

        if attach {
            // Desadjuntar SIEMPRE que hayamos adjuntado, pase lo que pase con lo de arriba.
            let _ = AttachThreadInput(our_thread, fg_thread, false);
        }
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
