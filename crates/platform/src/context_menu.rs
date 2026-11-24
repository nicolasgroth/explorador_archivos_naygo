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
//! ## Submenús diferidos y owner-draw (`IContextMenu2` / `IContextMenu3`)
//!
//! Varios submenús del Shell ("Enviar a", "Abrir con") y muchas
//! extensiones de terceros (7-Zip, Git…) no rellenan su submenú en
//! `QueryContextMenu`: lo hacen al recibir `WM_INITMENUPOPUP`, y dibujan sus
//! ítems (íconos) con `WM_MEASUREITEM` / `WM_DRAWITEM`. Esos mensajes llegan a la
//! ventana dueña del menú (el HWND de winit), no al Shell. Por eso, mientras
//! `TrackPopupMenuEx` está activo, se instala una **subclase temporal**
//! (`SetWindowSubclass`) que reenvía esos mensajes a `IContextMenu3::HandleMenuMsg2`
//! (o `IContextMenu2::HandleMenuMsg`). Un guard RAII la quita siempre al terminar.
//!
//! "Nuevo" (`CNewMenu`) NO se arregla con este reenvío: ese verbo solo existe en el menú de
//! FONDO de una carpeta (`CreateViewObject(IID_IContextMenu)`), que Naygo todavía no pide.
//! Queda como tarea aparte.
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

// Mensajes de menú que el Shell necesita recibir (valores de winuser.h; un test los
// contrasta con las constantes del crate `windows`).
#[cfg(any(windows, test))]
const WM_DRAWITEM: u32 = 0x002B;
#[cfg(any(windows, test))]
const WM_MEASUREITEM: u32 = 0x002C;
#[cfg(any(windows, test))]
const WM_INITMENUPOPUP: u32 = 0x0117;
#[cfg(any(windows, test))]
const WM_MENUCHAR: u32 = 0x0120;
#[cfg(any(windows, test))]
/// `ODT_MENU`: el ítem owner-draw pertenece a un menú (no a un control).
const ODT_MENU: u32 = 1;

#[cfg(any(windows, test))]
/// Qué versión de la interfaz de mensajes de menú ofrece el `IContextMenu` del Shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuMsgInterface {
    /// `IContextMenu3`: `HandleMenuMsg2`, con `LRESULT` (necesario para `WM_MENUCHAR`).
    V3,
    /// Solo `IContextMenu2`: `HandleMenuMsg`, sin `LRESULT`.
    V2,
}

#[cfg(any(windows, test))]
/// Decisión de la subclase para un mensaje recibido durante el menú.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuMsgRoute {
    /// Reenviar al Shell con `HandleMenuMsg2` y devolver su `LRESULT`.
    HandleMsg2,
    /// Reenviar al Shell con `HandleMenuMsg` y devolver 0.
    HandleMsg,
    /// No es asunto del Shell: `DefSubclassProc`.
    Default,
}

/// Valor que devuelve la subclase tras un reenvío exitoso al Shell. `shell_result` es el
/// `plResult` que escribió `HandleMenuMsg2` (`None` con `IContextMenu2`, que no tiene `LRESULT`).
///
/// `WM_DRAWITEM` / `WM_MEASUREITEM` se contestan con `TRUE` (1, la convención Win32 de "mensaje
/// procesado") cuando el Shell no dejó un valor propio. `WM_MENUCHAR` conserva lo que entrega el
/// Shell (empaqueta la acción y el ítem), y `WM_INITMENUPOPUP` devuelve 0 ("procesado").
#[cfg(any(windows, test))]
fn forwarded_result(msg: u32, shell_result: Option<isize>) -> isize {
    match msg {
        WM_DRAWITEM | WM_MEASUREITEM => match shell_result {
            Some(value) if value != 0 => value,
            _ => 1,
        },
        _ => shell_result.unwrap_or(0),
    }
}

