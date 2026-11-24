// Naygo — comparación pura de listados de una misma carpeta durante la sesión.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use crate::fs_model::Entry;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    New,
    Modified,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListingChange {
    pub path: PathBuf,
    pub kind: ChangeKind,
}

/// Compara los metadatos que ya entrega el listado. Los paths ausentes solo aparecen en el
/// resultado; jamás se convierten en filas del filesystem ni se pueden operar.
pub fn compare(previous: &[Entry], current: &[Entry]) -> Vec<ListingChange> {
    let old: HashMap<&PathBuf, &Entry> = previous.iter().map(|e| (&e.path, e)).collect();
    let now: HashMap<&PathBuf, &Entry> = current.iter().map(|e| (&e.path, e)).collect();
    let mut out = Vec::new();
    for entry in current {
        match old.get(&entry.path) {
            None => out.push(ListingChange {
                path: entry.path.clone(),
                kind: ChangeKind::New,
            }),
            Some(before)
                if before.size != entry.size
                    || before.modified != entry.modified
                    || before.kind != entry.kind =>
            {
                out.push(ListingChange {
                    path: entry.path.clone(),
                    kind: ChangeKind::Modified,
                })
            }
            _ => {}
        }
    }
    for entry in previous {
        if !now.contains_key(&entry.path) {
            out.push(ListingChange {
                path: entry.path.clone(),
                kind: ChangeKind::Missing,
            });
        }
    }
    out
}

/// Compara un snapshot compacto con el listado actual. Si el anterior fue expulsado, el
/// controlador no llama esta función y trata la visita como primera comparación.
pub fn compare_snapshot(
    previous: &[crate::listing_snapshot::SnapshotEntry],
    current: &[Entry],
) -> Vec<ListingChange> {
    let old: HashMap<&PathBuf, &crate::listing_snapshot::SnapshotEntry> =
        previous.iter().map(|entry| (&entry.path, entry)).collect();
    let now: HashMap<&PathBuf, &Entry> = current.iter().map(|entry| (&entry.path, entry)).collect();
    let mut out = Vec::new();
    for entry in current {
        match old.get(&entry.path) {
            None => out.push(ListingChange {
                path: entry.path.clone(),
                kind: ChangeKind::New,
            }),
            Some(before)
                if before.size != entry.size
                    || before.modified != entry.modified
                    || before.kind != entry.kind =>
            {
                out.push(ListingChange {
                    path: entry.path.clone(),
                    kind: ChangeKind::Modified,
                });
            }
            _ => {}
        }
    }
    for entry in previous {
        if !now.contains_key(&entry.path) {
            out.push(ListingChange {
                path: entry.path.clone(),
                kind: ChangeKind::Missing,
            });
        }
    }
    out
}

/// Devuelve las rutas presentes o modificadas para resaltarlas en el panel. Los ausentes son
/// informativos y nunca corresponden a filas visibles.
pub fn highlight_paths(changes: &[ListingChange]) -> HashSet<PathBuf> {
    changes
        .iter()
        .filter(|change| change.kind != ChangeKind::Missing)
        .map(|change| change.path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_model::{Entry, EntryKind};
    fn e(name: &str, size: u64) -> Entry {
        Entry {
            name: name.into(),
            path: name.into(),
            kind: EntryKind::File,
            size: Some(size),
            modified: None,
            created: None,
            hidden: false,
            system: false,
        }
    }
    #[test]
    fn clasifica_nuevos_modificados_y_ausentes() {
        let before = vec![e("igual", 1), e("mod", 1), e("ausente", 1)];
        let now = vec![e("igual", 1), e("mod", 2), e("nuevo", 1)];
        let got = compare(&before, &now);
        assert_eq!(got.iter().filter(|c| c.kind == ChangeKind::New).count(), 1);
        assert_eq!(
            got.iter()
                .filter(|c| c.kind == ChangeKind::Modified)
                .count(),
            1
        );
        assert_eq!(
            got.iter().filter(|c| c.kind == ChangeKind::Missing).count(),
            1
        );
    }

    #[test]
    fn rutas_resaltadas_excluyen_ausentes_y_preservan_identidad_completa() {
        let changes = vec![
            ListingChange {
                path: PathBuf::from("C:/uno/informe.txt"),
                kind: ChangeKind::New,
            },
            ListingChange {
                path: PathBuf::from("C:/dos/informe.txt"),
                kind: ChangeKind::Modified,
            },
            ListingChange {
                path: PathBuf::from("C:/tres/informe.txt"),
                kind: ChangeKind::Missing,
            },
        ];
        let paths = highlight_paths(&changes);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("C:/uno/informe.txt")));
        assert!(paths.contains(&PathBuf::from("C:/dos/informe.txt")));
        assert!(!paths.contains(&PathBuf::from("C:/tres/informe.txt")));
    }

    #[test]
    fn sin_cambios_no_resalta_rutas() {
        assert!(highlight_paths(&[]).is_empty());
    }

    #[test]
    fn comparar_snapshot_compacto_conserva_nuevos_modificados_y_ausentes() {
        let previous = vec![
            crate::listing_snapshot::SnapshotEntry::from(&e("igual", 1)),
            crate::listing_snapshot::SnapshotEntry::from(&e("mod", 1)),
            crate::listing_snapshot::SnapshotEntry::from(&e("ausente", 1)),
        ];
        let current = vec![e("igual", 1), e("mod", 2), e("nuevo", 1)];
        let changes = compare_snapshot(&previous, &current);
        assert_eq!(
            changes.iter().filter(|c| c.kind == ChangeKind::New).count(),
            1
        );
        assert_eq!(
            changes
                .iter()
                .filter(|c| c.kind == ChangeKind::Modified)
                .count(),
            1
        );
        assert_eq!(
            changes
                .iter()
                .filter(|c| c.kind == ChangeKind::Missing)
                .count(),
            1
        );
    }
}
