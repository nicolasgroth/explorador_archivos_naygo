// Naygo — papelera de Windows (Win32 IFileOperation, COM, aislado).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `move_to_trash` envía rutas a la Papelera de reciclaje vía la API COM moderna
//! `IFileOperation`. El borrado permanente NO vive aquí (lo hace `core::ops`).
//! Tolerante: errores se reportan en el `Result`, no tumban el proceso.

#[cfg(windows)]
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::{Arc, Mutex, OnceLock};

/// Identidad de un ítem que el Shell acaba de enviar a Papelera. Al deshacer se
/// busca dentro de Papelera por su ubicación y nombre originales.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashReceipt {
    pub original: PathBuf,
    /// Ubicación exacta en la Papelera local del filesystem (Linux). Windows identifica el ítem
    /// mediante la ubicación original del Shell y deja este campo vacío.
    pub trashed_path: Option<PathBuf>,
}

/// El enumerador de la Papelera puede estar desactualizado en sesiones sin Explorer (como los
/// runners alojados). `PostDeleteItem` sí entrega el elemento recién reciclado; conservamos su
/// nombre de parsing para que Deshacer no dependa de que la vista virtual se haya refrescado.
#[cfg(windows)]
static RECYCLE_ITEM_NAMES: OnceLock<Mutex<RecycleItemNames>> = OnceLock::new();

#[cfg(windows)]
const RECYCLE_ITEM_NAME_CAPACITY: usize = 1024;

#[cfg(windows)]
#[derive(Default)]
struct RecycleItemNames {
    by_original: HashMap<PathBuf, std::ffi::OsString>,
    insertion_order: VecDeque<PathBuf>,
}

#[cfg(windows)]
impl RecycleItemNames {
    fn get(&self, original: &PathBuf) -> Option<&std::ffi::OsString> {
        self.by_original.get(original)
    }

    fn insert(&mut self, original: PathBuf, parsing_name: std::ffi::OsString) {
        if self
            .by_original
            .insert(original.clone(), parsing_name)
            .is_some()
        {
            self.insertion_order.retain(|path| path != &original);
        }
        self.insertion_order.push_back(original);
        while self.by_original.len() > RECYCLE_ITEM_NAME_CAPACITY {
            let Some(oldest) = self.insertion_order.pop_front() else {
                break;
            };
            self.by_original.remove(&oldest);
        }
    }

    fn remove(&mut self, original: &PathBuf) {
        if self.by_original.remove(original).is_some() {
            self.insertion_order.retain(|path| path != original);
        }
    }
}

#[cfg(windows)]
fn recycle_item_names() -> &'static Mutex<RecycleItemNames> {
    RECYCLE_ITEM_NAMES.get_or_init(|| Mutex::new(RecycleItemNames::default()))
}

/// Avance informado por `IFileOperation`. Las unidades son las que entrega el Shell y pueden
/// incluir trabajo interno de carpetas, por lo que sirven para una barra proporcional.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrashProgress {
    pub work_total: u32,
    pub work_done: u32,
}

pub type CancelProbe = std::sync::Arc<dyn Fn() -> bool + Send + Sync>;

/// Código estable para un origen ausente cuya identidad de Papelera no pudo confirmarse.
/// La UI lo traduce; no es una instrucción para reintentar ni una garantía de recuperación.
pub const TRASH_UNCONFIRMED_IDENTITY: &str = "naygo:trash-unconfirmed-identity";

/// Error al enviar a papelera.
#[derive(Debug)]
pub enum TrashError {
    /// La papelera no está disponible en esta plataforma.
    NotSupported,
    /// La operación COM/Shell falló; el mensaje describe el HRESULT.
    Failed(String),
    /// El Shell recicló algunos orígenes, pero no pudo completar el lote. Los recibos
    /// conservan el deshacer de los elementos que sí llegaron a la Papelera.
    Partial {
        receipts: Vec<TrashReceipt>,
        failures: Vec<TrashItemFailure>,
    },
}

/// Fallo asociado a un origen: puede seguir presente o haber salido sin identidad confirmada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashItemFailure {
    /// Posición del origen en el slice que recibió `move_to_trash_with_progress`.
    pub source_index: usize,
    pub original: PathBuf,
    pub detail: String,
}

#[cfg(windows)]
pub fn move_to_trash(paths: &[PathBuf]) -> Result<Vec<TrashReceipt>, TrashError> {
    let (tx, _rx) = std::sync::mpsc::channel();
    move_to_trash_with_progress(paths, tx, std::sync::Arc::new(|| false))
}

#[cfg(windows)]
fn recycle_operation_flags() -> windows::Win32::UI::Shell::FILEOPERATION_FLAGS {
    use windows::Win32::UI::Shell::{
        FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
    };
    FOF_ALLOWUNDO | FOF_NOERRORUI | FOF_SILENT | FOF_WANTNUKEWARNING | FOFX_RECYCLEONDELETE
}

