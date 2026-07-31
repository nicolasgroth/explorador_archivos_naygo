// Naygo — helpers de ventana Win32 (HWND, geometría guardada, restaurar desde bandeja).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Helpers de ventana que envuelven `naygo_platform::window*`: HWND de las ventanas Slint,
// restauración de geometría guardada (one-shot por sesión) y mostrar/traer al frente desde
// la bandeja, el hotkey global o la instancia única. Extraídos de `main.rs` sin cambio de
// comportamiento (refactor por tamaño de archivo).

use crate::workspace_ctrl::WorkspaceCtrl;
use crate::*;
use std::cell::RefCell;
use std::rc::Rc;

/// El HWND de la ventana de Naygo (backend winit), para el menú contextual del Shell.
/// `None` si no se puede obtener (otro backend) — entonces se oculta "Más opciones de
/// Windows…". Usa raw-window-handle vía el feature `raw-window-handle-06` de slint.
pub(crate) fn naygo_hwnd(ui: &AppWindow) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = ui.window().window_handle();
    match handle.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(isize::from(h.hwnd)),
        _ => None,
    }
}

/// Aplica la geometría de ventana guardada (tamaño/posición/maximizado) UNA vez por sesión, en
/// el primer momento en que la ventana está VISIBLE. El gate de visibilidad es la clave: en el
/// arranque directo a bandeja la ventana no se muestra al inicio, y aplicar la geometría antes
/// de tiempo o bien se pierde (sin HWND todavía) o bien LA MUESTRA sola (`SetWindowPlacement`
/// usa SW_SHOWNORMAL/SW_MAXIMIZE). Se llama desde el tick (≤30 ms tras cualquier show, salto
/// imperceptible) y desde `on_wake`; `done` es el one-shot compartido — se consume recién con
/// la ventana visible (haya o no geometría guardada: si no hay nada guardado, tampoco habrá
/// después dentro de la misma sesión).
///
/// Si el rect guardado no cae en ningún monitor conectado (monitor desconectado, coords viejas),
/// centra en el principal conservando el tamaño guardado en vez de descartarlo.
#[cfg(windows)]
pub(crate) fn try_restore_saved_geometry(
    ui: &AppWindow,
    ctrl: &Rc<RefCell<WorkspaceCtrl>>,
    done: &std::cell::Cell<bool>,
) {
    if done.get() || !ui.window().is_visible() {
        return;
    }
    done.set(true);
    let saved = ctrl.borrow().config.settings.window;
    if let (Some(g), Some(hwnd)) = (saved, naygo_hwnd(ui)) {
        let mons = naygo_platform::window_geometry::monitors();
        let placement = if g.is_visible_on(&mons) {
            naygo_platform::window_geometry::Placement {
                width: g.width,
                height: g.height,
                x: g.x,
                y: g.y,
                maximized: g.maximized,
            }
        } else {
            // Fuera de pantalla (monitor desconectado / coords inválidas): centrar en el
            // monitor principal conservando el tamaño guardado.
            let (mx, my, mw, mh) = mons.first().copied().unwrap_or((0, 0, 1920, 1080));
            // Clamp del tamaño restaurado a algo sano: un settings.json corrupto puede traer
            // width/height 0 o u32::MAX; sin clamp la resta de centrado haría overflow y podría
            // quedar una ventana 0x0 o gigante. Mínimo 200px, máximo el tamaño del monitor.
            let cw = (g.width).clamp(200, mw.max(200));
            let ch = (g.height).clamp(200, mh.max(200));
            let x = mx + ((mw as i32 - cw as i32) / 2).max(0);
            let y = my + ((mh as i32 - ch as i32) / 2).max(0);
            naygo_platform::window_geometry::Placement {
                width: cw,
                height: ch,
                x,
                y,
                maximized: g.maximized,
            }
        };
        naygo_platform::window_geometry::set(hwnd, placement);
        crate::logging::log_line("geometría de ventana restaurada");
    }
}

/// El HWND de la ventana de splash (backend winit), para poder elevarla al frente (topmost). Mismo
/// mecanismo que `naygo_hwnd` pero sobre la ventana `Splash`. Solo se usa en release (el splash está
/// tras `cfg(not(debug_assertions))`); `#[allow(dead_code)]` evita el warning en builds debug.
#[allow(dead_code)]
pub(crate) fn splash_hwnd(splash: &Splash) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = splash.window().window_handle();
    match handle.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(isize::from(h.hwnd)),
        _ => None,
    }
}

/// Restaura y trae al frente la ventana de Naygo: desde la bandeja (escondida por el
/// cierre-a-bandeja con `set_taskbar_visible(false)`), desde minimizada, o incluso NUNCA mostrada
/// (arranque directo a bandeja, donde main() no llama `ui.show()`). Devuelve el botón de la barra
/// de tareas, re-muestra la ventana, la des-minimiza y la trae al foreground. Idempotente: si el
/// botón ya estaba, `true` solo re-afirma el estilo. La usan el atajo global, la activación por
/// instancia única y todas las rutas del tray que "abren" la ventana, para que restaurar por
/// cualquier vía deje la ventana en un estado consistente (visible, con botón, al frente).
///
/// Nota: para ESCONDER se usa `set_taskbar_visible(false)` (SW_HIDE + WS_EX_TOOLWINDOW) y NO
/// `window().hide()`: `hide()` decrementa el contador interno de ventanas visibles de Slint y, si es
/// la única visible, dispara `quit_event_loop()` — terminaría la app en vez de dejarla en bandeja.
#[cfg(windows)]
pub(crate) fn restore_window(ui: &AppWindow) {
    // Devolver el botón de la barra ANTES de mostrar, si ya hay HWND (idempotente).
    if let Some(hwnd) = naygo_hwnd(ui) {
        naygo_platform::window::set_taskbar_visible(hwnd, true);
    }
    let _ = ui.show();
    ui.window().set_minimized(false);
    // El HWND puede haber NACIDO recién con el `show()` (ventana nunca mostrada: arranque directo
    // a bandeja, donde main() no llama `ui.show()`) — se re-consulta después de mostrar para
    // traerla al frente igual en ese primer despliegue.
    if let Some(hwnd) = naygo_hwnd(ui) {
        naygo_platform::window::bring_to_front(hwnd);
    }
}
#[cfg(not(windows))]
pub(crate) fn restore_window(ui: &AppWindow) {
    let _ = ui.show();
    ui.window().set_minimized(false);
}

/// Acción del atajo global: SIEMPRE muestra y trae Naygo al frente (no minimiza). El usuario quiere
/// "invocar Naygo desde cualquier lado", no un toggle: aunque la ventana ya esté al frente, re-afirmar
/// mostrar/traer-al-frente es inofensivo (y rescata el caso de estar tapada por otra app pero técnicamente
/// "foreground"). Antes esto minimizaba si la ventana ya era la foreground; se cambió a mostrar siempre.
#[cfg(windows)]
pub(crate) fn toggle_window_visibility(ui: &AppWindow, _tray_active: bool) {
    restore_window(ui);
}
#[cfg(not(windows))]
pub(crate) fn toggle_window_visibility(ui: &AppWindow, _tray_active: bool) {
    let _ = ui.show();
}
