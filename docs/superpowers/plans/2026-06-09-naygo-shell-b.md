# shell-B — Menú contextual nativo de Windows — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** El ítem "Más opciones de Windows…" del menú contextual de Naygo muestra el menú NATIVO del shell de Windows (IContextMenu + TrackPopupMenuEx) sobre los archivos/carpetas seleccionados, e invoca el comando elegido.

**Architecture:** Un módulo nuevo `platform::context_menu` encapsula toda la cadena COM (PIDLs → IShellFolder → IContextMenu → HMENU → TrackPopupMenuEx → InvokeCommand), siguiendo el patrón de init COM de `trash.rs`. La UI captura el HWND de eframe en `NaygoApp::new`, habilita el placeholder existente, y al clickearlo difiere una petición (con coords de pantalla) que `NaygoApp` procesa llamando a `platform` y re-listando el panel si se invocó un comando.

**Tech Stack:** Rust, crate `windows` 0.62 (Win32 Shell/COM/Menus), eframe/egui 0.34, `raw-window-handle` 0.6.

**Estado de partida (rama `feat/shell-b`, desde `main` e74a24c):**
- `crates/platform/src/trash.rs`: PATRÓN COM a copiar. `CoInitializeEx(None, COINIT_APARTMENTTHREADED)` → `needs_uninit = hr.is_ok()`; toda la secuencia en un `unsafe` acotado dentro de una clausura `(|| -> Result<...> {...})()`; al final `if needs_uninit { CoUninitialize(); }`; errores `.map_err(|e| Err::Failed(e.to_string()))?`. Stub `#[cfg(not(windows))]` → `Err(NotSupported)`. Tests `#[cfg(windows)]`.
- `crates/platform/src/lib.rs`: lista de `pub mod` (clipboard, device_watch, dir_watch, drive_space, drives, locale, open, trash) + un `pub fn hello()`. (El doc-comment del módulo dice "en la Fase 1 está vacío" — desactualizado, no tocar salvo que moleste.)
- `crates/platform/Cargo.toml`: `windows` 0.62 con features `Win32_Globalization, Win32_Storage_FileSystem, Win32_System_WindowsProgramming, Win32_Foundation, Win32_System_Com, Win32_System_LibraryLoader, Win32_UI_Shell, Win32_UI_Shell_Common, Win32_UI_WindowsAndMessaging, Win32_System_DataExchange, Win32_System_Memory, Win32_System_Ole, Win32_Graphics_Gdi`. `tempfile` dev-dep.
- `crates/ui/src/panes/file_panel.rs`: la fn de render recibe `ops_actions: &mut Vec<Action>` (param, línea 60). El menú contextual (`row_resp.context_menu(|ui| {...})`, ~335-372) empuja `Action::Open/OpenWith/Copy/Cut/Paste/Rename/Delete` y al final tiene el placeholder DESHABILITADO:
  ```rust
  ui.add_enabled(false, egui::Button::new(i18n.t("op.more_windows")))
      .on_disabled_hover_text(i18n.t("op.more_windows_soon"));
  ```
  `context_focus = Some(i)` marca la fila del clic derecho (procesado después en ~409).
- `crates/core/src/keymap.rs:90`: `pub enum Action { ... Open, OpenWith, ... Delete, ..., ComputeSize }` — SIN payload. NO agregar variantes con datos aquí (poluciona keymap/i18n). El menú nativo se difiere por OTRO canal (ver Task 4).
- `crates/ui/src/app.rs`:
  - `pub fn apply_action(&mut self, action: Action)` (1344) — dispatch de acciones.
  - `fn selected_paths(&self) -> Vec<PathBuf>` (1486) — selección o entry enfocada del panel activo.
  - `fn active_dir(&self) -> Option<PathBuf>` (~1512).
  - `pub fn refresh_pane(&mut self, id: PaneId, dir: PathBuf)` (759) — re-lista.
  - `self.workspace.active_id() -> Option<PaneId>`, `self.workspace.active_files()`.
  - En `update`/`ui` se acumula `let mut ops_actions: Vec<Action> = Vec::new();` (2419), se pasa a `file_panel` (2433), y se drena `for action in ops_actions { ... apply_action ... }` (2471).
  - `NaygoApp::new(cc: &CreationContext, initial_dir: Option<PathBuf>)`.