#[cfg(windows)]
pub fn move_to_trash_with_progress(
    paths: &[PathBuf],
    progress: std::sync::mpsc::Sender<TrashProgress>,
    cancelled: CancelProbe,
) -> Result<Vec<TrashReceipt>, TrashError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOperation, IFileOperation, IFileOperationProgressSink, IShellItem,
        SHCreateItemFromParsingName,
    };

    if paths.is_empty() {
        return Ok(Vec::new());
    }

    // `std::fs` acepta rutas dependientes de la unidad que `SHCreateItemFromParsingName` puede
    // rechazar. Convertir a absoluta ocurre en este worker, sin canonicalizar: canonicalize
    // sigue el reparse point final y haría que el Shell reciclase el destino de un symlink o
    // junction en vez del enlace solicitado.
    let paths = absolute_existing_paths(paths)?;

    // SAFETY: toda la secuencia COM se ejecuta dentro de un único bloque unsafe.
    // CoUninitialize SOLO se llama si CoInitializeEx realmente inicializó COM en este
    // hilo (S_OK o S_FALSE). Si el hilo ya estaba en OTRO apartment
    // (RPC_E_CHANGED_MODE), NO debe llamarse CoUninitialize: sería un balance
    // incorrecto que decrementaría el refcount de COM de otro componente del hilo
    // (relevante porque esto puede llamarse desde el hilo de UI ya inicializado por
    // Slint/winit). Los punteros (PCWSTR sobre buffers vivos) son válidos.
    unsafe {
        // En 0.62 devuelve HRESULT. `is_ok()` cubre S_OK y S_FALSE (ya inicializado en
        // este apartment); ambos requieren un CoUninitialize de cierre.
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let needs_uninit = hr.is_ok();

        let result = (|| -> Result<Vec<TrashReceipt>, TrashError> {
            let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)
                .map_err(|e| TrashError::Failed(e.to_string()))?;

            // Pedir reciclaje y permitir que el Shell advierta si solo puede borrar de forma
            // permanente. No responder automáticamente «sí» a una eventual confirmación.
            op.SetOperationFlags(recycle_operation_flags())
                .map_err(|e| TrashError::Failed(e.to_string()))?;

            let recycled_items = Arc::new(Mutex::new(Vec::new()));
            let source_aliases = Arc::new(Mutex::new(HashMap::new()));
            let mut restore_paths = HashMap::new();
            let sink: IFileOperationProgressSink = ProgressSink {
                progress,
                cancelled,
                recycled_items: recycled_items.clone(),
                source_aliases: source_aliases.clone(),
            }
            .into();
            let cookie = op
                .Advise(&sink)
                .map_err(|e| TrashError::Failed(e.to_string()))?;

            for path in &paths {
                // Expandir el nombre antes de reciclarlo: el alias 8.3 desaparece al moverlo a
                // Papelera y usarlo como nombre de destino restauraría otro nombre. No resolver
                // el destino de junctions ni enlaces. Si no se puede consultar, no borrar.
                let restore_path = long_path_name(path).map_err(|error| {
                    TrashError::Failed(format!("no se pudo obtener el nombre original: {error}"))
                })?;
                restore_paths.insert(path.clone(), restore_path);
                let wide: Vec<u16> = path
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)
                    .map_err(|e| TrashError::Failed(e.to_string()))?;
                // El Shell puede expandir nombres 8.3 o variar la representación. Pedimos su
                // nombre ANTES de borrar y lo asociamos al origen exacto; no canonicalizamos ni
                // seguimos el último componente (que puede ser un junction o symlink).
                let shell_name =
                    shell_display_name(&item, windows::Win32::UI::Shell::SIGDN_FILESYSPATH)
                        .ok_or_else(|| {
                            TrashError::Failed(
                                "no se pudo obtener la identidad Shell del origen".to_string(),
                            )
                        })?;
                let shell_path = PathBuf::from(shell_name);
                let mut aliases = source_aliases.lock().map_err(|_| {
                    TrashError::Failed(
                        "no se pudo registrar la identidad Shell del origen".to_string(),
                    )
                })?;
                if aliases.insert(shell_path, path.clone()).is_some() {
                    return Err(TrashError::Failed(
                        "la selección contiene el mismo origen más de una vez".to_string(),
                    ));
                }
                drop(aliases);
                op.DeleteItem(&item, None)
                    .map_err(|e| TrashError::Failed(e.to_string()))?;
            }

            let performed = op.PerformOperations().map_err(|e| e.to_string());
            let _ = op.Unadvise(cookie);
            let aborted = op.GetAnyOperationsAborted().map(|value| value.as_bool());
            let callback_results = recycled_items
                .lock()
                .map(|items| items.clone())
                .unwrap_or_default();
            let mut inspection_failures = HashMap::new();
            let mut still_present = Vec::new();
            for path in &paths {
                match path_entry_exists(path) {
                    Ok(true) => still_present.push(path.clone()),
                    Ok(false) => {}
                    Err(error) => {
                        still_present.push(path.clone());
                        inspection_failures.entry(path.clone()).or_insert_with(|| {
                            format!("no se pudo comprobar el resultado de Papelera: {error}")
                        });
                    }
                }
            }
            // Diagnóstico de fixtures en CI: distingue callback Shell y
            // desaparición de ruta. No se compila en Naygo ni registra rutas de usuarios.
            #[cfg(test)]
            {
                let callback_facts: Vec<_> = callback_results
                    .iter()
                    .map(|item| {
                        (
                            item.original.as_ref().and_then(|original| {
                                paths
                                    .iter()
                                    .position(|path| windows_paths_equal(path, original))
                            }),
                            item.original_present,
                            item.newly_created_present,
                            item.parsing_name.is_some(),
                            item.hr_code,
                            item.failure.is_some(),
                        )
                    })
                    .collect();
                let remaining: Vec<_> = paths
                    .iter()
                    .map(|path| {
                        still_present
                            .iter()
                            .any(|item| windows_paths_equal(item, path))
                    })
                    .collect();
                eprintln!(
                    "trash-test callback_facts={callback_facts:?} inspection_failure_count={} still_present={remaining:?} performed_ok={} aborted={aborted:?}",
                    inspection_failures.len(), performed.is_ok()
                );
            }
            let global_failure = performed.err().or_else(|| match aborted {
                Ok(true) => Some("Windows canceló una o más eliminaciones".to_string()),
                Err(error) => Some(error.to_string()),
                Ok(false) => None,
            });
            let (mut receipts, failures) = classify_delete_results(
                &paths,
                &callback_results,
                &still_present,
                &inspection_failures,
                global_failure.as_deref(),
            );

            // El nombre de parsing proviene de `psiNewlyCreated` en PostDeleteItem y puede
            // resolver el ítem aunque la enumeración de FOLDERID_RecycleBinFolder esté vacía o
            // desactualizada. Solo se registran recibos cuya identidad fue confirmada.
            for receipt in &mut receipts {
                let item_name = callback_results
                    .iter()
                    .find(|item| {
                        item.original
                            .as_ref()
                            .is_some_and(|path| windows_paths_equal(path, &receipt.original))
                    })
                    .and_then(|item| item.parsing_name.as_ref());
                let restore_path = restore_paths
                    .get(&receipt.original)
                    .cloned()
                    .unwrap_or_else(|| receipt.original.clone());
                if let (Some(item_name), Ok(mut cache)) = (item_name, recycle_item_names().lock()) {
                    cache.insert(restore_path.clone(), item_name.clone());
                }
                receipt.original = restore_path;
            }
            if failures.is_empty() {
                Ok(receipts)
            } else {
                Err(TrashError::Partial { receipts, failures })
            }
        })();

        if needs_uninit {
            CoUninitialize();
        }
        result
    }
}

#[cfg(windows)]
fn long_path_name(path: &std::path::Path) -> std::io::Result<PathBuf> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetLongPathNameW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // GetLongPathName expande componentes 8.3; no hace canonicalize del destino de un enlace.
    let required = unsafe { GetLongPathNameW(PCWSTR(wide.as_ptr()), None) };
    if required == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut buffer = vec![0u16; required as usize];
    let written = unsafe { GetLongPathNameW(PCWSTR(wide.as_ptr()), Some(&mut buffer)) };
    if written == 0 {
        return Err(std::io::Error::last_os_error());
    }
    if written as usize >= buffer.len() {
        return Err(std::io::Error::other("la ruta cambió durante la consulta"));
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &buffer[..written as usize],
    )))
}

