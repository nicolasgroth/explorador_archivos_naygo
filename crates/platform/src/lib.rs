// Naygo — capa de plataforma: todo lo que toca Windows, aislado.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `naygo-platform` aísla las llamadas a Win32 / Shell / COM tras interfaces
//! limpias. En la Fase 1 está vacío a propósito: la integración del Shell
//! (íconos, ShellExecute, papelera, discos) y el drag&drop COM llegan en fases
//! posteriores. Este crate existe ahora para fijar la frontera arquitectónica.

pub mod autostart;
pub mod clipboard;
pub mod context_menu;
pub mod device_watch;
pub mod dir_watch;
pub mod dnd;
pub mod drive_space;
pub mod drives;
pub mod drop_target;
pub mod eject;
pub mod locale;
pub mod open;
pub mod time;
pub mod trash;
pub mod window;
pub mod window_geometry;

pub fn hello() -> &'static str {
    "naygo-platform"
}
