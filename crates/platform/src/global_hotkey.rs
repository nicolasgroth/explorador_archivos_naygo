// Naygo — hotkey global del sistema para mostrar/ocultar la ventana (RegisterHotKey).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Hotkey global del sistema para mostrar/ocultar Naygo. Envuelve la crate `global-hotkey`
//! (RegisterHotKey vía Win32). Tolerante: `register` devuelve `Result`; si el SO rechaza la
//! combinación (reservada / en uso), el llamador lo maneja. El manager mantiene VIVO el registro
//! (drop = se libera el hotkey), análogo a cómo `Tray` mantiene vivo el ícono.
//!
//! Entrega de pulsaciones por HANDLER, no por polling: antes `was_pressed` drenaba
//! `GlobalHotKeyEvent::receiver()` desde el tick de la UI, pero si el loop de Slint está DORMIDO
//! (reposo / bajo consumo) nadie llama al tick y la pulsación quedaba en el canal hasta el
//! próximo wake por otra causa. Ahora `install_wake_handler` instala un callback que corre EN
//! CALIENTE (fuera del loop de UI), marca un flag atómico y despierta la UI con el `waker` —
//! el mismo patrón que el tray (`ui-slint/src/tray.rs`). El tick solo consulta el flag.

use naygo_core::keymap::{Chord, KeyCode};
use std::sync::atomic::{AtomicBool, Ordering};

/// Flag de "hubo una pulsación pendiente". Lo ESCRIBE el handler global (hilo del hook de
/// `global-hotkey`) y lo CONSUME el tick de la UI vía `was_pressed`. Atómico porque handler y
/// tick corren en hilos distintos; `SeqCst` por simplicidad (una escritura por pulsación humana,
/// el costo es irrelevante).
static PRESSED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
pub struct GlobalHotkey {
    _manager: global_hotkey::GlobalHotKeyManager,
    /// Id del hotkey registrado. Ya NADIE lo consulta para filtrar eventos (ver
    /// `install_wake_handler`); se conserva SOLO como diagnóstico (loguear qué registro quedó
    /// vivo al re-registrar tras un cambio de combinación en Config).
    id: u32,
}

#[cfg(windows)]
impl GlobalHotkey {
    /// Id del registro, solo para diagnóstico/logging. No se usa para filtrar eventos.
    pub fn id(&self) -> u32 {
        self.id
    }
}

/// Traduce un `Chord` de Naygo a un `HotKey`. `None` si no es representable (sin modificadores,
/// o tecla no soportada). Exige ≥1 modificador (un hotkey global de una sola tecla es inaceptable).
/// Cobertura: letras a-z (case-insensitive), dígitos 0-9 y F1-F6 (los `KeyCode` de función que
/// existen en el keymap de Naygo).
#[cfg(windows)]
fn chord_to_hotkey(chord: &Chord) -> Option<global_hotkey::hotkey::HotKey> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};

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
    let code = match chord.key {
        KeyCode::Char(c) => match c.to_ascii_lowercase() {
            'a' => Code::KeyA,
            'b' => Code::KeyB,
            'c' => Code::KeyC,
            'd' => Code::KeyD,
            'e' => Code::KeyE,
            'f' => Code::KeyF,
            'g' => Code::KeyG,
            'h' => Code::KeyH,
            'i' => Code::KeyI,
            'j' => Code::KeyJ,
            'k' => Code::KeyK,
            'l' => Code::KeyL,
            'm' => Code::KeyM,
            'n' => Code::KeyN,
            'o' => Code::KeyO,
            'p' => Code::KeyP,
            'q' => Code::KeyQ,
            'r' => Code::KeyR,
            's' => Code::KeyS,
            't' => Code::KeyT,
            'u' => Code::KeyU,
            'v' => Code::KeyV,
            'w' => Code::KeyW,
            'x' => Code::KeyX,
            'y' => Code::KeyY,
            'z' => Code::KeyZ,
            '0' => Code::Digit0,
            '1' => Code::Digit1,
            '2' => Code::Digit2,
            '3' => Code::Digit3,
            '4' => Code::Digit4,
            '5' => Code::Digit5,
            '6' => Code::Digit6,
            '7' => Code::Digit7,
            '8' => Code::Digit8,
            '9' => Code::Digit9,
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

/// Registra el `chord` como hotkey global. `Err` si no es representable o el SO lo rechazó.
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

/// Instala el handler global de pulsaciones: cada `Pressed` marca el flag interno y llama
/// `waker()` para despertar el loop de UI (mismo patrón que el tray). Llamar UNA vez al arranque,
/// después del primer `register()`. El handler NO filtra por id: Naygo registra un único atajo
/// global, y al re-registrar (cambio de combinación en Config) el id cambia pero el handler
/// sigue siendo válido sin re-instalarse.
///
/// OJO (contrato de `global-hotkey`): con un handler instalado, `GlobalHotKeyEvent::receiver()`
/// DEJA de recibir eventos — los dos mecanismos no se pueden mezclar. Por eso `was_pressed`
/// consulta solo el flag y ya no toca el receiver: el handler es el único consumidor de eventos.
/// Preferimos handler sobre polling porque el polling depende de que el tick de la UI corra, y
/// en bajo consumo la UI duerme: la pulsación se perdería hasta el próximo wake por otra causa.
#[cfg(windows)]
pub fn install_wake_handler(waker: crate::dir_watch::Waker) {
    use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
    // El closure corre en el hilo del hook de `global-hotkey`, NO en el de la UI: solo marca el
    // flag y despierta; el trabajo real (mostrar la ventana) lo hace el tick al ver el flag.
    GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
        if ev.state == HotKeyState::Pressed {
            PRESSED.store(true, Ordering::SeqCst);
            waker();
        }
    }));
}

