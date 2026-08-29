// Naygo — papelera de Windows (Win32 IFileOperation, COM, aislado).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `move_to_trash` envía rutas a la Papelera de reciclaje vía la API COM moderna
//! `IFileOperation`. El borrado permanente NO vive aquí (lo hace `core::ops`).
//! Tolerante: errores se reportan en el `Result`, no tumban el proceso.

use std::path::PathBuf;

/// Identidad de un ítem que el Shell acaba de enviar a Papelera. Al deshacer se
/// busca dentro de Papelera por su ubicación y nombre originales.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashReceipt {
    pub original: PathBuf,
}

/// Avance informado por `IFileOperation`. Las unidades son las que entrega el Shell y pueden
/// incluir trabajo interno de carpetas, por lo que sirven para una barra proporcional.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrashProgress {
    pub work_total: u32,
    pub work_done: u32,
}

pub type CancelProbe = std::sync::Arc<dyn Fn() -> bool + Send + Sync>;

/// Error al enviar a papelera.
#[derive(Debug)]
pub enum TrashError {
    /// La papelera no está disponible en esta plataforma.
    NotSupported,
    /// La operación COM/Shell falló; el mensaje describe el HRESULT.
    Failed(String),
}

#[cfg(windows)]
pub fn move_to_trash(paths: &[PathBuf]) -> Result<Vec<TrashReceipt>, TrashError> {
    let (tx, _rx) = std::sync::mpsc::channel();
    move_to_trash_with_progress(paths, tx, std::sync::Arc::new(|| false))
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
        SHCreateItemFromParsingName, FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION,
        FOF_SILENT,
    };

    if paths.is_empty() {
        return Ok(Vec::new());
    }

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

            // Reciclar (no borrar), sin confirmación ni UI.
            op.SetOperationFlags(
                FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOFX_RECYCLEONDELETE,
            )
            .map_err(|e| TrashError::Failed(e.to_string()))?;

            let sink: IFileOperationProgressSink = ProgressSink {
                progress,
                cancelled,
            }
            .into();
            let cookie = op
                .Advise(&sink)
                .map_err(|e| TrashError::Failed(e.to_string()))?;

            for path in paths {
                let wide: Vec<u16> = path
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)
                    .map_err(|e| TrashError::Failed(e.to_string()))?;
                op.DeleteItem(&item, None)
                    .map_err(|e| TrashError::Failed(e.to_string()))?;
            }

            let performed = op
                .PerformOperations()
                .map_err(|e| TrashError::Failed(e.to_string()));
            let _ = op.Unadvise(cookie);
            performed?;
            let aborted = op
                .GetAnyOperationsAborted()
                .map_err(|e| TrashError::Failed(e.to_string()))?;
            if aborted.as_bool() {
                return Err(TrashError::Failed(
                    "Windows canceló una o más eliminaciones".to_string(),
                ));
            }
            Ok(paths
                .iter()
                .cloned()
                .map(|original| TrashReceipt { original })
                .collect())
        })();

        if needs_uninit {
            CoUninitialize();
        }
        result
    }
}

