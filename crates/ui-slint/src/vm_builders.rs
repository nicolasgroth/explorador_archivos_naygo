// Naygo — constructores de view-models y helpers puros de la capa UI (Slint).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Funciones puras de traducción core→VM (filas, columnas, inspector, preview, ops, paleta,
// settings) y helpers sin estado (editor de rename, import/export de packs, PNG de usuario).
// Extraídas de `main.rs` sin cambio de comportamiento (refactor por tamaño de archivo).

use crate::workspace_ctrl::WorkspaceCtrl;
use crate::*;
use slint::{ModelRc, SharedPixelBuffer, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

/// Publica una sesión de rename en el orden correcto y fuerza a recrear el `LineEdit`.
/// Cerrar primero invalida el editor anterior; su callback tardío lleva otra sesión y el
/// controlador lo ignora. Los offsets ya vienen expresados en bytes UTF-8 para Slint.
pub(crate) fn show_rename_editor(ui: &AppWindow, request: &workspace_ctrl::RenameRequest) {
    ui.set_rename_pane(-1);
    ui.set_rename_pos(-1);
    ui.set_rename_text(request.name.clone().into());
    // El draft arranca igual al nombre original; de ahí en adelante lo edita el usuario
    // (sobrevive re-montajes del delegate, a diferencia de `rename-text`).
    ui.set_rename_draft(request.name.clone().into());
    // Cursor de respaldo = fin del texto inicial (se usa solo si el delegate se re-monta).
    ui.set_rename_cursor(request.name.len() as i32);
    ui.set_rename_session(request.session as i32);
    ui.set_rename_selection_start(request.selection.0 as i32);
    ui.set_rename_selection_end(request.selection.1 as i32);
    ui.set_rename_pos(request.pos as i32);
    ui.set_rename_pane(request.pane.0 as i32);
}

/// Arma los textos traducidos del preview de comprimidos desde el catálogo i18n activo.
/// Se usa al arrancar y al cambiar el idioma, para no duplicar las claves en cada sitio.
pub(crate) fn archive_labels_from_config(
    cfg: &crate::config_ctrl::ConfigCtrl,
) -> naygo_core::archive_tree::ArchiveLabels {
    naygo_core::archive_tree::ArchiveLabels {
        files: cfg.t("archive.files"),
        folders: cfg.t("archive.folders"),
        uncompressed: cfg.t("archive.uncompressed"),
        more_entries: cfg.t("archive.more_entries"),
        and_more: cfg.t("archive.and_more"),
    }
}

/// Arma los mensajes de error del preview desde el catálogo i18n activo.
/// Se usa al arrancar y al cambiar el idioma, igual que `archive_labels_from_config`.
pub(crate) fn preview_msgs_from_config(
    cfg: &crate::config_ctrl::ConfigCtrl,
) -> crate::preview::PreviewMessages {
    crate::preview::PreviewMessages {
        not_previewable: cfg.t("preview.err.not_previewable"),
        archive_bad: cfg.t("preview.err.archive_bad"),
        read: cfg.t("preview.err.read"),
        image_big: cfg.t("preview.err.image_big"),
        cancelled: cfg.t("preview.err.cancelled"),
        decode: cfg.t("preview.err.decode"),
        svg_big: cfg.t("preview.err.svg_big"),
        svg_bad: cfg.t("preview.err.svg_bad"),
        rasterize: cfg.t("preview.err.rasterize"),
        pdf_big: cfg.t("preview.err.pdf_big"),
        pdf_pages: cfg.t("preview.pdf.pages"),
        pdf_no_text: cfg.t("preview.pdf.no_text"),
    }
}

pub(crate) fn int_to_purpose(p: i32) -> PanePurpose {
    match p {
        1 => PanePurpose::Tree,
        2 => PanePurpose::Inspector,
        3 => PanePurpose::History,
        4 => PanePurpose::Favorites,
        5 => PanePurpose::Preview,
        6 => PanePurpose::Operations,
        7 => PanePurpose::Basket,
        8 => PanePurpose::Search,
        9 => PanePurpose::Recents,
        _ => PanePurpose::Files,
    }
}

/// Copia un PNG de usuario al directorio `<config_dir>/icons/_user/` con un nombre único
/// (basado en el stem del archivo original más un contador de reintento para evitar colisiones).
/// Devuelve el nombre relativo del archivo copiado, o `None` si la copia falla.
pub(crate) fn copy_user_png(config_dir: &std::path::Path, src: &std::path::Path) -> Option<String> {
    let dest_dir = config_dir.join("icons").join("_user");
    if let Err(e) = std::fs::create_dir_all(&dest_dir) {
        eprintln!("[naygo] copy_user_png: no se pudo crear el directorio: {e}");
        return None;
    }
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("icon");
    // Sanear: solo alfanuméricos, guiones y guiones bajos.
    let stem: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(32)
        .collect();
    // Evitar colisiones con un contador de reintento.
    let mut rel = format!("{stem}.png");
    for i in 1u32.. {
        let candidate = dest_dir.join(&rel);
        if !candidate.exists() {
            break;
        }
        rel = format!("{stem}_{i}.png");
        if i > 9999 {
            // Defensa: no puede haber más de 9999 PNGs con el mismo stem.
            return None;
        }
    }
    if let Err(e) = std::fs::copy(src, dest_dir.join(&rel)) {
        eprintln!("[naygo] copy_user_png: error al copiar: {e}");
        return None;
    }
    Some(rel)
}

/// Muestra un modal de error (temático) si el resultado de un import/export falló;
/// silencioso si OK.
pub(crate) fn report<T>(ui: &slint::Weak<AppWindow>, r: Result<T, String>) {
    if let Err(e) = r {
        if let Some(ui) = ui.upgrade() {
            let tr = ui.global::<Tr>();
            ui.set_message(MessageVm {
                kind: 2,
                level: 2, // error
                title: "Naygo".into(),
                body: e.into(),
                confirm_label: tr.get_dlg_accept(),
                cancel_label: Default::default(),
                danger: false,
            });
        }
    }
}

/// Importa un archivo de pack (idioma/tema/config — cualquier extensión .naygo*, todas son un
/// .zip por dentro) y recarga el catálogo según el tipo que detecte el backend por su CONTENIDO.
/// Lógica común a los tres handlers de import por tópico: el filtro de extensión solo guía el
/// selector; `packs::import_zip` es la autoridad sobre qué se importó realmente.
pub(crate) fn import_topic(
    ctrl: &Rc<RefCell<WorkspaceCtrl>>,
    ui_weak: &slint::Weak<AppWindow>,
    cfg_weak: &slint::Weak<ConfigWindow>,
    refresh: &Rc<dyn Fn()>,
    path: &std::path::Path,
) {
    let config_dir = ctrl.borrow().config.config_dir.clone();
    match packs::import_zip(&config_dir, path) {
        Ok(kind) => {
            {
                let mut cb = ctrl.borrow_mut();
                let lang = cb.config.settings.language.clone();
                let theme = cb.config.settings.theme.clone();
                match kind {
                    packs::ImportKind::Lang(_) => {
                        cb.config.i18n = naygo_core::i18n::I18n::load(&config_dir, &lang);
                    }
                    packs::ImportKind::Theme(_) => {
                        cb.config.themes =
                            naygo_core::theme::ThemeCatalog::load(&config_dir, &theme);
                    }
                    packs::ImportKind::Config => {
                        let fresh = config_ctrl::ConfigCtrl::new(config_dir.clone());
                        cb.config = fresh;
                    }
                }
            }
            // Reaplicar textos y colores en AMBAS ventanas por si cambió el catálogo.
            let c = ctrl.borrow();
            if let Some(ui) = ui_weak.upgrade() {
                i18n_keys::apply(&ui, &c.config);
                theme_apply::apply(&ui, c.config.active_theme());
            }
            if let Some(cfg) = cfg_weak.upgrade() {
                i18n_keys::apply(&cfg, &c.config);
                theme_apply::apply(&cfg, c.config.active_theme());
            }
            drop(c);
            refresh();
        }
        Err(e) => report(ui_weak, Err::<(), String>(e)),
    }
}

/// Construye el `SettingsVm` (snapshot para la ventana de config) desde el ConfigCtrl.
pub(crate) fn build_settings_vm(c: &config_ctrl::ConfigCtrl) -> SettingsVm {
    use naygo_core::config::{BarPosition, OpsMode};
    let s = &c.settings;
    // Idiomas con marcador experimental para zh/ja/ko/hi.
    const EXPERIMENTAL_LANGS: &[&str] = &["zh", "ja", "ko", "hi"];
    // Nombre nativo del idioma + sufijo experimental cuando corresponde.
    let make_label = |code: &str| -> SharedString {
        let mut label = c.i18n.t(&format!("lang.{code}")).to_string();
        if EXPERIMENTAL_LANGS.contains(&code) {
            label.push_str(c.i18n.t("lang.experimental_suffix"));
        }
        SharedString::from(label)
    };
    let lang_codes: Vec<SharedString> = c
        .i18n
        .available()
        .iter()
        .map(|l| SharedString::from(l.as_str()))
        .collect();
    let languages: Vec<SharedString> = c
        .i18n
        .available()
        .iter()
        .map(|l| make_label(l.as_str()))
        .collect();
    // Índice del idioma activo dentro de lang_codes, para inicializar el combo en la
    // posición correcta (ThemeCombo deriva current-value de current-index, no al revés).
    let active_code = s.language.as_str();
    let active_idx = lang_codes
        .iter()
        .position(|c| c.as_str() == active_code)
        .unwrap_or(0) as i32;
    let themes: Vec<SharedString> = c
        .themes
        .available()
        .iter()
        .map(|t| SharedString::from(t.as_str()))
        .collect();
    let icon_sets: Vec<SharedString> = naygo_core::icon_set::IconSetCatalog::load(&c.config_dir)
        .available()
        .iter()
        .map(|s| SharedString::from(s.id.as_str()))
        .collect();
    SettingsVm {
        interface_font: s.fonts.interface.as_str().into(),
        listings_font: s.fonts.listings.as_str().into(),
        preview_font: s.fonts.preview.as_str().into(),
        bar_position: if s.bar_position == BarPosition::Side {
            1
        } else {
            0
        },
        icon_only: s.icon_only,
        show_parent: s.show_parent_entry,
        ops_mode: if s.ops_mode == OpsMode::Parallel {
            1
        } else {
            0
        },
        window_title_mode: match s.window_title_mode {
            naygo_core::WindowTitleMode::AppOnly => 0,
            naygo_core::WindowTitleMode::AppAndPath => 1,
            naygo_core::WindowTitleMode::PathOnly => 2,
        },
        confirm_trash: s.confirm_trash,
        confirm_drop_between_panes: s.confirm_drop_between_panes,
        show_op_summary: s.show_op_summary,
        size_no_subdirs: s.size_no_subdirs,
        autostart: s.autostart,
        autostart_minimized: s.autostart_minimized,
        date_format: match s.date_format {
            naygo_core::format::DateFormat::IsoMinute => 0,
            naygo_core::format::DateFormat::IsoDate => 1,
            naygo_core::format::DateFormat::DmyMinute => 2,
            naygo_core::format::DateFormat::DmyDate => 3,
        },
        size_format: match s.size_format {
            naygo_core::format::SizeFormat::Auto => 0,
            naygo_core::format::SizeFormat::Bytes => 1,
            naygo_core::format::SizeFormat::Kb => 2,
            naygo_core::format::SizeFormat::Mb => 3,
        },
        row_density: match s.row_density {
            naygo_core::config::RowDensity::Compact => 0,
            naygo_core::config::RowDensity::Comfortable => 1,
        },
        row_h: s.row_density.row_height(),
        ops_display: match s.ops_display {
            naygo_core::config::OpsDisplay::Panel => 0,
            naygo_core::config::OpsDisplay::Modal => 1,
            naygo_core::config::OpsDisplay::AlwaysVisible => 2,
        },
        paste_image_fmt: match s.paste_image_fmt {
            naygo_core::clipboard::ImageFmt::Png => 0,
            naygo_core::clipboard::ImageFmt::Jpg => 1,
        },
        paste_image_name: s.paste_image_name.clone().into(),
        paste_jpg_quality: s.paste_jpg_quality as i32,
        tray_enabled: s.tray_enabled,
        close_to_tray: s.close_to_tray,
        global_hotkey_enabled: s.global_hotkey_enabled,
        global_hotkey_text: config_ctrl::ConfigCtrl::chord_to_text(&s.global_hotkey).into(),
        new_items_at_end: s.new_items_at_end,
        low_power_mode: match s.low_power_mode {
            naygo_core::config::LowPowerMode::Auto => 0,
            naygo_core::config::LowPowerMode::Always => 1,
            naygo_core::config::LowPowerMode::Never => 2,
        },
        default_table_on: s.default_table.is_some(),
        paste_confirm: s.paste_confirm,
        paste_text_name: s.paste_text_name.clone().into(),
        paste_text_ext: s.paste_text_ext.clone().into(),
        language: s.language.as_str().into(),
        language_index: active_idx,
        theme: s.theme.as_str().into(),
        icon_set: s.icon_set.as_str().into(),
        languages: ModelRc::from(Rc::new(VecModel::from(languages))),
        language_codes: ModelRc::from(Rc::new(VecModel::from(lang_codes))),
        themes: ModelRc::from(Rc::new(VecModel::from(themes))),
        icon_sets: ModelRc::from(Rc::new(VecModel::from(icon_sets))),
        // Campos de la grilla de íconos por objeto — se poblarán en Task 15.
        icon_set_tintable: false,
        icon_rows: ModelRc::default(),
        icon_set_labels: ModelRc::default(),
        // Picker de ícono por objeto (Task 14): cerrado por defecto.
        icon_picker_key: SharedString::default(),
        icon_picker_choices: ModelRc::default(),
        // Galería de sets de íconos: se rellena en la fase 2b de refresh_config_vm.
        icon_set_cards: ModelRc::default(),
    }
}

/// El `PreviewVm` actual a partir del último resultado guardado en el controlador. El
/// resultado vivo se entrega por `poll()` en el timer y se cachea en el ctrl; aquí lo
/// reconstruimos para pintarlo. (Mantener la última vista evita parpadeo entre ticks.)
/// Nombre legible de un lenguaje de código para el combobox de Configuración (no es texto i18n:
/// son nombres propios de lenguajes, iguales en todos los idiomas).
pub(crate) fn code_lang_label(lang: naygo_core::preview::CodeLang) -> &'static str {
    use naygo_core::preview::CodeLang;
    match lang {
        CodeLang::Xml => "XML",
        CodeLang::Json => "JSON",
        CodeLang::Html => "HTML",
        CodeLang::Css => "CSS",
        CodeLang::JavaScript => "JavaScript",
        CodeLang::C => "C",
        CodeLang::Cpp => "C++",
        CodeLang::Java => "Java",
        CodeLang::Python => "Python",
        CodeLang::Rust => "Rust",
        CodeLang::Sql => "SQL",
        CodeLang::Bash => "Bash",
        CodeLang::Markdown => "Markdown",
        CodeLang::Yaml => "YAML",
        CodeLang::Toml => "TOML",
        CodeLang::Ini => "INI",
    }
}