- eframe 0.34 `CreationContext` implementa `HasWindowHandle` → `cc.window_handle() -> Result<WindowHandle, HandleError>`; `.as_raw() -> RawWindowHandle`; variante `RawWindowHandle::Win32(Win32WindowHandle { hwnd: NonZeroIsize, .. })`. `raw-window-handle` 0.6.2 es dep de eframe; el crate `ui` NO lo tiene directo → hay que añadirlo.

**Prerequisito de entorno:** Rust en PATH (`export PATH="$HOME/.cargo/bin:$PATH"`). NUNCA `2>&1` con cargo. `cargo fmt --all` antes de cada commit. Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt verdes antes de cada commit. Header de 2 líneas en archivos nuevos.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. `core` NO se toca (esto es 100% Windows/COM en `platform` + glue en `ui`). Tolerante: ruta inválida se omite, fallo COM → `Result`, NUNCA panic. **Documentación = entregable de primera clase** (doc-comments ricos del flujo COM; Task 6 actualiza el README a "sprint completo"). Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/shell-b`. NO cambiar de rama.

**Reparto de verificación:** el agente compila, clippy, fmt, y corre tests NO interactivos (rutas inválidas → `Err` sin panic; stub no-Windows). El grueso de la cadena COM es INTERACTIVO (abre un menú modal) y lo prueba Nicolás en vivo.

**SECUENCIA / riesgos:** Task 1 (platform::context_menu) es el grueso técnico y el más propenso a bugs (ownership de PIDLs, apartment threading). Tasks 2-5 son glue de UI. Las firmas COM de `windows` 0.62 deben verificarse contra la fuente al implementar — si una difiere de lo escrito, adaptar y reportar. El HWND de eframe y las coords de pantalla son los puntos inciertos de la UI.

---

## Estructura de archivos

```
crates/platform/src/context_menu.rs   # NUEVO: show_native_context_menu (toda la cadena COM)
crates/platform/src/lib.rs            # + pub mod context_menu;
crates/platform/Cargo.toml            # + sub-features de windows si el build las pide
crates/ui/Cargo.toml                  # + raw-window-handle = "0.6"
crates/ui/src/app.rs                  # captura HWND + campo + procesa la petición de menú nativo
crates/ui/src/panes/file_panel.rs     # habilita el placeholder + difiere la petición
crates/core/src/i18n/{es,en}.json     # ajustar tooltip + clave de error
README.md                             # estado: sprint completo
```

---

## Task 1: `platform::context_menu` — la cadena COM completa

**Files:**
- Create: `crates/platform/src/context_menu.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify (si el build lo pide): `crates/platform/Cargo.toml`

- [ ] **Step 1: Esqueleto + tipos + stub + tests (TDD primero lo testeable)**

