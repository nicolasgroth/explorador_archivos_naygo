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
use naygo_core::listing::{
    try_spawn_listing_filtered_with_preflight, ListingAdmission, ListingFilter, ListingOutcome,
    ListingStartError,
};
use std::sync::mpsc::TryRecvError;
use std::{cell::Cell, time::Instant};

const MAX_ENTRIES_PER_POLL: usize = 1_024;
const MAX_POLL_TIME: std::time::Duration = std::time::Duration::from_millis(3);

/// Presupuesto compartido entre los listados de archivos y el árbol durante un tick.
pub(crate) struct PollBudget {
    pub(crate) remaining_entries: usize,
    pub(crate) deadline: Instant,
}

impl PollBudget {
    pub(crate) fn new() -> Self {
        Self::with_limits(MAX_ENTRIES_PER_POLL, MAX_POLL_TIME)
    }

    fn with_limits(max_entries: usize, max_time: std::time::Duration) -> Self {
        Self {
            remaining_entries: max_entries,
            deadline: Instant::now() + max_time,
        }
    }
}

/// Estado de un listado en curso (worker + canal + token de cancelacion).
pub struct Listing {
    rx: naygo_core::listing::ListingReceiver,
    token: CancellationToken,
    /// `true` hasta que el consumidor reclame el primer lote con `take_fresh`. Marca que este
    /// listado es NUEVO (navegación o refresh): las entries viejas del panel deben REEMPLAZARSE
    /// —no acumularse— cuando lleguen las primeras nuevas. Sin esto, F5 (re-listar el mismo
    /// directorio sobre un panel ya poblado) duplicaba todas las filas con cada `extend`.
    fresh: bool,
    terminal: bool,
    pending_terminal: Option<ListingOutcome>,
    started_at: Instant,
    first_result_recorded: bool,
    cancel_recorded: Cell<bool>,
}

impl Listing {
    fn from_worker(
        rx: naygo_core::listing::ListingReceiver,
        token: CancellationToken,
        started_at: Instant,
    ) -> Self {
        Self {
            rx,
            token,
            fresh: true,
            terminal: false,
            pending_terminal: None,
            started_at,
            first_result_recorded: false,
            cancel_recorded: Cell::new(false),
        }
    }

    /// Intenta lanzar un listado con la plaza global compartida por Files y árbol. La recuperación
    /// transaccional del directorio y su padre ocurre en el worker antes de leer; el permiso queda
    /// capturado hasta la salida real, incluso si la UI cancela y lo retira.
    pub fn try_start(
        dir: std::path::PathBuf,
        admission: &ListingAdmission,
        filter: ListingFilter,
    ) -> Result<Listing, ListingStartError> {
        let token = CancellationToken::new();
        let started_at = Instant::now();
        let (rx, _handle) = try_spawn_listing_filtered_with_preflight(
            dir,
            token.clone(),
            filter,
            admission,
            |listed_dir, token| {
                let rename = naygo_platform::move_rename::PlatformNoReplaceRename;
                naygo_core::ops::recover_tree_replacements_in_parent(listed_dir, token, &rename)?;
                if let Some(parent) = listed_dir.parent() {
                    if parent != listed_dir {
                        naygo_core::ops::recover_tree_replacements_in_parent(
                            parent, token, &rename,
                        )?;
                    }
                }
                Ok(())
            },
        )?;
        Ok(Self::from_worker(rx, token, started_at))
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
        if !self.cancel_recorded.replace(true) {
            crate::metrics::record("listing.cancel_requested", self.started_at.elapsed());
        }
    }