pub(crate) fn current_preview_vm(c: &WorkspaceCtrl) -> PreviewVm {
    // Ruta del archivo enfocado (para "abrir con el programa del sistema"); usar `wanted`
    // primero evita que durante una carga larga se vea el path del archivo anterior.
    let path: SharedString = c
        .preview
        .wanted
        .as_ref()
        .or(c.preview.loaded.as_ref())
        .map(|p| SharedString::from(p.to_string_lossy().as_ref()))
        .unwrap_or_default();
    // Metadata por tipo del archivo enfocado (compartida por todos los modos de vista).
    let meta = meta_fields_model(c);
    let meta_loading = c.meta_loading();
    let mesh_interactive = c.preview.mesh_interactive();
    let mesh_busy = c.preview.busy() && mesh_interactive;
    let loading = c.preview.loading();
    let can_cancel = c.preview.can_cancel(std::time::Instant::now());
    match c.preview.last_view() {
        Some(preview::ViewCache::Text {
            text,
            truncated,
            highlighted,
        }) => {
            // Si el worker resaltó el texto, mapeamos cada línea/segmento al modelo de la UI; el
            // color (u8,u8,u8) de core se vuelve `slint::Color::from_rgb_u8`.
            let (is_hl, hl_lines): (bool, Vec<HlLineVm>) = match highlighted {
                Some(lines) => (
                    true,
                    lines
                        .iter()
                        .map(|l| {
                            let spans: Vec<HlSpanVm> = l
                                .spans
                                .iter()
                                .map(|s| HlSpanVm {
                                    text: SharedString::from(s.text.as_str()),
                                    color: slint::Color::from_rgb_u8(
                                        s.color.0, s.color.1, s.color.2,
                                    ),
                                })
                                .collect();
                            HlLineVm {
                                spans: ModelRc::from(Rc::new(VecModel::from(spans))),
                            }
                        })
                        .collect(),
                ),
                None => (false, Vec::new()),
            };
            PreviewVm {
                mode: 1,
                text: SharedString::from(text.as_str()),
                truncated: *truncated,
                image: slint::Image::default(),
                message: SharedString::new(),
                highlighted: is_hl,
                hl_lines: ModelRc::from(Rc::new(VecModel::from(hl_lines))),
                path,
                meta,
                meta_loading,
                mesh_interactive,
                mesh_busy,
                loading,
                can_cancel,
            }
        }
        Some(preview::ViewCache::Image {
            rgba,
            width,
            height,
        }) => {
            let buf = SharedPixelBuffer::clone_from_slice(rgba, *width, *height);
            PreviewVm {
                mode: 2,
                text: SharedString::new(),
                truncated: false,
                image: slint::Image::from_rgba8(buf),
                message: SharedString::new(),
                highlighted: false,
                hl_lines: ModelRc::default(),
                path,
                meta,
                meta_loading,
                mesh_interactive,
                mesh_busy,
                loading,
                can_cancel,
            }
        }
        Some(preview::ViewCache::Message(m)) => PreviewVm {
            mode: 3,
            text: SharedString::new(),
            truncated: false,
            image: slint::Image::default(),
            message: SharedString::from(m.as_str()),
            highlighted: false,
            hl_lines: ModelRc::default(),
            path,
            meta,
            meta_loading,
            mesh_interactive,
            mesh_busy,
            loading,
            can_cancel,
        },
        None => PreviewVm {
            mode: 0,
            text: SharedString::new(),
            truncated: false,
            image: slint::Image::default(),
            message: SharedString::new(),
            highlighted: false,
            hl_lines: ModelRc::default(),
            path: SharedString::new(),
            meta,
            meta_loading,
            mesh_interactive: false,
            mesh_busy: false,
            loading,
            can_cancel,
        },
    }
}

