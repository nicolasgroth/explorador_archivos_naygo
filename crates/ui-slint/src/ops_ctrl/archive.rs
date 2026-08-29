// Naygo — ejecución de operaciones ZIP dentro del controlador de operaciones.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
// Mantiene aislado el worker de comprimir/extraer. Reutiliza el contrato de
// `ActiveOp`, por lo que el panel, la cancelación y los conflictos se comportan
// igual que para una copia o movimiento normal.

use super::*;

impl OpsCtrl {
    /// Lanza el worker de comprimir/extraer. Reusa el MISMO canal/panel/cancelación que las ops
    /// normales: crea una `ActiveOp` con `rx` vivo (como `spawn_op`), de modo que `pump_ops` drene
    /// su progreso y la cancelación por id funcione igual que para Copy/Move. La diferencia con
    /// `spawn_op` es que NO hay plan ni motor (`engine::spawn`): el trabajo (escaneo + compresión/
    /// extracción) corre dentro del hilo del worker, que habla por el mismo `OpMsg` channel.
    ///
    /// `record_undo`: si es `true`, el worker calcula las `UndoAction` SEGURAS (ver `zip_undo_rx`)
    /// y las manda por un canal aparte; `pump_ops` las recoge al recibir `Done` y arma el
    /// `UndoEntry`. El deshacer trashea SOLO lo que la op CREÓ: el .zip al comprimir; al extraer,
    /// solo las rutas nuevas (nunca archivos/carpetas preexistentes del usuario).
    pub(super) fn spawn_zip_op(
        &mut self,
        id: u64,
        req: OpRequest,
        label: String,
        record_undo: bool,
        staging_guards: Vec<naygo_platform::drop_target::StagedDrop>,
    ) {
        use naygo_core::archive_ops::{compress_zip, extract_zip, ExtractConflict};
        let token = CancellationToken::new();
        let (tx, rx) = std::sync::mpsc::channel::<OpMsg>();
        // Canal de conflicto REAL para la extracción: el worker emite `OpMsg::Conflict` al chocar un
        // archivo y se BLOQUEA en `conflict_rx` esperando la decisión del usuario; `pump_ops` la
        // enruta de vuelta por `conflict_tx` (vía `resolve_conflict`). Para comprimir, el canal no se
        // usa: el `dest_name` ya viene DESAMBIGUADO desde `name_confirm` (`unique_zip_name`), así que
        // crear el `.zip` nunca pisa un archivo preexistente y no hay conflicto que preguntar. Se crea
        // igual para poblar `ActiveOp`.
        let (conflict_tx, conflict_rx) = std::sync::mpsc::channel::<ConflictDecision>();
        // Canal de UNA muestra para el inverso (deshacer) del zip: el worker lo calcula desde los
        // `ArchiveOpItem` (que traen `created`) y lo manda; `pump_ops` lo drena al `Done`. Solo se
        // engancha si `record_undo` (si no, ni se ofrece deshacer).
        let (undo_tx, undo_rx) = std::sync::mpsc::channel::<Vec<UndoAction>>();

        // Datos que el worker necesita (clonados antes de mover el closure al hilo).
        let token_worker = token.clone();
        let dest_dir = req.dest_dir.clone().unwrap_or_default();
        let kind = req.kind.clone();
        let sources = req.sources.clone();

        // Registrar la operación con el mismo contrato de `spawn_op`.
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
            request: record_undo.then(|| req.clone()),
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
            zip_undo_rx: record_undo.then_some(undo_rx),
            trash_receipts_rx: None,
            finished_epoch_secs: None,
            size_map: HashMap::new(),
            staging_guards,
        });

        std::thread::spawn(move || {
            // `tx` (Sender<OpMsg>) es Clone. Se clona para cada clausura `FnMut` porque ambas lo
            // capturan: `on_progress` por su clon y `on_conflict` por el suyo (una `&mut dyn FnMut`
            // no podría co-capturar el mismo `tx` por valor). El `tx` original queda libre para los
            // mensajes terminales (`Done`/`Cancelled`/`Failed`) al final del worker.
            let tx_progress = tx.clone();
            let tx_conflict = tx.clone();
            // El progreso del zip es por BYTES (no archivos): se manda como `OpProgress` con los
            // contadores de archivos en 0; el panel pinta la barra por el porcentaje de bytes.
            let mut on_progress = |done: u64, total: u64| {
                let _ = tx_progress.send(OpMsg::Progress(OpProgress {
                    bytes_done: done,
                    bytes_total: total,
                    files_done: 0,
                    files_total: 0,
                    current: PathBuf::new(),
                }));
            };
            let result = match &kind {
                OpKind::Compress { dest_name } => {
                    let dest_zip = dest_dir.join(dest_name);
                    let r = compress_zip(&sources, &dest_zip, &mut on_progress, &token_worker);
                    if record_undo {
                        // El inverso solo toca el ZIP nuevo; nunca los orígenes.
                        let acts = match &r {
                            Ok(_) => vec![UndoAction::TrashCreated { path: dest_zip }],
                            Err(_) => Vec::new(),
                        };
                        let _ = undo_tx.send(acts);
                    }
                    r
                }
                OpKind::Extract => {
                    let zip = sources.first().cloned().unwrap_or_default();
                    // La API de core entrega una sola ruta; se usa para ambos lados del prompt.
                    // La memoria `sticky` implementa correctamente "aplicar a todos".
                    let mut sticky: Option<ExtractConflict> = None;
                    let mut on_conflict = |target: &Path| -> ExtractConflict {
                        if let Some(s) = sticky {
                            return s;
                        }
                        let _ = tx_conflict.send(OpMsg::Conflict(ConflictPrompt::from_paths(
                            target.to_path_buf(),
                            target.to_path_buf(),
                        )));
                        loop {
                            if token_worker.is_cancelled() {
                                return ExtractConflict::Cancel;
                            }
                            match conflict_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                                Ok(d) => {
                                    let mapped = map_action_to_extract(d.action);
                                    if d.apply_all {
                                        sticky = Some(mapped);
                                    }
                                    return mapped;
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                    return ExtractConflict::Skip;
                                }
                            }
                        }
                    };
                    let r = extract_zip(
                        &zip,
                        &dest_dir,
                        &mut on_conflict,
                        &mut on_progress,
                        &token_worker,
                    );
                    if record_undo {
                        let acts = match &r {
                            Ok(items) => zip_undo_actions_from_items(items),
                            Err(_) => Vec::new(),
                        };
                        let _ = undo_tx.send(acts);
                    }
                    r
                }
                // Este módulo solo recibe Compress/Extract. Reportar, en vez de panic, evita que
                // una op quede colgada si el invariante se rompe.
                other => {
                    let _ = tx.send(OpMsg::Failed(format!(
                        "worker de zip recibió un tipo de operación no soportado: {other:?}"
                    )));
                    return;
                }
            };
            match result {
                Ok(items) => {
                    let _ = tx.send(OpMsg::Done(zip_summary(items)));
                }
                Err(naygo_core::archive_ops::ArchiveError::Cancelled) => {
                    let _ = tx.send(OpMsg::Cancelled(zip_summary(Vec::new())));
                }
                Err(e) => {
                    let _ = tx.send(OpMsg::Failed(e.to_string()));
                }
            }
        });
    }
}