Create `crates/platform/src/context_menu.rs` with the public types, the non-Windows stub, and the Windows entry point skeleton that handles the trivial tolerant cases first (empty / unresolvable paths) so a non-interactive test can pass:
```rust
// Naygo — menú contextual NATIVO del shell de Windows (COM, aislado).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Muestra el menú contextual nativo de Windows (el del clic derecho del Explorer)
//! para un conjunto de rutas, vía la cadena COM del Shell:
//!
//!   rutas → PIDLs (SHParseDisplayName)
//!         → carpeta padre + PIDL hijo (SHBindToParent → IShellFolder)
//!         → IContextMenu (IShellFolder::GetUIObjectOf)
//!         → HMENU (CreatePopupMenu + IContextMenu::QueryContextMenu)
//!         → elección modal (TrackPopupMenuEx, BLOQUEANTE)
//!         → ejecución del comando (IContextMenu::InvokeCommand)
//!
//! COM se inicializa apartment-threaded en el MISMO hilo de UI (ya inicializado por
//! eframe/winit); por eso seguimos el patrón de `trash.rs`: `CoUninitialize` solo si
//! `CoInitializeEx` realmente inicializó en este hilo (no ante `RPC_E_CHANGED_MODE`).
//! Tolerante: una ruta que no resuelve se omite; si no queda ninguna → `NoItems`;
//! cualquier HRESULT de fallo → `Failed`. NUNCA hace panic.

use std::path::PathBuf;

/// Qué pasó al mostrar el menú nativo.
#[derive(Debug, PartialEq, Eq)]
pub enum NativeMenuOutcome {
    /// El usuario eligió un comando (el panel debería re-listarse).
    Invoked,
    /// El usuario canceló (clic afuera / Esc).
    Cancelled,
}

/// Error al construir o mostrar el menú nativo.
#[derive(Debug)]
pub enum ShellError {
    /// No es Windows.
    NotSupported,
    /// Ninguna ruta resolvió a un PIDL (todas inválidas/ausentes o lista vacía).
    NoItems,
    /// Fallo COM/Shell; el mensaje describe el HRESULT.
    Failed(String),
}

/// Stub no-Windows: el menú nativo no existe fuera de Windows.
#[cfg(not(windows))]
pub fn show_native_context_menu(
    _hwnd: isize,
    _paths: &[PathBuf],
    _x: i32,
    _y: i32,
) -> Result<NativeMenuOutcome, ShellError> {
    Err(ShellError::NotSupported)
}
```
And add the Windows implementation (Step 2). For TDD now, add tests that don't open a modal menu:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[cfg(not(windows))]
    #[test]
    fn no_windows_es_notsupported() {
        let r = show_native_context_menu(0, &[PathBuf::from("x")], 0, 0);
        assert!(matches!(r, Err(ShellError::NotSupported)));
    }

    #[cfg(windows)]
    #[test]
    fn lista_vacia_es_noitems() {
        // Sin rutas → NoItems, sin abrir ningún menú (retorna antes de TrackPopupMenu).
        let r = show_native_context_menu(0, &[], 0, 0);
        assert!(matches!(r, Err(ShellError::NoItems)), "esperaba NoItems, fue {r:?}");
    }

    #[cfg(windows)]
    #[test]
    fn rutas_inexistentes_no_panic() {
        // Rutas que no resuelven a PIDL → NoItems (no abre menú, no panic). hwnd=0
        // es inofensivo porque retornamos antes de TrackPopupMenu.
        let r = show_native_context_menu(
            0,
            &[PathBuf::from("Z:\\no\\existe\\naygo-xyz-12345")],
            0,
            0,
        );
        assert!(
            matches!(r, Err(ShellError::NoItems)) || matches!(r, Err(ShellError::Failed(_))),
            "esperaba NoItems/Failed sin panic, fue {r:?}"
        );
    }
}
```
IMPORTANT for the test to be valid: the Windows implementation (Step 2) MUST resolve all PIDLs and return `NoItems` BEFORE calling `TrackPopupMenuEx`. So an empty/all-invalid path set never opens a modal menu — the test is non-interactive and safe in CI/headless.

- [ ] **Step 2: Implementar la cadena COM (Windows)**

Add the `#[cfg(windows)]` implementation. Mirror trash.rs's COM init/uninit balance EXACTLY. VERIFY every `windows` 0.62 signature against the crate source as you go (the names below are the intended ones; adapt to the real API and report any difference):
```rust
#[cfg(windows)]
pub fn show_native_context_menu(
    hwnd: isize,
    paths: &[PathBuf],
    x: i32,
    y: i32,
) -> Result<NativeMenuOutcome, ShellError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, POINT};
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        IContextMenu, IShellFolder, SHBindToParent, SHParseDisplayName, CMINVOKECOMMANDINFOEX,
    };
    use windows::Win32::UI::Shell::Common::ITEMIDLIST;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreatePopupMenu, DestroyMenu, TrackPopupMenuEx, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    };

    if paths.is_empty() {
        return Err(ShellError::NoItems);
    }
    let win = HWND(hwnd as *mut core::ffi::c_void); // adaptar al ctor real de HWND en 0.62

    const CMD_MIN: u32 = 1;
    const CMD_MAX: u32 = 0x7FFF;

    // SAFETY: toda la cadena COM en un unsafe acotado; CoUninitialize balanceado como
    // en trash.rs (solo si needs_uninit). Los PIDLs de SHParseDisplayName se liberan
    // con CoTaskMemFree; el PIDL hijo que devuelve SHBindToParent NO se libera (es
    // interno a la lista del padre). hwnd válido provisto por la UI.
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let needs_uninit = hr.is_ok();

        let result = (|| -> Result<NativeMenuOutcome, ShellError> {
            // 1. Rutas → PIDLs absolutos. Ruta que falla se omite.
            let mut pidls: Vec<*mut ITEMIDLIST> = Vec::new();
            for p in paths {
                let wide: Vec<u16> = p
                    .as_os_str()
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
                // SHParseDisplayName(PCWSTR, pbc, &mut pidl, sfgao_in, &mut sfgao_out)
                let ok = SHParseDisplayName(
                    PCWSTR(wide.as_ptr()),
                    None,
                    &mut pidl,
                    0,
                    std::ptr::null_mut(), // adaptar: puede pedir &mut u32
                );
                if ok.is_ok() && !pidl.is_null() {
                    pidls.push(pidl);
                }
            }
            if pidls.is_empty() {
                return Err(ShellError::NoItems);
            }

            // 2. Carpeta padre + PIDLs hijos. Asume carpeta común; se vincula por el
            //    primero y se reúnen los hijos (relativos) de los que comparten padre.
            let mut parent: Option<IShellFolder> = None;
            let mut children: Vec<*const ITEMIDLIST> = Vec::new();
            for &pidl in &pidls {
                let mut child: *mut ITEMIDLIST = std::ptr::null_mut();
                let mut sf: Option<IShellFolder> = None;
                // SHBindToParent(pidl, &IID_IShellFolder, &mut ppv, &mut child)
                let hr = SHBindToParent(pidl, &IShellFolder::IID, &mut sf as *mut _ as *mut _, &mut child);
                if hr.is_ok() {
                    if parent.is_none() {
                        parent = sf;
                    }
                    if !child.is_null() {
                        children.push(child as *const _);
                    }
                }
            }
            let parent = parent.ok_or(ShellError::NoItems)?;
            if children.is_empty() {
                return Err(ShellError::NoItems);
            }

            // 3. IContextMenu de los ítems.
            let mut ctxmenu: Option<IContextMenu> = None;
            parent
                .GetUIObjectOf(
                    win,
                    &children.iter().map(|&c| c).collect::<Vec<_>>(), // adaptar al tipo que pide GetUIObjectOf
                    &IContextMenu::IID,
                    None,
                    &mut ctxmenu as *mut _ as *mut _,
                )
                .map_err(|e| ShellError::Failed(e.to_string()))?;
            let ctxmenu = ctxmenu.ok_or_else(|| ShellError::Failed("IContextMenu nulo".into()))?;

            // 4. HMENU + QueryContextMenu.
            let hmenu = CreatePopupMenu().map_err(|e| ShellError::Failed(e.to_string()))?;
            let cleanup_menu = |h| { let _ = DestroyMenu(h); };
            ctxmenu
                .QueryContextMenu(hmenu, 0, CMD_MIN, CMD_MAX, CMF_NORMAL) // CMF_NORMAL = 0
                .map_err(|e| { cleanup_menu(hmenu); ShellError::Failed(e.to_string()) })?;

            // 5. Mostrar modal (BLOQUEA hasta elegir/cancelar). Coords de PANTALLA.
            let chosen = TrackPopupMenuEx(
                hmenu,
                (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
                x,
                y,
                win,
                None,
            );
            let id = chosen.0 as u32; // 0 = cancelado

            // 6. Invocar el comando elegido.
            let outcome = if id != 0 {
                let verb = (id - CMD_MIN) as usize;
                let mut info = CMINVOKECOMMANDINFOEX::default();
                info.cbSize = std::mem::size_of::<CMINVOKECOMMANDINFOEX>() as u32;
                info.hwnd = win;
                info.lpVerb = windows::core::PCSTR(verb as *const u8); // MAKEINTRESOURCEA(verb)
                info.nShow = windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL.0;
                // InvokeCommand(&info as *const _ as *const CMINVOKECOMMANDINFO)
                ctxmenu
                    .InvokeCommand(&info as *const _ as *const _)
                    .map_err(|e| ShellError::Failed(e.to_string()))?;
                NativeMenuOutcome::Invoked
            } else {
                NativeMenuOutcome::Cancelled
            };

            cleanup_menu(hmenu);
            // Liberar SOLO los PIDLs absolutos de SHParseDisplayName.
            for &pidl in &pidls {
                windows::Win32::System::Com::CoTaskMemFree(Some(pidl as *const _));
            }
            Ok(outcome)
        })();

        if needs_uninit {
            CoUninitialize();
        }
        result
    }
}
```
NOTE: the EXACT signatures (`SHParseDisplayName` arg count, `GetUIObjectOf` apidl type `&[*const ITEMIDLIST]` vs a count+ptr, `HWND` constructor, `CMINVOKECOMMANDINFOEX` field names, `CMF_NORMAL`/`SW_SHOWNORMAL` constants, `CoTaskMemFree`/`ILFree`) MUST be verified against `windows` 0.62 source (`~/.cargo/registry/src/*/windows*/`). Adapt the code until it compiles; keep the STRUCTURE (resolve-before-TrackPopup, trash.rs uninit balance, free SHParseDisplayName pidls only, never free the SHBindToParent child). If a needed item lives in a feature not yet enabled, add the sub-feature to `crates/platform/Cargo.toml` and report it.