/// Convierte rutas existentes a su forma absoluta para las API Shell, que no comparten la
/// tolerancia de `std::fs` a las rutas dependientes de unidad como `D:archivo.txt`.
#[cfg(windows)]
fn absolute_existing_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, TrashError> {
    paths
        .iter()
        .map(|path| {
            std::path::absolute(path)
                .and_then(|absolute| std::fs::symlink_metadata(&absolute).map(|_| absolute))
                .map(shell_parsing_path)
                .map_err(|error| {
                    TrashError::Failed(format!(
                        "no se pudo resolver la ruta para Papelera ({}): {error}",
                        path.display()
                    ))
                })
        })
        .collect()
}

#[cfg(windows)]
fn path_entry_exists(path: &std::path::Path) -> Result<bool, std::io::Error> {
    path_entry_presence(std::fs::symlink_metadata(path).map(|_| ()))
}

#[cfg(any(windows, test))]
fn path_entry_presence(metadata: Result<(), std::io::Error>) -> Result<bool, std::io::Error> {
    match metadata {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
#[derive(Clone, Debug)]
struct DeleteCallbackResult {
    original: Option<PathBuf>,
    parsing_name: Option<std::ffi::OsString>,
    failure: Option<String>,
    #[cfg(test)]
    newly_created_present: bool,
    #[cfg(test)]
    original_present: bool,
    #[cfg(test)]
    hr_code: i32,
}

#[cfg(windows)]
fn classify_delete_results(
    paths: &[PathBuf],
    callback_results: &[DeleteCallbackResult],
    still_present: &[PathBuf],
    inspection_failures: &HashMap<PathBuf, String>,
    global_failure: Option<&str>,
) -> (Vec<TrashReceipt>, Vec<TrashItemFailure>) {
    let mut receipts = Vec::new();
    let mut failures = Vec::new();
    for (source_index, original) in paths.iter().enumerate() {
        let callback = callback_results.iter().find(|item| {
            item.original
                .as_ref()
                .is_some_and(|path| windows_paths_equal(path, original))
        });
        let present = still_present
            .iter()
            .any(|path| windows_paths_equal(path, original));
        let confirmed =
            callback.is_some_and(|item| item.failure.is_none() && item.parsing_name.is_some());
        if confirmed {
            receipts.push(TrashReceipt {
                original: original.clone(),
                trashed_path: None,
            });
            if !present {
                continue;
            }
        }
        let detail = if present {
            callback
                .and_then(|item| item.failure.as_deref())
                .or_else(|| {
                    inspection_failures
                        .iter()
                        .find(|(path, _)| windows_paths_equal(path, original))
                        .map(|(_, error)| error.as_str())
                })
                .or(global_failure)
                .unwrap_or("el elemento sigue presente después de la operación Shell")
        } else {
            TRASH_UNCONFIRMED_IDENTITY
        };
        failures.push(TrashItemFailure {
            source_index,
            original: original.clone(),
            detail: detail.to_string(),
        });
    }
    (receipts, failures)
}

/// Compara identidad de ruta Windows según reglas ordinales sin perder unidades UTF-16 inválidas.
/// Esto se usa para asociar callbacks del Shell, recibos y fallos con su origen; una conversión
/// lossy podría fusionar dos nombres distintos. También normaliza `/` y conserva comparación
/// insensible a caja como en el comportamiento previo.
#[cfg(windows)]
fn windows_paths_equal(left: &std::path::Path, right: &std::path::Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};

    let normalize_separators = |path: &std::path::Path| {
        path.as_os_str()
            .encode_wide()
            .map(|unit| {
                if unit == b'/' as u16 {
                    b'\\' as u16
                } else {
                    unit
                }
            })
            .collect::<Vec<_>>()
    };
    let left_wide = normalize_separators(left);
    let right_wide = normalize_separators(right);
    if left_wide.is_empty() || right_wide.is_empty() {
        return left_wide.is_empty() && right_wide.is_empty();
    }

    // SAFETY: both slices remain alive for the call; CompareStringOrdinal takes explicit
    // slices and compares their UTF-16 code units without converting through UTF-8.
    unsafe { CompareStringOrdinal(&left_wide, &right_wide, true) == CSTR_EQUAL }
}

/// Recupera una cadena COM del Shell sin perder unidades UTF-16 ni filtrar su asignación.
#[cfg(windows)]
fn shell_display_name(
    item: &windows::Win32::UI::Shell::IShellItem,
    kind: windows::Win32::UI::Shell::SIGDN,
) -> Option<std::ffi::OsString> {
    let value = unsafe { item.GetDisplayName(kind) }.ok()?;
    take_shell_string(value)
}

/// Copia y libera un resultado LPWSTR asignado con el asignador de tareas COM.
#[cfg(windows)]
fn take_shell_string(value: windows::core::PWSTR) -> Option<std::ffi::OsString> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::System::Com::CoTaskMemFree;
    if value.0.is_null() {
        return None;
    }
    // SAFETY: el Shell entrega una cadena UTF-16 terminada en NUL.
    let result = std::ffi::OsString::from_wide(unsafe { value.as_wide() });
    // SAFETY: GetDisplayName/GetString asignan el buffer con el asignador de tareas COM.
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    Some(result)
}

#[cfg(windows)]
fn find_original_alias(
    aliases: &HashMap<PathBuf, PathBuf>,
    shell_path: &std::path::Path,
) -> Option<PathBuf> {
    aliases.get(shell_path).cloned().or_else(|| {
        let mut matches = aliases
            .iter()
            .filter(|(alias, _)| windows_paths_equal(alias, shell_path));
        let (_, original) = matches.next()?;
        // Si el directorio distingue mayúsculas, una coincidencia ambigua no es identidad.
        matches.next().is_none().then(|| original.clone())
    })
}

/// Empareja un elemento enumerado de la Papelera con un recibo reconstruido. El Shell puede
/// devolver la carpeta original con otra capitalización o separadores equivalentes; comparar
/// `Path` directamente es sensible a caja y puede dejar un recibo válido sin restaurar.
#[cfg(windows)]
fn recycle_bin_entry_matches(
    deleted_from: Option<&str>,
    display_name: Option<&str>,
    original_parent: &std::path::Path,
    original_name: &std::ffi::OsStr,
) -> bool {
    deleted_from
        .is_some_and(|folder| windows_paths_equal(std::path::Path::new(folder), original_parent))
        && display_name
            .and_then(|name| std::path::Path::new(name).file_name())
            .is_some_and(|name| name == original_name)
}