/// ¿Hubo una pulsación desde la última consulta? Consume el flag (swap a `false`).
/// No bloquea ni toca el receiver de `global-hotkey` (ver `install_wake_handler`): con el
/// handler instalado, el flag es la única fuente de verdad. Varias pulsaciones entre consultas
/// colapsan en un solo `true` — correcto para "mostrar la ventana al frente" (es idempotente).
pub fn was_pressed() -> bool {
    PRESSED.swap(false, Ordering::SeqCst)
}

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

/// Sin soporte fuera de Windows: no hay eventos que entreguen, el handler es un no-op y
/// `was_pressed` (compartido) siempre verá el flag en `false`.
#[cfg(not(windows))]
pub fn install_wake_handler(_waker: crate::dir_watch::Waker) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// `was_pressed` consume el flag: `true` UNA vez tras marcarlo, `false` después.
    /// Único test que toca `PRESSED` (es un static de proceso; si otro test lo tocara en
    /// paralelo habría carrera entre tests — mantenerlo así).
    #[test]
    fn was_pressed_consume_el_flag() {
        // Estado inicial limpio (nadie lo marcó todavía en este proceso de test).
        assert!(!was_pressed());
        // Simula lo que hace el handler al recibir `Pressed`.
        PRESSED.store(true, Ordering::SeqCst);
        assert!(was_pressed(), "la primera consulta debe ver la pulsación");
        assert!(
            !was_pressed(),
            "la segunda consulta ya no: el flag se consumió"
        );
    }

    #[cfg(windows)]
    mod windows_only {
        use super::super::chord_to_hotkey;
        use global_hotkey::hotkey::{Code, Modifiers};
        use naygo_core::keymap::{Chord, KeyCode};

        fn ctrl_alt(key: KeyCode) -> Chord {
            Chord {
                key,
                ctrl: true,
                shift: false,
                alt: true,
            }
        }

        /// El default nuevo de Naygo (Ctrl+Alt+Z) debe mapear a KeyZ con CONTROL|ALT.
        #[test]
        fn ctrl_alt_z_mapea_a_keyz() {
            let hk = chord_to_hotkey(&ctrl_alt(KeyCode::Char('z'))).expect("debe ser mapeable");
            assert_eq!(hk.key, Code::KeyZ);
            assert_eq!(hk.mods, Modifiers::CONTROL | Modifiers::ALT);
            // Mayúscula equivale (to_ascii_lowercase): mismo hotkey.
            let hk_upper =
                chord_to_hotkey(&ctrl_alt(KeyCode::Char('Z'))).expect("debe ser mapeable");
            assert_eq!(hk_upper, hk);
        }

        /// TODAS las letras a-z y dígitos 0-9 son mapeables (nada de letras sueltas en el match).
        #[test]
        fn cubre_todas_las_letras_y_digitos() {
            for c in ('a'..='z').chain('0'..='9') {
                assert!(
                    chord_to_hotkey(&ctrl_alt(KeyCode::Char(c))).is_some(),
                    "el carácter '{c}' debería ser mapeable"
                );
            }
        }

        /// F1-F6 (las F-keys que existen en el keymap) siguen mapeando.
        #[test]
        fn cubre_f_keys() {
            for key in [
                KeyCode::F1,
                KeyCode::F2,
                KeyCode::F3,
                KeyCode::F4,
                KeyCode::F5,
                KeyCode::F6,
            ] {
                assert!(chord_to_hotkey(&ctrl_alt(key)).is_some());
            }
            assert_eq!(
                chord_to_hotkey(&ctrl_alt(KeyCode::F6)).unwrap().key,
                Code::F6
            );
        }

        /// Sin modificadores se rechaza (un hotkey global de una sola tecla es inaceptable),
        /// igual que una tecla fuera del set soportado.
        #[test]
        fn rechaza_sin_modificadores_y_teclas_no_soportadas() {
            assert!(chord_to_hotkey(&Chord::plain(KeyCode::Char('q'))).is_none());
            assert!(chord_to_hotkey(&ctrl_alt(KeyCode::Enter)).is_none());
            assert!(chord_to_hotkey(&ctrl_alt(KeyCode::Char('ñ'))).is_none());
        }
    }
}
