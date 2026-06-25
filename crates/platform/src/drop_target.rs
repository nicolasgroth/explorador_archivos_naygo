// Naygo — destino de drop OLE (recibir archivos arrastrados). Aislado en platform.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Registra una ventana como destino de drop OLE (`IDropTarget` + `RegisterDragDrop`) para
//! que el usuario pueda arrastrar archivos desde el Explorador y soltarlos sobre Naygo.
//!
//! Este módulo cubre solo el lado de **RECIBIR** (el de **SACAR** vive en `dnd.rs`). Cuando
//! el usuario suelta archivos sobre la ventana, se envía un [`DropPayload`] por el canal y se
//! despierta la UI con el `waker` (la UI está dormida en reposo, clave para el bajo consumo).
//!
//! Tolerante (el SO es hostil): si OLE o el registro fallan, [`register`] devuelve un guard
//! **inerte** (no crashea; simplemente no llegarán drops). En no-Windows es un stub inerte.
//!
//! ## La cadena COM (lado receptor)
//!
//! El SO, durante un arrastre, busca en la ventana bajo el cursor un `IDropTarget` registrado
//! con `RegisterDragDrop`. Llama a `DragEnter`/`DragOver` para que indiquemos el efecto
//! (copiar/mover) y así pintar el cursor correcto, y a `Drop` cuando el usuario suelta. En
//! `Drop` leemos el `IDataObject` (formato CF_HDROP / HGLOBAL) y extraemos las rutas con el
//! helper compartido `clipboard::extract_hdrop_paths` (no duplicamos el bucle DragQueryFileW).

use crate::dir_watch::Waker;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

/// Un drop recibido: las rutas soltadas, si el usuario pidió MOVER (tecla Shift) en vez de
/// copiar, y el PUNTO del cursor al soltar (en coordenadas de PANTALLA, píxeles físicos) para
/// que la UI enrute el drop al panel que está bajo el cursor (no al panel activo).
pub struct DropPayload {
    /// Rutas absolutas de los archivos/carpetas soltados.
    pub paths: Vec<PathBuf>,
    /// `true` si `MK_SHIFT` estaba activo al soltar (mover); `false` = copiar.
    pub move_: bool,
    /// X del cursor al soltar, en coordenadas de PANTALLA (píxeles físicos). El SO lo entrega
    /// en `IDropTarget::Drop`. La UI lo convierte a coords de contenido para hit-testear paneles.
    pub screen_x: i32,
    /// Y del cursor al soltar, en coordenadas de PANTALLA (píxeles físicos).
    pub screen_y: i32,
}

/// Guard RAII: al dropearse, revoca el registro (`RevokeDragDrop`) y libera el target.
/// El campo interno solo existe para mantener vivo el registro mientras viva el guard.
pub struct DropTargetGuard {
    // Solo se mantiene vivo por su `Drop` (RAII): revoca el registro al soltarse. Nunca se
    // lee, de ahí el `allow(dead_code)`.
    #[cfg(windows)]
    #[allow(dead_code)]
    inner: Option<windows_impl::Registration>,
    #[cfg(not(windows))]
    _priv: (),
}

/// Stub no-Windows: recibir drops del SO no existe fuera de Windows. Guard inerte.
#[cfg(not(windows))]
pub fn register(_hwnd: isize, _tx: Sender<DropPayload>, _waker: Waker) -> DropTargetGuard {
    DropTargetGuard { _priv: () }
}

