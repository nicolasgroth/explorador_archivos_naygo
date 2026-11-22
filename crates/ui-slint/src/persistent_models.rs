// Naygo — modelos persistentes de la ventana y caché del VM de vista previa.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// `sync_rows` corre en cada tick de 30 ms mientras hay trabajo (por ejemplo, durante los 8 s de
// resaltado de un archivo recién llegado). Antes reconstruía en cada llamada varios
// `ModelRc::from(Rc::new(VecModel::from(..)))`: como `ModelRc` se compara por identidad, cada
// modelo nuevo marcaba sucias las propiedades que lo contienen (también structs como
// `ColumnMenuVm`, `BatchRenameVm` o el `InspectorVm` del panel) y el renderer por software
// repintaba overlays de ventana completa en cada tick. Medido en idle: 100 % de un núcleo durante
// esos 8 s. Aquí viven modelos que se crean UNA vez y se actualizan in situ; con el mismo
// `ModelRc`, `Property::set` de Slint detecta el valor igual y no notifica nada.

use crate::workspace_ctrl::WorkspaceCtrl;
use crate::*;
use slint::{Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// Actualiza `model` para que contenga `rows` notificando lo mínimo: con el mismo largo, solo
/// `set_row_data` en las filas distintas (los delegates sobreviven y el hover sigue válido); si el
/// largo cambió, `set_vec` (el contenido cambió de verdad). Sin cambios no emite nada.
pub(crate) fn update_rows<T: PartialEq + Clone + 'static>(model: &VecModel<T>, rows: Vec<T>) {
    if model.row_count() != rows.len() {
        model.set_vec(rows);
        return;
    }
    for (index, row) in rows.into_iter().enumerate() {
        if model.row_data(index).as_ref() != Some(&row) {
            model.set_row_data(index, row);
        }
    }
}

/// `ModelRc` que apunta al modelo persistente. Asignarlo a una propiedad que ya lo tiene no
/// notifica (`ModelRc` se compara por puntero).
pub(crate) fn model_rc<T: Clone + 'static>(model: &Rc<VecModel<T>>) -> ModelRc<T> {
    ModelRc::from(model.clone())
}

/// Modelos de nivel ventana que `sync_rows` mantiene entre llamadas.
pub(crate) struct PersistentModels {
    pub(crate) fav_tree: Rc<VecModel<FavTreeRow>>,
    pub(crate) fav_group_options: Rc<VecModel<PathSeg>>,
    pub(crate) op_running: Rc<VecModel<OpRowVm>>,
    pub(crate) op_queued: Rc<VecModel<OpRowVm>>,
    pub(crate) op_history: Rc<VecModel<OpRowVm>>,
    pub(crate) op_planning: Rc<VecModel<OpRowVm>>,
    pub(crate) resume: Rc<VecModel<ResumeRowVm>>,
    pub(crate) clipboard_history: Rc<VecModel<ClipboardHistoryRowVm>>,
    pub(crate) help: Rc<VecModel<HelpRowVm>>,
    /// Idioma con el que se armó la ayuda y si estaba abierta en la llamada anterior: la ayuda
    /// se reconstruye solo al cambiar el idioma o al abrirse (el keymap se edita con ella cerrada).
    pub(crate) help_language: Option<naygo_core::i18n::LangId>,
    pub(crate) help_was_open: bool,
    pub(crate) spaces_recents: Rc<VecModel<SharedString>>,
    pub(crate) column_exts: Rc<VecModel<ExtRowVm>>,
    pub(crate) batch_rows: Rc<VecModel<BatchRowVm>>,
    pub(crate) sync_rows: Rc<VecModel<SyncRowVm>>,
    pub(crate) inspector_meta: Rc<VecModel<MetaFieldVm>>,
    pub(crate) search_hits: Rc<VecModel<SearchHitVm>>,
    pub(crate) points: Rc<VecModel<PointRowVm>>,
}

impl PersistentModels {
    pub(crate) fn new() -> Self {
        Self {
            fav_tree: Rc::default(),
            fav_group_options: Rc::default(),
            op_running: Rc::default(),
            op_queued: Rc::default(),
            op_history: Rc::default(),
            op_planning: Rc::default(),
            resume: Rc::default(),
            clipboard_history: Rc::default(),
            help: Rc::default(),
            help_language: None,
            help_was_open: false,
            spaces_recents: Rc::default(),
            column_exts: Rc::default(),
            batch_rows: Rc::default(),
            sync_rows: Rc::default(),
            inspector_meta: Rc::default(),
            search_hits: Rc::default(),
            points: Rc::default(),
        }
    }

