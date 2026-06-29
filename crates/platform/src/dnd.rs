// Naygo — drag & drop con el SO (OLE/COM, aislado): sacar archivos al SO.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Interop OLE de arrastre con Windows. Este módulo cubre solo el lado de **SACAR**
//! archivos de Naygo hacia el SO (Explorador, escritorio, correo…) vía `DoDragDrop`.
//!
//! **Recibir** drops del SO NO se maneja aquí: winit ya registra su propio `IDropTarget`
//! en la ventana y egui expone las rutas soltadas en `ctx.input(|i| i.raw.dropped_files)`.
//! La capa de UI lee de ahí. (Antes registrábamos nuestro propio `IDropTarget` con
//! `RegisterDragDrop`, lo que colisionaba con el de winit —`DRAGDROP_E_ALREADYREGISTERED`—
//! y la rotación de `OleInitialize`/`OleUninitialize` en el hilo de UI ya OLE-inicializado
//! por winit desestabilizaba el arranque.)
//!
//! ## La cadena COM (lado emisor)
//!
//! Para arrastrar archivos FUERA de Naygo se ofrece al SO un objeto COM `IDataObject` que
//! expone las rutas en CF_HDROP, y un `IDropSource` que decide en cada frame del arrastre
//! si continuar/soltar/cancelar. `DoDragDrop` corre su propio bucle modal hasta que el
//! usuario suelta o cancela, y devuelve el efecto final. Tolerante: cualquier fallo se
//! reporta como `DndError`, nunca hace panic.

/// Resultado de un arrastre OLE iniciado por Naygo hacia el SO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragOutcome {
    /// El destino aceptó como COPIA (los archivos siguen en el origen).
    Copied,
    /// El destino aceptó como MOVER (el origen debería refrescarse: el dato ya no está).
    Moved,
    /// El usuario canceló (Esc) o soltó fuera de un destino válido. Nada cambió.
    Cancelled,
}

/// Error al iniciar un arrastre OLE hacia el SO.
#[derive(Debug)]
pub enum DndError {
    /// Plataforma no soportada (no-Windows).
    NotSupported,
    /// No había rutas que arrastrar.
    NoItems,
    /// Falló la cadena OLE (construcción del IDataObject, DoDragDrop, etc.).
    Failed(String),
}

/// Stub no-Windows: sacar drag&drop al SO no existe fuera de Windows.
#[cfg(not(windows))]
pub fn start_drag(_paths: &[std::path::PathBuf]) -> Result<DragOutcome, DndError> {
    Err(DndError::NotSupported)
}

#[cfg(windows)]
pub use windows_impl::start_drag;

#[cfg(windows)]
mod windows_impl {
    use super::{DndError, DragOutcome};
    use std::path::PathBuf;
    use windows::core::{implement, Ref, BOOL, HRESULT};
    use windows::Win32::Foundation::{
        DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_FORMATETC,
        OLE_E_ADVISENOTSUPPORTED, S_OK,
    };
    use windows::Win32::System::Com::{
        IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, DATADIR_GET,
        DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL,
    };
    use windows::Win32::System::Ole::{
        DoDragDrop, IDropSource, IDropSource_Impl, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY,
        DROPEFFECT_MOVE,
    };
    use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
    use windows::Win32::UI::Shell::SHCreateStdEnumFmtEtc;

