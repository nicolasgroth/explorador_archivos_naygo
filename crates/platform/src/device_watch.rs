// Naygo — detección de dispositivos (ventana message-only Win32, WM_DEVICECHANGE), aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Crea, EN SU PROPIO HILO, una ventana oculta `HWND_MESSAGE` que escucha
//! `WM_DEVICECHANGE` (llegada/quita de volúmenes) y emite `DeviceEvent::DrivesChanged`.
//! NO toca el HWND de eframe (aislamiento): la ventana es message-only, sin pintar nada,
//! y corre su propio bucle de mensajes en un hilo dedicado. Al dropear el handle, postea
//! `WM_CLOSE` y une el hilo. Tolerante: si la ventana no se crea, el handle queda inerte.

use std::sync::mpsc::Sender;

/// Waker para despertar la UI tras enviar un evento: la UI está DORMIDA en reposo (no
/// repinta sin motivo, clave para el bajo consumo en VMs sin GPU). Esta vigilancia corre
/// en su propio hilo, así que necesita un `Fn() + Send + Sync` (típicamente
/// `egui::Context::request_repaint`) para sacarla del sueño. `platform` no depende de egui.
pub type Waker = std::sync::Arc<dyn Fn() + Send + Sync>;

/// Evento de cambio de dispositivos. Por ahora un único caso: el conjunto de
/// volúmenes/unidades cambió (USB conectado/desconectado, montaje/desmontaje).
/// La capa superior reacciona re-escaneando las unidades.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceEvent {
    /// Cambió el conjunto de unidades disponibles. No dice cuál ni cómo: el
    /// consumidor debe re-escanear (el evento es solo un disparador).
    DrivesChanged,
}

/// Handle de la vigilancia de dispositivos. Mantiene vivo el hilo + la ventana
/// message-only mientras exista; al dropearse, los detiene limpiamente.
pub struct DeviceWatchHandle {
    // Solo se mantiene vivo por su `Drop` (RAII): desmonta el hilo + la ventana al
    // soltarse. Nunca se lee, de ahí el `allow(dead_code)`.
    #[cfg(windows)]
    #[allow(dead_code)]
    inner: Option<windows_impl::Inner>,
    #[cfg(not(windows))]
    _priv: (),
}

/// Stub no-Windows: la detección de dispositivos por mensajes del SO no existe
/// fuera de Windows. El handle queda inerte (no llegarán eventos).
#[cfg(not(windows))]
pub fn watch(_tx: Sender<DeviceEvent>, _waker: Waker) -> DeviceWatchHandle {
    DeviceWatchHandle { _priv: () }
}