#[cfg(any(windows, test))]
/// Decide si un mensaje se reenvía al Shell. Pura y sin Win32, para poder probarla.
///
/// `ctl_type` es el primer campo (`CtlType`) de `DRAWITEMSTRUCT`/`MEASUREITEMSTRUCT`
/// apuntado por `lParam` (`None` si `lParam` es nulo); solo importa en
/// `WM_DRAWITEM`/`WM_MEASUREITEM`, que se reenvían únicamente para ítems de menú
/// (`wParam == 0` y `CtlType == ODT_MENU`).
fn route_menu_msg(
    iface: MenuMsgInterface,
    msg: u32,
    wparam: usize,
    ctl_type: Option<u32>,
) -> MenuMsgRoute {
    let forward = match iface {
        MenuMsgInterface::V3 => MenuMsgRoute::HandleMsg2,
        MenuMsgInterface::V2 => MenuMsgRoute::HandleMsg,
    };
    match msg {
        WM_INITMENUPOPUP => forward,
        WM_DRAWITEM | WM_MEASUREITEM if wparam == 0 && ctl_type == Some(ODT_MENU) => forward,
        // Sin LRESULT, IContextMenu2 no puede responder WM_MENUCHAR.
        WM_MENUCHAR if iface == MenuMsgInterface::V3 => forward,
        _ => MenuMsgRoute::Default,
    }
}

/// Interfaz de mensajes de menú que se entrega a la subclase por `dwRefData`.
#[cfg(windows)]
enum MenuMsgHandler {
    V3(windows::Win32::UI::Shell::IContextMenu3),
    V2(windows::Win32::UI::Shell::IContextMenu2),
}

#[cfg(windows)]
impl MenuMsgHandler {
    /// Prefiere `IContextMenu3`; si no, `IContextMenu2`; si no hay ninguno, `None`.
    fn from_menu(menu: &windows::Win32::UI::Shell::IContextMenu) -> Option<Self> {
        use windows::core::Interface;
        if let Ok(v3) = menu.cast::<windows::Win32::UI::Shell::IContextMenu3>() {
            return Some(Self::V3(v3));
        }
        menu.cast::<windows::Win32::UI::Shell::IContextMenu2>()
            .ok()
            .map(Self::V2)
    }

    fn interface(&self) -> MenuMsgInterface {
        match self {
            Self::V3(_) => MenuMsgInterface::V3,
            Self::V2(_) => MenuMsgInterface::V2,
        }
    }
}

/// Funciones de subclase de comctl32, resueltas **por ordinal** en tiempo de ejecución.
///
/// Naygo no declara comctl32 v6 en su manifest, así que el proceso carga comctl32
/// v5.8, que exporta `SetWindowSubclass` & cía. **solo por ordinal** (410-413). Un
/// import estático por nombre (el que genera el crate `windows`) impide cargar el
/// `.exe` (`STATUS_ENTRYPOINT_NOT_FOUND`). Los ordinales son estables y valen en v5 y v6.
#[cfg(windows)]
type SetWindowSubclassFn = unsafe extern "system" fn(
    windows::Win32::Foundation::HWND,
    windows::Win32::UI::Shell::SUBCLASSPROC,
    usize,
    usize,
) -> windows::core::BOOL;
#[cfg(windows)]
type RemoveWindowSubclassFn = unsafe extern "system" fn(
    windows::Win32::Foundation::HWND,
    windows::Win32::UI::Shell::SUBCLASSPROC,
    usize,
) -> windows::core::BOOL;
#[cfg(windows)]
type DefSubclassProcFn = unsafe extern "system" fn(
    windows::Win32::Foundation::HWND,
    u32,
    windows::Win32::Foundation::WPARAM,
    windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT;
#[cfg(all(windows, test))]
type GetWindowSubclassFn = unsafe extern "system" fn(
    windows::Win32::Foundation::HWND,
    windows::Win32::UI::Shell::SUBCLASSPROC,
    usize,
    *mut usize,
) -> windows::core::BOOL;
/// Firma genérica que devuelve `GetProcAddress`.
#[cfg(windows)]
type RawProc = unsafe extern "system" fn() -> isize;

#[cfg(windows)]
struct Comctl32Subclass {
    set: SetWindowSubclassFn,
    remove: RemoveWindowSubclassFn,
    def: DefSubclassProcFn,
    #[cfg(test)]
    get: GetWindowSubclassFn,
}

#[cfg(windows)]
impl Comctl32Subclass {
    /// Tabla resuelta una vez por proceso; `None` si comctl32 no la ofrece (menú sin reenvío).
    fn get() -> Option<&'static Self> {
        static TABLE: std::sync::OnceLock<Option<Comctl32Subclass>> = std::sync::OnceLock::new();
        TABLE.get_or_init(Self::resolve).as_ref()
    }

    fn resolve() -> Option<Self> {
        use windows::core::{w, PCSTR};
        use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
        // SAFETY: comctl32 es un DLL del sistema; no se libera nunca, así que los
        // punteros a función siguen válidos durante toda la vida del proceso. Cada
        // ordinal se transmuta a la firma documentada de su función (commctrl.h).
        unsafe {
            let module = LoadLibraryW(w!("comctl32.dll")).ok()?;
            let by_ordinal = |ordinal: usize| GetProcAddress(module, PCSTR(ordinal as *const u8));
            let set = by_ordinal(410)?;
            #[cfg(test)]
            let get = by_ordinal(411)?;
            let remove = by_ordinal(412)?;
            let def = by_ordinal(413)?;
            Some(Self {
                set: std::mem::transmute::<RawProc, SetWindowSubclassFn>(set),
                remove: std::mem::transmute::<RawProc, RemoveWindowSubclassFn>(remove),
                def: std::mem::transmute::<RawProc, DefSubclassProcFn>(def),
                #[cfg(test)]
                get: std::mem::transmute::<RawProc, GetWindowSubclassFn>(get),
            })
        }
    }
}

