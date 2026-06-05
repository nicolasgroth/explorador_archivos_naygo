// Naygo — token de cancelación compartido para operaciones largas.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `CancellationToken` es un flag booleano compartido entre el hilo de UI (que
//! puede pedir cancelar) y un worker (que lo chequea entre cada paso). Clonar el
//! token comparte el mismo estado interno.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Token de cancelación compartido. Barato de clonar (Arc).
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Crea un token nuevo, no cancelado.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marca el token como cancelado. Idempotente.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// `true` si alguien ya pidió cancelar.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_nuevo_no_esta_cancelado() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancelar_se_propaga_a_los_clones() {
        let token = CancellationToken::new();
        let clon = token.clone();
        token.cancel();
        assert!(clon.is_cancelled(), "el clon comparte el estado");
    }
}
