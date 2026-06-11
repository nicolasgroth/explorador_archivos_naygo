// Naygo — sección Avanzado de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::config::HighlightDuration;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (title, sub) = (app.tr("settings.advanced"), app.tr("settings.advanced.sub"));
    super::section_header(ui, &title, &sub);
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

    super::group_sep(ui);
    super::group_label(ui, &app.tr("settings.ops.section"));

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

    super::group_sep(ui);
    super::group_label(ui, &app.tr("settings.paste.section"));

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

    super::group_sep(ui);
    super::group_label(ui, &app.tr("settings.watch.section"));

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
    ui.add_space(6.0);

    // Caché de carpetas visitadas (0 = desactivado). El cambio aplica en caliente:
    // la app recrea el caché al detectar el setting distinto.
    let l_cache = app.tr("settings.cache.max_dirs");
    ui.horizontal(|ui| {
        ui.label(l_cache);
        let mut v = app.settings.cache_max_dirs;
        if ui
            .add(egui::DragValue::new(&mut v).range(0..=500).speed(1))
            .changed()
        {
            app.settings.cache_max_dirs = v;
        }
    });
    ui.label(egui::RichText::new(app.tr("settings.cache.hint")).weak());

    // Integración con Windows: tray + cerrar-a-bandeja + inicio con Windows.
    super::group_sep(ui);
    super::group_label(ui, &app.tr("settings.system.section"));
    let l_tray = app.tr("settings.system.tray");
    let mut tray_on = app.settings.tray_enabled;
    if ui.checkbox(&mut tray_on, l_tray).changed() {
        app.settings.tray_enabled = tray_on;
    }
    ui.add_enabled_ui(app.settings.tray_enabled, |ui| {
        let l_ctt = app.tr("settings.system.close_to_tray");
        let mut ctt = app.settings.close_to_tray;
        if ui.checkbox(&mut ctt, l_ctt).changed() {
            app.settings.close_to_tray = ctt;
        }
    });
    // El checkbox de autostart refleja el REGISTRO real (no settings.json): así no
    // puede quedar desincronizado con lo que Windows va a hacer al arrancar.
    let l_auto = app.tr("settings.system.autostart");
    let mut auto = naygo_platform::autostart::is_enabled();
    if ui.checkbox(&mut auto, l_auto).changed() {
        if let Err(e) = naygo_platform::autostart::set_enabled(auto) {
            tracing::warn!("autostart: {e}");
        }
    }

    // Restaurar valores de fábrica: confirmación en dos pasos (el primer clic arma el
    // botón de confirmar por 4 s; el segundo ejecuta). El reset real lo procesa
    // `NaygoApp::logic` (patrón de acciones diferidas).
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);
    let confirm_id = egui::Id::new("naygo_factory_confirm_armed");
    let armed_at = ui.memory(|m| m.data.get_temp::<f64>(confirm_id));
    let now = ui.input(|i| i.time);
    let armed = armed_at.is_some_and(|t| now - t < 4.0);
    if armed {
        let l_confirm = app.tr("settings.factory_reset_confirm");
        let btn = egui::Button::new(
            egui::RichText::new(l_confirm).color(egui::Color32::from_rgb(0xE0, 0x55, 0x55)),
        );
        if ui.add(btn).clicked() {
            app.factory_reset_requested = true;
            ui.memory_mut(|m| m.data.remove::<f64>(confirm_id));
        }
        // Mantener vivo el conteo de los 4 s aunque no haya input.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(500));
    } else {
        if armed_at.is_some() {
            ui.memory_mut(|m| m.data.remove::<f64>(confirm_id));
        }
        let l_reset = app.tr("settings.factory_reset");
        if ui.button(l_reset).clicked() {
            ui.memory_mut(|m| m.data.insert_temp(confirm_id, now));
        }
    }
}