/// Procedimiento de la subclase temporal. `dwrefdata` es un `*const MenuMsgHandler`
/// válido mientras la subclase está instalada (ver `MenuSubclass`).
#[cfg(windows)]
unsafe extern "system" fn menu_subclass_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
    _id: usize,
    dwrefdata: usize,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    // Ningún panic puede cruzar la frontera FFI: si algo falla, comportamiento por defecto.
    let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Option<LRESULT> {
        if dwrefdata == 0 {
            return None;
        }
        // SAFETY: `MenuSubclass` garantiza que el puntero vive más que la subclase.
        let handler = unsafe { &*(dwrefdata as *const MenuMsgHandler) };
        let ctl_type = if matches!(msg, WM_DRAWITEM | WM_MEASUREITEM) && lparam.0 != 0 {
            // SAFETY: con WM_DRAWITEM/WM_MEASUREITEM, lParam apunta a DRAWITEMSTRUCT o
            // MEASUREITEMSTRUCT; ambos empiezan con `UINT CtlType`.
            Some(unsafe { *(lparam.0 as *const u32) })
        } else {
            None
        };
        match (
            route_menu_msg(handler.interface(), msg, wparam.0, ctl_type),
            handler,
        ) {
            (MenuMsgRoute::HandleMsg2, MenuMsgHandler::V3(menu)) => {
                let mut result = LRESULT(0);
                unsafe { menu.HandleMenuMsg2(msg, wparam, lparam, Some(&mut result)) }
                    .ok()
                    .map(|()| LRESULT(forwarded_result(msg, Some(result.0))))
            }
            (MenuMsgRoute::HandleMsg, MenuMsgHandler::V2(menu)) => {
                unsafe { menu.HandleMenuMsg(msg, wparam, lparam) }
                    .ok()
                    .map(|()| LRESULT(forwarded_result(msg, None)))
            }
            _ => None,
        }
    }));
    // Nota: `WM_INITMENUPOPUP` se reenvía SIEMPRE al Shell y no pasa por `DefSubclassProc` (el
    // Explorador de Windows hace lo mismo): el menú pertenece al Shell, que rellena sus
    // submenús diferidos aquí, y winit no usa menús propios en esta ventana, así que no hay un
    // procedimiento por defecto que deba ver este mensaje.
    match handled {
        Ok(Some(result)) => result,
        // La subclase solo se instala si la tabla se resolvió completa.
        _ => match Comctl32Subclass::get() {
            Some(comctl) => unsafe { (comctl.def)(hwnd, msg, wparam, lparam) },
            None => LRESULT(0),
        },
    }
}

/// Subclase temporal del HWND dueño del menú; se quita en `Drop` (todo camino, incluso unwind).
///
/// El préstamo `'a` del `MenuMsgHandler` obliga al compilador a que el handler viva
/// más que la subclase: `dwRefData` nunca queda colgando mientras esté instalada.
#[cfg(windows)]
struct MenuSubclass<'a> {
    hwnd: windows::Win32::Foundation::HWND,
    id: usize,
    _handler: std::marker::PhantomData<&'a MenuMsgHandler>,
}

