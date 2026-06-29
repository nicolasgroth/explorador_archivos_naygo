// Naygo — listado async para la UI Slint: worker del core + drenado por lotes.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// El worker del core emite un Entry por archivo. El controller drena con `poll` (sin
// bloquear) desde un slint::Timer de ~30ms que se APAGA al terminar (Done): 0 trabajo en
// reposo, repaints acotados (~30fps) durante el listado. Es el patron clave para el bajo
// consumo sin GPU (no inundar el event loop con miles de eventos por archivo).

use naygo_core::cancel::CancellationToken;
use naygo_core::fs_model::Entry;
use naygo_core::listing::{spawn_listing, spawn_listing_filtered, ListingFilter, ListingMsg};
use std::sync::mpsc::Receiver;

/// Estado de un listado en curso (worker + canal + token de cancelacion).
pub struct Listing {
    rx: Receiver<ListingMsg>,
    token: CancellationToken,
    /// `true` hasta que el consumidor reclame el primer lote con `take_fresh`. Marca que este
    /// listado es NUEVO (navegación o refresh): las entries viejas del panel deben REEMPLAZARSE
    /// —no acumularse— cuando lleguen las primeras nuevas. Sin esto, F5 (re-listar el mismo
    /// directorio sobre un panel ya poblado) duplicaba todas las filas con cada `extend`.
    fresh: bool,
}

impl Listing {
    /// Lanza el listado de `dir`. El worker corre en su hilo; el controller drenara `poll`.
    pub fn start(dir: std::path::PathBuf) -> Listing {
        let token = CancellationToken::new();
        let (rx, _handle) = spawn_listing(dir, token.clone());
        Listing {
            rx,
            token,
            fresh: true,
        }
    }

    /// Lanza un listado SOLO de directorios (para expandir una rama del árbol): el worker
    /// omite archivos y emite únicamente subcarpetas.
    pub fn start_dirs_only(dir: std::path::PathBuf) -> Listing {
        let token = CancellationToken::new();
        let (rx, _handle) = spawn_listing_filtered(dir, token.clone(), ListingFilter::DirsOnly);
        Listing {
            rx,
            token,
            fresh: true,
        }
    }

    /// Reclama el flag "primer lote" UNA sola vez: devuelve `true` la primera vez que se llama
    /// para un listado nuevo y `false` de ahí en adelante. El consumidor lo usa para vaciar las
    /// entries previas del panel justo antes de aplicar el primer lote (reemplazo limpio, sin
    /// parpadeo: las filas viejas se mantienen visibles hasta que llega la primera tanda nueva).
    pub fn take_fresh(&mut self) -> bool {
        std::mem::replace(&mut self.fresh, false)
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
