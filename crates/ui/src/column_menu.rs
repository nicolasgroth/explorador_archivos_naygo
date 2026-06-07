// Naygo — lógica pura del menú de columna (qué acción produce cada interacción).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! El render del desplegable llama a estas funciones puras para decidir qué
//! `TableAction` emitir. Separar la decisión del dibujo las hace testeables sin
//! egui. `show_menu` (más abajo) dibuja el desplegable y usa estas funciones.

use crate::table_actions::TableAction;
use naygo_core::columns::{sort_key_of, ColumnKind, TableState};
use naygo_core::filter::ColumnFilter;
use naygo_core::fs_model::SortSpec;
use std::collections::{BTreeMap, BTreeSet};

/// Acción al pedir "ordenar" por una columna en una dirección.
pub fn sort_action(kind: ColumnKind, ascending: bool, dirs_first: bool) -> TableAction {
    TableAction::SetSort(SortSpec {
        key: sort_key_of(kind),
        ascending,
        dirs_first,
    })
}

/// Acción al cambiar el texto del filtro de Nombre. Texto vacío → quitar filtro.
pub fn name_filter_action(contains: &str, case_sensitive: bool) -> TableAction {
    if contains.is_empty() {
        TableAction::ClearFilter(ColumnKind::Name)
    } else {
        TableAction::SetFilter(
            ColumnKind::Name,
            ColumnFilter::Text {
                contains: contains.to_string(),
                case_sensitive,
            },
        )
    }
}

/// Acción al cambiar el conjunto de extensiones marcadas. Vacío → quitar filtro.
pub fn extensions_filter_action(selected: std::collections::BTreeSet<String>) -> TableAction {
    if selected.is_empty() {
        TableAction::ClearFilter(ColumnKind::Extension)
    } else {
        TableAction::SetFilter(ColumnKind::Extension, ColumnFilter::Extensions(selected))
    }
}

/// Acción al fijar un rango de tamaño. Ambos None → quitar filtro.
pub fn size_filter_action(min: Option<u64>, max: Option<u64>) -> TableAction {
    if min.is_none() && max.is_none() {
        TableAction::ClearFilter(ColumnKind::Size)
    } else {
        TableAction::SetFilter(ColumnKind::Size, ColumnFilter::SizeRange { min, max })
    }
}

/// Unidad de tamaño para los controles de filtro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeUnit {
    Kb,
    // Mb/Gb: selector de unidad futuro; hoy los controles usan KB. Tested.
    #[allow(dead_code)]
    Mb,
    #[allow(dead_code)]
    Gb,
}

/// Convierte un valor + unidad (KB/MB/GB) a bytes. Para los controles de tamaño.
pub fn to_bytes(value: f64, unit: SizeUnit) -> u64 {
    let mult = match unit {
        SizeUnit::Kb => 1024.0,
        SizeUnit::Mb => 1024.0 * 1024.0,
        SizeUnit::Gb => 1024.0 * 1024.0 * 1024.0,
    };
    (value * mult).max(0.0) as u64
}

// ---------------------------------------------------------------------------
// Render del desplegable (egui). La lógica de qué acción emitir vive en las
// funciones puras de arriba; aquí solo dibujamos y leemos/guardamos el estado
// transitorio de los controles en `egui::Memory` (clave incluye el PaneId para
// que dos paneles no compartan el estado de UI del mismo tipo de columna).
// ---------------------------------------------------------------------------

