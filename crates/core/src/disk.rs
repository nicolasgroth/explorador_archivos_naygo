// Naygo — uso de disco: cálculo puro de espacio usado/libre y umbrales.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `DiskUsage` resume el espacio de una unidad (total y libre, en bytes) y deriva
//! el porcentaje USADO y los umbrales de alerta. Puro: sin Windows ni I/O. La
//! lectura real del espacio vive en `platform::drive_space`.

/// Espacio de una unidad, en bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskUsage {
    pub total: u64,
    pub free: u64,
}

impl DiskUsage {
    /// Bytes usados. Satura a 0 si `free > total` (datos inconsistentes del SO).
    pub fn used(self) -> u64 {
        self.total.saturating_sub(self.free)
    }

    /// Porcentaje USADO, 0..=100. `0` si `total == 0` (unidad sin tamaño conocido).
    pub fn percent_used(self) -> u8 {
        if self.total == 0 {
            return 0;
        }
        let pct = (self.used() as u128 * 100 / self.total as u128) as u64;
        pct.min(100) as u8
    }

    /// Uso alto: > 75% usado.
    pub fn is_high(self) -> bool {
        self.percent_used() > 75
    }

    /// Uso crítico: > 90% usado.
    pub fn is_critical(self) -> bool {
        self.percent_used() > 90
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_normal() {
        let d = DiskUsage {
            total: 1000,
            free: 400,
        };
        assert_eq!(d.used(), 600);
    }

    #[test]
    fn used_satura_si_free_mayor_que_total() {
        let d = DiskUsage {
            total: 100,
            free: 500,
        };
        assert_eq!(d.used(), 0);
        assert_eq!(d.percent_used(), 0);
    }

    #[test]
    fn percent_total_cero_es_cero() {
        let d = DiskUsage { total: 0, free: 0 };
        assert_eq!(d.percent_used(), 0);
    }

    #[test]
    fn percent_y_umbrales() {
        let half = DiskUsage {
            total: 1000,
            free: 500,
        };
        assert_eq!(half.percent_used(), 50);
        assert!(!half.is_high() && !half.is_critical());

        let p76 = DiskUsage {
            total: 1000,
            free: 240,
        }; // 76% usado
        assert_eq!(p76.percent_used(), 76);
        assert!(p76.is_high() && !p76.is_critical());

        let p91 = DiskUsage {
            total: 1000,
            free: 90,
        }; // 91% usado
        assert_eq!(p91.percent_used(), 91);
        assert!(p91.is_high() && p91.is_critical());

        let p75 = DiskUsage {
            total: 1000,
            free: 250,
        }; // 75% exacto: NO high (>75)
        assert_eq!(p75.percent_used(), 75);
        assert!(!p75.is_high());
    }

    #[test]
    fn percent_satura_a_100() {
        let full = DiskUsage {
            total: 1000,
            free: 0,
        };
        assert_eq!(full.percent_used(), 100);
        assert!(full.is_critical());
    }
}
