// Naygo — combobox de plantillas: recientes, favoritos, built-in, guardar.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! El botón "▦ Layouts" abre un menú con: Recientes (auto), Favoritos y Míos
//! (del usuario), Built-in, y "Guardar disposición actual…". La administración
//! fina (renombrar, limpiar) vive en Configuración (fase posterior).

use crate::app::NaygoApp;
use naygo_core::workspace::template::LayoutTemplate;

pub fn layouts_button(ui: &mut egui::Ui, app: &mut NaygoApp) {
    ui.menu_button("▦", |ui| {
        let now = now_epoch_secs();

        // Recientes.
        if !app.templates.recents.is_empty() {
            ui.label("🕘 Recientes");
            let recents: Vec<String> = app
                .templates
                .recents
                .iter()
                .map(|r| r.name.clone())
                .collect();
            for name in recents {
                if ui.button(&name).clicked() {
                    if let Some(tpl) = find_template(app, &name) {
                        app.apply_template(&tpl, now);
                    }
                    ui.close();
                }
            }
            ui.separator();
        }

        // Favoritos.
        let favs: Vec<LayoutTemplate> = app
            .templates
            .user
            .iter()
            .filter(|t| t.favorite)
            .cloned()
            .collect();
        if !favs.is_empty() {
            ui.label("★ Favoritos");
            for t in favs {
                if ui.button(&t.name).clicked() {
                    app.apply_template(&t, now);
                    ui.close();
                }
            }
            ui.separator();
        }

        // Míos (con marcar favorito / borrar).
        let mine: Vec<LayoutTemplate> = app.templates.user.clone();
        if !mine.is_empty() {
            ui.label("👤 Míos");
            for t in mine {
                ui.horizontal(|ui| {
                    if ui.button(&t.name).clicked() {
                        app.apply_template(&t, now);
                        ui.close();
                    }
                    let star = if t.favorite { "★" } else { "☆" };
                    if ui.small_button(star).on_hover_text("Favorito").clicked() {
                        app.templates.set_favorite(&t.name, !t.favorite);
                    }
                    if ui.small_button("🗑").on_hover_text("Borrar").clicked() {
                        app.templates.remove_user(&t.name);
                    }
                });
            }
            ui.separator();
        }

        // Built-in.
        ui.label("📋 Predefinidos");
        for t in LayoutTemplate::builtins() {
            if ui.button(&t.name).clicked() {
                app.apply_template(&t, now);
                ui.close();
            }
        }
        ui.separator();

        // Guardar disposición actual.
        if ui.button("💾 Guardar disposición actual…").clicked() {
            let n = app.templates.user.len() + 1;
            app.save_current_as_template(&format!("Mi layout {n}"));
            ui.close();
        }
    })
    .response
    .on_hover_text("Layouts");
}

/// Busca una plantilla por nombre entre las del usuario y las built-in.
fn find_template(app: &NaygoApp, name: &str) -> Option<LayoutTemplate> {
    app.templates
        .user
        .iter()
        .find(|t| t.name == name)
        .cloned()
        .or_else(|| {
            LayoutTemplate::builtins()
                .into_iter()
                .find(|t| t.name == name)
        })
}

/// Segundos epoch actuales (la UI puede llamar a SystemTime; core no).
fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
