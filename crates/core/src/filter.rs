// Naygo — filtros de columna del file panel (puros, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Filtros por columna y su combinación (AND). `matches` decide si un `Entry`
//! pasa TODOS los filtros activos. Puro y testeable; el recorrido lo hace la UI
//! sobre las entries en memoria (lineal, barato).

use crate::columns::ColumnKind;
use crate::fs_model::Entry;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

/// Filtro de una columna, según su tipo de dato.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColumnFilter {
    /// Nombre contiene una subcadena.
    Text {
        contains: String,
        case_sensitive: bool,
    },
    /// Extensión dentro del conjunto marcado (minúsculas; "" = sin extensión).
    Extensions(BTreeSet<String>),
    /// Tamaño en bytes dentro de [min, max] (None = sin límite ese lado).
    SizeRange { min: Option<u64>, max: Option<u64> },
    /// Fecha (modificación o creación, según la columna) dentro de [from, to].
    DateRange {
        from: Option<SystemTime>,
        to: Option<SystemTime>,
    },
}

/// Extensión de un `Entry` en minúsculas; "" si no tiene.
pub fn entry_extension(entry: &Entry) -> String {
    entry
        .path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// `true` si `entry` pasa TODOS los filtros activos (AND). Sin filtros = pasa.
pub fn matches(entry: &Entry, filters: &BTreeMap<ColumnKind, ColumnFilter>) -> bool {
    filters.iter().all(|(kind, f)| match_one(entry, *kind, f))
}

/// Evalúa un único filtro contra un entry.
fn match_one(entry: &Entry, kind: ColumnKind, f: &ColumnFilter) -> bool {
    match f {
        ColumnFilter::Text {
            contains,
            case_sensitive,
        } => {
            if contains.is_empty() {
                return true;
            }
            if *case_sensitive {
                entry.name.contains(contains.as_str())
            } else {
                entry.name.to_lowercase().contains(&contains.to_lowercase())
            }
        }
        ColumnFilter::Extensions(set) => {
            if set.is_empty() {
                return true;
            }
            set.contains(&entry_extension(entry))
        }
        ColumnFilter::SizeRange { min, max } => {
            let Some(size) = entry.size else {
                return false;
            };
            min.map(|m| size >= m).unwrap_or(true) && max.map(|m| size <= m).unwrap_or(true)
        }
        ColumnFilter::DateRange { from, to } => {
            let value = match kind {
                ColumnKind::Created => entry.created,
                _ => entry.modified,
            };
            let Some(t) = value else {
                return false;
            };
            from.map(|f| t >= f).unwrap_or(true) && to.map(|f| t <= f).unwrap_or(true)
        }
    }
}

/// Cuenta cuántos entries hay por extensión (para el filtro de tipos). "" = sin
/// extensión.
pub fn extension_counts(entries: &[Entry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for e in entries {
        *counts.entry(entry_extension(e)).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_model::EntryKind;
    use std::path::PathBuf;
    use std::time::Duration;

    fn entry(name: &str, size: Option<u64>) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind: if size.is_some() {
                EntryKind::File
            } else {
                EntryKind::Directory
            },
            size,
            modified: None,
            created: None,
            hidden: false,
            system: false,
        }
    }

    fn no_filters() -> BTreeMap<ColumnKind, ColumnFilter> {
        BTreeMap::new()
    }

    #[test]
    fn sin_filtros_pasa_todo() {
        assert!(matches(&entry("a.txt", Some(1)), &no_filters()));
    }

    #[test]
    fn text_contains_case_insensitive() {
        let mut f = no_filters();
        f.insert(
            ColumnKind::Name,
            ColumnFilter::Text {
                contains: "INFORME".into(),
                case_sensitive: false,
            },
        );
        assert!(matches(&entry("informe_final.pdf", Some(1)), &f));
        assert!(!matches(&entry("notas.txt", Some(1)), &f));
    }

    #[test]
    fn text_contains_case_sensitive() {
        let mut f = no_filters();
        f.insert(
            ColumnKind::Name,
            ColumnFilter::Text {
                contains: "Informe".into(),
                case_sensitive: true,
            },
        );
        assert!(matches(&entry("Informe.pdf", Some(1)), &f));
        assert!(!matches(&entry("informe.pdf", Some(1)), &f));
    }

    #[test]
    fn extensions_set_vacio_pasa_todo() {
        let mut f = no_filters();
        f.insert(
            ColumnKind::Extension,
            ColumnFilter::Extensions(BTreeSet::new()),
        );
        assert!(matches(&entry("a.txt", Some(1)), &f));
    }

    #[test]
    fn extensions_marca_tipos() {
        let mut set = BTreeSet::new();
        set.insert("pdf".to_string());
        set.insert("".to_string());
        let mut f = no_filters();
        f.insert(ColumnKind::Extension, ColumnFilter::Extensions(set));
        assert!(matches(&entry("doc.pdf", Some(1)), &f));
        assert!(matches(&entry("LEEME", Some(1)), &f));
        assert!(!matches(&entry("img.jpg", Some(1)), &f));
    }

    #[test]
    fn size_range_bordes_y_carpetas_fuera() {
        let mut f = no_filters();
        f.insert(
            ColumnKind::Size,
            ColumnFilter::SizeRange {
                min: Some(10),
                max: Some(100),
            },
        );
        assert!(matches(&entry("a", Some(10)), &f));
        assert!(matches(&entry("b", Some(100)), &f));
        assert!(!matches(&entry("c", Some(9)), &f));
        assert!(!matches(&entry("d", Some(101)), &f));
        assert!(!matches(&entry("dir", None), &f));
    }

    #[test]
    fn date_range_modified() {
        let base = SystemTime::UNIX_EPOCH;
        let mut e = entry("a", Some(1));
        e.modified = Some(base + Duration::from_secs(50));
        let mut f = no_filters();
        f.insert(
            ColumnKind::Modified,
            ColumnFilter::DateRange {
                from: Some(base + Duration::from_secs(10)),
                to: Some(base + Duration::from_secs(100)),
            },
        );
        assert!(matches(&e, &f));
        let mut e2 = entry("b", Some(1));
        e2.modified = Some(base + Duration::from_secs(5));
        assert!(!matches(&e2, &f));
    }

    #[test]
    fn multi_filtro_es_interseccion_and() {
        let mut set = BTreeSet::new();
        set.insert("pdf".to_string());
        let mut f = no_filters();
        f.insert(ColumnKind::Extension, ColumnFilter::Extensions(set));
        f.insert(
            ColumnKind::Name,
            ColumnFilter::Text {
                contains: "informe".into(),
                case_sensitive: false,
            },
        );
        assert!(matches(&entry("informe.pdf", Some(1)), &f));
        assert!(!matches(&entry("informe.txt", Some(1)), &f));
        assert!(!matches(&entry("otro.pdf", Some(1)), &f));
    }

    #[test]
    fn extension_counts_cuenta_por_tipo() {
        let entries = vec![
            entry("a.txt", Some(1)),
            entry("b.txt", Some(1)),
            entry("c.pdf", Some(1)),
            entry("LEEME", Some(1)),
        ];
        let counts = extension_counts(&entries);
        assert_eq!(counts.get("txt"), Some(&2));
        assert_eq!(counts.get("pdf"), Some(&1));
        assert_eq!(counts.get(""), Some(&1));
    }
}
