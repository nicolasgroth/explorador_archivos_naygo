// Naygo — recuperación de operaciones interrumpidas dentro del controlador.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// El journal se consulta y procesa fuera de la UI. Este módulo conserva el
// diálogo de reanudación como única frontera con el controlador principal.

use super::*;

impl OpsCtrl {
    /// Al arrancar la app: si hay journals pendientes, abre el modal de retomar.
    pub fn scan_resume(&mut self) {
        let pend = journal::scan(&self.config_dir);
        if !pend.is_empty() {
            self.pending_dialog = Some(OpDialog::Resume { items: pend });
        }
    }

    /// Retoma la operación journaleada `id`: replanifica los pasos pendientes y la lanza
    /// con un journal nuevo que reusa el id. Devuelve true si arrancó algo.
    pub fn resume(&mut self, id: &str) -> bool {
        // Tomar el journal del modal (si está ahí) o del disco.
        let journal = match &self.pending_dialog {
            Some(OpDialog::Resume { items }) => items.iter().find(|j| j.id == id).cloned(),
            _ => None,
        }
        .or_else(|| {
            journal::scan(&self.config_dir)
                .into_iter()
                .find(|j| j.id == id)
        });
        let Some(journal) = journal else {
            return false;
        };
        let resume = journal::resume_plan(&journal);
        if resume.plan.steps.is_empty() {
            // Nada pendiente: limpiar el journal y listo.
            journal::remove(&self.config_dir, id);
            self.drop_resume_item(id);
            return false;
        }
        // Cuántos orígenes se omiten por haber cambiado/desaparecido (se reporta al usuario).
        let resume_skipped = resume.skipped_changed.len();
        let label = journal.label();
        let token = CancellationToken::new();
        let (conflict_tx, conflict_rx) = std::sync::mpsc::channel::<ConflictDecision>();
        let writer = JournalWriter::new(
            &self.config_dir,
            OpJournal::new(
                journal.id.clone(),
                journal.kind.clone(),
                journal.conflict,
                resume.plan.clone(),
            ),
        );
        let size_map = size_map_of(&resume.plan);
        let (rx, _h) = engine::spawn(
            resume.plan,
            journal.kind.clone(),
            journal.conflict,
            token.clone(),
            conflict_rx,
            Some(writer),
        );
        let op_id = self.alloc_op_id();
        self.active_ops.push(ActiveOp {
            id: op_id,
            rx: Some(rx),
            conflict_tx,
            token,
            label,
            progress: None,
            summary: None,
            started: true,
            pending: None,
            journal_id: Some(journal.id.clone()),
            request: None,
            awaiting_conflict: None,
            awaiting_folders: None,
            resume_skipped,
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
            size_map,
            staging_guards: Vec::new(),
        });
        self.drop_resume_item(id);
        true
    }

    /// Descarta la operación journaleada `id` (borra el journal sin retomar).
    pub fn discard(&mut self, id: &str) {
        journal::remove(&self.config_dir, id);
        self.drop_resume_item(id);
    }

    /// Quita un ítem del modal Resume; si queda vacío, cierra el modal.
    fn drop_resume_item(&mut self, id: &str) {
        if let Some(OpDialog::Resume { items }) = &mut self.pending_dialog {
            items.retain(|j| j.id != id);
            if items.is_empty() {
                self.pending_dialog = None;
            }
        }
    }
}
