// Naygo — motor de operaciones: ejecuta un OpPlan en un worker, cancelable.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Ejecuta los pasos de un `OpPlan` copiando archivos POR BUFFERS (cancelable a
//! media copia), emitiendo `OpMsg` por canal. Un error de un paso no aborta la op
//! (se registra en el summary). Cancelar borra el parcial del archivo en curso.
//! Papelera: NO aquí (solo borrado permanente); la papelera la hace `platform`.
//! Conflictos: la UI los resuelve antes de spawnear (el motor recibe pasos limpios).

use super::journal::JournalWriter;
use super::{
    ConflictAction, ConflictDecision, ConflictPolicy, ConflictPrompt, OpItem, OpKind, OpMsg,
    OpOutcome, OpPlan, OpProgress, OpStep, OpSummary,
};
use crate::cancel::CancellationToken;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

/// Tamaño de buffer para copiar (y granularidad de cancelación dentro de un archivo).
const BUF_SIZE: usize = 256 * 1024;

/// Lanza la ejecución de un `OpPlan` en un hilo worker. Devuelve el extremo de
/// recepción del canal de mensajes y el `JoinHandle` del worker.
///
/// El worker emite `OpMsg::Progress` mientras trabaja y, al terminar, un único
/// `OpMsg::Cancelled(summary)` (si se canceló) o `OpMsg::Done(summary)`.
///
/// `conflict` es la política a aplicar ante un destino ya existente. En el modelo
/// de ops-A la UI resuelve los conflictos *antes* de spawnear, así que `conflict_rx`
/// queda sin uso (reservado para ops-B); la política se respeta directamente aquí.
pub fn spawn(
    plan: OpPlan,
    kind: OpKind,
    conflict: ConflictPolicy,
    token: CancellationToken,
    conflict_rx: Receiver<ConflictDecision>,
    journal: Option<JournalWriter>,
) -> (Receiver<OpMsg>, std::thread::JoinHandle<()>) {
    spawn_with_no_replace_rename(
        plan,
        kind,
        conflict,
        token,
        conflict_rx,
        journal,
        std::sync::Arc::new(NativeNoReplaceRename),
    )
}

/// Lanza el motor con la operación de plataforma que publica movimientos rápidos sin reemplazar.
/// La UI debe usar esta entrada cuando la plataforma ofrece una implementación atómica; `core`
/// conserva `spawn` para consumidores autónomos y tests.
pub fn spawn_with_no_replace_rename(
    plan: OpPlan,
    kind: OpKind,
    conflict: ConflictPolicy,
    token: CancellationToken,
    conflict_rx: Receiver<ConflictDecision>,
    mut journal: Option<JournalWriter>,
    no_replace_rename: std::sync::Arc<dyn NoReplaceRename>,
) -> (Receiver<OpMsg>, std::thread::JoinHandle<()>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let summary = run_plan_with_no_replace_rename(
            &plan,
            &kind,
            conflict,
            &token,
            &tx,
            &conflict_rx,
            journal.as_mut(),
            no_replace_rename.as_ref(),
        );
        if let Some(w) = journal.as_mut() {
            w.flush();
        }
        let final_msg = if token.is_cancelled() {
            OpMsg::Cancelled(summary)
        } else {
            OpMsg::Done(summary)
        };
        let _ = tx.send(final_msg);
    });
    (rx, handle)
}

/// Ejecuta todos los pasos de un `OpPlan` de forma síncrona, emitiendo progreso por
/// `tx`. Devuelve el `OpSummary` (no envía el mensaje final: eso lo hace `spawn`).
///
/// Reglas:
/// - Un paso que falla NO aborta la op: se registra como `OpOutcome::Failed` y se
///   sigue con el siguiente.
/// - La cancelación se chequea antes de cada paso y dentro de la copia (cada 256KB).
///   Una copia cancelada a media máquina borra el archivo parcial.
/// - Conflictos (destino existe): según `conflict` — Skip salta, Overwrite reemplaza,
///   Rename elige un nombre libre con `dedup_name`, Ask se trata como Overwrite (la
///   resolución real de Ask ocurre en la UI antes de spawnear, así que una llamada
///   directa al motor con Ask sobrescribe).
///
/// `_conflict_rx` queda reservado para ops-B (no se usa en ops-A).
pub fn run_plan(
    plan: &OpPlan,
    kind: &OpKind,
    conflict: ConflictPolicy,
    token: &CancellationToken,
    tx: &Sender<OpMsg>,
    conflict_rx: &Receiver<ConflictDecision>,
    journal: Option<&mut JournalWriter>,
) -> OpSummary {
    run_plan_with_no_replace_rename(
        plan,
        kind,
        conflict,
        token,
        tx,
        conflict_rx,
        journal,
        &NativeNoReplaceRename,
    )
}

/// Variante de [`run_plan`] que recibe el renombrado atómico de la plataforma.
#[allow(clippy::too_many_arguments)] // Mantiene juntas las dependencias explícitas de una ejecución.
pub fn run_plan_with_no_replace_rename(
    plan: &OpPlan,
    kind: &OpKind,
    conflict: ConflictPolicy,
    token: &CancellationToken,
    tx: &Sender<OpMsg>,
    conflict_rx: &Receiver<ConflictDecision>,
    mut journal: Option<&mut JournalWriter>,
    no_replace_rename: &dyn NoReplaceRename,
) -> OpSummary {
    let start = std::time::Instant::now();
    let mut summary = OpSummary::default();
    let mut files_done = 0usize;
    // Mayor prefijo CONTIGUO de pasos resueltos (Done/Skipped). Solo este avanza el
    // journal: si un paso falla, dejamos de avanzar el cursor para el resto de la op,
    // de modo que ese paso (y los siguientes) se vuelvan a intentar al retomar tras un
    // crash, en vez de darse por hechos. Re-hacer un paso es seguro (idempotente bajo
    // Overwrite).
    let mut journal_cursor = 0usize;
    let mut barrier_hit = false;
    // Acción elegida con "aplicar a todos" en un conflicto interactivo (ops-B): mientras
    // sea `Some`, los choques siguientes la usan sin volver a preguntar.
    let mut applied_all: Option<ConflictAction> = None;
    // Un Replace de carpeta se construye junto al destino y se publica al final. Hasta entonces
    // el destino anterior y el origen de Move permanecen intactos. Los temporales comparten
    // volumen con el destino, de modo que la publicación final es un rename sin reemplazo.
    let mut replacements =
        match prepare_tree_replacements(plan, token, &mut summary, no_replace_rename) {
            Ok(replacements) => replacements,
            Err(()) => {
                summary.elapsed_secs = start.elapsed().as_secs_f64();
                return summary;
            }
        };
    let mut execution_plan = plan.clone();
    execution_plan.pre_delete.clear();
    for step in &mut execution_plan.steps {
        for replacement in &replacements {
            if let Some(staged_path) =
                rebase_path(&step.to, &replacement.destination, &replacement.staging)
            {
                step.to = staged_path;
                break;
            }
        }
    }
    let mut step_item_indices = Vec::with_capacity(plan.steps.len());

    for (idx, step) in execution_plan.steps.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }
        let original_step = &plan.steps[idx];
        let staged = replacements.iter().any(|replacement| {
            super::plan::path_starts_with_ci(&original_step.to, &replacement.destination)
        });

        // Progreso antes de tocar este paso.
        let _ = tx.send(OpMsg::Progress(OpProgress {
            bytes_done: summary.bytes_done,
            bytes_total: plan.total_bytes,
            files_done,
            files_total: plan.total_files,
            current: original_step.to.clone(),
        }));

        // Acumulados en este punto: lo ya copiado (archivos previos) es el piso del
        // progreso por bytes que emitirá la copia de este archivo.
        let progress = ProgressCtx {
            prev_bytes: summary.bytes_done,
            bytes_total: plan.total_bytes,
            files_done,
            files_total: plan.total_files,
        };
        // Los pasos dentro de un árbol de reemplazo siempre copian al staging. Un Move retira
        // sus fuentes sólo tras publicar el árbol completo.
        let step_kind = if staged && matches!(kind, OpKind::Move) {
            OpKind::Copy
        } else {
            kind.clone()
        };
        let (record_path, outcome, bytes_added, counts_as_file) = exec_step(
            step,
            &step_kind,
            conflict,
            token,
            tx,
            conflict_rx,
            &mut applied_all,
            progress,
            no_replace_rename,
        );
        summary.bytes_done += bytes_added;
        if counts_as_file && matches!(outcome, OpOutcome::Done) {
            files_done += 1;
        }
        let resolved = matches!(outcome, OpOutcome::Done | OpOutcome::Skipped);
        // Provenance explícita: `dest` es el destino REAL del paso (puede diferir de `step.to`
        // por conflict-rename); `src` es el origen del paso (`step.from`, `None` al crear vacío).
        // Guardar el origen aquí es lo que permite deshacer un Move sin re-derivarlo por nombre.
        let record_path = replacements
            .iter()
            .find_map(|replacement| {
                rebase_path(&record_path, &replacement.staging, &replacement.destination)
            })
            .unwrap_or(record_path);
        step_item_indices.push(Some(summary.items.len()));
        summary.items.push(OpItem {
            dest: record_path,
            outcome,
            src: original_step.from.clone(),
        });

        // El cursor del journal avanza solo mientras los pasos se resuelven de forma
        // contigua. El primer fallo levanta una barrera: a partir de ahí el cursor no
        // avanza más (aunque pasos posteriores tengan éxito), así el paso fallido queda
        // pendiente para el retomar. `done_through = journal_cursor` (throttled).
        if resolved && !barrier_hit {
            journal_cursor = idx + 1;
        } else if !resolved {
            barrier_hit = true;
        }
        if replacements.is_empty() {
            if let Some(w) = journal.as_deref_mut() {
                w.record(journal_cursor, std::time::Instant::now());
            }
        }
    }

    for replacement in &mut replacements {
        let step_indices: Vec<usize> = plan
            .steps
            .iter()
            .enumerate()
            .filter_map(|(idx, step)| {
                super::plan::path_starts_with_ci(&step.to, &replacement.destination).then_some(idx)
            })
            .collect();
        let staged_ok = !token.is_cancelled()
            && !step_indices.is_empty()
            && step_indices.iter().all(|idx| {
                step_item_indices
                    .get(*idx)
                    .and_then(|item_idx| *item_idx)
                    .and_then(|item_idx| summary.items.get(item_idx))
                    .is_some_and(|item| matches!(item.outcome, OpOutcome::Done))
            });

        if !staged_ok {
            mark_unpublished_replacement_skipped(
                &step_indices,
                &plan.steps,
                &step_item_indices,
                &mut summary,
            );
            continue;
        }

        if let Err(error) =
            apply_replacement_directory_permissions(replacement, &plan.steps, &step_indices)
        {
            mark_unpublished_replacement_skipped(
                &step_indices,
                &plan.steps,
                &step_item_indices,
                &mut summary,
            );
            summary.items.push(OpItem {
                dest: replacement.destination.clone(),
                outcome: OpOutcome::Failed(format!(
                    "no se publicaron los cambios porque no se pudieron aplicar permisos de carpeta: {error}"
                )),
                src: Some(replacement.source.clone()),
            });
            continue;
        }

        match publish_tree_replacement(replacement, no_replace_rename) {
            Ok(backup) => {
                replacement.published = true;
                let backup_removed = match &backup.backup {
                    Some(path) => match remove_tree_entry_cancelable(path, token) {
                        Ok(true) => true,
                        Ok(false) => {
                            summary.items.push(OpItem {
                                dest: path.clone(),
                                outcome: OpOutcome::Skipped,
                                src: None,
                            });
                            false
                        }
                        Err(error) => {
                            summary.items.push(OpItem {
                                dest: path.clone(),
                                outcome: OpOutcome::Failed(format!(
                                    "el árbol nuevo quedó publicado; se conservó el respaldo anterior en {}: {error}",
                                    path.display()
                                )),
                                src: None,
                            });
                            false
                        }
                    },
                    None => true,
                };
                if backup_removed {
                    if let Err(error) = finalize_tree_replacement(&backup) {
                        summary.items.push(OpItem {
                            dest: replacement.destination.clone(),
                            outcome: OpOutcome::Failed(format!(
                                "el árbol nuevo quedó publicado; se conserva el registro de recuperación {}: {error}",
                                backup.journal.display()
                            )),
                            src: Some(replacement.source.clone()),
                        });
                    }
                }
            }
            Err(error) => {
                mark_unpublished_replacement_skipped(
                    &step_indices,
                    &plan.steps,
                    &step_item_indices,
                    &mut summary,
                );
                summary.items.push(OpItem {
                    dest: replacement.destination.clone(),
                    outcome: OpOutcome::Failed(error),
                    src: Some(replacement.source.clone()),
                });
            }
        }
    }

    let replacements_published =
        !replacements.is_empty() && replacements.iter().all(|replacement| replacement.published);
    let all_steps_published = replacements_published
        && !token.is_cancelled()
        && !summary
            .items
            .iter()
            .any(|item| matches!(item.outcome, OpOutcome::Failed(_) | OpOutcome::Skipped))
        && step_item_indices.iter().all(|item_idx| {
            item_idx
                .and_then(|item_idx| summary.items.get(item_idx))
                .is_some_and(|item| matches!(item.outcome, OpOutcome::Done))
        });
    if all_steps_published {
        // Un crash durante staging reintenta desde cero; sólo avanzamos el journal cuando todos
        // los árboles nuevos ya están publicados y el origen de Move aún está intacto.
        if let Some(writer) = journal {
            writer.record(plan.steps.len(), std::time::Instant::now());
            writer.flush();
        }
        if matches!(kind, OpKind::Move) {
            for replacement in &replacements {
                let step_indices: Vec<usize> = plan
                    .steps
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, step)| {
                        super::plan::path_starts_with_ci(&step.to, &replacement.destination)
                            .then_some(idx)
                    })
                    .collect();
                remove_published_move_sources(
                    replacement,
                    &plan.steps,
                    &step_indices,
                    &step_item_indices,
                    &mut summary,
                    token,
                );
            }
        }
    }

    summary.elapsed_secs = start.elapsed().as_secs_f64();
    summary
}

/// Árbol de reemplazo en staging del mismo volumen; su journal nace antes de crear el directorio.
struct TreeReplacementStage {
    destination: PathBuf,
    source: PathBuf,
    staging: PathBuf,
    transaction_id: String,
    backup: PathBuf,
    journal: PathBuf,
    marker: PathBuf,
    _active: ActiveReplacementGuard,
    preserve_recovery: bool,
    published: bool,
}

struct TreePublication {
    backup: Option<PathBuf>,
    journal: PathBuf,
    marker: PathBuf,
}

