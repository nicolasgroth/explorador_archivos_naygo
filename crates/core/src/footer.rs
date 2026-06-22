// Naygo — footer por panel: campos, plantillas y render de la barra inferior.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA del footer de cada panel (sin UI ni Windows). La UI calcula los datos
//! crudos por panel (`FooterData`) y `render` produce el string final según la plantilla.
//! Nunca falla: datos ausentes (disco no disponible) se muestran como `—`.

use crate::disk::DiskUsage;
use serde::{Deserialize, Serialize};

/// Plantilla del footer. `Custom` lleva el template de tokens del usuario.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FooterPreset {
    /// "{sel}/{total} · {marked}"
    #[default]
    Compact,
    /// "{sel} de {total} sel · {marked} · {free} libres / {disk_total} ({pct})"
    Full,
    /// "{free} libres / {disk_total} ({pct})"
    DiskOnly,
    /// "{sel} de {total} sel · {marked}"
    SelectionOnly,
    /// Plantilla libre del usuario (con tokens).
    Custom(String),
}

impl FooterPreset {
    /// El template string asociado al preset (para Custom, el del usuario).
    pub fn template(&self) -> &str {
        match self {
            FooterPreset::Compact => "{sel}/{total} · {marked}",
            FooterPreset::Full => {
                "{sel} de {total} sel · {marked} · {free} libres / {disk_total} ({pct})"
            }
            FooterPreset::DiskOnly => "{free} libres / {disk_total} ({pct})",
            FooterPreset::SelectionOnly => "{sel} de {total} sel · {marked}",
            FooterPreset::Custom(t) => t.as_str(),
        }
    }
}

/// Datos crudos que la UI calcula por panel y pasa a `render`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FooterData {
    pub sel_count: usize,
    pub total_count: usize,
    pub marked_bytes: u64,
    /// `None` = disco no disponible (red caída, panel especial).
    pub disk: Option<DiskUsage>,
    pub item_count: usize,
    pub file_count: usize,
    pub dir_count: usize,
}