pub(crate) fn to_row_data(r: bridge::PlainRow) -> RowData {
    let cells: Vec<SharedString> = r
        .cells
        .iter()
        .map(|c| SharedString::from(c.as_str()))
        .collect();
    RowData {
        name: SharedString::from(r.name.as_str()),
        cells: ModelRc::from(Rc::new(VecModel::from(cells))),
        is_dir: r.is_dir,
        selected: r.selected,
        focused: r.focused,
        cut: r.cut,
        highlight: r.highlight,
        compare_state: r.compare_state as i32,
        filter_match: r.filter_match,
        match_pre: SharedString::from(r.match_pre.as_str()),
        match_mid: SharedString::from(r.match_mid.as_str()),
        match_post: SharedString::from(r.match_post.as_str()),
        icon: r.icon,
        depth: r.depth as i32,
    }
}

pub(crate) fn to_column_vm(c: bridge::ColumnInfo) -> ColumnVm {
    ColumnVm {
        kind: c.kind,
        label: SharedString::from(c.label.as_str()),
        width: c.width,
        align_right: c.align_right,
        sort_dir: c.sort_dir,
        has_filter: c.has_filter,
    }
}

pub(crate) fn to_column_toggle_vm(c: bridge::ColumnToggle) -> ColumnToggleVm {
    ColumnToggleVm {
        kind: c.kind,
        label: SharedString::from(c.label.as_str()),
        visible: c.visible,
        fixed: c.fixed,
    }
}

