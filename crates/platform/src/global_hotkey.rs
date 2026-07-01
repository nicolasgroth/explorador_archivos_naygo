// Naygo — hotkey global del sistema para mostrar/ocultar la ventana (RegisterHotKey).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Hotkey global del sistema para mostrar/ocultar Naygo. Envuelve la crate `global-hotkey`
//! (RegisterHotKey vía Win32). Tolerante: `register` devuelve `Result`; si el SO rechaza la
//! combinación (reservada / en uso), el llamador lo maneja. El manager mantiene VIVO el registro
//! (drop = se libera el hotkey), análogo a cómo `Tray` mantiene vivo el ícono.

use naygo_core::keymap::{Chord, KeyCode};

#[cfg(windows)]
pub struct GlobalHotkey {
    _manager: global_hotkey::GlobalHotKeyManager,
    id: u32,
}

#[cfg(windows)]
impl GlobalHotkey {
    pub fn id(&self) -> u32 {
        self.id
    }
}

/// Traduce un `Chord` de Naygo a un `HotKey`. `None` si no es representable (sin modificadores,
/// o tecla no soportada). Exige ≥1 modificador (un hotkey global de una sola tecla es inaceptable).
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

/// ¿Llegó un evento de PRESIÓN del hotkey con `id`? No bloquea: drena el receptor global.
/// SUPUESTO de un solo hotkey: drena TODOS los eventos y descarta los de otros `id` (no los
/// re-encola). Naygo registra un único hotkey global, así que es correcto. Si en el futuro se
/// registraran varios y se sondearan por separado, el primer `was_pressed` se tragaría los
/// eventos de los demás — habría que cambiar a un demultiplexado por id.
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
