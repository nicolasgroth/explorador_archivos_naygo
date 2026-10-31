// Naygo — ejecución y restauración de borrados por lote en la Papelera.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// La Papelera es una operación Shell y no una copia normal: este módulo adapta
// su progreso y sus recibos al contrato común de `ActiveOp`.

use super::*;

/// Transforma los fallos indexados del worker Shell al resumen por origen de la UI. La ruta del
/// recibo puede estar normalizada a absoluta; el índice evita compararla con una entrada relativa.
fn partial_trash_items(
    sources: &[PathBuf],
    failures: &[naygo_platform::trash::TrashItemFailure],
) -> (Vec<OpItem>, usize) {
    let items = sources
        .iter()
        .cloned()
        .enumerate()
        .map(|(source_index, path)| {
            let outcome = failures
                .iter()
                .find(|failure| failure.source_index == source_index)
                .map(|failure| OpOutcome::Failed(failure.detail.clone()))
                .unwrap_or(OpOutcome::Done);
            OpItem {
                dest: path.clone(),
                outcome,
                src: Some(path),
            }
        })
        .collect();
    let succeeded = sources.len().saturating_sub(failures.len());
    (items, succeeded)
}

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
                let (receipts, items, successful_count, error_detail) = match result {
                    Ok(receipts) => (receipts, completed_items, total_items, None),
                    Err(naygo_platform::trash::TrashError::Partial { receipts, failures }) => {
                        let (items, succeeded) = partial_trash_items(&sources, &failures);
                        let detail = failures
                            .iter()
                            .map(|failure| {
                                format!("{}: {}", failure.original.display(), failure.detail)
                            })
                            .collect::<Vec<_>>()
                            .join("; ");
                        (receipts, items, succeeded, Some(detail))
                    }
                    Err(error) => {
                        let detail = format!("{error:?}");
                        (Vec::new(), failed_items(&detail), 0, Some(detail))
                    }
                };
                if !receipts.is_empty() {
                    let _ = receipt_tx.send(receipts);
                }
                if let Some(detail) = error_detail {
                    crate::logging::log_line(&format!(
                        "Papelera completó {successful_count}/{} ítem(s): {detail}",
                        sources.len()
                    ));
                }
                let summary = OpSummary {
                    bytes_done: if successful_count == total_items {
                        total_bytes
                    } else {
                        // El Shell no informa un tamaño fiable para los elementos parciales.
                        0
                    },
                    elapsed_secs: 0.0,
                    items,
                };
                if successful_count == total_items {
                    let _ = tx.send(OpMsg::Progress(OpProgress {
                        bytes_done: total_bytes,
                        bytes_total: total_bytes,
                        files_done: total_items,
                        files_total: total_items,
                        current,
                    }));
                }
                if worker_token.is_cancelled() {
                    let _ = tx.send(OpMsg::Cancelled(summary));
                } else {
                    let _ = tx.send(OpMsg::Done(summary));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resultados_parciales_mapean_por_indice_si_la_ruta_es_relativa() {
        let sources = vec![
            PathBuf::from(r"docs\ok.txt"),
            PathBuf::from(r"docs\fail.txt"),
        ];
        let failures = vec![naygo_platform::trash::TrashItemFailure {
            source_index: 1,
            original: PathBuf::from(r"C:\work\docs\fail.txt"),
            detail: "acceso denegado".to_string(),
        }];

        let (items, succeeded) = partial_trash_items(&sources, &failures);

        assert_eq!(succeeded, 1);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].src.as_ref(), Some(&sources[0]));
        assert!(matches!(items[0].outcome, OpOutcome::Done));
        assert_eq!(items[1].src.as_ref(), Some(&sources[1]));
        assert!(matches!(
            &items[1].outcome,
            OpOutcome::Failed(detail) if detail == "acceso denegado"
        ));
    }
}
