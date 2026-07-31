// Naygo — modelos de lista ESTABLES de la ventana (panes, splitters, por-panel).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Slint es modo retenido: un `for p in root.panes` recrea un panel por cada ELEMENTO del
// modelo. Si se reemplaza el VecModel entero en cada refresco, Slint destruye y recrea cada
// panel + sus ListView en cada tick → se pierde el scroll y se cortan los gestos. Por eso
// estos modelos son ESTABLES y se mutan in situ (ver `sync.rs`). Extraídos de `main.rs` sin
// cambio de comportamiento (refactor por tamaño de archivo).

use crate::*;
use naygo_core::workspace::layout::Rect;
use naygo_core::workspace::{PaneId, PanePurpose};
use std::collections::HashMap;
use std::rc::Rc;

/// Modelos de lista ESTABLES de un panel (solo el que aplica a su tipo se usa).
pub(crate) struct PaneModels {
    pub(crate) rows: Rc<VecModel<RowData>>,
    /// Columnas visibles del panel Files (6C): se actualizan in situ como las filas.
    pub(crate) columns: Rc<VecModel<ColumnVm>>,
    /// TODAS las columnas (para el menú agregar/quitar) (6C).
    pub(crate) col_menu: Rc<VecModel<ColumnToggleVm>>,
    pub(crate) tree: Rc<VecModel<TreeRow>>,
    pub(crate) favs: Rc<VecModel<NavRow>>,
    pub(crate) recents: Rc<VecModel<NavRow>>,
    /// Árbol de favoritos editable (panel Favoritos): grupos + hojas aplanados con sangría.
    pub(crate) fav_tree: Rc<VecModel<FavTreeRow>>,
    pub(crate) hist: Rc<VecModel<HistRow>>,
}

impl PaneModels {
    pub(crate) fn new() -> PaneModels {
        PaneModels {
            rows: Rc::new(VecModel::default()),
            columns: Rc::new(VecModel::default()),
            col_menu: Rc::new(VecModel::default()),
            tree: Rc::new(VecModel::default()),
            favs: Rc::new(VecModel::default()),
            recents: Rc::new(VecModel::default()),
            fav_tree: Rc::new(VecModel::default()),
            hist: Rc::new(VecModel::default()),
        }
    }
}

/// Modelos estables que persisten entre refrescos (ver nota de cabecera).
pub(crate) struct Models {
    pub(crate) panes: Rc<VecModel<PaneVm>>,
    pub(crate) splits: Rc<VecModel<SplitVm>>,
    /// Candidatos del selector de panel destino (vacío = sin selector).
    pub(crate) picks: Rc<VecModel<PickVm>>,
    /// Modelos de lista estables por panel (se actualizan in situ, no se recrean).
    pub(crate) per_pane: HashMap<PaneId, PaneModels>,
    /// IDs de panel VISIBLES en el orden actual del modelo `panes`.
    pub(crate) pane_ids: Vec<PaneId>,
    /// Grupos de pestañas con los que se construyó la estructura (para detectar cambios de
    /// agrupación que no alteran los ids visibles, p. ej. activar otra pestaña).
    pub(crate) groups: Vec<(Vec<PaneId>, usize)>,
    /// Área con la que se construyó la estructura actual (para detectar resize).
    pub(crate) area: Rect,
}

impl Models {
    pub(crate) fn new() -> Models {
        Models {
            panes: Rc::new(VecModel::default()),
            splits: Rc::new(VecModel::default()),
            picks: Rc::new(VecModel::default()),
            per_pane: HashMap::new(),
            pane_ids: Vec::new(),
            groups: Vec::new(),
            area: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
        }
    }

    pub(crate) fn models_for(&mut self, id: PaneId) -> &PaneModels {
        self.per_pane.entry(id).or_insert_with(PaneModels::new)
    }
}

pub(crate) fn rects_equal(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 0.5
        && (a.y - b.y).abs() < 0.5
        && (a.w - b.w).abs() < 0.5
        && (a.h - b.h).abs() < 0.5
}

pub(crate) fn purpose_to_int(p: PanePurpose) -> i32 {
    match p {
        PanePurpose::Files => 0,
        PanePurpose::Tree => 1,
        PanePurpose::Inspector => 2,
        PanePurpose::History => 3,
        PanePurpose::Favorites => 4,
        PanePurpose::Preview => 5,
        PanePurpose::Operations => 6,
    }
}
