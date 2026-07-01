# Hotkey global para mostrar/ocultar Naygo — Diseño

> Spec de diseño. Fecha: 2026-07-01. Autor: Nicolás Groth / ISGroth.
> Estado: aprobado, pendiente de plan de implementación.

## Qué es

Un atajo de teclado **global del sistema** (funciona desde cualquier aplicación, con
Naygo minimizado, en la bandeja o detrás de otras ventanas) que **alterna** la
visibilidad de Naygo:

- Naygo oculto (en bandeja) o minimizado → mostrar + traer al frente + enfocar.
- Naygo visible pero DETRÁS de otra ventana → traer al frente + enfocar.
- Naygo ya es la ventana activa/al frente → esconder a la bandeja.

Es lo más cercano a un "Win+E propio", pero técnicamente sólido (Windows reserva la
tecla Win y `RegisterHotKey` no la admite; ver más abajo).

## Contexto y estado actual

- **NO existe hoy** ningún hotkey global (`grep RegisterHotKey` = 0). El keymap actual
  (`crates/core/src/keymap.rs`) es de atajos INTERNOS: solo funcionan con Naygo
  enfocado. `Chord` soporta `ctrl`/`shift`/`alt` + tecla; NO tiene la tecla Win.
- **La mitad de destino ya está construida**: el tray (`crates/ui-slint/src/tray.rs`)
  ya sabe "mostrar + enfocar la ventana principal" (`TrayMsg::Open`), y el
  `on_close_requested` ya sabe esconder a bandeja. El helper `naygo_hwnd(&ui)` ya
  obtiene el HWND. Falta solo el DISPARADOR global y el cableado del toggle.
- **Patrón de integración probado**: el tray integra eventos del SO con el event loop
  de Slint vía un canal `mpsc` + un `waker` que despierta la UI. La crate
  `global-hotkey` (de los mismos autores que `tray-icon`) usa exactamente el mismo
  patrón (`GlobalHotKeyEvent::receiver()`), así que se integra idéntico.

## Decisión técnica de fondo: por qué NO la tecla Win

`RegisterHotKey` (la API Win32 de hotkeys globales) NO permite registrar combinaciones
con la tecla Windows de forma fiable: Microsoft reserva `MOD_WIN` para el sistema. NO
existe ninguna combinación `Win + <letra>` de 2 teclas que una app capture limpiamente
(Win+E = Explorador, Win+W = Ink/Widgets, Win+N = Notificaciones, etc.). Las apps que
parecen hacerlo usan hooks de teclado de bajo nivel: frágiles, se pelean con el SO, mal
comportamiento. El estándar de la industria para hotkeys globales de apps son las
combinaciones **Ctrl+Alt+<tecla>** (Slack, Discord, PowerToys, Flow Launcher), que el
SO deja libres. Por eso Naygo NO ofrece la tecla Win en la captura de combinación.

## Arquitectura (respeta las 3 capas)

### `core` (`crates/core/src/`)

- **Reusa `Chord` tal cual** (ctrl/shift/alt + `KeyCode`): ya cubre `Ctrl+Alt+Q`. No se
  añade la tecla Win (no es registrable).
- **`Settings` gana dos campos**:
  - `global_hotkey_enabled: bool` — default **`true`** (activado de fábrica).
    `#[serde(default = "default_global_hotkey_enabled")]`.
  - `global_hotkey: Chord` — default **Ctrl+Alt+Q** (`Chord { key: Char('q'), ctrl:
    true, alt: true, shift: false }`). `#[serde(default = "default_global_hotkey")]`.
- Retro-compat: un settings.json sin estos campos hereda los defaults (no requiere
  subir CONFIG_VERSION; son campos aditivos con `#[serde(default)]`).
- Si la traducción `Chord → (modificadores, virtual-key)` se puede expresar de forma
  pura (sin tocar Win32), vive en `core` o en un helper testeable; sólo la parte que
  toca `windows` va en `platform`.

### `platform` (`crates/platform/src/global_hotkey.rs`, nuevo)

Envuelve la crate `global-hotkey` (MIT, autores de `tray-icon`; añadir a
`crates/platform/Cargo.toml`). Interfaz:

- Un tipo `GlobalHotkey` que MANTIENE VIVO el registro (drop = se libera el hotkey),
  análogo a cómo `Tray` mantiene vivo el ícono.
- `register(chord: &Chord) -> Result<GlobalHotkey, String>`: traduce el `Chord` a los
  modificadores/código de la crate, registra vía `RegisterHotKey`, y devuelve `Err` con
  mensaje si el SO lo rechaza (combinación reservada / en uso).
- Un receptor de eventos integrable con el event loop (canal + waker, igual que el
  tray): cuando se presiona el hotkey, empuja un mensaje y despierta la UI.
- Stubs `#[cfg(not(windows))]` que devuelven error/nada (deja el punto único a
  reimplementar para Linux más adelante).

### `ui-slint` (`crates/ui-slint/src/main.rs` + config)

