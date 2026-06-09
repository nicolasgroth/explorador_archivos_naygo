# Naygo — shell-B: menú contextual nativo de Windows — Diseño

> Última fase del sprint de funcionalidad. Activa el menú contextual NATIVO del shell
> de Windows (el del clic derecho del Explorer) sobre los archivos/carpetas
> seleccionados, vía COM (IContextMenu / IShellFolder + TrackPopupMenuEx).

Autor: Nicolás Groth / ISGroth (Chile), 2026, MIT. Repo:
`github.com/nicolasgroth/explorador_archivos_naygo`.

## Contexto y punto de partida

- `platform` aísla TODO lo COM/Win32 (precedente directo: `trash.rs` con
  `IFileOperation`, init COM cuidadoso). Módulos actuales: clipboard, device_watch,
  dir_watch, drive_space, drives, locale, open, trash.
- `crates/platform/Cargo.toml` ya habilita las features del crate `windows` 0.62:
  `Win32_UI_Shell`, `Win32_UI_Shell_Common`, `Win32_UI_WindowsAndMessaging`,
  `Win32_System_Com`, `Win32_Foundation`, `Win32_System_Ole`, `Win32_System_Memory`,
  etc. (Verificar al implementar si falta alguna sub-feature de menús/PIDL, p. ej.
  algo de `Win32_UI_Shell_Common` o funciones `ILxxx`.)
- `crates/ui/src/panes/file_panel.rs` (~370): el menú contextual del file panel ya
  tiene, como ÚLTIMO ítem, un placeholder DESHABILITADO:
  `ui.add_enabled(false, egui::Button::new(i18n.t("op.more_windows"))).on_disabled_hover_text(i18n.t("op.more_windows_soon"))`.
  shell-B lo activa. Las acciones del menú se DIFIEREN a `NaygoApp` vía `ops_actions`
  (patrón de acción diferida: acumular en el closure, procesar tras pintar).
- `trash.rs` muestra el patrón COM a seguir: `CoInitializeEx(None,
  COINIT_APARTMENTTHREADED)`, `needs_uninit = hr.is_ok()` (cubre S_OK y S_FALSE),
  `CoUninitialize` SOLO si `needs_uninit` (NO si `RPC_E_CHANGED_MODE`, porque el hilo
  de UI ya fue inicializado por eframe/winit), toda la secuencia en un `unsafe`
  acotado, errores como `Result` tipado, nunca panic.
- La UI NO captura hoy el HWND de la ventana (no usa raw-window-handle). eframe 0.34
  lo expone (re-exporta `raw_window_handle`); se capturará en `NaygoApp::new` desde el
  `CreationContext`.

## Decisiones tomadas (brainstorm 2026-06-09)

- **Enfoque**: menú NATIVO real vía HWND — construir el `IContextMenu` y mostrarlo con
  `TrackPopupMenuEx` sobre la ventana de Naygo. Fidelidad total (submenús, íconos,
  "Enviar a", "Propiedades", apps de terceros). NO reimplementar el menú en egui.
- **Disparo**: submenú/ítem "Más opciones de Windows…" (el placeholder existente). El
  menú propio de Naygo (rápido) primero; el nativo SOLO bajo demanda al clickear ese
  ítem (es caro de construir: enumera handlers del shell).
- **Objetos**: los archivos/carpetas SELECCIONADOS (o el enfocado bajo el clic
  derecho; multi-selección si hay varias). El fondo del panel (menú de la carpeta
  actual) queda FUERA de alcance (iteración posterior).
- **Bloqueo UI**: `TrackPopupMenuEx` es modal/bloqueante — se bloquea el hilo de UI
  mientras el menú está abierto (comportamiento normal de Windows; es interacción
  explícita del usuario, NO I/O de fondo). Sin hilos extra, sin COM cross-apartment.
- **Post-comando**: tras invocar un comando, RE-LISTAR el panel siempre (garantía
  inmediata de consistencia; el watcher es respaldo).
- **Sin firma / docs**: documentación entregable de primera clase (doc-comments ricos
  del flujo COM + nota de integración con el shell + estado del README).

## Componentes

### 1. `platform::context_menu` (NUEVO módulo, todo COM/Win32)

