// Naygo — build script del prototipo Slint: compila ui/app.slint a código Rust.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

fn main() {
    slint_build::compile("ui/app.slint").expect("compilar app.slint");
}
