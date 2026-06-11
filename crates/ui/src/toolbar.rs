// Naygo — barra de íconos: navegación + layouts + agregar panel + ajustes.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Barra de acciones con botones solo-ícono (tooltips). Posición configurable
//! (arriba o al costado), según `Settings.bar_position`. Atrás/adelante se
//! habilitan según el historial del panel activo.

use crate::app::NaygoApp;
use crate::input::Action;
use naygo_core::config::BarPosition;
use naygo_core::icon_kind::ActionIcon;

/// Pinta la barra en la posición configurada. Debe llamarse al inicio de `ui()`.
pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    match app.settings.bar_position {
        BarPosition::Top => {
            egui::Panel::top("toolbar").show_inside(ui, |ui| {
                ui.horizontal(|ui| buttons(ui, app));
            });
        }
        BarPosition::Side => {
            egui::Panel::left("toolbar")
                .resizable(false)
                .exact_size(40.0)
                .show_inside(ui, |ui| {
                    ui.vertical(|ui| buttons(ui, app));
                });
        }
    }
}

fn buttons(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (can_back, can_forward) = app
        .workspace
        .active_files()
        .map(|f| (f.history.can_back(), f.history.can_forward()))
        .unwrap_or((false, false));

    // Precalcular las etiquetas (tooltips) antes de los widgets que toman
    // `&mut app`, para no enredar los préstamos.
    let lbl_back = app.tr("toolbar.back");
    let lbl_forward = app.tr("toolbar.forward");
    let lbl_up = app.tr("toolbar.up");
    let lbl_refresh = app.tr("toolbar.refresh");
    let lbl_add_pane = app.tr("toolbar.add_pane");
    let lbl_copy = app.tr("op.copy");
    let lbl_cut = app.tr("op.cut");
    let lbl_paste = app.tr("op.paste");
    let lbl_delete = app.tr("op.delete");
    let lbl_new_file = app.tr("op.new_file");
    let lbl_new_folder = app.tr("op.new_folder");
    let lbl_settings = app.tr("toolbar.settings");

    // Estilo de íconos y colores resueltos UNA vez, antes de tomar `&app.icons`
    // (préstamo inmutable que coexiste con los cuerpos de los `if`, ya que estos
    // solo mutan `app` *después* de que el botón devolvió su `bool`).
    let style = app.settings.toolbar_icon_style;
    let glyph_color = match app.settings.toolbar_glyph_color {
        Some(tc) => egui::Color32::from_rgb(tc.r, tc.g, tc.b),
        None => app.active_theme.accent(),
    };
    // El set `mono` son siluetas monocromáticas: se tiñen con el color de texto del
    // tema. Cualquier otro set se pinta tal cual (sin tinte).
    let tint_mono = if app.icons.set() == "mono" {
        Some(app.active_theme.text())
    } else {
        None
    };

    // Recolectar el botón pulsado y procesar la acción DESPUÉS de pintarlos, para no
    // solapar el préstamo inmutable de `&app.icons` con los métodos `&mut app`. Cada
    // `icon_button` re-toma `&app.icons` de forma puntual; el `layouts_button`
    // intercalado mutará `app` en su propio statement, sin solape.
    // Historial del panel activo para el menú contextual de atrás/adelante:
    // (índice en la pila, nombre corto, ruta completa, es-el-actual). Precalculado
    // para que el closure del menú no tome prestado `app`.
    let history_items: Vec<(usize, String, String, bool)> = app
        .workspace
        .active_files()
        .map(|f| {
            let (stack, cursor) = f.history.stack();
            stack
                .iter()
                .enumerate()
                .rev() // el más reciente arriba, como en los navegadores
                .map(|(i, p)| {
                    let short = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string());
                    (i, short, p.display().to_string(), Some(i) == cursor)
                })
                .collect()
        })
        .unwrap_or_default();
    let mut history_jump: Option<usize> = None;
    // Menú de historial compartido por atrás y adelante (clic derecho).
    let history_menu = |ui: &mut egui::Ui, jump: &mut Option<usize>| {
        for (i, short, full, current) in &history_items {
            let text = if *current {
                egui::RichText::new(format!("• {short}")).strong()
            } else {
                egui::RichText::new(format!("   {short}"))
            };
            if ui.button(text).on_hover_text(full).clicked() {
                *jump = Some(*i);
                ui.close();
            }
        }
    };

    let mut clicked: Option<ActionIcon> = None;
    macro_rules! btn {
        ($glyph:expr, $action:expr, $tip:expr, $enabled:expr) => {
            if icon_button(
                ui,
                $glyph,
                $action,
                $tip,
                $enabled,
                style,
                glyph_color,
                &app.icons,
                tint_mono,
            )
            .clicked()
            {
                clicked = Some($action);
            }
        };
    }
    // Atrás/adelante fuera del macro: además del clic, su CLIC DERECHO abre el
    // menú con el historial del panel activo para saltar varios pasos de una vez.
    let back_resp = icon_button(
        ui,
        "◀",
        ActionIcon::Back,
        &lbl_back,
        can_back,
        style,
        glyph_color,
        &app.icons,
        tint_mono,
    );
    if back_resp.clicked() {
        clicked = Some(ActionIcon::Back);
    }
    back_resp.context_menu(|ui| history_menu(ui, &mut history_jump));
    let fwd_resp = icon_button(
        ui,
        "▶",
        ActionIcon::Forward,
        &lbl_forward,
        can_forward,
        style,
        glyph_color,
        &app.icons,
        tint_mono,
    );
    if fwd_resp.clicked() {
        clicked = Some(ActionIcon::Forward);
    }
    fwd_resp.context_menu(|ui| history_menu(ui, &mut history_jump));
    if let Some(i) = history_jump {
        app.history_jump_active(i);
    }
    btn!("▲", ActionIcon::Up, &lbl_up, true);
    btn!("⟳", ActionIcon::Refresh, &lbl_refresh, true);
    ui.separator();
    // Operaciones de archivo: mismos disparadores que el teclado / menú contextual.
    btn!("⧉", ActionIcon::Copy, &lbl_copy, true);
    btn!("✂", ActionIcon::Cut, &lbl_cut, true);
    btn!("📋", ActionIcon::Paste, &lbl_paste, true);
    btn!("🗑", ActionIcon::Delete, &lbl_delete, true);
    btn!("🗋", ActionIcon::NewFile, &lbl_new_file, true);
    btn!("🗀", ActionIcon::NewFolder, &lbl_new_folder, true);

    ui.separator();
    crate::templates_menu::layouts_button(ui, app);
    btn!("➕", ActionIcon::AddPane, &lbl_add_pane, true);
    // Menú de OTROS paneles (Historial/Árbol/Propiedades): ▾ chico junto al ➕.
    ui.menu_button("▾", |ui| {
        if ui.button(app.tr("pane.history.title")).clicked() {
            app.add_pane_of(naygo_core::workspace::PanePurpose::History);
            ui.close();
        }
        if ui.button(app.tr("pane.tree.title")).clicked() {
            app.add_pane_of(naygo_core::workspace::PanePurpose::Tree);
            ui.close();
        }
        if ui.button(app.tr("pane.inspector.title")).clicked() {
            app.add_pane_of(naygo_core::workspace::PanePurpose::Inspector);
            ui.close();
        }
    })
    .response
    .on_hover_text(app.tr("toolbar.add_other"));

    if let Some(action) = clicked {
        match action {
            ActionIcon::Back => app.apply_action(Action::GoBack),
            ActionIcon::Forward => app.apply_action(Action::GoForward),
            ActionIcon::Up => app.apply_action(Action::GoUp),
            ActionIcon::Refresh => {
                if let (Some(id), Some(dir)) = (
                    app.workspace.active_id(),
                    app.workspace.active_files().map(|f| f.current_dir.clone()),
                ) {
                    app.refresh_pane(id, dir);
                }
            }
            ActionIcon::Copy => app.apply_action(Action::Copy),
            ActionIcon::Cut => app.apply_action(Action::Cut),
            ActionIcon::Paste => app.apply_action(Action::Paste),
            ActionIcon::Delete => app.apply_action(Action::Delete),
            ActionIcon::NewFile => app.apply_action(Action::NewFile),
            ActionIcon::NewFolder => app.apply_action(Action::NewDir),
            ActionIcon::AddPane => app.add_files_pane(),
            ActionIcon::Settings => app.settings_open = true,
        }
    }

    ui.separator();
    // Strip de unidades de acceso rápido. Clic → navegar el panel activo a la raíz.
    // Se construye `drive_roots` (clonando fuera de `app.drives_cache`) ANTES del
    // bucle para no tener a `app` prestado mientras el cuerpo llama a `app.*`.
    let drive_roots: Vec<(String, std::path::PathBuf)> = app
        .drives_cache
        .iter()
        .map(|d| (d.path.to_string_lossy().into_owned(), d.path.clone()))
        .collect();
    let mut navigate_to: Option<std::path::PathBuf> = None;
    for (label, path) in &drive_roots {
        // Mostrar la letra de unidad (p. ej. "C:") de forma compacta. Las unidades
        // son etiquetas de texto (no acciones del pack), así que se pintan como
        // botón de texto con el color de glifo resuelto.
        let short = label.trim_end_matches(['\\', '/']).to_string();
        if drive_button(ui, &short, label, glyph_color) {
            navigate_to = Some(path.clone());
        }
    }
    if let Some(path) = navigate_to {
        app.navigate_active_to(path);
    }

    // Botón de ajustes: a la derecha del todo si la barra es horizontal (Top). Honra
    // el mismo estilo (glifo coloreado o ícono del pack) que el resto de la barra.
    let icons = &app.icons;
    let settings_clicked = if matches!(app.settings.bar_position, BarPosition::Top) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            icon_button(
                ui,
                "⚙",
                ActionIcon::Settings,
                &lbl_settings,
                true,
                style,
                glyph_color,
                icons,
                tint_mono,
            )
            .clicked()
        })
        .inner
    } else {
        icon_button(
            ui,
            "⚙",
            ActionIcon::Settings,
            &lbl_settings,
            true,
            style,
            glyph_color,
            icons,
            tint_mono,
        )
        .clicked()
    };
    if settings_clicked {
        app.settings_open = true;
    }
}

