// Naygo — assets de íconos: ahora viven en naygo-core::icons (compartidos con la UI Slint).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! La tabla de PNGs embebidos se movió a `naygo_core::icons` para que la UI Slint
//! la comparta. Este módulo re-exporta la API pública para no romper la capa egui.

pub use naygo_core::icons::{all_keys, bytes_for, file_name};
