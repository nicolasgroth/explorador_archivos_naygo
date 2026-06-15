// Naygo — puente de temas: vuelca los colores del tema activo del core al global `Theme` de
// Slint. Se llama al arrancar y cada vez que se cambia de tema en la configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::{AppWindow, Theme};
use naygo_core::theme::{Theme as CoreTheme, ThemeColor};
use slint::{Color, ComponentHandle};

/// Convierte un color del core (RGB u8) a un `slint::Color` opaco.
fn col(c: ThemeColor) -> Color {
    Color::from_rgb_u8(c.r, c.g, c.b)
}

/// Aplica todos los colores del tema `t` al global `Theme` de la ventana `ui`.
pub fn apply(ui: &AppWindow, t: &CoreTheme) {
    let theme = ui.global::<Theme>();
    theme.set_accent(col(t.accent));
    theme.set_panel_bg(col(t.panel_bg));
    theme.set_row_bg(col(t.row_bg));
    theme.set_row_alt_bg(col(t.row_alt_bg));
    theme.set_text(col(t.text));
    theme.set_text_dim(col(t.text_dim));
    theme.set_selection_bg(col(t.selection_bg));
    theme.set_active_bar(col(t.active_bar));
    theme.set_error(col(t.error));
    theme.set_highlight(col(t.highlight));
    theme.set_border(col(t.border));
}
