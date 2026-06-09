// Naygo — drag & drop con el SO (OLE/COM, aislado): recibir y sacar archivos.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Interop OLE de arrastre con Windows. Esta parte (recibir) implementa un `IDropTarget`
//! COM registrado en la ventana de Naygo: cuando el usuario suelta archivos arrastrados
//! desde el Explorador (u otra app), se extraen las rutas (CF_HDROP) y se envían a la
//! app por un canal. Patrón COM como `trash.rs`/`context_menu.rs`; OLE inicializado en el
//! hilo de UI. Tolerante: cualquier fallo se reporta/ignora, nunca hace panic.
//!
//! ## La cadena COM
//!
//! Windows entrega un drop a una ventana a través de un objeto COM `IDropTarget` que la
//! app registra con `RegisterDragDrop`. Mientras el cursor sobrevuela la ventana con un
//! arrastre activo, el SO llama a `DragEnter`/`DragOver` para preguntar qué efecto
//! aceptamos (copiar, mover, nada). Al soltar, llama a `Drop` con un `IDataObject` que
//! describe lo arrastrado en uno o más formatos del portapapeles. Para archivos del
//! Explorador el formato es **CF_HDROP**: un `HGLOBAL` con una estructura `DROPFILES`
//! seguida de las rutas. No la parseamos a mano: `DragQueryFileW` la recorre por nosotros.
//!
//! La secuencia en `Drop`:
//! 1. `IDataObject::GetData(FORMATETC{ cfFormat: CF_HDROP, tymed: TYMED_HGLOBAL, ... })`
//!    → un `STGMEDIUM` cuyo `hGlobal` es el `HDROP`.
//! 2. `DragQueryFileW(hdrop, 0xFFFFFFFF, None)` → número de archivos.
//! 3. Por cada índice, `DragQueryFileW` con un buffer wide → la ruta.
//! 4. `ReleaseStgMedium` libera el medio (la app es dueña del `STGMEDIUM` que devolvió
//!    `GetData`).
//!
//! ## Sutileza del apartamento + OLE
//!
//! `RegisterDragDrop` exige que el hilo tenga **OLE** inicializado (no solo COM): por eso
//! usamos `OleInitialize`/`OleUninitialize` en vez de `CoInitializeEx`/`CoUninitialize`.
//! `OleInitialize` ya inicializa COM como STA por debajo. Replicamos el patrón de
//! `context_menu.rs`: solo desinicializamos (`OleUninitialize`) si **nosotros** logramos
//! inicializar OLE en este hilo (`hr.is_ok()`); si el hilo ya estaba en otro apartamento
//! (`RPC_E_CHANGED_MODE`) NO desbalanceamos el refcount. Esto corre en el hilo de UI, que
//! eframe/winit ya inicializó como STA.

use std::path::PathBuf;

/// Archivos soltados en la ventana de Naygo desde el SO.
#[derive(Debug, Clone)]
pub struct DroppedFiles {
    pub paths: Vec<PathBuf>,
    pub screen_x: i32,
    pub screen_y: i32,
    /// `true` si el efecto fue copiar (vs mover). Para archivos externos, normalmente copy.
    pub effect_copy: bool,
}

/// Stub no-Windows: recibir drag&drop del SO no existe fuera de Windows.
#[cfg(not(windows))]
pub fn register_drop_target(
    _hwnd: isize,
    _tx: std::sync::mpsc::Sender<DroppedFiles>,
) -> Option<()> {
    None
}

#[cfg(windows)]
pub use windows_impl::{register_drop_target, DropTargetGuard};

#[cfg(windows)]
mod windows_impl {
    use super::DroppedFiles;
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::sync::mpsc::Sender;
    use windows::core::{implement, Ref};
    use windows::Win32::Foundation::{HWND, POINTL};
    use windows::Win32::System::Com::{IDataObject, FORMATETC, TYMED_HGLOBAL};
    use windows::Win32::System::Ole::{
        IDropTarget, IDropTarget_Impl, OleInitialize, OleUninitialize, RegisterDragDrop,
        ReleaseStgMedium, RevokeDragDrop, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY,
    };
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