    /// Drena TODO lo acumulado en el canal AHORA (sin bloquear). Devuelve las entries
    /// nuevas del lote y un resultado terminal explícito, hasta el presupuesto de un tick.
    /// Si quedan mensajes, siguen en el canal para el próximo tick. Si el canal se cierra sin
    /// mensaje terminal, se trata como fallo para que el timer y la navegación terminen.
    #[cfg(test)]
    pub fn poll(&mut self) -> (Vec<Entry>, Option<ListingOutcome>) {
        let mut budget = PollBudget::new();
        let mut group_remaining = MAX_ENTRIES_PER_POLL;
        self.poll_with_shared_budget(&mut budget, &mut group_remaining)
    }

    /// Recoge como máximo `max_entries` filas o hasta `max_time`, lo que ocurra primero.
    /// El límite de tiempo es cooperativo y se revisa entre mensajes; no bloquea esperando datos.
    #[cfg(test)]
    fn poll_with_budget(
        &mut self,
        max_entries: usize,
        max_time: std::time::Duration,
    ) -> (Vec<Entry>, Option<ListingOutcome>) {
        let mut budget = PollBudget::with_limits(max_entries, max_time);
        let mut group_remaining = max_entries;
        self.poll_with_shared_budget(&mut budget, &mut group_remaining)
    }

