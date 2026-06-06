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
            ui.label(format!("🕘 {}", app.tr("templates.recents")));
            let recents: Vec<String> = app
                .templates
                .recents
                .iter()
                .map(|r| r.name.clone())
                .collect();
            for name in recents {
                let label = builtin_label(app, &name);
                if ui.button(label).clicked() {
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
            ui.label(format!("★ {}", app.tr("templates.favorites")));
            for t in favs {
                let label = builtin_label(app, &t.name);
                if ui.button(label).clicked() {
                    app.apply_template(&t, now);
                    ui.close();
                }
            }
            ui.separator();
        }

        // Míos (con marcar favorito / borrar).
        let mine: Vec<LayoutTemplate> = app.templates.user.clone();
        if !mine.is_empty() {
            ui.label(format!("👤 {}", app.tr("templates.mine")));
            let tip_fav = app.tr("templates.favorite");
            let tip_del = app.tr("templates.delete");
            for t in mine {
                let label = builtin_label(app, &t.name);
                ui.horizontal(|ui| {
                    if ui.button(label).clicked() {
                        app.apply_template(&t, now);
                        ui.close();
                    }
                    let star = if t.favorite { "★" } else { "☆" };
                    if ui.small_button(star).on_hover_text(&tip_fav).clicked() {
                        app.templates.set_favorite(&t.name, !t.favorite);
                    }
                    if ui.small_button("🗑").on_hover_text(&tip_del).clicked() {
                        app.templates.remove_user(&t.name);
                    }
                });
            }
            ui.separator();
        }

        // Built-in.
        ui.label(format!("📋 {}", app.tr("templates.builtin")));
        for t in LayoutTemplate::builtins() {
            let label = builtin_label(app, &t.name);
            if ui.button(label).clicked() {
                app.apply_template(&t, now);
                ui.close();
            }
        }
        ui.separator();

        // Guardar disposición actual.
        if ui
            .button(format!("💾 {}", app.tr("templates.save_current")))
            .clicked()
        {
            let n = app.templates.user.len() + 1;
            app.save_current_as_template(&format!("Mi layout {n}"));
            ui.close();
        }
    })
    .response
    .on_hover_text(app.tr("toolbar.layouts"));
}

/// Etiqueta a mostrar para una plantilla: las built-in se traducen; las del
/// usuario conservan su nombre literal. El `name` real sigue usándose para la
/// lógica (apply/record/favorite/remove); solo cambia el texto visible.
fn builtin_label(app: &NaygoApp, name: &str) -> String {
    let key = match name {
        "Minimalista" => "template.minimalista",
        "Clásico" => "template.clasico",
        "Dual-pane" => "template.dual_pane",
        "Power-user" => "template.power_user",
        _ => return name.to_string(), // plantilla del usuario: nombre literal
    };
    app.tr(key)
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
