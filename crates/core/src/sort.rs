// Naygo — ordenamiento puro de entradas de un panel.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Ordena un `Vec<Entry>` según un `SortSpec`. Función pura: misma entrada,
//! misma salida; sin I/O. Comparación de nombres case-insensitive (estilo
//! Windows). El orden es estable para que reordenar no "salte".

use crate::fs_model::{Entry, SortKey, SortSpec};

/// Ordena `entries` in-place según `spec`.
pub fn sort_entries(entries: &mut [Entry], spec: &SortSpec) {
    entries.sort_by(|a, b| {
        // Carpetas primero si así se pidió, sin importar la clave.
        if spec.dirs_first {
            let a_dir = a.is_dir();
            let b_dir = b.is_dir();
            if a_dir != b_dir {
                // dir (true) debe ir antes que archivo (false).
                return b_dir.cmp(&a_dir);
            }
        }

        let ordering = match spec.key {
            SortKey::Name => cmp_name(a, b),
            SortKey::Extension => cmp_extension(a, b),
            SortKey::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Created => a.created.cmp(&b.created),
            SortKey::Kind => format!("{:?}", a.kind).cmp(&format!("{:?}", b.kind)),
        };

        if spec.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    });
}

/// Comparación de nombres case-insensitive estilo Windows.
fn cmp_name(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    a.name.to_lowercase().cmp(&b.name.to_lowercase())
}

/// Comparación por extensión del path, case-insensitive. Sin extensión = vacío.
/// Usa folding ASCII (`to_ascii_lowercase`) a propósito: las extensiones reales
/// son ASCII y así se evita el costo Unicode de `to_lowercase` (a diferencia de
/// `cmp_name`, que sí maneja nombres Unicode).
fn cmp_extension(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    let ext = |e: &Entry| {
        e.path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
    };
    ext(a).cmp(&ext(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_model::EntryKind;
    use std::path::PathBuf;

    fn entry(name: &str, kind: EntryKind, size: u64) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind,
            size: Some(size),
            modified: None,
            created: None,
            hidden: false,
        }
    }

    #[test]
    fn por_nombre_ascendente_case_insensitive() {
        let mut v = vec![
            entry("banana.txt", EntryKind::File, 1),
            entry("Apple.txt", EntryKind::File, 1),
            entry("cherry.txt", EntryKind::File, 1),
        ];
        let spec = SortSpec {
            key: SortKey::Name,
            ascending: true,
            dirs_first: false,
        };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["Apple.txt", "banana.txt", "cherry.txt"]);
    }

    #[test]
    fn dirs_first_pone_carpetas_arriba() {
        let mut v = vec![
            entry("zeta.txt", EntryKind::File, 1),
            entry("alpha_dir", EntryKind::Directory, 0),
        ];
        let spec = SortSpec {
            key: SortKey::Name,
            ascending: true,
            dirs_first: true,
        };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["alpha_dir", "zeta.txt"],
            "la carpeta va primero"
        );
    }

    #[test]
    fn por_tamano_descendente() {
        let mut v = vec![
            entry("small", EntryKind::File, 10),
            entry("big", EntryKind::File, 100),
            entry("mid", EntryKind::File, 50),
        ];
        let spec = SortSpec {
            key: SortKey::Size,
            ascending: false,
            dirs_first: false,
        };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["big", "mid", "small"]);
    }

    #[test]
    fn por_extension_ascendente() {
        let mut v = vec![
            entry("z.txt", EntryKind::File, 1),
            entry("a.zip", EntryKind::File, 1),
            entry("m.jpg", EntryKind::File, 1),
        ];
        let spec = SortSpec {
            key: SortKey::Extension,
            ascending: true,
            dirs_first: false,
        };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["m.jpg", "z.txt", "a.zip"]); // jpg < txt < zip
    }

    #[test]
    fn por_extension_case_insensitive() {
        let mut v = vec![
            entry("b.TXT", EntryKind::File, 1),
            entry("a.zip", EntryKind::File, 1),
        ];
        let spec = SortSpec {
            key: SortKey::Extension,
            ascending: true,
            dirs_first: false,
        };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["b.TXT", "a.zip"]); // txt < zip regardless of case
    }

    #[test]
    fn por_creacion_descendente() {
        use std::time::{Duration, SystemTime};
        let base = SystemTime::UNIX_EPOCH;
        let mut older = entry("viejo", EntryKind::File, 1);
        older.created = Some(base);
        let mut newer = entry("nuevo", EntryKind::File, 1);
        newer.created = Some(base + Duration::from_secs(100));
        let mut v = vec![older, newer];
        let spec = SortSpec {
            key: SortKey::Created,
            ascending: false,
            dirs_first: false,
        };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["nuevo", "viejo"]); // descendente: más nuevo primero
    }

    #[test]
    fn dirs_first_no_se_invierte_en_orden_descendente() {
        // Contrato sutil: con ascending=false, las carpetas DEBEN seguir arriba.
        // El reverse aplica solo dentro de cada grupo (clave), nunca al agrupado
        // dir-vs-archivo. Un refactor que moviera el chequeo dirs_first después
        // del reverse rompería esto y pasaría los otros tests; este lo protege.
        let mut v = vec![
            entry("aaa.txt", EntryKind::File, 1),
            entry("alpha_dir", EntryKind::Directory, 0),
            entry("zeta.txt", EntryKind::File, 1),
            entry("beta_dir", EntryKind::Directory, 0),
        ];
        let spec = SortSpec {
            key: SortKey::Name,
            ascending: false,
            dirs_first: true,
        };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        // Carpetas primero (reversadas por nombre entre sí), luego archivos (reversados).
        assert_eq!(names, vec!["beta_dir", "alpha_dir", "zeta.txt", "aaa.txt"]);
    }
}
