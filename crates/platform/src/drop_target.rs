// Naygo — destino de drop OLE (recibir archivos arrastrados). Aislado en platform.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

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
//! `Drop` priorizamos archivos virtuales (`CFSTR_FILEDESCRIPTORW` + `CFSTR_FILECONTENTS`) cuando
//! están disponibles: 7-Zip/WinRAR pueden publicar además un `CF_HDROP` temporal y retirarlo al
//! volver de `Drop`, antes de que nuestra planificación asíncrona alcance a leerlo. Los streams
//! virtuales se marshalean a un worker COM, se materializan en un staging propio y recién entonces
//! se entregan a la UI. Si la fuente solo ofrece `CF_HDROP` (Explorer/escritorio), extraemos esas
//! rutas con el helper compartido `clipboard::extract_hdrop_paths`.

use crate::dir_watch::Waker;
use std::path::PathBuf;
use std::sync::{mpsc::Sender, Arc};

/// Mantiene vivo el staging de un drop de archivos virtuales. La última referencia programa su
/// eliminación en un worker: ni cancelar el modal ni terminar una copia hace I/O en el hilo UI.
#[derive(Debug)]
pub struct StagedDropGuard {
    root: PathBuf,
}

impl StagedDropGuard {
    #[cfg(windows)]
    fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Raíz privada del staging, útil para diagnóstico y tests internos del flujo.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }
}

impl Drop for StagedDropGuard {
    fn drop(&mut self) {
        let root = self.root.clone();
        std::thread::spawn(move || {
            if let Err(error) = std::fs::remove_dir_all(&root) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(?error, path = %root.display(), "no se pudo limpiar staging de drop virtual");
                }
            }
        });
    }
}

/// Referencia compartida al staging: viaja desde platform hasta el modal/cola/operación para que
/// los archivos sigan existiendo durante toda la copia y se limpien al soltar la última referencia.
pub type StagedDrop = Arc<StagedDropGuard>;

/// Un drop recibido: las rutas soltadas, si el usuario pidió MOVER (tecla Shift) en vez de
/// copiar, y el PUNTO del cursor al soltar (en coordenadas de PANTALLA, píxeles físicos) para
/// que la UI enrute el drop al panel que está bajo el cursor (no al panel activo).
pub struct DropPayload {
    /// Rutas absolutas de los archivos/carpetas soltados.
    pub paths: Vec<PathBuf>,
    /// `true` si `MK_SHIFT` estaba activo al soltar (mover); `false` = copiar.
    pub move_: bool,
    /// `true` si `MK_CONTROL` estaba activo al soltar (copiar SIEMPRE, aunque sea el mismo disco).
    /// El teclado de la app queda stale durante `DoDragDrop`, así que este flag —leído del
    /// `grfKeyState` que Windows entrega al SOLTAR— es la ÚNICA fuente fiable del Ctrl del usuario.
    pub copy_forced: bool,
    /// X del cursor al soltar, en coordenadas de PANTALLA (píxeles físicos). El SO lo entrega
    /// en `IDropTarget::Drop`. La UI lo convierte a coords de contenido para hit-testear paneles.
    pub screen_x: i32,
    /// Y del cursor al soltar, en coordenadas de PANTALLA (píxeles físicos).
    pub screen_y: i32,
    /// Mantiene vivos los archivos materializados de una fuente OLE virtual. `None` para rutas
    /// reales (`CF_HDROP`). La UI debe conservarlo hasta que la operación termine o se cancele.
    pub staging: Option<StagedDrop>,
    /// Detalle técnico de un fallo al materializar archivos virtuales. En ese caso `paths` está
    /// vacío y la UI muestra un toast localizado sin intentar iniciar una operación.
    pub error: Option<String>,
}

