# Hotkey global — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Un atajo de teclado global del sistema (Ctrl+Alt+Q por defecto, configurable) que alterna Naygo entre el frente y la bandeja, funcionando desde cualquier aplicación.

**Architecture:** El registro Win32 (`RegisterHotKey` vía la crate `global-hotkey`) vive en `platform/global_hotkey.rs`, aislado tras una interfaz `Result`. El "qué combinación" son dos campos nuevos en `core::config::Settings` (reusando `Chord`). El "qué hacer" (toggle mostrar/ocultar) vive en `ui-slint/main.rs`, integrado con el event loop igual que el tray (canal + `Waker`), decidiendo con `GetForegroundWindow` vs el HWND de Naygo.

**Tech Stack:** Rust, Slint 1.16, crate `global-hotkey` (MIT, autores de `tray-icon`), Win32 (`RegisterHotKey`/`GetForegroundWindow` vía crate `windows`), serde_json.

**Rama:** `feat/hotkey-global` (ya creada; el spec ya está comiteado ahí en `1cb3695`).

**Build:** `cargo build`/`cargo test` desde la raíz. UI Slint con `CARGO_BUILD_JOBS=2` si el compilador crashea por paralelismo. Cada commit termina con la firma de coautoría del repo. Header Naygo (Naygo — … / Copyright / SPDX) en archivos nuevos.

---

## Fase 1 — Config en core (Chord + campos de Settings)

### Task 1: Constructor `Chord::ctrl_alt` + campos de Settings

**Files:**
- Modify: `crates/core/src/keymap.rs` (añadir `Chord::ctrl_alt`, junto a `ctrl_shift` ~línea 72)
- Modify: `crates/core/src/config/mod.rs` (dos campos en `Settings` + defaults)
- Test: `crates/core/src/config/mod.rs` (módulo `tests`)

- [ ] **Step 1: Añadir `Chord::ctrl_alt` en keymap.rs**

En `crates/core/src/keymap.rs`, junto a los otros constructores de `Chord` (después de
`ctrl_shift`, ~línea 80), añadir:

```rust
    /// Con Ctrl+Alt.
    pub fn ctrl_alt(key: KeyCode) -> Chord {
        Chord {
            key,
            ctrl: true,
            shift: false,
            alt: true,
        }
    }
```

- [ ] **Step 2: Escribir el test de los defaults de Settings (falla)**

En el módulo `tests` de `crates/core/src/config/mod.rs`:

```rust
    #[test]
    fn defaults_del_hotkey_global() {
        let s = Settings::default();
        assert!(s.global_hotkey_enabled, "el hotkey global viene activado de fábrica");
        // Default: Ctrl+Alt+Q.
        assert_eq!(
            s.global_hotkey,
            crate::keymap::Chord::ctrl_alt(crate::keymap::KeyCode::Char('q'))
        );
    }

    #[test]
    fn settings_viejo_sin_hotkey_hereda_defaults() {
        // Un settings.json sin los campos nuevos los hereda por #[serde(default)].
        let json = r#"{"version":2,"bar_position":"Top","icon_only":false}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert!(s.global_hotkey_enabled);
        assert_eq!(
            s.global_hotkey,
            crate::keymap::Chord::ctrl_alt(crate::keymap::KeyCode::Char('q'))
        );
    }
```

- [ ] **Step 3: Ejecutar para ver que falla**

Run: `cargo test -p naygo-core defaults_del_hotkey_global 2>&1 | head -15`
Expected: FALLA a compilar — `Settings` no tiene `global_hotkey_enabled`/`global_hotkey`.

- [ ] **Step 4: Añadir los campos a `Settings`**

En `crates/core/src/config/mod.rs`, al final de los campos del struct `Settings` (antes
del `}` que lo cierra), añadir:

```rust
    /// Atajo GLOBAL del sistema para mostrar/ocultar Naygo (funciona desde cualquier app).
    /// Activado de fábrica. `#[serde(default)]` retro-compat.
    #[serde(default = "default_global_hotkey_enabled")]
    pub global_hotkey_enabled: bool,
    /// Combinación del atajo global. Default Ctrl+Alt+Q. La tecla Win NO se soporta
    /// (`RegisterHotKey` la reserva el sistema). `#[serde(default)]` retro-compat.
    #[serde(default = "default_global_hotkey")]
    pub global_hotkey: crate::keymap::Chord,