/// Registra `hwnd` como destino de drop OLE. Cuando el usuario suelta archivos, envía un
/// [`DropPayload`] por `tx` y despierta la UI con `waker`. `hwnd` es el handle nativo
/// (`isize`). Tolerante: si OLE/registro falla, devuelve un guard inerte (no crashea;
/// simplemente no llegarán drops). Debe llamarse en el **hilo de UI** (apartamento STA).
#[cfg(windows)]
pub fn register(hwnd: isize, tx: Sender<DropPayload>, waker: Waker) -> DropTargetGuard {
    match windows_impl::register(hwnd, tx, waker) {
        Some(reg) => DropTargetGuard { inner: Some(reg) },
        None => {
            tracing::warn!(
                hwnd,
                "no se pudo registrar el destino de drop OLE; guard inerte"
            );
            DropTargetGuard { inner: None }
        }
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{DropPayload, Waker};
    use std::sync::mpsc::Sender;
    use windows::core::{implement, Ref};
    use windows::Win32::Foundation::{HWND, POINTL};
    use windows::Win32::System::Com::{
        IDataObject, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL,
    };
    use windows::Win32::System::Ole::{
        IDropTarget, IDropTarget_Impl, OleInitialize, RegisterDragDrop, ReleaseStgMedium,
        RevokeDragDrop, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE,
    };
    use windows::Win32::System::SystemServices::{MK_SHIFT, MODIFIERKEYS_FLAGS};
    use windows::Win32::UI::Shell::HDROP;

    /// Construye el `HWND` a partir del handle nativo `isize`. En windows 0.62 `HWND`
    /// envuelve un puntero crudo.
    fn hwnd_from_isize(hwnd: isize) -> HWND {
        HWND(hwnd as *mut core::ffi::c_void)
    }

    /// FORMATETC que pide "CF_HDROP como HGLOBAL, contenido completo". Es el formato que
    /// solicitamos al `IDataObject` en `Drop` (espejo del que ofrece el lado emisor en
    /// `dnd.rs::hdrop_formatetc`).
    fn hdrop_formatetc() -> FORMATETC {
        FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        }
    }

    /// El efecto a mostrar según los modificadores de teclado: Shift → MOVER, si no COPIAR.
    fn effect_for(grfkeystate: MODIFIERKEYS_FLAGS) -> DROPEFFECT {
        if (grfkeystate.0 & MK_SHIFT.0) != 0 {
            DROPEFFECT_MOVE
        } else {
            DROPEFFECT_COPY
        }
    }

    /// `IDropTarget` que reenvía las rutas soltadas por un canal y despierta la UI.
    /// `#[implement(IDropTarget)]` genera vtable/refcount/QueryInterface; solo escribimos
    /// los cuatro métodos del trait.
    #[implement(IDropTarget)]
    struct NaygoDropTarget {
        tx: Sender<DropPayload>,
        waker: Waker,
    }

    impl IDropTarget_Impl for NaygoDropTarget_Impl {
        /// El cursor entra en la ventana durante un arrastre: indicamos el efecto a pintar.
        fn DragEnter(
            &self,
            _pdataobj: Ref<IDataObject>,
            grfkeystate: MODIFIERKEYS_FLAGS,
            _pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            // SAFETY: el SO entrega un puntero válido a un DROPEFFECT escribible.
            unsafe {
                if let Some(eff) = pdweffect.as_mut() {
                    *eff = effect_for(grfkeystate);
                }
            }
            Ok(())
        }

        /// El cursor se mueve dentro de la ventana: refrescamos el efecto (Shift puede
        /// cambiar a mitad del arrastre).
        fn DragOver(
            &self,
            grfkeystate: MODIFIERKEYS_FLAGS,
            _pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            // SAFETY: igual que en DragEnter; puntero del SO a un DROPEFFECT escribible.
            unsafe {
                if let Some(eff) = pdweffect.as_mut() {
                    *eff = effect_for(grfkeystate);
                }
            }
            Ok(())
        }

        /// El cursor sale de la ventana sin soltar: nada que hacer.
        fn DragLeave(&self) -> windows::core::Result<()> {
            Ok(())
        }

        /// El usuario suelta: leemos CF_HDROP del IDataObject, extraemos las rutas y las
        /// enviamos por el canal, despertando la UI.
        fn Drop(
            &self,
            pdataobj: Ref<IDataObject>,
            grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            // Punto del cursor al soltar, en coordenadas de PANTALLA (píxeles físicos). La UI lo
            // usa para enrutar el drop al panel bajo el cursor.
            let (screen_x, screen_y) = (pt.x, pt.y);
            let effect = effect_for(grfkeystate);
            // Reflejar el efecto elegido en la salida (el SO lo usa para la animación final).
            // SAFETY: puntero del SO a un DROPEFFECT escribible.
            unsafe {
                if let Some(eff) = pdweffect.as_mut() {
                    *eff = effect;
                }
            }

            let move_ = (grfkeystate.0 & MK_SHIFT.0) != 0;

            // Tomar el IDataObject (puede venir nulo en casos raros).
            let data = match pdataobj.as_ref() {
                Some(d) => d,
                None => return Ok(()),
            };

            // Pedir CF_HDROP / HGLOBAL y extraer las rutas. Cualquier fallo se traga
            // (el SO es hostil): no llegan rutas, pero no crasheamos.
            let format = hdrop_formatetc();
            // SAFETY: `format` vive durante la llamada; GetData copia lo que necesita y nos
            // devuelve un STGMEDIUM cuya propiedad pasamos a `release_stgmedium`.
            let medium: STGMEDIUM = match unsafe { data.GetData(&format) } {
                Ok(m) => m,
                Err(_) => return Ok(()),
            };

            // El medio debe ser un HGLOBAL; extraer el HDROP y leer las rutas.
            if medium.tymed == TYMED_HGLOBAL.0 as u32 {
                // SAFETY: tymed == HGLOBAL garantiza que la unión contiene `hGlobal`. El
                // puntero del HGLOBAL es un bloque DROPFILES válido durante esta llamada;
                // extract_hdrop_paths solo lee con DragQueryFileW, no libera nada.
                let paths = unsafe {
                    let hdrop = HDROP(medium.u.hGlobal.0);
                    crate::clipboard::windows_impl::extract_hdrop_paths(hdrop)
                };

                if !paths.is_empty() {
                    // Enviar el payload; si el receptor ya colgó, lo ignoramos.
                    let _ = self.tx.send(DropPayload {
                        paths,
                        move_,
                        screen_x,
                        screen_y,
                    });
                    (self.waker)();
                }
            }

            // Liberar el STGMEDIUM (libera el HGLOBAL según su pUnkForRelease/tymed).
            // SAFETY: `medium` es el que nos devolvió GetData y aún no se ha liberado.
            let mut medium = medium;
            unsafe {
                ReleaseStgMedium(&mut medium);
            }

            Ok(())
        }
    }

    /// Mantiene vivo el registro de drop. Su `Drop` revoca con `RevokeDragDrop`.
    pub struct Registration {
        hwnd: HWND,
        // Mantiene viva la interfaz mientras dure el registro (RevokeDragDrop la libera, pero
        // conservar la referencia es explícito y evita sorpresas de ciclo de vida).
        _target: IDropTarget,
    }

    impl Drop for Registration {
        fn drop(&mut self) {
            // SAFETY: `hwnd` fue registrado con RegisterDragDrop en este mismo hilo; revocar
            // es la operación inversa y es segura aunque el registro ya no estuviera (devuelve
            // error, que ignoramos).
            unsafe {
                let _ = RevokeDragDrop(self.hwnd);
            }
        }
    }

    /// Inicializa OLE en este hilo (idempotente/tolerante) y registra el `IDropTarget`.
    /// Devuelve `Some(Registration)` si el registro fue exitoso, o `None` si algo falló.
    pub fn register(hwnd: isize, tx: Sender<DropPayload>, waker: Waker) -> Option<Registration> {
        let hwnd = hwnd_from_isize(hwnd);
        if hwnd.0.is_null() {
            return None;
        }

        // OleInitialize debe llamarse una vez por hilo antes de RegisterDragDrop. Si el hilo
        // ya está OLE-inicializado (p. ej. por winit), devuelve S_FALSE o RPC_E_CHANGED_MODE;
        // ambos son tolerables (no llamamos OleUninitialize: no fuimos quienes inicializamos).
        // SAFETY: llamada estándar de inicialización OLE en el hilo de UI (STA).
        unsafe {
            let _ = OleInitialize(None);
        }

        let target: IDropTarget = NaygoDropTarget { tx, waker }.into();

        // SAFETY: hwnd no es nulo y `target` es una interfaz IDropTarget válida; el SO toma
        // su propia referencia. Cualquier error (p. ej. DRAGDROP_E_ALREADYREGISTERED) → None.
        let result = unsafe { RegisterDragDrop(hwnd, &target) };
        if result.is_err() {
            tracing::warn!(?result, "RegisterDragDrop falló");
            return None;
        }

        Some(Registration {
            hwnd,
            _target: target,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn noop_waker() -> Waker {
        std::sync::Arc::new(|| {})
    }

    /// En no-Windows, `register` siempre devuelve un guard inerte sin tocar OLE.
    /// En Windows, un `hwnd` nulo (0) no es registrable: también debe quedar inerte y NO
    /// crashear (contrato tolerante). En ambos casos basta con que no haga panic y el guard
    /// se pueda dropear (que en Windows no debe revocar nada porque nunca registró).
    #[test]
    fn register_hwnd_nulo_o_no_windows_es_inerte() {
        let (tx, _rx) = channel::<DropPayload>();
        let guard = register(0, tx, noop_waker());
        // El guard se dropea aquí sin panic.
        drop(guard);
    }
}
