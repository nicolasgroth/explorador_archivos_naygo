// Naygo — ejecución y restauración de borrados por lote en la Papelera.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// La Papelera es una operación Shell y no una copia normal: este módulo adapta
// su progreso y sus recibos al contrato común de `ActiveOp`.

use super::*;

impl OpsCtrl {
    /// Restaura una entrada previamente enviada a Papelera sin bloquear la UI.
    pub fn start_trash_restore(
        &mut self,
        receipts: Vec<naygo_platform::trash::TrashReceipt>,
        label: String,
    ) {
        if receipts.is_empty() {
            return;
        }
        self.prune_finished();
        let id = self.alloc_op_id();
        let token = CancellationToken::new();
        let (tx, rx) = std::sync::mpsc::channel::<OpMsg>();
        let (conflict_tx, _conflict_rx) = std::sync::mpsc::channel::<ConflictDecision>();
        let items: Vec<OpItem> = receipts
            .iter()
            .map(|receipt| OpItem {
                dest: receipt.original.clone(),
                outcome: OpOutcome::Done,
                src: None,
            })
            .collect();
        std::thread::spawn(
            move || match naygo_platform::trash::restore_from_trash(&receipts) {
                Ok(()) => {
                    let _ = tx.send(OpMsg::Done(OpSummary {
                        bytes_done: 0,
                        elapsed_secs: 0.0,
                        items,
                    }));
                }
                Err(error) => {
                    let _ = tx.send(OpMsg::Failed(format!("{error:?}")));
                }
            },
        );
        self.active_ops.push(ActiveOp {
            id,
            rx: Some(rx),
            conflict_tx,
            token,
            label,
            progress: None,
            summary: None,
            started: true,
            pending: None,
            journal_id: None,
            request: None,
            awaiting_conflict: None,
            awaiting_folders: None,
            resume_skipped: 0,
            started_at: None,
            last_sample: None,
            peak_speed: 0,
            plan_rx: None,
            plan_kind: OpKind::Copy,
            plan_record_undo: false,
            scan_files: 0,
            scan_bytes: 0,
            pending_req: None,
            zip_undo_rx: None,
            trash_receipts_rx: None,
            finished_epoch_secs: None,
            size_map: HashMap::new(),
            staging_guards: Vec::new(),
        });
    }

    /// Promueve un borrado a papelera ya escaneado a un worker Shell observable.
    pub(super) fn promote_planning_to_trash(
        &mut self,
        idx: usize,
        plan: OpPlan,
        request: Option<OpRequest>,
    ) {
        let token = self.active_ops[idx].token.clone();
        let sources = request
            .as_ref()
            .map(|r| r.sources.clone())
            .unwrap_or_default();
        let total_bytes = plan.total_bytes;
        // IFileOperation recibe y recicla los ORÍGENES de primer nivel de manera atómica. El
        // plan recursivo sirve para estimar bytes, pero no puede ser la fuente de verdad del
        // resumen: si el Shell rechaza el directorio raíz, no se deben informar sus hijos como
        // eliminados. Además, este vector conserva el diagnóstico aunque el plan sea vacío.
        let total_items = sources.len();
        let completed_items: Vec<OpItem> = sources
            .iter()
            .cloned()
            .map(|path| OpItem {
                dest: path.clone(),
                outcome: OpOutcome::Done,
                src: Some(path),
            })
            .collect();
        let failed_sources = sources.clone();
        let failed_items = move |error: &str| {
            failed_sources
                .iter()
                .cloned()
                .map(|path| OpItem {
                    dest: path.clone(),
                    outcome: OpOutcome::Failed(error.to_string()),
                    src: Some(path),
                })
                .collect::<Vec<_>>()
        };
        let current = sources.first().cloned().unwrap_or_default();
        let (tx, rx) = std::sync::mpsc::channel::<OpMsg>();
        let (receipt_tx, receipt_rx) = std::sync::mpsc::channel();
        let worker_token = token.clone();
        let _ = std::thread::Builder::new()
            .name("naygo-trash".into())
            .spawn(move || {
                let (progress_tx, progress_rx) =
                    std::sync::mpsc::channel::<naygo_platform::trash::TrashProgress>();
                let progress_out = tx.clone();
                let progress_current = current.clone();
                let bridge = std::thread::spawn(move || {
                    while let Ok(p) = progress_rx.recv() {
                        let total = p.work_total.max(1) as u64;
                        let done = (p.work_done as u64).min(total);
                        let _ = progress_out.send(OpMsg::Progress(OpProgress {
                            bytes_done: total_bytes.saturating_mul(done) / total,
                            bytes_total: total_bytes,
                            files_done: total_items.saturating_mul(done as usize) / total as usize,
                            files_total: total_items,
                            current: progress_current.clone(),
                        }));
                    }
                });
                let cancel_token = worker_token.clone();
                let result = naygo_platform::trash::move_to_trash_with_progress(
                    &sources,
                    progress_tx,
                    std::sync::Arc::new(move || cancel_token.is_cancelled()),
                );
                let _ = bridge.join();
                let summary = OpSummary {
                    bytes_done: if result.is_ok() { total_bytes } else { 0 },
                    elapsed_secs: 0.0,
                    items: if result.is_ok() {
                        completed_items
                    } else {
                        Vec::new()
                    },
                };
                if worker_token.is_cancelled() {
                    let _ = tx.send(OpMsg::Cancelled(summary));
                } else {
                    match result {
                        Ok(receipts) => {
                            let _ = receipt_tx.send(receipts);
                            let _ = tx.send(OpMsg::Progress(OpProgress {
                                bytes_done: total_bytes,
                                bytes_total: total_bytes,
                                files_done: total_items,
                                files_total: total_items,
                                current,
                            }));
                            let _ = tx.send(OpMsg::Done(summary));
                        }
                        Err(error) => {
                            // No enviar `OpMsg::Failed` pelado: el panel sólo recibe un resumen
                            // por ítem y antes lo convertía en «hecho: 0» sin indicar que la
                            // Papelera había fallado. Mantener el motivo por ruta evita una
                            // confirmación falsa y deja el detalle disponible en el historial.
                            let detail = format!("{error:?}");
                            crate::logging::log_line(&format!(
                                "Papelera no completó {} ítem(s): {detail}",
                                sources.len()
                            ));
                            let _ = tx.send(OpMsg::Done(OpSummary {
                                bytes_done: 0,
                                elapsed_secs: 0.0,
                                items: failed_items(&detail),
                            }));
                        }
                    }
                }
            });
        let op = &mut self.active_ops[idx];
        op.rx = Some(rx);
        op.request = request;
        op.size_map = size_map_of(&plan);
        op.plan_rx = None;
        op.scan_files = 0;
        op.scan_bytes = 0;
        op.trash_receipts_rx = Some(receipt_rx);
    }
}