```

Y las funciones default (junto a otras `fn default_*`):

```rust
/// Default de `global_hotkey_enabled`: true (activado de fábrica).
fn default_global_hotkey_enabled() -> bool {
    true
}

/// Default de `global_hotkey`: Ctrl+Alt+Q.
fn default_global_hotkey() -> crate::keymap::Chord {
    crate::keymap::Chord::ctrl_alt(crate::keymap::KeyCode::Char('q'))
}
```

En el `impl Default for Settings` MANUAL, añadir en la posición correcta del literal:

```rust
            global_hotkey_enabled: true,
            global_hotkey: crate::keymap::Chord::ctrl_alt(crate::keymap::KeyCode::Char('q')),
```

- [ ] **Step 5: Adaptar tests de Settings que construyen el struct completo**

Buscar en el módulo `tests` de `config/mod.rs` los literales `Settings { ... }` completos
(p. ej. `settings_round_trip`, `settings_round_trip_con_idioma`). Los que usen
`..Settings::default()` no cambian. El que enumere todos los campos (`settings_round_trip`)
necesita los dos nuevos; añadir con valores NO-default para ejercitar el round-trip, p. ej.:

```rust
            global_hotkey_enabled: false,
            global_hotkey: crate::keymap::Chord::alt(crate::keymap::KeyCode::Char('z')),
```

- [ ] **Step 6: Ejecutar los tests**

Run: `cargo test -p naygo-core 2>&1 | grep -E "test result:" | head -2`
Expected: PASS — toda la suite de core, incluidos `defaults_del_hotkey_global` y
`settings_viejo_sin_hotkey_hereda_defaults`.

- [ ] **Step 7: Clippy + commit**

Run: `cargo clippy -p naygo-core --all-targets 2>&1 | tail -8`
Expected: sin warnings.

```bash
cargo fmt -p naygo-core
git add crates/core/src/keymap.rs crates/core/src/config/mod.rs
git commit -m "$(cat <<'EOF'
feat(core): config del hotkey global (global_hotkey_enabled + global_hotkey, default Ctrl+Alt+Q)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 2 — Registro Win32 en platform

### Task 2: Módulo `platform/global_hotkey.rs` con la crate `global-hotkey`

**Files:**
- Modify: `crates/platform/Cargo.toml` (dependencia `global-hotkey`)
- Create: `crates/platform/src/global_hotkey.rs`
- Modify: `crates/platform/src/lib.rs` (declarar `pub mod global_hotkey;`)

- [ ] **Step 1: Añadir la dependencia**

En `crates/platform/Cargo.toml`, bajo `[target.'cfg(windows)'.dependencies]` (junto a
`windows`), añadir:

```toml
# Hotkey global del sistema (RegisterHotKey). MIT, mismos autores que tray-icon.
global-hotkey = "0.6"
```

(Verificar la última versión 0.x disponible; si `0.6` no existe, usar la más reciente y
ajustar la API según su changelog. El patrón de la API — `GlobalHotKeyManager::new()`,
`register(HotKey)`, `GlobalHotKeyEvent::receiver()` — es estable entre versiones 0.x.)

- [ ] **Step 2: Crear `crates/platform/src/global_hotkey.rs`**

Con el header Naygo (copiar el formato de `autostart.rs`) y:

```rust
//! Hotkey global del sistema para mostrar/ocultar Naygo. Envuelve la crate `global-hotkey`
//! (RegisterHotKey vía Win32). Tolerante: `register` devuelve `Result`; si el SO rechaza la
//! combinación (reservada / en uso), el llamador lo maneja. El manager mantiene VIVO el registro
//! (drop = se libera el hotkey), análogo a cómo `Tray` mantiene vivo el ícono.

use naygo_core::keymap::{Chord, KeyCode};

/// Un hotkey global registrado. Mantenerlo vivo mantiene el registro; al dropearlo, el hotkey
/// se libera. `None`/error si no se pudo registrar.
#[cfg(windows)]
pub struct GlobalHotkey {
    _manager: global_hotkey::GlobalHotKeyManager,
    id: u32,
}

#[cfg(windows)]
impl GlobalHotkey {
    /// El id del hotkey registrado (para reconocer sus eventos en el receptor).
    pub fn id(&self) -> u32 {
        self.id
    }
}

/// Traduce un `Chord` de Naygo a un `HotKey` de la crate `global-hotkey`. Devuelve `None` si el
/// `Chord` no es representable (p. ej. sin modificadores — un hotkey global de una sola tecla es
/// inaceptable; la UI ya lo previene, esto es defensa).
#[cfg(windows)]
fn chord_to_hotkey(chord: &Chord) -> Option<global_hotkey::hotkey::HotKey> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};

    // Exigir al menos un modificador.
    if !(chord.ctrl || chord.alt || chord.shift) {
        return None;
    }
    let mut mods = Modifiers::empty();
    if chord.ctrl {
        mods |= Modifiers::CONTROL;
    }
    if chord.alt {
        mods |= Modifiers::ALT;
    }
    if chord.shift {
        mods |= Modifiers::SHIFT;
    }
    // Traducir la tecla. Solo letras/dígitos y F1..F6 tienen sentido para un hotkey global;
    // el resto → None (no registrable).
    let code = match chord.key {
        KeyCode::Char(c) => match c.to_ascii_lowercase() {
            'a' => Code::KeyA, 'b' => Code::KeyB, 'c' => Code::KeyC, 'd' => Code::KeyD,
            'e' => Code::KeyE, 'f' => Code::KeyF, 'g' => Code::KeyG, 'h' => Code::KeyH,
            'i' => Code::KeyI, 'j' => Code::KeyJ, 'k' => Code::KeyK, 'l' => Code::KeyL,
            'm' => Code::KeyM, 'n' => Code::KeyN, 'o' => Code::KeyO, 'p' => Code::KeyP,
            'q' => Code::KeyQ, 'r' => Code::KeyR, 's' => Code::KeyS, 't' => Code::KeyT,
            'u' => Code::KeyU, 'v' => Code::KeyV, 'w' => Code::KeyW, 'x' => Code::KeyX,
            'y' => Code::KeyY, 'z' => Code::KeyZ,
            '0' => Code::Digit0, '1' => Code::Digit1, '2' => Code::Digit2, '3' => Code::Digit3,
            '4' => Code::Digit4, '5' => Code::Digit5, '6' => Code::Digit6, '7' => Code::Digit7,
            '8' => Code::Digit8, '9' => Code::Digit9,
            _ => return None,
        },
        KeyCode::F1 => Code::F1,
        KeyCode::F2 => Code::F2,
        KeyCode::F3 => Code::F3,
        KeyCode::F4 => Code::F4,
        KeyCode::F5 => Code::F5,
        KeyCode::F6 => Code::F6,
        _ => return None,
    };
    Some(HotKey::new(Some(mods), code))
}

/// Registra el `chord` como hotkey global. `Ok(GlobalHotkey)` si el SO lo aceptó; `Err(msg)` si
/// la combinación no es representable o el SO la rechazó (reservada / en uso). Nunca panic.
#[cfg(windows)]
pub fn register(chord: &Chord) -> Result<GlobalHotkey, String> {
    let hotkey = chord_to_hotkey(chord)
        .ok_or_else(|| "combinación no válida para un atajo global".to_string())?;
    let manager = global_hotkey::GlobalHotKeyManager::new()
        .map_err(|e| format!("no se pudo iniciar el gestor de atajos globales: {e}"))?;
    manager
        .register(hotkey)
        .map_err(|e| format!("el sistema rechazó la combinación (¿en uso?): {e}"))?;
    Ok(GlobalHotkey {
        _manager: manager,
        id: hotkey.id(),
    })
}

/// ¿Llegó un evento de PRESIÓN del hotkey con `id`? No bloquea: drena el receptor global de la
/// crate. Devuelve `true` si el hotkey `id` fue presionado desde la última llamada. Lo llama el
/// tick de la UI (igual que drena el canal del tray).
#[cfg(windows)]
pub fn was_pressed(id: u32) -> bool {
    use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
    let mut pressed = false;
    while let Ok(ev) = GlobalHotKeyEvent::receiver().try_recv() {
        if ev.id == id && ev.state == HotKeyState::Pressed {
            pressed = true;
        }
    }
    pressed
}

// --- Stubs no-Windows (deja el punto único a reimplementar para Linux) ---

#[cfg(not(windows))]
pub struct GlobalHotkey;

#[cfg(not(windows))]
impl GlobalHotkey {
    pub fn id(&self) -> u32 {
        0
    }
}

#[cfg(not(windows))]
pub fn register(_chord: &Chord) -> Result<GlobalHotkey, String> {
    Err("atajo global solo soportado en Windows".to_string())
}

#[cfg(not(windows))]
pub fn was_pressed(_id: u32) -> bool {
    false
}
```