    pub(crate) fn poll_with_shared_budget(
        &mut self,
        budget: &mut PollBudget,
        group_remaining: &mut usize,
    ) -> (Vec<Entry>, Option<ListingOutcome>) {
        let _span = crate::metrics::span("listing.poll");
        let mut batch = Vec::new();
        if self.terminal {
            return (batch, None);
        }
        let entry_limit = budget.remaining_entries.min(*group_remaining);
        let mut terminal = None;
        let mut entries_disconnected = false;
        loop {
            if batch.len() >= entry_limit
                || budget.remaining_entries == 0
                || *group_remaining == 0
                || Instant::now() >= budget.deadline
            {
                break;
            }
            match self.rx.try_recv_entry() {
                Ok(entry) => {
                    batch.push(entry);
                    continue;
                }
                Err(TryRecvError::Disconnected) => entries_disconnected = true,
                Err(TryRecvError::Empty) => {}
            }
            if let Some(outcome) = self.pending_terminal.take() {
                // El worker publica el terminal después de todas sus filas. Revisa la cola de
                // nuevo antes de entregarlo para cubrir el cruce entre ambos canales.
                terminal = Some(outcome);
                break;
            }
            match self.rx.try_recv_terminal() {
                Ok(outcome) => {
                    self.pending_terminal = Some(outcome);
                    continue;
                }
                Err(TryRecvError::Empty) if entries_disconnected => {
                    terminal = Some(disconnected_failure());
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    terminal = Some(disconnected_failure());
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        budget.remaining_entries = budget.remaining_entries.saturating_sub(batch.len());
        *group_remaining = (*group_remaining).saturating_sub(batch.len());
        self.terminal = terminal.is_some();
        if !self.first_result_recorded && (!batch.is_empty() || terminal.is_some()) {
            self.first_result_recorded = true;
            let kind = if batch.is_empty() {
                "listing.first_terminal"
            } else {
                "listing.first_batch"
            };
            crate::metrics::record(kind, self.started_at.elapsed());
        }
        if terminal.is_some() {
            crate::metrics::record("listing.complete", self.started_at.elapsed());
        }
        (batch, terminal)
    }
}

impl Drop for Listing {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

fn disconnected_failure() -> ListingOutcome {
    ListingOutcome::Failed(naygo_core::listing::ListingFailure {
        kind: std::io::ErrorKind::Other,
        message: "worker de listado desconectado sin resultado terminal".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canal_desconectado_sin_terminal_se_convierte_en_fallo() {
        let (entries_tx, entries) = std::sync::mpsc::sync_channel(1);
        let (terminal_tx, terminal) = std::sync::mpsc::channel();
        drop((entries_tx, terminal_tx));
        let mut listing = Listing {
            rx: naygo_core::listing::ListingReceiver::new(entries, terminal),
            token: CancellationToken::new(),
            fresh: true,
            terminal: false,
            pending_terminal: None,
            started_at: Instant::now(),
            first_result_recorded: false,
            cancel_recorded: Cell::new(false),
        };

        let (entries, terminal) = listing.poll();
        assert!(entries.is_empty());
        assert!(matches!(
            terminal,
            Some(ListingOutcome::Failed(ref failure))
                if failure.kind == std::io::ErrorKind::Other
        ));
        assert!(
            listing.poll().1.is_none(),
            "el terminal se entrega una sola vez"
        );
    }

    #[test]
    fn poll_limitado_conserva_entradas_y_terminal_para_el_siguiente_tick() {
        let dir = tempfile::tempdir().unwrap();
        let (entries_tx, entries) = std::sync::mpsc::sync_channel(4);
        let (terminal_tx, terminal) = std::sync::mpsc::channel();
        for name in ["a", "b", "c"] {
            entries_tx
                .send(Entry {
                    name: name.into(),
                    path: dir.path().join(name),
                    kind: naygo_core::fs_model::EntryKind::File,
                    size: Some(1),
                    modified: None,
                    created: None,
                    hidden: false,
                    system: false,
                })
                .unwrap();
        }
        terminal_tx.send(ListingOutcome::Complete).unwrap();
        let mut listing = Listing {
            rx: naygo_core::listing::ListingReceiver::new(entries, terminal),
            token: CancellationToken::new(),
            fresh: true,
            terminal: false,
            pending_terminal: None,
            started_at: Instant::now(),
            first_result_recorded: false,
            cancel_recorded: Cell::new(false),
        };

        let (first, first_terminal) =
            listing.poll_with_budget(2, std::time::Duration::from_secs(1));
        assert_eq!(first.len(), 2);
        assert!(first_terminal.is_none());
        assert!(
            listing.fresh,
            "el consumidor aún no reclama la primera tanda"
        );

        let (last, terminal) = listing.poll_with_budget(2, std::time::Duration::from_secs(1));
        assert_eq!(last.len(), 1);
        assert_eq!(terminal, Some(ListingOutcome::Complete));
        assert!(listing
            .poll_with_budget(2, std::time::Duration::from_secs(1))
            .0
            .is_empty());
        assert!(listing
            .poll_with_budget(2, std::time::Duration::from_secs(1))
            .1
            .is_none());
    }

    #[test]
    fn presupuesto_compartido_acota_la_suma_de_varios_listados() {
        let dir = tempfile::tempdir().unwrap();
        let mut listings = Vec::new();
        let mut entry_senders = Vec::new();
        let mut terminal_senders = Vec::new();
        for name in ["first", "second"] {
            let (entry_tx, entries) = std::sync::mpsc::sync_channel(1);
            let (terminal_tx, terminal) = std::sync::mpsc::channel();
            entry_tx
                .send(Entry {
                    name: name.into(),
                    path: dir.path().join(name),
                    kind: naygo_core::fs_model::EntryKind::File,
                    size: Some(1),
                    modified: None,
                    created: None,
                    hidden: false,
                    system: false,
                })
                .unwrap();
            listings.push(Listing {
                rx: naygo_core::listing::ListingReceiver::new(entries, terminal),
                token: CancellationToken::new(),
                fresh: true,
                terminal: false,
                pending_terminal: None,
                started_at: Instant::now(),
                first_result_recorded: false,
                cancel_recorded: Cell::new(false),
            });
            entry_senders.push(entry_tx);
            terminal_senders.push(terminal_tx);
        }

        let mut budget = PollBudget::with_limits(1, std::time::Duration::from_secs(1));
        let mut group_remaining = 1;
        let (first, _) = listings[0].poll_with_shared_budget(&mut budget, &mut group_remaining);
        let (second, _) = listings[1].poll_with_shared_budget(&mut budget, &mut group_remaining);
        assert_eq!(first.len(), 1);
        assert!(second.is_empty());
        assert_eq!(budget.remaining_entries, 0);
    }
}
