// Naygo — panel Favoritos: carpetas ancladas + sección de recientes.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lista los favoritos (en el orden del usuario: el 1º responde a `Ctrl+1`) y,
//! debajo, las carpetas recientes (MRU global ya podada de rutas inexistentes por
//! `NaygoApp` antes de pintar). El panel NO ejecuta nada: el clic en una entrada
//! se acumula en `navigate` y el "quitar" del menú contextual en `remove`;
//! `NaygoApp` los procesa tras pintar (patrón de acciones diferidas del Historial).
//! No hace I/O.

use std::path::PathBuf;

pub fn show(
    ui: &mut egui::Ui,
    favorites: &[(String, PathBuf)],
    recents: &[PathBuf],
    i18n: &naygo_core::i18n::I18n,
    theme: &crate::theme_apply::ActiveTheme,
    navigate: &mut Vec<PathBuf>,
    remove: &mut Vec<PathBuf>,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        if favorites.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(i18n.t("fav.empty")).weak());
        } else {
            for (i, (label, path)) in favorites.iter().enumerate() {
                ui.horizontal(|ui| {
                    // Hint del atajo: los 9 primeros responden a Ctrl+1..9.
                    if i < 9 {
                        ui.label(egui::RichText::new((i + 1).to_string()).weak().size(11.0));
                    }
                    let resp = ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(format!("★ {label}")).color(theme.accent()),
                            )
                            .frame(false),
                        )
                        .on_hover_text(path.display().to_string());
                    if resp.clicked() {
                        navigate.push(path.clone());
                    }
                    // Clic derecho sobre un favorito → quitar. Va en un menú de un
                    // ítem (no quitado instantáneo) para que un clic derecho
                    // accidental no destruya el favorito sin confirmación visual.
                    resp.context_menu(|ui| {
                        if ui.button(i18n.t("fav.remove")).clicked() {
                            remove.push(path.clone());
                            ui.close();
                        }
                    });
                });
            }
        }

        // ── Recientes ──
        ui.add_space(6.0);
        ui.separator();
        ui.label(egui::RichText::new(i18n.t("fav.recents")).strong());
        if recents.is_empty() {
            ui.label(egui::RichText::new(i18n.t("fav.recents_empty")).weak());
        } else {
            for path in recents {
                // Etiqueta corta (nombre de la carpeta); raíces de unidad muestran
                // la ruta completa (no tienen file_name).
                let short = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                let resp = ui
                    .add(egui::Button::new(short).frame(false))
                    .on_hover_text(path.display().to_string());
                if resp.clicked() {
                    navigate.push(path.clone());
                }
            }
        }
    });
}