- [ ] **Step 3: Declarar el módulo**

En `crates/platform/src/lib.rs`, junto a los otros `pub mod`, añadir:

```rust
pub mod global_hotkey;
```

- [ ] **Step 4: Compilar**

Run: `cargo build -p naygo-platform 2>&1 | tail -20`
Expected: compila. Si falla por la API de `global-hotkey` (nombres de `Code`/`Modifiers`/
`HotKeyState` distintos en la versión resuelta), ajustar a lo que exponga esa versión —
mirar `cargo doc -p global-hotkey --open` o el error concreto. La estructura
(manager+register+receiver+event con `.id`/`.state`) es estable.

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy -p naygo-platform --all-targets 2>&1 | tail -8`
Expected: sin warnings.

```bash
cargo fmt -p naygo-platform
git add crates/platform/Cargo.toml crates/platform/src/global_hotkey.rs crates/platform/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(platform): registro de hotkey global vía RegisterHotKey (crate global-hotkey)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 3 — Integración en la UI (registro + toggle)

### Task 3: Registrar al arrancar + drenar el evento + toggle mostrar/ocultar

**Files:**
- Modify: `crates/ui-slint/src/main.rs` (registrar tras crear el ctrl; drenar en el tick; toggle)

- [ ] **Step 1: Registrar el hotkey al arrancar**

En `main.rs`, tras crear el `ctrl` (~línea 232) y tras la sincronización de autostart,
registrar el hotkey si está activo. El registro debe mantenerse VIVO durante toda la
ejecución (guardarlo en una variable que viva hasta el final de `main`, como `tray`):

```rust
    // Hotkey global (mostrar/ocultar Naygo desde cualquier app). Se mantiene vivo hasta el final.
    #[cfg(windows)]
    let global_hotkey: Option<naygo_platform::global_hotkey::GlobalHotkey> = {
        let s = &ctrl.borrow().config.settings;
        if s.global_hotkey_enabled {
            match naygo_platform::global_hotkey::register(&s.global_hotkey) {
                Ok(h) => {
                    logging::log_line("hotkey global registrado");
                    Some(h)
                }
                Err(e) => {
                    // Al arrancar, un fallo NO muestra modal (para no molestar en cada arranque):
                    // se loguea y el hotkey queda inactivo. El usuario puede recapturar en Config.
                    logging::log_line(&format!("no se pudo registrar el hotkey global: {e}"));
                    None
                }
            }
        } else {
            None
        }
    };
    #[cfg(not(windows))]
    let global_hotkey: Option<naygo_platform::global_hotkey::GlobalHotkey> = None;
```

- [ ] **Step 2: Drenar el evento del hotkey en el tick, junto al tray**

En el tick del event loop, donde se drena el canal del tray (~línea 1445,
`while let Ok(msg) = t.rx.try_recv()`), añadir DESPUÉS de ese bloque el chequeo del hotkey.
Necesita el `id` del hotkey y el `ui_weak`/`ctrl`/`tray_active` que ya están en ese scope:

```rust
        // Hotkey global: ¿se presionó? Alterna mostrar/ocultar Naygo.
        #[cfg(windows)]
        if let Some(h) = global_hotkey.as_ref() {
            if naygo_platform::global_hotkey::was_pressed(h.id()) {
                if let Some(ui) = ui_weak.upgrade() {
                    toggle_window_visibility(&ui, tray_active);
                }
            }
        }
```

(Si `global_hotkey` no está en el scope del closure del tick, clonar lo necesario al
crearlo: `global_hotkey` es `Option<GlobalHotkey>` que no es `Clone`; en su lugar, extraer
el `id` (`Option<u32>`) ANTES del closure y capturar ese `u32` por copia, y mantener el
`GlobalHotkey` vivo en el scope de `main`. Es decir: `let hotkey_id: Option<u32> =
global_hotkey.as_ref().map(|h| h.id());` y en el tick usar `hotkey_id` + `was_pressed(id)`.)

- [ ] **Step 3: Implementar el helper `toggle_window_visibility`**