#[cfg(windows)]
impl<'a> MenuSubclass<'a> {
    /// Instala la subclase; `None` si Windows la rechaza (el menú sigue, sin reenvío).
    fn install(
        hwnd: windows::Win32::Foundation::HWND,
        handler: &'a MenuMsgHandler,
    ) -> Option<Self> {
        let comctl = Comctl32Subclass::get()?;
        let data = handler as *const MenuMsgHandler as usize;
        // El id es la dirección del handler: único por invocación, aun si hubiera
        // menús anidados sobre el mismo HWND.
        let id = data;
        // SAFETY: `menu_subclass_proc` es `extern "system"` con la firma SUBCLASSPROC;
        // `data` apunta a un handler que vive `'a`, más que este guard.
        let ok = unsafe { (comctl.set)(hwnd, Some(menu_subclass_proc), id, data) }.as_bool();
        ok.then_some(Self {
            hwnd,
            id,
            _handler: std::marker::PhantomData,
        })
    }
}

#[cfg(windows)]
impl Drop for MenuSubclass<'_> {
    fn drop(&mut self) {
        // `install` solo construye el guard si la tabla existe.
        if let Some(comctl) = Comctl32Subclass::get() {
            // SAFETY: mismo hwnd, procedimiento e id con que se instaló; si el HWND ya
            // no existe, la llamada solo devuelve FALSE.
            let _ = unsafe { (comctl.remove)(self.hwnd, Some(menu_subclass_proc), self.id) };
        }
    }
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
            //    Mientras está abierto, una subclase temporal reenvía al Shell los
            //    mensajes de submenús diferidos y owner-draw (ver doc del módulo).
            //
            //    SAFETY (subclase): `menu_handler` se declara ANTES del guard y el
            //    guard toma un préstamo `&menu_handler`, así que el handler (y su
            //    referencia COM) vive más que la subclase; el bloque interno hace que
            //    el guard se destruya (RemoveWindowSubclass) al volver
            //    TrackPopupMenuEx, también si hubiera unwind. `TrackPopupMenuEx` es
            //    modal y corre en este mismo hilo (dueño del HWND), así que la
            //    subclase solo se invoca aquí dentro, nunca desde otro hilo.
            let menu_handler = MenuMsgHandler::from_menu(&context_menu);
            let flags = (TPM_RETURNCMD | TPM_RIGHTBUTTON).0;
            let chosen = {
                let _subclass = menu_handler
                    .as_ref()
                    .and_then(|handler| MenuSubclass::install(hwnd, handler));
                TrackPopupMenuEx(hmenu, flags, x, y, hwnd, None).0 as u32
            };

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

    #[test]
    fn route_menu_msg_reenvia_solo_mensajes_de_menu() {
        use MenuMsgInterface::{V2, V3};
        use MenuMsgRoute::{Default, HandleMsg, HandleMsg2};
        // WM_INITMENUPOPUP: siempre al Shell, con la API de la interfaz disponible.
        assert_eq!(
            route_menu_msg(V3, WM_INITMENUPOPUP, 0x1234, None),
            HandleMsg2
        );
        assert_eq!(
            route_menu_msg(V2, WM_INITMENUPOPUP, 0x1234, None),
            HandleMsg
        );
        // Owner-draw de menú (wParam 0 + ODT_MENU): al Shell.
        for msg in [WM_DRAWITEM, WM_MEASUREITEM] {
            assert_eq!(route_menu_msg(V3, msg, 0, Some(ODT_MENU)), HandleMsg2);
            assert_eq!(route_menu_msg(V2, msg, 0, Some(ODT_MENU)), HandleMsg);
            // Owner-draw de un control (wParam = id del control) o de otro tipo: no.
            assert_eq!(route_menu_msg(V3, msg, 7, Some(ODT_MENU)), Default);
            assert_eq!(route_menu_msg(V3, msg, 0, Some(4)), Default);
            assert_eq!(route_menu_msg(V2, msg, 0, None), Default);
        }
        // WM_MENUCHAR requiere LRESULT: solo IContextMenu3.
        assert_eq!(route_menu_msg(V3, WM_MENUCHAR, 0, None), HandleMsg2);
        assert_eq!(route_menu_msg(V2, WM_MENUCHAR, 0, None), Default);
        // Cualquier otro mensaje: DefSubclassProc.
        for msg in [0x0000, 0x0001, 0x000F, 0x0111, 0x011F, 0x0200] {
            assert_eq!(route_menu_msg(V3, msg, 0, Some(ODT_MENU)), Default);
            assert_eq!(route_menu_msg(V2, msg, 0, Some(ODT_MENU)), Default);
        }
    }

    #[test]
    fn forwarded_result_sigue_la_convencion_win32() {
        // Owner-draw procesado: TRUE (1) tanto con IContextMenu2 (sin LRESULT) como con
        // IContextMenu3 que no escribió plResult (0).
        for msg in [WM_DRAWITEM, WM_MEASUREITEM] {
            assert_eq!(forwarded_result(msg, None), 1);
            assert_eq!(forwarded_result(msg, Some(0)), 1);
            // Si el Shell dejó un valor propio, se respeta.
            assert_eq!(forwarded_result(msg, Some(7)), 7);
        }
        // WM_MENUCHAR conserva lo que entrega el Shell (incluido el 0).
        assert_eq!(
            forwarded_result(WM_MENUCHAR, Some(0x0002_0005)),
            0x0002_0005
        );
        assert_eq!(forwarded_result(WM_MENUCHAR, Some(0)), 0);
        // WM_INITMENUPOPUP: 0 ("procesado").
        assert_eq!(forwarded_result(WM_INITMENUPOPUP, None), 0);
        assert_eq!(forwarded_result(WM_INITMENUPOPUP, Some(0)), 0);
    }

    #[cfg(windows)]
    #[test]
    fn constantes_de_mensajes_coinciden_con_win32() {
        use windows::Win32::UI::WindowsAndMessaging as wm;
        assert_eq!(WM_DRAWITEM, wm::WM_DRAWITEM);
        assert_eq!(WM_MEASUREITEM, wm::WM_MEASUREITEM);
        assert_eq!(WM_INITMENUPOPUP, wm::WM_INITMENUPOPUP);
        assert_eq!(WM_MENUCHAR, wm::WM_MENUCHAR);
    }

    /// El menú real de un archivo expone IContextMenu2/3 y la subclase se instala y quita
    /// sobre una ventana real sin dejar rastro (sin mostrar el menú).
    #[cfg(windows)]
    #[test]
    fn handler_y_subclase_sobre_ventana_real() {
        use windows::core::w;
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::System::Com::{
            CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::{IContextMenu, IShellFolder, SHBindToParent};
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SendMessageW, WINDOW_EX_STYLE, WS_OVERLAPPED,
        };
        let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        assert!(init.is_ok(), "COM STA: {init:?}");
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("menu.txt");
        std::fs::write(&file, b"x").unwrap();
        let pidls = ShellPidls::resolve(std::slice::from_ref(&file)).unwrap();
        unsafe {
            let mut child = std::ptr::null_mut();
            let parent: IShellFolder = SHBindToParent(pidls.0[0], Some(&mut child)).unwrap();
            let menu: IContextMenu = parent
                .GetUIObjectOf(
                    windows::Win32::Foundation::HWND(std::ptr::null_mut()),
                    &[child.cast_const()],
                    None,
                )
                .unwrap();
            let handler = MenuMsgHandler::from_menu(&menu)
                .expect("el menú de archivos del Shell implementa IContextMenu2/3");
            // Ventana real del hilo (clase de sistema "STATIC"), nunca visible.
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("naygo-test"),
                WS_OVERLAPPED,
                0,
                0,
                10,
                10,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let comctl = Comctl32Subclass::get().expect("subclases de comctl32 por ordinal");
            let mut data = 0usize;
            {
                let guard = MenuSubclass::install(hwnd, &handler).expect("subclase instalada");
                assert!(
                    (comctl.get)(hwnd, Some(menu_subclass_proc), guard.id, &mut data).as_bool()
                );
                assert_eq!(data, &handler as *const MenuMsgHandler as usize);
                // Un mensaje ajeno al menú atraviesa la subclase hasta DefSubclassProc.
                let _ = SendMessageW(hwnd, 0x0000, Some(WPARAM(0)), Some(LPARAM(0)));
                // Owner-draw que no es de menú (wParam != 0) tampoco se reenvía.
                let _ = SendMessageW(hwnd, WM_MEASUREITEM, Some(WPARAM(5)), Some(LPARAM(0)));
            }
            let id = &handler as *const MenuMsgHandler as usize;
            assert!(
                !(comctl.get)(hwnd, Some(menu_subclass_proc), id, &mut data).as_bool(),
                "el guard quitó la subclase"
            );
            let _ = DestroyWindow(hwnd);
            drop(handler);
            drop(menu);
        }
        drop(pidls);
        unsafe { CoUninitialize() };
    }

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
