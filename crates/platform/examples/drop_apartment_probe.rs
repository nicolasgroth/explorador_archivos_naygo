// Naygo — sondeo COM: callback no ágil registrado en STA distinto al dueño del HWND.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
// No realiza un arrastre de Windows: invoca una interfaz marshaleada de manera sintética.

#[cfg(not(windows))]
fn main() {
    eprintln!("Solo Windows");
}

#[cfg(windows)]
mod win {
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        mpsc, Arc,
    };
    use std::time::{Duration, Instant};
    use windows::core::{implement, w, Interface, Ref};
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINTL, WPARAM};
    use windows::Win32::System::Com::Marshal::CoMarshalInterThreadInterfaceInStream;
    use windows::Win32::System::Com::StructuredStorage::CoGetInterfaceAndReleaseStream;
    use windows::Win32::System::Com::{IDataObject, IStream};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Ole::{
        IDropTarget, IDropTarget_Impl, OleInitialize, OleUninitialize, RegisterDragDrop,
        RevokeDragDrop, DROPEFFECT, DROPEFFECT_COPY,
    };
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        PeekMessageW, PostThreadMessageW, RegisterClassW, UnregisterClassW, MSG, PM_REMOVE,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_QUIT, WNDCLASSW,
    };

    #[implement(IDropTarget, Agile = false)]
    struct Target {
        observed: Arc<AtomicU32>,
    }
    impl IDropTarget_Impl for Target_Impl {
        fn DragEnter(
            &self,
            _: Ref<IDataObject>,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            _: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            self.observed
                .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
            Ok(())
        }
        fn DragOver(
            &self,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            _: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn DragLeave(&self) -> windows::core::Result<()> {
            Ok(())
        }
        fn Drop(
            &self,
            _: Ref<IDataObject>,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            _: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            Ok(())
        }
    }
    #[implement(IDropTarget)]
    struct AgileTarget {
        observed: Arc<AtomicU32>,
    }
    impl IDropTarget_Impl for AgileTarget_Impl {
        fn DragEnter(
            &self,
            _: Ref<IDataObject>,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            _: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            self.observed
                .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
            Ok(())
        }
        fn DragOver(
            &self,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            _: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            Ok(())
        }
        fn DragLeave(&self) -> windows::core::Result<()> {
            Ok(())
        }
        fn Drop(
            &self,
            _: Ref<IDataObject>,
            _: MODIFIERKEYS_FLAGS,
            _: &POINTL,
            _: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            Ok(())
        }
    }
    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }
    pub fn run() -> windows::core::Result<()> {
        let agile = std::env::args().any(|arg| arg == "--agile");
        unsafe { OleInitialize(None)? };
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None)? }.0);
        let class = w!("NaygoCrossStaApartmentProbe");
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
        let raw_hwnd = hwnd.0 as isize;
        let observed = Arc::new(AtomicU32::new(0));
        let observed_worker = observed.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let tid = unsafe { GetCurrentThreadId() };
            let result = (|| -> windows::core::Result<usize> {
                unsafe { OleInitialize(None)? };
                let target: IDropTarget = if agile {
                    AgileTarget {
                        observed: observed_worker,
                    }
                    .into()
                } else {
                    Target {
                        observed: observed_worker,
                    }
                    .into()
                };
                unsafe {
                    RegisterDragDrop(HWND(raw_hwnd as *mut _), &target)?;
                }
                let stream =
                    unsafe { CoMarshalInterThreadInterfaceInStream(&IDropTarget::IID, &target)? };
                Ok(stream.into_raw() as usize)
            })();
            let success = result.is_ok();
            let _ = ready_tx.send((tid, result));
            if success {
                let mut message = MSG::default();
                while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                    unsafe { DispatchMessageW(&message) };
                }
                let _ = unsafe { RevokeDragDrop(HWND(raw_hwnd as *mut _)) };
            }
            unsafe { OleUninitialize() };
            let _ = done_tx.send(());
        });
        let mut message = MSG::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        let (worker_tid, raw) = loop {
            if let Ok(result) = ready_rx.try_recv() {
                break result;
            }
            while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe { DispatchMessageW(&message) };
            }
            if Instant::now() >= deadline {
                eprintln!("registration_timeout");
                std::process::exit(2);
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        let raw = raw?;
        let stream = unsafe { IStream::from_raw(raw as *mut _) };
        let proxy: windows::core::Result<IDropTarget> =
            unsafe { CoGetInterfaceAndReleaseStream(&stream) };
        std::mem::forget(stream);
        let proxy = proxy?;
        let mut effect = DROPEFFECT_COPY;
        unsafe {
            proxy.DragEnter(
                None::<&IDataObject>,
                MODIFIERKEYS_FLAGS(0),
                POINTL { x: 0, y: 0 },
                &mut effect,
            )?
        };
        let callback_tid = observed.load(Ordering::SeqCst);
        println!("agile={agile} window_thread={ui_tid} registration_thread={worker_tid} synthetic_callback_thread={callback_tid}");
        let expected_callback = if agile { ui_tid } else { worker_tid };
        if callback_tid != expected_callback || worker_tid == ui_tid {
            eprintln!("unexpected_callback_thread");
            std::process::exit(4);
        }
        drop(proxy);
        unsafe { PostThreadMessageW(worker_tid, WM_QUIT, WPARAM(0), LPARAM(0))? };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if done_rx.try_recv().is_ok() {
                break;
            }
            while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe { DispatchMessageW(&message) };
            }
            if Instant::now() >= deadline {
                eprintln!("revoke_timeout");
                std::process::exit(3);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        worker.join().expect("worker OLE falló");
        unsafe {
            DestroyWindow(hwnd)?;
            UnregisterClassW(class, Some(instance))?;
            OleUninitialize();
        }
        Ok(())
    }
}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    win::run()
}