    /// Objeto COM que implementa `IDropTarget`. Sostiene el `Sender` por el que despacha
    /// los archivos soltados a la app. `#[implement(IDropTarget)]` genera el andamiaje
    /// (vtable, refcount, `QueryInterface`) y nos deja escribir solo los métodos del trait.
    #[implement(IDropTarget)]
    struct NaygoDropTarget {
        tx: Sender<DroppedFiles>,
    }

    impl IDropTarget_Impl for NaygoDropTarget_Impl {
        fn DragEnter(
            &self,
            _pdataobj: Ref<IDataObject>,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            _pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            // Aceptamos el arrastre como "copiar". SAFETY: el SO provee un puntero válido.
            unsafe {
                if !pdweffect.is_null() {
                    *pdweffect = DROPEFFECT_COPY;
                }
            }
            Ok(())
        }

        fn DragOver(
            &self,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            _pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            // SAFETY: puntero del SO; lo escribimos solo si no es nulo.
            unsafe {
                if !pdweffect.is_null() {
                    *pdweffect = DROPEFFECT_COPY;
                }
            }
            Ok(())
        }

        fn DragLeave(&self) -> windows::core::Result<()> {
            Ok(())
        }

        fn Drop(
            &self,
            pdataobj: Ref<IDataObject>,
            _grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            // Reportamos "copiar" como efecto final (lo que pintamos en DragOver). SAFETY:
            // puntero del SO. Esto debe pasar pase lo que pase con la extracción.
            unsafe {
                if !pdweffect.is_null() {
                    *pdweffect = DROPEFFECT_COPY;
                }
            }

            // Extraer rutas del IDataObject. Tolerante: si algo falla, no enviamos nada y
            // devolvemos Ok (nunca hacemos panic ni rompemos el arrastre del SO).
            let paths = pdataobj
                .as_ref()
                .map(extract_hdrop_paths)
                .unwrap_or_default();

            if !paths.is_empty() {
                let dropped = DroppedFiles {
                    paths,
                    screen_x: pt.x,
                    screen_y: pt.y,
                    effect_copy: true,
                };
                // Si el receptor ya no existe (app cerrando), ignorar.
                let _ = self.tx.send(dropped);
            }

            Ok(())
        }
    }

    /// Extrae las rutas CF_HDROP de un `IDataObject`. Devuelve vacío ante cualquier fallo
    /// (formato ausente, HGLOBAL nulo, etc.). Libera el `STGMEDIUM` que `GetData` entrega.
    fn extract_hdrop_paths(data: &IDataObject) -> Vec<PathBuf> {
        // FORMATETC pidiendo CF_HDROP como HGLOBAL, contenido completo.
        let format = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: windows::Win32::System::Com::DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };

        // SAFETY: `format` es válido; `GetData` devuelve un STGMEDIUM del que somos dueños
        // y que liberamos con `ReleaseStgMedium`. Si el dato no está en CF_HDROP, GetData
        // falla y devolvemos vacío.
        unsafe {
            let mut medium = match data.GetData(&format) {
                Ok(m) => m,
                Err(_) => return Vec::new(),
            };

            // El HGLOBAL del medio es el HDROP. Si el tipo no es el esperado o es nulo,
            // liberamos y salimos.
            let hglobal = medium.u.hGlobal;
            if hglobal.0.is_null() {
                ReleaseStgMedium(&mut medium);
                return Vec::new();
            }
            let hdrop = HDROP(hglobal.0);

            let paths = query_hdrop_files(hdrop);

            // Liberar el medio: somos dueños del STGMEDIUM devuelto por GetData.
            ReleaseStgMedium(&mut medium);
            paths
        }
    }

    /// Recorre un `HDROP` con `DragQueryFileW` y devuelve las rutas como `PathBuf`.
    ///
    /// SAFETY: `hdrop` debe ser un HDROP válido (proviene del HGLOBAL del STGMEDIUM de un
    /// CF_HDROP). `DragQueryFileW(_, 0xFFFFFFFF, None)` devuelve la cuenta; con un índice
    /// concreto y un buffer del tamaño exacto, copia la ruta (sin NUL final en el conteo).
    unsafe fn query_hdrop_files(hdrop: HDROP) -> Vec<PathBuf> {
        use std::os::windows::ffi::OsStringExt;

        let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
        if count == 0 {
            return Vec::new();
        }

        let mut paths = Vec::with_capacity(count as usize);
        for i in 0..count {
            // Longitud (en chars, sin NUL) de la ruta i.
            let len = DragQueryFileW(hdrop, i, None);
            if len == 0 {
                continue;
            }
            // Buffer con espacio para el NUL final que DragQueryFileW escribe.
            let mut buf = vec![0u16; len as usize + 1];
            let copied = DragQueryFileW(hdrop, i, Some(buf.as_mut_slice()));
            if copied == 0 {
                continue;
            }
            // `copied` es la longitud sin NUL; recortamos a esa longitud.
            buf.truncate(copied as usize);
            paths.push(PathBuf::from(std::ffi::OsString::from_wide(&buf)));
        }
        paths
    }

    /// Guard que mantiene vivo el drop target y lo revoca al `Drop`.
    ///
    /// Sostiene la interfaz `IDropTarget` (que mantiene vivo el objeto COM con su `Sender`),
    /// el HWND donde se registró, y si nosotros inicializamos OLE en este hilo. Al dropearse:
    /// `RevokeDragDrop(hwnd)` y, si corresponde, `OleUninitialize`.
    ///
    /// `!Send` (sostiene una interfaz COM STA). Vive en el hilo de UI de Naygo, así que
    /// nunca cruza de hilo: correcto para apartment threading.
    pub struct DropTargetGuard {
        hwnd: isize,
        // Se mantiene vivo solo por su efecto en el refcount COM (mantiene el objeto y su
        // Sender vivos mientras el SO pueda invocar el drop target). No se lee.
        #[allow(dead_code)]
        target: IDropTarget,
        needs_uninit: bool,
    }

    impl Drop for DropTargetGuard {
        fn drop(&mut self) {
            // SAFETY: hwnd es el HWND donde registramos el target; RevokeDragDrop deshace
            // RegisterDragDrop. OleUninitialize SOLO si nosotros inicializamos OLE aquí
            // (mismo balance que context_menu.rs / trash.rs).
            unsafe {
                let hwnd = HWND(self.hwnd as *mut c_void);
                if let Err(e) = RevokeDragDrop(hwnd) {
                    tracing::warn!(error = %e, "dnd: RevokeDragDrop falló");
                }
                if self.needs_uninit {
                    OleUninitialize();
                }
            }
        }
    }

    /// Registra el drop target en la ventana `hwnd`. `None` si no se pudo (la app sigue sin
    /// recibir drops externos). Debe llamarse en el hilo de UI.
    pub fn register_drop_target(hwnd: isize, tx: Sender<DroppedFiles>) -> Option<DropTargetGuard> {
        // SAFETY: secuencia OLE/COM completa. OleInitialize inicializa OLE+COM (STA) en
        // este hilo; needs_uninit rastrea si fuimos NOSOTROS (para no desbalancear el
        // refcount si el hilo ya estaba inicializado por eframe → RPC_E_CHANGED_MODE).
        unsafe {
            let hr = OleInitialize(None);
            let needs_uninit = hr.is_ok();

            // Construir el objeto COM y obtener su interfaz IDropTarget.
            let target: IDropTarget = NaygoDropTarget { tx }.into();

            match RegisterDragDrop(HWND(hwnd as *mut c_void), &target) {
                Ok(()) => Some(DropTargetGuard {
                    hwnd,
                    target,
                    needs_uninit,
                }),
                Err(e) => {
                    tracing::warn!(error = %e, "dnd: RegisterDragDrop falló; sin drops externos");
                    // No quedó nada registrado; deshacer el OleInitialize si fue nuestro.
                    if needs_uninit {
                        OleUninitialize();
                    }
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El stub no-Windows devuelve None sin crashear.
    #[cfg(not(windows))]
    #[test]
    fn stub_no_windows_es_none() {
        let (tx, _rx) = std::sync::mpsc::channel();
        assert!(register_drop_target(0, tx).is_none());
    }

    /// Smoke de construcción del tipo público (sin registrar en una ventana real).
    #[test]
    fn dropped_files_construye() {
        let d = DroppedFiles {
            paths: vec![PathBuf::from("C:/x")],
            screen_x: 10,
            screen_y: 20,
            effect_copy: true,
        };
        assert_eq!(d.paths.len(), 1);
        assert!(d.effect_copy);
    }
}