    // ====================================================================================
    // SACAR drag&drop al SO: IDataObject + IDropSource + DoDragDrop (Naygo → Explorer).
    // ====================================================================================
    //
    // ## La cadena COM (lado emisor)
    //
    // Para arrastrar archivos FUERA de Naygo (al Explorador, escritorio, un correo…) hay
    // que ofrecer al SO un objeto COM `IDataObject` que exponga las rutas en CF_HDROP, y un
    // `IDropSource` que decida en cada frame del arrastre si continuar/soltar/cancelar.
    // Luego se llama a `DoDragDrop`, que corre **su propio bucle modal** (captura el mouse,
    // dibuja el cursor de arrastre, hace polling al IDropSource) hasta que el usuario suelta
    // o cancela. Devuelve un HRESULT (`DRAGDROP_S_DROP` / `DRAGDROP_S_CANCEL`) y el efecto
    // final por `*pdweffect`.
    //
    // ## Por qué se implementa IDataObject a mano
    //
    // `SHCreateDataObject` (windows 0.62) construye un IDataObject listo PERO a partir de
    // **PIDLs** del namespace del Shell (como `context_menu.rs`), no de un HGLOBAL CF_HDROP.
    // Para CF_HDROP puro lo natural es un IDataObject propio mínimo. Sí reutilizamos el
    // helper del Shell `SHCreateStdEnumFmtEtc` para `EnumFormatEtc` (evita implementar a mano
    // un IEnumFORMATETC, que es la parte más tediosa). El resto de métodos del IDataObject que
    // un drop target del Shell no necesita para CF_HDROP se stubean con `E_NOTIMPL`/códigos
    // estándar.
    //
    // ## Memoria de CF_HDROP
    //
    // `GetData` debe devolver un STGMEDIUM cuyo HGLOBAL sea **una copia fresca**: el receptor
    // (vía `ReleaseStgMedium`) la liberará. Si entregáramos el mismo HGLOBAL en dos GetData,
    // se liberaría dos veces. Por eso reconstruimos el bloque DROPFILES en cada GetData con
    // el helper compartido `clipboard::build_hdrop_global` y dejamos `pUnkForRelease = null`
    // (→ el SO lo libera con `GlobalFree`, que es como lo asignó `build_hdrop_global`).