impl Drop for TreeReplacementStage {
    fn drop(&mut self) {
        if self.published || self.preserve_recovery {
            return;
        }
        let cleanup_token = CancellationToken::new();
        let _ = remove_tree_entry_cancelable(&self.staging, &cleanup_token);
        let _ = remove_file_if_present(&self.marker);
        let _ = remove_file_if_present(&self.journal);
        if let Some(parent) = self.journal.parent().and_then(Path::parent) {
            let _ = remove_empty_tree_recovery_dir(parent);
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TreeReplacementRecord {
    version: u8,
    id: String,
    destination_name: PathBuf,
    staging_name: PathBuf,
    backup_name: PathBuf,
}

/// Prepara staging para destinos marcados Replace y protege cualquier origen del plan.
fn prepare_tree_replacements(
    plan: &OpPlan,
    token: &CancellationToken,
    summary: &mut OpSummary,
    no_replace_rename: &dyn NoReplaceRename,
) -> Result<Vec<TreeReplacementStage>, ()> {
    let mut replacements = Vec::new();
    for target in &plan.pre_delete {
        if token.is_cancelled() {
            break;
        }
        let source_root = plan
            .steps
            .iter()
            .find(|step| step.is_dir && super::plan::paths_eq_ci(&step.to, target))
            .and_then(|step| step.from.as_ref());
        let unsafe_target = plan.steps.iter().any(|step| {
            step.from.as_ref().is_some_and(|from| {
                super::plan::paths_eq_ci(from, target)
                    || super::plan::path_starts_with_ci(from, target)
            })
        });
        if unsafe_target {
            summary.items.push(OpItem {
                dest: target.clone(),
                outcome: OpOutcome::Failed(
                    "reemplazo rechazado: el destino es (o contiene) un origen de la operación"
                        .into(),
                ),
                src: source_root.cloned(),
            });
            return Err(());
        }

        let Some(parent) = target.parent() else {
            summary.items.push(OpItem {
                dest: target.clone(),
                outcome: OpOutcome::Failed("destino sin carpeta padre para staging".into()),
                src: source_root.cloned(),
            });
            return Err(());
        };
        let active_targets =
            match recover_tree_replacements_in_parent(parent, token, no_replace_rename) {
                Ok(active_targets) => active_targets,
                Err(error) => {
                    summary.items.push(OpItem {
                        dest: target.clone(),
                        outcome: OpOutcome::Failed(format!(
                        "no se pudo recuperar el reemplazo interrumpido antes de continuar: {error}"
                    )),
                        src: source_root.cloned(),
                    });
                    return Err(());
                }
            };
        if active_targets
            .iter()
            .any(|name| super::plan::paths_eq_ci(&parent.join(name), target))
        {
            summary.items.push(OpItem {
                dest: target.clone(),
                outcome: OpOutcome::Failed(
                    "ya hay un reemplazo de esta carpeta en curso; inténtalo de nuevo al terminar"
                        .into(),
                ),
                src: source_root.cloned(),
            });
            return Err(());
        }

        let target_metadata = match std::fs::symlink_metadata(target) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                summary.items.push(OpItem {
                    dest: target.clone(),
                    outcome: OpOutcome::Failed(error.to_string()),
                    src: source_root.cloned(),
                });
                return Err(());
            }
        };
        if target_metadata.file_type().is_symlink() {
            summary.items.push(OpItem {
                dest: target.clone(),
                outcome: OpOutcome::Failed(
                    "el destino es un enlace; no se reemplazó para evitar seguir o retirar su destino"
                        .into(),
                ),
                src: source_root.cloned(),
            });
            return Err(());
        }
        if !target_metadata.is_dir() {
            summary.items.push(OpItem {
                dest: target.clone(),
                outcome: OpOutcome::Failed(
                    "el destino dejó de ser una carpeta; no se reemplazó".into(),
                ),
                src: source_root.cloned(),
            });
            continue;
        }
        let Some(source_root) = source_root else {
            summary.items.push(OpItem {
                dest: target.clone(),
                outcome: OpOutcome::Failed(
                    "reemplazo rechazado: falta la raíz de origen correspondiente en el plan"
                        .into(),
                ),
                src: None,
            });
            return Err(());
        };
        let transaction_id = unique_replacement_id();
        let active = ActiveReplacementGuard::new(&transaction_id);
        let backup = match unique_replacement_backup_path(target) {
            Ok(path) => path,
            Err(error) => {
                summary.items.push(OpItem {
                    dest: target.clone(),
                    outcome: OpOutcome::Failed(format!(
                        "no se pudo reservar respaldo del destino: {error}"
                    )),
                    src: Some(source_root.clone()),
                });
                return Err(());
            }
        };
        let destination = target.clone();
        let staging = parent.join(format!("{TREE_STAGE_PREFIX}{transaction_id}"));
        let record_data = create_tree_replacement_record(
            &destination,
            &staging,
            &backup,
            &transaction_id,
            no_replace_rename,
        );
        let journal = match record_data {
            Ok(path) => path,
            Err(error) => {
                summary.items.push(OpItem {
                    dest: target.clone(),
                    outcome: OpOutcome::Failed(format!(
                        "no se pudo preparar la recuperación del reemplazo: {error}"
                    )),
                    src: Some(source_root.clone()),
                });
                return Err(());
            }
        };
        if let Err(error) = std::fs::create_dir(&staging) {
            let _ = remove_file_if_present(&journal);
            let _ = remove_empty_tree_recovery_dir(parent);
            summary.items.push(OpItem {
                dest: target.clone(),
                outcome: OpOutcome::Failed(format!(
                    "no se pudo preparar el árbol temporal de reemplazo: {error}"
                )),
                src: Some(source_root.clone()),
            });
            return Err(());
        }
        let marker = tree_marker_path(&staging, &transaction_id);
        if let Err(error) = create_tree_replacement_marker(&marker, &transaction_id) {
            let _ = remove_tree_entry_cancelable(&staging, &CancellationToken::new());
            let _ = remove_file_if_present(&journal);
            let _ = remove_empty_tree_recovery_dir(parent);
            summary.items.push(OpItem {
                dest: target.clone(),
                outcome: OpOutcome::Failed(format!(
                    "no se pudo marcar el staging para recuperación: {error}"
                )),
                src: Some(source_root.clone()),
            });
            return Err(());
        }
        replacements.push(TreeReplacementStage {
            destination,
            source: source_root.clone(),
            staging,
            transaction_id: transaction_id.clone(),
            backup,
            journal,
            marker,
            _active: active,
            preserve_recovery: false,
            published: false,
        });
    }
    Ok(replacements)
}

/// Traduce una ruta descendiente a su equivalente bajo staging o bajo el destino publicado.
fn rebase_path(path: &Path, root: &Path, replacement: &Path) -> Option<std::path::PathBuf> {
    let relative = path.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        Some(replacement.to_path_buf())
    } else {
        Some(replacement.join(relative))
    }
}

fn mark_unpublished_replacement_skipped(
    indices: &[usize],
    steps: &[OpStep],
    item_indices: &[Option<usize>],
    summary: &mut OpSummary,
) {
    for idx in indices {
        let Some(item_idx) = item_indices.get(*idx).and_then(|index| *index) else {
            continue;
        };
        if let Some(item) = summary.items.get_mut(item_idx) {
            if matches!(item.outcome, OpOutcome::Done) {
                item.outcome = OpOutcome::Skipped;
                if !steps[*idx].is_dir {
                    summary.bytes_done = summary.bytes_done.saturating_sub(steps[*idx].bytes);
                }
            }
        }
    }
}

fn apply_replacement_directory_permissions(
    replacement: &TreeReplacementStage,
    steps: &[OpStep],
    indices: &[usize],
) -> std::io::Result<()> {
    let mut directories: Vec<_> = indices
        .iter()
        .copied()
        .filter(|idx| steps[*idx].is_dir)
        .collect();
    directories.sort_by_key(|idx| std::cmp::Reverse(steps[*idx].to.components().count()));
    for idx in directories {
        let (Some(source), Some(staged)) = (
            steps[idx].from.as_deref(),
            rebase_path(
                &steps[idx].to,
                &replacement.destination,
                &replacement.staging,
            ),
        ) else {
            continue;
        };
        let permissions = std::fs::metadata(source)?.permissions();
        std::fs::set_permissions(staged, permissions)?;
    }
    Ok(())
}

/// Aparta el destino previo y publica el staging. La API de archivos no ofrece intercambio
/// atómico de directorios en Windows; entre ambos renames la ruta puede estar ausente. Si falla el
/// segundo rename, restaura el anterior sin reemplazar otra ruta que haya aparecido en la carrera.
/// Si la restauración también falla, conserva el respaldo e informa su ubicación.
fn publish_tree_replacement(
    replacement: &mut TreeReplacementStage,
    no_replace_rename: &dyn NoReplaceRename,
) -> Result<TreePublication, String> {
    let backup = &replacement.backup;
    let had_destination = match std::fs::symlink_metadata(&replacement.destination) {
        Ok(_) => {
            no_replace_rename
                .rename_noreplace(&replacement.destination, backup)
                .map_err(|error| {
                    format!("no se pudo apartar el destino anterior sin reemplazarlo: {error}")
                })?;
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(format!("no se pudo revisar el destino anterior: {error}")),
    };

    if let Err(error) =
        no_replace_rename.rename_noreplace(&replacement.staging, &replacement.destination)
    {
        if had_destination {
            if let Err(restore_error) =
                no_replace_rename.rename_noreplace(backup, &replacement.destination)
            {
                replacement.preserve_recovery = true;
                return Err(format!(
                    "no se publicó el árbol nuevo ({error}); no se pudo restaurar el destino anterior ({restore_error}); recuperación pendiente en {}",
                    replacement.journal.display()
                ));
            }
        }
        return Err(format!("no se pudo publicar el árbol nuevo: {error}"));
    }
    replacement.published = true;
    Ok(TreePublication {
        backup: had_destination.then_some(backup.clone()),
        journal: replacement.journal.clone(),
        marker: tree_marker_path(&replacement.destination, &replacement.transaction_id),
    })
}

const TREE_RECOVERY_DIR: &str = ".naygo-recovery-naygo-v1";
const TREE_RECOVERY_OWNER: &str = ".owner";
const TREE_RECOVERY_OWNER_CONTENT: &[u8] = b"Naygo tree replacement recovery directory v1";
const TREE_RECORD_PREFIX: &str = "txn-";
const TREE_STAGE_PREFIX: &str = ".naygo-replace-";
const TREE_BACKUP_PREFIX: &str = ".naygo-backup-";
const TREE_MARKER_PREFIX: &str = ".naygo-replace-marker-";

fn unique_replacement_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{:x}-{:x}-{:x}",
        std::process::id(),
        time,
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn tree_recovery_dir(parent: &Path) -> PathBuf {
    parent.join(TREE_RECOVERY_DIR)
}

fn tree_marker_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{TREE_MARKER_PREFIX}{id}"))
}

fn tree_record_path(parent: &Path, id: &str) -> PathBuf {
    tree_recovery_dir(parent).join(format!("{TREE_RECORD_PREFIX}{id}.json"))
}

fn tree_recovery_dir_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

fn ensure_tree_recovery_dir(parent: &Path) -> std::io::Result<PathBuf> {
    use std::fs::OpenOptions;
    let path = tree_recovery_dir(parent);
    let created = match std::fs::create_dir(&path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error),
    };
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "la ruta reservada de recuperación no es una carpeta normal",
        ));
    }
    let owner = path.join(TREE_RECOVERY_OWNER);
    if created {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&owner)?;
        file.write_all(TREE_RECOVERY_OWNER_CONTENT)?;
        file.sync_all()?;
    } else if !owns_tree_recovery_dir(&path) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "la ruta reservada de recuperación ya existe y no pertenece a Naygo",
        ));
    }
    Ok(path)
}

/// Identifica el directorio auxiliar por su sentinel sin seguir enlaces simbólicos.
pub fn owns_tree_recovery_dir(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    let Ok(owner_metadata) = std::fs::symlink_metadata(path.join(TREE_RECOVERY_OWNER)) else {
        return false;
    };
    metadata.is_dir()
        && !metadata.file_type().is_symlink()
        && owner_metadata.is_file()
        && !owner_metadata.file_type().is_symlink()
        && owner_metadata.len() == TREE_RECOVERY_OWNER_CONTENT.len() as u64
        && std::fs::read(path.join(TREE_RECOVERY_OWNER))
            .ok()
            .as_deref()
            == Some(TREE_RECOVERY_OWNER_CONTENT)
}

fn create_tree_replacement_record(
    destination: &Path,
    staging_root: &Path,
    backup: &Path,
    transaction_id: &str,
    no_replace_rename: &dyn NoReplaceRename,
) -> std::io::Result<PathBuf> {
    use std::fs::OpenOptions;
    let _lock = tree_recovery_dir_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let parent = destination.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destino sin carpeta padre",
        )
    })?;
    let recovery_dir = ensure_tree_recovery_dir(parent)?;

    let destination_name = destination.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "destino sin nombre")
    })?;
    let staging_name = staging_root.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "staging sin nombre")
    })?;
    let backup_name = backup.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "respaldo sin nombre")
    })?;
    let record = TreeReplacementRecord {
        version: 1,
        id: transaction_id.to_owned(),
        destination_name: PathBuf::from(destination_name),
        staging_name: PathBuf::from(staging_name),
        backup_name: PathBuf::from(backup_name),
    };
    let journal = tree_record_path(parent, transaction_id);
    let temporary = recovery_dir.join(format!("tmp-{transaction_id}.json"));
    let bytes = serde_json::to_vec(&record).map_err(std::io::Error::other)?;
    let mut record_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    if let Err(error) = record_file
        .write_all(&bytes)
        .and_then(|()| record_file.sync_all())
    {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    drop(record_file);
    if let Err(error) = no_replace_rename.rename_noreplace(&temporary, &journal) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(journal)
}

fn create_tree_replacement_marker(marker: &Path, id: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(marker)?;
    file.write_all(id.as_bytes())?;
    file.sync_all()
}

struct ActiveReplacementGuard {
    id: String,
}

impl ActiveReplacementGuard {
    fn new(id: &str) -> Self {
        active_replacement_ids()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id.to_owned());
        Self { id: id.to_owned() }
    }
}

impl Drop for ActiveReplacementGuard {
    fn drop(&mut self) {
        active_replacement_ids()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
    }
}

fn active_replacement_ids() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static ACTIVE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    ACTIVE.get_or_init(Default::default)
}

fn is_tree_replacement_active(id: &str) -> bool {
    active_replacement_ids()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(id)
}

/// Recupera Replace interrumpidos en el padre antes de listar sus contenidos o reintentar la op.
/// La consulta y los renames corren en un worker, validan registros/sentinels y nunca pisan una ruta
/// que apareció después. Los registros activos del mismo proceso no se consumen ni se duplican.
pub fn recover_tree_replacements_in_parent(
    parent: &Path,
    token: &CancellationToken,
    no_replace_rename: &dyn NoReplaceRename,
) -> std::io::Result<Vec<PathBuf>> {
    let recovery_dir = tree_recovery_dir(parent);
    let metadata = match std::fs::symlink_metadata(&recovery_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "la ruta reservada de recuperación no es una carpeta normal",
        ));
    }
    if !owns_tree_recovery_dir(&recovery_dir) {
        return Ok(Vec::new());
    }

    let mut active_targets = Vec::new();
    for entry in std::fs::read_dir(&recovery_dir)? {
        if token.is_cancelled() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "recuperación cancelada",
            ));
        }
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(id) = name
            .strip_prefix("tmp-")
            .and_then(|name| name.strip_suffix(".json"))
        {
            if !is_safe_replacement_id(id) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "nombre de temporal de recuperación inválido; no se modificó",
                ));
            }
            if is_tree_replacement_active(id) {
                continue;
            }
            let metadata = std::fs::symlink_metadata(entry.path())?;
            if metadata.is_file() && !metadata.file_type().is_symlink() {
                remove_file_if_present(&entry.path())?;
                continue;
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "temporal de recuperación inválido; no se modificó",
            ));
        }
        if !name.starts_with(TREE_RECORD_PREFIX) || !name.ends_with(".json") {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 16 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "registro de recuperación inválido; se conservaron los archivos relacionados",
            ));
        }
        let record: TreeReplacementRecord =
            serde_json::from_slice(&std::fs::read(entry.path())?)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        validate_tree_replacement_record(name, &record)?;
        if is_tree_replacement_active(&record.id) {
            active_targets.push(record.destination_name);
            continue;
        }
        recover_tree_replacement_record(parent, &record, token, no_replace_rename)?;
    }

    remove_empty_tree_recovery_dir(parent)?;
    Ok(active_targets)
}

fn validate_tree_replacement_record(
    filename: &str,
    record: &TreeReplacementRecord,
) -> std::io::Result<()> {
    let expected_filename = format!("{TREE_RECORD_PREFIX}{}.json", record.id);
    let single_component = |path: &Path| {
        let mut components = path.components();
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    };
    let destination_name = record.destination_name.file_name();
    let backup_name = record.backup_name.file_name();
    let expected_backup_prefix = format!(
        "{TREE_BACKUP_PREFIX}{}-",
        record.destination_name.to_string_lossy()
    );
    if record.version != 1
        || !is_safe_replacement_id(&record.id)
        || filename != expected_filename
        || !single_component(&record.destination_name)
        || !single_component(&record.staging_name)
        || !single_component(&record.backup_name)
        || !record
            .staging_name
            .to_string_lossy()
            .starts_with(TREE_STAGE_PREFIX)
        || !record
            .backup_name
            .to_string_lossy()
            .starts_with(&expected_backup_prefix)
        || destination_name.is_none()
        || backup_name.is_none()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "registro de recuperación no corresponde a una transacción Naygo segura",
        ));
    }
    Ok(())
}

fn is_safe_replacement_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
}

