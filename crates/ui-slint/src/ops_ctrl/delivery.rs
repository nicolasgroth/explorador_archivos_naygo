// Naygo — publicar entregas mediante el panel normal de operaciones.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

impl OpsCtrl {
    /// Muestra una preparación OLE sin bloquear UI. El token también se consulta durante la
    /// captura y copia; una llamada COM en curso termina antes de completar la cancelación.
    pub fn start_drop_preparation(
        &mut self,
        preparation: naygo_platform::drop_target::DropPreparation,
        label: String,
    ) {
        self.prune_finished();
        let id = self.alloc_op_id();
        self.drop_preparations.insert(id);
        let (conflict_tx, _) = std::sync::mpsc::channel();
        self.active_ops.push(ActiveOp {
            id,
            rx: Some(preparation.receiver),
            conflict_tx,
            token: preparation.token,
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
            directory_sources: Default::default(),
            type_count_cursor: 0,
            type_counts: (0, 0, 0),
            staging_guards: Vec::new(),
        });
    }

    pub fn start_delivery(
        &mut self,
        plan: naygo_core::delivery::DeliveryPlan,
        label: String,
        staging_guards: Vec<naygo_platform::drop_target::StagedDrop>,
    ) -> u64 {
        self.prune_finished();
        let id = self.alloc_op_id();
        let token = CancellationToken::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let (conflict_tx, _) = std::sync::mpsc::channel();
        let request = OpRequest {
            kind: OpKind::Copy,
            sources: Vec::new(),
            dest_dir: plan.destination.parent().map(Path::to_path_buf),
            conflict: ConflictPolicy::Skip,
        };
        self.active_ops.push(ActiveOp {
            id,
            rx: Some(rx),
            conflict_tx,
            token: token.clone(),
            label,
            progress: None,
            summary: None,
            started: true,
            pending: None,
            journal_id: None,
            request: Some(request),
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
            directory_sources: Default::default(),
            type_count_cursor: 0,
            type_counts: (0, 0, 0),
            staging_guards,
        });
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let message = match naygo_core::delivery::execute(&plan, &token, &tx) {
                Ok(destination) => OpMsg::Done(OpSummary {
                    items: vec![naygo_core::ops::OpItem {
                        dest: destination,
                        src: None,
                        outcome: OpOutcome::Done,
                    }],
                    bytes_done: plan.total_bytes,
                    elapsed_secs: start.elapsed().as_secs_f64(),
                }),
                Err(naygo_core::delivery::DeliveryError::Cancelled) => {
                    OpMsg::Cancelled(OpSummary::default())
                }
                // Conservar el fallo por destino: el historial no debe mostrar un falso «hecho: 0».
                Err(error) => OpMsg::Done(OpSummary {
                    items: vec![OpItem {
                        dest: plan.destination.clone(),
                        src: None,
                        outcome: OpOutcome::Failed(error.to_string()),
                    }],
                    elapsed_secs: start.elapsed().as_secs_f64(),
                    ..Default::default()
                }),
            };
            let _ = tx.send(message);
        });
        id
    }
}

#[cfg(test)]
mod drop_preparation_tests {
    use super::*;

    #[test]
    fn preparation_can_be_cancelled_and_does_not_enter_history() {
        let config = tempfile::tempdir().unwrap();
        let mut ops = OpsCtrl::new(config.path().to_path_buf());
        let token = CancellationToken::new();
        let (tx, rx) = std::sync::mpsc::channel();
        ops.start_drop_preparation(
            naygo_platform::drop_target::DropPreparation {
                token: token.clone(),
                receiver: rx,
            },
            "Preparando".to_string(),
        );
        let id = ops.active_ops[0].id as i32;
        ops.cancel_op(id);
        assert!(token.is_cancelled());
        tx.send(OpMsg::Cancelled(OpSummary::default())).unwrap();
        ops.pump_ops();
        assert!(ops.active_ops.is_empty());
        assert!(ops.drop_preparations.is_empty());
    }
}