pub(crate) fn to_op_dialog_vm(d: ops_ctrl::OpDialogVmData) -> OpDialogVm {
    OpDialogVm {
        kind: d.kind,
        del_count: d.del_count,
        del_permanent: d.del_permanent,
        del_preview: SharedString::from(d.del_preview.as_str()),
        conflict_name: SharedString::from(d.conflict_name.as_str()),
        op_kind: d.op_kind,
        conflict_from: SharedString::from(d.conflict_from.as_str()),
        conflict_to: SharedString::from(d.conflict_to.as_str()),
        existing_name: SharedString::from(d.existing_name.as_str()),
        existing_size: SharedString::from(d.existing_size.as_str()),
        existing_date: SharedString::from(d.existing_date.as_str()),
        existing_ext: SharedString::from(d.existing_ext.as_str()),
        existing_is_dir: d.existing_is_dir,
        incoming_name: SharedString::from(d.incoming_name.as_str()),
        incoming_size: SharedString::from(d.incoming_size.as_str()),
        incoming_date: SharedString::from(d.incoming_date.as_str()),
        incoming_ext: SharedString::from(d.incoming_ext.as_str()),
        incoming_is_dir: d.incoming_is_dir,
        name_title: SharedString::from(d.name_title.as_str()),
        name_value: SharedString::from(d.name_value.as_str()),
        name_valid: d.name_valid,
        name_conflict_for: SharedString::from(d.name_conflict_for.as_str()),
        name_is_compress: d.name_is_compress,
        paste_name: SharedString::from(d.paste_name.as_str()),
        paste_is_image: d.paste_is_image,
        folder_name: SharedString::from(d.folder_name.as_str()),
        folder_more: d.folder_more,
    }
}