/// Dibuja el contenido del desplegable de una columna: botones de orden, una
/// sección plegable "Filtrar…", una sección plegable "Columnas…" y un botón
/// "Quitar filtro" si la columna tiene uno activo. Acumula `TableAction`s.
#[allow(clippy::too_many_arguments)]
pub fn show_menu(
    ui: &mut egui::Ui,
    pane_id: u64,
    kind: ColumnKind,
    table: &TableState,
    sort: SortSpec,
    ext_counts: &BTreeMap<String, usize>,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<TableAction>,
) {
    ui.set_min_width(220.0);

    // Orden directo (ascendente / descendente). `dirs_first` se conserva del
    // estado actual para no cambiar el comportamiento de carpetas-primero.
    if ui.button(i18n.t("menu.sort_asc")).clicked() {
        actions.push(sort_action(kind, true, sort.dirs_first));
        ui.close();
    }
    if ui.button(i18n.t("menu.sort_desc")).clicked() {
        actions.push(sort_action(kind, false, sort.dirs_first));
        ui.close();
    }

    ui.separator();

    // Filtrar… (sub-sección plegable, depende del tipo de columna).
    ui.collapsing(i18n.t("menu.filter"), |ui| {
        filter_controls(ui, pane_id, kind, ext_counts, i18n, actions);
    });

    // Columnas… (mostrar/ocultar columnas; Nombre deshabilitado).
    ui.collapsing(i18n.t("menu.columns"), |ui| {
        columns_controls(ui, table, i18n, actions);
    });

    // Quitar filtro de esta columna (solo si hay uno activo).
    if table.filters.contains_key(&kind) {
        ui.separator();
        if ui.button(i18n.t("menu.clear_filter")).clicked() {
            actions.push(TableAction::ClearFilter(kind));
        }
    }
}

/// Controles de filtro según el tipo de columna. LIVE: cualquier cambio emite la
/// acción correspondiente en el acto.
fn filter_controls(
    ui: &mut egui::Ui,
    pane_id: u64,
    kind: ColumnKind,
    ext_counts: &BTreeMap<String, usize>,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<TableAction>,
) {
    match kind {
        ColumnKind::Name => {
            // Estado del texto y del flag case-sensitive en memoria (por panel+columna).
            let text_id = ui.make_persistent_id(("col_filter_name_text", pane_id, kind));
            let case_id = ui.make_persistent_id(("col_filter_name_case", pane_id, kind));
            let mut text: String = ui
                .memory(|m| m.data.get_temp::<String>(text_id))
                .unwrap_or_default();
            let mut case: bool = ui
                .memory(|m| m.data.get_temp::<bool>(case_id))
                .unwrap_or(false);

            ui.label(i18n.t("filter.name_contains"));
            let mut changed = false;
            if ui.text_edit_singleline(&mut text).changed() {
                changed = true;
            }
            if ui
                .checkbox(&mut case, i18n.t("filter.case_sensitive"))
                .changed()
            {
                changed = true;
            }
            if changed {
                ui.memory_mut(|m| m.data.insert_temp(text_id, text.clone()));
                ui.memory_mut(|m| m.data.insert_temp(case_id, case));
                actions.push(name_filter_action(&text, case));
            }
        }
        ColumnKind::Extension => {
            // Conjunto de extensiones marcadas en memoria (por panel+columna).
            let set_id = ui.make_persistent_id(("col_filter_ext_set", pane_id, kind));
            let mut set: BTreeSet<String> = ui
                .memory(|m| m.data.get_temp::<BTreeSet<String>>(set_id))
                .unwrap_or_default();

            ui.label(i18n.t("filter.search_type"));
            let mut changed = false;
            for (ext, count) in ext_counts {
                let label = if ext.is_empty() {
                    format!("{} ({count})", i18n.t("filter.no_extension"))
                } else {
                    format!(".{ext} ({count})")
                };
                let mut checked = set.contains(ext);
                if ui.checkbox(&mut checked, label).changed() {
                    if checked {
                        set.insert(ext.clone());
                    } else {
                        set.remove(ext);
                    }
                    changed = true;
                }
            }
            if changed {
                ui.memory_mut(|m| m.data.insert_temp(set_id, set.clone()));
                actions.push(extensions_filter_action(set));
            }
        }
        ColumnKind::Size => {
            // Dos campos (desde/hasta) en KB; el estado de texto vive en memoria.
            let from_id = ui.make_persistent_id(("col_filter_size_from", pane_id, kind));
            let to_id = ui.make_persistent_id(("col_filter_size_to", pane_id, kind));
            let mut from_txt: String = ui
                .memory(|m| m.data.get_temp::<String>(from_id))
                .unwrap_or_default();
            let mut to_txt: String = ui
                .memory(|m| m.data.get_temp::<String>(to_id))
                .unwrap_or_default();

            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label(i18n.t("filter.size_from"));
                if ui.text_edit_singleline(&mut from_txt).changed() {
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label(i18n.t("filter.size_to"));
                if ui.text_edit_singleline(&mut to_txt).changed() {
                    changed = true;
                }
            });
            if changed {
                ui.memory_mut(|m| m.data.insert_temp(from_id, from_txt.clone()));
                ui.memory_mut(|m| m.data.insert_temp(to_id, to_txt.clone()));
                let min = parse_kb(&from_txt);
                let max = parse_kb(&to_txt);
                actions.push(size_filter_action(min, max));
            }
        }
        ColumnKind::Modified | ColumnKind::Created => {
            // PLACEHDER (Tarea 8 lo reemplaza por un control de fecha real). No
            // emite ninguna acción. Sin texto hardcodeado: solo claves i18n + "—".
            ui.horizontal(|ui| {
                ui.label(i18n.t("filter.date_from"));
                ui.weak("—");
            });
            ui.horizontal(|ui| {
                ui.label(i18n.t("filter.date_to"));
                ui.weak("—");
            });
        }
    }
}

