// Naygo — compila los .slint de la capa UI Slint.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
fn main() {
    slint_build::compile("ui/app-window.slint").expect("compilar app-window.slint");
}
