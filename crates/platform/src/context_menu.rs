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
//!    absoluto** (relativo a la raíz del escritorio). Si alguna ruta no resuelve,
//!    se rechaza el lote completo antes de mostrar el menú.
//! 2. `SHBindToParent` — dado un PIDL absoluto, obtiene la carpeta padre como
//!    `IShellFolder` **y** el PIDL *hijo* (relativo a esa carpeta). El PIDL hijo
//!    apunta dentro de la lista del padre: **no se libera**.
//! 3. `IShellFolder::GetUIObjectOf` — pide al padre un `IContextMenu` para el
//!    conjunto de hijos. (Todos los items deben compartir el mismo padre; aquí
//!    comprobamos el PIDL del padre de cada item antes de agruparlos.)
//! 4. `CreatePopupMenu` + `IContextMenu::QueryContextMenu` — el Shell rellena un
//!    `HMENU` con sus verbos (Abrir, Copiar, Propiedades, extensiones de
//!    terceros…), numerados en el rango `[CMD_MIN, CMD_MAX]`.
//! 5. `TrackPopupMenuEx` con `TPM_RETURNCMD` — muestra el menú **modal** en las
//!    coordenadas de pantalla `(x, y)` y devuelve el id elegido (0 = cancelado).
//! 6. `IContextMenu::InvokeCommand` — ejecuta el verbo (id - CMD_MIN).
//!
//! ## Sutileza del apartamento (apartment threading)
//!
//! Esto corre en el **hilo de UI**, que Slint/winit ya inicializó como STA. Por
//! eso replicamos el patrón de `trash.rs`: `CoInitializeEx` con
//! `COINIT_APARTMENTTHREADED` y `CoUninitialize` **solo** si realmente
//! inicializamos COM en este hilo (`hr.is_ok()`). Si el hilo ya estaba en otro
//! apartamento (`RPC_E_CHANGED_MODE`), no desbalanceamos el refcount de COM.
//!
//! ## Tolerancia
//!
//! El filesystem es hostil: rutas que desaparecen, discos de red caídos,
//! permisos. Los errores se propagan sin mostrar un menú sobre una selección parcial.
//! Las pruebas preparan PIDL y obtienen `IContextMenu` sin presentar ni ejecutar comandos.

#[cfg(windows)]
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
    /// No se proporcionaron elementos.
    NoItems,
    /// Algún elemento dejó de estar disponible; nunca operar sobre un subconjunto.
    IncompleteSelection,
    /// El menú de un único IShellFolder no admite elementos de padres diferentes.
    DifferentParents,
    /// La operación COM/Shell falló; el mensaje describe el HRESULT.
    Failed(String),
}

/// PIDL absolutos poseídos durante todo el menú y liberados incluso si falla la preparación.
#[cfg(windows)]
struct ShellPidls(Vec<*mut windows::Win32::UI::Shell::Common::ITEMIDLIST>);

#[cfg(windows)]
impl Drop for ShellPidls {
    fn drop(&mut self) {
        for &pidl in &self.0 {
            unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(pidl.cast())) };
        }
    }
}

#[cfg(windows)]
impl ShellPidls {
    /// Resuelve todas las rutas y contrasta los padres por identidad Shell, sin abrir menús.
    fn resolve(paths: &[PathBuf]) -> Result<Self, ShellError> {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::{
            ILClone, ILFree, ILIsEqual, ILRemoveLastID, SHParseDisplayName,
        };
        if paths.is_empty() {
            return Err(ShellError::NoItems);
        }
        let mut result = Self(Vec::with_capacity(paths.len()));
        for path in paths {
            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let mut pidl = std::ptr::null_mut();
            let parsed =
                unsafe { SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None) };
            if parsed.is_err() || pidl.is_null() {
                if !pidl.is_null() {
                    unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(pidl.cast())) };
                }
                return Err(ShellError::IncompleteSelection);
            }
            result.0.push(pidl);
        }
        // Clonar el PIDL absoluto para quitar su último segmento sin invalidar el hijo.
        let parent_of =
            |pidl| -> Result<*mut windows::Win32::UI::Shell::Common::ITEMIDLIST, ShellError> {
                let parent = unsafe { ILClone(pidl) };
                if parent.is_null() {
                    return Err(ShellError::Failed("ILClone failed".into()));
                }
                if !unsafe { ILRemoveLastID(Some(parent)) }.as_bool() {
                    unsafe { ILFree(Some(parent)) };
                    return Err(ShellError::Failed("ILRemoveLastID failed".into()));
                }
                Ok(parent)
            };
        let first = parent_of(result.0[0])?;
        for &pidl in result.0.iter().skip(1) {
            let parent = match parent_of(pidl) {
                Ok(parent) => parent,
                Err(error) => {
                    unsafe { ILFree(Some(first)) };
                    return Err(error);
                }
            };
            let same = unsafe { ILIsEqual(first, parent) }.as_bool();
            unsafe { ILFree(Some(parent)) };
            if !same {
                unsafe { ILFree(Some(first)) };
                return Err(ShellError::DifferentParents);
            }
        }
        unsafe { ILFree(Some(first)) };
        Ok(result)
    }
}

