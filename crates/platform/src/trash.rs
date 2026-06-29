// Naygo — papelera de Windows (Win32 IFileOperation, COM, aislado).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `move_to_trash` envía rutas a la Papelera de reciclaje vía la API COM moderna
//! `IFileOperation`. El borrado permanente NO vive aquí (lo hace `core::ops`).
//! Tolerante: errores se reportan en el `Result`, no tumban el proceso.

use std::path::PathBuf;

/// Error al enviar a papelera.
#[derive(Debug)]
pub enum TrashError {
    /// La papelera no está disponible en esta plataforma.
    NotSupported,
    /// La operación COM/Shell falló; el mensaje describe el HRESULT.
    Failed(String),
}

#[cfg(windows)]
pub fn move_to_trash(paths: &[PathBuf]) -> Result<(), TrashError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOperation, IFileOperation, IShellItem, SHCreateItemFromParsingName,
        FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT,
    };

    if paths.is_empty() {
        return Ok(());
    }

    // SAFETY: toda la secuencia COM se ejecuta dentro de un único bloque unsafe.
    // CoUninitialize SOLO se llama si CoInitializeEx realmente inicializó COM en este
    // hilo (S_OK o S_FALSE). Si el hilo ya estaba en OTRO apartment
    // (RPC_E_CHANGED_MODE), NO debe llamarse CoUninitialize: sería un balance
    // incorrecto que decrementaría el refcount de COM de otro componente del hilo
    // (relevante porque esto puede llamarse desde el hilo de UI ya inicializado por
    // eframe/winit). Los punteros (PCWSTR sobre buffers vivos) son válidos.
    unsafe {
        // En 0.62 devuelve HRESULT. `is_ok()` cubre S_OK y S_FALSE (ya inicializado en
        // este apartment); ambos requieren un CoUninitialize de cierre.
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let needs_uninit = hr.is_ok();

        let result = (|| -> Result<(), TrashError> {
            let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)
                .map_err(|e| TrashError::Failed(e.to_string()))?;

            // Reciclar (no borrar), sin confirmación ni UI.
            op.SetOperationFlags(
                FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOFX_RECYCLEONDELETE,
            )
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

            op.PerformOperations()
                .map_err(|e| TrashError::Failed(e.to_string()))?;
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
pub fn move_to_trash(_paths: &[PathBuf]) -> Result<(), TrashError> {
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
        let res = move_to_trash(std::slice::from_ref(&f));
        assert!(res.is_ok(), "move_to_trash falló: {res:?}");
        assert!(!f.exists(), "el archivo debería haber ido a la papelera");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
