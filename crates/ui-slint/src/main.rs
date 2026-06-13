// Naygo — arranque de la capa UI en Slint (Fase 1).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
// NOTA: durante la construcción incremental de la Fase 1, algunos módulos (bridge, keys)
// se escriben y testean antes de cablearse en `main` (Tarea 5). Se permite dead_code
// transitoriamente; la Tarea 5 conecta todo y deja el crate sin código muerto.
#![allow(dead_code)]
mod bridge;
mod keys;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.run()
}