#[cfg(windows)]
#[windows::core::implement(windows::Win32::UI::Shell::IFileOperationProgressSink)]
struct ProgressSink {
    progress: std::sync::mpsc::Sender<TrashProgress>,
    cancelled: CancelProbe,
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
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
        _: windows_core::HRESULT,
        _: windows_core::Ref<windows::Win32::UI::Shell::IShellItem>,
    ) -> windows_core::Result<()> {
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

/// Restaura ítems identificados durante `move_to_trash_with_progress` a sus
/// ubicaciones originales. Corre en un worker de Naygo; no toca el hilo UI.
#[cfg(windows)]
pub fn restore_from_trash(receipts: &[TrashReceipt]) -> Result<(), TrashError> {
    use windows::core::{Interface, GUID, PCSTR};
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{
        BHID_EnumItems, BHID_SFUIObject, FOLDERID_RecycleBinFolder, IContextMenu, IEnumShellItems,
        IShellItem, IShellItem2, SHGetKnownFolderItem, CMF_NORMAL, CMINVOKECOMMANDINFO,
        KF_FLAG_DEFAULT, SIGDN_NORMALDISPLAY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{CreatePopupMenu, DestroyMenu};

    if receipts.is_empty() {
        return Ok(());
    }
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let needs_uninit = hr.is_ok();
        let result = (|| -> Result<(), TrashError> {
            let recycle_bin: IShellItem =
                SHGetKnownFolderItem(&FOLDERID_RecycleBinFolder, KF_FLAG_DEFAULT, None)
                    .map_err(|e| TrashError::Failed(e.to_string()))?;
            let enumerator: IEnumShellItems = recycle_bin
                .BindToHandler(None, &BHID_EnumItems)
                .map_err(|e| TrashError::Failed(e.to_string()))?;
            let mut candidates = Vec::<IShellItem>::new();
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
            const DELETED_FROM: PROPERTYKEY = PROPERTYKEY {
                fmtid: GUID::from_u128(0x9b174b33_40ff_11d2_a27e_00c04fc30871),
                pid: 2,
            };
            for receipt in receipts {
                if receipt.original.exists() {
                    return Err(TrashError::Failed(format!(
                        "el destino está ocupado: {}",
                        receipt.original.display()
                    )));
                }
                let original_parent = receipt.original.parent().ok_or_else(|| {
                    TrashError::Failed(format!(
                        "ruta sin carpeta padre: {}",
                        receipt.original.display()
                    ))
                })?;
                let original_name = receipt.original.file_name().unwrap_or_default();
                let Some(position) = candidates.iter().position(|item| {
                    let deleted_from = item
                        .cast::<IShellItem2>()
                        .ok()
                        .and_then(|item2| item2.GetString(&DELETED_FROM).ok())
                        .and_then(|value| value.to_string().ok());
                    let name = item
                        .GetDisplayName(SIGDN_NORMALDISPLAY)
                        .ok()
                        .and_then(|value| value.to_string().ok());
                    deleted_from
                        .as_deref()
                        .is_some_and(|folder| std::path::Path::new(folder) == original_parent)
                        && name
                            .as_deref()
                            .and_then(|name| std::path::Path::new(name).file_name())
                            .is_some_and(|name| name == original_name)
                }) else {
                    return Err(TrashError::Failed(format!(
                        "el elemento ya no está en Papelera: {}",
                        receipt.original.display()
                    )));
                };
                // No reutilizar el mismo objeto lógico si una operación contiene
                // nombres repetidos en rutas distintas o una restauración múltiple.
                let item = candidates.remove(position);
                // `MoveItem` trata el contenido físico $Rxxxx de Papelera como un
                // archivo normal y lo deja con ese nombre. El verbo Shell
                // `undelete` conserva el nombre y la ubicación originales, igual
                // que el botón Restaurar del Explorador.
                let menu: IContextMenu = item
                    .BindToHandler(None, &BHID_SFUIObject)
                    .map_err(|e| TrashError::Failed(e.to_string()))?;
                let hmenu = CreatePopupMenu().map_err(|e| TrashError::Failed(e.to_string()))?;
                let queried = menu.QueryContextMenu(hmenu, 0, 1, 0x7fff, CMF_NORMAL).ok();
                let _ = DestroyMenu(hmenu);
                queried.map_err(|e| TrashError::Failed(e.to_string()))?;
                const UNDELETE: &[u8] = b"undelete\0";
                let command = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    lpVerb: PCSTR(UNDELETE.as_ptr()),
                    ..Default::default()
                };
                menu.InvokeCommand(&command)
                    .map_err(|e| TrashError::Failed(e.to_string()))?;
            }
            Ok(())
        })();
        if needs_uninit {
            CoUninitialize();
        }
        result
    }
}

/// Stub no-Windows: la papelera no está disponible.
#[cfg(not(windows))]
pub fn move_to_trash(_paths: &[PathBuf]) -> Result<Vec<TrashReceipt>, TrashError> {
    Err(TrashError::NotSupported)
}

#[cfg(not(windows))]
pub fn move_to_trash_with_progress(
    _paths: &[PathBuf],
    _progress: std::sync::mpsc::Sender<TrashProgress>,
    _cancelled: CancelProbe,
) -> Result<Vec<TrashReceipt>, TrashError> {
    Err(TrashError::NotSupported)
}

#[cfg(not(windows))]
pub fn restore_from_trash(_receipts: &[TrashReceipt]) -> Result<(), TrashError> {
    Err(TrashError::NotSupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn enviar_un_archivo_a_papelera() {
        let dir = std::env::temp_dir().join(format!("naygo_trash_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("borrame.txt");
        std::fs::write(&f, b"x").unwrap();
        assert!(f.exists());
        let receipts = move_to_trash(std::slice::from_ref(&f)).expect("move_to_trash falló");
        assert_eq!(receipts.len(), 1, "debe capturar el ítem de Papelera");
        assert!(!f.exists(), "el archivo debería haber ido a la papelera");
        restore_from_trash(&receipts).expect("restore_from_trash falló");
        assert!(f.exists(), "el archivo debe volver a su ruta original");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
