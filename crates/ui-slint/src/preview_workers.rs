// Naygo — plazas de workers de preview, retenidas hasta que el hilo termina realmente.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use naygo_core::preview_policy::{Reservation, RetainedBudget, WORKER_ALLOWANCE, WORKING_BUDGET};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc,
};

/// Un receptor descartado NO libera capacidad: el permiso pertenece al hilo, incluido
/// durante I/O bloqueado o retirada por cancelación. No espera ni hace join desde UI.
/// Reserves a conservative working envelope BEFORE dispatch (no memory is allocated
/// by the reservation). Child Jobs, bounded IPC and local raster scratch enforce the
/// individual limits. Completed channel payloads remain separately count/size-bounded;
/// retained UI content has its own budget. This is not a limit on total process RSS.
#[derive(Clone)]
pub(super) struct Workers(Arc<AtomicUsize>, RetainedBudget);
impl Default for Workers {
    fn default() -> Self {
        Self(Arc::default(), RetainedBudget::new(WORKING_BUDGET))
    }
}
pub(super) const LIMIT: usize = 2;
struct Permit {
    count: Arc<AtomicUsize>,
    _reservation: Reservation,
    _metrics: crate::metrics::JobLease,
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::AcqRel);
    }
}
impl Workers {
    pub(super) fn has_capacity(&self) -> bool {
        self.0.load(Ordering::Acquire) < LIMIT
    }
    pub(super) fn spawn<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> std::io::Result<Option<mpsc::Receiver<T>>> {
        crate::metrics::job_submitted(crate::metrics::JobKind::Preview);
        let Some(reservation) = self.1.reserve(WORKER_ALLOWANCE) else {
            crate::metrics::job_rejected(crate::metrics::JobKind::Preview);
            return Ok(None);
        };
        if self
            .0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < LIMIT).then_some(n + 1)
            })
            .is_err()
        {
            crate::metrics::job_rejected(crate::metrics::JobKind::Preview);
            return Ok(None);
        }
        let permit = Permit {
            count: self.0.clone(),
            _reservation: reservation,
            _metrics: crate::metrics::job_started(crate::metrics::JobKind::Preview),
        };
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("naygo-preview".into())
            .spawn(move || {
                let _permit = permit;
                let result = work();
                let _ = tx.send(result);
            })?;
        Ok(Some(rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discarded_receiver_does_not_release_live_worker_capacity() {
        let workers = Workers::default();
        let mut exits = Vec::new();
        for _ in 0..LIMIT {
            let (tx, rx) = mpsc::channel();
            exits.push(tx);
            drop(
                workers
                    .spawn(move || {
                        rx.recv().unwrap();
                    })
                    .unwrap(),
            );
        }
        assert_eq!(workers.0.load(Ordering::Acquire), LIMIT);
        assert_eq!(workers.1.used(), LIMIT * WORKER_ALLOWANCE);
        assert!(workers.spawn(|| ()).unwrap().is_none());
        for exit in exits {
            exit.send(()).unwrap();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while workers.0.load(Ordering::Acquire) != 0 {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        // Dropping the count precedes the reservation field by a few instructions.
        while workers.1.used() != 0 {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(workers.spawn(|| 42).unwrap().unwrap().recv().unwrap(), 42);
    }

    #[test]
    fn temporary_envelope_covers_each_reader_and_renderer() {
        const {
            assert!(
                naygo_platform::preview_process::MEMORY_LIMIT
                    + naygo_core::mesh_preview::SCENE_MAX_BYTES
                    + 1024 * 1024
                    <= WORKER_ALLOWANCE
            );
            assert!(naygo_core::mesh_preview::RENDER_WORK_MAX_BYTES <= WORKER_ALLOWANCE);
        }
        assert_eq!(LIMIT * WORKER_ALLOWANCE, WORKING_BUDGET);
    }
}