/// Parsea un campo de texto en KB a bytes. Vacío o inválido → None.
fn parse_kb(s: &str) -> Option<u64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().map(|v| to_bytes(v, SizeUnit::Kb))
}

/// Controles mostrar/ocultar columnas: un checkbox por columna (marcado=visible).
/// Nombre queda deshabilitado (nunca se oculta). Cada cambio emite ToggleColumn.
fn columns_controls(
    ui: &mut egui::Ui,
    table: &TableState,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<TableAction>,
) {
    for col in &table.columns {
        let kind = col.kind;
        let mut visible = col.visible;
        ui.add_enabled_ui(kind != ColumnKind::Name, |ui| {
            if ui
                .checkbox(&mut visible, i18n.t(column_title_key(kind)))
                .changed()
            {
                actions.push(TableAction::ToggleColumn(kind));
            }
        });
    }
}

/// Clave i18n del título de una columna (1:1 con `col.*`).
fn column_title_key(kind: ColumnKind) -> &'static str {
    match kind {
        ColumnKind::Name => "col.name",
        ColumnKind::Extension => "col.extension",
        ColumnKind::Size => "col.size",
        ColumnKind::Modified => "col.modified",
        ColumnKind::Created => "col.created",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::fs_model::SortKey;

    #[test]
    fn sort_action_usa_sortkey_de_la_columna() {
        let a = sort_action(ColumnKind::Extension, true, true);
        match a {
            TableAction::SetSort(spec) => {
                assert_eq!(spec.key, SortKey::Extension);
                assert!(spec.ascending);
                assert!(spec.dirs_first);
            }
            _ => panic!("esperaba SetSort"),
        }
    }

    #[test]
    fn name_filter_vacio_quita_filtro() {
        assert_eq!(
            name_filter_action("", false),
            TableAction::ClearFilter(ColumnKind::Name)
        );
    }

    #[test]
    fn name_filter_con_texto_setea() {
        let a = name_filter_action("doc", true);
        assert_eq!(
            a,
            TableAction::SetFilter(
                ColumnKind::Name,
                ColumnFilter::Text {
                    contains: "doc".into(),
                    case_sensitive: true
                }
            )
        );
    }

    #[test]
    fn extensions_vacio_quita_filtro() {
        assert_eq!(
            extensions_filter_action(BTreeSet::new()),
            TableAction::ClearFilter(ColumnKind::Extension)
        );
    }

    #[test]
    fn size_ambos_none_quita_filtro() {
        assert_eq!(
            size_filter_action(None, None),
            TableAction::ClearFilter(ColumnKind::Size)
        );
    }

    #[test]
    fn to_bytes_convierte_unidades() {
        assert_eq!(to_bytes(1.0, SizeUnit::Kb), 1024);
        assert_eq!(to_bytes(2.0, SizeUnit::Mb), 2 * 1024 * 1024);
        assert_eq!(to_bytes(1.0, SizeUnit::Gb), 1024 * 1024 * 1024);
    }
}