/// El Shell necesita la forma de parsing habitual, no el prefijo extendido `\\?\\`; en UNC se
/// conserva el doble backslash de red.
#[cfg(windows)]
fn shell_parsing_path(path: PathBuf) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    let unc: Vec<u16> = r"\\?\UNC\".encode_utf16().collect();
    let device: Vec<u16> = r"\\?\".encode_utf16().collect();
    if let Some(rest) = wide.strip_prefix(unc.as_slice()) {
        let mut normalized = vec![b'\\' as u16, b'\\' as u16];
        normalized.extend_from_slice(rest);
        PathBuf::from(std::ffi::OsString::from_wide(&normalized))
    } else if wide.starts_with(&device)
        && wide.len() >= 7
        && matches!(wide[4], 65..=90 | 97..=122)
        && wide[5] == b':' as u16
        && wide[6] == b'\\' as u16
    {
        PathBuf::from(std::ffi::OsString::from_wide(&wide[4..]))
    } else {
        path
    }
}

#[cfg(windows)]
#[windows::core::implement(windows::Win32::UI::Shell::IFileOperationProgressSink)]
struct ProgressSink {
    progress: std::sync::mpsc::Sender<TrashProgress>,
    cancelled: CancelProbe,
    /// Resultado por ruta que informa `PostDeleteItem`; no se presupone que los callbacks
    /// lleguen en el mismo orden en que se encolaron las operaciones.
    recycled_items: Arc<Mutex<Vec<DeleteCallbackResult>>>,
    source_aliases: Arc<Mutex<HashMap<PathBuf, PathBuf>>>,
}

