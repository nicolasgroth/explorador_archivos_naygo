// Naygo — token de cancelación compartido para operaciones largas.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! `CancellationToken` es un estado compartido entre el hilo de UI (que puede
//! pedir cancelar o pausar) y un worker (que lo chequea entre cada paso). Clonar
//! el token comparte el mismo estado interno.
//!
//! Además de cancelar, el token soporta **pausa real**: el worker llama a
//! [`CancellationToken::wait_if_paused`] entre pasos y, si está pausado, se
//! suspende sin quemar CPU (espera sobre una condvar) hasta que se reanude o se
//! cancele.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

/// Token de cancelación + pausa compartido. Barato de clonar (Arc).
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    // Para despertar al worker cuando se reanuda o se cancela (sin sondear con sleep).
    waker: Arc<(Mutex<()>, Condvar)>,
}

impl CancellationToken {
    /// Crea un token nuevo, no cancelado y no pausado.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marca el token como cancelado. Idempotente.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.waker.1.notify_all();
    }

    /// `true` si alguien ya pidió cancelar.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Marca el token como pausado. El worker se suspenderá en su próximo
    /// `wait_if_paused`.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    /// Reanuda: el worker pausado continúa.
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.waker.1.notify_all();
    }

    /// `true` si alguien pidió pausar (y aún no se reanudó).
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Si está pausado, BLOQUEA hasta que se reanude o se cancele. No quema CPU
    /// (condvar). Retorna de inmediato si no está pausado o si ya está cancelado.
    pub fn wait_if_paused(&self) {
        if !self.is_paused() || self.is_cancelled() {
            return;
        }
        // `into_inner` recupera el guard aunque otro hilo haya paniqueado con el lock
        // tomado: el estado protegido es solo una barrera de condvar (sin invariantes
        // que romper) y un panic ajeno no debe tumbar en cascada a todos los workers.
        let mut guard = self.waker.0.lock().unwrap_or_else(|e| e.into_inner());
        while self.is_paused() && !self.is_cancelled() {
            let (g, _timeout) = self
                .waker
                .1
                .wait_timeout(guard, std::time::Duration::from_millis(200))
                // Mismo criterio: ignorar el poisoning, el estado sigue siendo válido.
                .unwrap_or_else(|e| e.into_inner());
            guard = g;
        }
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

    #[test]
    fn pausar_y_reanudar() {
        let t = CancellationToken::new();
        assert!(!t.is_paused());
        t.pause();
        assert!(t.is_paused());
        t.resume();
        assert!(!t.is_paused());
    }

    #[test]
    fn pausa_se_comparte_entre_clones() {
        let t = CancellationToken::new();
        let c = t.clone();
        t.pause();
        assert!(c.is_paused(), "el clon comparte el estado de pausa");
    }

    #[test]
    fn wait_if_paused_retorna_si_cancelado_estando_pausado() {
        let t = CancellationToken::new();
        t.pause();
        t.cancel();
        // No debe colgar: si está pausado pero cancelado, wait_if_paused retorna en seguida.
        t.wait_if_paused();
        assert!(t.is_cancelled());
    }

    #[test]
    fn mutex_envenenado_no_tumba_al_worker() {
        // Si un hilo paniquea con el lock tomado (mutex envenenado), wait_if_paused
        // NO debe paniquear en cascada: recupera el guard con into_inner.
        let t = CancellationToken::new();
        let c = t.clone();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _g = c.waker.0.lock().unwrap();
            panic!("boom con el lock tomado");
        }));
        assert!(
            r.is_err(),
            "el hilo efectivamente paniqueó (lock envenenado)"
        );
        // Pausar y esperar en otro hilo: toma el lock envenenado y espera en la condvar.
        t.pause();
        let t2 = t.clone();
        let h = std::thread::spawn(move || t2.wait_if_paused());
        // Dar tiempo a que tome el lock; luego reanudar para que despierte y salga.
        std::thread::sleep(std::time::Duration::from_millis(50));
        t.resume();
        // Si wait_if_paused hubiera paniqueado por el poisoning, join devuelve Err.
        assert!(h.join().is_ok(), "el worker sobrevivió al mutex envenenado");
    }
}