#[cfg(windows)]
pub fn show_native_context_menu(
    hwnd: isize,
    paths: &[PathBuf],
    x: i32,
    y: i32,
) -> Result<NativeMenuOutcome, ShellError> {
    use std::ffi::c_void;
    use windows::core::PCSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::Shell::{
        IContextMenu, IShellFolder, SHBindToParent, CMF_NORMAL, CMINVOKECOMMANDINFO,
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
    // inicializado por Slint/winit). Mismo patrón que `trash.rs`.
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let needs_uninit = hr.is_ok();

        let result = (|| -> Result<NativeMenuOutcome, ShellError> {
            // Resolver y validar el lote completo ANTES de ofrecer un comando.
            let owned_pidls = ShellPidls::resolve(paths)?;
            let abs_pidls = &owned_pidls.0;

            // 2) Obtener el padre (IShellFolder) y el PIDL hijo de cada item.
            //    Ya se comprobó que todos comparten el mismo padre en el Shell.
            let mut child_last: *mut ITEMIDLIST = std::ptr::null_mut();
            let parent: IShellFolder = match SHBindToParent(abs_pidls[0], Some(&mut child_last)) {
                Ok(p) => p,
                Err(e) => {
                    return Err(ShellError::Failed(e.to_string()));
                }
            };
            if child_last.is_null() {
                return Err(ShellError::IncompleteSelection);
            }

            // El primer hijo viene de SHBindToParent; los demás los obtenemos por
            // SHBindToParent también (cada uno apunta dentro de la lista de SU
            // padre absoluto; no se liberan).
            let mut child_pidls: Vec<*const ITEMIDLIST> = Vec::with_capacity(abs_pidls.len());
            child_pidls.push(child_last as *const ITEMIDLIST);
            for &abs in abs_pidls.iter().skip(1) {
                let mut last: *mut ITEMIDLIST = std::ptr::null_mut();
                // Reutilizamos SHBindToParent por su PIDL hijo; ya validamos el padre.
                match SHBindToParent::<IShellFolder>(abs, Some(&mut last)) {
                    Ok(_) if !last.is_null() => child_pidls.push(last as *const ITEMIDLIST),
                    Ok(_) => return Err(ShellError::IncompleteSelection),
                    Err(e) => {
                        return Err(ShellError::Failed(e.to_string()));
                    }
                }
            }

            // 3) Pedir el IContextMenu para los hijos. El helper genérico de
            //    GetUIObjectOf<T> infiere el IID y toma apidl: &[*const ITEMIDLIST].
            let context_menu: IContextMenu = match parent.GetUIObjectOf(hwnd, &child_pidls, None) {
                Ok(cm) => cm,
                Err(e) => {
                    return Err(ShellError::Failed(e.to_string()));
                }
            };

            // 4) Crear el HMENU y dejar que el Shell lo rellene.
            let hmenu = match CreatePopupMenu() {
                Ok(h) => h,
                Err(e) => {
                    return Err(ShellError::Failed(e.to_string()));
                }
            };

            // QueryContextMenu devuelve un HRESULT directo (no Result).
            if let Err(e) = context_menu
                .QueryContextMenu(hmenu, 0, CMD_MIN, CMD_MAX, CMF_NORMAL)
                .ok()
            {
                let _ = DestroyMenu(hmenu);
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
                        return Err(ShellError::Failed(e.to_string()));
                    }
                }
            } else {
                NativeMenuOutcome::Cancelled
            };

            // 7) Limpieza: destruir el HMENU y liberar SOLO los PIDLs absolutos.
            let _ = DestroyMenu(hmenu);
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
            matches!(r, Err(ShellError::IncompleteSelection))
                || matches!(r, Err(ShellError::Failed(_))),
            "fue {r:?}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn pidl_batch_requires_all_items_and_same_shell_parent() {
        use windows::Win32::System::Com::{
            CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
        };
        let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        assert!(init.is_ok(), "COM STA: {init:?}");
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("uno.txt");
        let b = dir.path().join("dos ñ.txt");
        let nested = dir.path().join("sub");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        std::fs::create_dir(&nested).unwrap();
        let other = nested.join("otro.txt");
        std::fs::write(&other, b"c").unwrap();
        let all = ShellPidls::resolve(&[a.clone(), b.clone()]).unwrap();
        assert_eq!(all.0.len(), 2, "el lote completo se entrega al Shell");
        // Obtener el objeto real de Shell para ambos hijos sin mostrar ni ejecutar el menú.
        unsafe {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::Shell::{IContextMenu, IShellFolder, SHBindToParent};
            let mut first = std::ptr::null_mut();
            let parent: IShellFolder = SHBindToParent(all.0[0], Some(&mut first)).unwrap();
            let mut second = std::ptr::null_mut();
            let _: IShellFolder = SHBindToParent(all.0[1], Some(&mut second)).unwrap();
            assert!(!first.is_null() && !second.is_null());
            let children = [first.cast_const(), second.cast_const()];
            let context: IContextMenu = parent
                .GetUIObjectOf(HWND(std::ptr::null_mut()), &children, None)
                .unwrap();
            drop(context);
        }
        drop(all);
        assert!(matches!(
            ShellPidls::resolve(&[a.clone(), dir.path().join("ausente")]),
            Err(ShellError::IncompleteSelection)
        ));
        assert!(matches!(
            ShellPidls::resolve(&[a, other]),
            Err(ShellError::DifferentParents)
        ));
        unsafe { CoUninitialize() };
    }
}