pub(crate) fn to_op_row_vm(r: ops_ctrl::OpRowData) -> OpRowVm {
    OpRowVm {
        has_errors: r.has_errors,
        index: r.index,
        label: SharedString::from(r.label.as_str()),
        percent: r.percent,
        status: SharedString::from(r.status.as_str()),
        running: r.running,
        paused: r.paused,
        bytes_done: SharedString::from(r.bytes_done.as_str()),
        bytes_total: SharedString::from(r.bytes_total.as_str()),
        files_done: r.files_done,
        files_total: r.files_total,
        current_file: SharedString::from(r.current_file.as_str()),
        speed: SharedString::from(r.speed.as_str()),
        speed_peak: SharedString::from(r.speed_peak.as_str()),
        eta: SharedString::from(r.eta.as_str()),
        elapsed: SharedString::from(r.elapsed.as_str()),
        kind: r.kind,
        op_kind: r.op_kind,
        when: SharedString::from(r.when.as_str()),
        files_summary: SharedString::from(r.files_summary.as_str()),
        has_file_list: r.has_file_list,
        files_done_count: r.files_done_count,
    }
}

pub(crate) fn to_op_file_vm(e: ops_ctrl::OpFileEntry) -> OpFileVm {
    OpFileVm {
        name: SharedString::from(e.name.as_str()),
        rel_path: SharedString::from(e.rel_path.as_str()),
        size: SharedString::from(e.size.as_str()),
        status: e.status,
        detail: SharedString::from(e.detail.as_str()),
    }
}

