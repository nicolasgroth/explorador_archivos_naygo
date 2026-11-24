// Naygo — sondeo aislado de registro OLE desde un STA distinto al dueño de la ventana.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
// No interactúa con ventanas existentes ni con archivos del usuario.

#[cfg(not(windows))]
fn main() {
    eprintln!("Solo Windows");
}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};
    use windows::core::w;
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        PeekMessageW, PostThreadMessageW, RegisterClassW, UnregisterClassW, MSG, PM_REMOVE,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_QUIT, WNDCLASSW,
    };

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }
    unsafe { OleInitialize(None)? };
    let instance = HINSTANCE(unsafe { GetModuleHandleW(None)? }.0);
    let class = w!("NaygoCrossStaRegistrationProbe");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: instance,
        lpszClassName: class,
        ..Default::default()
    };
    if unsafe { RegisterClassW(&wc) } == 0 {
        return Err(windows::core::Error::from_thread());
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!(""),
            WINDOW_STYLE::default(),
            0,
            0,
            20,
            20,
            None,
            None,
            Some(instance),
            None,
        )?
    };
    let ui_tid = unsafe { GetCurrentThreadId() };
    let hwnd_raw = hwnd.0 as isize;
    let (status_tx, status_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let (drop_tx, _drop_rx) = mpsc::channel();
    let (hover_tx, _hover_rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let tid = unsafe { GetCurrentThreadId() };
        // register() llama OleInitialize; el guard debe revocarse en este mismo STA.
        let guard =
            naygo_platform::drop_target::register(hwnd_raw, drop_tx, hover_tx, Arc::new(|| {}));
        let registered = guard.is_registered();
        let _ = status_tx.send((tid, registered));
        if registered {
            let mut message = MSG::default();
            while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                unsafe { DispatchMessageW(&message) };
            }
        }
        drop(guard);
        unsafe { OleUninitialize() };
        let _ = done_tx.send(());
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut message = MSG::default();
    let result = loop {
        if let Ok(result) = status_rx.try_recv() {
            break result;
        }
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe { DispatchMessageW(&message) };
        }
        if Instant::now() >= deadline {
            eprintln!("registration_timeout; proceso de prueba terminará");
            std::process::exit(2);
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    println!(
        "window_thread={ui_tid} registration_thread={} registered={}",
        result.0, result.1
    );
    if result.1 {
        unsafe { PostThreadMessageW(result.0, WM_QUIT, WPARAM(0), LPARAM(0))? };
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if done_rx.try_recv().is_ok() {
            break;
        }
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe { DispatchMessageW(&message) };
        }
        if Instant::now() >= deadline {
            eprintln!("revoke_timeout; proceso de prueba terminará");
            std::process::exit(3);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    worker.join().expect("falló el worker OLE");
    unsafe {
        DestroyWindow(hwnd)?;
        UnregisterClassW(class, Some(instance))?;
        OleUninitialize();
    }
    if !result.1 {
        return Err(windows::core::Error::empty());
    }
    Ok(())
}
