// Naygo — snapshots compactos y acotados de visitas a carpetas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Retiene solo los metadatos necesarios para comparar una carpeta entre visitas. El presupuesto
//! es provisional hasta medirlo con W-07; las expulsiones nunca se interpretan como eliminaciones.

use crate::fs_model::{Entry, EntryKind};
use std::collections::HashMap;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Presupuesto provisional: mantiene historia útil en equipos modestos sin retención ilimitada.
pub const DEFAULT_SNAPSHOT_BUDGET_BYTES: usize = 32 * 1024 * 1024;
/// Tope de entradas entre todas las carpetas, además del tope de bytes estimados.
pub const DEFAULT_SNAPSHOT_ENTRY_LIMIT: usize = 100_000;

/// Estado mínimo que basta para calcular nuevos, modificados y ausentes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotEntry {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
}

impl From<&Entry> for SnapshotEntry {
    fn from(entry: &Entry) -> Self {
        Self {
            path: entry.path.clone(),
            kind: entry.kind,
            size: entry.size,
            modified: entry.modified,
        }
    }
}

/// Snapshot de una visita, en el orden recibido. El orden no afecta la comparación.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListingSnapshot {
    pub entries: Vec<SnapshotEntry>,
    estimated_bytes: usize,
}

impl ListingSnapshot {
    fn new(dir: &Path, entries: &[Entry]) -> Self {
        let compact: Vec<_> = entries.iter().map(SnapshotEntry::from).collect();
        let entry_bytes = compact.iter().fold(
            compact
                .capacity()
                .saturating_mul(size_of::<SnapshotEntry>()),
            |total, entry| total.saturating_add(path_storage_bytes(&entry.path)),
        );
        let estimated_bytes = entry_bytes
            .saturating_add(path_storage_bytes(dir))
            .saturating_add(size_of::<StoredSnapshot>() + 64);
        Self {
            entries: compact,
            estimated_bytes,
        }
    }

    pub fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
    }
}