Como función libre en `main.rs` (junto a `naygo_hwnd`, ~línea 5712):

```rust
/// Alterna la visibilidad de Naygo para el hotkey global: si Naygo NO es la ventana activa
/// (oculta, minimizada o detrás) la muestra + trae al frente + enfoca; si YA es la ventana
/// activa, la esconde a la bandeja (solo si el tray está activo — si no, no esconde, para no
/// dejar la app inalcanzable). Coherente con `should_quit_on_close`.
#[cfg(windows)]
fn toggle_window_visibility(ui: &AppWindow, tray_active: bool) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let Some(hwnd) = naygo_hwnd(ui) else {
        // Sin HWND resoluble: al menos mostrar.
        let _ = ui.show();
        return;
    };
    let is_foreground = unsafe { GetForegroundWindow() } == HWND(hwnd as *mut _);
    if is_foreground && tray_active {
        // Naygo está al frente y hay bandeja → esconder a la bandeja.
        let _ = ui.window().hide();
    } else {
        // Oculta / detrás / minimizada → mostrar + des-minimizar + traer al frente.
        let _ = ui.show();
        ui.window().set_minimized(false);
        naygo_platform::window::bring_to_front(hwnd);
    }
}
#[cfg(not(windows))]
fn toggle_window_visibility(ui: &AppWindow, _tray_active: bool) {
    let _ = ui.show();
}
```

IMPORTANTE sobre `ui.window().hide()`: en Slint, `hide()` decrementa el contador de ventanas
visibles y puede llamar `quit_event_loop()` si llega a 0 (documentado en la Fase 5 del lote
anterior). Aquí es SEGURO porque el propósito de "esconder a bandeja" es exactamente el que ya
usa el `on_close_requested` con `CloseRequestResponse::HideWindow`. VERIFICAR en implementación
que esconder por esta vía deja el proceso vivo (el tray sigue). Si `hide()` matara el loop,
usar `set_minimized(true)` como en el arranque-minimizado, o replicar el camino exacto de
`on_close_requested`. Anotar el mecanismo elegido.

- [ ] **Step 4: Compilar**

Run: `CARGO_BUILD_JOBS=2 cargo build -p naygo-ui-slint 2>&1 | tail -15`
Expected: compila. Vigilar el borrow de `ctrl` al leer settings para registrar (scope corto).

- [ ] **Step 5: Commit**

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/src/main.rs
git commit -m "$(cat <<'EOF'
feat(ui): registrar el hotkey global y alternar mostrar/ocultar Naygo

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 4 — Configuración en la UI

### Task 4: Setters en ConfigCtrl + re-registro + validación

**Files:**
- Modify: `crates/ui-slint/src/config_ctrl.rs` (setters)
- Modify: `crates/ui-slint/src/main.rs` (re-registrar tras cambiar la config; handlers)

- [ ] **Step 1: Setters en `ConfigCtrl`**

En `crates/ui-slint/src/config_ctrl.rs`, junto a los otros setters (p. ej.
`set_autostart`), añadir. Devuelven `Result<(), String>` para que la UI muestre el aviso si
el SO rechaza:

```rust
    /// Activa/desactiva el hotkey global y persiste. El re-registro real lo hace la UI (main),
    /// que tiene el `GlobalHotkey` vivo; aquí solo se persiste el flag.
    pub fn set_global_hotkey_enabled(&mut self, on: bool) {
        self.settings.global_hotkey_enabled = on;
        self.save();
    }

    /// Cambia la combinación del hotkey global y persiste. Valida que tenga ≥1 modificador
    /// (un hotkey global de una sola tecla es inaceptable). Devuelve Err si la combinación no
    /// es válida; el re-registro real (y el aviso de rechazo del SO) lo maneja la UI.
    pub fn set_global_hotkey(&mut self, chord: Chord) -> Result<(), String> {
        if !(chord.ctrl || chord.alt || chord.shift) {
            return Err("el atajo global necesita al menos un modificador (Ctrl/Alt/Shift)".into());
        }
        self.settings.global_hotkey = chord;
        self.save();
        Ok(())
    }
```

(Verificar que `Chord` está importado en `config_ctrl.rs`; si no, `use naygo_core::keymap::Chord;`.)

- [ ] **Step 2: Re-registro en caliente desde main**

