// Naygo — sección Avanzado de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::config::HighlightDuration;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let title = app.tr("settings.advanced");
    ui.heading(title);
    ui.add_space(8.0);
    let (l_dir, dir) = (app.tr("settings.config_dir"), app.config_dir_display());
    ui.horizontal(|ui| {
        ui.label(l_dir);
        ui.monospace(dir);
    });
    let l_ver = app.tr("settings.version");
    ui.horizontal(|ui| {
        ui.label(l_ver);
        ui.monospace(env!("CARGO_PKG_VERSION"));
    });

    ui.add_space(12.0);
    ui.heading(app.tr("settings.ops.section"));
    ui.add_space(6.0);

    // Modo de ejecución.
    let (l_mode, l_queue, l_parallel) = (
        app.tr("settings.ops.mode"),
        app.tr("settings.ops.mode.queue"),
        app.tr("settings.ops.mode.parallel"),
    );
    ui.label(l_mode);
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.settings.ops_mode,
            naygo_core::config::OpsMode::Queue,
            l_queue,
        );
        ui.selectable_value(
            &mut app.settings.ops_mode,
            naygo_core::config::OpsMode::Parallel,
            l_parallel,
        );
    });
    ui.add_space(6.0);

    // Cómo se muestra el progreso.
    let (l_display, l_panel, l_modal, l_always) = (
        app.tr("settings.ops.display"),
        app.tr("settings.ops.display.panel"),
        app.tr("settings.ops.display.modal"),
        app.tr("settings.ops.display.always"),
    );
    ui.label(l_display);
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.settings.ops_display,
            naygo_core::config::OpsDisplay::Panel,
            l_panel,
        );
        ui.selectable_value(
            &mut app.settings.ops_display,
            naygo_core::config::OpsDisplay::Modal,
            l_modal,
        );
        ui.selectable_value(
            &mut app.settings.ops_display,
            naygo_core::config::OpsDisplay::AlwaysVisible,
            l_always,
        );
    });
    ui.add_space(6.0);

    // Checkboxes.
    let l_confirm_trash = app.tr("settings.ops.confirm_trash");
    let mut confirm_trash = app.settings.confirm_trash;
    if ui.checkbox(&mut confirm_trash, l_confirm_trash).changed() {
        app.settings.confirm_trash = confirm_trash;
    }
    let l_show_summary = app.tr("settings.ops.show_summary");
    let mut show_summary = app.settings.show_op_summary;
    if ui.checkbox(&mut show_summary, l_show_summary).changed() {
        app.settings.show_op_summary = show_summary;
    }

    ui.add_space(12.0);
    ui.heading(app.tr("settings.paste.section"));
    ui.add_space(6.0);

    // Confirmar antes de crear (modo B).
    let l_confirm = app.tr("settings.paste.confirm");
    let mut confirm = app.settings.paste_confirm;
    if ui.checkbox(&mut confirm, l_confirm).changed() {
        app.settings.paste_confirm = confirm;
    }
    ui.add_space(6.0);

    // Nombres (plantillas con {fecha}).
    ui.label(app.tr("settings.paste.date_hint"));
    let l_tname = app.tr("settings.paste.text_name");
    ui.horizontal(|ui| {
        ui.label(l_tname);
        ui.text_edit_singleline(&mut app.settings.paste_text_name);
    });
    let l_text = app.tr("settings.paste.text_ext");
    ui.horizontal(|ui| {
        ui.label(l_text);
        ui.text_edit_singleline(&mut app.settings.paste_text_ext);
    });
    let l_iname = app.tr("settings.paste.image_name");
    ui.horizontal(|ui| {
        ui.label(l_iname);
        ui.text_edit_singleline(&mut app.settings.paste_image_name);
    });
    ui.add_space(6.0);

    // Formato de imagen.
    let (l_fmt, l_png, l_jpg) = (
        app.tr("settings.paste.image_fmt"),
        app.tr("settings.paste.fmt_png"),
        app.tr("settings.paste.fmt_jpg"),
    );
    ui.label(l_fmt);
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.settings.paste_image_fmt,
            naygo_core::clipboard::ImageFmt::Png,
            l_png,
        );
        ui.selectable_value(
            &mut app.settings.paste_image_fmt,
            naygo_core::clipboard::ImageFmt::Jpg,
            l_jpg,
        );
    });
    ui.add_space(6.0);

    // Calidad JPG (solo aplica si el formato es JPG).
    let l_quality = app.tr("settings.paste.jpg_quality");
    ui.add_enabled_ui(
        app.settings.paste_image_fmt == naygo_core::clipboard::ImageFmt::Jpg,
        |ui| {
            ui.horizontal(|ui| {
                ui.label(l_quality);
                ui.add(egui::Slider::new(
                    &mut app.settings.paste_jpg_quality,
                    1..=100,
                ));
            });
        },
    );

    ui.add_space(12.0);
    ui.heading(app.tr("settings.watch.section"));
    ui.add_space(6.0);

    // Duración del resaltado de archivos recién aparecidos (watcher).
    let (l_dur, l_until_interact, l_fade, l_until_refresh) = (
        app.tr("settings.watch.highlight_duration"),
        app.tr("settings.watch.until_interact"),
        app.tr("settings.watch.fade"),
        app.tr("settings.watch.until_refresh"),
    );
    ui.label(l_dur);
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.settings.highlight_duration,
            HighlightDuration::UntilInteract,
            l_until_interact,
        );
        // FadeSeconds lleva un valor; el selector fija 6s (si el usuario tenía otra N,
        // se normaliza a 6 al elegir esta opción, aceptable para el selector simple).
        ui.selectable_value(
            &mut app.settings.highlight_duration,
            HighlightDuration::FadeSeconds(6),
            l_fade,
        );
        ui.selectable_value(
            &mut app.settings.highlight_duration,
            HighlightDuration::UntilRefresh,
            l_until_refresh,
        );
    });
    ui.add_space(6.0);

    // Agrupar archivos nuevos al final de la vista.
    let l_new_at_end = app.tr("settings.watch.new_at_end");
    let mut new_at_end = app.settings.new_items_at_end;
    if ui.checkbox(&mut new_at_end, l_new_at_end).changed() {
        app.settings.new_items_at_end = new_at_end;
    }
    ui.add_space(6.0);

    // Tamaño de carpeta (F3): recursivo vs. solo primer nivel.
    let l_size = app.tr("settings.size_no_subdirs");
    let mut size_no_subdirs = app.settings.size_no_subdirs;
    if ui.checkbox(&mut size_no_subdirs, l_size).changed() {
        app.settings.size_no_subdirs = size_no_subdirs;
    }
}
