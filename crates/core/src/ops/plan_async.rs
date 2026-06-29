// Naygo — planificación de copia/movimiento en segundo plano (worker + canal + cancelación).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Para una carpeta grande, planificar una copia/movimiento implica recorrer TODO el árbol
//! de orígenes con `read_dir`/`metadata` (lo hace `plan::expand`). Hacerlo en el hilo de UI
//! congela la ventana: viola la regla de oro del proyecto (la UI nunca hace I/O de disco).
//!
//! `spawn_plan` mueve ese recorrido a un worker, espejando el molde de `search::spawn_search`
//! y `deep_listing::spawn_deep_listing`: un hilo recorre el árbol con un `CancellationToken`,
//! emite `PlanMsg::Progress` con throttle (cada ~100 ms) mientras avanza, y al terminar emite
//! `PlanMsg::Done(OpPlan)` (o `Cancelled`/`Failed`). La UI drena el `Receiver` frame a frame y
//! recién entonces arranca el motor de copia. El recorrido REUSA `plan::plan_transfer_with`
//! (no se duplica el walk); solo se le inyecta un sink de progreso y el predicado de cancelación.
//!
//! Solo Copy/Move pasan por aquí (son los que recorren el árbol). El resto de operaciones
//! (Delete/Rename/Create/BatchRename) planifican en O(1) y siguen usando `plan()` directo.

