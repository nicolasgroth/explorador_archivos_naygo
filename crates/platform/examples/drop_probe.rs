// Naygo — receptor OLE aislado para diagnosticar arrastres con fixtures sintéticas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
// Solo observa el receptor existente; no copia el resultado ni cambia la configuración de Naygo.

#[cfg(not(windows))]
fn main() {
    eprintln!("Esta prueba requiere Windows.");
}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::sync::{mpsc, Arc};
    use windows::core::w;
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostMessageW,
        PostQuitMessage, RegisterClassW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
        CW_USEDEFAULT, MSG, SW_SHOW, WINDOW_EX_STYLE, WM_APP, WM_DESTROY, WNDCLASSW,
        WS_OVERLAPPEDWINDOW,
    };

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_DESTROY {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        } else {
            unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
        }
    }

    unsafe { OleInitialize(None)? };
    let instance = HINSTANCE(unsafe { GetModuleHandleW(None)? }.0);
    let class = w!("NaygoDropProbe");
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class,
        ..Default::default()
    };
    if unsafe { RegisterClassW(&wc) } == 0 {
        unsafe { OleUninitialize() };
        return Err(windows::core::Error::from_thread());
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("Naygo - prueba OLE (solo fixtures)"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            520,
            300,
            None,
            None,
            Some(instance),
            None,
        )?
    };
    let (tx, rx) = mpsc::channel();
    let (hover_tx, hover_rx) = mpsc::channel();
    let hwnd_raw = hwnd.0 as isize;
    let wake = Arc::new(move || {
        let _ = unsafe {
            PostMessageW(
                Some(HWND(hwnd_raw as *mut _)),
                WM_APP + 1,
                WPARAM(0),
                LPARAM(0),
            )
        };
    });
    let guard = naygo_platform::drop_target::register(hwnd_raw, tx, hover_tx, wake);
    if !guard.is_registered() {
        eprintln!("No se pudo registrar el receptor OLE");
        unsafe { OleUninitialize() };
        return Err(windows::core::Error::from_thread());
    }
    let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
    println!("probe_ready hwnd={hwnd_raw}; arrastrar solamente fixture sintética; cerrar la ventana para terminar");
    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
        if result <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        while hover_rx.try_recv().is_ok() {}
        while let Ok(payload) = rx.try_recv() {
            println!(
                "probe_drop paths={:?} error={:?} staged={} move={} copy_forced={}",
                payload.paths,
                payload.error,
                payload.staging.is_some(),
                payload.move_,
                payload.copy_forced
            );
        }
    }
    drop(guard);
    unsafe { OleUninitialize() };
    Ok(())
}
