// Naygo — eventos nativos: AltGr del editor y despertar por visibilidad, sin filesystem.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use crate::AppWindow;
use slint::ComponentHandle;

fn printable_path_text(focused: bool, altgr: bool, text: Option<&str>) -> Option<&str> {
    text.filter(|text| focused && altgr && !text.is_empty() && !text.chars().any(char::is_control))
}

/// Slint 1.16 puede rechazar caracteres de AltGr por el Ctrl+Alt que entrega Windows.
/// Reenviamos sólo texto imprimible al editor enfocado, liberando temporalmente esos
/// modificadores. TextInput conserva inserción, selección y undo; jamás concatenamos texto.
pub(crate) fn wire(ui: &AppWindow) {
    use i_slint_backend_winit::{
        winit::{
            event::{ElementState, WindowEvent},
            keyboard::{KeyCode, PhysicalKey},
        },
        EventResult, WinitWindowAccessor,
    };
    use slint::platform::{Key, WindowEvent as Input};

    let weak = ui.as_weak();
    let mut held = [false; 4]; // ControlLeft, ControlRight, AltLeft, AltRight
    ui.window().on_winit_window_event(move |window, event| {
        if matches!(
            event,
            WindowEvent::Focused(_) | WindowEvent::Resized(_) | WindowEvent::Occluded(_)
        ) {
            // El tick puede estar dormido tras suspender el preview. Restaurar desde la
            // barra de tareas no pasa por un callback Slint de navegación: despertarlo
            // una vez permite consultar IsIconic/visibilidad real. Diferir evita reentrar
            // al adaptador winit mientras entrega el evento; no añade un reloj periódico.
            let wake = weak.clone();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                if let Some(ui) = wake.upgrade() {
                    ui.invoke_wake();
                }
            });
        }
        match event {
            WindowEvent::Focused(false) => held = [false; 4],
            WindowEvent::KeyboardInput { event, .. } => {
                let index = match event.physical_key {
                    PhysicalKey::Code(KeyCode::ControlLeft) => Some(0),
                    PhysicalKey::Code(KeyCode::ControlRight) => Some(1),
                    PhysicalKey::Code(KeyCode::AltLeft) => Some(2),
                    PhysicalKey::Code(KeyCode::AltRight) => Some(3),
                    _ => None,
                };
                if let Some(index) = index {
                    held[index] = event.state == ElementState::Pressed;
                }
                let focused = weak
                    .upgrade()
                    .is_some_and(|ui| ui.get_edit_pane() >= 0 && ui.get_edit_input_focused());
                if let Some(text) = printable_path_text(
                    focused && event.state == ElementState::Pressed,
                    held[3],
                    event.text.as_deref(),
                ) {
                    for key in [Key::Control, Key::ControlR, Key::Alt, Key::AltGr] {
                        window.dispatch_event(Input::KeyReleased { text: key.into() });
                    }
                    window.dispatch_event(if event.repeat {
                        Input::KeyPressRepeated { text: text.into() }
                    } else {
                        Input::KeyPressed { text: text.into() }
                    });
                    window.dispatch_event(Input::KeyReleased { text: text.into() });
                    for (key, down) in [Key::Control, Key::ControlR, Key::Alt, Key::AltGr]
                        .into_iter()
                        .zip(held)
                    {
                        if down {
                            window.dispatch_event(Input::KeyPressed { text: key.into() });
                        }
                    }
                    return EventResult::PreventDefault;
                }
            }
            _ => {}
        }
        EventResult::Propagate
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn altgr_is_scoped_to_printable_text_in_the_focused_path_editor() {
        assert_eq!(printable_path_text(true, true, Some("\\")), Some("\\"));
        assert_eq!(printable_path_text(true, true, Some("ñ")), Some("ñ"));
        assert_eq!(printable_path_text(false, true, Some("\\")), None);
        assert_eq!(printable_path_text(true, false, Some("c")), None);
        assert_eq!(printable_path_text(true, true, Some("\r")), None);
        assert_eq!(printable_path_text(true, true, Some("")), None);
    }
}