pub(crate) fn to_op_file_context_vm(c: ops_ctrl::OpFileContext) -> OpFileContextVm {
    OpFileContextVm {
        kind: c.kind,
        origin: SharedString::from(c.origin.as_str()),
        dest: SharedString::from(c.dest.as_str()),
        total_files: c.total_files,
        done: c.done,
        skipped: c.skipped,
        failed: c.failed,
        total_size: SharedString::from(c.total_size.as_str()),
    }
}

/// Categoría de comando de la paleta → el `int` que espera `PaletteItemVm.category`
/// (0=Acción 1=Archivo 2=Reciente 3=Favorito 4=Tema 5=Config). Espeja `CommandCategory`.
pub(crate) fn palette_category_to_int(c: naygo_core::palette::CommandCategory) -> i32 {
    use naygo_core::palette::CommandCategory as Cat;
    match c {
        Cat::Action => 0,
        Cat::File => 1,
        Cat::Recent => 2,
        Cat::Favorite => 3,
        Cat::Theme => 4,
        Cat::Config => 5,
    }
}

/// Mapea los resultados de `filter_and_rank` a la vista: un `PaletteItemVm` por match (con el
/// label partido en segmentos resaltados/normales según `hit_positions`) y, en paralelo, el
/// índice del COMANDO que cada fila ejecuta (para que `on_palette_run(result_idx)` sepa qué
/// comando correr). Devuelve `(items, cmd_indices)` con la misma longitud y orden.
pub(crate) fn palette_items_from_matches(
    commands: &[naygo_core::palette::Command],
    matches: &[naygo_core::palette::CommandMatch],
) -> (Vec<PaletteItemVm>, Vec<usize>) {
    let mut items: Vec<PaletteItemVm> = Vec::with_capacity(matches.len());
    let mut cmd_indices: Vec<usize> = Vec::with_capacity(matches.len());
    for m in matches {
        let Some(cmd) = commands.get(m.index) else {
            continue;
        };
        // Partir el label en runs contiguos: cada char se marca como hit si su índice está en
        // `hit_positions`. Acumulamos en spans alternando el flag `hit`.
        let chars: Vec<char> = cmd.label.chars().collect();
        let hit_set: std::collections::HashSet<usize> = m.hit_positions.iter().copied().collect();
        let mut spans: Vec<PaletteSpanVm> = Vec::new();
        let mut cur = String::new();
        let mut cur_hit: Option<bool> = None;
        for (i, ch) in chars.iter().enumerate() {
            let is_hit = hit_set.contains(&i);
            match cur_hit {
                Some(h) if h == is_hit => cur.push(*ch),
                Some(h) => {
                    spans.push(PaletteSpanVm {
                        text: SharedString::from(cur.as_str()),
                        hit: h,
                    });
                    cur.clear();
                    cur.push(*ch);
                    cur_hit = Some(is_hit);
                }
                None => {
                    cur.push(*ch);
                    cur_hit = Some(is_hit);
                }
            }
        }
        if let Some(h) = cur_hit {
            spans.push(PaletteSpanVm {
                text: SharedString::from(cur.as_str()),
                hit: h,
            });
        }
        items.push(PaletteItemVm {
            spans: ModelRc::from(Rc::new(VecModel::from(spans))),
            category: palette_category_to_int(cmd.category),
            shortcut: SharedString::from(cmd.shortcut.as_str()),
        });
        cmd_indices.push(m.index);
    }
    (items, cmd_indices)
}

