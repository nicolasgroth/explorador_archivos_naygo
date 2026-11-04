// Naygo — instantánea inmutable y revocable de destinos de arrastre externos.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! La UI invalida antes de cambiar la presentación y publica al quedar coherente. La captura
//! no consulta disco y rechaza regiones ambiguas. La corrección depende de que TODO cambio
//! relevante, incluido viewport, DPI, ventana y contenido visible, invalide antes de mutar.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotTarget {
    Files { pane_id: u64, directory: PathBuf },
    Basket { pane_id: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ScreenRect {
    fn contains(self, x: i32, y: i32) -> bool {
        self.left < self.right
            && self.top < self.bottom
            && x >= self.left
            && x < self.right
            && y >= self.top
            && y < self.bottom
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropRegion {
    pub bounds: ScreenRect,
    pub target: SnapshotTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedTarget {
    pub revision: u64,
    pub target: SnapshotTarget,
}

#[derive(Default)]
struct State {
    revision: u64,
    regions: Option<Vec<DropRegion>>,
}

/// Clonable entre UI y STA OLE. Los tickets impiden publicar un cálculo obsoleto.
#[derive(Clone, Default)]
pub struct DropSnapshot(Arc<RwLock<State>>);

impl DropSnapshot {
    /// Invalida inmediatamente antes de un cambio visual y retorna el ticket de publicación.
    pub fn invalidate(&self) -> u64 {
        let mut state = self
            .0
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.revision = state.revision.wrapping_add(1);
        state.regions = None;
        state.revision
    }

    /// Solo acepta el resultado del cálculo más reciente. Regiones sin tamaño son rechazadas.
    pub fn publish(&self, ticket: u64, regions: Vec<DropRegion>) -> bool {
        let Ok(mut state) = self.0.write() else {
            return false;
        };
        if state.revision != ticket
            || regions
                .iter()
                .any(|r| r.bounds.left >= r.bounds.right || r.bounds.top >= r.bounds.bottom)
        {
            return false;
        }
        state.regions = Some(regions);
        true
    }

    /// Captura una región única al soltar; cero o varias regiones son inseguras.
    pub fn capture(&self, x: i32, y: i32) -> Option<CapturedTarget> {
        let state = self.0.read().ok()?;
        let mut hits = state
            .regions
            .as_ref()?
            .iter()
            .filter(|region| region.bounds.contains(x, y));
        let target = hits.next()?.target.clone();
        if hits.next().is_some() {
            return None;
        }
        Some(CapturedTarget {
            revision: state.revision,
            target,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, right: i32, id: u64) -> DropRegion {
        DropRegion {
            bounds: ScreenRect {
                left,
                right,
                top: 0,
                bottom: 100,
            },
            target: SnapshotTarget::Files {
                pane_id: id,
                directory: PathBuf::from("C:\\mismo"),
            },
        }
    }

    #[test]
    fn same_path_in_two_panels_preserves_identity_and_edges() {
        let snapshot = DropSnapshot::default();
        let ticket = snapshot.invalidate();
        assert!(snapshot.publish(ticket, vec![rect(0, 100, 1), rect(100, 200, 2)]));
        assert!(matches!(
            snapshot.capture(100, 2).unwrap().target,
            SnapshotTarget::Files { pane_id: 2, .. }
        ));
        assert!(snapshot.capture(200, 2).is_none());
    }

    #[test]
    fn stale_publish_cannot_restore_an_invalid_snapshot() {
        let snapshot = DropSnapshot::default();
        let old = snapshot.invalidate();
        let new = snapshot.invalidate();
        assert!(!snapshot.publish(old, vec![rect(0, 100, 1)]));
        assert!(snapshot.capture(1, 1).is_none());
        assert!(snapshot.publish(new, vec![rect(0, 100, 2)]));
        assert!(matches!(
            snapshot.capture(1, 1).unwrap().target,
            SnapshotTarget::Files { pane_id: 2, .. }
        ));
    }

    #[test]
    fn ambiguous_overlap_rejected_and_capture_remains_immutable() {
        let snapshot = DropSnapshot::default();
        let ticket = snapshot.invalidate();
        assert!(snapshot.publish(ticket, vec![rect(0, 100, 1)]));
        let captured = snapshot.capture(5, 5).unwrap();
        let ticket = snapshot.invalidate();
        assert!(snapshot.capture(5, 5).is_none());
        assert!(snapshot.publish(ticket, vec![rect(0, 100, 2), rect(0, 50, 3)]));
        assert!(snapshot.capture(5, 5).is_none());
        assert!(matches!(
            captured.target,
            SnapshotTarget::Files { pane_id: 1, .. }
        ));
    }

    #[test]
    fn poisoned_state_fails_closed() {
        let snapshot = DropSnapshot::default();
        let ticket = snapshot.invalidate();
        assert!(snapshot.publish(ticket, vec![rect(0, 100, 1)]));
        let broken = snapshot.clone();
        let _ = std::thread::spawn(move || {
            let _guard = broken.0.write().unwrap();
            panic!("simular interrupción de actualización");
        })
        .join();
        assert!(snapshot.capture(1, 1).is_none());
        assert!(!snapshot.publish(ticket, vec![rect(0, 100, 1)]));
    }
}
