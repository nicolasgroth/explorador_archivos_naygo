// Naygo — path-bar interactiva: breadcrumbs clicables + edición con autocompletado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! La barra de ruta del tope de cada panel `Files`. Dos modos excluyentes:
//!
//! - **Breadcrumbs** (default): cada segmento es un botón que navega a esa
//!   carpeta; a la derecha, los íconos 📋 (copiar ruta) y ☆/★ (favorito). El clic
//!   en la zona vacía (o `Ctrl+L`/`F4` vía keymap) pasa a edición.
//! - **Edición**: un `TextEdit` con la ruta completa pre-seleccionada y un popup
//!   de autocompletado (subcarpetas del padre del texto tecleado). Enter navega
//!   si la ruta existe, Esc o perder el foco cancelan, Tab/clic completan.
//!
//! El render NO muta `NaygoApp` ni hace I/O de listado: navega vía `PaneRequest`
//! y difiere el resto en `PathBarAction`s (patrón estándar del repo). Los nombres
//! del autocompletado los calcula un WORKER en `NaygoApp` (la UI jamás hace
//! `read_dir`); aquí solo se filtran (función pura de `core::path_segments`).
//! Únicas excepciones toleradas: el `is_dir()` puntual al confirmar con Enter y
//! el portapapeles de egui al copiar.

use crate::docking::PaneRequest;
use naygo_core::path_segments::{filter_candidates, split_edit_buffer, split_segments};
use naygo_core::workspace::PaneId;
use std::path::{Path, PathBuf};

/// Máximo de candidatos visibles en el popup de autocompletado.
const MAX_CANDIDATES: usize = 12;

/// Acción diferida de la path-bar, procesada por `NaygoApp` tras pintar (la barra
/// no tiene `&mut app`; mismo patrón que `TableAction`/`TreeAction`).
pub enum PathBarAction {
    /// La ruta ya se copió al portapapeles (egui); falta el status "copiado".
    Copied,
    /// Alternar la carpeta como favorita (y persistir).
    ToggleFavorite(PathBuf),
    /// Enter sobre una ruta inexistente: status de error con la ruta tecleada.
    BadPath(String),
}

/// Parámetros de la path-bar que `NaygoApp` presta vía el TabViewer (agrupados en
/// un struct para no inflar más la firma de `file_panel::show`).
pub struct PathBarParams<'a> {
    /// ¿La carpeta actual ya es favorita? (decide ☆ vs ★).
    pub is_favorite: bool,
    /// Estado de edición de ruta (vive en `NaygoApp`, como `inline_rename`).
    pub path_edit: &'a mut Option<crate::app::PathEdit>,
    /// Acciones diferidas hacia `NaygoApp`.
    pub actions: &'a mut Vec<PathBarAction>,
    /// Carpeta padre cuyos nombres de subcarpeta están cargados por el worker.
    pub ac_parent: &'a str,
    /// Nombres de subcarpetas de `ac_parent` (los candidatos sin filtrar).
    pub ac_names: &'a [String],
    /// Carpetas recientes (para marcar candidatos con ★ débil).
    pub recents: &'a [PathBuf],
}

/// Pinta la path-bar del panel `id` parado en `current_dir`.
pub fn show(
    ui: &mut egui::Ui,
    id: PaneId,
    current_dir: &Path,
    pending: &mut Vec<PaneRequest>,
    i18n: &naygo_core::i18n::I18n,
    theme: &crate::theme_apply::ActiveTheme,
    mut p: PathBarParams<'_>,
) {
    let editing = p.path_edit.as_ref().is_some_and(|e| e.pane == id);
    if editing {
        edit_mode(ui, id, pending, &mut p);
    } else {
        breadcrumb_mode(ui, id, current_dir, pending, i18n, theme, &mut p);
    }
}