API de alto nivel:
```rust
/// Resultado de mostrar el menú nativo.
pub enum NativeMenuOutcome {
    /// El usuario invocó un comando (el panel debería re-listarse).
    Invoked,
    /// El usuario canceló (clic afuera / Esc).
    Cancelled,
}

#[derive(Debug)]
pub enum ShellError {
    NotSupported,        // no-Windows
    NoItems,             // ninguna ruta resolvió a PIDL
    Failed(String),      // HRESULT u otro fallo COM
}

/// Muestra el menú contextual nativo de Windows para `paths` en (x, y) de pantalla,
/// sobre la ventana `hwnd`. Bloqueante (modal). Tolerante: rutas inválidas se omiten;
/// si no queda ninguna → NoItems; cualquier fallo COM → Failed. Nunca panic.
#[cfg(windows)]
pub fn show_native_context_menu(
    hwnd: isize,            // HWND de la ventana de Naygo (raw)
    paths: &[PathBuf],
    x: i32,
    y: i32,
) -> Result<NativeMenuOutcome, ShellError>;

// stub no-Windows: devuelve Err(ShellError::NotSupported).
```
(El tipo exacto del HWND — `isize` raw vs `windows::Win32::Foundation::HWND` — se
decide al implementar según cómo llegue desde raw-window-handle; la firma pública
puede tomar `isize` y construir el `HWND` adentro para no filtrar el crate `windows`
a la UI.)

Secuencia interna (hilo de UI, `unsafe` acotado, tolerante por paso):
1. `CoInitializeEx(None, COINIT_APARTMENTTHREADED)`; `needs_uninit = hr.is_ok()`
   (patrón trash.rs; `CoUninitialize` al final SOLO si `needs_uninit`).
2. Rutas → PIDLs absolutos (`SHParseDisplayName` o `ILCreateFromPath`). Ruta que
   falla se omite (loguea). Si no queda ninguna → `NoItems`.
3. Padre + hijos: `SHBindToParent` para obtener el `IShellFolder` de la carpeta padre
   y el PIDL hijo (relativo) de cada ítem. (Asume que la selección comparte carpeta —
   verdadero en un panel; si hubiera rutas de carpetas distintas, se usa la primera
   carpeta y se omiten las de otras, o se documenta la limitación.)
4. `IContextMenu`: `parent.GetUIObjectOf(hwnd, &[child_pidls], &IID_IContextMenu)`.
5. `HMENU`: `CreatePopupMenu` + `QueryContextMenu(hmenu, 0, CMD_MIN, CMD_MAX,
   CMF_NORMAL)` con un rango de IDs reservado (p. ej. `CMD_MIN=1`, `CMD_MAX=0x7FFF`).
6. Modal: `TrackPopupMenuEx(hmenu, TPM_RETURNCMD | TPM_RIGHTBUTTON, x, y, hwnd, None)`
   → ID elegido (0 = cancelado).
7. Invocar (si ID != 0): armar `CMINVOKECOMMANDINFOEX` (verbo = `id - CMD_MIN` por
   `MAKEINTRESOURCE`/`lpVerb` como ordinal, `hwnd` owner, `nShow = SW_SHOWNORMAL`),
   `context_menu.InvokeCommand(&info)`. → `Invoked`.
8. Limpieza: `DestroyMenu`, liberar PIDLs (`CoTaskMemFree`/`ILFree`), interfaces COM
   se sueltan por Drop del crate `windows`; `CoUninitialize` si `needs_uninit`.

Devuelve `Ok(Invoked)` / `Ok(Cancelled)` / `Err(ShellError)`. Cada paso COM mapea su
fallo a `ShellError::Failed(hresult.to_string())` vía `?`. Nunca panic.

Doc-comments RICOS (entregable de primera clase): explicar la cadena PIDL→IShellFolder
→IContextMenu→HMENU→TrackPopupMenu→InvokeCommand, por qué apartment-threaded en el
hilo de UI, la sutileza de `RPC_E_CHANGED_MODE`, y la liberación de PIDLs.

### 2. UI — activar el placeholder + procesar la acción

- `crates/ui/src/panes/file_panel.rs`: el ítem "Más opciones de Windows…" se HABILITA.
  Al clickearlo, difiere una acción nueva con la posición del clic, p. ej.
  `Action::ShowNativeMenu { x, y }` (o se reutiliza `ops_actions` con un nuevo verbo).
  NO se llama COM dentro del closure de egui — se difiere a `NaygoApp` (patrón de
  acción diferida). La posición de pantalla del menú se obtiene del puntero
  (`ui.input(|i| i.pointer.interact_pos())` mapeado a coords de pantalla, o las
  coords del evento; verificar cómo obtener coords de PANTALLA en egui 0.34 —
  `ViewportInfo`/`ctx.input` + offset de la ventana).