- [ ] **Step 3: Declarar el módulo**

In `crates/platform/src/lib.rs`, add `pub mod context_menu;` to the module list.

- [ ] **Step 4: Verificar**

Run: `cargo build -p naygo-platform` → compiles (fix signatures/features until it does).
Run: `cargo test -p naygo-platform context_menu` → the non-interactive tests PASS (lista_vacia_es_noitems, rutas_inexistentes_no_panic on Windows; no_windows on other OS). These must NOT open a modal menu.
Run: `cargo clippy -p naygo-platform --all-targets -- -D warnings` → clean. `cargo fmt --all`.

- [ ] **Step 5: Commit**
```
git add crates/platform/src/context_menu.rs crates/platform/src/lib.rs crates/platform/Cargo.toml
git commit -m "feat(platform): menú contextual nativo de Windows vía COM (context_menu)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `show_native_context_menu(isize, &[PathBuf], i32, i32) -> Result<NativeMenuOutcome, ShellError>`, `NativeMenuOutcome{Invoked,Cancelled}`, `ShellError{NotSupported,NoItems,Failed}` EXACTOS (Tasks 4-5 dependen).

---

## Task 2: UI — capturar el HWND de eframe

**Files:**
- Modify: `crates/ui/Cargo.toml`
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: Añadir raw-window-handle al crate ui**

In `crates/ui/Cargo.toml` `[dependencies]`, add:
```toml
raw-window-handle = "0.6"
```
(Match eframe's version 0.6.2; `"0.6"` resolves to it.)

- [ ] **Step 2: Capturar el HWND en NaygoApp::new + campo**

Modify `crates/ui/src/app.rs`:
a) Add a field to `NaygoApp`: `hwnd: Option<isize>,`.
b) In `NaygoApp::new(cc, initial_dir)`, near the top (before building the struct), extract the HWND:
```rust
        // HWND de la ventana (para el menú contextual nativo de Windows). Si no se
        // puede obtener, queda None y el ítem "Más opciones de Windows…" se deshabilita.
        let hwnd: Option<isize> = {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            match cc.window_handle() {
                Ok(h) => match h.as_raw() {
                    RawWindowHandle::Win32(w) => Some(w.hwnd.get()),
                    _ => None,
                },
                Err(_) => None,
            }
        };
