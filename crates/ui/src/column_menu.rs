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

/// Convierte (año, mes 1-12, día 1-31) a un `SystemTime` (medianoche UTC de ese
/// día), usando solo `std::time`. `None` si la fecha es inválida o anterior a 1970.
pub fn ymd_to_system_time(year: i32, month: u32, day: u32) -> Option<std::time::SystemTime> {
    let days = days_from_civil(year, month, day)?;
    let secs = (days as u64).checked_mul(86_400)?;
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

/// Días desde 1970-01-01 para una fecha civil válida (proleptic Gregorian).
/// `None` si la fecha es inválida (mes/día fuera de rango, año < 1970, día > días
/// del mes). Algoritmo de días civiles de Howard Hinnant.
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year < 1970 {
        return None;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let dim = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day > dim[(month - 1) as usize] {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m = month as i64;
    let d = day as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    Some(era * 146097 + doe - 719468)
}

/// (año, mes, día) de un `SystemTime` (UTC), para mostrar un `DateRange` en los
/// controles. `(0,0,0)` si es anterior a la época.
pub fn system_time_to_ymd(t: std::time::SystemTime) -> (i32, u32, u32) {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    civil_from_days(days)
}

/// Inverso de `days_from_civil`: días desde 1970-01-01 → (año, mes, día).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u32, d as u32)
}

/// Acción al fijar un rango de fecha para una columna (Modified/Created). Ambos
/// None → quitar filtro.
pub fn date_filter_action(
    kind: ColumnKind,
    from: Option<std::time::SystemTime>,
    to: Option<std::time::SystemTime>,
) -> TableAction {
    if from.is_none() && to.is_none() {
        TableAction::ClearFilter(kind)
    } else {
        TableAction::SetFilter(kind, ColumnFilter::DateRange { from, to })
    }
}

// ---------------------------------------------------------------------------
// Render del desplegable (egui). La lógica de qué acción emitir vive en las
// funciones puras de arriba; aquí solo dibujamos. El estado mostrado por los
// controles de filtro se deriva de `table.filters` (fuente de verdad) cada frame,
// no de `egui::Memory`: así no hay desync entre lo que se ve y lo que filtra.
// ---------------------------------------------------------------------------

/// Dibuja el contenido del desplegable de una columna: botones de orden, una
/// sección plegable "Filtrar…", una sección plegable "Columnas…" y un botón
/// "Quitar filtro" si la columna tiene uno activo. Acumula `TableAction`s.
#[allow(clippy::too_many_arguments)]
pub fn show_menu(
    ui: &mut egui::Ui,
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
        filter_controls(ui, kind, table, ext_counts, i18n, actions);
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
///
/// FUENTE DE VERDAD: `table.filters`. El estado mostrado por cada control se
/// deriva del filtro activo cada frame (patrón "controlled"), NO de la memoria de
/// egui. La memoria de egui no se reconciliaba con `table.filters`, lo que causaba
/// (1) resurrección del filtro tras "Quitar filtro" y (2) controles en blanco con
/// filtros persistidos/restaurados. El patrón controlled funciona porque la acción
/// emitida se aplica a `table.filters` entre frames (en app.rs, tras cada `ui()`),
/// y al frame siguiente el control se re-siembra ya con el valor nuevo; egui
/// conserva foco/cursor por id de widget, así que se puede escribir con fluidez.
fn filter_controls(
    ui: &mut egui::Ui,
    kind: ColumnKind,
    table: &TableState,
    ext_counts: &BTreeMap<String, usize>,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<TableAction>,
) {
    match kind {
        ColumnKind::Name => {
            // Estado actual = la verdad (table.filters), no la memoria.
            let (mut text, mut case) = match table.filters.get(&ColumnKind::Name) {
                Some(ColumnFilter::Text {
                    contains,
                    case_sensitive,
                }) => (contains.clone(), *case_sensitive),
                _ => (String::new(), false),
            };

            ui.label(i18n.t("filter.name_contains"));
            let mut changed = ui.text_edit_singleline(&mut text).changed();
            changed |= ui
                .checkbox(&mut case, i18n.t("filter.case_sensitive"))
                .changed();
            if changed {
                actions.push(name_filter_action(&text, case)); // vacío → ClearFilter
            }
        }
        ColumnKind::Extension => {
            // Conjunto marcado derivado del filtro activo (no de la memoria).
            let mut selected: BTreeSet<String> = match table.filters.get(&ColumnKind::Extension) {
                Some(ColumnFilter::Extensions(s)) => s.clone(),
                _ => BTreeSet::new(),
            };

            ui.label(i18n.t("filter.search_type"));
            let mut changed = false;
            for (ext, count) in ext_counts {
                let label = if ext.is_empty() {
                    format!("{} ({count})", i18n.t("filter.no_extension"))
                } else {
                    format!(".{ext} ({count})")
                };
                let mut on = selected.contains(ext);
                if ui.checkbox(&mut on, label).changed() {
                    if on {
                        selected.insert(ext.clone());
                    } else {
                        selected.remove(ext);
                    }
                    changed = true;
                }
            }
            if changed {
                actions.push(extensions_filter_action(selected)); // vacío → ClearFilter
            }
        }
        ColumnKind::Size => {
            // Texto de KB derivado del filtro actual (bytes → KB). Round-trip por
            // 1024: la entrada ES en KB, así que es consistente (500 KB → 512000 B
            // → 500 KB). Sin memoria: la verdad es table.filters.
            let (mut from_txt, mut to_txt) = match table.filters.get(&ColumnKind::Size) {
                Some(ColumnFilter::SizeRange { min, max }) => (
                    min.map(|b| (b / 1024).to_string()).unwrap_or_default(),
                    max.map(|b| (b / 1024).to_string()).unwrap_or_default(),
                ),
                _ => (String::new(), String::new()),
            };

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
                let min = parse_kb(&from_txt);
                let max = parse_kb(&to_txt);
                actions.push(size_filter_action(min, max));
            }
        }
        ColumnKind::Modified | ColumnKind::Created => {
            // Estado actual desde table.filters (fuente de verdad). Cada extremo se
            // muestra como (año, mes, día); (0,0,0) = sin límite por ese lado.
            // Un extremo solo aplica si y/m/d forman una fecha completa válida
            // (mes/día = 0 → ymd_to_system_time devuelve None → sin límite).
            let (mut from, mut to) = match table.filters.get(&kind) {
                Some(ColumnFilter::DateRange { from, to }) => (
                    from.map(system_time_to_ymd).unwrap_or((0, 0, 0)),
                    to.map(system_time_to_ymd).unwrap_or((0, 0, 0)),
                ),
                _ => ((0, 0, 0), (0, 0, 0)),
            };
            let mut changed = false;
            ui.label(i18n.t("filter.date_from"));
            ui.horizontal(|ui| {
                changed |= ui
                    .add(egui::DragValue::new(&mut from.0).range(0..=9999))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut from.1).range(0..=12))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut from.2).range(0..=31))
                    .changed();
            });
            ui.label(i18n.t("filter.date_to"));
            ui.horizontal(|ui| {
                changed |= ui
                    .add(egui::DragValue::new(&mut to.0).range(0..=9999))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut to.1).range(0..=12))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut to.2).range(0..=31))
                    .changed();
            });
            if changed {
                let f = ymd_to_system_time(from.0, from.1, from.2);
                let t = ymd_to_system_time(to.0, to.1, to.2);
                actions.push(date_filter_action(kind, f, t));
            }
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

    #[test]
    fn ymd_epoch_es_cero_dias() {
        assert_eq!(
            ymd_to_system_time(1970, 1, 1).unwrap(),
            std::time::UNIX_EPOCH
        );
    }
    #[test]
    fn ymd_un_dia_despues() {
        assert_eq!(
            ymd_to_system_time(1970, 1, 2).unwrap(),
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(86_400)
        );
    }
    #[test]
    fn ymd_rechaza_fecha_invalida() {
        assert!(ymd_to_system_time(2021, 2, 29).is_none()); // no bisiesto
        assert!(ymd_to_system_time(2020, 2, 29).is_some()); // bisiesto
        assert!(ymd_to_system_time(2021, 13, 1).is_none());
        assert!(ymd_to_system_time(1969, 1, 1).is_none());
    }
    #[test]
    fn ymd_roundtrip() {
        for (y, m, d) in [(1970, 1, 1), (2000, 2, 29), (2026, 6, 7), (1999, 12, 31)] {
            let t = ymd_to_system_time(y, m, d).unwrap();
            assert_eq!(system_time_to_ymd(t), (y, m, d), "roundtrip {y}-{m}-{d}");
        }
    }
    #[test]
    fn date_filter_ambos_none_quita() {
        assert_eq!(
            date_filter_action(ColumnKind::Modified, None, None),
            TableAction::ClearFilter(ColumnKind::Modified)
        );
    }
}
