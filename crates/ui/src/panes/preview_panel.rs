// Naygo — panel Preview: vista previa liviana del archivo enfocado (texto/imagen).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta el resultado de la vista previa que `NaygoApp` calculó en un worker (texto
//! truncado en monospace, imagen escalada, o un mensaje neutro). NO hace I/O ni decide
//! qué previsualizar: solo dibuja el `PreviewView` que recibe.

use crate::app::PreviewView;

pub fn show(ui: &mut egui::Ui, view: &PreviewView, i18n: &naygo_core::i18n::I18n) {
    match view {
        PreviewView::Empty => {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(i18n.t("preview.empty")).weak());
        }
        PreviewView::Message(key) => {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(i18n.t(key)).weak());
        }
        PreviewView::Text { text, truncated } => {
            egui::ScrollArea::both().show(ui, |ui| {
                // Monospace para texto/código; selección deshabilitada (es solo lectura).
                ui.add(
                    egui::Label::new(egui::RichText::new(text).monospace())
                        .wrap_mode(egui::TextWrapMode::Extend),
                );
                if *truncated {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(i18n.t("preview.truncated"))
                            .weak()
                            .italics(),
                    );
                }
            });
        }
        PreviewView::Image(tex) => {
            egui::ScrollArea::both().show(ui, |ui| {
                // Imagen proporcional al ancho disponible, sin agrandar más allá de su
                // tamaño real (shrink-to-fit).
                let avail = ui.available_size();
                let size = tex.size_vec2();
                let scale = (avail.x / size.x).clamp(0.05, 1.0);
                ui.add(egui::Image::new(tex).fit_to_exact_size(size * scale));
            });
        }
    }
}