```
Add `hwnd,` to the `NaygoApp { ... }` struct literal.
(VERIFY: `cc.window_handle()` comes from `HasWindowHandle` (eframe's CreationContext impls it). `as_raw()` returns `RawWindowHandle`; `Win32` variant has `hwnd: NonZeroIsize` → `.get()` gives `isize`. Adapt field/method names to raw-window-handle 0.6.2 exactly.)

- [ ] **Step 3: Verificar**

Run: `cargo build -p naygo-ui` → compiles. `cargo clippy -p naygo-ui --all-targets -- -D warnings` → clean (the `hwnd` field will be "unused" until Task 5 — if clippy flags dead_code, that's expected; Task 5 uses it THIS branch, so prefer to land Task 5 before worrying; if you must commit now and clippy fails, add a temporary `#[allow(dead_code)]` on the field WITH a comment "usado en Task 5" and remove it in Task 5). `cargo fmt --all`.

- [ ] **Step 4: Commit**
```
git add crates/ui/Cargo.toml crates/ui/src/app.rs
git commit -m "feat(ui): capturar el HWND de la ventana (para el menú nativo)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `hwnd: Option<isize>` EXACTO (Task 5 lo usa).

---

## Task 3: i18n — ajustar tooltip + clave de error

**Files:**
- Modify: `crates/core/src/i18n/es.json`
- Modify: `crates/core/src/i18n/en.json`

- [ ] **Step 1: Ajustar claves en AMBOS json**

Read both. `op.more_windows` ("Más opciones de Windows…") se conserva. `op.more_windows_soon` (tooltip "próximamente…") YA NO aplica — reemplazar su VALOR por una descripción vigente, p. ej.:
- ES `op.more_windows_soon`: `"Abre el menú contextual de Windows para los elementos seleccionados"`
- EN `op.more_windows_soon`: `"Opens the Windows context menu for the selected items"`
(Se mantiene la CLAVE para no romper referencias; solo cambia el texto. Si preferís, podés mantenerla como hover-text del ítem habilitado.)
Add an error key (used in Task 5 if the native menu fails):
- ES `op.more_windows_error`: `"No se pudo abrir el menú de Windows"`
- EN `op.more_windows_error`: `"Could not open the Windows menu"`
Keys MUST be identical in both files (i18n parity test).

- [ ] **Step 2: Verificar**

Run: `cargo test -p naygo-core i18n` → parity test green. `cargo build -p naygo-core` → ok.

- [ ] **Step 3: Commit**
```
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "i18n: textos del menú contextual nativo (shell-B)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: UI — habilitar el placeholder + diferir la petición

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/ui/src/app.rs`

El menú nativo se difiere por un canal PROPIO (no `Action`, que es enum sin payload y vive en keymap). Usamos un `Option` en `NaygoApp` con las coords de pantalla.

- [ ] **Step 1: Campo de petición en NaygoApp**

In `crates/ui/src/app.rs`, add to `NaygoApp`: `native_menu_request: Option<(f32, f32)>,` (screen x,y). Init `native_menu_request: None,` in the struct literal in `new`.

- [ ] **Step 2: file_panel — habilitar el ítem y capturar la petición**

The render fn needs a way to signal the request back to NaygoApp. The simplest: pass a `&mut Option<(f32,f32)>` out-param alongside `ops_actions`, OR reuse a new out-param. Add a parameter `native_menu_request: &mut Option<(f32, f32)>` to the file_panel render fn signature (next to `ops_actions: &mut Vec<Action>`), and in the context menu replace the disabled placeholder with an enabled button:
```rust
                            ui.separator();
                            // Menú contextual NATIVO de Windows para los ítems
                            // seleccionados (shell-B). Se difiere a NaygoApp con las
                            // coords de PANTALLA del clic (TrackPopupMenuEx usa pantalla).
                            if ui.button(i18n.t("op.more_windows")).clicked() {
                                context_focus = Some(i);
                                // Pos de pantalla = pos del puntero en la ventana +
                                // origen de la ventana en pantalla.
                                let screen = ui.input(|inp| {
                                    let p = inp.pointer.interact_pos().unwrap_or_default();
                                    let origin = inp
                                        .viewport()
                                        .outer_rect
                                        .map(|r| r.min)
                                        .unwrap_or_default();
                                    (p.x + origin.x, p.y + origin.y)
                                });
                                *native_menu_request = Some(screen);
                                ui.close();
                            }