El `GlobalHotkey` vivo está en `main`. Para re-registrar tras un cambio de config, el
patrón más simple: el hotkey vive en un `Rc<RefCell<Option<GlobalHotkey>>>` (en vez de un
`let` inmóvil), y un helper `rearm_global_hotkey(&ctrl, &slot)` que: lee la config actual,
dropea el registro viejo (soltando el hotkey), y si está activo re-registra. Se llama (a) al
arrancar y (b) desde los handlers de config. Reemplazar el `let global_hotkey` de la Task 3
Step 1 por:

```rust
    let global_hotkey_slot: Rc<RefCell<Option<naygo_platform::global_hotkey::GlobalHotkey>>> =
        Rc::new(RefCell::new(None));
    let hotkey_id: Rc<std::cell::Cell<Option<u32>>> = Rc::new(std::cell::Cell::new(None));

    // Helper: (re)registra el hotkey global según la config actual. Devuelve Err si el SO lo
    // rechaza (para que el llamador muestre el aviso). Al arrancar, el llamador ignora el Err
    // (solo loguea). Al cambiar en config, el llamador muestra el modal.
    let rearm_hotkey = {
        let ctrl = ctrl.clone();
        let slot = global_hotkey_slot.clone();
        let id_cell = hotkey_id.clone();
        move || -> Result<(), String> {
            // Soltar el registro anterior primero.
            *slot.borrow_mut() = None;
            id_cell.set(None);
            let s = ctrl.borrow().config.settings.clone();
            if !s.global_hotkey_enabled {
                return Ok(());
            }
            match naygo_platform::global_hotkey::register(&s.global_hotkey) {
                Ok(h) => {
                    id_cell.set(Some(h.id()));
                    *slot.borrow_mut() = Some(h);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
    };

    // Registro inicial (un fallo solo se loguea).
    #[cfg(windows)]
    if let Err(e) = rearm_hotkey() {
        logging::log_line(&format!("no se pudo registrar el hotkey global: {e}"));
    }
```

En el tick, el chequeo usa `hotkey_id.get()`:

```rust
        #[cfg(windows)]
        if let Some(id) = hotkey_id.get() {
            if naygo_platform::global_hotkey::was_pressed(id) {
                if let Some(ui) = ui_weak.upgrade() {
                    toggle_window_visibility(&ui, tray_active);
                }
            }
        }
```

(`Settings` debe derivar `Clone` para `.clone()` — ya lo hace.)

- [ ] **Step 3: Handlers de config cableados**

Añadir los callbacks Slint y sus handlers en `main.rs`. Al activar/desactivar o cambiar la
combinación: persistir vía el setter, re-armar el hotkey, y si `rearm_hotkey()` devuelve Err,
mostrar el aviso con `MessageModal` (el mismo que usan expulsar-USB/errores import-export) y
revertir el toggle a apagado (para no quedar "activado sin efecto"). Ejemplo del handler del
toggle:

```rust
        ui.on_set_global_hotkey_enabled(move |on| {
            ctrl.borrow_mut().config.set_global_hotkey_enabled(on);
            if on {
                if let Err(e) = rearm_hotkey() {
                    // Falló el registro: avisar y revertir a apagado.
                    ctrl.borrow_mut().config.set_global_hotkey_enabled(false);
                    // Mostrar MessageModal con el texto de "no se pudo registrar" + e.
                    /* usar el mecanismo de MessageModal ya existente */
                }
            } else {
                let _ = rearm_hotkey(); // apagar = soltar el registro (nunca falla)
            }
            // refrescar el VM de config para reflejar el estado real
        });
```

(El handler de la captura de combinación llama `set_global_hotkey(chord)` y luego
`rearm_hotkey()`, con el mismo manejo de Err. Reusar el flujo de captura del editor de atajos
existente para obtener el `Chord` desde las teclas presionadas.)

- [ ] **Step 4: UI en config-window.slint**

Añadir en la sección Integración (junto a bandeja/autostart):
- Un `Field` con `Switch` para `global_hotkey_enabled` → callback `set-global-hotkey-enabled(bool)`.
- Un campo de captura de combinación (reusar el componente del editor de atajos del toolbar)
  que muestre el chord actual vía `ConfigCtrl::chord_to_text(&settings.global_hotkey)` y, al
  capturar, llame `set-global-hotkey(...)`. Habilitado solo si el toggle está activo.
