// Naygo — footer por panel: campos, plantillas y render de la barra inferior.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Lógica PURA del footer de cada panel (sin UI ni Windows). La UI calcula los datos
//! crudos por panel (`FooterData`) y `render` produce el string final según la plantilla.
//! Nunca falla: datos ausentes (disco no disponible) se muestran como `—`.

use crate::disk::DiskUsage;
use crate::format::{format_size, SizeFormat};
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

/// Renderiza el footer sustituyendo tokens en el template del preset. Nunca falla.
/// Tokens de disco con `disk == None` → `—`. Tokens desconocidos → se dejan literales.
pub fn render(preset: &FooterPreset, data: &FooterData, size_fmt: SizeFormat) -> String {
    let dash = "—";
    let (free, disk_total, pct) = match &data.disk {
        Some(u) => (
            format_size(u.free, size_fmt),
            format_size(u.total, size_fmt),
            format!("{}%", u.percent_used()),
        ),
        None => (dash.to_string(), dash.to_string(), dash.to_string()),
    };
    let marked = format_size(data.marked_bytes, size_fmt);

    let mut out = preset.template().to_string();
    let pairs: [(&str, String); 9] = [
        ("{sel}", data.sel_count.to_string()),
        ("{total}", data.total_count.to_string()),
        ("{marked}", marked),
        ("{free}", free),
        ("{disk_total}", disk_total),
        ("{pct}", pct),
        ("{items}", data.item_count.to_string()),
        ("{files}", data.file_count.to_string()),
        ("{dirs}", data.dir_count.to_string()),
    ];
    for (token, value) in pairs.iter() {
        out = out.replace(token, value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::DiskUsage;

    fn sample_with_disk() -> FooterData {
        FooterData {
            sel_count: 3,
            total_count: 12,
            marked_bytes: 4_404_019, // ~4,2 MB
            disk: Some(DiskUsage {
                total: 500_000_000_000,
                free: 112_000_000_000,
            }),
            item_count: 12,
            file_count: 8,
            dir_count: 4,
        }
    }

    #[test]
    fn compact_con_disco() {
        let s = render(
            &FooterPreset::Compact,
            &sample_with_disk(),
            SizeFormat::Auto,
        );
        assert!(s.contains("3/12"), "esperaba sel/total: {s}");
        assert!(s.contains("MB"), "esperaba bytes marcados: {s}");
    }

    #[test]
    fn full_incluye_disco_y_pct() {
        let s = render(&FooterPreset::Full, &sample_with_disk(), SizeFormat::Auto);
        assert!(s.contains("3 de 12"), "{s}");
        assert!(s.contains("libres"), "{s}");
        assert!(s.contains('%'), "esperaba el porcentaje: {s}");
    }

    #[test]
    fn disco_none_muestra_guion() {
        let mut d = sample_with_disk();
        d.disk = None;
        let s = render(&FooterPreset::DiskOnly, &d, SizeFormat::Auto);
        assert!(s.contains('—'), "disco ausente debe dar —: {s}");
    }

    #[test]
    fn custom_token_desconocido_queda_literal() {
        let p = FooterPreset::Custom("{sel} {desconocido} {dirs}".to_string());
        let s = render(&p, &sample_with_disk(), SizeFormat::Auto);
        assert!(
            s.contains("{desconocido}"),
            "token raro se deja literal: {s}"
        );
        assert!(s.contains("3"), "{s}");
        assert!(s.contains("4"), "dirs=4: {s}");
    }

    #[test]
    fn nunca_panica_con_total_cero() {
        let d = FooterData::default(); // todo 0, disco None
        let s = render(&FooterPreset::Full, &d, SizeFormat::Auto);
        assert!(!s.is_empty());
    }
}
