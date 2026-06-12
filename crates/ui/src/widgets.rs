// Naygo — widgets reutilizables de la UI (segmented control, etc.).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Widgets propios que dan a la Configuración un acabado más "2026" que los
//! `selectable_value` planos. `segmented` pinta un grupo tipo "pill": un contenedor
//! redondeado con las opciones en fila; la activa lleva fondo de acento suave y texto
//! fuerte, las inactivas hover sutil. Devuelve `true` si la selección cambió.

/// Un control segmentado (pill group): elige UN valor entre varios. `value` es el valor
/// actual (se muta al elegir); `options` son los pares (valor, etiqueta). `accent` viene
/// del tema. Devuelve `true` si el valor cambió este frame.
///
/// Estilo: contenedor redondeado con un leve fondo; la opción activa va con fondo de
/// acento a baja opacidad + texto en acento y negrita; las inactivas, texto normal con
/// hover sutil. Altura cómoda (~26 px) y esquinas redondeadas.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[(T, &str)],
    accent: egui::Color32,
) -> bool {
    let mut changed = false;
    // Contenedor: un frame redondeado con padding pequeño que agrupa las opciones.
    let container_bg =
        egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 10);
    egui::Frame::new()
        .fill(container_bg)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for (val, label) in options {
                    let selected = *value == *val;
                    if pill(ui, label, selected, accent).clicked() && !selected {
                        *value = *val;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Un segmento individual del control. Pinta el fondo/realce según `selected` y devuelve
/// su `Response` para que el caller detecte el clic.
fn pill(ui: &mut egui::Ui, label: &str, selected: bool, accent: egui::Color32) -> egui::Response {
    // Tamaño cómodo: alto fijo ~22 px; ancho = texto medido + padding lateral.
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), egui::FontId::proportional(14.0), accent);
    let desired = egui::vec2(galley.size().x + 20.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        // Fondo del segmento activo: acento a baja opacidad; el hover de un inactivo, más
        // tenue aún. Esquinas redondeadas un poco menores que el contenedor.
        let radius = egui::CornerRadius::same(6);
        if selected {
            // Variante B (elegida por Nicolás): fondo de acento suave + borde fino de
            // acento en la opción activa, para distinguirla con más claridad.
            let fill =
                egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 55);
            painter.rect_filled(rect, radius, fill);
            painter.rect_stroke(
                rect.shrink(0.5),
                radius,
                egui::Stroke::new(1.0, accent),
                egui::StrokeKind::Inside,
            );
        } else if resp.hovered() {
            let fill =
                egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 22);
            painter.rect_filled(rect, radius, fill);
        }
        // Texto centrado: en acento si está activo, color de texto normal si no.
        let text_color = if selected {
            accent
        } else {
            ui.visuals().text_color()
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(14.0),
            text_color,
        );
    }
    resp
}