- `NaygoApp`:
  - Guarda el **HWND** capturado en `new` desde `CreationContext` vía
    `raw_window_handle` (eframe 0.34). Si no se obtiene → `None`, y el ítem del menú
    queda DESHABILITADO (degradación limpia, como hoy).
  - Al procesar la acción: junta las rutas seleccionadas (o la enfocada del panel
    activo), llama `platform::context_menu::show_native_context_menu(hwnd, &paths,
    x, y)`.
  - `Ok(Invoked)` → re-listar el panel activo (consistencia inmediata). `Ok(Cancelled)`
    → nada. `Err(_)` → status discreto (i18n).
- i18n: las claves `op.more_windows` / `op.more_windows_soon` ya existen (shell-A);
  ajustar el texto del tooltip si hace falta (ya no es "próximamente"). Agregar una
  clave de error si el menú nativo falla.

### 3. HWND desde eframe

En `NaygoApp::new(cc, ...)`, obtener el raw window handle del `CreationContext` (eframe
0.34 implementa `HasWindowHandle` o expone `cc.window_handle()` / un raw handle).
Extraer el `HWND` (variante `RawWindowHandle::Win32`) como `isize` y guardarlo en un
campo `hwnd: Option<isize>`. Verificar la API exacta de eframe/winit 0.34 para esto;
si la versión no lo expone limpio, documentar el fallback (ítem deshabilitado).

## Verificación (reparto)

- **El agente**: compila (`cargo build`), `clippy --workspace --all-targets -- -D
  warnings`, `fmt`, y tests que NO requieran interacción modal:
  - `show_native_context_menu` con rutas inexistentes → `Err(NoItems)`/`Err(Failed)`
    o el caso vacío, sin panic.
  - El stub no-Windows → `Err(NotSupported)`.
  - (El grueso de la cadena COM es interactivo y no se puede testear headless; se
    documenta.)
- **Nicolás (visual/manual)**: clic derecho sobre un archivo → "Más opciones de
  Windows…" → aparece el menú NATIVO del Explorer (Propiedades, Enviar a, comprimir,
  7-Zip/Git si están instalados, etc.); elegir un comando lo ejecuta; el panel se
  re-lista; cancelar no hace nada; multi-selección opera sobre todos.

## Fuera de alcance (explícito)

- Menú nativo del **fondo del panel** (carpeta actual: Nuevo→, Pegar, Propiedades de
  la carpeta) — es otro camino COM (IShellFolder de la carpeta misma); iteración
  posterior.
- Verbo default vía IContextMenu / doble-clic (ya cubierto por `open_default` de
  shell-A).
- Owner-draw, íconos custom, o filtrar/editar el menú nativo — se usa el HMENU del
  shell tal cual.
- Selección con rutas de CARPETAS DISTINTAS en un mismo menú (se asume carpeta común;
  documentar la limitación si aplica).

## Notas de riesgo / cuidado para el plan

- **Firmas COM en `windows` 0.62**: `SHParseDisplayName`, `SHBindToParent`,
  `IShellFolder::GetUIObjectOf`, `IContextMenu::QueryContextMenu`/`InvokeCommand`,
  `TrackPopupMenuEx`, `CreatePopupMenu`/`DestroyMenu`, `CMINVOKECOMMANDINFOEX`,
  `ILFree`/`CoTaskMemFree`. Verificar cada una contra la fuente del crate al
  implementar; algunas viven en sub-features que quizá haya que añadir al Cargo.toml.
- **HWND desde eframe 0.34**: confirmar la API (`window_handle()` / raw-window-handle
  re-export). Es el punto más incierto; si no se obtiene, ítem deshabilitado.
- **Coords de pantalla**: `TrackPopupMenuEx` usa coords de PANTALLA, no de la ventana.
  Mapear la posición del puntero de egui (coords de ventana) a pantalla (sumar la
  posición de la ventana, vía `ViewportInfo`/`outer_rect`). Verificar.
- **Apartment threading**: COM en el hilo de UI ya inicializado por eframe — seguir
  EXACTO el patrón `RPC_E_CHANGED_MODE` de trash.rs para no desbalancear el refcount.
- **Liberación de PIDLs**: cada PIDL de `SHParseDisplayName` se libera con
  `CoTaskMemFree`/`ILFree`; el del padre lo gestiona `SHBindToParent` (NO liberar el
  hijo que devuelve, es interno). Cuidar el ownership exacto para no liberar de más.
- **InvokeCommand puede bloquear/abrir diálogos** (Propiedades es modal). Es esperado
  y aceptable (bloqueo de UI consentido). Algunos comandos pueden tardar; está OK.
- **Multi-selección de carpetas distintas**: si la selección cruza carpetas, el camino
  `SHBindToParent` por la primera carpeta no cubre las demás; documentar/limitar.
- **Documentación**: doc-comments del flujo COM + actualizar README (este es el ÚLTIMO
  ítem del sprint → el README pasa a "sprint de funcionalidad completo").