#[cfg(windows)]
#[allow(non_snake_case)]
impl windows::Win32::UI::Shell::IFileOperationProgressSink_Impl for ProgressSink_Impl {
    fn StartOperations(&self) -> windows_core::Result<()> {
        Ok(())
    }
    fn FinishOperations(&self, hrresult: windows_core::HRESULT) -> windows_core::Result<()> {
        hrresult.ok()
    }
    fn PreRenameItem(
        &self,
        _: u32,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: &windows_core::PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PostRenameItem(
        &self,
        _: u32,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: &windows_core::PCWSTR,
        hrresult: windows_core::HRESULT,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
    ) -> windows_core::Result<()> {
        hrresult.ok()
    }
    fn PreMoveItem(
        &self,
        _: u32,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: &windows_core::PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PostMoveItem(
        &self,
        _: u32,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: &windows_core::PCWSTR,
        _: windows_core::HRESULT,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _: u32,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: &windows_core::PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PostCopyItem(
        &self,
        _: u32,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: &windows_core::PCWSTR,
        _: windows_core::HRESULT,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PreDeleteItem(
        &self,
        _: u32,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
    ) -> windows_core::Result<()> {
        if (self.cancelled)() {
            Err(windows_core::Error::from_hresult(windows_core::HRESULT(
                0x800704C7u32 as i32,
            )))
        } else {
            Ok(())
        }
    }
    fn PostDeleteItem(
        &self,
        _: u32,
        item: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        hrresult: windows_core::HRESULT,
        newly_created: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
    ) -> windows_core::Result<()> {
        let original = item
            .as_ref()
            .and_then(|item| shell_display_name(item, windows::Win32::UI::Shell::SIGDN_FILESYSPATH))
            .and_then(|name| {
                self.source_aliases
                    .lock()
                    .ok()
                    .and_then(|aliases| find_original_alias(&aliases, &PathBuf::from(name)))
            });
        let parsing_name = newly_created.as_ref().and_then(|item| {
            shell_display_name(
                item,
                windows::Win32::UI::Shell::SIGDN_DESKTOPABSOLUTEPARSING,
            )
        });
        if let Ok(mut items) = self.recycled_items.lock() {
            items.push(DeleteCallbackResult {
                original,
                parsing_name,
                failure: hrresult
                    .is_err()
                    .then(|| format!("DeleteItem falló ({hrresult})")),
                #[cfg(test)]
                newly_created_present: newly_created.as_ref().is_some(),
                #[cfg(test)]
                original_present: item.as_ref().is_some(),
                #[cfg(test)]
                hr_code: hrresult.0,
            });
        }
        Ok(())
    }
    fn PreNewItem(
        &self,
        _: u32,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: &windows_core::PCWSTR,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn PostNewItem(
        &self,
        _: u32,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: &windows_core::PCWSTR,
        _: &windows_core::PCWSTR,
        _: u32,
        _: windows_core::HRESULT,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
    ) -> windows_core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, total: u32, done: u32) -> windows_core::Result<()> {
        let _ = self.progress.send(TrashProgress {
            work_total: total,
            work_done: done,
        });
        if (self.cancelled)() {
            Err(windows_core::Error::from_hresult(windows_core::HRESULT(
                0x800704C7u32 as i32,
            )))
        } else {
            Ok(())
        }
    }
    fn ResetTimer(&self) -> windows_core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows_core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows_core::Result<()> {
        Ok(())
    }
}

/// Restaura ítems identificados durante `move_to_trash_with_progress` a sus ubicaciones
/// originales. Prioriza el `IShellItem` reportado por la operación y usa enumeración como
/// respaldo para recibos reconstruidos. Corre en un worker de Naygo; no toca el hilo UI.
#[cfg(windows)]
pub fn restore_from_trash(receipts: &[TrashReceipt]) -> Result<(), TrashError> {
    restore_from_trash_with_enumerator(receipts, enumerate_recycle_bin_items)
}

#[cfg(windows)]
fn restore_from_trash_with_enumerator(
    receipts: &[TrashReceipt],
    mut enumerate: impl FnMut() -> Result<Vec<windows::Win32::UI::Shell::IShellItem>, String>,
) -> Result<(), TrashError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, GUID, PCSTR, PCWSTR};
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        BHID_SFUIObject, FileOperation, IContextMenu, IFileOperation, IShellItem, IShellItem2,
        SHCreateItemFromParsingName, CMF_NORMAL, CMINVOKECOMMANDINFO, FOF_NOCONFIRMATION,
        FOF_NOERRORUI, FOF_SILENT, SIGDN_NORMALDISPLAY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{CreatePopupMenu, DestroyMenu};

    if receipts.is_empty() {
        return Ok(());
    }
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let needs_uninit = hr.is_ok();
        let result = (|| -> Result<(), TrashError> {
            // Resolver primero los recibos propios evita requerir el proveedor de la carpeta
            // virtual de Papelera, que puede faltar o no actualizarse en runners sin Explorer.
            // La enumeración se crea más abajo, únicamente como respaldo para recibos reconstruidos.
            let mut candidates = None;
            let mut enumeration_error = None;
            const DELETED_FROM: PROPERTYKEY = PROPERTYKEY {
                fmtid: GUID::from_u128(0x9b174b33_40ff_11d2_a27e_00c04fc30871),
                pid: 2,
            };
            for receipt in receipts {
                match path_entry_exists(&receipt.original) {
                    Ok(true) => {
                        return Err(TrashError::Failed(format!(
                            "el destino está ocupado: {}",
                            receipt.original.display()
                        )));
                    }
                    Ok(false) => {}
                    Err(error) => {
                        return Err(TrashError::Failed(format!(
                            "no se pudo comprobar si el destino está ocupado ({}): {error}",
                            receipt.original.display()
                        )));
                    }
                }
                let original_parent = receipt.original.parent().ok_or_else(|| {
                    TrashError::Failed(format!(
                        "ruta sin carpeta padre: {}",
                        receipt.original.display()
                    ))
                })?;
                let original_name = receipt.original.file_name().unwrap_or_default();
                let direct_item = recycle_item_names()
                    .lock()
                    .ok()
                    .and_then(|cache| cache.get(&receipt.original).cloned())
                    .and_then(|parsing_name| {
                        let wide: Vec<u16> = parsing_name.encode_wide().chain(Some(0)).collect();
                        SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).ok()
                    });
                let item = if let Some(item) = direct_item {
                    item
                } else {
                    if candidates.is_none() && enumeration_error.is_none() {
                        match enumerate() {
                            Ok(items) => candidates = Some(items),
                            Err(error) => enumeration_error = Some(error),
                        }
                    }
                    let Some(position) = candidates.as_ref().and_then(|items| {
                        items.iter().position(|item| {
                            let deleted_from = item
                                .cast::<IShellItem2>()
                                .ok()
                                .and_then(|item2| item2.GetString(&DELETED_FROM).ok())
                                .and_then(take_shell_string)
                                .and_then(|value| value.into_string().ok());
                            let name = shell_display_name(item, SIGDN_NORMALDISPLAY)
                                .and_then(|value| value.into_string().ok());
                            recycle_bin_entry_matches(
                                deleted_from.as_deref(),
                                name.as_deref(),
                                original_parent,
                                original_name,
                            )
                        })
                    }) else {
                        let detail = enumeration_error
                            .as_deref()
                            .map(|error| format!("; tampoco se pudo enumerar la Papelera: {error}"))
                            .unwrap_or_default();
                        return Err(TrashError::Failed(format!(
                            "el elemento ya no está en Papelera: {}{detail}",
                            receipt.original.display(),
                        )));
                    };
                    // No reutilizar el mismo objeto lógico si una operación contiene
                    // nombres repetidos en rutas distintas o una restauración múltiple.
                    candidates
                        .as_mut()
                        .expect("la enumeración está disponible")
                        .remove(position)
                };
                // Mover el IShellItem virtual de la Papelera conserva su identidad y permite
                // restaurarlo sin depender de la vista de Explorer ni de una asociación de
                // menú contextual (ausentes en runners Windows no interactivos).
                let restored = (|| -> Result<(), String> {
                    let operation: IFileOperation =
                        CoCreateInstance(&FileOperation, None, CLSCTX_ALL)
                            .map_err(|error| error.to_string())?;
                    operation
                        .SetOperationFlags(FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT)
                        .map_err(|error| error.to_string())?;
                    let parent_wide: Vec<u16> = original_parent
                        .as_os_str()
                        .encode_wide()
                        .chain(Some(0))
                        .collect();
                    let parent: IShellItem =
                        SHCreateItemFromParsingName(PCWSTR(parent_wide.as_ptr()), None)
                            .map_err(|error| error.to_string())?;
                    let name_wide: Vec<u16> = original_name.encode_wide().chain(Some(0)).collect();
                    operation
                        .MoveItem(&item, &parent, PCWSTR(name_wide.as_ptr()), None)
                        .map_err(|error| error.to_string())?;
                    operation
                        .PerformOperations()
                        .map_err(|error| error.to_string())?;
                    if operation
                        .GetAnyOperationsAborted()
                        .map_err(|error| error.to_string())?
                        .as_bool()
                    {
                        return Err("Windows canceló la restauración".to_string());
                    }
                    match path_entry_exists(&receipt.original) {
                        Ok(true) => {}
                        Ok(false) => {
                            return Err("el Shell no devolvió el archivo a su ruta original".into())
                        }
                        Err(error) => {
                            return Err(format!(
                                "no se pudo comprobar el resultado de la restauración: {error}"
                            ));
                        }
                    }
                    Ok(())
                })();

                if let Err(move_error) = restored {
                    match path_entry_exists(&receipt.original) {
                        Ok(true) => {}
                        Ok(false) => {
                            // Respaldo para proveedores Shell que exponen `undelete` pero no
                            // implementan MoveItem desde su carpeta virtual de Papelera.
                            let menu: IContextMenu = item
                            .BindToHandler(None, &BHID_SFUIObject)
                            .map_err(|error| {
                                TrashError::Failed(format!(
                                    "no se pudo restaurar con IFileOperation ({move_error}) ni abrir el menú Shell: {error}"
                                ))
                            })?;
                            let hmenu = CreatePopupMenu()
                                .map_err(|error| TrashError::Failed(error.to_string()))?;
                            let queried =
                                menu.QueryContextMenu(hmenu, 0, 1, 0x7fff, CMF_NORMAL).ok();
                            let _ = DestroyMenu(hmenu);
                            queried.map_err(|error| TrashError::Failed(error.to_string()))?;
                            const UNDELETE: &[u8] = b"undelete\0";
                            let command = CMINVOKECOMMANDINFO {
                                cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                                lpVerb: PCSTR(UNDELETE.as_ptr()),
                                ..Default::default()
                            };
                            menu.InvokeCommand(&command)
                                .map_err(|error| TrashError::Failed(error.to_string()))?;
                        }
                        Err(error) => {
                            return Err(TrashError::Failed(format!(
                                "no se pudo comprobar el destino después del fallo de IFileOperation ({move_error}): {error}"
                            )));
                        }
                    }
                }
                match path_entry_exists(&receipt.original) {
                    Ok(true) => {}
                    Ok(false) => {
                        return Err(TrashError::Failed(format!(
                            "el Shell no restauró el archivo a su ruta original: {}",
                            receipt.original.display()
                        )));
                    }
                    Err(error) => {
                        return Err(TrashError::Failed(format!(
                            "no se pudo comprobar la ruta después de restaurar ({}): {error}",
                            receipt.original.display()
                        )));
                    }
                }
                if let Ok(mut cache) = recycle_item_names().lock() {
                    cache.remove(&receipt.original);
                }
            }
            Ok(())
        })();
        if needs_uninit {
            CoUninitialize();
        }
        result
    }
}

