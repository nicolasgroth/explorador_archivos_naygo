// Naygo — sección Atajos: editor de keybindings configurable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use crate::input::chord_text;
use naygo_core::keymap::{Action, Chord};

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (title, sub) = (
        app.tr("settings.shortcuts"),
        app.tr("settings.shortcuts.sub"),
    );
    super::section_header(ui, &title, &sub);

    let search_hint = app.tr("settings.shortcuts.search");
    let reset_all_label = app.tr("settings.shortcuts.reset_all");
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.shortcut_search).hint_text(search_hint));
        if ui.button(reset_all_label).clicked() {
            app.keymap.reset_all();
            app.shortcut_conflict = None;
            app.save_keymap_now();
        }
    });
    if let Some(msg) = app.shortcut_conflict.clone() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(msg).color(egui::Color32::from_rgb(0xe0, 0xa0, 0x30)));
    }
    ui.add_space(6.0);

    let query = app.shortcut_search.to_lowercase();
    let add_label = app.tr("settings.shortcuts.add");
    let capturing_label = app.tr("settings.shortcuts.capturing");
    let reset_one_label = app.tr("settings.shortcuts.reset_one");
    let none_label = app.tr("settings.shortcuts.none");

    // Filas (acción, nombre, chords) precomputadas para no prestar app mientras pintamos.
    let rows: Vec<(Action, String, Vec<Chord>)> = Action::all()
        .iter()
        .map(|a| (*a, app.tr(a.i18n_key()), app.keymap.chords_for(*a).to_vec()))
        .filter(|(_, name, _)| query.is_empty() || name.to_lowercase().contains(&query))
        .collect();

    let mut to_unbind: Option<(Action, Chord)> = None;
    let mut to_capture: Option<Action> = None;
    let mut to_reset: Option<Action> = None;

    egui::Grid::new("shortcuts_editor")
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            for (action, name, chords) in &rows {
                ui.label(name);
                ui.horizontal(|ui| {
                    if chords.is_empty() && app.shortcut_capture != Some(*action) {
                        ui.label(egui::RichText::new(&none_label).weak());
                    }
                    for c in chords {
                        ui.label(egui::RichText::new(chord_text(c)).monospace());
                        if ui.small_button("×").clicked() {
                            to_unbind = Some((*action, *c));
                        }
                    }
                    if app.shortcut_capture == Some(*action) {
                        ui.label(egui::RichText::new(&capturing_label).italics());
                    } else if ui.small_button(&add_label).clicked() {
                        to_capture = Some(*action);
                    }
                });
                if ui
                    .small_button("↺")
                    .on_hover_text(&reset_one_label)
                    .clicked()
                {
                    to_reset = Some(*action);
                }
                ui.end_row();
            }
        });

    if let Some((a, c)) = to_unbind {
        app.keymap.unbind(a, &c);
        app.shortcut_conflict = None;
        app.save_keymap_now();
    }
    if let Some(a) = to_capture {
        app.shortcut_capture = Some(a);
        app.shortcut_conflict = None;
    }
    if let Some(a) = to_reset {
        app.keymap.reset_action(a);
        app.shortcut_conflict = None;
        app.save_keymap_now();
    }
}