fn recover_tree_replacement_record(
    parent: &Path,
    record: &TreeReplacementRecord,
    token: &CancellationToken,
    no_replace_rename: &dyn NoReplaceRename,
) -> std::io::Result<()> {
    let destination = parent.join(&record.destination_name);
    let staging = parent.join(&record.staging_name);
    let backup = parent.join(&record.backup_name);
    let journal = tree_record_path(parent, &record.id);
    let marker_name = format!("{TREE_MARKER_PREFIX}{}", record.id);
    let marker_at_destination = destination.join(&marker_name);
    let marker_at_staging = staging.join(&marker_name);
    let destination_metadata = optional_symlink_metadata(&destination)?;
    let staging_metadata = optional_symlink_metadata(&staging)?;
    let backup_metadata = optional_symlink_metadata(&backup)?;

    if destination_metadata
        .as_ref()
        .is_some_and(|metadata| metadata.file_type().is_symlink() || !metadata.is_dir())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "la ruta destino está ocupada por un elemento que no es una carpeta",
        ));
    }
    for metadata in [&staging_metadata, &backup_metadata].into_iter().flatten() {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "staging o respaldo no es una carpeta Naygo normal; no se modificó",
            ));
        }
    }
    let marker_published = match destination_metadata.as_ref() {
        Some(_) => marker_matches(&marker_at_destination, &record.id)?,
        None => false,
    };
    if marker_published {
        remove_tree_entry_for_recovery(&backup, token)?;
        remove_tree_entry_for_recovery(&staging, token)?;
        remove_file_if_present(&marker_at_destination)?;
        remove_file_if_present(&journal)?;
        return Ok(());
    }

    match (&destination_metadata, &staging_metadata, &backup_metadata) {
        (None, Some(_), Some(_)) | (None, None, Some(_)) => {
            no_replace_rename.rename_noreplace(&backup, &destination)?;
            remove_tree_entry_for_recovery(&staging, token)?;
            remove_file_if_present(&marker_at_staging)?;
            remove_file_if_present(&journal)?;
            Ok(())
        }
        (None, Some(_), None) => {
            remove_tree_entry_for_recovery(&staging, token)?;
            remove_file_if_present(&marker_at_staging)?;
            remove_file_if_present(&journal)?;
            Ok(())
        }
        (Some(_), Some(_), None) => {
            // The old target was never moved. Discard only our identified staging tree.
            remove_tree_entry_for_recovery(&staging, token)?;
            remove_file_if_present(&marker_at_staging)?;
            remove_file_if_present(&journal)?;
            Ok(())
        }
        (Some(_), Some(_), Some(_)) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "otro proceso ocupó el destino durante Replace; se conservaron destino, staging y respaldo",
        )),
        (Some(_), None, Some(_)) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "no se pudo confirmar si el árbol publicado pertenece a Naygo; se conservó el respaldo",
        )),
        (Some(_), None, None) | (None, None, None) => {
            remove_file_if_present(&journal)?;
            Ok(())
        }
    }
}

fn remove_tree_entry_for_recovery(path: &Path, token: &CancellationToken) -> std::io::Result<()> {
    match remove_tree_entry_cancelable(path, token)? {
        true => Ok(()),
        false => Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "recuperación cancelada; el registro se conserva para reintentar",
        )),
    }
}

fn optional_symlink_metadata(path: &Path) -> std::io::Result<Option<std::fs::Metadata>> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn marker_matches(marker: &Path, expected: &str) -> std::io::Result<bool> {
    match optional_symlink_metadata(marker)? {
        Some(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            Ok(std::fs::read(marker)? == expected.as_bytes())
        }
        Some(_) => Ok(false),
        None => Ok(false),
    }
}

fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_empty_tree_recovery_dir(parent: &Path) -> std::io::Result<()> {
    let _lock = tree_recovery_dir_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let path = tree_recovery_dir(parent);
    if !owns_tree_recovery_dir(&path) {
        return Ok(());
    }
    let mut entries = match std::fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    match entries.next() {
        None => match std::fs::remove_dir(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        },
        Some(Ok(entry)) if entry.file_name() == TREE_RECOVERY_OWNER => {
            if entries.next().is_some() {
                return Ok(());
            }
            remove_file_if_present(&path.join(TREE_RECOVERY_OWNER))?;
            match std::fs::remove_dir(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
        Some(Ok(_)) => Ok(()),
        Some(Err(error)) => Err(error),
    }
}

fn finalize_tree_replacement(publication: &TreePublication) -> std::io::Result<()> {
    remove_file_if_present(&publication.marker)?;
    remove_file_if_present(&publication.journal)?;
    if let Some(parent) = publication.journal.parent().and_then(Path::parent) {
        remove_empty_tree_recovery_dir(parent)?;
    }
    Ok(())
}

fn unique_replacement_backup_path(destination: &Path) -> std::io::Result<std::path::PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_BACKUP_ID: AtomicU64 = AtomicU64::new(0);
    let parent = destination.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destino sin carpeta padre",
        )
    })?;
    let name = destination
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("tree"));
    for _ in 0..128 {
        let id = NEXT_BACKUP_ID.fetch_add(1, Ordering::Relaxed);
        let mut backup_name = std::ffi::OsString::from(".naygo-backup-");
        backup_name.push(name);
        backup_name.push(format!("-{}-{id}", std::process::id()));
        let candidate = parent.join(backup_name);
        match std::fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no se encontró un nombre libre para el respaldo",
    ))
}

fn remove_tree_entry_cancelable(path: &Path, token: &CancellationToken) -> std::io::Result<bool> {
    if token.is_cancelled() {
        return Ok(false);
    }
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(error) => return Err(error),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        remove_dir_all_cancelable(path, token)
    } else {
        std::fs::remove_file(path).map(|()| true)
    }
}

/// Un Move de reemplazo retira sólo los archivos que siguen teniendo el contenido publicado.
/// Si un origen cambió mientras se copiaba, lo conserva y reporta la discrepancia.
fn remove_published_move_sources(
    replacement: &TreeReplacementStage,
    steps: &[OpStep],
    indices: &[usize],
    item_indices: &[Option<usize>],
    summary: &mut OpSummary,
    token: &CancellationToken,
) {
    for idx in indices.iter().copied().filter(|idx| !steps[*idx].is_dir) {
        if token.is_cancelled() {
            return;
        }
        let Some(item_idx) = item_indices.get(idx).and_then(|index| *index) else {
            continue;
        };
        if !matches!(summary.items[item_idx].outcome, OpOutcome::Done) {
            continue;
        }
        let Some(source) = steps[idx].from.as_deref() else {
            continue;
        };
        if !super::plan::path_starts_with_ci(source, &replacement.source) {
            continue;
        }
        let equal = match files_equal_content(source, &steps[idx].to, token) {
            Ok(Some(equal)) => equal,
            Ok(None) => return,
            Err(error) => {
                summary.items[item_idx].outcome = OpOutcome::Failed(format!(
                    "árbol publicado; no se pudo revalidar el origen y se conserva: {error}"
                ));
                continue;
            }
        };
        if !equal {
            summary.items[item_idx].outcome = OpOutcome::Failed(
                "árbol publicado; el origen cambió durante el movimiento y se conserva".into(),
            );
            continue;
        }
        if let Err(error) = std::fs::remove_file(source) {
            if error.kind() != std::io::ErrorKind::NotFound {
                summary.items[item_idx].outcome =
                    OpOutcome::Failed(format!("árbol publicado; no se retiró el origen: {error}"));
            }
        }
    }

    for idx in indices.iter().copied().filter(|idx| steps[*idx].is_dir) {
        if token.is_cancelled() {
            return;
        }
        let Some(source) = steps[idx].from.as_deref() else {
            continue;
        };
        if !super::plan::path_starts_with_ci(source, &replacement.source) {
            continue;
        }
        if let Err(error) = remove_empty_move_source(source) {
            if let Some(item_idx) = item_indices.get(idx).and_then(|index| *index) {
                summary.items[item_idx].outcome = OpOutcome::Failed(format!(
                    "árbol publicado; no se retiró la carpeta origen: {error}"
                ));
            }
        }
    }
}

fn files_equal_content(
    left: &Path,
    right: &Path,
    token: &CancellationToken,
) -> std::io::Result<Option<bool>> {
    let mut left = match std::fs::File::open(left) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Some(true)),
        Err(error) => return Err(error),
    };
    let mut right = std::fs::File::open(right)?;
    let mut left_buf = [0u8; 64 * 1024];
    let mut right_buf = [0u8; 64 * 1024];
    loop {
        if token.is_cancelled() {
            return Ok(None);
        }
        let left_len = std::io::Read::read(&mut left, &mut left_buf)?;
        let right_len = std::io::Read::read(&mut right, &mut right_buf)?;
        if left_len != right_len || left_buf[..left_len] != right_buf[..right_len] {
            return Ok(Some(false));
        }
        if left_len == 0 {
            return Ok(Some(true));
        }
    }
}

/// Acumulados de la operación en el momento de copiar este archivo, para que el
/// progreso por bytes refleje el avance total (no solo el del archivo actual).
#[derive(Clone, Copy)]
struct ProgressCtx {
    /// Bytes de los archivos YA terminados de esta operación.
    prev_bytes: u64,
    /// Total de bytes del plan completo.
    bytes_total: u64,
    /// Archivos ya terminados.
    files_done: usize,
    /// Total de archivos del plan.
    files_total: usize,
}

/// Ejecuta un paso individual. Devuelve `(ruta_registrada, resultado, bytes_sumados,
/// cuenta_como_archivo)`.
#[allow(clippy::too_many_arguments)]
fn exec_step(
    step: &OpStep,
    kind: &OpKind,
    conflict: ConflictPolicy,
    token: &CancellationToken,
    tx: &Sender<OpMsg>,
    conflict_rx: &Receiver<ConflictDecision>,
    applied_all: &mut Option<ConflictAction>,
    progress: ProgressCtx,
    no_replace_rename: &dyn NoReplaceRename,
) -> (std::path::PathBuf, OpOutcome, u64, bool) {
    match kind {
        OpKind::Copy | OpKind::Duplicate => exec_copy_step(
            step,
            conflict,
            token,
            false,
            tx,
            conflict_rx,
            applied_all,
            progress,
            no_replace_rename,
        ),
        OpKind::Move => exec_copy_step(
            step,
            conflict,
            token,
            true,
            tx,
            conflict_rx,
            applied_all,
            progress,
            no_replace_rename,
        ),
        OpKind::Delete { to_trash } => {
            if *to_trash {
                // El motor solo hace borrado permanente; la papelera la maneja platform.
                (
                    step.to.clone(),
                    OpOutcome::Failed("papelera se maneja en platform".into()),
                    0,
                    !step.is_dir,
                )
            } else {
                let outcome = exec_delete(step, token);
                let done = matches!(outcome, OpOutcome::Done);
                (
                    step.to.clone(),
                    outcome,
                    if done { step.bytes } else { 0 },
                    true,
                )
            }
        }
        // BatchRename: cada paso ya viene con su par from→to (ordenado por
        // dependencia en el plan); se ejecuta igual que un rename simple, pero nunca pisa un
        // destino que aparezca desde la planificación.
        OpKind::Rename { .. } | OpKind::BatchRename { .. } => {
            let outcome = exec_rename(step, no_replace_rename);
            (step.to.clone(), outcome, 0, true)
        }
        OpKind::CreateDir { .. } => {
            let outcome = exec_create_dir(&step.to);
            (step.to.clone(), outcome, 0, false)
        }
        OpKind::CreateFile { .. } => {
            let outcome = exec_create_file(&step.to);
            (step.to.clone(), outcome, 0, true)
        }
        OpKind::Compress { .. } | OpKind::Extract => {
            // El worker de zip (ui-slint) maneja estas ops; nunca deberían llegar al engine de pasos.
            debug_assert!(false, "Compress/Extract no pasan por exec_step");
            (
                step.to.clone(),
                OpOutcome::Failed("compress/extract fuera del engine".into()),
                0,
                false,
            )
        }
    }
}

/// Resuelve QUÉ acción aplicar a un choque (el destino `existing` ya existe). Encapsula la lógica
/// de la política y de "aplicar a todos":
///
/// - Políticas fijas (`Skip`/`Overwrite`/`Rename`) → su acción directa.
/// - `Ask` → si hay una acción memorizada (`applied_all`), la reusa SIN preguntar; si no, emite
///   `OpMsg::Conflict` (con metadatos) y espera la `ConflictDecision` de la UI, memorizando la
///   acción si pidió "aplicar a todos".
///
/// `SkipIdentical` es una POLÍTICA, no una acción terminal: cada vez que aplica (sea memorizada o
/// elegida por ítem) compara los metadatos de `existing` vs `incoming` con `are_identical`. Si son
/// idénticos devuelve `Skip` (no recopia lo que ya está igual); si difieren, vuelve a preguntar a
/// la UI (un choque real que merece decisión). Por eso esta función nunca devuelve `SkipIdentical`:
/// lo traduce a `Skip` u otra acción concreta.
#[allow(clippy::too_many_arguments)]
fn resolve_action(
    conflict: ConflictPolicy,
    existing: &Path,
    incoming: &Path,
    token: &CancellationToken,
    tx: &Sender<OpMsg>,
    conflict_rx: &Receiver<ConflictDecision>,
    applied_all: &mut Option<ConflictAction>,
) -> ConflictAction {
    match conflict {
        ConflictPolicy::Skip => ConflictAction::Skip,
        ConflictPolicy::Overwrite => ConflictAction::Overwrite,
        ConflictPolicy::Rename => ConflictAction::Rename,
        ConflictPolicy::Ask => {
            // Acción memorizada con "aplicar a todos". `SkipIdentical` memorizado NO se devuelve
            // tal cual: se evalúa por ítem (idénticos → Skip; si difieren, cae a preguntar).
            if let Some(prev) = applied_all.clone() {
                match prev {
                    ConflictAction::SkipIdentical => {
                        if super::are_identical(existing, incoming) {
                            return ConflictAction::Skip;
                        }
                        // Difieren: este choque merece decisión → preguntar (cae al loop de abajo).
                    }
                    other => return other,
                }
            }
            // Bucle de pregunta: normalmente se pregunta UNA vez. Pero "Saltar idénticos" sobre un
            // par que DIFIERE no es una decisión terminal (la palabra es "saltar", nunca debe
            // sobrescribir en silencio): si difieren, se vuelve a mostrar el modal para que el
            // usuario decida de verdad qué hacer con ESE archivo. Idénticos → Skip y listo.
            loop {
                // Preguntar a la UI: prompt ENRIQUECIDO con metadatos de ambos lados (la UI los
                // muestra LADO A LADO). El motor ya tiene las rutas a mano.
                let _ = tx.send(OpMsg::Conflict(ConflictPrompt::from_paths(
                    existing.to_path_buf(),
                    incoming.to_path_buf(),
                )));
                // Esperar la decisión sin colgar si se cancela: poll con timeout.
                let decision = loop {
                    if token.is_cancelled() {
                        return ConflictAction::Skip;
                    }
                    match conflict_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                        Ok(d) => break d,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            return ConflictAction::Skip;
                        }
                    }
                };
                // "Saltar idénticos" elegido suelto o como política: evaluarlo YA para este ítem.
                // Idénticos → Skip. Si DIFIEREN: memorizar la política si pidió "aplicar a todos"
                // (para que los siguientes idénticos se salten solos) y volver a preguntar por ESTE
                // archivo (nunca sobrescribir sin que el usuario lo confirme explícitamente).
                if matches!(decision.action, ConflictAction::SkipIdentical) {
                    if super::are_identical(existing, incoming) {
                        if decision.apply_all {
                            *applied_all = Some(ConflictAction::SkipIdentical);
                        }
                        return ConflictAction::Skip;
                    }
                    if decision.apply_all {
                        *applied_all = Some(ConflictAction::SkipIdentical);
                    }
                    // Difieren: re-preguntar (el loop vuelve a enviar el prompt).
                    continue;
                }
                // Cualquier otra acción es terminal. "Aplicar a todos" no aplica a `RenameTo` (cada
                // archivo necesita su propio nombre); la UI ya lo deshabilita, pero por las dudas
                // no lo memorizamos.
                if decision.apply_all && !matches!(decision.action, ConflictAction::RenameTo(_)) {
                    *applied_all = Some(decision.action.clone());
                }
                return decision.action;
            }
        }
    }
}

