// Naygo — watcher de dispositivos (Fase 5B): detecta enchufar/quitar unidades (USB) y avisa
// para re-escanear unidades y reubicar paneles cuya raíz desapareció.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Corre en su propio hilo (platform::device_watch, ventana message-only WM_DEVICECHANGE) y
// está dormido salvo ante un cambio real, que despierta la UI con el `waker`. El hilo de UI
// solo drena el canal en el tick.

use naygo_platform::device_watch::{self, DeviceEvent, DeviceWatchHandle};
use naygo_platform::dir_watch::Waker;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Vigila los cambios de unidades del sistema. Al dropearse, detiene el hilo y la ventana.
pub struct Devices {
    _handle: DeviceWatchHandle,
    rx: Receiver<DeviceEvent>,
}

impl Devices {
    /// Arranca la vigilancia de dispositivos. `waker` despierta la UI dormida ante un cambio.
    pub fn start(waker: Waker) -> Devices {
        let (tx, rx): (Sender<DeviceEvent>, Receiver<DeviceEvent>) = channel();
        let handle = device_watch::watch(tx, waker);
        Devices {
            _handle: handle,
            rx,
        }
    }

    /// ¿Llegó al menos un evento de cambio de unidades desde el último drenado? Drena todos.
    pub fn drives_changed(&self) -> bool {
        let mut changed = false;
        while self.rx.try_recv().is_ok() {
            changed = true;
        }
        changed
    }
}
