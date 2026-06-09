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

/// Stub no-Windows: recibir drag&drop del SO no existe fuera de Windows.
#[cfg(not(windows))]
pub fn register_drop_target(
    _hwnd: isize,
    _tx: std::sync::mpsc::Sender<DroppedFiles>,
) -> Option<()> {
    None
}

/// Stub no-Windows: sacar drag&drop al SO no existe fuera de Windows.
#[cfg(not(windows))]
pub fn start_drag(_paths: &[std::path::PathBuf]) -> Result<DragOutcome, DndError> {
    Err(DndError::NotSupported)
}

#[cfg(windows)]
pub use windows_impl::{register_drop_target, start_drag, DropTargetGuard};

#[cfg(windows)]
mod windows_impl {
    use super::{DndError, DragOutcome, DroppedFiles};
    use std::ffi::c_void;
    use std::path::PathBuf;
    use std::sync::mpsc::Sender;
    use windows::core::{implement, Ref, BOOL, HRESULT};
    use windows::Win32::Foundation::{
        DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_FORMATETC, HWND,
        OLE_E_ADVISENOTSUPPORTED, POINTL, S_OK,
    };
    use windows::Win32::System::Com::{
        IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, DATADIR_GET,
        DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL,
    };
    use windows::Win32::System::Ole::{
        DoDragDrop, IDropSource, IDropSource_Impl, IDropTarget, IDropTarget_Impl, OleInitialize,
        OleUninitialize, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop, CF_HDROP, DROPEFFECT,
        DROPEFFECT_COPY, DROPEFFECT_MOVE,
    };
    use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
    use windows::Win32::UI::Shell::{DragQueryFileW, SHCreateStdEnumFmtEtc, HDROP};

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

        // SAFETY: secuencia OLE/COM completa en el hilo de UI (STA). Balanceamos OLE igual
        // que register_drop_target: solo desinicializamos si fuimos nosotros quien inicializó.
        unsafe {
            let hr = OleInitialize(None);
            let needs_uninit = hr.is_ok();

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

            if needs_uninit {
                OleUninitialize();
            }

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

    /// El stub no-Windows devuelve None sin crashear.
    #[cfg(not(windows))]
    #[test]
    fn stub_no_windows_es_none() {
        let (tx, _rx) = std::sync::mpsc::channel();
        assert!(register_drop_target(0, tx).is_none());
    }

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
