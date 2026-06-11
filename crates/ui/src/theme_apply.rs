// Naygo — traducción de un Theme (core) a egui::Visuals y su aplicación al Context.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `core` define la paleta (datos); aquí se convierte a `egui::Visuals` y se aplica
//! en caliente. `ActiveTheme` guarda el tema resuelto y expone sus tokens como
//! `egui::Color32` para los paneles que pintan acentos propios (selección de fila,
//! barra del panel activo, indicadores, línea de drop).

use naygo_core::theme::{Theme, ThemeBase, ThemeColor, ThemeId};

/// El tema activo resuelto + su id (para el hot-swap).
pub struct ActiveTheme {
    pub id: ThemeId,
    pub theme: Theme,
}

impl ActiveTheme {
    pub fn new(id: ThemeId, theme: Theme) -> Self {
        ActiveTheme { id, theme }
    }
    pub fn accent(&self) -> egui::Color32 {
        to_color32(self.theme.accent)
    }
    /// Color de texto principal del tema. Se usa para teñir los íconos del set
    /// `mono` (siluetas monocromáticas) en la barra de herramientas.
    pub fn text(&self) -> egui::Color32 {
        to_color32(self.theme.text)
    }
    /// Color base del resaltado de archivos recién aparecidos (se tiñe el fondo de la fila).
    pub fn highlight(&self) -> egui::Color32 {
        to_color32(self.theme.highlight)
    }
    // Reservado: barra del panel activo (hoy el realce usa accent()); se mantiene el
    // token para futuros estilos que diferencien barra y acento.
    #[allow(dead_code)]
    pub fn active_bar(&self) -> egui::Color32 {
        to_color32(self.theme.active_bar)
    }
    pub fn selection_bg(&self) -> egui::Color32 {
        to_color32(self.theme.selection_bg)
    }
    // Reservado: color de texto tenue (rutas/secundarios) para cuando se tematice ese
    // matiz de forma explícita en los paneles.
    #[allow(dead_code)]
    pub fn text_dim(&self) -> egui::Color32 {
        to_color32(self.theme.text_dim)
    }
    pub fn error(&self) -> egui::Color32 {
        to_color32(self.theme.error)
    }
}

/// Convierte un `ThemeColor` a `egui::Color32` (opaco).
pub fn to_color32(c: ThemeColor) -> egui::Color32 {
    egui::Color32::from_rgb(c.r, c.g, c.b)
}

/// Traduce el tema a `egui::Visuals` y lo aplica al contexto (hot-swap).
pub fn apply(theme: &Theme, ctx: &egui::Context) {
    let mut v = match theme.base {
        ThemeBase::Dark => egui::Visuals::dark(),
        ThemeBase::Light => egui::Visuals::light(),
    };
    v.panel_fill = to_color32(theme.panel_bg);
    v.window_fill = to_color32(theme.panel_bg);
    v.extreme_bg_color = to_color32(theme.row_alt_bg);
    v.selection.bg_fill = to_color32(theme.selection_bg);
    v.selection.stroke.color = to_color32(theme.accent);
    v.hyperlink_color = to_color32(theme.accent);
    // Color de texto normal vía el stroke de widgets no-interactivos (lo que usa la
    // mayoría de los `Label`). NO se usa `override_text_color` porque aplana el texto
    // tenue (`.weak()`): egui deriva el tenue de este stroke mezclándolo hacia el
    // fondo, así que dejándolo derivar se conserva la distinción normal/tenue.
    v.widgets.noninteractive.fg_stroke.color = to_color32(theme.text);
    v.widgets.noninteractive.bg_stroke.color = to_color32(theme.border);
    ctx.set_visuals(v);
    // Sin selección de texto en labels: con `selectable_labels` (default true de egui)
    // CADA `ui.label` sensa `click_and_drag` para el cursor de texto, queda ENCIMA de
    // la fila de la tabla en el hit-test y le ROBA el clic — la fila nunca recibe
    // `clicked()` (causa raíz de "no puedo seleccionar con el mouse"). Naygo no
    // selecciona texto de celdas; un texto que sí deba copiarse puede optar puntual
    // con `Label::selectable(true)`.
    ctx.global_style_mut(|s| s.interaction.selectable_labels = false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_color32_mapea_rgb() {
        let c = to_color32(ThemeColor::new(0x2f, 0x81, 0xf7));
        assert_eq!(c, egui::Color32::from_rgb(0x2f, 0x81, 0xf7));
    }
}
