// Naygo — arranque de la capa UI en Slint (Fase 1).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.run()
}