/// Empieza a vigilar cambios de dispositivos. Lanza un hilo dedicado con una
/// ventana message-only que escucha `WM_DEVICECHANGE` y emite `DrivesChanged`, despertando
/// la UI con `waker`. Si la ventana no puede crearse (caso raro), el handle queda inerte.
#[cfg(windows)]
pub fn watch(tx: Sender<DeviceEvent>, waker: Waker) -> DeviceWatchHandle {
    DeviceWatchHandle {
        inner: windows_impl::start(tx, waker),
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{DeviceEvent, Waker};
    use std::ffi::c_void;
    use std::sync::mpsc::{sync_channel, Sender};
    use std::sync::Once;
    use std::thread::JoinHandle;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
        PostMessageW, PostQuitMessage, RegisterClassW, SetWindowLongPtrW, TranslateMessage,
        CREATESTRUCTW, DBT_DEVICEARRIVAL, DBT_DEVICEREMOVECOMPLETE, GWLP_USERDATA, HWND_MESSAGE,
        MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WM_DEVICECHANGE, WM_NCCREATE,
        WNDCLASSW,
    };

    /// Nombre de la clase de ventana (wide, NUL-terminado). Único para Naygo para no
    /// chocar con otras clases del proceso. La clase se registra una sola vez por proceso.
    const CLASS_NAME: PCWSTR = windows::core::w!("naygo_device_watch_wndclass");

    /// Garantiza que `RegisterClassW` se llame una única vez por proceso, aunque
    /// `watch()` se invoque varias veces (registrar dos veces la misma clase falla).
    static REGISTER_ONCE: Once = Once::new();

    /// Estado interno vivo: el hilo del bucle de mensajes + el HWND (como bits) para
    /// poder postearle `WM_CLOSE` desde `Drop`.
    pub(super) struct Inner {
        thread: Option<JoinHandle<()>>,
        hwnd_bits: isize,
    }

    /// Registra la clase de ventana (idempotente vía `Once`). No propaga error: si
    /// `RegisterClassW` falla, `CreateWindowExW` fallará después y se reporta ahí.
    fn ensure_class_registered() {
        REGISTER_ONCE.call_once(|| {
            // SAFETY: GetModuleHandleW(None) devuelve el módulo del proceso actual.
            // El WNDCLASSW se llena con punteros válidos (CLASS_NAME es estático).
            unsafe {
                let hinstance = match GetModuleHandleW(None) {
                    Ok(h) => h,
                    Err(e) => {
                        tracing::warn!(error = %e, "device_watch: GetModuleHandleW falló");
                        return;
                    }
                };
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(wndproc),
                    hInstance: hinstance.into(),
                    lpszClassName: CLASS_NAME,
                    ..Default::default()
                };
                let atom = RegisterClassW(&wc);
                if atom == 0 {
                    tracing::warn!("device_watch: RegisterClassW devolvió 0 (clase no registrada)");
                }
            }
        });
    }

    /// Carga boxeada que vive en `GWLP_USERDATA`: el `Sender` para emitir el evento y el
    /// `Waker` para despertar la UI tras emitirlo.
    type Payload = (Sender<DeviceEvent>, Waker);

    /// Procedimiento de ventana. Maneja el ciclo de vida del `Payload` boxeado (guardado
    /// en `GWLP_USERDATA`) y traduce `WM_DEVICECHANGE` en `DeviceEvent::DrivesChanged`.
    ///
    /// SAFETY: invocado por el SO con un HWND válido. El puntero en GWLP_USERDATA es el
    /// `Box<Payload>` que pasamos como `lpCreateParams`; lo creamos en WM_NCCREATE y lo
    /// liberamos en WM_DESTROY, así que entre medio es válido y exclusivo de esta ventana.
    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_NCCREATE => {
                // `lparam` apunta a un CREATESTRUCTW; su `lpCreateParams` es el Box<Payload>
                // crudo que pasamos a CreateWindowExW. Lo guardamos en GWLP_USERDATA.
                let cs = lparam.0 as *const CREATESTRUCTW;
                if !cs.is_null() {
                    let payload_ptr = (*cs).lpCreateParams as *mut Payload;
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, payload_ptr as isize);
                }
                // Devolver TRUE explícito tras guardar el puntero: si DefWindowProcW
                // devolviera FALSE aquí, la creación abortaría SIN enviar WM_DESTROY y el
                // Box<Sender> ya guardado se filtraría. TRUE garantiza que la creación
                // continúe y que la liberación ocurra en WM_DESTROY.
                let _ = DefWindowProcW(hwnd, msg, wparam, lparam);
                LRESULT(1)
            }
            WM_DEVICECHANGE => {
                // Solo nos interesan llegada/quita completas de un dispositivo. wparam trae
                // el subevento; el conjunto de unidades pudo cambiar -> disparar re-escaneo.
                if wparam.0 as u32 == DBT_DEVICEARRIVAL
                    || wparam.0 as u32 == DBT_DEVICEREMOVECOMPLETE
                {
                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Payload;
                    if !ptr.is_null() {
                        let (tx, waker) = &*ptr;
                        // Si el receptor se cayó, ignorar (la app puede estar cerrando).
                        let _ = tx.send(DeviceEvent::DrivesChanged);
                        // Despertar la UI: estaba dormida, debe re-escanear las unidades.
                        waker();
                    }
                }
                // TRUE: concedemos el cambio (relevante para subeventos de query, no para
                // arrival/removecomplete, pero devolver TRUE es lo correcto/inocuo aquí).
                LRESULT(1)
            }
            WM_DESTROY => {
                // Liberar el Box<Sender> que guardamos en WM_NCCREATE y cerrar el bucle.
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if ptr != 0 {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(ptr as *mut Payload));
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    /// Lanza el hilo con la ventana message-only. Devuelve `Some(Inner)` si la ventana
    /// se creó y el bucle arrancó; `None` si la creación falló (handle inerte).
    pub(super) fn start(tx: Sender<DeviceEvent>, waker: Waker) -> Option<Inner> {
        // Canal para que el hilo nos devuelva el HWND (como isize). `Some(bits)` en éxito,
        // `None` si CreateWindowExW falló. sync_channel(1) basta: un único envío.
        let (hwnd_tx, hwnd_rx) = sync_channel::<Option<isize>>(1);

        let thread = std::thread::Builder::new()
            .name("naygo-device-watch".to_string())
            .spawn(move || {
                ensure_class_registered();

                // El Box<Payload> (Sender + Waker) viaja como lpCreateParams; WM_NCCREATE se
                // hace dueño de él.
                let payload_box: Box<Payload> = Box::new((tx, waker));
                let create_params = Box::into_raw(payload_box) as *const c_void;

                // SAFETY: clase ya registrada (o se reporta su falla); HWND_MESSAGE crea una
                // ventana message-only (sin UI, aislada del HWND de eframe). hinstance del módulo.
                let hwnd = unsafe {
                    let hinstance = GetModuleHandleW(None).ok().map(|h| h.into());
                    CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        CLASS_NAME,
                        PCWSTR::null(),
                        WINDOW_STYLE(0),
                        0,
                        0,
                        0,
                        0,
                        Some(HWND_MESSAGE),
                        None,
                        hinstance,
                        Some(create_params),
                    )
                };

                let hwnd = match hwnd {
                    Ok(h) if !h.0.is_null() => h,
                    other => {
                        // CreateWindowExW falló. Si WM_NCCREATE NO llegó a ejecutarse, el Box
                        // sigue siendo nuestro: hay que liberarlo para no filtrarlo.
                        tracing::warn!(
                            ?other,
                            "device_watch: CreateWindowExW falló; handle inerte"
                        );
                        // SAFETY: recuperamos el Box solo si la ventana no se creó (no hubo
                        // WM_NCCREATE que se apropiara del puntero).
                        unsafe {
                            drop(Box::from_raw(create_params as *mut Payload));
                        }
                        let _ = hwnd_tx.send(None);
                        return;
                    }
                };

                // Avisar al hilo padre el HWND ANTES de entrar al bucle (Drop lo necesita).
                let _ = hwnd_tx.send(Some(hwnd.0 as isize));

                // Bucle de mensajes. GetMessageW devuelve FALSE al recibir WM_QUIT
                // (lo postea WM_DESTROY), terminando el bucle limpiamente.
                // SAFETY: msg es un buffer válido; pasamos None como hwnd para recibir
                // todos los mensajes de este hilo (la ventana message-only incluida).
                unsafe {
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .ok()?;

        // Esperar el HWND (o el fallo) del hilo. Si el hilo no envía (panic al spawnear el
        // cuerpo), recv falla -> handle inerte, y unimos el hilo igualmente.
        match hwnd_rx.recv() {
            Ok(Some(bits)) => Some(Inner {
                thread: Some(thread),
                hwnd_bits: bits,
            }),
            _ => {
                // Ventana no creada (o canal cerrado): el hilo ya terminó o terminará solo.
                let _ = thread.join();
                None
            }
        }
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            if let Some(thread) = self.thread.take() {
                // Postear WM_CLOSE a la ventana -> DefWindowProcW la destruye -> WM_DESTROY
                // -> PostQuitMessage -> GetMessageW devuelve FALSE -> el bucle termina.
                // SAFETY: hwnd_bits es el HWND válido que el hilo nos envió; la ventana sigue
                // viva hasta que su hilo procese WM_CLOSE (ese hilo es el único que la destruye).
                unsafe {
                    let hwnd = HWND(self.hwnd_bits as *mut c_void);
                    if let Err(e) = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) {
                        tracing::warn!(error = %e, "device_watch: PostMessageW(WM_CLOSE) falló");
                    }
                }
                // Unir el hilo: bloquea hasta que el bucle de mensajes termine. Si el join
                // falla (panic en el hilo), lo registramos pero no propagamos en Drop.
                if thread.join().is_err() {
                    tracing::warn!("device_watch: el hilo del watcher entró en pánico");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    /// Smoke de ciclo de vida: arrancar el watcher, dejarlo correr un instante y
    /// dropearlo. Lo crítico es que el Drop desmonte limpio SIN colgarse (si este test
    /// retorna, el bucle de mensajes salió y el hilo se unió). `#[ignore]` porque crea
    /// una ventana real del SO; correr con:
    ///   cargo test -p naygo-platform device_watch_lifecycle -- --ignored --test-threads=1
    /// Si tienes un USB a mano, conéctalo/desconéctalo durante la pausa y deberías ver
    /// un DeviceEvent::DrivesChanged por el canal (no se exige aquí para no depender de HW).
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn device_watch_lifecycle() {
        let (tx, _rx) = channel();
        let h = watch(tx, std::sync::Arc::new(|| {}));
        std::thread::sleep(Duration::from_millis(200));
        drop(h); // No debe colgarse ni entrar en pánico.
    }

    /// El stub no-Windows produce un handle inerte sin crashear.
    #[cfg(not(windows))]
    #[test]
    fn stub_no_windows_handle_inerte() {
        let (tx, _rx) = channel();
        let _h = watch(tx, std::sync::Arc::new(|| {}));
    }
}