```
(VERIFY egui 0.34: `ui.input(|i| i.pointer.interact_pos())`, `i.viewport().outer_rect: Option<Rect>` (ViewportInfo), `Rect::min`. If `outer_rect` isn't available/None, fall back to `inner_rect` or to the raw pointer pos in window coords (the menu would appear at a slightly wrong spot but still works) — keep it tolerant, never panic. Report what you used.)
Update the CALL SITE in `app.rs` (~2433) to pass the new out-param: add `let mut native_menu_request: Option<(f32,f32)> = None;` before rendering panels, pass `native_menu_request: &mut native_menu_request` (or positional) to the file_panel call, and after the render, store it: `if native_menu_request.is_some() { self.native_menu_request = native_menu_request; }`.
(Adapt to whether file_panel is called once or per-pane; if per-pane in a loop, accumulate into one Option — last writer wins, fine since only one context menu is open at a time.)

- [ ] **Step 3: Verificar (compila; el handler real es Task 5)**

Run: `cargo build -p naygo-ui` → compiles. `cargo clippy -p naygo-ui --all-targets -- -D warnings` → clean (the stored `native_menu_request` is consumed in Task 5; if clippy flags it unused, land Task 5 in the same session — or temporary allow + comment). `cargo fmt --all`.

- [ ] **Step 4: Commit**
```
git add crates/ui/src/panes/file_panel.rs crates/ui/src/app.rs
git commit -m "feat(ui): habilitar 'Más opciones de Windows' y diferir la petición

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `native_menu_request: Option<(f32,f32)>` EXACTO (Task 5 lo consume).