/// Evento de HOVER durante un arrastre, para resaltar EN VIVO el panel bajo el cursor (borde +
/// título) mientras el usuario arrastra archivos sobre la ventana. Viaja por un canal HERMANO
/// (`drag_tx`), SEPARADO del [`DropPayload`] final (`drop_tx`): el hover es de alta frecuencia y
/// puramente visual; el drop es el commit. Mezclarlos obligaría a la UI a distinguir variantes en
/// el mismo canal y arriesgaría que un hover se cuele como si fuera un drop.
///
/// `Over` se emite en `DragEnter`/`DragOver` (el SO los dispara MUCHÍSIMO, en cada movimiento);
/// `Leave` en `DragLeave` y en `Drop` (al salir o al soltar, el resaltado debe quitarse). El punto
/// va en coordenadas de PANTALLA (píxeles físicos), igual que `DropPayload`, y la UI lo convierte a
/// coords de contenido con la MISMA fórmula. No hay throttle aquí: el lado UI recalcula el panel y
/// solo re-pinta si CAMBIÓ respecto del último (un hit-test + comparación, barato).
pub enum DragHover {
    /// El cursor está sobre la ventana en este punto de PANTALLA (físico) durante un arrastre.
    Over { screen_x: i32, screen_y: i32 },
    /// El cursor salió de la ventana (o se soltó): limpiar cualquier resaltado de hover.
    Leave,
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

impl DropTargetGuard {
    /// `true` solo cuando `RegisterDragDrop` quedó realmente instalado. Un guard
    /// inerte permite que la UI reintente tras la creación tardía del HWND/OLE.
    pub fn is_registered(&self) -> bool {
        #[cfg(windows)]
        {
            self.inner.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

/// Stub no-Windows: recibir drops del SO no existe fuera de Windows. Guard inerte.
#[cfg(not(windows))]
pub fn register(
    _hwnd: isize,
    _tx: Sender<DropPayload>,
    _drag_tx: Sender<DragHover>,
    _waker: Waker,
) -> DropTargetGuard {
    DropTargetGuard { _priv: () }
}

/// Registra `hwnd` como destino de drop OLE. Cuando el usuario suelta archivos, envía un
/// [`DropPayload`] por `tx`; mientras arrastra sobre la ventana, envía eventos [`DragHover`] por
/// `drag_tx` (para resaltar el panel bajo el cursor). En ambos casos despierta la UI con `waker`.
/// `hwnd` es el handle nativo (`isize`). Tolerante: si OLE/registro falla, devuelve un guard inerte
/// (no crashea; simplemente no llegarán drops). Debe llamarse en el **hilo de UI** (apartamento STA).
#[cfg(windows)]
pub fn register(
    hwnd: isize,
    tx: Sender<DropPayload>,
    drag_tx: Sender<DragHover>,
    waker: Waker,
) -> DropTargetGuard {
    match windows_impl::register(hwnd, tx, drag_tx, waker) {
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
    use super::{DragHover, DropPayload, StagedDropGuard, Waker};
    use std::collections::HashSet;
    use std::io::Write;
    use std::path::{Component, Path, PathBuf};
    use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
    use std::sync::{
        mpsc::{sync_channel, Sender},
        Arc,
    };
    use windows::core::{implement, Interface, Ref};
    use windows::Win32::Foundation::{HGLOBAL, HWND, POINTL};
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
    use windows::Win32::System::Com::Marshal::CoMarshalInterThreadInterfaceInStream;
    use windows::Win32::System::Com::StructuredStorage::CoGetInterfaceAndReleaseStream;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, IDataObject, IStream, COINIT_MULTITHREADED,
        DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL, TYMED_ISTREAM,
    };
    use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::{
        IDropTarget, IDropTarget_Impl, OleInitialize, RegisterDragDrop, ReleaseStgMedium,
        RevokeDragDrop, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE,
    };
    use windows::Win32::System::SystemServices::{MK_CONTROL, MK_SHIFT, MODIFIERKEYS_FLAGS};
    use windows::Win32::UI::Shell::{
        CFSTR_FILECONTENTS, CFSTR_FILEDESCRIPTORW, FILEDESCRIPTORW, HDROP,
    };

    const ACCEPT_NONE: u8 = 0;
    const ACCEPT_HDROP: u8 = 1;
    const ACCEPT_VIRTUAL: u8 = 2;
    const MAX_VIRTUAL_ITEMS: usize = 100_000;
    static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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

    fn registered_format(name: windows::core::PCWSTR) -> Option<u16> {
        // SAFETY: `name` es una cadena wide estática terminada en NUL provista por Windows.
        let value = unsafe { RegisterClipboardFormatW(name) };
        (value != 0).then_some(value as u16)
    }

    fn descriptor_formatetc() -> Option<FORMATETC> {
        Some(FORMATETC {
            cfFormat: registered_format(CFSTR_FILEDESCRIPTORW)?,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        })
    }

    fn contents_formatetc(index: usize) -> Option<FORMATETC> {
        Some(FORMATETC {
            cfFormat: registered_format(CFSTR_FILECONTENTS)?,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: i32::try_from(index).ok()?,
            tymed: (TYMED_ISTREAM.0 | TYMED_HGLOBAL.0) as u32,
        })
    }

    /// El efecto a mostrar según los modificadores de teclado: Ctrl → COPIAR (siempre, tiene
    /// prioridad), Shift → MOVER, si no COPIAR (el default).
    fn effect_for(grfkeystate: MODIFIERKEYS_FLAGS) -> DROPEFFECT {
        if (grfkeystate.0 & MK_CONTROL.0) != 0 {
            DROPEFFECT_COPY
        } else if (grfkeystate.0 & MK_SHIFT.0) != 0 {
            DROPEFFECT_MOVE
        } else {
            DROPEFFECT_COPY
        }
    }

    fn effect_for_kind(kind: u8, grfkeystate: MODIFIERKEYS_FLAGS) -> DROPEFFECT {
        match kind {
            ACCEPT_HDROP => effect_for(grfkeystate),
            // Un archivo virtual se materializa en staging y luego se COPIA al destino. Mostrar
            // Move sería engañoso: no se puede mover una entrada fuera de un .zip.
            ACCEPT_VIRTUAL => DROPEFFECT_COPY,
            _ => DROPEFFECT(0),
        }
    }

    /// Elige el formato que se conserva durante el ciclo asíncrono de Naygo.
    ///
    /// Cuando un archivador entrega ambos, `CF_HDROP` puede ser una extracción de vida corta.
    /// `CFSTR_FILEDESCRIPTORW` permite pedir los streams y escribir nuestro staging antes de
    /// planificar, por lo que tiene prioridad. El helper es puro para proteger esa prioridad con
    /// una regresión sin necesitar un IDataObject COM falso.
    fn preferred_drop_kind(has_virtual: bool, has_hdrop: bool) -> u8 {
        if has_virtual {
            ACCEPT_VIRTUAL
        } else if has_hdrop {
            ACCEPT_HDROP
        } else {
            ACCEPT_NONE
        }
    }

    /// `CF_HDROP` de 7-Zip no siempre representa una ruta estable: el archivador extrae cada
    /// selección a `Temp\\7zE…` y puede borrar ese archivo al volver de `IDropTarget::Drop`.
    /// El planificador de Naygo corre después, por lo que recibir esa ruta directamente equivale
    /// a una carrera y termina como `SourceUnreadable`.
    fn is_7zip_temporary_hdrop(path: &Path) -> bool {
        let temp = std::env::temp_dir();
        path.starts_with(&temp)
            && path.parent().and_then(Path::file_name).is_some_and(|name| {
                name.to_string_lossy()
                    .to_ascii_lowercase()
                    .starts_with("7ze")
            })
    }

    fn needs_ephemeral_hdrop_staging(paths: &[PathBuf]) -> bool {
        paths.iter().any(|path| is_7zip_temporary_hdrop(path))
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct VirtualDescriptor {
        relative: PathBuf,
        is_dir: bool,
    }

    /// Lee el HGLOBAL de FILEGROUPDESCRIPTORW. Es metadata acotada (nombres/atributos), no el
    /// contenido: hacerlo dentro de Drop no introduce I/O de disco ni consume los streams.
    fn read_virtual_descriptors(data: &IDataObject) -> Result<Vec<VirtualDescriptor>, String> {
        let format = descriptor_formatetc()
            .ok_or_else(|| "Windows no registró FileGroupDescriptorW".to_string())?;
        // SAFETY: FORMATETC válido; el STGMEDIUM se libera en todos los caminos posteriores.
        let mut medium = unsafe { data.GetData(&format) }
            .map_err(|error| format!("no se pudieron leer los descriptores: {error}"))?;
        let result = if medium.tymed == TYMED_HGLOBAL.0 as u32 {
            // SAFETY: tymed confirma que la unión contiene hGlobal.
            let hglobal = unsafe { medium.u.hGlobal };
            parse_descriptor_hglobal(hglobal)
        } else {
            Err("FileGroupDescriptorW no llegó como HGLOBAL".to_string())
        };
        // SAFETY: medio devuelto por GetData, aún no liberado.
        unsafe { ReleaseStgMedium(&mut medium) };
        result
    }

    fn parse_descriptor_hglobal(hglobal: HGLOBAL) -> Result<Vec<VirtualDescriptor>, String> {
        // SAFETY: GlobalSize/GlobalLock son las operaciones correspondientes al HGLOBAL recibido.
        let len = unsafe { GlobalSize(hglobal) };
        if len < size_of::<u32>() {
            return Err("descriptor virtual truncado".to_string());
        }
        let ptr = unsafe { GlobalLock(hglobal) };
        if ptr.is_null() {
            return Err("no se pudo bloquear el descriptor virtual".to_string());
        }
        // SAFETY: `ptr` apunta a `len` bytes hasta GlobalUnlock; solo se leen.
        let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
        let result = parse_descriptor_bytes(bytes);
        // GlobalUnlock devuelve error cuando el lock-count llega a cero; no afecta los bytes ya
        // copiados a PathBuf. Ignoramos ese estado documentado.
        let _ = unsafe { GlobalUnlock(hglobal) };
        result
    }

    fn parse_descriptor_bytes(bytes: &[u8]) -> Result<Vec<VirtualDescriptor>, String> {
        let count = u32::from_le_bytes(
            bytes
                .get(..4)
                .ok_or_else(|| "descriptor virtual truncado".to_string())?
                .try_into()
                .map_err(|_| "descriptor virtual inválido".to_string())?,
        ) as usize;
        if count == 0 {
            return Err("la fuente virtual no contiene archivos".to_string());
        }
        if count > MAX_VIRTUAL_ITEMS {
            return Err(format!(
                "la fuente virtual contiene demasiados elementos ({count})"
            ));
        }
        let item_size = size_of::<FILEDESCRIPTORW>();
        let required = 4usize
            .checked_add(
                count
                    .checked_mul(item_size)
                    .ok_or_else(|| "descriptor virtual demasiado grande".to_string())?,
            )
            .ok_or_else(|| "descriptor virtual demasiado grande".to_string())?;
        if bytes.len() < required {
            return Err("descriptor virtual truncado".to_string());
        }

        let mut descriptors = Vec::with_capacity(count);
        let mut seen = HashSet::with_capacity(count);
        for index in 0..count {
            let offset = 4 + index * item_size;
            // FILEDESCRIPTORW es packed(1) y comienza tras un u32: read_unaligned es obligatorio.
            // El rango ya fue validado contra `required`.
            let descriptor = unsafe {
                std::ptr::read_unaligned(bytes.as_ptr().add(offset).cast::<FILEDESCRIPTORW>())
            };
            // Copiar el array packed a una variable alineada antes de crear slices/referencias.
            let file_name = unsafe { std::ptr::addr_of!(descriptor.cFileName).read_unaligned() };
            let end = file_name
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(file_name.len());
            let name = String::from_utf16(&file_name[..end])
                .map_err(|_| format!("nombre UTF-16 inválido en el elemento {}", index + 1))?;
            let relative = sanitize_virtual_path(&name)?;
            let identity = relative.to_string_lossy().to_lowercase();
            if !seen.insert(identity) {
                return Err(format!("ruta virtual duplicada: {}", relative.display()));
            }
            descriptors.push(VirtualDescriptor {
                relative,
                is_dir: (descriptor.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0) != 0,
            });
        }
        Ok(descriptors)
    }

    /// Convierte un nombre relativo del proveedor en una ruta segura bajo staging. Rechaza
    /// traversal, rutas absolutas, ADS, nombres de dispositivo y caracteres inválidos de Win32.
    fn sanitize_virtual_path(raw: &str) -> Result<PathBuf, String> {
        let normalized = raw.replace('/', "\\");
        let path = Path::new(&normalized);
        let mut safe = PathBuf::new();
        for component in path.components() {
            let Component::Normal(value) = component else {
                return Err(format!("ruta virtual insegura: {raw}"));
            };
            let value = value.to_string_lossy();
            if value.is_empty()
                || value.ends_with([' ', '.'])
                || value
                    .chars()
                    .any(|ch| ch < ' ' || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
                || is_reserved_windows_name(&value)
            {
                return Err(format!("nombre virtual no permitido: {value}"));
            }
            safe.push(value.as_ref());
        }
        if safe.as_os_str().is_empty() {
            return Err("nombre virtual vacío".to_string());
        }
        Ok(safe)
    }

    fn is_reserved_windows_name(value: &str) -> bool {
        let stem = value
            .split('.')
            .next()
            .unwrap_or(value)
            .trim_end_matches(' ');
        let upper = stem.to_ascii_uppercase();
        matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || reserved_numbered_name(&upper, "COM")
            || reserved_numbered_name(&upper, "LPT")
    }

    fn reserved_numbered_name(value: &str, prefix: &str) -> bool {
        value
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
    }

    struct ComWorkerGuard;

    impl Drop for ComWorkerGuard {
        fn drop(&mut self) {
            // SAFETY: solo se construye después de un CoInitializeEx exitoso en este hilo.
            unsafe { CoUninitialize() };
        }
    }

    fn materialize_virtual_drop(
        marshaled_raw: usize,
        descriptors: &[VirtualDescriptor],
    ) -> Result<(Vec<PathBuf>, Arc<StagedDropGuard>), String> {
        // SAFETY: inicialización COM del worker; balanceada por ComWorkerGuard.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| format!("no se pudo inicializar COM en el worker: {error}"))?;
        let _com = ComWorkerGuard;
        // SAFETY: el puntero proviene de `IStream::into_raw` justo antes de lanzar este worker y
        // representa el stream creado por CoMarshalInterThreadInterfaceInStream.
        let marshaled = unsafe { IStream::from_raw(marshaled_raw as *mut core::ffi::c_void) };
        // SAFETY: consume el stream de marshaling una sola vez y entrega el proxy IDataObject.
        let data_result: windows::core::Result<IDataObject> =
            unsafe { CoGetInterfaceAndReleaseStream(&marshaled) };
        // CoGetInterfaceAndReleaseStream ya liberó la referencia COM subyacente al stream. Evitar
        // que el wrapper Rust haga un segundo Release al salir del scope.
        std::mem::forget(marshaled);
        let data =
            data_result.map_err(|error| format!("no se pudo recuperar el origen OLE: {error}"))?;

        let root = create_staging_dir()?;
        let guard = Arc::new(StagedDropGuard::new(root.clone()));
        for (index, descriptor) in descriptors.iter().enumerate() {
            let destination = root.join(&descriptor.relative);
            if descriptor.is_dir {
                std::fs::create_dir_all(&destination).map_err(|error| {
                    format!(
                        "no se pudo crear {}: {error}",
                        descriptor.relative.display()
                    )
                })?;
                continue;
            }
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("no se pudo crear {}: {error}", parent.display()))?;
            }
            write_virtual_file(&data, index, &destination)?;
        }
        let paths = virtual_top_level_paths(&root, descriptors);
        if paths.is_empty() {
            return Err("la fuente virtual no produjo archivos".to_string());
        }
        Ok((paths, guard))
    }

    /// Archivo de 7-Zip ya abierto mientras el `Drop` OLE todavía está activo. El handle mantiene
    /// legible el contenido aunque 7-Zip retire su nombre temporal inmediatamente después.
    struct OpenedHdropFile {
        source: PathBuf,
        file: std::fs::File,
    }

    fn open_ephemeral_hdrop_files(paths: &[PathBuf]) -> Result<Vec<OpenedHdropFile>, String> {
        let mut opened = Vec::with_capacity(paths.len());
        let mut names = HashSet::with_capacity(paths.len());
        for source in paths {
            let metadata = std::fs::metadata(source).map_err(|error| {
                format!(
                    "no se pudo abrir el temporal de 7-Zip {}: {error}",
                    source.display()
                )
            })?;
            if !metadata.is_file() {
                return Err(format!(
                    "el temporal de 7-Zip no es un archivo: {}",
                    source.display()
                ));
            }
            let name = source.file_name().ok_or_else(|| {
                format!("el temporal de 7-Zip no tiene nombre: {}", source.display())
            })?;
            let identity = name.to_string_lossy().to_ascii_lowercase();
            if !names.insert(identity) {
                return Err(format!(
                    "7-Zip entregó dos temporales con el mismo nombre: {}",
                    name.to_string_lossy()
                ));
            }
            let file = std::fs::File::open(source).map_err(|error| {
                format!(
                    "no se pudo retener el temporal de 7-Zip {}: {error}",
                    source.display()
                )
            })?;
            opened.push(OpenedHdropFile {
                source: source.clone(),
                file,
            });
        }
        Ok(opened)
    }

    fn stage_opened_hdrop_files(
        files: Vec<OpenedHdropFile>,
    ) -> Result<(Vec<PathBuf>, Arc<StagedDropGuard>), String> {
        let root = create_staging_dir()?;
        let guard = Arc::new(StagedDropGuard::new(root.clone()));
        let mut paths = Vec::with_capacity(files.len());
        for mut source in files {
            let name = source.source.file_name().ok_or_else(|| {
                format!(
                    "el temporal retenido de 7-Zip no tiene nombre: {}",
                    source.source.display()
                )
            })?;
            let destination = root.join(name);
            let mut output = std::fs::File::create(&destination).map_err(|error| {
                format!(
                    "no se pudo crear el staging para {}: {error}",
                    source.source.display()
                )
            })?;
            std::io::copy(&mut source.file, &mut output).map_err(|error| {
                format!(
                    "no se pudo copiar el temporal de 7-Zip {}: {error}",
                    source.source.display()
                )
            })?;
            output.flush().map_err(|error| {
                format!("no se pudo finalizar {}: {error}", destination.display())
            })?;
            paths.push(destination);
        }
        Ok((paths, guard))
    }

    /// Inicia el snapshot de una ruta temporal de 7-Zip en un worker. La espera de dos segundos
    /// es SOLO hasta que el worker abrió los handles (no hasta copiar bytes); sin ese acuse, el
    /// proveedor puede borrar el temporal antes de que el worker alcance a abrirlo. La copia de
    /// verdad, potencialmente grande, sigue fuera del callback OLE y del hilo de UI.
    fn spawn_ephemeral_hdrop_staging(
        paths: Vec<PathBuf>,
        screen_x: i32,
        screen_y: i32,
        tx: Sender<DropPayload>,
        waker: Waker,
    ) {
        let (ready_tx, ready_rx) = sync_channel::<()>(1);
        std::thread::spawn(move || {
            let opened = open_ephemeral_hdrop_files(&paths);
            // Aunque el receptor ya haya agotado el timeout, enviar no bloquea y el worker aún
            // puede informar el error concreto a la UI.
            let _ = ready_tx.send(());
            let payload = match opened.and_then(stage_opened_hdrop_files) {
                Ok((paths, staging)) => DropPayload {
                    paths,
                    move_: false,
                    copy_forced: true,
                    screen_x,
                    screen_y,
                    staging: Some(staging),
                    error: None,
                },
                Err(error) => DropPayload {
                    paths: Vec::new(),
                    move_: false,
                    copy_forced: true,
                    screen_x,
                    screen_y,
                    staging: None,
                    error: Some(error),
                },
            };
            let _ = tx.send(payload);
            (waker)();
        });
        if ready_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .is_err()
        {
            tracing::warn!("el worker no alcanzó a abrir el temporal de 7-Zip durante Drop");
        }
    }

    fn create_staging_dir() -> Result<PathBuf, String> {
        let base = std::env::temp_dir().join("Naygo").join("virtual-drops");
        std::fs::create_dir_all(&base)
            .map_err(|error| format!("no se pudo crear el staging temporal: {error}"))?;
        for _ in 0..32 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = base.join(format!("{}-{sequence}", std::process::id()));
            match std::fs::create_dir(&root) {
                Ok(()) => return Ok(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!("no se pudo crear el staging temporal: {error}"));
                }
            }
        }
        Err("no se pudo reservar un staging temporal único".to_string())
    }

    fn write_virtual_file(
        data: &IDataObject,
        index: usize,
        destination: &Path,
    ) -> Result<(), String> {
        let format = contents_formatetc(index)
            .ok_or_else(|| "Windows no registró FileContents".to_string())?;
        // SAFETY: FORMATETC válido; el medio se libera antes de retornar.
        let mut medium = unsafe { data.GetData(&format) }.map_err(|error| {
            format!("no se pudo leer el archivo virtual {}: {error}", index + 1)
        })?;
        let partial = destination
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".naygo-virtual-{index}.part"));
        let result = (|| -> Result<(), String> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&partial)
                .map_err(|error| format!("no se pudo crear {}: {error}", destination.display()))?;
            match medium.tymed {
                value if value == TYMED_ISTREAM.0 as u32 => {
                    // SAFETY: el discriminante TYMED_ISTREAM hace activa la rama `pstm`. Clonamos
                    // la interfaz para leer y ReleaseStgMedium libera su propia referencia.
                    let stream = unsafe { (*medium.u.pstm).clone() }
                        .ok_or_else(|| "FileContents entregó un IStream nulo".to_string())?;
                    copy_stream(&stream, &mut file)?;
                }
                value if value == TYMED_HGLOBAL.0 as u32 => {
                    // SAFETY: el discriminante TYMED_HGLOBAL hace activa la rama `hGlobal`.
                    let hglobal = unsafe { medium.u.hGlobal };
                    copy_hglobal(hglobal, &mut file)?;
                }
                _ => return Err("FileContents entregó un medio no compatible".to_string()),
            }
            file.flush().map_err(|error| error.to_string())?;
            drop(file);
            std::fs::rename(&partial, destination)
                .map_err(|error| format!("no se pudo finalizar {}: {error}", destination.display()))
        })();
        // SAFETY: medio devuelto por GetData, aún no liberado.
        unsafe { ReleaseStgMedium(&mut medium) };
        if result.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        result
    }

    fn copy_stream(stream: &IStream, file: &mut std::fs::File) -> Result<(), String> {
        let mut buffer = vec![0u8; 256 * 1024];
        loop {
            let mut read = 0u32;
            // SAFETY: buffer válido para `len` bytes; `read` vive durante la llamada.
            unsafe {
                stream.Read(
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    Some(&mut read),
                )
            }
            .ok()
            .map_err(|error| format!("falló la lectura del stream virtual: {error}"))?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read as usize])
                .map_err(|error| format!("falló la escritura del archivo temporal: {error}"))?;
        }
        Ok(())
    }

    fn copy_hglobal(hglobal: HGLOBAL, file: &mut std::fs::File) -> Result<(), String> {
        // SAFETY: GlobalSize/GlobalLock son las operaciones del HGLOBAL recibido.
        let len = unsafe { GlobalSize(hglobal) };
        let ptr = unsafe { GlobalLock(hglobal) };
        if ptr.is_null() && len != 0 {
            return Err("no se pudo bloquear el contenido virtual".to_string());
        }
        if len != 0 {
            // SAFETY: ptr apunta a `len` bytes hasta GlobalUnlock.
            let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) };
            file.write_all(bytes)
                .map_err(|error| format!("falló la escritura del archivo temporal: {error}"))?;
        }
        if !ptr.is_null() {
            let _ = unsafe { GlobalUnlock(hglobal) };
        }
        Ok(())
    }

    fn virtual_top_level_paths(root: &Path, descriptors: &[VirtualDescriptor]) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        let mut paths = Vec::new();
        for descriptor in descriptors {
            let Some(first) = descriptor.relative.components().next() else {
                continue;
            };
            let Component::Normal(first) = first else {
                continue;
            };
            let identity = first.to_string_lossy().to_lowercase();
            if seen.insert(identity) {
                paths.push(root.join(first));
            }
        }
        paths
    }

    /// `IDropTarget` que reenvía las rutas soltadas por un canal y despierta la UI.
    /// `#[implement(IDropTarget)]` genera vtable/refcount/QueryInterface; solo escribimos
    /// los cuatro métodos del trait.
    #[implement(IDropTarget)]
    struct NaygoDropTarget {
        tx: Sender<DropPayload>,
        /// Canal hermano para los eventos de HOVER (resaltar el panel bajo el cursor). Separado de
        /// `tx` a propósito: el hover es de alta frecuencia y visual; el drop es el commit.
        drag_tx: Sender<DragHover>,
        waker: Waker,
        /// Formato aceptado en el arrastre actual. `DragOver` no recibe el IDataObject, por eso
        /// conserva la decisión tomada en `DragEnter` y no pinta un efecto para datos inválidos.
        accepted_kind: AtomicU8,
    }

    impl IDropTarget_Impl for NaygoDropTarget_Impl {
        /// El cursor entra en la ventana durante un arrastre: indicamos el efecto a pintar y
        /// reportamos el punto para resaltar el panel bajo el cursor.
        fn DragEnter(
            &self,
            pdataobj: Ref<IDataObject>,
            grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            // SAFETY: el SO entrega un puntero válido a un DROPEFFECT escribible.
            let kind = pdataobj.as_ref().map_or(ACCEPT_NONE, |data| {
                // Un archivador puede ofrecer AMBOS formatos. Su CF_HDROP suele apuntar a una
                // extracción temporal que desaparece apenas `Drop` retorna; como Naygo planifica
                // en un worker, esa ruta llega tarde y falla con SourceUnreadable. El formato
                // virtual entrega el contenido de forma estable y se materializa en un staging
                // propio ANTES de iniciar la operación. Explorer normalmente solo ofrece HDROP,
                // por lo que conserva el camino rápido de rutas reales.
                let has_virtual = descriptor_formatetc()
                    .is_some_and(|format| unsafe { data.QueryGetData(&format).is_ok() });
                let has_hdrop = unsafe { data.QueryGetData(&hdrop_formatetc()).is_ok() };
                preferred_drop_kind(has_virtual, has_hdrop)
            });
            self.accepted_kind.store(kind, Ordering::Relaxed);
            unsafe {
                if let Some(eff) = pdweffect.as_mut() {
                    *eff = effect_for_kind(kind, grfkeystate);
                }
            }
            self.emit_hover(pt);
            Ok(())
        }

        /// El cursor se mueve dentro de la ventana: refrescamos el efecto (Shift puede
        /// cambiar a mitad del arrastre) y reportamos el nuevo punto para el resaltado.
        fn DragOver(
            &self,
            grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            // SAFETY: igual que en DragEnter; puntero del SO a un DROPEFFECT escribible.
            unsafe {
                if let Some(eff) = pdweffect.as_mut() {
                    *eff = effect_for_kind(self.accepted_kind.load(Ordering::Relaxed), grfkeystate);
                }
            }
            self.emit_hover(pt);
            Ok(())
        }

        /// El cursor sale de la ventana sin soltar: limpiar el resaltado del panel.
        fn DragLeave(&self) -> windows::core::Result<()> {
            self.accepted_kind.store(ACCEPT_NONE, Ordering::Relaxed);
            self.emit_leave();
            Ok(())
        }

        /// El usuario suelta: priorizamos la variante virtual si está disponible; así evitamos
        /// rutas temporales efímeras de archivadores. Si la fuente solo ofrece `CF_HDROP`, usamos
        /// sus rutas reales. La materialización posterior usa workers, pero `GetData` se invoca en
        /// este callback y algunos proveedores pueden bloquearlo al extraer contenido; además el
        /// snapshot de temporales espera hasta dos segundos. Esa latencia aún requiere solución.
        fn Drop(
            &self,
            pdataobj: Ref<IDataObject>,
            grfkeystate: MODIFIERKEYS_FLAGS,
            pt: &POINTL,
            pdweffect: *mut DROPEFFECT,
        ) -> windows::core::Result<()> {
            // Tras un Drop el SO NO envía DragLeave: limpiamos el resaltado del panel al
            // ENTRAR al método, así los early-returns de abajo (dataobject nulo, GetData
            // fallido) no dejan el borde resaltado pegado.
            let accepted_kind = self.accepted_kind.swap(ACCEPT_NONE, Ordering::Relaxed);
            self.emit_leave();

            // Punto del cursor al soltar, en coordenadas de PANTALLA (píxeles físicos). La UI lo
            // usa para enrutar el drop al panel bajo el cursor.
            let (screen_x, screen_y) = (pt.x, pt.y);
            let effect = effect_for_kind(accepted_kind, grfkeystate);
            // Reflejar el efecto elegido en la salida (el SO lo usa para la animación final).
            // SAFETY: puntero del SO a un DROPEFFECT escribible.
            unsafe {
                if let Some(eff) = pdweffect.as_mut() {
                    *eff = effect;
                }
            }

            let move_ = (grfkeystate.0 & MK_SHIFT.0) != 0;
            let copy_forced = (grfkeystate.0 & MK_CONTROL.0) != 0;

            // Tomar el IDataObject (puede venir nulo en casos raros).
            let data = match pdataobj.as_ref() {
                Some(d) => d,
                None => return Ok(()),
            };

            // Camino de rutas reales. Explorer/escritorio entregan rutas persistentes y siguen
            // directo al planificador. 7-Zip, en cambio, entrega un `CF_HDROP` a un archivo
            // efímero en Temp: lo snapshotteamos antes de retornar de Drop.
            if accepted_kind == ACCEPT_HDROP {
                let format = hdrop_formatetc();
                // SAFETY: `format` vive durante GetData; liberamos el medio antes de retornar.
                if let Ok(mut medium) = unsafe { data.GetData(&format) } {
                    if medium.tymed == TYMED_HGLOBAL.0 as u32 {
                        // SAFETY: con TYMED_HGLOBAL la unión contiene un HDROP válido mientras
                        // viva el STGMEDIUM.
                        let paths = unsafe {
                            let hdrop = HDROP(medium.u.hGlobal.0);
                            crate::clipboard::windows_impl::extract_hdrop_paths(hdrop)
                        };
                        if !paths.is_empty() {
                            if needs_ephemeral_hdrop_staging(&paths) {
                                spawn_ephemeral_hdrop_staging(
                                    paths,
                                    screen_x,
                                    screen_y,
                                    self.tx.clone(),
                                    self.waker.clone(),
                                );
                            } else {
                                self.send_payload(DropPayload {
                                    paths,
                                    move_,
                                    copy_forced,
                                    screen_x,
                                    screen_y,
                                    staging: None,
                                    error: None,
                                });
                            }
                        }
                    }
                    // SAFETY: medio devuelto por GetData, aún no liberado.
                    unsafe { ReleaseStgMedium(&mut medium) };
                }
                return Ok(());
            }

            if accepted_kind != ACCEPT_VIRTUAL {
                return Ok(());
            }

            let descriptors = match read_virtual_descriptors(data) {
                Ok(value) => value,
                Err(error) => {
                    self.send_virtual_error(screen_x, screen_y, error);
                    return Ok(());
                }
            };

            // COM exige marshaling explícito entre el STA de la UI y el worker. Pasamos el
            // stream marshaleado como puntero opaco (su única operación válida será reconstruirlo
            // con `IStream::from_raw` en el worker ya inicializado para COM).
            let marshaled =
                match unsafe { CoMarshalInterThreadInterfaceInStream(&IDataObject::IID, data) } {
                    Ok(stream) => stream,
                    Err(error) => {
                        self.send_virtual_error(
                            screen_x,
                            screen_y,
                            format!("no se pudo transferir el origen OLE al worker: {error}"),
                        );
                        return Ok(());
                    }
                };
            let marshaled_raw = marshaled.into_raw() as usize;
            let tx = self.tx.clone();
            let waker = self.waker.clone();
            std::thread::spawn(move || {
                let result = materialize_virtual_drop(marshaled_raw, &descriptors);
                let payload = match result {
                    Ok((paths, staging)) => DropPayload {
                        paths,
                        // Una entrada de archivo comprimido solo se puede copiar al filesystem.
                        move_: false,
                        copy_forced: true,
                        screen_x,
                        screen_y,
                        staging: Some(staging),
                        error: None,
                    },
                    Err(error) => DropPayload {
                        paths: Vec::new(),
                        move_: false,
                        copy_forced: true,
                        screen_x,
                        screen_y,
                        staging: None,
                        error: Some(error),
                    },
                };
                let _ = tx.send(payload);
                (waker)();
            });

            Ok(())
        }
    }

    impl NaygoDropTarget_Impl {
        fn send_payload(&self, payload: DropPayload) {
            let _ = self.tx.send(payload);
            (self.waker)();
        }

        fn send_virtual_error(&self, screen_x: i32, screen_y: i32, error: String) {
            self.send_payload(DropPayload {
                paths: Vec::new(),
                move_: false,
                copy_forced: true,
                screen_x,
                screen_y,
                staging: None,
                error: Some(error),
            });
        }

        /// Reportar el punto actual del cursor (PANTALLA, físico) por el canal de hover y despertar
        /// la UI para que recalcule y re-pinte el panel resaltado. Tolerante: si el receptor colgó,
        /// se ignora. El SO llama a esto MUCHÍSIMO (cada movimiento); el costo del lado UI es un
        /// hit-test + comparación, así que no hace falta throttle aquí.
        fn emit_hover(&self, pt: &POINTL) {
            let _ = self.drag_tx.send(DragHover::Over {
                screen_x: pt.x,
                screen_y: pt.y,
            });
            (self.waker)();
        }

        /// Avisar que el arrastre dejó la ventana (o se soltó): la UI limpia el resaltado.
        fn emit_leave(&self) {
            let _ = self.drag_tx.send(DragHover::Leave);
            (self.waker)();
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
    pub fn register(
        hwnd: isize,
        tx: Sender<DropPayload>,
        drag_tx: Sender<DragHover>,
        waker: Waker,
    ) -> Option<Registration> {
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

        let target: IDropTarget = NaygoDropTarget {
            tx,
            drag_tx,
            waker,
            accepted_kind: AtomicU8::new(ACCEPT_NONE),
        }
        .into();

        // winit (sobre el que corre Slint) ya registró SU PROPIO IDropTarget al crear la
        // ventana: tiene drag&drop ON por defecto y Slint 1.16 no lo desactiva. Una ventana
        // Win32 admite UN SOLO drop target, así que sin esto nuestro RegisterDragDrop falla
        // con DRAGDROP_E_ALREADYREGISTERED y el target de Naygo nunca recibe los drops (el
        // drop intra-app entre paneles no llegaba al canal). Revocamos el de winit primero.
        // No se pierde nada: Naygo no consume los eventos de archivo de winit (no hay puente),
        // y winit no re-registra en runtime (solo lo hace una vez en la creación).
        // SAFETY: hwnd válido; revocar un target inexistente solo devuelve error (ignorado).
        unsafe {
            let _ = RevokeDragDrop(hwnd);
        }

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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn sondeo_cowait_mide_despacho_de_mensaje_de_ventana() {
            use std::sync::atomic::AtomicUsize;
            use windows::core::w;
            use windows::Win32::Foundation::{
                CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
            };
            use windows::Win32::System::Com::{
                CoWaitForMultipleHandles, COINIT_APARTMENTTHREADED, COWAIT_DISPATCH_CALLS,
                COWAIT_DISPATCH_WINDOW_MESSAGES,
            };
            use windows::Win32::System::LibraryLoader::GetModuleHandleW;
            use windows::Win32::System::Threading::{CreateEventW, SetEvent};
            use windows::Win32::UI::WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
                PostMessageW, RegisterClassW, UnregisterClassW, HWND_MESSAGE, MSG, PM_REMOVE,
                WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WNDCLASSW,
            };

            static SEEN: AtomicUsize = AtomicUsize::new(0);
            unsafe extern "system" fn proc(
                hwnd: HWND,
                msg: u32,
                wp: WPARAM,
                lp: LPARAM,
            ) -> LRESULT {
                if msg == WM_APP + 47 {
                    SEEN.fetch_add(1, Ordering::SeqCst);
                    LRESULT(0)
                } else {
                    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
                }
            }
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .unwrap();
            let _com = ComWorkerGuard;
            SEEN.store(0, Ordering::SeqCst);
            let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }.unwrap().0);
            let class = w!("NaygoCoWaitProbeWindow");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            assert_ne!(unsafe { RegisterClassW(&wc) }, 0);
            let hwnd = unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class,
                    w!(""),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    0,
                    0,
                    Some(HWND_MESSAGE),
                    None,
                    Some(instance),
                    None,
                )
            }
            .unwrap();
            let event = unsafe { CreateEventW(None, true, false, None) }.unwrap();
            let raw_event = event.0 as isize;
            let raw_hwnd = hwnd.0 as isize;
            let worker = std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                unsafe {
                    PostMessageW(
                        Some(HWND(raw_hwnd as *mut _)),
                        WM_APP + 47,
                        WPARAM(0),
                        LPARAM(0),
                    )
                }
                .unwrap();
                std::thread::sleep(std::time::Duration::from_millis(400));
                unsafe { SetEvent(HANDLE(raw_event as *mut _)) }.unwrap();
            });
            let flags = (COWAIT_DISPATCH_WINDOW_MESSAGES.0 | COWAIT_DISPATCH_CALLS.0) as u32;
            let waited = unsafe { CoWaitForMultipleHandles(flags, 5_000, &[event]) };
            worker.join().unwrap();
            let during_wait = SEEN.load(Ordering::SeqCst);
            let mut message = MSG::default();
            while unsafe {
                PeekMessageW(
                    &mut message,
                    Some(hwnd),
                    WM_APP + 47,
                    WM_APP + 47,
                    PM_REMOVE,
                )
            }
            .as_bool()
            {
                unsafe { DispatchMessageW(&message) };
            }
            let after_pump = SEEN.load(Ordering::SeqCst);
            unsafe { DestroyWindow(hwnd) }.unwrap();
            unsafe { UnregisterClassW(class, Some(instance)) }.unwrap();
            unsafe { CloseHandle(event) }.unwrap();
            assert!(waited.is_ok(), "espera no completó: {waited:?}");
            assert_eq!(after_pump, 1, "el mensaje de control se perdió");
            eprintln!("cowait_window_message_during_wait={during_wait} after_pump={after_pump}");
            // Sondeo informativo: no certifica repintado, entrada, winit ni portabilidad.
        }

        #[test]
        fn com_marshaling_worker_puede_consultar_hdrop_con_espera_sta() {
            use windows::Win32::Foundation::{CloseHandle, HANDLE};
            use windows::Win32::System::Com::{
                CoWaitForMultipleHandles, COINIT_APARTMENTTHREADED, COWAIT_DISPATCH_CALLS,
                COWAIT_DISPATCH_WINDOW_MESSAGES,
            };
            use windows::Win32::System::Threading::{CreateEventW, SetEvent};

            struct Event(isize);
            impl Drop for Event {
                fn drop(&mut self) {
                    let _ = unsafe { CloseHandle(HANDLE(self.0 as *mut _)) };
                }
            }
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .unwrap();
            let _com = ComWorkerGuard;
            let expected = std::env::temp_dir().join("naygo-ole-synthetic-file.txt");
            let object = crate::dnd::test_file_data_object(vec![expected.clone()]);
            let marshaled =
                unsafe { CoMarshalInterThreadInterfaceInStream(&IDataObject::IID, &object) }
                    .unwrap();
            let raw = marshaled.into_raw() as usize;
            let event = Arc::new(Event(
                unsafe { CreateEventW(None, true, false, None) }.unwrap().0 as isize,
            ));
            let worker_event = event.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                let result = (|| -> Result<Vec<PathBuf>, String> {
                    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
                        .ok()
                        .map_err(|e| e.to_string())?;
                    let _com = ComWorkerGuard;
                    let stream = unsafe { IStream::from_raw(raw as *mut _) };
                    let data: windows::core::Result<IDataObject> =
                        unsafe { CoGetInterfaceAndReleaseStream(&stream) };
                    std::mem::forget(stream);
                    let data = data.map_err(|e| e.to_string())?;
                    let mut medium =
                        unsafe { data.GetData(&hdrop_formatetc()) }.map_err(|e| e.to_string())?;
                    let paths = unsafe {
                        crate::clipboard::windows_impl::extract_hdrop_paths(HDROP(
                            medium.u.hGlobal.0,
                        ))
                    };
                    unsafe { ReleaseStgMedium(&mut medium) };
                    Ok(paths)
                })();
                let _ = tx.send(result);
                let _ = unsafe { SetEvent(HANDLE(worker_event.0 as *mut _)) };
            });
            // Prueba limitada de COM STA/MTA; no demuestra respuesta de Slint ni del proveedor real.
            let handle = HANDLE(event.0 as *mut _);
            let waited_with_dispatch = unsafe {
                CoWaitForMultipleHandles(COWAIT_DISPATCH_WINDOW_MESSAGES.0 as u32, 5_000, &[handle])
            };
            // La prueba exige llamadas COM, no respuesta de UI. El flag de mensajes puede ser
            // rechazado según el entorno; en ese caso se prueba solo la espera COM ordinaria.
            let waited = match waited_with_dispatch {
                Ok(index) => Ok(index),
                Err(error) => {
                    eprintln!("despacho de ventana no disponible: {error:?}");
                    let flags =
                        (COWAIT_DISPATCH_WINDOW_MESSAGES.0 | COWAIT_DISPATCH_CALLS.0) as u32;
                    match unsafe { CoWaitForMultipleHandles(flags, 5_000, &[handle]) } {
                        Ok(index) => {
                            eprintln!("espera con ambos flags completó");
                            Ok(index)
                        }
                        Err(error) => {
                            eprintln!("despacho combinado no disponible: {error:?}");
                            unsafe { CoWaitForMultipleHandles(0, 5_000, &[handle]) }
                        }
                    }
                }
            };
            assert!(waited.is_ok(), "la espera COM no completó: {waited:?}");
            assert_eq!(rx.recv().unwrap().unwrap(), vec![expected]);
            worker.join().unwrap();
        }

        #[test]
        fn archivador_con_ambos_formatos_prefiere_staging_virtual() {
            assert_eq!(preferred_drop_kind(true, true), ACCEPT_VIRTUAL);
            assert_eq!(preferred_drop_kind(true, false), ACCEPT_VIRTUAL);
            assert_eq!(preferred_drop_kind(false, true), ACCEPT_HDROP);
            assert_eq!(preferred_drop_kind(false, false), ACCEPT_NONE);
        }

        #[test]
        fn hdrop_temporal_de_7zip_se_detecta_y_se_snapshottea() {
            let temp = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
            let seven_zip_dir = temp.path().join("7zE1234567");
            std::fs::create_dir(&seven_zip_dir).unwrap();
            let source = seven_zip_dir.join("LEEME.txt");
            std::fs::write(&source, b"contenido de prueba").unwrap();

            assert!(is_7zip_temporary_hdrop(&source));
            assert!(needs_ephemeral_hdrop_staging(std::slice::from_ref(&source)));

            let opened = open_ephemeral_hdrop_files(std::slice::from_ref(&source)).unwrap();
            // El staging se construye desde el handle abierto, no desde la ruta de 7-Zip.
            std::fs::remove_file(&source).unwrap();
            let (paths, _guard) = stage_opened_hdrop_files(opened).unwrap();
            assert_eq!(paths.len(), 1);
            assert_eq!(std::fs::read(&paths[0]).unwrap(), b"contenido de prueba");
        }

        #[test]
        fn hdrop_temporal_de_archivo_funciona_por_interfaz_ole() {
            let temp = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
            let parent = temp.path().join("7zE1234567");
            std::fs::create_dir(&parent).unwrap();
            let source = parent.join("file.txt");
            std::fs::write(&source, b"contenido").unwrap();
            let object = crate::dnd::test_file_data_object(vec![source]);
            let (tx, rx) = std::sync::mpsc::channel();
            let (hover_tx, _hover_rx) = std::sync::mpsc::channel();
            let target: IDropTarget = NaygoDropTarget {
                tx,
                drag_tx: hover_tx,
                waker: Arc::new(|| {}),
                accepted_kind: AtomicU8::new(ACCEPT_NONE),
            }
            .into();
            let point = POINTL { x: 10, y: 10 };
            let mut effect = DROPEFFECT_COPY;
            unsafe {
                target
                    .DragEnter(&object, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                    .unwrap();
                target
                    .Drop(&object, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                    .unwrap();
            }
            let payload = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            assert!(payload.error.is_none(), "drop falló: {:?}", payload.error);
            assert!(payload.staging.is_some());
            assert_eq!(payload.paths.len(), 1);
            assert_eq!(std::fs::read(&payload.paths[0]).unwrap(), b"contenido");
        }

        // Caso de aceptación pendiente: el proveedor sintético entrega CF_HDROP como un
        // archivador, pero no reproduce extracción ni ciclo de vida COM de 7-Zip.
        #[test]
        #[ignore = "pendiente soporte de carpetas temporales y validación OLE real"]
        fn hdrop_temporal_de_carpeta_preserva_arbol_y_vacios() {
            let temp = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
            let source = temp.path().join("7zE8765432").join("carpeta");
            std::fs::create_dir_all(source.join("nested/empty")).unwrap();
            std::fs::write(source.join("nested/file.txt"), b"contenido").unwrap();
            let object = crate::dnd::test_file_data_object(vec![source]);
            let (tx, rx) = std::sync::mpsc::channel();
            let (hover_tx, _hover_rx) = std::sync::mpsc::channel();
            let target: IDropTarget = NaygoDropTarget {
                tx,
                drag_tx: hover_tx,
                waker: Arc::new(|| {}),
                accepted_kind: AtomicU8::new(ACCEPT_NONE),
            }
            .into();
            let point = POINTL { x: 10, y: 10 };
            let mut effect = DROPEFFECT_COPY;
            unsafe {
                target
                    .DragEnter(&object, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                    .unwrap();
                target
                    .Drop(&object, MODIFIERKEYS_FLAGS(0), point, &mut effect)
                    .unwrap();
            }
            let payload = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
            assert!(payload.error.is_none(), "drop falló: {:?}", payload.error);
            assert!(payload.staging.is_some());
            assert_eq!(payload.paths.len(), 1);
            assert_eq!(
                std::fs::read(payload.paths[0].join("nested/file.txt")).unwrap(),
                b"contenido"
            );
            assert!(payload.paths[0].join("nested/empty").is_dir());
        }

        #[test]
        fn virtual_path_rechaza_traversal_ads_y_dispositivos() {
            assert_eq!(
                sanitize_virtual_path("docs\\informe.txt").unwrap(),
                PathBuf::from("docs\\informe.txt")
            );
            for unsafe_name in [
                "..\\escape.txt",
                "C:\\absoluto.txt",
                "docs\\flujo:oculto",
                "CON.txt",
                "docs\\termina. ",
            ] {
                assert!(
                    sanitize_virtual_path(unsafe_name).is_err(),
                    "debía rechazar {unsafe_name}"
                );
            }
        }

        #[test]
        fn top_level_no_duplica_hijos_de_una_carpeta_virtual() {
            let descriptors = vec![
                VirtualDescriptor {
                    relative: PathBuf::from("carpeta"),
                    is_dir: true,
                },
                VirtualDescriptor {
                    relative: PathBuf::from("carpeta\\uno.txt"),
                    is_dir: false,
                },
                VirtualDescriptor {
                    relative: PathBuf::from("suelto.txt"),
                    is_dir: false,
                },
            ];
            assert_eq!(
                virtual_top_level_paths(Path::new("C:\\stage"), &descriptors),
                vec![
                    PathBuf::from("C:\\stage\\carpeta"),
                    PathBuf::from("C:\\stage\\suelto.txt")
                ]
            );
        }

        #[test]
        fn descriptor_bytes_valida_limites_y_extrae_metadata() {
            let encoded: Vec<u16> = "docs".encode_utf16().collect();
            let mut file_name = [0u16; 260];
            file_name[..encoded.len()].copy_from_slice(&encoded);
            let descriptor = FILEDESCRIPTORW {
                dwFileAttributes: FILE_ATTRIBUTE_DIRECTORY.0,
                cFileName: file_name,
                ..Default::default()
            };
            let mut bytes = vec![0u8; 4 + size_of::<FILEDESCRIPTORW>()];
            bytes[..4].copy_from_slice(&1u32.to_le_bytes());
            // El wire-format es packed: escribir sin asumir alineación del Vec<u8>.
            unsafe {
                std::ptr::write_unaligned(
                    bytes.as_mut_ptr().add(4).cast::<FILEDESCRIPTORW>(),
                    descriptor,
                )
            };
            assert_eq!(
                parse_descriptor_bytes(&bytes).unwrap(),
                vec![VirtualDescriptor {
                    relative: PathBuf::from("docs"),
                    is_dir: true,
                }]
            );
            assert!(parse_descriptor_bytes(&bytes[..bytes.len() - 1]).is_err());
            assert!(parse_descriptor_bytes(&0u32.to_le_bytes()).is_err());
        }
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
        let (drag_tx, _drag_rx) = channel::<DragHover>();
        // Al salir del scope se libera sin panic; en Windows también ejerce su Drop RAII.
        let _guard = register(0, tx, drag_tx, noop_waker());
    }
}
