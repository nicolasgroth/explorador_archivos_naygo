// Naygo — prueba aislada de subclass y mensajes en ventana Slint/winit.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
// No implementa captura de destino: no usar en producción.

#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
mod probe {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::ComponentHandle;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    slint::slint! {
        import { FilePanel } from "../ui/file-panel.slint";
        import { RowData } from "../ui/types.slint";
        export { RowData } from "../ui/types.slint";
        export component Probe inherits Window {
            title: "Naygo — prueba temporal de mensajes";
            width: 300px; height: 300px;
            in-out property <int> count: 0;
            in-out property <length> drag-y: -10000px;
            in-out property <int> row-hit: -2;
            in-out property <int> focus-row: -1;
            in-out property <int> restore-row: -1;
            in-out property <string> restore-key;
            in property <[RowData]> items;
            panel := FilePanel {
                width: parent.width; height: parent.height;
                pane-id: 7;
                rows: root.items;
                focused-row: root.focus-row;
                restore-scroll-row: root.restore-row;
                restore-scroll-key: root.restore-key;
                drag-client-y: root.drag-y;
                drop-target-row(id, row) => { root.row-hit = row; }
            }
            public function hit-row-at(y: length) -> int { return panel.drop-row-at(y); }
        }
    }

    type Hwnd = *mut std::ffi::c_void;
    type Callback = unsafe extern "system" fn(Hwnd, u32, usize, isize, usize, usize) -> isize;
    #[link(name = "comctl32")]
    unsafe extern "system" {
        fn SetWindowSubclass(h: Hwnd, cb: Callback, id: usize, data: usize) -> i32;
        fn RemoveWindowSubclass(h: Hwnd, cb: Callback, id: usize) -> i32;
        fn DefSubclassProc(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SendMessageTimeoutW(
            h: Hwnd,
            m: u32,
            w: usize,
            l: isize,
            flags: u32,
            timeout: u32,
            result: *mut usize,
        ) -> isize;
    }
    const PING: u32 = 0x8000 + 0x6e1;
    const REENTER: u32 = PING + 1;
    const SLOW: u32 = PING + 2;
    const ROWS: u32 = PING + 3;
    const SCROLLED_ROWS: u32 = PING + 4;
    thread_local! {
        static UI: std::cell::RefCell<Option<slint::Weak<Probe>>> = const { std::cell::RefCell::new(None) };
        static BUSY: Cell<bool> = const { Cell::new(false) };
    }
    static SLOW_DONE: AtomicBool = AtomicBool::new(false);
    // Fixture: petición compartida indexada; 0=pendiente, 1=respondida, 2=abandonada.
    // El callback conserva Arc incluso si quien espera ha retirado la entrada por timeout.
    static REQUEST: Mutex<Option<(usize, Arc<AtomicU8>)>> = Mutex::new(None);

    unsafe extern "system" fn subclass(
        h: Hwnd,
        m: u32,
        w: usize,
        l: isize,
        _: usize,
        _: usize,
    ) -> isize {
        match m {
            PING => {
                if BUSY.with(Cell::get) {
                    return 99;
                }
                UI.with(|slot| {
                    let Ok(slot) = slot.try_borrow() else {
                        return 0;
                    };
                    let Some(ui) = slot.as_ref().and_then(slint::Weak::upgrade) else {
                        return 0;
                    };
                    ui.set_count(ui.get_count() + 1);
                    42
                })
            }
            REENTER => {
                BUSY.with(|busy| busy.set(true));
                let mut result = 0;
                // Same UI thread: nested native message is processed synchronously.
                unsafe {
                    SendMessageTimeoutW(h, PING, 0, 0, 0x22, 1, &mut result);
                }
                BUSY.with(|busy| busy.set(false));
                result as isize
            }
            ROWS => UI.with(|slot| {
                let Ok(slot) = slot.try_borrow() else {
                    return 0;
                };
                let Some(ui) = slot.as_ref().and_then(slint::Weak::upgrade) else {
                    return 0;
                };
                let mut found = 0;
                for y in 0..1000 {
                    let row = ui.invoke_hit_row_at(y as f32);
                    if row == 0 {
                        found |= 1;
                    }
                    if row == 1 {
                        found |= 2;
                    }
                    if row == -1 {
                        found |= 4;
                    }
                    if row == -2 {
                        found |= 8;
                    }
                }
                found
            }),
            SCROLLED_ROWS => UI.with(|slot| {
                let Ok(slot) = slot.try_borrow() else {
                    return -1;
                };
                let Some(ui) = slot.as_ref().and_then(slint::Weak::upgrade) else {
                    return -1;
                };
                (0..300)
                    .map(|y| ui.invoke_hit_row_at(y as f32))
                    .max()
                    .unwrap_or(-1) as isize
            }),
            SLOW => {
                let request = REQUEST.try_lock().ok().and_then(|slot| {
                    slot.as_ref()
                        .filter(|(id, _)| *id == w)
                        .map(|(_, state)| state.clone())
                });
                std::thread::sleep(Duration::from_millis(250));
                if let Some(state) = request {
                    let _ = state.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
                }
                SLOW_DONE.store(true, Ordering::Release);
                7
            }
            _ => unsafe { DefSubclassProc(h, m, w, l) },
        }
    }

    pub fn run() {
        let backend = i_slint_backend_winit::Backend::new_with_renderer_by_name(Some("software"))
            .expect("backend winit");
        slint::platform::set_platform(Box::new(backend)).expect("configurar backend");
        let ui = Probe::new().expect("ventana Slint");
        ui.set_items(slint::ModelRc::new(slint::VecModel::from(vec![
            RowData::default(),
            RowData::default(),
        ])));
        ui.show().expect("mostrar ventana de prueba");
        let hwnd_slot = std::rc::Rc::new(Cell::new(0isize));
        let hwnd_for_timer = hwnd_slot.clone();
        let worker_slot = std::rc::Rc::new(std::cell::RefCell::new(None));
        let worker_for_timer = worker_slot.clone();
        let weak = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(200), move || {
            let ui = weak.upgrade().expect("ventana viva");
            let handle = ui.window().window_handle();
            let RawWindowHandle::Win32(h) = handle.window_handle().expect("HWND").as_raw() else {
                panic!("backend no Win32")
            };
            let hwnd = isize::from(h.hwnd);
            hwnd_for_timer.set(hwnd);
            UI.with(|slot| *slot.borrow_mut() = Some(ui.as_weak()));
            assert_ne!(
                unsafe { SetWindowSubclass(hwnd as Hwnd, subclass, 1, 0) },
                0
            );
            let weak_for_scroll = ui.as_weak();
            *worker_for_timer.borrow_mut() = Some(std::thread::spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut result = 0;
                    assert_ne!(
                        unsafe {
                            SendMessageTimeoutW(hwnd as Hwnd, PING, 0, 0, 0x22, 2000, &mut result)
                        },
                        0
                    );
                    assert_eq!(result, 42);
                    assert_ne!(
                        unsafe {
                            SendMessageTimeoutW(
                                hwnd as Hwnd,
                                REENTER,
                                0,
                                0,
                                0x22,
                                2000,
                                &mut result,
                            )
                        },
                        0
                    );
                    assert_eq!(result, 99);
                    assert_ne!(
                        unsafe {
                            SendMessageTimeoutW(hwnd as Hwnd, ROWS, 0, 0, 0x22, 2000, &mut result)
                        },
                        0
                    );
                    println!("file_panel_sync_hit_mask={result}");
                    assert_eq!(result, 15, "filas, zona vacía y fuera del viewport");
                    slint::invoke_from_event_loop(move || {
                        let ui = weak_for_scroll.upgrade().expect("ventana viva");
                        ui.set_items(slint::ModelRc::new(slint::VecModel::from(
                            vec![RowData::default(); 40],
                        )));
                        ui.set_restore_row(30);
                        ui.set_restore_key("scroll-test".into());
                        ui.window().request_redraw();
                    })
                    .expect("actualizar fixture");
                    std::thread::sleep(Duration::from_millis(500));
                    assert_ne!(
                        unsafe {
                            SendMessageTimeoutW(
                                hwnd as Hwnd,
                                SCROLLED_ROWS,
                                0,
                                0,
                                0x22,
                                2000,
                                &mut result,
                            )
                        },
                        0
                    );
                    println!("file_panel_scrolled_max_row={result}");
                    assert!(
                        result >= 30,
                        "el hit-test debe reflejar el viewport desplazado"
                    );
                    let request = Arc::new(AtomicU8::new(0));
                    *REQUEST.lock().expect("registro") = Some((5, request.clone()));
                    assert_eq!(
                        unsafe {
                            SendMessageTimeoutW(hwnd as Hwnd, SLOW, 5, 0, 0x22, 30, &mut result)
                        },
                        0
                    );
                    assert!(request
                        .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok());
                    *REQUEST.lock().expect("registro") = None;
                    assert!(!SLOW_DONE.load(Ordering::Acquire));
                    while !SLOW_DONE.load(Ordering::Acquire) {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    assert_eq!(request.load(Ordering::Acquire), 2);
                    assert_eq!(Arc::strong_count(&request), 1);
                }));
                let _ = slint::quit_event_loop();
                if let Err(error) = outcome {
                    std::panic::resume_unwind(error);
                }
            }));
        });
        slint::run_event_loop().expect("event loop");
        worker_slot
            .borrow_mut()
            .take()
            .expect("worker iniciado")
            .join()
            .expect("worker correcto");
        let hwnd = hwnd_slot.get();
        assert_ne!(hwnd, 0);
        assert_eq!(ui.get_count(), 1);
        assert_ne!(
            unsafe { RemoveWindowSubclass(hwnd as Hwnd, subclass, 1) },
            0
        );
        UI.with(|slot| *slot.borrow_mut() = None);
        ui.hide().expect("ocultar");
        println!("winit_subclass=ok; slint_property=1; reentrancy=rejected; timeout_and_late_callback=ok; subclass_removed=ok");
    }
}

#[cfg(windows)]
fn main() {
    probe::run();
}