/// Enumera los elementos de Papelera solo para recibos que no conservan un nombre de parsing
/// directo. Algunos runners Windows no exponen el proveedor de la carpeta virtual, pero eso no
/// debe impedir restaurar un elemento cuya identidad entregó `PostDeleteItem`.
#[cfg(windows)]
fn enumerate_recycle_bin_items() -> Result<Vec<windows::Win32::UI::Shell::IShellItem>, String> {
    use windows::Win32::UI::Shell::{
        BHID_EnumItems, FOLDERID_RecycleBinFolder, IEnumShellItems, SHGetKnownFolderItem,
        KF_FLAG_DEFAULT,
    };

    // SAFETY: COM se inicializó en el hilo llamador y las referencias/arrays viven durante cada
    // llamada. Todo el acceso ocurre antes de salir de ese mismo apartment.
    unsafe {
        let recycle_bin: windows::Win32::UI::Shell::IShellItem =
            SHGetKnownFolderItem(&FOLDERID_RecycleBinFolder, KF_FLAG_DEFAULT, None)
                .map_err(|error| error.to_string())?;
        let enumerator: IEnumShellItems = recycle_bin
            .BindToHandler(None, &BHID_EnumItems)
            .map_err(|error| error.to_string())?;
        let mut candidates = Vec::new();
        loop {
            let mut next = [None];
            if enumerator.Next(&mut next, None).is_err() {
                break;
            }
            let Some(item) = next.into_iter().next().flatten() else {
                break;
            };
            candidates.push(item);
        }
        Ok(candidates)
    }
}

/// Backend Linux conforme a FreeDesktop Trash Specification.
#[cfg(target_os = "linux")]
mod linux_trash;

#[cfg(target_os = "linux")]
pub use linux_trash::{move_to_trash, move_to_trash_with_progress, restore_from_trash};

/// Stub para plataformas sin backend de papelera.
#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn move_to_trash(_paths: &[PathBuf]) -> Result<Vec<TrashReceipt>, TrashError> {
    Err(TrashError::NotSupported)
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn move_to_trash_with_progress(
    _paths: &[PathBuf],
    _progress: std::sync::mpsc::Sender<TrashProgress>,
    _cancelled: CancelProbe,
) -> Result<Vec<TrashReceipt>, TrashError> {
    Err(TrashError::NotSupported)
}

