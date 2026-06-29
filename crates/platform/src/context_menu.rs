// Naygo — menú contextual nativo de Windows (Win32 IContextMenu, COM, aislado).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `show_native_context_menu` construye y muestra el **menú contextual nativo del
//! Shell de Windows** (el mismo que aparece al hacer clic derecho en el
//! Explorador) sobre un conjunto de rutas, y ejecuta el comando elegido.
//!
//! ## La cadena COM
//!
//! El Shell de Windows no expone el menú contextual a partir de rutas de texto,
//! sino a partir de **PIDLs** (Pointer to an ID List): identificadores binarios
//! opacos del namespace del Shell. La secuencia completa es:
//!
//! 1. `SHParseDisplayName` — convierte cada ruta (`C:\...`) en un **PIDL
//!    absoluto** (relativo a la raíz del escritorio). Si la ruta no existe o no
//!    es parte del namespace, falla; esas rutas se descartan.
//! 2. `SHBindToParent` — dado un PIDL absoluto, obtiene la carpeta padre como
//!    `IShellFolder` **y** el PIDL *hijo* (relativo a esa carpeta). El PIDL hijo
//!    apunta dentro de la lista del padre: **no se libera**.
//! 3. `IShellFolder::GetUIObjectOf` — pide al padre un `IContextMenu` para el
//!    conjunto de hijos. (Todos los items deben compartir el mismo padre; aquí
//!    usamos el padre del primer item y agrupamos sus hijos.)
//! 4. `CreatePopupMenu` + `IContextMenu::QueryContextMenu` — el Shell rellena un
//!    `HMENU` con sus verbos (Abrir, Copiar, Propiedades, extensiones de
//!    terceros…), numerados en el rango `[CMD_MIN, CMD_MAX]`.
//! 5. `TrackPopupMenuEx` con `TPM_RETURNCMD` — muestra el menú **modal** en las
//!    coordenadas de pantalla `(x, y)` y devuelve el id elegido (0 = cancelado).
//! 6. `IContextMenu::InvokeCommand` — ejecuta el verbo (id - CMD_MIN).
//!
//! ## Sutileza del apartamento (apartment threading)
//!
//! Esto corre en el **hilo de UI**, que eframe/winit ya inicializó como STA. Por
//! eso replicamos el patrón de `trash.rs`: `CoInitializeEx` con
//! `COINIT_APARTMENTTHREADED` y `CoUninitialize` **solo** si realmente
//! inicializamos COM en este hilo (`hr.is_ok()`). Si el hilo ya estaba en otro
//! apartamento (`RPC_E_CHANGED_MODE`), no desbalanceamos el refcount de COM.
//!
//! ## Tolerancia
//!
//! El filesystem es hostil: rutas que desaparecen, discos de red caídos,
//! permisos. Toda falla COM se mapea a `ShellError::Failed`; nunca hace panic. Si
//! ninguna ruta resuelve a un PIDL, devuelve `NoItems` **antes** de mostrar
//! cualquier menú (esto hace los tests no interactivos).

use std::path::PathBuf;

/// Resultado de mostrar el menú: el usuario eligió un comando o canceló.
#[derive(Debug, PartialEq, Eq)]
pub enum NativeMenuOutcome {
    /// El usuario eligió un verbo y se invocó.
    Invoked,
    /// El usuario cerró el menú sin elegir (Esc / clic fuera).
    Cancelled,
}

/// Error al construir o mostrar el menú contextual nativo.
#[derive(Debug)]
pub enum ShellError {
    /// El menú nativo no está disponible en esta plataforma.
    NotSupported,
    /// No hay items válidos para los que mostrar un menú (lista vacía o ninguna
    /// ruta resolvió a un PIDL del Shell).
    NoItems,
    /// La operación COM/Shell falló; el mensaje describe el HRESULT.
    Failed(String),
}

