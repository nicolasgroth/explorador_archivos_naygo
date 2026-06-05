// Naygo — núcleo: lógica pura de filesystem, sin UI ni Windows.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `naygo-core` contiene toda la lógica testeable del explorador: modelo de
//! filesystem, motor de listado por streaming, ordenamiento y cancelación.
//! No depende de egui ni de Windows.

pub mod cancel;
pub mod fs_model;
pub mod sort;

pub use cancel::CancellationToken;
pub use fs_model::{Entry, EntryKind, PaneState, SortKey, SortSpec, ViewMode};
pub use sort::sort_entries;