/// Ejecuta un paso de copia (o mover, si `is_move`). Maneja carpetas (crea el dir),
/// conflictos según la política, y copia archivos por buffers (cancelable).
#[allow(clippy::too_many_arguments)]
fn exec_copy_step(
    step: &OpStep,
    conflict: ConflictPolicy,
    token: &CancellationToken,
    is_move: bool,
    tx: &Sender<OpMsg>,
    conflict_rx: &Receiver<ConflictDecision>,
    applied_all: &mut Option<ConflictAction>,
    progress: ProgressCtx,
    no_replace_rename: &dyn NoReplaceRename,
) -> (std::path::PathBuf, OpOutcome, u64, bool) {
    // Paso de carpeta: asegurar el directorio destino, no cuenta como archivo.
    if step.is_dir {
        let outcome = match std::fs::create_dir_all(&step.to) {
            Err(error) => OpOutcome::Failed(error.to_string()),
            Ok(()) if is_move && !token.is_cancelled() => match step.from.as_deref() {
                Some(from) => match remove_empty_move_source(from) {
                    Ok(()) => OpOutcome::Done,
                    Err(error) => OpOutcome::Failed(format!(
                        "destino creado pero no se retiró el directorio origen: {error}"
                    )),
                },
                None => OpOutcome::Done,
            },
            Ok(()) => OpOutcome::Done,
        };
        return (step.to.clone(), outcome, 0, false);
    }

    let from = match &step.from {
        Some(p) => p.clone(),
        None => {
            return (
                step.to.clone(),
                OpOutcome::Failed("paso sin origen".into()),
                0,
                true,
            )
        }
    };

    // Resolver el destino según la política de conflictos. Con `Ask` (ops-B), si el
    // destino existe se consulta a la UI por ítem: emite `OpMsg::Conflict` y bloquea
    // esperando una `ConflictDecision` por `conflict_rx`. Con "aplicar a todos", la acción
    // elegida se memoriza en `applied_all` y los choques siguientes no vuelven a preguntar.
    let mut overwrite_destination = false;
    let to = if step.to.exists() {
        let effective: ConflictAction = resolve_action(
            conflict,
            &step.to,
            &from,
            token,
            tx,
            conflict_rx,
            applied_all,
        );
        match effective {
            ConflictAction::Skip => return (step.to.clone(), OpOutcome::Skipped, 0, true),
            ConflictAction::Rename => super::dedup_name(&step.to, &|p: &Path| p.exists()),
            ConflictAction::Overwrite => {
                overwrite_destination = true;
                step.to.clone()
            }
            // Nombre elegido por el usuario: destino = mismo directorio, nombre nuevo. Si ESE
            // nombre también existe, `dedup_name` lo desambigua (salvaguarda: nunca pisar). Si el
            // nombre quedó vacío/inválido (no debería: la UI valida), se cae a `dedup_name` del
            // destino original para no fallar.
            ConflictAction::RenameTo(name) => {
                let chosen = if name.trim().is_empty() {
                    step.to.clone()
                } else {
                    step.to.with_file_name(name)
                };
                super::dedup_name(&chosen, &|p: &Path| p.exists())
            }
            // "Renombrar antiguo": apartar el EXISTENTE a un nombre " (N)" libre y dejar entrar el
            // nuevo con su nombre original (`step.to`). El rename sin reemplazo cierra la carrera
            // si otro proceso ocupa el respaldo tras dedup_name; ante cualquier error, no se pisa
            // el destino y se reporta el paso como fallido.
            ConflictAction::RenameExisting => {
                let apartado = super::dedup_name(&step.to, &|p: &Path| p.exists());
                if let Err(e) = no_replace_rename.rename_noreplace(&step.to, &apartado) {
                    return (
                        step.to.clone(),
                        OpOutcome::Failed(format!(
                            "no se pudo renombrar el archivo existente: {e}"
                        )),
                        0,
                        true,
                    );
                }
                // El existente ya no está en `step.to`: el nuevo entra con su nombre original.
                step.to.clone()
            }
            // `SkipIdentical` ya se resolvió a Skip/Overwrite (o re-preguntó) dentro de
            // `resolve_action`; nunca debería llegar aquí. Por las dudas, tratamos como Skip.
            ConflictAction::SkipIdentical => return (step.to.clone(), OpOutcome::Skipped, 0, true),
        }
    } else {
        step.to.clone()
    };

    // Asegurar que el directorio padre del destino existe.
    if let Some(parent) = to.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return (to.clone(), OpOutcome::Failed(e.to_string()), 0, true);
        }
    }

    // Mover en el mismo volumen: rename rápido, con NOREPLACE cuando el usuario no autorizó
    // sobrescribir. En Linux `rename(2)` reemplaza un destino que aparezca tras el check de
    // conflicto; `rename_noreplace` cierra esa carrera. Si el kernel informa EXDEV, copiamos
    // a un temporal junto al destino y retiramos el origen sólo después de publicar el archivo.
    if is_move {
        let moved = if overwrite_destination {
            std::fs::rename(&from, &to)
        } else {
            no_replace_rename.rename_noreplace(&from, &to)
        };
        match moved {
            Ok(()) => return (to, OpOutcome::Done, step.bytes, true),
            Err(error)
                if error.kind() == std::io::ErrorKind::CrossesDevices || overwrite_destination => {}
            Err(error) => return (to, OpOutcome::Failed(error.to_string()), 0, true),
        }
    }

    // Closure que arma y envía el progreso por bytes durante la copia. `this_file`
    // son los bytes de ESTE archivo copiados hasta ahora; se suman a los bytes de
    // los archivos ya terminados (`prev_bytes`) para el total acumulado de la op.
    let on_bytes = |this_file: u64| {
        let _ = tx.send(OpMsg::Progress(OpProgress {
            bytes_done: progress.prev_bytes + this_file,
            bytes_total: progress.bytes_total,
            files_done: progress.files_done,
            files_total: progress.files_total,
            current: from.clone(),
        }));
    };

    match copy_buffered(&from, &to, overwrite_destination, token, on_bytes, || {}) {
        Ok(true) => {
            if is_move {
                // Borrar el origen tras una copia exitosa (fin del move cross-volume).
                if let Err(e) = std::fs::remove_file(&from) {
                    return (
                        to,
                        OpOutcome::Failed(format!("copiado pero no se borró el origen: {e}")),
                        step.bytes,
                        true,
                    );
                }
            }
            (to, OpOutcome::Done, step.bytes, true)
        }
        Ok(false) => {
            // El parcial vive en un temporal junto al destino y se limpia al descartarlo.
            // El destino anterior nunca se abre ni se trunca antes de publicar.
            (to, OpOutcome::Skipped, 0, true)
        }
        Err(e) => {
            // El temporal se limpia al salir; cualquier destino previo se conserva.
            (to, OpOutcome::Failed(e.to_string()), 0, true)
        }
    }
}

/// Retira únicamente un directorio de origen vacío después de mover sus descendientes.
/// Los directorios que aún contienen archivos (por ejemplo, omitidos o fallidos) se conservan.
/// No sigue ni elimina symlinks/junctions y trata una ruta ya ausente como trabajo terminado.
fn remove_empty_move_source(from: &Path) -> std::io::Result<()> {
    let metadata = match std::fs::symlink_metadata(from) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(());
    }
    match std::fs::remove_dir(from) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// Copia `from` → `to` por bloques de 256KB, chequeando la cancelación y la pausa
/// entre bloques. Devuelve `Ok(true)` si copió todo, `Ok(false)` si se canceló a
/// media copia, `Err` ante un error de I/O.
///
/// `on_bytes` recibe los bytes copiados de ESTE archivo hasta el momento. Se llama
/// con throttle (cada ~100 ms, no por bloque) durante la copia y una vez al final
/// con el total del archivo; así una copia larga emite progreso en vivo sin inundar
/// el canal. El llamador arma el `OpProgress` sumando los bytes de archivos previos.
fn copy_buffered(
    from: &Path,
    to: &Path,
    overwrite: bool,
    token: &CancellationToken,
    mut on_bytes: impl FnMut(u64),
    mut after_chunk: impl FnMut(),
) -> std::io::Result<bool> {
    let mut reader = std::fs::File::open(from)?;
    let source_permissions = std::fs::metadata(from)?.permissions();
    let parent = to
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut writer = tempfile::Builder::new()
        .prefix(".naygo-copy-")
        .tempfile_in(parent)?;
    let mut buf = vec![0u8; BUF_SIZE];
    let mut copied: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    loop {
        // Pausa: suspende sin cerrar el archivo; retorna al reanudar o cancelar.
        token.wait_if_paused();
        if token.is_cancelled() {
            return Ok(false);
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        copied += n as u64;
        after_chunk();
        if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
            on_bytes(copied);
            last_emit = std::time::Instant::now();
        }
    }
    writer.flush()?;
    if token.is_cancelled() {
        return Ok(false);
    }
    std::fs::set_permissions(writer.path(), source_permissions)?;
    // Publicar sólo después de copiar y aplicar permisos: persist reemplaza atómicamente cuando
    // se autorizó Overwrite; persist_noclobber vuelve a validar el destino al final y no puede
    // pisar una ruta que apareció durante la copia. El temporal comparte filesystem con `to`.
    let persisted = if overwrite {
        writer.persist(to)
    } else {
        writer.persist_noclobber(to)
    };
    if let Err(error) = persisted {
        return Err(error.error);
    }
    // Emisión final sólo después de que la publicación tuvo éxito.
    on_bytes(copied);
    Ok(true)
}

/// Abstrae la operación de renombrado rápido de Move para mantener Win32 fuera de `core`.
/// Las implementaciones de plataforma deben fallar si el destino ya existe. El fallback nativo
/// de `core` queda para consumidores autónomos; en Windows la UI inyecta el adaptador de `platform`.
pub trait NoReplaceRename: Send + Sync {
    fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()>;
}

/// Implementación autónoma del core: atómica en Linux y best-effort en Windows/otros targets.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeNoReplaceRename;

impl NoReplaceRename for NativeNoReplaceRename {
    fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        rename_noreplace(from, to)
    }
}

/// Renombra sin reemplazar un destino que pudiera aparecer durante la operación.
#[cfg(target_os = "linux")]
fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

/// Windows permite que `std::fs::rename` reemplace el destino; comprobar primero evita
/// sobrescrituras ante conflictos ya visibles para consumidores autónomos, pero no cierra una
/// creación concurrente. La UI debe inyectar la implementación de `platform`, que sí renombra
/// atómicamente sin reemplazar; el fallback entre volúmenes permanece cancelable en el motor.
#[cfg(windows)]
fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(to) {
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "el destino ya existe",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    std::fs::rename(from, to)
}

/// Fallback para targets fuera del alfa Ubuntu/Windows: comprueba antes de `rename`, pero la
/// operación no ofrece garantía atómica de no reemplazo en todos los sistemas.
#[cfg(not(any(target_os = "linux", windows)))]
fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    if to.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "el destino apareció antes del movimiento",
        ));
    }
    std::fs::rename(from, to)
}

/// Borra (permanente) un archivo o carpeta del paso. El borrado de carpetas es
/// CANCELABLE por entrada (un `remove_dir_all` crudo no se puede interrumpir: una
/// carpeta grande congelaría el worker hasta terminar). Cancelar a mitad deja lo no
/// alcanzado en disco y reporta el paso como Skipped (la op entera sale como Cancelled).
fn exec_delete(step: &OpStep, token: &CancellationToken) -> OpOutcome {
    let target = step.from.as_ref().unwrap_or(&step.to);
    if step.is_dir {
        if token.is_cancelled() {
            return OpOutcome::Skipped;
        }
        match remove_dir_tolerante(target) {
            Ok(()) => OpOutcome::Done,
            // Compatibilidad con journals antiguos, donde una carpeta completa era un solo paso.
            Err(e) if e.kind() == std::io::ErrorKind::DirectoryNotEmpty => {
                match remove_dir_all_cancelable(target, token) {
                    Ok(true) => OpOutcome::Done,
                    Ok(false) => OpOutcome::Skipped,
                    Err(e) => OpOutcome::Failed(e.to_string()),
                }
            }
            Err(e) => OpOutcome::Failed(e.to_string()),
        }
    } else {
        match std::fs::remove_file(target) {
            Ok(()) => OpOutcome::Done,
            Err(e) => OpOutcome::Failed(e.to_string()),
        }
    }
}

/// Borrado recursivo CANCELABLE de un directorio: recorre el árbol con una pila
/// propia (post-orden: primero el contenido, luego el dir vacío) y chequea `token`
/// por entrada (mismo patrón que `sizing::dir_size_walk`). Devuelve `Ok(true)` si
/// borró todo, `Ok(false)` si se canceló a mitad (queda en disco lo no alcanzado),
/// `Err` ante un error de I/O (permiso denegado, archivo en uso). Una ruta que
/// desaparece a mitad del recorrido NO es error (el objetivo —que no exista— ya se
/// cumplió). No sigue symlinks: borra el enlace, nunca el destino del enlace.
fn remove_dir_all_cancelable(root: &Path, token: &CancellationToken) -> std::io::Result<bool> {
    // Contrato de `remove_dir_all`: la raíz debe ser un directorio; un archivo aquí
    // es un error de planificación (no se borra un archivo suelto por esta vía).
    let root_meta = std::fs::symlink_metadata(root)?;
    if !root_meta.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("no es un directorio: {}", root.display()),
        ));
    }
    // Pila de trabajo: (ruta, contenido_ya_borrado). El bool marca el pase de
    // post-orden: al re-encontrar un dir con el flag en true, ya está vacío y se borra.
    let mut stack: Vec<(std::path::PathBuf, bool)> = vec![(root.to_path_buf(), false)];
    while let Some((path, emptied)) = stack.pop() {
        if token.is_cancelled() {
            return Ok(false);
        }
        if emptied {
            remove_dir_tolerante(&path)?;
            continue;
        }
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            // Desapareció a mitad del borrado (otro proceso): objetivo cumplido.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        let ft = meta.file_type();
        if ft.is_symlink() {
            // Enlace: borrar el ENLACE, nunca seguirlo. En Windows un symlink a
            // carpeta/junction solo se borra con remove_dir (remove_file falla).
            if std::fs::remove_file(&path).is_err() {
                remove_dir_tolerante(&path)?;
            }
        } else if ft.is_dir() {
            stack.push((path.clone(), true));
            for entry in std::fs::read_dir(&path)? {
                let entry = entry?;
                stack.push((entry.path(), false));
            }
        } else {
            clear_readonly(&path);
            remove_file_tolerante(&path)?;
        }
    }
    Ok(true)
}

