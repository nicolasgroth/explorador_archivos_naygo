// Naygo — listado async para la UI Slint: worker del core + drenado por lotes.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// El worker del core emite un Entry por archivo. El controller drena con `poll` (sin
// bloquear) desde un slint::Timer de ~30ms que se APAGA al terminar (Done): 0 trabajo en
// reposo, repaints acotados (~30fps) durante el listado. Es el patron clave para el bajo
// consumo sin GPU (no inundar el event loop con miles de eventos por archivo).

use naygo_core::cancel::CancellationToken;
use naygo_core::fs_model::Entry;
use naygo_core::listing::{spawn_listing, ListingMsg};
use std::sync::mpsc::Receiver;

/// Estado de un listado en curso (worker + canal + token de cancelacion).
pub struct Listing {
    rx: Receiver<ListingMsg>,
    token: CancellationToken,
}

impl Listing {
    /// Lanza el listado de `dir`. El worker corre en su hilo; el controller drenara `poll`.
    pub fn start(dir: std::path::PathBuf) -> Listing {
        let token = CancellationToken::new();
        let (rx, _handle) = spawn_listing(dir, token.clone());
        Listing { rx, token }
    }

    /// Cancela el listado (al navegar a otra carpeta antes de terminar).
    pub fn cancel(&self) {
        self.token.cancel();
    }

    /// Drena TODO lo acumulado en el canal AHORA (sin bloquear). Devuelve las entries
    /// nuevas del lote y si el listado TERMINO (Done/Error/Cancelled).
    pub fn poll(&self) -> (Vec<Entry>, bool) {
        let mut batch = Vec::new();
        let mut done = false;
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ListingMsg::Entry(e) => batch.push(e),
                ListingMsg::Done | ListingMsg::Cancelled | ListingMsg::Error(_) => {
                    done = true;
                    break;
                }
            }
        }
        (batch, done)
    }
}