    /// FORMATETC que describe "CF_HDROP como HGLOBAL, contenido completo". Es el único
    /// formato que ofrecemos. Igual al que `extract_hdrop_paths` pide en el lado receptor.
    fn hdrop_formatetc() -> FORMATETC {
        FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        }
    }

    /// `true` si `f` pide exactamente nuestro formato (CF_HDROP / HGLOBAL / CONTENT). El
    /// `lindex` no se exige (-1 es "todo"); algunos clientes lo dejan en 0.
    fn formatetc_is_hdrop(f: &FORMATETC) -> bool {
        f.cfFormat == CF_HDROP.0
            && (f.tymed & TYMED_HGLOBAL.0 as u32) != 0
            && f.dwAspect == DVASPECT_CONTENT.0
    }

    /// IDataObject mínimo que expone un conjunto de rutas como CF_HDROP. Guarda las rutas
    /// y reconstruye el HGLOBAL bajo demanda en cada `GetData`. `#[implement(IDataObject)]`
    /// genera vtable/refcount/QueryInterface; solo escribimos los métodos del trait.
    #[implement(IDataObject)]
    struct FilesDataObject {
        paths: Vec<PathBuf>,
    }

    impl IDataObject_Impl for FilesDataObject_Impl {
        /// Único método "real": ante CF_HDROP devuelve un STGMEDIUM con un HGLOBAL fresco.
        fn GetData(&self, pformatetcin: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
            // SAFETY: el SO entrega un puntero válido a un FORMATETC; lo leemos por copia.
            let format = unsafe {
                match pformatetcin.as_ref() {
                    Some(f) => *f,
                    None => return Err(DV_E_FORMATETC.into()),
                }
            };
            if !formatetc_is_hdrop(&format) {
                return Err(DV_E_FORMATETC.into());
            }

            // Construir un HGLOBAL CF_HDROP nuevo (copia): el receptor lo liberará.
            // SAFETY: build_hdrop_global escribe en memoria recién asignada por GlobalAlloc.
            let hglobal = unsafe {
                crate::clipboard::windows_impl::build_hdrop_global(&self.paths).map_err(|e| {
                    windows::core::Error::new(
                        windows::Win32::Foundation::E_OUTOFMEMORY,
                        format!("{e:?}"),
                    )
                })?
            };

            let medium = STGMEDIUM {
                tymed: TYMED_HGLOBAL.0 as u32,
                u: windows::Win32::System::Com::STGMEDIUM_0 { hGlobal: hglobal },
                // null → ReleaseStgMedium liberará el HGLOBAL con GlobalFree (correcto, así
                // lo asignó build_hdrop_global). Es el patrón estándar para HGLOBAL propio.
                pUnkForRelease: std::mem::ManuallyDrop::new(None),
            };
            Ok(medium)
        }

        /// No soportamos rellenar un medio provisto por el cliente.
        fn GetDataHere(
            &self,
            _pformatetc: *const FORMATETC,
            _pmedium: *mut STGMEDIUM,
        ) -> windows::core::Result<()> {
            Err(DV_E_FORMATETC.into())
        }

        /// "¿Tienes este formato?" → S_OK para CF_HDROP, DV_E_FORMATETC si no.
        fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
            // SAFETY: puntero del SO; lo leemos solo si no es nulo.
            let ok = unsafe { pformatetc.as_ref().map(formatetc_is_hdrop).unwrap_or(false) };
            if ok {
                S_OK
            } else {
                DV_E_FORMATETC
            }
        }

        /// No canonicalizamos formatos: devolvemos el de entrada tal cual (S_OK con copia)
        /// según la convención; basta con indicar que no hay forma canónica distinta.
        fn GetCanonicalFormatEtc(
            &self,
            _pformatectin: *const FORMATETC,
            pformatetcout: *mut FORMATETC,
        ) -> HRESULT {
            // SAFETY: dejamos el ptd a null en la salida (sin dispositivo de destino).
            unsafe {
                if let Some(out) = pformatetcout.as_mut() {
                    *out = FORMATETC::default();
                    out.ptd = std::ptr::null_mut();
                }
            }
            // DATA_S_SAMEFORMATETC indicaría "el mismo"; E_NOTIMPL es aceptable y más simple.
            windows::Win32::Foundation::E_NOTIMPL
        }

        /// Es un objeto de solo lectura (origen de arrastre): no aceptamos SetData.
        fn SetData(
            &self,
            _pformatetc: *const FORMATETC,
            _pmedium: *const STGMEDIUM,
            _frelease: BOOL,
        ) -> windows::core::Result<()> {
            Err(windows::Win32::Foundation::E_NOTIMPL.into())
        }

        /// Enumeración de formatos: solo para GET, y solo CF_HDROP. Reutilizamos el helper
        /// del Shell `SHCreateStdEnumFmtEtc` para no implementar IEnumFORMATETC a mano.
        fn EnumFormatEtc(&self, dwdirection: u32) -> windows::core::Result<IEnumFORMATETC> {
            if dwdirection != DATADIR_GET.0 as u32 {
                // No exponemos formatos para SET.
                return Err(windows::Win32::Foundation::E_NOTIMPL.into());
            }
            let formats = [hdrop_formatetc()];
            // SAFETY: `formats` vive durante la llamada; SHCreateStdEnumFmtEtc copia su
            // contenido en el enumerador que devuelve.
            unsafe { SHCreateStdEnumFmtEtc(&formats) }
        }

        /// No soportamos notificaciones de cambio (objeto efímero de arrastre).
        fn DAdvise(
            &self,
            _pformatetc: *const FORMATETC,
            _advf: u32,
            _padvsink: Ref<IAdviseSink>,
        ) -> windows::core::Result<u32> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }

        fn DUnadvise(&self, _dwconnection: u32) -> windows::core::Result<()> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }

        fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
            Err(OLE_E_ADVISENOTSUPPORTED.into())
        }
    }

    /// IDropSource mínimo: gobierna el bucle modal de `DoDragDrop`. El SO lo invoca en cada
    /// iteración para preguntar si el arrastre continúa, y para elegir el cursor.
    #[implement(IDropSource)]
    struct NaygoDropSource;

    impl IDropSource_Impl for NaygoDropSource_Impl {
        /// El SO pregunta en cada paso: ¿seguir, soltar o cancelar?
        /// - Esc presionado → cancelar.
        /// - Botón izquierdo SOLTADO (ya no está en grfkeystate) → soltar.
        /// - Si no → continuar (S_OK).
        fn QueryContinueDrag(
            &self,
            fescapepressed: BOOL,
            grfkeystate: MODIFIERKEYS_FLAGS,
        ) -> HRESULT {
            if fescapepressed.as_bool() {
                return DRAGDROP_S_CANCEL;
            }
            // ¿Sigue presionado el botón izquierdo (el que inició el arrastre)?
            if (grfkeystate.0 & MK_LBUTTON.0) == 0 {
                // Se soltó → el destino bajo el cursor recibe el drop.
                return DRAGDROP_S_DROP;
            }
            S_OK
        }

        /// Usamos los cursores de arrastre por defecto del SO (flecha + signo +/→).
        fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
            DRAGDROP_S_USEDEFAULTCURSORS
        }
    }

    /// Inicia un arrastre OLE de `paths` hacia el SO (Explorer, escritorio, correo…).
    ///
    /// **BLOQUEANTE**: `DoDragDrop` corre su propio bucle modal hasta que el usuario suelta
    /// o cancela. Debe llamarse en el **hilo de UI** (apartamento STA), y FUERA del closure
    /// de render de egui (el bucle modal toma el control del mouse mientras dura). Devuelve
    /// el efecto resultante. Tolerante: nunca hace panic; cualquier fallo → `DndError`.
    pub fn start_drag(paths: &[PathBuf]) -> Result<DragOutcome, DndError> {
        if paths.is_empty() {
            return Err(DndError::NoItems);
        }

        // El hilo de UI ya está OLE-inicializado por winit (registra su propio IDropTarget);
        // NO hacemos OleInitialize/Uninitialize propios (perturbaba el arranque). DoDragDrop
        // usa el OLE del hilo.
        // SAFETY: secuencia OLE/COM completa en el hilo de UI (STA).
        unsafe {
            // Construir los dos objetos COM y obtener sus interfaces.
            let data_object: IDataObject = FilesDataObject {
                paths: paths.to_vec(),
            }
            .into();
            let drop_source: IDropSource = NaygoDropSource.into();

            let mut effect = DROPEFFECT::default();
            // El bucle modal del SO. Ofrecemos COPY|MOVE; el destino y los modificadores del
            // usuario deciden el efecto final, que el SO escribe en `effect`.
            let result: HRESULT = DoDragDrop(
                &data_object,
                &drop_source,
                DROPEFFECT_COPY | DROPEFFECT_MOVE,
                &mut effect,
            );

            // Mapear el HRESULT + efecto a nuestro DragOutcome.
            let outcome = if result == DRAGDROP_S_DROP {
                if (effect.0 & DROPEFFECT_MOVE.0) != 0 {
                    DragOutcome::Moved
                } else {
                    DragOutcome::Copied
                }
            } else if result == DRAGDROP_S_CANCEL {
                DragOutcome::Cancelled
            } else {
                // Cualquier otro HRESULT (E_*): el arrastre no se completó. Lo tratamos como
                // fallo tolerante, no como panic.
                return Err(DndError::Failed(format!("DoDragDrop devolvió {result:?}")));
            };

            Ok(outcome)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `start_drag` con lista vacía devuelve `NoItems` sin tocar OLE (no entra al bucle
    /// modal). En no-Windows el stub también es `NoItems` por la misma guarda… salvo que
    /// el stub no-Windows devuelve `NotSupported`; por eso solo lo afirmamos en Windows.
    #[cfg(windows)]
    #[test]
    fn start_drag_vacio_es_noitems() {
        match start_drag(&[]) {
            Err(DndError::NoItems) => {}
            other => panic!("esperaba NoItems, vino {other:?}"),
        }
    }

    /// En no-Windows, `start_drag` siempre es `NotSupported`.
    #[cfg(not(windows))]
    #[test]
    fn start_drag_no_windows_es_notsupported() {
        match start_drag(&[]) {
            Err(DndError::NotSupported) => {}
            other => panic!("esperaba NotSupported, vino {other:?}"),
        }
    }
}
