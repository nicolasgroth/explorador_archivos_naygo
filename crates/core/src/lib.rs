// Naygo — núcleo: lógica pura de filesystem, sin UI ni Windows.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `naygo-core` contiene toda la lógica testeable del explorador: modelo de
//! filesystem, motor de listado por streaming, ordenamiento y cancelación.
//! No depende de egui ni de Windows.

pub mod cancel;
pub mod config;
pub mod fs_model;
pub mod i18n;
pub mod icon_kind;
pub mod listing;
pub mod sort;
pub mod workspace;

pub use cancel::CancellationToken;
pub use config::{BarPosition, IconSet, Settings};
pub use fs_model::{Entry, EntryKind, PaneState, SortKey, SortSpec, ViewMode};
pub use i18n::{I18n, LangId};
pub use icon_kind::{category_for_extension, icon_key_for, DriveKind, FileCategory, IconKey};
pub use listing::{spawn_listing, ListingMsg};
pub use sort::sort_entries;
pub use workspace::{
    FilePaneState, LayoutTemplate, NavHistory, PaneId, PaneNode, PanePurpose, TemplateStore,
    Workspace,
};