#[cfg(all(not(windows), not(target_os = "linux")))]
pub fn restore_from_trash(_receipts: &[TrashReceipt]) -> Result<(), TrashError> {
    Err(TrashError::NotSupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_entry_presence_only_treats_not_found_as_absence() {
        assert!(path_entry_presence(Ok(())).unwrap());
        assert!(!path_entry_presence(Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        )))
        .unwrap());
        let error = path_entry_presence(Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "inaccessible",
        )))
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[cfg(windows)]
    #[test]
    fn resultados_parciales_conservan_recibo_y_fallo_por_origen() {
        let succeeded = PathBuf::from(r"C:\temp\reciclado.txt");
        let failed = PathBuf::from(r"C:\temp\retenido.txt");
        let callbacks = vec![DeleteCallbackResult {
            original: Some(succeeded.clone()),
            parsing_name: Some("recycle-bin:item-1".into()),
            failure: None,
            newly_created_present: true,
            original_present: true,
            hr_code: 0,
        }];
        let mut fallback_failures = HashMap::new();
        fallback_failures.insert(failed.clone(), "acceso denegado".to_string());

        let (receipts, failures) = classify_delete_results(
            &[succeeded.clone(), failed.clone()],
            &callbacks,
            std::slice::from_ref(&failed),
            &fallback_failures,
            None,
        );

        assert_eq!(
            receipts,
            vec![TrashReceipt {
                original: succeeded,
                trashed_path: None,
            }]
        );
        assert_eq!(
            failures,
            vec![TrashItemFailure {
                source_index: 1,
                original: failed,
                detail: "acceso denegado".to_string(),
            }]
        );
    }

    #[cfg(windows)]
    #[test]
    fn origen_ausente_sin_identidad_confirmada_no_genera_recibo() {
        let no_callback = PathBuf::from(r"C:\temp\sin-callback.txt");
        let no_identity = PathBuf::from(r"C:\temp\sin-identidad.txt");
        let confirmed = PathBuf::from(r"C:\temp\confirmado.txt");
        let callbacks = vec![
            DeleteCallbackResult {
                original: Some(no_identity.clone()),
                parsing_name: None,
                failure: None,
                newly_created_present: false,
                original_present: true,
                hr_code: 0,
            },
            DeleteCallbackResult {
                original: Some(confirmed.clone()),
                parsing_name: Some("item-confirmado".into()),
                failure: None,
                newly_created_present: true,
                original_present: true,
                hr_code: 0,
            },
        ];
        let (receipts, failures) = classify_delete_results(
            &[no_callback, no_identity, confirmed.clone()],
            &callbacks,
            &[],
            &HashMap::new(),
            None,
        );
        assert_eq!(
            receipts,
            vec![TrashReceipt {
                original: confirmed,
                trashed_path: None
            }]
        );
        assert_eq!(failures.len(), 2);
        assert_eq!((failures[0].source_index, failures[1].source_index), (0, 1));
        assert!(failures
            .iter()
            .all(|failure| failure.detail == TRASH_UNCONFIRMED_IDENTITY));
    }

    #[cfg(windows)]
    #[test]
    fn reciclaje_no_confirma_automaticamente_una_eliminacion_permanente() {
        use windows::Win32::UI::Shell::{
            FOFX_RECYCLEONDELETE, FOF_NOCONFIRMATION, FOF_WANTNUKEWARNING,
        };
        let flags = recycle_operation_flags();
        assert_ne!(flags.0 & FOFX_RECYCLEONDELETE.0, 0);
        assert_ne!(flags.0 & FOF_WANTNUKEWARNING.0, 0);
        assert_eq!(flags.0 & FOF_NOCONFIRMATION.0, 0);
    }

    #[cfg(windows)]
    #[test]
    fn recibo_confirmado_se_conserva_si_la_ruta_vuelve_a_ocuparse() {
        let path = PathBuf::from(r"C:\temp\recreado.txt");
        let callbacks = vec![DeleteCallbackResult {
            original: Some(path.clone()),
            parsing_name: Some("reciclado".into()),
            failure: None,
            newly_created_present: true,
            original_present: true,
            hr_code: 0,
        }];
        let (receipts, failures) = classify_delete_results(
            std::slice::from_ref(&path),
            &callbacks,
            std::slice::from_ref(&path),
            &HashMap::new(),
            None,
        );
        assert_eq!(
            receipts,
            vec![TrashReceipt {
                original: path,
                trashed_path: None
            }]
        );
        assert_eq!(failures.len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn papelera_recicla_el_junction_sin_seguir_su_destino() {
        use std::os::windows::fs::MetadataExt;
        use std::process::Command;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("destino real");
        let junction = dir.path().join("enlace a destino");
        std::fs::create_dir(&target).unwrap();
        let sentinel = target.join("conservar.txt");
        std::fs::write(&sentinel, b"debe permanecer intacto").unwrap();
        let output = Command::new("cmd.exe")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().expect("ruta Windows válida"),
                target.to_str().expect("ruta Windows válida"),
            ])
            .output()
            .expect("mklink debe estar disponible en Windows");
        assert!(
            output.status.success(),
            "no se pudo crear el junction: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_ne!(
            std::fs::symlink_metadata(&junction)
                .unwrap()
                .file_attributes()
                & 0x400,
            0,
            "el fixture debe ser un reparse point"
        );

        let absolute = absolute_existing_paths(std::slice::from_ref(&junction)).unwrap();
        assert_eq!(absolute[0], std::path::absolute(&junction).unwrap());
        assert_ne!(absolute[0], std::fs::canonicalize(&junction).unwrap());

        let receipts =
            move_to_trash(std::slice::from_ref(&junction)).expect("el junction debe ir a Papelera");
        assert_eq!(receipts.len(), 1);
        assert!(!junction.exists(), "debe retirarse el junction original");
        assert!(target.is_dir(), "el destino real debe conservarse");
        assert_eq!(
            std::fs::read(&sentinel).unwrap(),
            b"debe permanecer intacto",
            "el contenido del destino no debe tocarse"
        );

        restore_from_trash(&receipts).expect("el junction debe poder restaurarse");
        assert!(junction.is_dir());
        assert!(
            std::fs::symlink_metadata(&junction)
                .unwrap()
                .file_attributes()
                & 0x400
                != 0
        );
        assert_eq!(std::fs::read(sentinel).unwrap(), b"debe permanecer intacto");
    }

    #[cfg(windows)]
    #[test]
    fn path_entry_exists_detecta_junction_colgante() {
        use std::process::Command;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("destino ya eliminado");
        let junction = dir.path().join("junction colgante");
        std::fs::create_dir(&target).unwrap();
        let output = Command::new("cmd.exe")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().expect("ruta Windows válida"),
                target.to_str().expect("ruta Windows válida"),
            ])
            .output()
            .expect("mklink debe estar disponible en Windows");
        assert!(
            output.status.success(),
            "no se pudo crear el junction: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        std::fs::remove_dir(&target).unwrap();
        assert!(
            !junction.exists(),
            "Path::exists sigue el destino y debe detectar el junction como colgante"
        );
        assert!(
            path_entry_exists(&junction).unwrap(),
            "symlink_metadata debe seguir viendo la entrada reparse ocupada"
        );
    }

    #[cfg(windows)]
    #[test]
    fn restore_rechaza_junction_colgante_como_destino_ocupado() {
        use std::process::Command;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("destino ya eliminado");
        let junction = dir.path().join("junction ocupado");
        std::fs::create_dir(&target).unwrap();
        let output = Command::new("cmd.exe")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().expect("ruta Windows válida"),
                target.to_str().expect("ruta Windows válida"),
            ])
            .output()
            .expect("mklink debe estar disponible en Windows");
        assert!(
            output.status.success(),
            "no se pudo crear el junction: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_dir(&target).unwrap();

        let receipt = TrashReceipt {
            original: junction.clone(),
            trashed_path: None,
        };
        let mut enumeration_calls = 0;
        let result = restore_from_trash_with_enumerator(std::slice::from_ref(&receipt), || {
            enumeration_calls += 1;
            Err("un destino ocupado no debe consultar la Papelera".to_string())
        });

        let Err(TrashError::Failed(message)) = result else {
            panic!("la restauración debe rechazar el junction colgante");
        };
        assert!(message.contains("el destino está ocupado"), "{message}");
        assert_eq!(enumeration_calls, 0);
        assert!(path_entry_exists(&junction).unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn comparacion_de_rutas_no_colapsa_utf16_invalido_distinto() {
        use std::os::windows::ffi::OsStringExt;

        let mut left = r"C:\temp\item".encode_utf16().collect::<Vec<_>>();
        left.push(0xD800);
        let mut right = r"C:\temp\item".encode_utf16().collect::<Vec<_>>();
        right.push(0xD801);

        let left = PathBuf::from(std::ffi::OsString::from_wide(&left));
        let right = PathBuf::from(std::ffi::OsString::from_wide(&right));
        assert_ne!(left, right);
        assert!(
            !windows_paths_equal(&left, &right),
            "rutas UTF-16 distintas no deben coincidir por una conversión con sustitución"
        );
        assert!(windows_paths_equal(&left, &left));
    }

    #[cfg(windows)]
    #[test]
    fn shell_parsing_path_preserva_utf16_invalido_y_rutas_de_dispositivo() {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        for (input, expected) in [
            (r"\\?\C:\temp\archivo", r"C:\temp\archivo"),
            (
                r"\\?\UNC\servidor\share\archivo",
                r"\\servidor\share\archivo",
            ),
        ] {
            let mut original: Vec<u16> = input.encode_utf16().collect();
            original.push(0xD800);
            let mut wanted: Vec<u16> = expected.encode_utf16().collect();
            wanted.push(0xD800);
            let path = PathBuf::from(std::ffi::OsString::from_wide(&original));
            assert_eq!(
                shell_parsing_path(path)
                    .as_os_str()
                    .encode_wide()
                    .collect::<Vec<_>>(),
                wanted
            );
        }
        let device = PathBuf::from(r"\\?\Volume{12345678-1234-1234-1234-123456789012}\file");
        assert_eq!(shell_parsing_path(device.clone()), device);
    }

    #[cfg(windows)]
    #[test]
    fn comparacion_de_rutas_conserva_semantica_insensible_a_caja_y_separadores() {
        assert!(windows_paths_equal(
            std::path::Path::new(r"C:\Temp\MiArchivo.txt"),
            std::path::Path::new("c:/temp/miarchivo.txt")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn alias_shell_recupera_la_ruta_original_sin_resolver_junctions() {
        let original = PathBuf::from(r"C:\Users\RUNNER~1\Temp\enlace");
        let shell_name = PathBuf::from(r"C:\Users\runneradmin\Temp\enlace");
        let mut aliases = HashMap::new();
        aliases.insert(shell_name, original.clone());
        assert_eq!(
            find_original_alias(
                &aliases,
                std::path::Path::new("c:/USERS/RUNNERADMIN/temp/enlace")
            ),
            Some(original)
        );
        assert!(find_original_alias(
            &aliases,
            std::path::Path::new(r"C:\Users\runneradmin\Temp\otro")
        )
        .is_none());
        aliases.insert(PathBuf::from(r"C:\case\A"), PathBuf::from("first"));
        aliases.insert(PathBuf::from(r"c:\case\a"), PathBuf::from("second"));
        assert!(find_original_alias(&aliases, std::path::Path::new("C:/CASE/a")).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn ruta_corta_se_recicla_y_restaura_por_identidad_shell() {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetShortPathNameW;

        let dir = tempfile::tempdir().unwrap();
        let folder = dir
            .path()
            .join("Carpeta larga para prueba de nombres cortos");
        std::fs::create_dir(&folder).unwrap();
        let file = folder.join("archivo largo de papelera.txt");
        std::fs::write(&file, b"contenido de prueba").unwrap();
        let wide: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
        let required = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), None) };
        assert!(
            required > 0,
            "GetShortPathNameW falló para fixture existente"
        );
        let mut output = vec![0u16; required as usize];
        let written = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), Some(&mut output)) };
        assert!(written > 0 && (written as usize) < output.len());
        let short = PathBuf::from(std::ffi::OsString::from_wide(&output[..written as usize]));
        if windows_paths_equal(&short, &file) {
            eprintln!("el volumen no expuso un alias 8.3 distinto; el test de mapeo sintético cubre la asociación");
            return;
        }
        let receipts = move_to_trash(std::slice::from_ref(&short)).expect("reciclar alias corto");
        assert_eq!(receipts.len(), 1);
        assert!(recycle_item_names()
            .lock()
            .unwrap()
            .by_original
            .contains_key(&receipts[0].original));
        restore_from_trash_with_enumerator(&receipts, || Err("no debe enumerar".to_string()))
            .expect("restaurar usando la identidad del Shell");
        assert_eq!(std::fs::read(&file).unwrap(), b"contenido de prueba");
    }

    #[cfg(windows)]
    #[test]
    fn recibo_reconstruido_empareja_carpeta_shell_con_caja_y_separadores_distintos() {
        assert_ne!(
            std::path::Path::new("c:/users/nicolas/documentos"),
            std::path::Path::new(r"C:\Users\Nicolas\Documentos"),
            "la igualdad léxica de Path es demasiado estricta para una ruta Windows"
        );
        assert!(recycle_bin_entry_matches(
            Some("c:/users/nicolas/documentos"),
            Some("informe.txt"),
            std::path::Path::new(r"C:\Users\Nicolas\Documentos"),
            std::ffi::OsStr::new("informe.txt"),
        ));
        assert!(!recycle_bin_entry_matches(
            Some(r"C:\Users\Nicolas\Otra carpeta"),
            Some("informe.txt"),
            std::path::Path::new(r"C:\Users\Nicolas\Documentos"),
            std::ffi::OsStr::new("informe.txt"),
        ));
        assert!(!recycle_bin_entry_matches(
            Some(r"C:\Users\Nicolas\Documentos"),
            Some("otro.txt"),
            std::path::Path::new(r"C:\Users\Nicolas\Documentos"),
            std::ffi::OsStr::new("informe.txt"),
        ));
    }

    #[cfg(windows)]
    #[test]
    fn enviar_un_archivo_a_papelera() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("borrame.txt");
        std::fs::write(&f, b"x").unwrap();
        assert!(f.exists());
        let receipts = move_to_trash(std::slice::from_ref(&f)).expect("move_to_trash falló");
        assert_eq!(receipts.len(), 1, "debe capturar el ítem de Papelera");
        assert!(!f.exists(), "el archivo debería haber ido a la papelera");
        assert!(
            recycle_item_names()
                .lock()
                .unwrap()
                .by_original
                .contains_key(&receipts[0].original),
            "PostDeleteItem debe entregar el ítem real de Papelera"
        );
        let mut enumeration_calls = 0;
        restore_from_trash_with_enumerator(&receipts, || {
            enumeration_calls += 1;
            Err("el test no debe necesitar enumeración".to_string())
        })
        .expect("el recibo directo debe restaurarse sin enumerar la Papelera");
        assert_eq!(
            enumeration_calls, 0,
            "el recibo directo no debe enumerar la Papelera"
        );
        assert!(f.exists(), "el archivo debe volver a su ruta original");
    }

    #[cfg(windows)]
    #[test]
    fn recibo_reconstruido_reporta_si_falta_el_enumerador_de_papelera() {
        let dir = tempfile::tempdir().unwrap();
        let receipt = TrashReceipt {
            original: dir.path().join("ya-no-existe.txt"),
            trashed_path: None,
        };
        let mut enumeration_calls = 0;
        let result = restore_from_trash_with_enumerator(std::slice::from_ref(&receipt), || {
            enumeration_calls += 1;
            Err("proveedor de Papelera no disponible".to_string())
        });

        let Err(TrashError::Failed(message)) = result else {
            panic!("el recibo reconstruido debe propagar el fallo del enumerador");
        };
        assert_eq!(enumeration_calls, 1, "el respaldo se intenta una sola vez");
        assert!(message.contains("ya no está en Papelera"));
        assert!(message.contains("proveedor de Papelera no disponible"));
    }

    #[cfg(windows)]
    #[test]
    fn cache_de_recibos_conserva_los_mas_recientes_en_orden_fifo() {
        let mut cache = RecycleItemNames::default();
        let paths: Vec<_> = (0..=RECYCLE_ITEM_NAME_CAPACITY)
            .map(|index| PathBuf::from(format!("item-{index}")))
            .collect();

        for (index, path) in paths.iter().take(RECYCLE_ITEM_NAME_CAPACITY).enumerate() {
            cache.insert(path.clone(), format!("parsing-{index}").into());
        }
        // Reinsertar una clave la vuelve reciente antes de que se supere la capacidad.
        cache.insert(paths[0].clone(), "parsing-0-new".into());
        cache.insert(
            paths[RECYCLE_ITEM_NAME_CAPACITY].clone(),
            format!("parsing-{}", RECYCLE_ITEM_NAME_CAPACITY).into(),
        );

        assert!(cache.get(&paths[0]).is_some());
        assert!(cache.get(&paths[1]).is_none());
        assert!(cache.get(&paths[RECYCLE_ITEM_NAME_CAPACITY]).is_some());
        assert_eq!(cache.by_original.len(), RECYCLE_ITEM_NAME_CAPACITY);
    }

    #[cfg(windows)]
    #[test]
    fn cache_de_identidad_shell_preserva_utf16_no_valido() {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let original = PathBuf::from(r"C:\temp\original");
        let value = std::ffi::OsString::from_wide(&[b'X' as u16, 0xD800]);
        let mut cache = RecycleItemNames::default();
        cache.insert(original.clone(), value);
        assert_eq!(
            cache
                .get(&original)
                .unwrap()
                .encode_wide()
                .collect::<Vec<_>>(),
            [b'X' as u16, 0xD800]
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn papelera_no_windows_se_informa_como_no_soportada() {
        assert!(matches!(move_to_trash(&[]), Err(TrashError::NotSupported)));
        assert!(matches!(
            restore_from_trash(&[]),
            Err(TrashError::NotSupported)
        ));
    }
}