/// Borra un archivo tolerando que haya desaparecido (NotFound = objetivo cumplido).
fn remove_file_tolerante(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Borra un directorio (vacío o enlace) tolerando que haya desaparecido.
fn remove_dir_tolerante(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Quita el atributo de solo-lectura antes de borrar (Windows no deja borrar
/// archivos readonly; el `remove_dir_all` de std hace esto internamente).
// allow: el lint advierte por Unix (world-writable), pero esta fn es Windows-only.
#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)]
fn clear_readonly(path: &Path) {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

/// En no-Windows el atributo readonly no bloquea el borrado: no-op.
#[cfg(not(windows))]
fn clear_readonly(_path: &Path) {}

/// Renombra `from` → `to` (mismo directorio, nombre nuevo) sin pisar un destino concurrente.
fn exec_rename(step: &OpStep, no_replace_rename: &dyn NoReplaceRename) -> OpOutcome {
    let from = match &step.from {
        Some(p) => p,
        None => return OpOutcome::Failed("rename sin origen".into()),
    };
    // En Windows, un cambio que solo altera el case apunta al mismo nombre lógico. Debe usar
    // rename normal para que NTFS actualice la grafía; el adaptador no-clobber lo interpretaría
    // como un destino ya ocupado. No puede reemplazar otro archivo: ambas rutas son el mismo
    // componente dentro del mismo directorio lógico.
    #[cfg(windows)]
    let result = if crate::path_identity::key(from) == crate::path_identity::key(&step.to) {
        std::fs::rename(from, &step.to)
    } else {
        no_replace_rename.rename_noreplace(from, &step.to)
    };
    #[cfg(not(windows))]
    let result = no_replace_rename.rename_noreplace(from, &step.to);

    match result {
        Ok(()) => OpOutcome::Done,
        Err(e) => OpOutcome::Failed(e.to_string()),
    }
}

/// Crea una carpeta vacía (incluyendo padres faltantes).
fn exec_create_dir(to: &Path) -> OpOutcome {
    match std::fs::create_dir_all(to) {
        Ok(()) => OpOutcome::Done,
        Err(e) => OpOutcome::Failed(e.to_string()),
    }
}

/// Crea un archivo vacío (falla si ya existe, para no pisar contenido).
fn exec_create_file(to: &Path) -> OpOutcome {
    if let Some(parent) = to.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return OpOutcome::Failed(e.to_string());
        }
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
    {
        Ok(_) => OpOutcome::Done,
        Err(e) => OpOutcome::Failed(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::plan::plan;
    use super::super::{ConflictPolicy, OpKind, OpRequest};
    use super::*;
    use std::fs;
    use std::sync::mpsc;

    fn run(req: OpRequest) -> (Vec<OpMsg>, OpSummary) {
        let p = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let summary = run_plan(&p, &req.kind, req.conflict, &token, &tx, &crx, None);
        drop(tx);
        let msgs: Vec<OpMsg> = rx.into_iter().collect();
        (msgs, summary)
    }

    #[test]
    fn copia_grande_emite_progreso_durante_la_copia() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("big.bin");
        // 2 MiB → 8 bloques de 256K → debe emitir más de un Progress durante la copia.
        fs::write(&src, vec![7u8; 2 * 1024 * 1024]).unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = OpRequest {
            kind: OpKind::Copy,
            sources: vec![src],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Overwrite,
        };
        let p = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        run_plan(
            &p,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &token,
            &tx,
            &crx,
            None,
        );
        drop(tx);
        let progresos = rx
            .iter()
            .filter(|m| matches!(m, OpMsg::Progress(_)))
            .count();
        assert!(
            progresos >= 2,
            "esperaba >=2 Progress durante la copia, hubo {progresos}"
        );
        assert_eq!(
            fs::read(dest.join("big.bin")).unwrap().len(),
            2 * 1024 * 1024
        );
    }

    #[test]
    fn create_dir_anidada_crea_la_jerarquia_en_disco() {
        // El nombre `a\b\c` (ruta relativa anidada) debe materializar dir/a/b/c en disco.
        // Cubre el camino completo plan() + run_plan() (exec_create_dir usa create_dir_all).
        let dir = tempfile::tempdir().unwrap();
        let req = OpRequest {
            kind: OpKind::CreateDir {
                name: "a\\b\\c".into(),
            },
            sources: vec![],
            dest_dir: Some(dir.path().to_path_buf()),
            conflict: ConflictPolicy::Ask,
        };
        let (_msgs, summary) = run(req);
        assert!(
            !summary
                .items
                .iter()
                .any(|i| matches!(i.outcome, OpOutcome::Failed(_))),
            "no debe fallar ningún paso"
        );
        let anidada = dir.path().join("a").join("b").join("c");
        assert!(anidada.is_dir(), "la cadena anidada debe existir en disco");
    }

    #[test]
    fn create_dir_anidada_no_escapa_del_destino() {
        // SEGURIDAD de extremo a extremo: `..` se rechaza en el plan, así que NUNCA se crea
        // nada fuera de la carpeta destino.
        let parent = tempfile::tempdir().unwrap();
        let dest = parent.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        let req = OpRequest {
            kind: OpKind::CreateDir {
                name: "..\\fuera".into(),
            },
            sources: vec![],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Ask,
        };
        // El plan rechaza el nombre: no hay ejecución posible.
        assert!(matches!(
            super::super::plan::plan(&req),
            Err(super::super::plan::PlanError::InvalidName(_))
        ));
        assert!(
            !parent.path().join("fuera").exists(),
            "no debe crearse nada fuera del destino"
        );
    }

    #[test]
    fn copy_archivo_crea_destino() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"contenido").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = OpRequest {
            kind: OpKind::Copy,
            sources: vec![src],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Overwrite,
        };
        let (_msgs, summary) = run(req);
        assert!(dest.join("a.txt").exists());
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"contenido");
        assert_eq!(summary.count_done(), 1);
    }

    #[test]
    fn copy_conflicto_skip_no_sobrescribe() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"nuevo").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("a.txt"), b"viejo").unwrap();
        let req = OpRequest {
            kind: OpKind::Copy,
            sources: vec![src],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Skip,
        };
        let (_m, summary) = run(req);
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"viejo");
        assert_eq!(summary.count_skipped(), 1);
    }

    #[test]
    fn copy_conflicto_overwrite_reemplaza() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"nuevo").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("a.txt"), b"viejo").unwrap();
        let req = OpRequest {
            kind: OpKind::Copy,
            sources: vec![src],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Overwrite,
        };
        let (_m, _s) = run(req);
        assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"nuevo");
    }

    #[test]
    fn replace_borra_el_destino_y_deja_solo_el_origen_con_origen_intacto() {
        // Conflicto de carpeta resuelto con "Reemplazar": el destino existe y tiene un archivo
        // EXTRA que no está en el origen. Tras aplicar Replace (pre_delete) + copiar, el destino
        // debe quedar con SOLO el contenido del origen (el extra desapareció) y el ORIGEN intacto.
        use crate::ops::{apply_folder_decision, folder_conflicts, FolderDecision};
        let dir = tempfile::tempdir().unwrap();
        // Origen: carpeta/ con a.txt ("nuevo").
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"nuevo").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&src, fs::Permissions::from_mode(0o750)).unwrap();
        }
        // Destino: dst/carpeta/ con extra.txt ("viejo") — archivo que solo existe en el destino.
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let dest_root = dest.join("carpeta");
        fs::create_dir(&dest_root).unwrap();
        fs::write(dest_root.join("extra.txt"), b"viejo").unwrap();

        let req = super::super::transfer(false, vec![src.clone()], dest.clone());
        // Detectar el conflicto y aplicar Replace al plan.
        let conflicts = folder_conflicts(&req);
        assert_eq!(conflicts.len(), 1);
        let mut p = plan(&req).unwrap();
        apply_folder_decision(&mut p, &conflicts[0].dest_root, FolderDecision::Replace);
        assert_eq!(p.pre_delete, vec![dest_root.clone()]);

        // Ejecutar el plan (con el pre_delete dentro del motor).
        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let summary = run_plan(&p, &req.kind, req.conflict, &token, &tx, &crx, None);
        drop(tx);

        // El destino quedó con SOLO a.txt (el extra.txt se borró con el Replace).
        assert!(dest_root.join("a.txt").exists(), "se copió el origen");
        assert_eq!(fs::read(dest_root.join("a.txt")).unwrap(), b"nuevo");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&dest_root).unwrap().permissions().mode() & 0o777,
                0o750,
                "el root conserva los permisos POSIX del origen"
            );
        }
        assert!(
            !dest_root.join("extra.txt").exists(),
            "el archivo extra del destino desapareció (Replace borró la carpeta)"
        );
        // El ORIGEN sigue intacto: NUNCA se borra el origen.
        assert!(src.join("a.txt").exists(), "el origen no se tocó");
        assert_eq!(fs::read(src.join("a.txt")).unwrap(), b"nuevo");
        assert!(summary.count_failed() == 0, "sin fallos");
        assert!(
            !tree_recovery_dir(&dest).exists(),
            "un Replace exitoso retira su registro de recuperación"
        );
    }

    #[test]
    fn replace_falla_al_preparar_y_conserva_destino_y_origen() {
        use crate::ops::{apply_folder_decision, folder_conflicts, FolderDecision};
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("folder");
        fs::create_dir(&src).unwrap();
        let source_file = src.join("new.txt");
        fs::write(&source_file, b"new").unwrap();
        let dest_parent = dir.path().join("dest");
        fs::create_dir(&dest_parent).unwrap();
        let dest_root = dest_parent.join("folder");
        fs::create_dir(&dest_root).unwrap();
        fs::write(dest_root.join("old.txt"), b"old tree").unwrap();

        let req = super::super::transfer(false, vec![src.clone()], dest_parent);
        let conflict = folder_conflicts(&req).pop().unwrap();
        let mut plan = plan(&req).unwrap();
        apply_folder_decision(&mut plan, &conflict.dest_root, FolderDecision::Replace);
        fs::remove_file(&source_file).unwrap(); // induce a failure after staging starts

        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, conflict_rx) = mpsc::channel::<ConflictDecision>();
        let summary = run_plan(
            &plan,
            &req.kind,
            req.conflict,
            &token,
            &tx,
            &conflict_rx,
            None,
        );

        assert!(summary.count_failed() > 0);
        assert_eq!(fs::read(dest_root.join("old.txt")).unwrap(), b"old tree");
        assert!(!dest_root.join("new.txt").exists());
        assert!(src.is_dir());
        assert!(!dir
            .path()
            .join("dest")
            .read_dir()
            .unwrap()
            .flatten()
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".naygo-replace-")));
    }

    #[test]
    fn move_replace_publica_el_arbol_y_retira_solo_los_origenes_sin_cambios() {
        use crate::ops::{apply_folder_decision, folder_conflicts, FolderDecision};
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("folder");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("nested/new.txt"), b"new payload").unwrap();
        let dest_parent = dir.path().join("dest");
        fs::create_dir(&dest_parent).unwrap();
        let dest_root = dest_parent.join("folder");
        fs::create_dir(&dest_root).unwrap();
        fs::write(dest_root.join("old.txt"), b"old tree").unwrap();

        let req = super::super::transfer(true, vec![src.clone()], dest_parent.clone());
        let conflict = folder_conflicts(&req).pop().unwrap();
        let mut plan = plan(&req).unwrap();
        apply_folder_decision(&mut plan, &conflict.dest_root, FolderDecision::Replace);
        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, conflict_rx) = mpsc::channel::<ConflictDecision>();
        let summary = run_plan(
            &plan,
            &req.kind,
            req.conflict,
            &token,
            &tx,
            &conflict_rx,
            None,
        );

        assert_eq!(summary.count_failed(), 0, "{summary:?}");
        assert_eq!(
            fs::read(dest_root.join("nested/new.txt")).unwrap(),
            b"new payload"
        );
        assert!(!dest_root.join("old.txt").exists());
        assert!(!src.exists(), "el origen se retira después de publicar");
        assert_eq!(
            summary
                .items
                .iter()
                .filter(|item| matches!(item.outcome, OpOutcome::Done))
                .count(),
            plan.steps.len()
        );
    }

    #[test]
    fn replace_si_falla_publicar_restaurar_destino_anterior() {
        use crate::ops::{
            apply_folder_decision, folder_conflicts, FolderDecision, NativeNoReplaceRename,
            NoReplaceRename,
        };
        struct FailStagingPublish(std::path::PathBuf);
        impl NoReplaceRename for FailStagingPublish {
            fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
                if to == self.0
                    && from
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with(".naygo-replace-"))
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "fallo de publicación simulado",
                    ));
                }
                NativeNoReplaceRename.rename_noreplace(from, to)
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("folder");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("new.txt"), b"new").unwrap();
        let dest_parent = dir.path().join("dest");
        fs::create_dir(&dest_parent).unwrap();
        let dest_root = dest_parent.join("folder");
        fs::create_dir(&dest_root).unwrap();
        fs::write(dest_root.join("old.txt"), b"old").unwrap();
        let req = super::super::transfer(false, vec![src.clone()], dest_parent.clone());
        let conflict = folder_conflicts(&req).pop().unwrap();
        let mut plan = plan(&req).unwrap();
        apply_folder_decision(&mut plan, &conflict.dest_root, FolderDecision::Replace);
        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, conflict_rx) = mpsc::channel::<ConflictDecision>();
        let summary = super::run_plan_with_no_replace_rename(
            &plan,
            &req.kind,
            req.conflict,
            &token,
            &tx,
            &conflict_rx,
            None,
            &FailStagingPublish(dest_root.clone()),
        );

        assert!(summary.count_failed() > 0);
        assert_eq!(fs::read(dest_root.join("old.txt")).unwrap(), b"old");
        assert!(!dest_root.join("new.txt").exists());
        assert_eq!(fs::read(src.join("new.txt")).unwrap(), b"new");
        assert!(!dest_parent.read_dir().unwrap().flatten().any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with(".naygo-backup-")));
    }

    #[test]
    fn replace_concurrente_en_publicacion_no_pisa_y_conserva_respaldo() {
        use crate::ops::{
            apply_folder_decision, folder_conflicts, FolderDecision, NativeNoReplaceRename,
            NoReplaceRename,
        };
        struct CreateDestinationAtPublish(std::path::PathBuf);
        impl NoReplaceRename for CreateDestinationAtPublish {
            fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
                if to == self.0
                    && from
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with(".naygo-replace-"))
                {
                    fs::create_dir(&self.0)?;
                    fs::write(self.0.join("racing.txt"), b"concurrent")?;
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "destino creado durante la publicación",
                    ));
                }
                NativeNoReplaceRename.rename_noreplace(from, to)
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("folder");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("new.txt"), b"new").unwrap();
        let dest_parent = dir.path().join("dest");
        fs::create_dir(&dest_parent).unwrap();
        let dest_root = dest_parent.join("folder");
        fs::create_dir(&dest_root).unwrap();
        fs::write(dest_root.join("old.txt"), b"old").unwrap();
        let req = super::super::transfer(false, vec![src.clone()], dest_parent.clone());
        let conflict = folder_conflicts(&req).pop().unwrap();
        let mut plan = plan(&req).unwrap();
        apply_folder_decision(&mut plan, &conflict.dest_root, FolderDecision::Replace);
        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, conflict_rx) = mpsc::channel::<ConflictDecision>();
        let summary = super::run_plan_with_no_replace_rename(
            &plan,
            &req.kind,
            req.conflict,
            &token,
            &tx,
            &conflict_rx,
            None,
            &CreateDestinationAtPublish(dest_root.clone()),
        );

        assert!(summary.count_failed() > 0);
        assert_eq!(
            fs::read(dest_root.join("racing.txt")).unwrap(),
            b"concurrent"
        );
        let backup = dest_parent
            .read_dir()
            .unwrap()
            .flatten()
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".naygo-backup-")
            })
            .expect("el árbol anterior queda en el respaldo si la ruta vuelve a ocuparse");
        assert_eq!(fs::read(backup.path().join("old.txt")).unwrap(), b"old");
        assert_eq!(fs::read(src.join("new.txt")).unwrap(), b"new");
    }

    fn write_interrupted_replace_record(
        parent: &Path,
        id: &str,
        destination_name: &str,
        staging_name: &str,
        backup_name: &str,
    ) -> PathBuf {
        let recovery_dir = tree_recovery_dir(parent);
        fs::create_dir_all(&recovery_dir).unwrap();
        fs::write(
            recovery_dir.join(TREE_RECOVERY_OWNER),
            TREE_RECOVERY_OWNER_CONTENT,
        )
        .unwrap();
        let record = TreeReplacementRecord {
            version: 1,
            id: id.to_owned(),
            destination_name: PathBuf::from(destination_name),
            staging_name: PathBuf::from(staging_name),
            backup_name: PathBuf::from(backup_name),
        };
        let path = tree_record_path(parent, id);
        fs::write(
            path.with_extension("tmp"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        fs::rename(path.with_extension("tmp"), &path).unwrap();
        path
    }

    fn add_interrupted_replace_marker(staging: &Path, id: &str) {
        fs::write(tree_marker_path(staging, id), id.as_bytes()).unwrap();
    }

    #[test]
    fn recupera_replace_interrumpido_entre_renames_y_restablece_el_destino_anterior() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("dest");
        let staging = parent.path().join(".naygo-replace-staged");
        let backup = parent.path().join(".naygo-backup-dest-11-2");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("old.txt"), b"old tree").unwrap();
        fs::create_dir(&staging).unwrap();
        fs::write(staging.join("new.txt"), b"new tree").unwrap();
        add_interrupted_replace_marker(&staging, "a1-b2-c3");
        fs::rename(&destination, &backup).unwrap();
        let journal = write_interrupted_replace_record(
            parent.path(),
            "a1-b2-c3",
            "dest",
            ".naygo-replace-staged",
            ".naygo-backup-dest-11-2",
        );

        recover_tree_replacements_in_parent(
            parent.path(),
            &CancellationToken::new(),
            &NativeNoReplaceRename,
        )
        .unwrap();

        assert_eq!(fs::read(destination.join("old.txt")).unwrap(), b"old tree");
        assert!(!destination.join("new.txt").exists());
        assert!(!staging.exists());
        assert!(!backup.exists());
        assert!(!journal.exists());
        assert!(!tree_recovery_dir(parent.path()).exists());
    }

    #[test]
    fn recupera_replace_interrumpido_durante_staging_y_conserva_el_destino_anterior() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("dest");
        let staging = parent.path().join(".naygo-replace-staged");
        let backup = parent.path().join(".naygo-backup-dest-11-2");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("old.txt"), b"old tree").unwrap();
        fs::create_dir(&staging).unwrap();
        fs::write(staging.join("partial.txt"), b"partial copy").unwrap();
        add_interrupted_replace_marker(&staging, "a1-b2-c3");
        let journal = write_interrupted_replace_record(
            parent.path(),
            "a1-b2-c3",
            "dest",
            ".naygo-replace-staged",
            ".naygo-backup-dest-11-2",
        );

        recover_tree_replacements_in_parent(
            parent.path(),
            &CancellationToken::new(),
            &NativeNoReplaceRename,
        )
        .unwrap();

        assert_eq!(fs::read(destination.join("old.txt")).unwrap(), b"old tree");
        assert!(!destination.join("partial.txt").exists());
        assert!(!staging.exists());
        assert!(!backup.exists());
        assert!(!journal.exists());
    }

    #[test]
    fn recupera_replace_publicado_y_limpia_el_respaldo_sin_borrar_el_arbol_nuevo() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("dest");
        let staging = parent.path().join(".naygo-replace-staged");
        let backup = parent.path().join(".naygo-backup-dest-11-2");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("old.txt"), b"old tree").unwrap();
        fs::create_dir(&staging).unwrap();
        fs::write(staging.join("new.txt"), b"new tree").unwrap();
        add_interrupted_replace_marker(&staging, "a1-b2-c3");
        fs::rename(&destination, &backup).unwrap();
        fs::rename(&staging, &destination).unwrap();
        let journal = write_interrupted_replace_record(
            parent.path(),
            "a1-b2-c3",
            "dest",
            ".naygo-replace-staged",
            ".naygo-backup-dest-11-2",
        );

        recover_tree_replacements_in_parent(
            parent.path(),
            &CancellationToken::new(),
            &NativeNoReplaceRename,
        )
        .unwrap();

        assert_eq!(fs::read(destination.join("new.txt")).unwrap(), b"new tree");
        assert!(!destination.join("old.txt").exists());
        assert!(!backup.exists());
        assert!(!journal.exists());
    }

    #[test]
    fn replace_reinicia_despues_de_terminar_el_proceso_entre_los_renames() {
        const CHILD_ROOT: &str = "NAYGO_REPLACE_RECOVERY_CHILD_ROOT";
        const CHILD_PHASE: &str = "NAYGO_REPLACE_RECOVERY_CHILD_PHASE";
        const TEST_NAME: &str =
            "ops::engine::tests::replace_reinicia_despues_de_terminar_el_proceso_entre_los_renames";

        if let Ok(root) = std::env::var(CHILD_ROOT) {
            struct ExitAtRename {
                target: PathBuf,
                phase: String,
            }
            impl NoReplaceRename for ExitAtRename {
                fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
                    NativeNoReplaceRename.rename_noreplace(from, to)?;
                    let stopped_between = self.phase == "between"
                        && from == self.target
                        && to.file_name().is_some_and(|name| {
                            name.to_string_lossy().starts_with(TREE_BACKUP_PREFIX)
                        });
                    let stopped_published = self.phase == "published"
                        && from.file_name().is_some_and(|name| {
                            name.to_string_lossy().starts_with(TREE_STAGE_PREFIX)
                        })
                        && to == self.target;
                    if stopped_between {
                        std::process::exit(86);
                    }
                    if stopped_published {
                        std::process::exit(87);
                    }
                    Ok(())
                }
            }

            let parent = PathBuf::from(root);
            let source = parent.join("source/folder");
            let destination_parent = parent.join("dest");
            let destination = destination_parent.join("folder");
            let request = super::super::transfer(false, vec![source], destination_parent);
            let conflict = crate::ops::folder_conflicts(&request).pop().unwrap();
            let mut plan = plan(&request).unwrap();
            crate::ops::apply_folder_decision(
                &mut plan,
                &conflict.dest_root,
                crate::ops::FolderDecision::Replace,
            );
            let (tx, _rx) = mpsc::channel::<OpMsg>();
            let (_ctx, conflict_rx) = mpsc::channel::<ConflictDecision>();
            let summary = run_plan_with_no_replace_rename(
                &plan,
                &request.kind,
                request.conflict,
                &CancellationToken::new(),
                &tx,
                &conflict_rx,
                None,
                &ExitAtRename {
                    target: destination,
                    phase: std::env::var(CHILD_PHASE).unwrap(),
                },
            );
            assert_eq!(
                summary.count_failed(),
                0,
                "child must exit at injected boundary"
            );
            return;
        }

        for (phase, exit_code) in [("between", 86), ("published", 87)] {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source/folder");
            let destination = root.path().join("dest/folder");
            fs::create_dir_all(&source).unwrap();
            fs::create_dir_all(&destination).unwrap();
            fs::write(source.join("new.txt"), b"new tree").unwrap();
            fs::write(destination.join("old.txt"), b"old tree").unwrap();

            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg(TEST_NAME)
                .arg("--nocapture")
                .env(CHILD_ROOT, root.path())
                .env(CHILD_PHASE, phase)
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(exit_code), "phase={phase}, {status:?}");

            let active = recover_tree_replacements_in_parent(
                root.path().join("dest").as_path(),
                &CancellationToken::new(),
                &NativeNoReplaceRename,
            )
            .unwrap();
            assert!(active.is_empty());
            if phase == "between" {
                assert_eq!(fs::read(destination.join("old.txt")).unwrap(), b"old tree");
                assert!(!destination.join("new.txt").exists());
            } else {
                assert_eq!(fs::read(destination.join("new.txt")).unwrap(), b"new tree");
                assert!(!destination.join("old.txt").exists());
            }
            assert!(!tree_recovery_dir(root.path().join("dest").as_path()).exists());
            assert_eq!(fs::read(source.join("new.txt")).unwrap(), b"new tree");
        }
    }

    #[test]
    fn recuperacion_no_pisa_una_ruta_concurrente_y_conserva_respaldo_y_staging() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("dest");
        let staging = parent.path().join(".naygo-replace-staged");
        let backup = parent.path().join(".naygo-backup-dest-11-2");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("old.txt"), b"old tree").unwrap();
        fs::create_dir(&staging).unwrap();
        fs::write(staging.join("new.txt"), b"new tree").unwrap();
        add_interrupted_replace_marker(&staging, "a1-b2-c3");
        fs::rename(&destination, &backup).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("external.txt"), b"external").unwrap();
        let journal = write_interrupted_replace_record(
            parent.path(),
            "a1-b2-c3",
            "dest",
            ".naygo-replace-staged",
            ".naygo-backup-dest-11-2",
        );

        let error = recover_tree_replacements_in_parent(
            parent.path(),
            &CancellationToken::new(),
            &NativeNoReplaceRename,
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            fs::read(destination.join("external.txt")).unwrap(),
            b"external"
        );
        assert_eq!(fs::read(backup.join("old.txt")).unwrap(), b"old tree");
        assert_eq!(fs::read(staging.join("new.txt")).unwrap(), b"new tree");
        assert!(journal.exists());
    }

    #[test]
    fn registro_de_recuperacion_con_ruta_ascendente_se_rechaza_sin_tocar_archivos() {
        let parent = tempfile::tempdir().unwrap();
        let outside = parent.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep.txt"), b"keep").unwrap();
        let destination = parent.path().join("dest");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("old.txt"), b"old tree").unwrap();
        write_interrupted_replace_record(
            parent.path(),
            "a1-b2-c3",
            "dest",
            "..\\outside",
            ".naygo-backup-dest-11-2",
        );

        let error = recover_tree_replacements_in_parent(
            parent.path(),
            &CancellationToken::new(),
            &NativeNoReplaceRename,
        )
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(fs::read(outside.join("keep.txt")).unwrap(), b"keep");
        assert_eq!(fs::read(destination.join("old.txt")).unwrap(), b"old tree");
    }

    #[test]
    fn carpeta_reservada_de_recuperacion_de_usuario_no_se_reutiliza_ni_se_borra() {
        use crate::ops::{apply_folder_decision, folder_conflicts, FolderDecision};
        let parent = tempfile::tempdir().unwrap();
        let source = parent.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("new.txt"), b"new").unwrap();
        let destination_parent = parent.path().join("dest");
        fs::create_dir(&destination_parent).unwrap();
        let destination = destination_parent.join("source");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("old.txt"), b"old").unwrap();
        let user_recovery_dir = tree_recovery_dir(&destination_parent);
        fs::create_dir(&user_recovery_dir).unwrap();
        fs::write(user_recovery_dir.join("user-data.txt"), b"keep").unwrap();
        let request =
            super::super::transfer(false, vec![source.clone()], destination_parent.clone());
        let conflict = folder_conflicts(&request).pop().unwrap();
        let mut plan = plan(&request).unwrap();
        apply_folder_decision(&mut plan, &conflict.dest_root, FolderDecision::Replace);
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, conflict_rx) = mpsc::channel::<ConflictDecision>();

        let summary = run_plan_with_no_replace_rename(
            &plan,
            &request.kind,
            request.conflict,
            &CancellationToken::new(),
            &tx,
            &conflict_rx,
            None,
            &NativeNoReplaceRename,
        );

        assert!(summary.count_failed() > 0);
        assert_eq!(fs::read(destination.join("old.txt")).unwrap(), b"old");
        assert_eq!(fs::read(source.join("new.txt")).unwrap(), b"new");
        assert_eq!(
            fs::read(user_recovery_dir.join("user-data.txt")).unwrap(),
            b"keep"
        );
        assert!(!owns_tree_recovery_dir(&user_recovery_dir));
    }

    #[test]
    fn listado_no_interfiere_con_un_replace_activo_en_el_mismo_proceso() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("dest");
        let staging = parent.path().join(".naygo-replace-staged");
        let backup = parent.path().join(".naygo-backup-dest-11-2");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("old.txt"), b"old tree").unwrap();
        fs::create_dir(&staging).unwrap();
        add_interrupted_replace_marker(&staging, "a1-b2-c3");
        fs::rename(&destination, &backup).unwrap();
        let _active = ActiveReplacementGuard::new("a1-b2-c3");
        write_interrupted_replace_record(
            parent.path(),
            "a1-b2-c3",
            "dest",
            ".naygo-replace-staged",
            ".naygo-backup-dest-11-2",
        );

        recover_tree_replacements_in_parent(
            parent.path(),
            &CancellationToken::new(),
            &NativeNoReplaceRename,
        )
        .unwrap();

        assert!(!destination.exists());
        assert_eq!(fs::read(backup.join("old.txt")).unwrap(), b"old tree");
        assert!(staging.exists());
    }

    #[test]
    fn replace_cancelado_antes_de_empezar_conserva_ambos_arboles() {
        use crate::ops::{apply_folder_decision, folder_conflicts, FolderDecision};
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("folder");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("new.txt"), b"new").unwrap();
        let dest_parent = dir.path().join("dest");
        fs::create_dir(&dest_parent).unwrap();
        let dest_root = dest_parent.join("folder");
        fs::create_dir(&dest_root).unwrap();
        fs::write(dest_root.join("old.txt"), b"old").unwrap();
        let req = super::super::transfer(false, vec![src.clone()], dest_parent.clone());
        let conflict = folder_conflicts(&req).pop().unwrap();
        let mut plan = plan(&req).unwrap();
        apply_folder_decision(&mut plan, &conflict.dest_root, FolderDecision::Replace);
        let token = CancellationToken::new();
        token.cancel();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, conflict_rx) = mpsc::channel::<ConflictDecision>();
        let _ = run_plan(
            &plan,
            &req.kind,
            req.conflict,
            &token,
            &tx,
            &conflict_rx,
            None,
        );

        assert_eq!(fs::read(dest_root.join("old.txt")).unwrap(), b"old");
        assert_eq!(fs::read(src.join("new.txt")).unwrap(), b"new");
        assert!(!dest_parent.read_dir().unwrap().flatten().any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with(".naygo-replace-")));
    }

    #[test]
    fn pre_delete_jamas_borra_un_origen_del_plan() {
        // RED DE SEGURIDAD CRÍTICA (cinturón-y-tirantes del motor): si por CUALQUIER camino el
        // `pre_delete` viene contaminado con una ruta que ES un `step.from` (o un ancestro de uno),
        // el motor JAMÁS debe ejecutar `remove_dir_all` sobre ella: borraría el ORIGEN de la copia
        // (pérdida total de datos). Simulamos ese plan envenenado a mano y verificamos que tras
        // `run_plan` el origen sigue intacto con su contenido.
        let dir = tempfile::tempdir().unwrap();
        // Origen: carpeta/ con a.txt.
        let origen = dir.path().join("carpeta");
        fs::create_dir(&origen).unwrap();
        fs::write(origen.join("a.txt"), b"datos-importantes").unwrap();
        // Destino donde se copiaría el árbol del origen.
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let dest_root = dest.join("carpeta");

        // Plan ENVENENADO: copia desde DENTRO del origen, pero pre_delete apunta al origen MISMO
        // (lo que ocurriría si folder_conflicts no descartara dest_root==src y se eligiera Replace).
        let plan = OpPlan {
            steps: vec![
                OpStep {
                    from: None,
                    to: dest_root.clone(),
                    bytes: 0,
                    is_dir: true,
                },
                OpStep {
                    from: Some(origen.join("a.txt")),
                    to: dest_root.join("a.txt"),
                    bytes: 17,
                    is_dir: false,
                },
            ],
            total_bytes: 17,
            total_files: 1,
            // El origen entero, marcado (erróneamente) para borrar antes de copiar.
            pre_delete: vec![origen.clone()],
        };

        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let _summary = run_plan(
            &plan,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &token,
            &tx,
            &crx,
            None,
        );
        drop(tx);

        // LO CRUCIAL: el origen NUNCA se borró. Sigue existiendo con su contenido.
        assert!(
            origen.exists(),
            "el motor borró el ORIGEN (pre_delete contaminado): pérdida total de datos"
        );
        assert!(
            origen.join("a.txt").exists(),
            "el contenido del origen sigue intacto"
        );
        assert_eq!(
            fs::read(origen.join("a.txt")).unwrap(),
            b"datos-importantes"
        );
    }

    #[test]
    fn pre_delete_no_borra_un_ancestro_de_un_origen() {
        // Variante: pre_delete apunta a un ANCESTRO de un `step.from` (no al from exacto). El motor
        // debe igual abstenerse: borrar el ancestro se llevaría el origen por delante.
        let dir = tempfile::tempdir().unwrap();
        // Ancestro/ contiene origen/ que contiene a.txt; el step copia desde origen/a.txt.
        let ancestro = dir.path().join("ancestro");
        let origen = ancestro.join("origen");
        fs::create_dir_all(&origen).unwrap();
        fs::write(origen.join("a.txt"), b"x").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();

        let plan = OpPlan {
            steps: vec![OpStep {
                from: Some(origen.join("a.txt")),
                to: dest.join("a.txt"),
                bytes: 1,
                is_dir: false,
            }],
            total_bytes: 1,
            total_files: 1,
            pre_delete: vec![ancestro.clone()],
        };

        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let _ = run_plan(
            &plan,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &token,
            &tx,
            &crx,
            None,
        );
        drop(tx);

        assert!(
            origen.join("a.txt").exists(),
            "el motor borró un ancestro del origen: pérdida de datos"
        );
    }

    #[test]
    fn pre_delete_no_borra_un_origen_que_solo_difiere_en_capitalizacion() {
        // BUG DE PÉRDIDA DE DATOS (case-sensitivity): el `pre_delete` apunta al origen pero escrito
        // con OTRA capitalización (`CARPETA` vs el real `carpeta`). Con `==`/`starts_with` crudos,
        // la red de seguridad NO reconocía que es el mismo directorio y `remove_dir_all` borraba el
        // ORIGEN. En Windows ambas grafías son la MISMA carpeta: la guarda case-insensitive DEBE
        // abstenerse de borrar.
        let dir = tempfile::tempdir().unwrap();
        // El directorio real en disco se llama "carpeta".
        let origen = dir.path().join("carpeta");
        fs::create_dir(&origen).unwrap();
        fs::write(origen.join("a.txt"), b"datos-importantes").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let dest_root = dest.join("carpeta");
        // `pre_delete` apunta al origen pero con la grafía "CARPETA" (mismo dir en Windows).
        let origen_otra_grafia = dir.path().join("CARPETA");

        let plan = OpPlan {
            steps: vec![
                OpStep {
                    from: None,
                    to: dest_root.clone(),
                    bytes: 0,
                    is_dir: true,
                },
                OpStep {
                    from: Some(origen.join("a.txt")),
                    to: dest_root.join("a.txt"),
                    bytes: 17,
                    is_dir: false,
                },
            ],
            total_bytes: 17,
            total_files: 1,
            pre_delete: vec![origen_otra_grafia],
        };

        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let _ = run_plan(
            &plan,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &token,
            &tx,
            &crx,
            None,
        );
        drop(tx);

        // LO CRUCIAL: el origen NO se borró pese a la grafía distinta en pre_delete.
        assert!(
            origen.join("a.txt").exists(),
            "el motor borró el origen por diferencia de capitalización: pérdida total de datos"
        );
        assert_eq!(
            fs::read(origen.join("a.txt")).unwrap(),
            b"datos-importantes"
        );
    }

    #[test]
    fn pre_delete_de_destino_ausente_no_falla() {
        // Si la carpeta a borrar ya no existe (objetivo cumplido), no es un error: la op corre OK.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let mut p = plan(&super::super::transfer(false, vec![src], dest.clone())).unwrap();
        // pre_delete apunta a una carpeta inexistente.
        p.pre_delete.push(dest.join("no_existe"));
        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let summary = run_plan(
            &p,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &token,
            &tx,
            &crx,
            None,
        );
        drop(tx);
        assert_eq!(summary.count_failed(), 0, "borrar lo ausente no falla");
        assert!(dest.join("a.txt").exists(), "la copia siguió normal");
    }

    #[test]
    fn pre_delete_que_falla_no_paniquea_y_la_op_sigue() {
        // El borrado del destino falla (lo simulamos apuntando a un ARCHIVO, no a una carpeta:
        // remove_dir_all sobre un archivo da Err). La op NO debe paniquear: registra un Failed y
        // continúa copiando los pasos.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"hola").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        // Un archivo (no carpeta) que será el blanco "no borrable como dir".
        let archivo_no_dir = dest.join("soy_un_archivo");
        fs::write(&archivo_no_dir, b"no soy carpeta").unwrap();
        let mut p = plan(&super::super::transfer(false, vec![src], dest.clone())).unwrap();
        p.pre_delete.push(archivo_no_dir.clone());
        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let _summary = run_plan(
            &p,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &token,
            &tx,
            &crx,
            None,
        );
        drop(tx);
        // En Windows remove_dir_all sobre un archivo falla → un Failed registrado, sin pánico.
        // (Defensivo: en plataformas donde no falle, simplemente habrá 0 fallos; lo crucial es que
        // la copia siguió y no hubo pánico.)
        assert!(
            dest.join("a.txt").exists(),
            "la copia siguió pese al pre_delete"
        );
    }

    #[test]
    fn pre_delete_no_corre_si_ya_se_cancelo() {
        // Si el token ya está cancelado, el pre_delete NO borra el destino (cancelación universal).
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let dest_root = dest.join("carpeta");
        fs::create_dir(&dest_root).unwrap();
        fs::write(dest_root.join("keep.txt"), b"conservar").unwrap();
        let mut p = plan(&super::super::transfer(false, vec![src], dest.clone())).unwrap();
        p.pre_delete.push(dest_root.clone());
        let token = CancellationToken::new();
        token.cancel(); // cancelado ANTES de correr
        let (tx, _rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let _summary = run_plan(
            &p,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &token,
            &tx,
            &crx,
            None,
        );
        drop(tx);
        assert!(
            dest_root.join("keep.txt").exists(),
            "cancelado antes → el pre_delete no borró el destino"
        );
    }

    #[test]
    fn move_borra_el_origen() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = OpRequest {
            kind: OpKind::Move,
            sources: vec![src.clone()],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Overwrite,
        };
        let (_m, _s) = run(req);
        assert!(!src.exists());
        assert!(dest.join("a.txt").exists());
    }

    #[test]
    fn move_destino_ocupado_atomicamente_por_platform_conserva_ambos() {
        struct DestinationAppearsDuringRename;
        impl NoReplaceRename for DestinationAppearsDuringRename {
            fn rename_noreplace(&self, _from: &Path, to: &Path) -> std::io::Result<()> {
                fs::write(to, b"external").unwrap();
                Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "destino ocupado durante el rename atómico",
                ))
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let dest = dir.path().join("dest");
        fs::write(&source, b"source").unwrap();
        fs::create_dir(&dest).unwrap();
        let request = super::super::transfer(true, vec![source.clone()], dest.clone());
        let plan = plan(&request).unwrap();
        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel();
        let (_conflict_tx, conflict_rx) = mpsc::channel();

        let summary = run_plan_with_no_replace_rename(
            &plan,
            &request.kind,
            request.conflict,
            &token,
            &tx,
            &conflict_rx,
            None,
            &DestinationAppearsDuringRename,
        );

        assert_eq!(summary.count_failed(), 1);
        assert_eq!(fs::read(&source).unwrap(), b"source");
        assert_eq!(fs::read(dest.join("source.txt")).unwrap(), b"external");
    }

    #[test]
    fn move_platform_informa_otro_volumen_y_motor_usa_fallback_cancelable() {
        struct CrossVolumeRename;
        impl NoReplaceRename for CrossVolumeRename {
            fn rename_noreplace(&self, _from: &Path, _to: &Path) -> std::io::Result<()> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::CrossesDevices,
                    "volúmenes distintos",
                ))
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let dest = dir.path().join("dest");
        fs::write(&source, b"payload").unwrap();
        fs::create_dir(&dest).unwrap();
        let request = super::super::transfer(true, vec![source.clone()], dest.clone());
        let plan = plan(&request).unwrap();
        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel();
        let (_conflict_tx, conflict_rx) = mpsc::channel();

        let summary = run_plan_with_no_replace_rename(
            &plan,
            &request.kind,
            request.conflict,
            &token,
            &tx,
            &conflict_rx,
            None,
            &CrossVolumeRename,
        );

        assert_eq!(summary.count_done(), 1);
        assert!(
            !source.exists(),
            "se retira el origen tras publicar el fallback"
        );
        assert_eq!(fs::read(dest.join("source.txt")).unwrap(), b"payload");
    }

    #[test]
    fn move_carpeta_retirada_directorios_vacios_en_postorden() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("origen");
        let nested = src.join("sub").join("vacio");
        fs::create_dir_all(&nested).unwrap();
        fs::write(src.join("raiz.txt"), b"raiz").unwrap();
        fs::write(src.join("sub").join("hijo.txt"), b"hijo").unwrap();
        let dest = dir.path().join("destino");
        fs::create_dir(&dest).unwrap();

        let req = super::super::transfer(true, vec![src.clone()], dest.clone());
        let plan = plan(&req).unwrap();
        let directory_sources: Vec<_> = plan
            .steps
            .iter()
            .filter(|step| step.is_dir)
            .filter_map(|step| step.from.clone())
            .collect();
        assert_eq!(
            directory_sources,
            vec![nested.clone(), src.join("sub"), src.clone()]
        );

        let (_messages, summary) = run(req);
        let moved = dest.join("origen");
        assert_eq!(fs::read(moved.join("raiz.txt")).unwrap(), b"raiz");
        assert_eq!(
            fs::read(moved.join("sub").join("hijo.txt")).unwrap(),
            b"hijo"
        );
        assert!(moved.join("sub").join("vacio").is_dir());
        assert!(!src.exists(), "las carpetas de origen vacías se retiran");
        assert_eq!(summary.count_failed(), 0);
    }

    #[test]
    fn move_skip_archivo_conserva_su_directorio_de_origen() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("origen");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("sub").join("igual.txt"), b"origen").unwrap();
        let dest = dir.path().join("destino");
        fs::create_dir_all(dest.join("origen").join("sub")).unwrap();
        fs::write(
            dest.join("origen").join("sub").join("igual.txt"),
            b"destino",
        )
        .unwrap();
        let req = OpRequest {
            kind: OpKind::Move,
            sources: vec![src.clone()],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Skip,
        };

        let (_messages, summary) = run(req);

        assert_eq!(summary.count_skipped(), 1);
        assert_eq!(
            fs::read(src.join("sub").join("igual.txt")).unwrap(),
            b"origen"
        );
        assert!(src.join("sub").is_dir());
        assert_eq!(
            fs::read(dest.join("origen").join("sub").join("igual.txt")).unwrap(),
            b"destino"
        );
    }

    #[test]
    fn cancelar_antes_de_empezar_no_copia() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let p = plan(&OpRequest {
            kind: OpKind::Copy,
            sources: vec![src],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Overwrite,
        })
        .unwrap();
        let token = CancellationToken::new();
        token.cancel();
        let (tx, _rx) = mpsc::channel();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let _summary = run_plan(
            &p,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &token,
            &tx,
            &crx,
            None,
        );
        assert!(!dest.join("a.txt").exists());
    }

    #[test]
    fn run_plan_con_journal_actualiza_done_through() {
        use crate::ops::journal::{journal_path, JournalWriter, OpJournal};
        let dir = tempfile::tempdir().unwrap();
        for (n, c) in [("a", "aa"), ("b", "bbb")] {
            std::fs::write(dir.path().join(n), c.as_bytes()).unwrap();
        }
        let dest = dir.path().join("dst");
        std::fs::create_dir(&dest).unwrap();
        let req = OpRequest {
            kind: OpKind::Copy,
            sources: vec![dir.path().join("a"), dir.path().join("b")],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Overwrite,
        };
        let p = plan(&req).unwrap();
        let cfg = dir.path();
        let journal = OpJournal::new("eng1".into(), req.kind.clone(), req.conflict, p.clone());
        let mut writer = JournalWriter::new(cfg, journal);
        let token = CancellationToken::new();
        let (tx, _rx) = std::sync::mpsc::channel();
        let (_ctx, crx) = std::sync::mpsc::channel();
        let _summary = run_plan(
            &p,
            &req.kind,
            req.conflict,
            &token,
            &tx,
            &crx,
            Some(&mut writer),
        );
        writer.flush();
        let back: OpJournal =
            serde_json::from_str(&std::fs::read_to_string(journal_path(cfg, "eng1")).unwrap())
                .unwrap();
        assert_eq!(back.done_through, 2);
        assert!(dest.join("a").exists() && dest.join("b").exists());
    }

    #[test]
    fn run_plan_con_journal_no_avanza_sobre_paso_fallido() {
        // Plan de 3 pasos de copia donde el 2º (índice 1) falla porque el origen no
        // existe. El cursor del journal debe quedarse en 1 (solo el 1er paso resuelto),
        // de modo que al retomar se re-intente el paso fallido y los siguientes.
        use crate::ops::journal::{journal_path, JournalWriter, OpJournal};
        use crate::ops::{OpPlan, OpStep};
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), b"aa").unwrap();
        std::fs::write(dir.path().join("c"), b"cccc").unwrap();
        // "b" NO se crea → su paso de copia fallará.
        let dest = dir.path().join("dst");
        std::fs::create_dir(&dest).unwrap();
        let mk = |n: &str, b: u64| OpStep {
            from: Some(dir.path().join(n)),
            to: dest.join(n),
            bytes: b,
            is_dir: false,
        };
        let p = OpPlan {
            steps: vec![mk("a", 2), mk("b", 3), mk("c", 4)],
            total_bytes: 9,
            total_files: 3,
            pre_delete: Vec::new(),
        };
        let cfg = dir.path();
        let journal = OpJournal::new(
            "engbar".into(),
            OpKind::Copy,
            ConflictPolicy::Overwrite,
            p.clone(),
        );
        let mut writer = JournalWriter::new(cfg, journal);
        let token = CancellationToken::new();
        let (tx, _rx) = std::sync::mpsc::channel();
        let (_ctx, crx) = std::sync::mpsc::channel();
        let summary = run_plan(
            &p,
            &OpKind::Copy,
            ConflictPolicy::Overwrite,
            &token,
            &tx,
            &crx,
            Some(&mut writer),
        );
        writer.flush();
        let back: OpJournal =
            serde_json::from_str(&std::fs::read_to_string(journal_path(cfg, "engbar")).unwrap())
                .unwrap();
        // Barrera en el paso 1 (índice 1, falló): el cursor no pasa de 1, aunque el
        // paso 2 (índice 2, "c") sí se copia con éxito.
        assert_eq!(back.done_through, 1);
        assert_eq!(summary.count_failed(), 1);
    }

    #[test]
    fn delete_permanente_borra() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"x").unwrap();
        let req = OpRequest {
            kind: OpKind::Delete { to_trash: false },
            sources: vec![src.clone()],
            dest_dir: None,
            conflict: ConflictPolicy::Overwrite,
        };
        let (_m, summary) = run(req);
        assert!(!src.exists());
        assert_eq!(summary.count_done(), 1);
    }

    #[test]
    fn remove_dir_all_cancelable_completo_borra_el_arbol() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("arbol");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("a.txt"), b"x").unwrap();
        fs::write(root.join("sub").join("b.txt"), b"y").unwrap();
        let token = CancellationToken::new();
        let done = remove_dir_all_cancelable(&root, &token).unwrap();
        assert!(done, "sin cancelar borra todo");
        assert!(!root.exists(), "el árbol completo desapareció");
    }

    #[test]
    fn remove_dir_all_cancelable_con_token_cancelado_no_toca_nada() {
        // Token cancelado ANTES de empezar: el chequeo por entrada aborta limpio en
        // la primera entrada y el árbol queda intacto.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("arbol");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("a.txt"), b"x").unwrap();
        fs::write(root.join("sub").join("b.txt"), b"y").unwrap();
        let token = CancellationToken::new();
        token.cancel();
        let done = remove_dir_all_cancelable(&root, &token).unwrap();
        assert!(!done, "cancelado → no completó");
        assert!(root.join("sub").join("b.txt").exists(), "no borró nada");
    }

    #[test]
    fn remove_dir_all_cancelable_cancelado_a_mitad_aborta() {
        // Árbol grande + cancelación desde otro hilo a ~1ms: puede alcanzar a
        // terminar (disco rápido) o abortar a mitad. Si aborta, la raíz NO puede
        // haber desaparecido (post-orden: la raíz se borra ÚLTIMA, solo si terminó).
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("arbol");
        for i in 0..50 {
            let sub = root.join(format!("sub{i}"));
            fs::create_dir_all(&sub).unwrap();
            for j in 0..100 {
                fs::write(sub.join(format!("f{j}.txt")), b"x").unwrap();
            }
        }
        let token = CancellationToken::new();
        let t2 = token.clone();
        let h = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1));
            t2.cancel();
        });
        let done = remove_dir_all_cancelable(&root, &token).unwrap();
        h.join().unwrap();
        if !done {
            assert!(root.exists(), "abortó a mitad: queda lo no alcanzado");
        }
    }

    #[test]
    fn remove_dir_all_cancelable_raiz_archivo_es_error() {
        // Contrato de remove_dir_all: borrar un ARCHIVO por esta vía es error (no se
        // borra un archivo suelto disfrazado de carpeta).
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        fs::write(&f, b"x").unwrap();
        let token = CancellationToken::new();
        assert!(remove_dir_all_cancelable(&f, &token).is_err());
        assert!(f.exists(), "el archivo no se tocó");
    }

    #[test]
    fn exec_delete_carpeta_cancelada_aborta_limpio() {
        // exec_delete con token cancelado: el borrado recursivo aborta limpio, el paso
        // queda Skipped (no Done) y la carpeta queda intacta en disco.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("carpeta");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("a.txt"), b"datos").unwrap();
        let step = OpStep {
            from: Some(target.clone()),
            to: target.clone(),
            bytes: 0,
            is_dir: true,
        };
        let token = CancellationToken::new();
        token.cancel();
        let outcome = exec_delete(&step, &token);
        assert!(
            matches!(outcome, OpOutcome::Skipped),
            "cancelado → Skipped, no Done ni Failed"
        );
        assert!(target.join("a.txt").exists(), "no borró nada");
    }

    // --- ops-B: conflicto interactivo por-ítem ---

    use super::super::ConflictAction;

    /// Corre un plan con política `Ask`, respondiendo cada `OpMsg::Conflict` con la
    /// `ConflictDecision` dada (en un hilo aparte). Devuelve el summary y cuántos
    /// `Conflict` se emitieron. Si nunca llega un `Conflict` y sí un `Done`, se considera
    /// error de la lógica (el motor no preguntó cuando debía).
    fn run_ask(req: OpRequest, decision: ConflictDecision) -> (OpSummary, usize) {
        let p = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<OpMsg>();
        let (ctx, crx) = mpsc::channel::<ConflictDecision>();
        let conflicts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let conflicts2 = conflicts.clone();
        let resp = std::thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                if let OpMsg::Conflict(_) = msg {
                    conflicts2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    // `ConflictDecision` ya no es `Copy` (puede llevar un nombre en `RenameTo`);
                    // se clona para responder a cada conflicto.
                    let _ = ctx.send(decision.clone());
                }
            }
        });
        let summary = run_plan(&p, &req.kind, ConflictPolicy::Ask, &token, &tx, &crx, None);
        drop(tx);
        let _ = resp.join();
        (summary, conflicts.load(std::sync::atomic::Ordering::SeqCst))
    }

    #[test]
    fn ask_emite_conflict_y_aplica_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"NUEVO").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
        let req = super::super::transfer(false, vec![src], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::Overwrite,
                apply_all: false,
            },
        );
        assert_eq!(conflicts, 1, "debió emitir exactamente un Conflict");
        assert_eq!(summary.count_done(), 1);
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "NUEVO");
    }

    #[test]
    fn cancelar_overwrite_a_mitad_conserva_el_destino_anterior() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.bin");
        let destination = dir.path().join("destination.bin");
        fs::write(&source, vec![0x5A; BUF_SIZE * 2]).unwrap();
        fs::write(&destination, b"contenido anterior").unwrap();

        let token = CancellationToken::new();
        let mut chunks = 0;
        let copied = copy_buffered(
            &source,
            &destination,
            true,
            &token,
            |_| {},
            || {
                chunks += 1;
                if chunks == 1 {
                    token.cancel();
                }
            },
        )
        .unwrap();

        assert!(!copied, "la cancelación tras el primer bloque debe abortar");
        assert_eq!(fs::read(&destination).unwrap(), b"contenido anterior");
        assert!(source.exists(), "la fuente de copia se conserva");
        assert!(
            fs::read_dir(dir.path()).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".naygo-copy-")),
            "el temporal parcial se limpia"
        );
    }

    #[test]
    fn copy_noclobber_preserva_un_destino_que_aparece_durante_la_copia() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.bin");
        let destination = dir.path().join("destination.bin");
        fs::write(&source, vec![0x6B; BUF_SIZE * 2]).unwrap();

        let token = CancellationToken::new();
        let mut destination_created = false;
        let result = copy_buffered(
            &source,
            &destination,
            false,
            &token,
            |_| {},
            || {
                if !destination_created {
                    fs::write(&destination, b"created by another process").unwrap();
                    destination_created = true;
                }
            },
        );

        assert!(
            matches!(result, Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists),
            "la publicación debe fallar sin reemplazar"
        );
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"created by another process"
        );
        assert!(
            fs::read_dir(dir.path()).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".naygo-copy-")),
            "el temporal se limpia al fallar la publicación"
        );
    }

    #[test]
    fn move_rename_noreplace_no_pisa_un_destino_existente() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("destination.txt");
        fs::write(&source, b"source").unwrap();
        fs::write(&destination, b"destination").unwrap();

        let result = rename_noreplace(&source, &destination);

        assert!(
            matches!(result, Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists),
            "el movimiento no debe reemplazar un destino existente"
        );
        assert_eq!(fs::read(&source).unwrap(), b"source");
        assert_eq!(fs::read(&destination).unwrap(), b"destination");
    }

    #[test]
    fn rename_no_pisa_destino_que_aparece_despues_de_planificar() {
        struct DestinationAppears;
        impl NoReplaceRename for DestinationAppears {
            fn rename_noreplace(&self, from: &Path, to: &Path) -> std::io::Result<()> {
                fs::write(to, b"created by another process")?;
                NativeNoReplaceRename.rename_noreplace(from, to)
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("destination.txt");
        fs::write(&source, b"source").unwrap();
        let req = OpRequest {
            kind: OpKind::Rename {
                new_name: "destination.txt".into(),
            },
            sources: vec![source.clone()],
            dest_dir: None,
            conflict: ConflictPolicy::Overwrite,
        };
        let plan = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<OpMsg>();
        let (_ctx, conflict_rx) = mpsc::channel::<ConflictDecision>();

        let summary = run_plan_with_no_replace_rename(
            &plan,
            &req.kind,
            req.conflict,
            &token,
            &tx,
            &conflict_rx,
            None,
            &DestinationAppears,
        );
        drop(tx);
        let _ = rx.into_iter().collect::<Vec<_>>();

        assert_eq!(summary.count_failed(), 1);
        assert_eq!(fs::read(&source).unwrap(), b"source");
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"created by another process"
        );
    }

    #[cfg(windows)]
    #[test]
    fn rename_solo_case_actualiza_la_grafia_en_windows() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("naygo-case-source.txt");
        fs::write(&source, b"source").unwrap();
        let req = OpRequest {
            kind: OpKind::Rename {
                new_name: "NAYGO-CASE-SOURCE.TXT".into(),
            },
            sources: vec![source.clone()],
            dest_dir: None,
            conflict: ConflictPolicy::Overwrite,
        };

        let (_messages, summary) = run(req);

        assert_eq!(summary.count_done(), 1);
        let actual_name = fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();
        assert_eq!(actual_name, "NAYGO-CASE-SOURCE.TXT");
        assert_eq!(
            fs::read(dir.path().join("NAYGO-CASE-SOURCE.TXT")).unwrap(),
            b"source"
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_preserva_el_modo_ejecutable_posix() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("script.sh");
        let destination = dir.path().join("copied.sh");
        fs::write(&source, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o751)).unwrap();

        let copied = copy_buffered(
            &source,
            &destination,
            false,
            &CancellationToken::new(),
            |_| {},
            || {},
        )
        .unwrap();

        assert!(copied);
        assert_eq!(
            fs::metadata(destination).unwrap().permissions().mode() & 0o777,
            0o751
        );
    }

    // Confirma la causa raíz del bug "pegar+sobrescribir copió solo unos MB de un archivo
    // grande": el camino Ask+Overwrite NO trunca. Copia un archivo MULTI-CHUNK (mayor que
    // BUF_SIZE) y verifica que el destino queda con TODOS los bytes. Si esto pasa, el motor
    // copia completo y el síntoma de "se detuvo en N MB" es de la UI (sin progreso visible
    // la copia en curso parece detenida) o de un cierre prematuro, no del motor.
    #[test]
    fn ask_overwrite_copia_archivo_grande_completo() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("big.bin");
        // 1 MiB = 4 chunks de 256 KiB + cambia el patrón por byte para detectar truncamiento.
        let size = 1024 * 1024;
        let payload: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        fs::write(&src, &payload).unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        // El destino YA existe (con otro contenido más corto) → fuerza el camino de conflicto.
        fs::write(dst.join("big.bin"), b"viejo-corto").unwrap();
        let req = super::super::transfer(false, vec![src], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::Overwrite,
                apply_all: false,
            },
        );
        assert_eq!(conflicts, 1, "debió preguntar una vez");
        assert_eq!(summary.count_done(), 1, "debió completar la copia");
        let copied = fs::read(dst.join("big.bin")).unwrap();
        assert_eq!(copied.len(), size, "copió {} bytes de {size}", copied.len());
        assert_eq!(
            copied, payload,
            "el contenido copiado no coincide byte a byte"
        );
    }

    #[test]
    fn ask_skip_deja_el_existente() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"NUEVO").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
        let req = super::super::transfer(false, vec![src], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::Skip,
                apply_all: false,
            },
        );
        assert_eq!(conflicts, 1);
        assert_eq!(summary.count_skipped(), 1);
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "VIEJO");
    }

    #[test]
    fn ask_rename_crea_copia() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"NUEVO").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
        let req = super::super::transfer(false, vec![src], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::Rename,
                apply_all: false,
            },
        );
        assert_eq!(conflicts, 1);
        assert_eq!(summary.count_done(), 1);
        // El original intacto; la copia con sufijo " (2)".
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "VIEJO");
        assert_eq!(fs::read_to_string(dst.join("a (2).txt")).unwrap(), "NUEVO");
    }

    #[test]
    fn ask_rename_to_usa_el_nombre_elegido() {
        // BUG 1: "Renombrar" con nombre custom escribe en el nombre ELEGIDO (no en un sufijo
        // automático). El original queda intacto; el nuevo aparece con el nombre del usuario.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"NUEVO").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
        let req = super::super::transfer(false, vec![src], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::RenameTo("elegido.txt".to_string()),
                apply_all: false,
            },
        );
        assert_eq!(conflicts, 1);
        assert_eq!(summary.count_done(), 1);
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "VIEJO");
        assert_eq!(
            fs::read_to_string(dst.join("elegido.txt")).unwrap(),
            "NUEVO"
        );
        // No debe haber creado el sufijo automático.
        assert!(!dst.join("a (2).txt").exists());
    }

    #[test]
    fn ask_rename_to_choca_aplica_dedup_como_salvaguarda() {
        // Si el nombre elegido TAMBIÉN existe, el motor NO pisa: desambigua con " (N)".
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"NUEVO").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
        // El nombre elegido ya está ocupado en el destino.
        fs::write(dst.join("ocupado.txt"), b"YA-ESTABA").unwrap();
        let req = super::super::transfer(false, vec![src], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::RenameTo("ocupado.txt".to_string()),
                apply_all: false,
            },
        );
        assert_eq!(conflicts, 1);
        assert_eq!(summary.count_done(), 1);
        // El ocupado intacto; el nuevo desambiguado a "ocupado (2).txt".
        assert_eq!(
            fs::read_to_string(dst.join("ocupado.txt")).unwrap(),
            "YA-ESTABA"
        );
        assert_eq!(
            fs::read_to_string(dst.join("ocupado (2).txt")).unwrap(),
            "NUEVO"
        );
    }

    #[test]
    fn ask_rename_to_respeta_el_move() {
        // RenameTo en un MOVER: el origen se borra (es mover) y el destino lleva el nombre nuevo.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"NUEVO").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
        // is_move = true.
        let req = super::super::transfer(true, vec![src.clone()], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::RenameTo("movido.txt".to_string()),
                apply_all: false,
            },
        );
        assert_eq!(conflicts, 1);
        assert_eq!(summary.count_done(), 1);
        assert_eq!(fs::read_to_string(dst.join("movido.txt")).unwrap(), "NUEVO");
        // El existente quedó intacto y el ORIGEN se borró (era mover).
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "VIEJO");
        assert!(!src.exists(), "el origen debe borrarse en un mover");
    }

    #[test]
    fn apply_all_no_vuelve_a_preguntar() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, b"NA").unwrap();
        fs::write(&b, b"NB").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("a.txt"), b"VA").unwrap();
        fs::write(dst.join("b.txt"), b"VB").unwrap();
        let req = super::super::transfer(false, vec![a, b], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::Overwrite,
                apply_all: true,
            },
        );
        // Dos choques pero "aplicar a todos" → un solo Conflict emitido.
        assert_eq!(conflicts, 1, "con apply_all solo se pregunta una vez");
        assert_eq!(summary.count_done(), 2);
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "NA");
        assert_eq!(fs::read_to_string(dst.join("b.txt")).unwrap(), "NB");
    }

    #[test]
    fn ask_rename_existing_aparta_el_viejo_y_entra_el_nuevo() {
        // "Renombrar antiguo": el EXISTENTE se aparta a "a (2).txt" y el NUEVO entra como "a.txt".
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"NUEVO").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
        let req = super::super::transfer(false, vec![src], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::RenameExisting,
                apply_all: false,
            },
        );
        assert_eq!(conflicts, 1);
        assert_eq!(summary.count_done(), 1);
        // El nuevo entró con el nombre original; el viejo quedó apartado con sufijo.
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "NUEVO");
        assert_eq!(fs::read_to_string(dst.join("a (2).txt")).unwrap(), "VIEJO");
    }

    #[test]
    fn ask_rename_existing_no_pisa_un_destino_de_respaldo_que_aparece_en_la_carrera() {
        struct BackupDestinationAppears;
        impl NoReplaceRename for BackupDestinationAppears {
            fn rename_noreplace(&self, _from: &Path, to: &Path) -> std::io::Result<()> {
                fs::write(to, b"EXTERNO").unwrap();
                Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "el nombre de respaldo se ocupó durante el rename",
                ))
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"NUEVO").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        let existing = dst.join("a.txt");
        let backup = dst.join("a (2).txt");
        fs::write(&existing, b"VIEJO").unwrap();
        let req = super::super::transfer(false, vec![src.clone()], dst);
        let p = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<OpMsg>();
        let (ctx, crx) = mpsc::channel::<ConflictDecision>();
        let responder = std::thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                if matches!(msg, OpMsg::Conflict(_)) {
                    let _ = ctx.send(ConflictDecision {
                        action: ConflictAction::RenameExisting,
                        apply_all: false,
                    });
                }
            }
        });

        let summary = run_plan_with_no_replace_rename(
            &p,
            &req.kind,
            ConflictPolicy::Ask,
            &token,
            &tx,
            &crx,
            None,
            &BackupDestinationAppears,
        );
        drop(tx);
        responder.join().unwrap();

        assert_eq!(summary.count_failed(), 1);
        assert_eq!(fs::read(&src).unwrap(), b"NUEVO");
        assert_eq!(fs::read(&existing).unwrap(), b"VIEJO");
        assert_eq!(fs::read(&backup).unwrap(), b"EXTERNO");
    }

    #[test]
    fn ask_skip_identical_salta_los_iguales() {
        // "Saltar idénticos": el existente y el entrante son IGUALES (mismo tamaño y mtime) → se
        // salta sin recopiar (el existente queda intacto).
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"mismo").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        let existing = dst.join("a.txt");
        fs::write(&existing, b"mismo").unwrap();
        // Igualar el mtime para que `are_identical` los considere idénticos.
        let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&src)
            .unwrap()
            .set_modified(t)
            .unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&existing)
            .unwrap()
            .set_modified(t)
            .unwrap();
        let req = super::super::transfer(false, vec![src], dst.clone());
        let (summary, conflicts) = run_ask(
            req,
            ConflictDecision {
                action: ConflictAction::SkipIdentical,
                apply_all: false,
            },
        );
        assert_eq!(conflicts, 1, "preguntó una vez");
        assert_eq!(summary.count_skipped(), 1, "idénticos → saltado");
    }

    #[test]
    fn skip_identical_suelto_sobre_distintos_repregunta_no_sobrescribe() {
        // "Saltar idénticos" elegido SUELTO (sin aplicar-a-todos) sobre un par que DIFIERE NO debe
        // sobrescribir en silencio (la palabra es "saltar"): debe volver a preguntar para que el
        // usuario decida de verdad. El test responde el primer prompt con SkipIdentical y el segundo
        // con Skip → el destino queda INTACTO (no pisado). Esperado: 2 prompts, destino sin cambiar.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"origen-distinto").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        let existing = dst.join("a.txt");
        fs::write(&existing, b"destino-original").unwrap();
        // mtime distinto a propósito (no igualamos) → are_identical = false.

        let req = super::super::transfer(false, vec![src], dst.clone());
        let p = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<OpMsg>();
        let (ctx, crx) = mpsc::channel::<ConflictDecision>();
        let prompts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let prompts2 = prompts.clone();
        // 1er prompt: SkipIdentical (suelto). Como difieren, el motor RE-pregunta. 2º prompt: Skip.
        let resp = std::thread::spawn(move || {
            let mut first = true;
            while let Ok(msg) = rx.recv() {
                if let OpMsg::Conflict(_) = msg {
                    prompts2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let action = if first {
                        first = false;
                        ConflictAction::SkipIdentical
                    } else {
                        ConflictAction::Skip
                    };
                    let _ = ctx.send(ConflictDecision {
                        action,
                        apply_all: false,
                    });
                }
            }
        });
        let summary = run_plan(&p, &req.kind, ConflictPolicy::Ask, &token, &tx, &crx, None);
        drop(tx);
        let _ = resp.join();

        assert_eq!(
            prompts.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "difieren → SkipIdentical suelto vuelve a preguntar (2 prompts)"
        );
        assert_eq!(
            fs::read_to_string(&existing).unwrap(),
            "destino-original",
            "el destino NO se sobrescribió: 'saltar idénticos' jamás pisa sin confirmar"
        );
        assert_eq!(summary.count_skipped(), 1);
    }

    #[test]
    fn skip_identical_apply_all_salta_iguales_y_repregunta_distintos() {
        // Política "saltar idénticos" con aplicar-a-todos: dos choques, uno IDÉNTICO (se salta sin
        // re-preguntar) y otro DISTINTO (se vuelve a preguntar). El test responde el segundo prompt
        // con Overwrite. Esperado: 2 prompts (el inicial que activa la política + el del distinto),
        // el idéntico saltado, el distinto sobrescrito.
        let dir = tempfile::tempdir().unwrap();
        // "ig.txt" idéntico en ambos lados; "di.txt" distinto.
        let ig_src = dir.path().join("ig.txt");
        let di_src = dir.path().join("di.txt");
        fs::write(&ig_src, b"igual").unwrap();
        fs::write(&di_src, b"origen-nuevo").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("ig.txt"), b"igual").unwrap();
        fs::write(dst.join("di.txt"), b"destino-viejo").unwrap();
        // Igualar mtime SOLO del par idéntico.
        let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        for p in [ig_src.clone(), dst.join("ig.txt")] {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }

        // Orden de los orígenes: el plan los procesa en el orden dado. Ponemos el idéntico PRIMERO
        // para que active la política, y el distinto después.
        let req = super::super::transfer(false, vec![ig_src, di_src], dst.clone());
        let p = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<OpMsg>();
        let (ctx, crx) = mpsc::channel::<ConflictDecision>();
        let prompts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let prompts2 = prompts.clone();
        // Primer prompt: SkipIdentical + aplicar a todos (activa la política). Siguientes: Overwrite.
        let resp = std::thread::spawn(move || {
            let mut first = true;
            while let Ok(msg) = rx.recv() {
                if let OpMsg::Conflict(_) = msg {
                    prompts2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let action = if first {
                        first = false;
                        ConflictDecision {
                            action: ConflictAction::SkipIdentical,
                            apply_all: true,
                        }
                    } else {
                        ConflictDecision {
                            action: ConflictAction::Overwrite,
                            apply_all: false,
                        }
                    };
                    let _ = ctx.send(action);
                }
            }
        });
        let summary = run_plan(&p, &req.kind, ConflictPolicy::Ask, &token, &tx, &crx, None);
        drop(tx);
        let _ = resp.join();
        let n = prompts.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            n, 2,
            "el idéntico se saltó solo; el distinto volvió a preguntar (2 prompts)"
        );
        // El idéntico quedó intacto (skip), el distinto fue sobrescrito por el origen.
        assert_eq!(fs::read_to_string(dst.join("ig.txt")).unwrap(), "igual");
        assert_eq!(
            fs::read_to_string(dst.join("di.txt")).unwrap(),
            "origen-nuevo"
        );
        assert_eq!(summary.count_skipped(), 1);
        assert_eq!(summary.count_done(), 1);
    }

    #[test]
    fn cancelar_durante_espera_aborta() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"NUEVO").unwrap();
        let dst = dir.path().join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
        let req = super::super::transfer(false, vec![src], dst.clone());
        let p = plan(&req).unwrap();
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel::<OpMsg>();
        let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
        let token2 = token.clone();
        // Cuando llegue el Conflict, NO responder: cancelar el token.
        let resp = std::thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                if let OpMsg::Conflict(_) = msg {
                    token2.cancel();
                }
            }
        });
        let summary = run_plan(&p, &req.kind, ConflictPolicy::Ask, &token, &tx, &crx, None);
        drop(tx);
        let _ = resp.join();
        // El item quedó Skipped (no se copió) y el existente intacto.
        assert_eq!(summary.count_done(), 0);
        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "VIEJO");
    }
}