fn path_storage_bytes(path: &Path) -> usize {
    // El doble del texto UTF-8 aproxima el almacenamiento UTF-16 en Windows y es conservador
    // en POSIX. El margen fijo cubre la cabecera de la asignación del buffer.
    path.to_string_lossy()
        .len()
        .saturating_mul(2)
        .saturating_add(16)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotInsert {
    Stored,
    TooLarge,
}

#[derive(Clone, Debug)]
struct StoredSnapshot {
    snapshot: ListingSnapshot,
    last_access: u64,
}

/// Almacén LRU compartido por los paneles y acotado por bytes estimados y entradas retenidas.
/// No hace I/O y se actualiza solo cuando el controlador confirma un listado completo.
#[derive(Debug)]
pub struct ListingSnapshotStore {
    snapshots: HashMap<PathBuf, StoredSnapshot>,
    retained_bytes: usize,
    retained_entries: usize,
    budget_bytes: usize,
    entry_limit: usize,
    clock: u64,
}

impl Default for ListingSnapshotStore {
    fn default() -> Self {
        Self::new(DEFAULT_SNAPSHOT_BUDGET_BYTES, DEFAULT_SNAPSHOT_ENTRY_LIMIT)
    }
}

impl ListingSnapshotStore {
    pub fn new(budget_bytes: usize, entry_limit: usize) -> Self {
        Self {
            snapshots: HashMap::new(),
            retained_bytes: 0,
            retained_entries: 0,
            budget_bytes,
            entry_limit,
            clock: 0,
        }
    }

    /// Consulta y actualiza el uso LRU. Llamar desde el worker de UI es seguro: solo toca memoria.
    pub fn get(&mut self, dir: &Path) -> Option<&ListingSnapshot> {
        self.clock = self.clock.saturating_add(1);
        let stored = self.snapshots.get_mut(dir)?;
        stored.last_access = self.clock;
        Some(&stored.snapshot)
    }

    /// Reemplaza el snapshot tras un éxito completo. Un snapshot que excede un límite no se
    /// retiene; tampoco deja una versión anterior obsoleta disponible para futuras visitas.
    pub fn insert(&mut self, dir: PathBuf, entries: &[Entry]) -> SnapshotInsert {
        let snapshot = ListingSnapshot::new(&dir, entries);
        let bytes = snapshot.estimated_bytes();
        let count = snapshot.entries.len();
        self.remove(&dir);
        if bytes > self.budget_bytes || count > self.entry_limit {
            return SnapshotInsert::TooLarge;
        }

        while self
            .retained_bytes
            .checked_add(bytes)
            .is_none_or(|total| total > self.budget_bytes)
            || self
                .retained_entries
                .checked_add(count)
                .is_none_or(|total| total > self.entry_limit)
        {
            let Some(oldest) = self.oldest_key() else {
                return SnapshotInsert::TooLarge;
            };
            self.remove(&oldest);
        }

        self.clock = self.clock.saturating_add(1);
        self.retained_bytes = self
            .retained_bytes
            .checked_add(bytes)
            .expect("budget checked");
        self.retained_entries = self
            .retained_entries
            .checked_add(count)
            .expect("entry limit checked");
        self.snapshots.insert(
            dir,
            StoredSnapshot {
                snapshot,
                last_access: self.clock,
            },
        );
        SnapshotInsert::Stored
    }

    pub fn remove(&mut self, dir: &Path) -> Option<ListingSnapshot> {
        let stored = self.snapshots.remove(dir)?;
        self.retained_bytes = self
            .retained_bytes
            .saturating_sub(stored.snapshot.estimated_bytes());
        self.retained_entries = self
            .retained_entries
            .saturating_sub(stored.snapshot.entries.len());
        Some(stored.snapshot)
    }

    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub fn retained_entries(&self) -> usize {
        self.retained_entries
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    fn oldest_key(&self) -> Option<PathBuf> {
        self.snapshots
            .iter()
            .min_by(|(path_a, a), (path_b, b)| {
                a.last_access
                    .cmp(&b.last_access)
                    .then_with(|| path_a.cmp(path_b))
            })
            .map(|(path, _)| path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_model::EntryKind;

    fn entry(path: PathBuf, size: u64) -> Entry {
        Entry {
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path,
            kind: EntryKind::File,
            size: Some(size),
            modified: None,
            created: None,
            hidden: false,
            system: false,
        }
    }

    #[test]
    fn admite_el_limite_exacto_y_rechaza_el_overflow() {
        let dir = PathBuf::from("C:/exacto");
        let entries = vec![entry(dir.join("a.txt"), 1)];
        let exact = ListingSnapshot::new(&dir, &entries).estimated_bytes();
        let mut accepted = ListingSnapshotStore::new(exact, 1);
        assert_eq!(
            accepted.insert(dir.clone(), &entries),
            SnapshotInsert::Stored
        );
        assert_eq!(accepted.retained_bytes(), exact);

        let mut rejected = ListingSnapshotStore::new(exact - 1, 1);
        assert_eq!(rejected.insert(dir, &entries), SnapshotInsert::TooLarge);
        assert!(rejected.is_empty());
    }

    #[test]
    fn reemplazar_la_misma_carpeta_no_duplica_retencion() {
        let dir = PathBuf::from("C:/visita");
        let mut store = ListingSnapshotStore::default();
        assert_eq!(
            store.insert(dir.clone(), &[entry(dir.join("a"), 1)]),
            SnapshotInsert::Stored
        );
        assert_eq!(
            store.insert(dir.clone(), &[entry(dir.join("b"), 2)]),
            SnapshotInsert::Stored
        );
        assert_eq!(store.len(), 1);
        assert_eq!(store.retained_entries(), 1);
        assert!(store.get(&dir).unwrap().entries[0].path.ends_with("b"));
    }

    #[test]
    fn lru_expulsa_la_visita_menos_reciente_deterministicamente() {
        let a = PathBuf::from("C:/a");
        let b = PathBuf::from("C:/b");
        let c = PathBuf::from("C:/c");
        let rows = |dir: &Path| vec![entry(dir.join("one"), 1)];
        let bytes = ListingSnapshot::new(&a, &rows(&a)).estimated_bytes();
        let mut store = ListingSnapshotStore::new(bytes * 2, 2);
        assert_eq!(store.insert(a.clone(), &rows(&a)), SnapshotInsert::Stored);
        assert_eq!(store.insert(b.clone(), &rows(&b)), SnapshotInsert::Stored);
        assert!(store.get(&a).is_some());
        assert_eq!(store.insert(c.clone(), &rows(&c)), SnapshotInsert::Stored);
        assert!(store.get(&a).is_some());
        assert!(store.get(&b).is_none());
        assert!(store.get(&c).is_some());
        assert_eq!(store.retained_entries(), 2);
    }

    #[test]
    fn una_visita_demasiado_grande_no_deja_un_snapshot_anterior_obsoleto() {
        let dir = PathBuf::from("C:/grande");
        let small = vec![entry(dir.join("small"), 1)];
        let cap = ListingSnapshot::new(&dir, &small).estimated_bytes();
        let mut store = ListingSnapshotStore::new(cap, 1);
        assert_eq!(store.insert(dir.clone(), &small), SnapshotInsert::Stored);
        let large = vec![entry(dir.join("large1"), 1), entry(dir.join("large2"), 2)];
        assert_eq!(store.insert(dir.clone(), &large), SnapshotInsert::TooLarge);
        assert!(store.get(&dir).is_none());
        assert_eq!(store.retained_entries(), 0);
    }
}