/// Modo breadcrumbs: segmentos clicables + íconos copiar/favorito + zona de edición.
fn breadcrumb_mode(
    ui: &mut egui::Ui,
    id: PaneId,
    current_dir: &Path,
    pending: &mut Vec<PaneRequest>,
    i18n: &naygo_core::i18n::I18n,
    theme: &crate::theme_apply::ActiveTheme,
    p: &mut PathBarParams<'_>,
) {
    ui.horizontal(|ui| {
        // Los íconos van anclados a la DERECHA: se pintan primero en un layout
        // right_to_left y el resto del ancho queda para los breadcrumbs.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // ☆/★ favorito (estrella llena + color de acento si ya lo es).
            let (glyph, tip) = if p.is_favorite {
                ("★", i18n.t("pathbar.fav_remove"))
            } else {
                ("☆", i18n.t("pathbar.fav_add"))
            };
            let star = if p.is_favorite {
                egui::RichText::new(glyph).color(theme.accent())
            } else {
                egui::RichText::new(glyph)
            };
            if ui
                .add(egui::Button::new(star).frame(false))
                .on_hover_text(tip)
                .clicked()
            {
                p.actions
                    .push(PathBarAction::ToggleFavorite(current_dir.to_path_buf()));
            }
            // 📋 copiar la ruta. El portapapeles de egui no es I/O de disco; el
            // status "copiado" se difiere a NaygoApp (texto i18n centralizado).
            if ui
                .add(egui::Button::new("📋").frame(false))
                .on_hover_text(i18n.t("pathbar.copy"))
                .clicked()
            {
                ui.ctx().copy_text(current_dir.display().to_string());
                p.actions.push(PathBarAction::Copied);
            }

            // El resto del ancho: breadcrumbs (izq→der) + zona vacía clicable. Se acota
            // al ancho que QUEDA tras reservar los íconos de la derecha (📋, ☆/★), y se
            // recorta (clip) si no cabe, para que esos íconos NUNCA se tapen con una ruta
            // larga (antes compartían contenedor y los breadcrumbs los empujaban fuera).
            let remaining = ui.available_width();
            ui.allocate_ui_with_layout(
                egui::vec2(remaining, ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_clip_rect(ui.max_rect());
                    // Botones chicos (Button, no Label): la lección de los clics muertos
                    // aplica solo DENTRO de la tabla; aquí estamos fuera y un Button es
                    // la superficie de clic correcta (selectable_labels está apagado
                    // globalmente en theme_apply).
                    for (i, (label, dir)) in split_segments(current_dir).into_iter().enumerate() {
                        if i > 0 {
                            ui.label(egui::RichText::new("›").weak());
                        }
                        if ui.small_button(label).clicked() {
                            pending.push(PaneRequest::Activate { id });
                            pending.push(PaneRequest::NavigateTo { id, dir });
                        }
                    }
                    // Zona vacía a la derecha de los segmentos: clic = pasar a edición
                    // (como Explorer). Es un área con sense propio, no un widget visible.
                    let w = ui.available_width().max(24.0);
                    let (_, resp) = ui.allocate_exact_size(
                        egui::vec2(w, ui.spacing().interact_size.y),
                        egui::Sense::click(),
                    );
                    if resp.on_hover_text(i18n.t("pathbar.edit_hint")).clicked() {
                        *p.path_edit = Some(crate::app::PathEdit {
                            pane: id,
                            text: current_dir.display().to_string(),
                            focus_pending: true,
                            select_pending: true,
                        });
                    }
                },
            );
        });
    });
}

