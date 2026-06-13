// Naygo — build script del prototipo Slint: compila ui/app.slint a código Rust.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

fn main() {
    slint_build::compile("ui/app.slint").expect("compilar app.slint");
}
