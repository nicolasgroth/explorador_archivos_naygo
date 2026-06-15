// Naygo — motor de operaciones: ejecuta un OpPlan en un worker, cancelable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Ejecuta los pasos de un `OpPlan` copiando archivos POR BUFFERS (cancelable a
//! media copia), emitiendo `OpMsg` por canal. Un error de un paso no aborta la op
//! (se registra en el summary). Cancelar borra el parcial del archivo en curso.
//! Papelera: NO aquí (solo borrado permanente); la papelera la hace `platform`.
//! Conflictos: la UI los resuelve antes de spawnear (el motor recibe pasos limpios).

use super::journal::JournalWriter;
use super::{
    ConflictAction, ConflictDecision, ConflictPolicy, ConflictPrompt, OpKind, OpMsg, OpOutcome,
    OpPlan, OpProgress, OpStep, OpSummary,
};
use crate::cancel::CancellationToken;
use std::io::{Read, Write};
use std::path::Path;
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
    mut journal: Option<JournalWriter>,
) -> (Receiver<OpMsg>, std::thread::JoinHandle<()>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let summary = run_plan(
            &plan,
            &kind,
            conflict,
            &token,
            &tx,
            &conflict_rx,
            journal.as_mut(),
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
    mut journal: Option<&mut JournalWriter>,
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

    for (idx, step) in plan.steps.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }

        // Progreso antes de tocar este paso.
        let _ = tx.send(OpMsg::Progress(OpProgress {
            bytes_done: summary.bytes_done,
            bytes_total: plan.total_bytes,
            files_done,
            files_total: plan.total_files,
            current: step.to.clone(),
        }));

        let (record_path, outcome, bytes_added, counts_as_file) =
            exec_step(step, kind, conflict, token, tx, conflict_rx, &mut applied_all);
        summary.bytes_done += bytes_added;
        if counts_as_file && matches!(outcome, OpOutcome::Done) {
            files_done += 1;
        }
        let resolved = matches!(outcome, OpOutcome::Done | OpOutcome::Skipped);
        summary.items.push((record_path, outcome));

        // El cursor del journal avanza solo mientras los pasos se resuelven de forma
        // contigua. El primer fallo levanta una barrera: a partir de ahí el cursor no
        // avanza más (aunque pasos posteriores tengan éxito), así el paso fallido queda
        // pendiente para el retomar. `done_through = journal_cursor` (throttled).
        if resolved && !barrier_hit {
            journal_cursor = idx + 1;
        } else if !resolved {
            barrier_hit = true;
        }
        if let Some(w) = journal.as_deref_mut() {
            w.record(journal_cursor, std::time::Instant::now());
        }
    }

    summary.elapsed_secs = start.elapsed().as_secs_f64();
    summary
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
) -> (std::path::PathBuf, OpOutcome, u64, bool) {
    match kind {
        OpKind::Copy => exec_copy_step(step, conflict, token, false, tx, conflict_rx, applied_all),
        OpKind::Move => exec_copy_step(step, conflict, token, true, tx, conflict_rx, applied_all),
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
                let outcome = exec_delete(step);
                (step.to.clone(), outcome, 0, !step.is_dir)
            }
        }
        // BatchRename: cada paso ya viene con su par from→to (ordenado por
        // dependencia en el plan); se ejecuta igual que un rename simple.
        OpKind::Rename { .. } | OpKind::BatchRename { .. } => {
            let outcome = exec_rename(step);
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
) -> (std::path::PathBuf, OpOutcome, u64, bool) {
    // Paso de carpeta: asegurar el directorio destino, no cuenta como archivo.
    if step.is_dir {
        let outcome = match std::fs::create_dir_all(&step.to) {
            Ok(()) => OpOutcome::Done,
            Err(e) => OpOutcome::Failed(e.to_string()),
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
    let to = if step.to.exists() {
        let effective: ConflictAction = match conflict {
            ConflictPolicy::Skip => ConflictAction::Skip,
            ConflictPolicy::Overwrite => ConflictAction::Overwrite,
            ConflictPolicy::Rename => ConflictAction::Rename,
            ConflictPolicy::Ask => {
                if let Some(prev) = *applied_all {
                    prev
                } else {
                    let _ = tx.send(OpMsg::Conflict(ConflictPrompt {
                        existing: step.to.clone(),
                        incoming: from.clone(),
                    }));
                    // Esperar la decisión sin colgar si se cancela: poll con timeout.
                    let decision = loop {
                        if token.is_cancelled() {
                            return (step.to.clone(), OpOutcome::Skipped, 0, true);
                        }
                        match conflict_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                            Ok(d) => break d,
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                return (step.to.clone(), OpOutcome::Skipped, 0, true);
                            }
                        }
                    };
                    if decision.apply_all {
                        *applied_all = Some(decision.action);
                    }
                    decision.action
                }
            }
        };
        match effective {
            ConflictAction::Skip => return (step.to.clone(), OpOutcome::Skipped, 0, true),
            ConflictAction::Rename => super::dedup_name(&step.to, &|p: &Path| p.exists()),
            ConflictAction::Overwrite => step.to.clone(),
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

    // Mover en el mismo volumen: intentar rename primero (rápido, sin copiar bytes).
    // Si falla (típicamente cross-volume), cae al fallback de copiar + borrar origen.
    if is_move && std::fs::rename(&from, &to).is_ok() {
        return (to, OpOutcome::Done, step.bytes, true);
    }

    match copy_buffered(&from, &to, token) {
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
            // Cancelado a media copia: borrar el parcial.
            let _ = std::fs::remove_file(&to);
            (to, OpOutcome::Skipped, 0, true)
        }
        Err(e) => {
            // Falló: limpiar cualquier parcial dejado.
            let _ = std::fs::remove_file(&to);
            (to, OpOutcome::Failed(e.to_string()), 0, true)
        }
    }
}

/// Copia `from` → `to` por bloques de 256KB, chequeando la cancelación entre bloques.
/// Devuelve `Ok(true)` si copió todo, `Ok(false)` si se canceló a media copia,
/// `Err` ante un error de I/O.
fn copy_buffered(from: &Path, to: &Path, token: &CancellationToken) -> std::io::Result<bool> {
    let mut reader = std::fs::File::open(from)?;
    let mut writer = std::fs::File::create(to)?;
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        if token.is_cancelled() {
            return Ok(false);
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
    }
    writer.flush()?;
    Ok(true)
}

/// Borra (permanente) un archivo o carpeta del paso.
fn exec_delete(step: &OpStep) -> OpOutcome {
    let target = step.from.as_ref().unwrap_or(&step.to);
    let result = if step.is_dir {
        std::fs::remove_dir_all(target)
    } else {
        std::fs::remove_file(target)
    };
    match result {
        Ok(()) => OpOutcome::Done,
        Err(e) => OpOutcome::Failed(e.to_string()),
    }
}

/// Renombra `from` → `to` (mismo directorio, nombre nuevo).
fn exec_rename(step: &OpStep) -> OpOutcome {
    let from = match &step.from {
        Some(p) => p,
        None => return OpOutcome::Failed("rename sin origen".into()),
    };
    match std::fs::rename(from, &step.to) {
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
                    let _ = ctx.send(decision);
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
