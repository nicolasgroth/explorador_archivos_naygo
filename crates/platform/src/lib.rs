// Naygo — capa de plataforma: todo lo que toca Windows, aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `naygo-platform` aísla las llamadas a Win32 / Shell / COM tras interfaces
//! limpias. En la Fase 1 está vacío a propósito: la integración del Shell
//! (íconos, ShellExecute, papelera, discos) y el drag&drop COM llegan en fases
//! posteriores. Este crate existe ahora para fijar la frontera arquitectónica.

pub mod clipboard;
pub mod context_menu;
pub mod device_watch;
pub mod dir_watch;
pub mod dnd;
pub mod drive_space;
pub mod drives;
pub mod locale;
pub mod open;
pub mod trash;

pub fn hello() -> &'static str {
    "naygo-platform"
}