/// Botón de texto (etiqueta de unidad) con tooltip; usa el color de glifo resuelto.
fn drive_button(ui: &mut egui::Ui, label: &str, tip: &str, color: egui::Color32) -> bool {
    let txt = egui::RichText::new(label).color(color);
    ui.button(txt).on_hover_text(tip).clicked()
}

/// Un botón de acción de la barra: glifo Unicode coloreado (`Glyphs`) o el ícono del
/// pack activo (`Pack`), tinteado solo cuando el set activo es `mono`. Tooltip y
/// estado habilitado idénticos en ambos estilos.
#[allow(clippy::too_many_arguments)]
fn icon_button(
    ui: &mut egui::Ui,
    glyph: &str,
    action: ActionIcon,
    tip: &str,
    enabled: bool,
    style: naygo_core::config::ToolbarIconStyle,
    glyph_color: egui::Color32,
    icons: &crate::icons::IconProvider,
    tint_mono: Option<egui::Color32>,
) -> egui::Response {
    use naygo_core::config::ToolbarIconStyle;
    let resp = match style {
        ToolbarIconStyle::Glyphs => {
            let txt = egui::RichText::new(glyph).color(glyph_color).size(16.0);
            ui.add_enabled(enabled, egui::Button::new(txt))
        }
        ToolbarIconStyle::Pack => {
            let tex = icons.texture(naygo_core::icon_kind::IconKey::Action(action));
            let mut img = egui::Image::new(tex).fit_to_exact_size(egui::vec2(18.0, 18.0));
            if let Some(c) = tint_mono {
                img = img.tint(c);
            }
            ui.add_enabled(enabled, egui::Button::image(img))
        }
    };
    resp.on_hover_text(tip)
}