---

## Task 5: UI — procesar la petición: mostrar el menú nativo y re-listar

**Files:**
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: Procesar native_menu_request tras drenar ops_actions**

In `crates/ui/src/app.rs`, AFTER the `for action in ops_actions { ... }` loop (~2471), add the handler:
```rust
        // Menú contextual nativo de Windows (shell-B): se procesa fuera del closure de
        // egui (COM no debe correr dentro del render del menú). Bloquea el hilo de UI
        // mientras el menú modal está abierto (interacción explícita, no I/O de fondo).
        if let Some((sx, sy)) = self.native_menu_request.take() {
            self.show_native_menu(sx as i32, sy as i32);
        }
```
And add the method:
```rust
    /// Muestra el menú contextual NATIVO de Windows para la selección del panel activo
    /// en las coords de pantalla (sx, sy). Tras invocar un comando, re-lista el panel
    /// (consistencia inmediata). Tolerante: sin HWND o sin selección no hace nada;
    /// un fallo se reporta discreto en el status.
    fn show_native_menu(&mut self, sx: i32, sy: i32) {
        let Some(hwnd) = self.hwnd else {
            return; // sin HWND no se puede (no debería pasar: el ítem se mostraría igual)
        };
        let paths = self.selected_paths();
        if paths.is_empty() {
            return;
        }
        match naygo_platform::context_menu::show_native_context_menu(hwnd, &paths, sx, sy) {
            Ok(naygo_platform::context_menu::NativeMenuOutcome::Invoked) => {
                // Un comando pudo crear/renombrar/borrar → re-listar el panel activo.
                if let (Some(id), Some(dir)) =
                    (self.workspace.active_id(), self.active_dir())
                {
                    self.refresh_pane(id, dir);
                }
            }
            Ok(naygo_platform::context_menu::NativeMenuOutcome::Cancelled) => {}
            Err(e) => {
                tracing::warn!("menú nativo falló: {e:?}");
                self.status = self.i18n.t("op.more_windows_error");
            }
        }
    }
```
(VERIFY: `self.active_dir()` exists (~1512) and `refresh_pane(id, dir)` (759). `self.status` is the status string field. `self.i18n.t(...)`. Adapt names to the real ones in app.rs. If Task 2 added a temporary `#[allow(dead_code)]` on `hwnd`, REMOVE it now — it's used here. Same for any temp allow on `native_menu_request`.)

- [ ] **Step 2: Verificar**

Run: `cargo build --workspace` → compiles. `cargo test --workspace` → green (the platform non-interactive tests + i18n parity). `cargo clippy --workspace --all-targets -- -D warnings` → clean (no more dead_code on hwnd/native_menu_request). `cargo fmt --all`.
MANUAL (Nicolás): clic derecho sobre un archivo → "Más opciones de Windows…" → menú nativo del Explorer; invocar un comando lo ejecuta y re-lista; cancelar no hace nada.

- [ ] **Step 3: Commit**
```
git add crates/ui/src/app.rs
git commit -m "feat(ui): mostrar el menú nativo de Windows y re-listar tras invocar

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Cierre — README (sprint completo) + verificación final + push

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README — estado del sprint completo**

In `README.md`, replace the status block (currently "Fase distribución/instalador …") with:
```markdown
> **Estado:** Sprint de funcionalidad COMPLETO. Operaciones (ops-A/B), paste
> inteligente, shell-A (abrir + discos), watcher, atajos configurables, sizing (F3),
> toolbar-icons, distribución/instalador y shell-B (menú contextual nativo de Windows)
> mergeados a `main`. Diseño de shell-B en
> [`docs/superpowers/specs/2026-06-09-naygo-shell-b-design.md`](docs/superpowers/specs/2026-06-09-naygo-shell-b-design.md).
```

- [ ] **Step 2: Verificación final del workspace**

Run: `cargo build --workspace` → compiles. `cargo build --release -p naygo-ui` → release compila. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all -- --check` → clean.

- [ ] **Step 3: Commit y push**
```
git add README.md
git commit -m "docs: README — sprint de funcionalidad completo (shell-B incluido)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/shell-b
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| `platform::context_menu::show_native_context_menu` (cadena COM completa) | 1 |
| Tipos `NativeMenuOutcome` / `ShellError` + stub no-Windows | 1 |
| Resolver PIDLs antes de TrackPopup (test no interactivo) | 1 |
| Patrón COM init/uninit de trash.rs (RPC_E_CHANGED_MODE) | 1 |
| Ownership de PIDLs (liberar SHParseDisplayName, no el de SHBindToParent) | 1 |
| HWND desde eframe (raw-window-handle) | 2 |
| Habilitar el placeholder + diferir con coords de pantalla | 4 |
| Procesar la petición + invocar + re-listar | 5 |
| Coords de pantalla (pointer + origen de ventana) | 4 |
| i18n (tooltip + error) | 3 |
| Degradación sin HWND (ítem/handler no-op) | 2, 5 |
| Doc-comments ricos del flujo COM | 1 |
| README sprint completo | 6 |
| Verificación repartida (agente no-interactivo / Nicolás visual) | 1 (tests), 5/6 (manual) |
| FUERA: fondo del panel, owner-draw, multi-carpeta, verbo default | (no se tocan) |

**Notas de riesgo (recordatorio para el implementador):**
- **Firmas COM `windows` 0.62** (Task 1): el código del plan usa los nombres intencionados; VERIFICAR cada uno contra `~/.cargo/registry/src/*/windows*/` y adaptar hasta compilar — especialmente `SHParseDisplayName` (cantidad de args), `GetUIObjectOf` (tipo del array de pidls), el ctor de `HWND`, los campos de `CMINVOKECOMMANDINFOEX`, `CMF_NORMAL`/`SW_SHOWNORMAL`, `CoTaskMemFree`/`ILFree`. Reportar las diferencias. Si falta una sub-feature, añadirla al Cargo.toml de platform.
- **Ownership de PIDLs** (Task 1): liberar SOLO los absolutos de `SHParseDisplayName` con `CoTaskMemFree`; NUNCA el PIDL hijo que devuelve `SHBindToParent` (es interno). Es lo más propenso a bugs (double-free / leak).
- **Resolver-antes-de-TrackPopup** (Task 1): imprescindible para que los tests sean no-interactivos. La lista vacía / todo-inválido retorna `NoItems` sin abrir menú.
- **HWND de eframe 0.34** (Task 2): `cc.window_handle()?.as_raw()` → `RawWindowHandle::Win32(w).hwnd.get()`. Punto incierto; si la API difiere, adaptar; si no se obtiene → None → ítem deshabilitado.
- **Coords de pantalla** (Task 4): `TrackPopupMenuEx` usa coords de PANTALLA. Sumar el origen de la ventana (`viewport().outer_rect.min`) al pointer. Tolerante si no hay outer_rect.
- **dead_code transitorio** (Tasks 2,4): `hwnd`/`native_menu_request` quedan sin uso hasta Task 5 — si clippy `-D warnings` falla al commitear Tasks 2/4, usar `#[allow(dead_code)]` con comentario "usado en Task 5" y QUITARLO en Task 5.
- **InvokeCommand modal** (Task 1): "Propiedades" etc. abren diálogos propios; el bloqueo de UI es esperado y aceptable.
- **`core` no se toca**; `Action` (keymap) NO gana variantes — el menú nativo va por `native_menu_request`.
```