    /// `true` si hay que reconstruir la ayuda: al abrirse o al cambiar el idioma con ella abierta.
    pub(crate) fn help_needs_rebuild(
        &mut self,
        open: bool,
        language: &naygo_core::i18n::LangId,
    ) -> bool {
        let opened = open && !self.help_was_open;
        self.help_was_open = open;
        let language_changed = self.help_language.as_ref() != Some(language);
        if open && (opened || language_changed) {
            self.help_language = Some(language.clone());
            return true;
        }
        false
    }
}

/// Caché del `PreviewVm`. La parte pesada (texto, imagen, líneas resaltadas con sus spans) se
/// arma una vez por revisión de la vista (`PreviewState::view_revision`) e idioma; los campos
/// baratos (ruta, estado de carga, metadata) se completan en cada llamada. Lo comparten
/// `sync_rows` y el timer de animación, por eso vive en `Models`.
#[derive(Default)]
pub(crate) struct PreviewVmCache {
    revision: Option<u64>,
    language: Option<naygo_core::i18n::LangId>,
    base: PreviewVm,
    meta: Rc<VecModel<MetaFieldVm>>,
}

impl PreviewVmCache {
    pub(crate) fn vm(&mut self, c: &WorkspaceCtrl) -> PreviewVm {
        let revision = c.preview.view_revision();
        let language = &c.config.settings.language;
        if self.revision != Some(revision) || self.language.as_ref() != Some(language) {
            self.base = crate::vm_builders::preview_base_vm(c);
            self.revision = Some(revision);
            self.language = Some(language.clone());
        }
        update_rows(&self.meta, crate::vm_builders::meta_fields(c));
        let mut vm = self.base.clone();
        crate::vm_builders::fill_preview_state(&mut vm, c, model_rc(&self.meta));
        vm
    }
}

/// Igualdad barata entre dos `PreviewVm` salidos de `PreviewVmCache`. La base pesada (texto,
/// imagen, mensaje, líneas resaltadas) se identifica por el puntero de `hl_lines`: cada
/// reconstrucción de la base crea un modelo nuevo (aunque esté vacío), así que el mismo puntero
/// implica la misma base sin comparar el texto ni los píxeles de la imagen en cada tick.
pub(crate) fn preview_vm_eq(left: &PreviewVm, right: &PreviewVm) -> bool {
    left.hl_lines == right.hl_lines
        && left.path == right.path
        && left.meta == right.meta
        && left.meta_loading == right.meta_loading
        && left.mesh_interactive == right.mesh_interactive
        && left.mesh_busy == right.mesh_busy
        && left.mesh_color == right.mesh_color
        && left.mesh_surface == right.mesh_surface
        && left.loading == right.loading
        && left.can_cancel == right.can_cancel
        && left.can_retry == right.can_retry
        && left.animation_state == right.animation_state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_rows_sin_cambios_no_toca_el_modelo() {
        let model = VecModel::from(vec![1, 2, 3]);
        update_rows(&model, vec![1, 2, 3]);
        assert_eq!(model.iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn update_rows_mismo_largo_cambia_solo_la_fila_distinta() {
        let model = VecModel::from(vec![1, 2, 3]);
        update_rows(&model, vec![1, 9, 3]);
        assert_eq!(model.iter().collect::<Vec<_>>(), vec![1, 9, 3]);
    }

    #[test]
    fn update_rows_otro_largo_reemplaza() {
        let model = VecModel::from(vec![1, 2, 3]);
        update_rows(&model, vec![4]);
        assert_eq!(model.iter().collect::<Vec<_>>(), vec![4]);
        update_rows(&model, Vec::new());
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn ayuda_se_reconstruye_al_abrir_o_cambiar_idioma() {
        let es = naygo_core::i18n::LangId("es".into());
        let en = naygo_core::i18n::LangId("en".into());
        let mut models = PersistentModels::new();
        assert!(!models.help_needs_rebuild(false, &es));
        assert!(models.help_needs_rebuild(true, &es));
        assert!(!models.help_needs_rebuild(true, &es));
        assert!(models.help_needs_rebuild(true, &en));
        assert!(!models.help_needs_rebuild(false, &en));
        assert!(models.help_needs_rebuild(true, &en));
    }
}
