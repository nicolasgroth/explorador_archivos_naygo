// Naygo — política de calidad y presupuesto compartido de contenido retenido del preview.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// Admission envelope per live preview job: isolated child (192 MiB) + largest IPC
/// scene (48 MiB) + pipe/protocol overhead. In-process render scratch is <=64 MiB.
/// This is a reservation, not a preallocation or a claim about application-wide RSS.
pub const WORKER_ALLOWANCE: usize = 256 * 1024 * 1024;
pub const WORKING_BUDGET: usize = 2 * WORKER_ALLOWANCE;

/// Defaults compatibles: la opción económica sólo reduce salida/presentación, no altera
/// reglas, codificaciones ni el canvas necesario para componer una animación correctamente.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Quality {
    pub economical: bool,
}
impl Quality {
    pub fn animation_side(self) -> u32 {
        if self.economical {
            512
        } else {
            768
        }
    }
    pub fn interval_us(self) -> u64 {
        if self.economical {
            100_000
        } else {
            66_667
        }
    }
    pub fn image_side(self) -> u32 {
        if self.economical {
            1024
        } else {
            crate::preview::IMAGE_MAX_SIDE
        }
    }
    pub fn mesh_size(self) -> (u32, u32) {
        if self.economical {
            (512, 384)
        } else {
            (800, 600)
        }
    }
}

/// Contabilidad de buffers retenidos, NO del RSS total ni de los temporales del parser.
/// Todos los paneles Preview comparten un controlador/budget; los permisos siguen al
/// último propietario (incluidos workers retirados). Nunca espera en el hilo de UI.
#[derive(Clone, Debug)]
pub struct RetainedBudget {
    used: Arc<AtomicUsize>,
    limit: usize,
}
#[derive(Debug)]
pub struct Reservation {
    used: Arc<AtomicUsize>,
    bytes: usize,
}
impl RetainedBudget {
    pub fn new(limit: usize) -> Self {
        Self {
            used: Arc::new(AtomicUsize::new(0)),
            limit,
        }
    }
    pub fn reserve(&self, bytes: usize) -> Option<Reservation> {
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|&total| total <= self.limit)
            })
            .ok()?;
        Some(Reservation {
            used: self.used.clone(),
            bytes,
        })
    }
    pub fn used(&self) -> usize {
        self.used.load(Ordering::Acquire)
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quality_defaults_and_economy_are_bounded() {
        let normal = Quality::default();
        let economy = Quality { economical: true };
        assert_eq!(
            (normal.animation_side(), normal.interval_us()),
            (768, 66_667)
        );
        assert_eq!(
            (economy.animation_side(), economy.interval_us()),
            (512, 100_000)
        );
        assert!(economy.image_side() <= normal.image_side());
        assert_eq!(economy.mesh_size(), (512, 384));
    }
    #[test]
    fn shared_reservations_survive_owner_and_release_once() {
        let budget = RetainedBudget::new(100);
        let shared = budget.clone();
        let lease = Arc::new(budget.reserve(70).unwrap());
        let worker = lease.clone();
        drop(lease);
        assert!(shared.reserve(31).is_none());
        assert!(shared.reserve(usize::MAX).is_none());
        assert_eq!(shared.used(), 70);
        drop(worker);
        assert_eq!(shared.used(), 0);
        assert!(shared.reserve(100).is_some());
        assert!(RetainedBudget::new(0).reserve(1).is_none());
    }
}