pub(crate) fn to_nav_row(r: bridge::NavRow) -> NavRow {
    NavRow {
        label: SharedString::from(r.label.as_str()),
        path: SharedString::from(r.path.as_str()),
        icon: r.icon,
        removable: r.removable,
    }
}

pub(crate) fn to_hist_row(r: bridge::HistRow) -> HistRow {
    HistRow {
        id: r.id as i32,
        label: SharedString::from(r.label.as_str()),
        when: SharedString::from(r.when.as_str()),
        count: r.count,
        undoable: r.undoable,
        undone: r.undone,
        reason: SharedString::from(r.reason.as_str()),
    }
}

/// Convierte una ruta de la pila de navegación en una entrada del menú ▾ de historial: `name` es
/// el nombre de la carpeta (o la ruta completa si es raíz, p. ej. "C:\") y `path` la ruta completa
/// atenuada. Lo consumen los menús de Atrás/Adelante de la toolbar.
pub(crate) fn path_to_history_item(p: &std::path::Path) -> HistoryItemVm {
    let display = p.display().to_string();
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| display.clone());
    HistoryItemVm {
        name: SharedString::from(name),
        path: SharedString::from(display),
    }
}

pub(crate) fn to_tree_row(r: bridge::TreeRow) -> TreeRow {
    TreeRow {
        depth: r.depth,
        name: SharedString::from(r.name.as_str()),
        path: SharedString::from(r.path.as_str()),
        expanded: r.expanded,
        has_children: r.has_children,
        is_drive: r.is_drive,
        active: r.active,
        loading: r.loading,
        error: r.error,
        disk_percent: r.disk_percent,
        disk_detail: SharedString::from(r.disk_detail.as_str()),
        special_icon: SharedString::from(r.special_icon.as_str()),
        icon: r.icon,
    }
}