- Al arrancar, si `global_hotkey_enabled`, intenta `register(&settings.global_hotkey)`.
  Si falla → log a `naygo.log` (sin modal en cada arranque) y queda inactivo.
- Integra el receptor con el loop igual que el tray. Al recibir el evento del hotkey →
  **decide el toggle** consultando `GetForegroundWindow()` vs `naygo_hwnd()`:
  - Naygo NO es la ventana activa (oculto, minimizado o detrás) → mostrar + enfocar
    (reusa el camino de `TrayMsg::Open`).
  - Naygo ES la ventana activa → esconder a bandeja (reusa el camino de esconder-a-
    bandeja), **solo si `tray_active`**; si no hay tray, no esconde (para no dejar Naygo
    inalcanzable — coherente con `should_quit_on_close`).
- Config: setter `set_global_hotkey_enabled(bool)` y `set_global_hotkey(Chord)` en
  `ConfigCtrl`, que re-registran el hotkey (mismo patrón best-effort que
  `set_autostart_minimized`).

## Configuración (UI en la ventana de Config)

Sección Integración (junto a bandeja/autostart):

- Toggle **"Atajo global para abrir Naygo"** → `global_hotkey_enabled`.
- Campo de **captura de combinación** (reusa el mecanismo del editor de atajos del
  toolbar): el usuario hace clic, presiona la combinación, se captura y se muestra como
  texto ("Ctrl+Alt+Q"). Habilitado solo si el toggle está activo (`enabled` del Switch/
  campo, patrón ya existente).
- **Validación al capturar**: se exige al menos un modificador (Ctrl/Alt/Shift) — una
  sola tecla como hotkey global la capturaría en todo el sistema. La tecla Win no se
  ofrece.
- Claves i18n nuevas (label + tooltip) en los 10 idiomas.

## Manejo del rechazo del SO

`RegisterHotKey` puede fallar si la combinación ya está tomada.

- **Al configurar** (activar el toggle o cambiar la combinación) y el registro falla →
  aviso temático (reusa `MessageModal`): *"No se pudo registrar la combinación X (puede
  estar en uso por Windows u otra aplicación). Elige otra."* El estado NO queda como
  "activado pero sin efecto": si el registro falla, el toggle no queda encendido en
  falso.
- **Al arrancar** y el registro de la combinación guardada falla (p. ej. dejó de estar
  disponible tras un update de Windows) → se loguea a `naygo.log` y el hotkey queda
  inactivo, SIN modal (para no molestar en cada arranque). El usuario podrá recapturar
  una combinación válida en Config.

## Testing

- **`core`**: tests de serde de los campos nuevos de `Settings` (default correcto,
  retro-compat de un settings.json sin ellos); test de la traducción `Chord →
  (modificadores, vkey)` si esa lógica es pura.
- **`platform`**: sin tests unitarios (requiere el SO real, como `window_geometry`); se
  valida en la VM.
- **`ui-slint`**: test del cableado de config (toggle/captura persisten), siguiendo el
  patrón existente.
- **Verificación visual VM**: (1) presionar el hotkey desde otra app trae Naygo al
  frente; (2) presionarlo de nuevo con Naygo al frente lo esconde a bandeja; (3) cambiar
  la combinación funciona; (4) intentar una combinación reservada muestra el aviso; (5)
  con el toggle apagado, el hotkey no hace nada.

## Tabla de decisiones

| Punto | Decisión |
|-------|----------|
| Default combinación | Ctrl+Alt+Q (Win no es registrable; Ctrl+Alt es seguro) |
| Default on/off | Activado de fábrica |
| Comportamiento | Toggle: no-activa → al frente; activa → a bandeja (vía `GetForegroundWindow`) |
| Guarda | Solo esconde si `tray_active`; si no, solo trae al frente |
| Configuración | Toggle + captura de combinación; exige ≥1 modificador; sin tecla Win |
| Rechazo del SO | Modal al configurar; solo-log al arrancar |
| Crate | `global-hotkey` (MIT, autores de `tray-icon`) |
| Capas | core (config+Chord), platform/global_hotkey.rs (Win32), ui-slint (toggle+integración) |

## Fuera de alcance (YAGNI)

- Múltiples hotkeys globales o hotkeys por-acción (p. ej. uno que abra un panel nuevo).
- Soporte de la tecla Win (no registrable de forma fiable).
- **Ventana nueva del SO / multi-ventana**: es un cambio de arquitectura de fondo
  (Naygo asume una `AppWindow` + un `WorkspaceCtrl` + una sesión + una geometría). Se
  trata como PROYECTO APARTE, con su propio brainstorm/spec tras investigar la viabilidad
  en Slint. No se mezcla con este lote.

## Principios respetados

- Separación de capas: el registro Win32 en `platform`, el "qué hacer" en la UI, el
  "qué combinación" en `core`. `platform/global_hotkey.rs` queda como el único punto a
  reimplementar para Linux.
- El entorno es hostil: `register` devuelve `Result`; el SO puede rechazar; nunca panic.
- i18n desde el día uno: claves nuevas en los 10 idiomas.
- Regenerar `dist/` tras los cambios (portable + instalador).