#[cfg(windows)]
pub fn show_native_context_menu(
    hwnd: isize,
    paths: &[PathBuf],
    x: i32,
    y: i32,
) -> Result<NativeMenuOutcome, ShellError> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCSTR, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        IContextMenu, IShellFolder, SHBindToParent, SHParseDisplayName, CMF_NORMAL,
        CMINVOKECOMMANDINFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreatePopupMenu, DestroyMenu, TrackPopupMenuEx, SW_SHOWNORMAL, TPM_RETURNCMD,
        TPM_RIGHTBUTTON,
    };

    if paths.is_empty() {
        return Err(ShellError::NoItems);
    }

    // Rango de ids que el Shell puede asignar a sus verbos en el HMENU.
    const CMD_MIN: u32 = 1;
    const CMD_MAX: u32 = 0x7FFF;

    let hwnd = HWND(hwnd as *mut c_void);

    // SAFETY: toda la secuencia COM se ejecuta dentro de un único bloque unsafe.
    // CoUninitialize SOLO se llama si CoInitializeEx inicializó COM en este hilo;
    // si el hilo ya estaba en otro apartamento (RPC_E_CHANGED_MODE) NO se llama
    // para no desbalancear el refcount (esto puede correr en el hilo de UI ya
    // inicializado por eframe). Mismo patrón que `trash.rs`.
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let needs_uninit = hr.is_ok();

        let result = (|| -> Result<NativeMenuOutcome, ShellError> {
            // 1) Resolver cada ruta a un PIDL absoluto. Las que fallan se
            //    descartan. Guardamos los PIDLs absolutos para liberarlos al
            //    final (son propiedad nuestra: vienen de SHParseDisplayName).
            let mut abs_pidls: Vec<*mut ITEMIDLIST> = Vec::with_capacity(paths.len());
            for path in paths {
                let wide: Vec<u16> = path
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
                // SHParseDisplayName(pszname, pbc, ppidl, sfgaoin, psfgaoout).
                if SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None).is_ok()
                    && !pidl.is_null()
                {
                    abs_pidls.push(pidl);
                }
            }

            // Si ninguna ruta resolvió: NoItems, ANTES de mostrar menú alguno.
            // Esto es lo que hace que los tests sean no interactivos.
            if abs_pidls.is_empty() {
                return Err(ShellError::NoItems);
            }

            // Helper que libera todos los PIDLs absolutos. Los PIDLs hijos
            // (de SHBindToParent) NO se liberan: son internos a la lista del
            // padre. Los interfaces COM se sueltan solos al salir de scope.
            let free_abs = |pidls: &[*mut ITEMIDLIST]| {
                for &p in pidls {
                    CoTaskMemFree(Some(p as *const c_void));
                }
            };

            // 2) Obtener el padre (IShellFolder) y el PIDL hijo de cada item.
            //    Todos los items deben compartir padre para un solo IContextMenu;
            //    usamos el padre del primer item y recolectamos los hijos.
            let mut child_last: *mut ITEMIDLIST = std::ptr::null_mut();
            let parent: IShellFolder = match SHBindToParent(abs_pidls[0], Some(&mut child_last)) {
                Ok(p) => p,
                Err(e) => {
                    free_abs(&abs_pidls);
                    return Err(ShellError::Failed(e.to_string()));
                }
            };

            // El primer hijo viene de SHBindToParent; los demás los obtenemos por
            // SHBindToParent también (cada uno apunta dentro de la lista de SU
            // padre absoluto; no se liberan).
            let mut child_pidls: Vec<*const ITEMIDLIST> = Vec::with_capacity(abs_pidls.len());
            child_pidls.push(child_last as *const ITEMIDLIST);
            for &abs in abs_pidls.iter().skip(1) {
                let mut last: *mut ITEMIDLIST = std::ptr::null_mut();
                // Reutilizamos SHBindToParent solo por su PIDL hijo; descartamos
                // el IShellFolder devuelto (asumimos mismo padre que el primero).
                match SHBindToParent::<IShellFolder>(abs, Some(&mut last)) {
                    Ok(_) if !last.is_null() => child_pidls.push(last as *const ITEMIDLIST),
                    Ok(_) => {}
                    Err(e) => {
                        free_abs(&abs_pidls);
                        return Err(ShellError::Failed(e.to_string()));
                    }
                }
            }

            // 3) Pedir el IContextMenu para los hijos. El helper genérico de
            //    GetUIObjectOf<T> infiere el IID y toma apidl: &[*const ITEMIDLIST].
            let context_menu: IContextMenu = match parent.GetUIObjectOf(hwnd, &child_pidls, None) {
                Ok(cm) => cm,
                Err(e) => {
                    free_abs(&abs_pidls);
                    return Err(ShellError::Failed(e.to_string()));
                }
            };

            // 4) Crear el HMENU y dejar que el Shell lo rellene.
            let hmenu = match CreatePopupMenu() {
                Ok(h) => h,
                Err(e) => {
                    free_abs(&abs_pidls);
                    return Err(ShellError::Failed(e.to_string()));
                }
            };

            // QueryContextMenu devuelve un HRESULT directo (no Result).
            if let Err(e) = context_menu
                .QueryContextMenu(hmenu, 0, CMD_MIN, CMD_MAX, CMF_NORMAL)
                .ok()
            {
                let _ = DestroyMenu(hmenu);
                free_abs(&abs_pidls);
                return Err(ShellError::Failed(e.to_string()));
            }

            // 5) Mostrar el menú modal. Con TPM_RETURNCMD devuelve el id elegido
            //    (0 = cancelado) en lugar de postear un WM_COMMAND. x,y son
            //    coordenadas de PANTALLA provistas por el llamador.
            let flags = (TPM_RETURNCMD | TPM_RIGHTBUTTON).0;
            let chosen = TrackPopupMenuEx(hmenu, flags, x, y, hwnd, None).0 as u32;

            let outcome = if chosen != 0 {
                // 6) Invocar el verbo elegido. lpVerb es un MAKEINTRESOURCEA del
                //    índice (id - CMD_MIN) codificado como puntero.
                let info = CMINVOKECOMMANDINFO {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                    hwnd,
                    lpVerb: PCSTR((chosen - CMD_MIN) as usize as *const u8),
                    nShow: SW_SHOWNORMAL.0,
                    ..Default::default()
                };
                match context_menu.InvokeCommand(&info as *const CMINVOKECOMMANDINFO) {
                    Ok(()) => NativeMenuOutcome::Invoked,
                    Err(e) => {
                        let _ = DestroyMenu(hmenu);
                        free_abs(&abs_pidls);
                        return Err(ShellError::Failed(e.to_string()));
                    }
                }
            } else {
                NativeMenuOutcome::Cancelled
            };

            // 7) Limpieza: destruir el HMENU y liberar SOLO los PIDLs absolutos.
            let _ = DestroyMenu(hmenu);
            free_abs(&abs_pidls);
            Ok(outcome)
        })();

        if needs_uninit {
            CoUninitialize();
        }
        result
    }
}

/// Stub no-Windows: el menú contextual nativo no está disponible.
#[cfg(not(windows))]
pub fn show_native_context_menu(
    _hwnd: isize,
    _paths: &[std::path::PathBuf],
    _x: i32,
    _y: i32,
) -> Result<NativeMenuOutcome, ShellError> {
    Err(ShellError::NotSupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[cfg(not(windows))]
    #[test]
    fn no_windows_es_notsupported() {
        assert!(matches!(
            show_native_context_menu(0, &[PathBuf::from("x")], 0, 0),
            Err(ShellError::NotSupported)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn lista_vacia_es_noitems() {
        assert!(matches!(
            show_native_context_menu(0, &[], 0, 0),
            Err(ShellError::NoItems)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn rutas_inexistentes_no_panic() {
        let r =
            show_native_context_menu(0, &[PathBuf::from("Z:\\no\\existe\\naygo-xyz-12345")], 0, 0);
        assert!(
            matches!(r, Err(ShellError::NoItems)) || matches!(r, Err(ShellError::Failed(_))),
            "fue {r:?}"
        );
    }
}