- El VM de config (types.slint) gana `global-hotkey-enabled: bool` y `global-hotkey-text: string`;
  poblarlos en el builder del VM desde settings (`chord_to_text`).

- [ ] **Step 5: i18n en los 10 idiomas**

Añadir a `crates/core/src/i18n/*.json` (JSON planos) las claves:
`slint.cfg.global_hotkey` (label) y `slint.cfg.tip.global_hotkey` (tooltip), traducidas a los
10 idiomas (no dejar EN donde haya traducción). Textos:
- ES label: "Atajo global para abrir Naygo"
- ES tip: "Una combinación de teclas que muestra u oculta Naygo desde cualquier aplicación. Requiere el ícono de bandeja. La tecla Windows no se puede usar."
- EN label: "Global shortcut to open Naygo"
- EN tip: "A key combination that shows or hides Naygo from any application. Requires the tray icon. The Windows key cannot be used."
Cablear en `i18n_keys.rs` (`set_cfg_global_hotkey` / `set_cfg_tip_global_hotkey`), siguiendo
el patrón de `set_cfg_close_to_tray`.

- [ ] **Step 6: Compilar + tests + commit**

Run: `CARGO_BUILD_JOBS=2 cargo build -p naygo-ui-slint 2>&1 | tail -12`
Expected: compila.
Run: `cargo test -p naygo-ui-slint 2>&1 | grep -E "test result:" | head -2`
Expected: verde.
Run: `python -c "import json; [json.load(open(f'crates/core/src/i18n/{f}.json',encoding='utf-8')) for f in ['de','en','es','fr','hi','it','ja','ko','pt','zh']]; print('JSON OK')"`
Expected: JSON OK.

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint crates/core/src/i18n
git commit -m "$(cat <<'EOF'
feat(ui): configurar el hotkey global en Config (toggle + captura + aviso de rechazo) + i18n

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 5 — Integración final

### Task 5: Suite completa + regenerar dist

- [ ] **Step 1: Suite completa**

Run: `cargo test 2>&1 | grep -E "test result:|FAILED" | head -12`
Expected: todo verde.

- [ ] **Step 2: Clippy total**

Run: `cargo clippy --all-targets 2>&1 | tail -12`
Expected: sin warnings.

- [ ] **Step 3: Regenerar dist**

Pre-compilar release con Bash (el script `.ps1` aborta por el stderr de cargo bajo PS 5.1):
```bash
cargo build --release
```
Luego el empaquetado vía Bash:
```bash
powershell.exe -ExecutionPolicy Bypass -File scripts/build-release.ps1
```
Expected: portable + instalador en `dist/`.

---

## Verificación visual pendiente (Nicolás, en la VM)

Estos puntos requieren la VM (los tests no los cubren):

- [ ] Con Naygo minimizado/en bandeja: presionar Ctrl+Alt+Q desde otra app → Naygo aparece al frente.
- [ ] Con Naygo al frente: presionar Ctrl+Alt+Q → Naygo se esconde a la bandeja.
- [ ] Con Naygo detrás de otra ventana: Ctrl+Alt+Q → lo trae al frente (no lo esconde).
- [ ] Config: cambiar la combinación funciona; el hotkey nuevo responde.
- [ ] Config: intentar una combinación reservada (p. ej. Ctrl+Alt+Del no debería capturarse; probar una en uso) → muestra el aviso y no queda "activado sin efecto".
- [ ] Config: apagar el toggle → el hotkey deja de responder.

## Notas de cobertura (self-review)

- Spec §"core": Fase 1 (Chord::ctrl_alt + campos + defaults). ✓
- Spec §"platform": Fase 2 (global_hotkey.rs + register/was_pressed + stubs no-Windows). ✓
- Spec §"ui-slint": Fase 3 (registro + toggle) + Fase 4 (config). ✓
- Spec §"Configuración": Fase 4 (toggle + captura + validación ≥1 modificador). ✓
- Spec §"Manejo del rechazo del SO": modal al configurar (Task 4 Step 3) + solo-log al arrancar
  (Task 3 Step 1 / Task 4 Step 2). ✓
- Spec §"Testing": tests de core (Fase 1), sin tests platform (VM), test config (Fase 4),
  verificación visual VM (arriba). ✓
- Spec §"no tecla Win": `chord_to_hotkey` no mapea la tecla Win (no existe en `Chord`); la UI no
  la ofrece. ✓