pub(crate) fn to_fav_tree_row(r: bridge::FavTreeRow) -> FavTreeRow {
    FavTreeRow {
        depth: r.depth,
        is_group: r.is_group,
        name: SharedString::from(r.name.as_str()),
        path: SharedString::from(r.path.as_str()),
        path_hint: SharedString::from(r.path_hint.as_str()),
        group_id: SharedString::from(r.group_id.as_str()),
        name_path: SharedString::from(r.name_path.as_str()),
        expanded: r.expanded,
        has_children: r.has_children,
        icon: r.icon,
    }
}

pub(crate) fn to_inspector_vm(i: bridge::InspectorInfo) -> InspectorVm {
    InspectorVm {
        present: i.present,
        name: SharedString::from(i.name.as_str()),
        kind: SharedString::from(i.kind.as_str()),
        path: SharedString::from(i.path.as_str()),
        size: SharedString::from(i.size.as_str()),
        modified: SharedString::from(i.modified.as_str()),
        created: SharedString::from(i.created.as_str()),
        is_dir: i.is_dir,
        size_calc: SharedString::from(i.size_calc.as_str()),
        // La metadata por tipo (dimensiones, versión) se puebla en `sync_rows` leyendo del worker
        // (ahí está el ctrl con la traducción i18n); aquí van los valores por defecto.
        meta: ModelRc::default(),
        meta_loading: false,
        has_provenance: false,
    }
}

/// Construye el modelo de campos de metadata para un VM, traduciendo cada clave i18n de etiqueta
/// con `config.t`. Los pares vienen del worker como (clave_i18n, valor_formateado).
pub(crate) fn meta_fields_model(c: &WorkspaceCtrl) -> ModelRc<MetaFieldVm> {
    let fields: Vec<MetaFieldVm> = c
        .meta_fields()
        .iter()
        .map(|(label_key, value)| MetaFieldVm {
            label: SharedString::from(c.config.t(label_key)),
            value: SharedString::from(value.as_str()),
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(fields)))
}