/// Modo edición: TextEdit + popup de autocompletado. Las teclas del gesto
/// (Enter/Esc/Tab) se CONSUMEN antes de crear el widget (patrón exacto del rename
/// inline) para que el TextEdit no las procese también.
fn edit_mode(
    ui: &mut egui::Ui,
    id: PaneId,
    pending: &mut Vec<PaneRequest>,
    p: &mut PathBarParams<'_>,
) {
    let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let esc = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let tab = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab));

    // Señales resueltas dentro del préstamo de `st`; se aplican al salir (no se
    // puede asignar `*p.path_edit = None` mientras `st` lo tiene prestado).
    let mut close = false;
    let mut navigate: Option<PathBuf> = None;
    {
        let st = p.path_edit.as_mut().expect("edit_mode implica Some");
        let te_id = egui::Id::new(("naygo_path_edit", id.0));

        // ORDEN CRÍTICO: la selección "todo" se aplica ANTES de crear el TextEdit
        // (el widget carga su estado al crearse; después llega tarde — la lección
        // de apply_rename_selection).
        if st.select_pending {
            set_cursor_range(ui.ctx(), te_id, 0, st.text.chars().count());
            st.select_pending = false;
        }

        // Candidatos: subcarpetas del padre del texto tecleado que empiecen con el
        // último segmento. El worker carga `ac_names`; aquí solo se filtra (puro).
        let (parent, prefix) = split_edit_buffer(&st.text);
        let candidates: Vec<String> = if !parent.is_empty() && parent == p.ac_parent {
            filter_candidates(p.ac_names, &prefix, MAX_CANDIDATES)
        } else {
            Vec::new()
        };

        let resp = ui.add(
            egui::TextEdit::singleline(&mut st.text)
                .id(te_id)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY),
        );
        if st.focus_pending {
            resp.request_focus();
            st.focus_pending = false;
        }

        // Tab completa con el PRIMER candidato; el clic en el popup, con el suyo.
        let mut completed: Option<String> = if tab {
            candidates.first().cloned()
        } else {
            None
        };

        // Popup bajo el TextEdit (Area anclada al rect; capa Foreground para que
        // quede sobre la tabla). Solo si hay candidatos y el editor tiene foco.
        if !candidates.is_empty() && (resp.has_focus() || st.focus_pending) {
            egui::Area::new(te_id.with("ac_popup"))
                .order(egui::Order::Foreground)
                .fixed_pos(resp.rect.left_bottom() + egui::vec2(0.0, 2.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width((resp.rect.width() * 0.5).clamp(160.0, 420.0));
                        for name in &candidates {
                            // ★ débil si la carpeta también está en recientes
                            // (comparación en memoria: ≤30 recientes × ≤12 candidatos).
                            let full = PathBuf::from(format!("{parent}{name}"));
                            let in_recents = p.recents.iter().any(|r| r == &full);
                            let text = if in_recents {
                                format!("{name} ★")
                            } else {
                                name.clone()
                            };
                            if ui.small_button(text).clicked() {
                                completed = Some(name.clone());
                            }
                        }
                    });
                });
        }

        if let Some(name) = completed {
            // Reemplazar el último segmento por el candidato + separador; el popup
            // se recalcula al frame siguiente con el padre nuevo.
            st.text = format!("{parent}{name}{}", std::path::MAIN_SEPARATOR);
            let end = st.text.chars().count();
            set_cursor_range(ui.ctx(), te_id, end, end);
            // El clic en el popup roba el foco del TextEdit: lo recuperamos para
            // seguir tecleando (y para que `lost_focus` no cancele la edición).
            resp.request_focus();
        } else if esc {
            close = true;
        } else if enter {
            // `is_dir()` puntual (metadata local, excepción tolerada): si existe se
            // navega; si no, status de error y se sigue editando.
            let target = PathBuf::from(st.text.trim());
            if target.is_dir() {
                navigate = Some(target);
                close = true;
            } else {
                p.actions.push(PathBarAction::BadPath(st.text.clone()));
            }
        } else if resp.lost_focus() {
            // Clic fuera del editor (Enter/Esc ya se consumieron y no llegan acá).
            close = true;
        }
    }
    if let Some(dir) = navigate {
        pending.push(PaneRequest::Activate { id });
        pending.push(PaneRequest::NavigateTo { id, dir });
    }
    if close {
        *p.path_edit = None;
    }
}

/// Fija el rango de selección/cursor del TextEdit `te_id` (en CHARS). Con
/// `start == end` es solo posición de cursor; con `0..len` selecciona todo.
fn set_cursor_range(ctx: &egui::Context, te_id: egui::Id, start: usize, end: usize) {
    let mut state = egui::text_edit::TextEditState::load(ctx, te_id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    state.store(ctx, te_id);
}
