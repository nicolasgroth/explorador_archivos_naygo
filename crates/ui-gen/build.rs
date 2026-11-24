// Naygo — compila los .slint de ui/ a Rust (lo incluye src/lib.rs con `include_modules!`).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

fn main() {
    // El compilador de la UI declarativa supera el stack principal de 1 MiB de MSVC.
    // Esta reserva sólo existe durante el build; no aumenta el stack/RAM de Naygo.
    std::thread::Builder::new()
        .name("slint-compiler".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| slint_build::compile("ui/app-window.slint").expect("compilar app-window.slint"))
        .expect("iniciar compilador Slint")
        .join()
        .expect("compilador Slint");
    // Solo ui/ invalida este build script (slint-build además declara cada .slint que leyó).
    println!("cargo:rerun-if-changed=ui");
}