use super::plan::{plan, plan_transfer_with, PlanError};
use super::{OpKind, OpPlan, OpRequest};
use crate::cancel::CancellationToken;
use std::sync::mpsc::{channel, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Throttle del mensaje `Progress` mientras se recorre el árbol (igual idea que en `search`).
const PROGRESS_THROTTLE: Duration = Duration::from_millis(100);

/// Mensajes del worker de planificación hacia la UI. Espeja a `SearchMsg`/`DeepMsg`: progreso
/// incremental + un mensaje terminal (`Done`/`Cancelled`/`Failed`).
/// (No deriva `Eq` por consistencia con el resto; `PartialEq` basta para los tests.)
#[derive(Debug, Clone, PartialEq)]
pub enum PlanMsg {
    /// Avance del escaneo: archivos y bytes contabilizados hasta ahora (throttled).
    Progress { files: u64, bytes: u64 },
    /// Terminó de recorrer: el plan completo, listo para arrancar el motor.
    Done(OpPlan),
    /// Se abortó porque el token fue cancelado durante el escaneo.
    Cancelled,
    /// No se pudo planificar (origen ilegible, destino dentro del origen, etc.).
    Failed(PlanError),
}

/// Lanza la planificación de `req` en un worker. Devuelve el receptor del canal (la UI lo
/// drena frame a frame) y el `JoinHandle`. Cancelable vía `token`.
///
/// Para Copy/Move recorre el árbol emitiendo progreso. Para el resto (planificación O(1)) llama
/// a `plan()` directo y emite `Done`/`Failed` de una; igual corre en el worker para que la UI
/// tenga un único camino (siempre consume un `Receiver<PlanMsg>`), sin congelar nunca el hilo.
pub fn spawn_plan(req: OpRequest, token: CancellationToken) -> (Receiver<PlanMsg>, JoinHandle<()>) {
    let (tx, rx) = channel();
    let handle = thread::spawn(move || {
        // Solo Copy/Move recorren árbol; el resto planifica en O(1).
        let is_transfer = matches!(req.kind, OpKind::Copy | OpKind::Move);
        if !is_transfer {
            let msg = match plan(&req) {
                Ok(p) => PlanMsg::Done(p),
                Err(e) => PlanMsg::Failed(e),
            };
            let _ = tx.send(msg);
            return;
        }

        // Recorrido con progreso: REUSA `plan_transfer_with`, inyectando el sink y el predicado.
        // El PRIMER archivo emite ya (feedback inmediato "Calculando… 1 archivo"); a partir de
        // ahí, throttle para no inundar el canal en árboles enormes.
        let mut last: Option<Instant> = None;
        let mut sink = |files: usize, bytes: u64| {
            let fire = match last {
                None => true,
                Some(t) => t.elapsed() >= PROGRESS_THROTTLE,
            };
            if fire {
                let _ = tx.send(PlanMsg::Progress {
                    files: files as u64,
                    bytes,
                });
                last = Some(Instant::now());
            }
        };
        let cancelled = || token.is_cancelled();
        let result = plan_transfer_with(&req, &mut sink, &cancelled);

        // Si se canceló durante el escaneo, prima el `Cancelled` (el plan parcial se descarta).
        if token.is_cancelled() {
            let _ = tx.send(PlanMsg::Cancelled);
            return;
        }
        let msg = match result {
            Ok(p) => PlanMsg::Done(p),
            Err(e) => PlanMsg::Failed(e),
        };
        let _ = tx.send(msg);
    });
    (rx, handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{transfer, ConflictPolicy, OpKind, OpRequest};
    use std::fs;
    use std::path::PathBuf;

    fn drain(rx: Receiver<PlanMsg>) -> Vec<PlanMsg> {
        rx.into_iter().collect()
    }

    /// Árbol temporal con varios archivos y subcarpetas; devuelve (tempdir, src, dest).
    fn arbol() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"aa").unwrap(); // 2 bytes
        fs::write(src.join("b.txt"), b"bbbb").unwrap(); // 4 bytes
        fs::create_dir(src.join("sub")).unwrap();
        fs::write(src.join("sub/c.txt"), b"cccccc").unwrap(); // 6 bytes
        fs::create_dir(src.join("sub/deep")).unwrap();
        fs::write(src.join("sub/deep/d.txt"), b"dddddddd").unwrap(); // 8 bytes
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        (dir, src, dest)
    }

    #[test]
    fn emite_progress_y_done_con_totales_correctos() {
        let (_dir, src, dest) = arbol();
        let req = transfer(false, vec![src], dest);
        // Verdad de referencia: el plan SÍNCRONO sobre el mismo árbol.
        let sync = plan(&req).unwrap();
        assert_eq!(sync.total_files, 4);
        assert_eq!(sync.total_bytes, 2 + 4 + 6 + 8);

        let token = CancellationToken::new();
        let (rx, _h) = spawn_plan(req, token);
        let msgs = drain(rx);

        // El último mensaje debe ser Done con el MISMO plan que el síncrono.
        let done = msgs
            .iter()
            .find_map(|m| match m {
                PlanMsg::Done(p) => Some(p.clone()),
                _ => None,
            })
            .expect("emite Done");
        assert_eq!(done.total_files, sync.total_files);
        assert_eq!(done.total_bytes, sync.total_bytes);
        assert_eq!(done.steps.len(), sync.steps.len());
        // Llega al menos un Progress (el primer archivo lo dispara siempre).
        assert!(
            msgs.iter().any(|m| matches!(m, PlanMsg::Progress { .. })),
            "emite al menos un Progress durante el escaneo: {msgs:?}"
        );
        // No hubo cancelación ni fallo.
        assert!(!msgs.iter().any(|m| matches!(m, PlanMsg::Cancelled)));
        assert!(!msgs.iter().any(|m| matches!(m, PlanMsg::Failed(_))));
    }

    #[test]
    fn cancelar_antes_de_empezar_emite_cancelled_y_no_done() {
        let (_dir, src, dest) = arbol();
        let req = transfer(false, vec![src], dest);
        let token = CancellationToken::new();
        token.cancel(); // cancelado ANTES de drenar: el worker corta de inmediato
        let (rx, _h) = spawn_plan(req, token);
        let msgs = drain(rx);
        assert!(
            msgs.iter().any(|m| matches!(m, PlanMsg::Cancelled)),
            "emite Cancelled: {msgs:?}"
        );
        assert!(
            !msgs.iter().any(|m| matches!(m, PlanMsg::Done(_))),
            "NO emite Done si se canceló"
        );
    }

    #[test]
    fn origen_inexistente_emite_failed() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("no_existe");
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = transfer(false, vec![src], dest);
        let token = CancellationToken::new();
        let (rx, _h) = spawn_plan(req, token);
        let msgs = drain(rx);
        assert!(
            msgs.iter()
                .any(|m| matches!(m, PlanMsg::Failed(PlanError::SourceUnreadable(_)))),
            "origen ilegible → Failed(SourceUnreadable): {msgs:?}"
        );
    }

    #[test]
    fn delete_planifica_directo_sin_recorrer_arbol() {
        // Una op que no recorre árbol (Delete) igual pasa por spawn_plan y emite Done O(1).
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        fs::write(&f, b"x").unwrap();
        let req = OpRequest {
            kind: OpKind::Delete { to_trash: false },
            sources: vec![f],
            dest_dir: None,
            conflict: ConflictPolicy::Overwrite,
        };
        let token = CancellationToken::new();
        let (rx, _h) = spawn_plan(req, token);
        let msgs = drain(rx);
        assert!(msgs.iter().any(|m| matches!(m, PlanMsg::Done(_))));
    }
}
