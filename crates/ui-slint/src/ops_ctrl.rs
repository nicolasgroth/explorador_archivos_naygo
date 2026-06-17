// Naygo — controlador de operaciones de archivo de la UI Slint (Fase 3).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Posee TODO el estado de operaciones: ops en curso/terminadas, el modal activo, el
// historial de deshacer, el set de rutas "cortadas" (corte visual) y las ops a retomar.
// El motor de core corre en su hilo; `pump_ops` drena el progreso desde el slint::Timer.
// Espeja el patrón del NaygoApp de egui, pero aislado del resto del controlador.

use naygo_core::cancel::CancellationToken;
use naygo_core::ops::engine;
use naygo_core::ops::journal::{self, JournalWriter, OpJournal};
use naygo_core::ops::undo::{self, UndoEntry};
use naygo_core::ops::{
    ConflictAction, ConflictDecision, ConflictPolicy, ConflictPrompt, OpKind, OpMsg, OpPlan,
    OpProgress, OpRequest, OpSummary,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

/// Para qué se pide un nombre en el modal `NameInput`. (El rename pasó a ser inline en 6D;
/// el modal queda para crear archivo/carpeta y para confirmar el nombre al pegar.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamePurpose {
    NewFile,
    NewDir,
    /// Confirmar el nombre de un archivo pegado (texto/imagen). `ext` es la extensión a
    /// concatenar (sin punto) y `bytes` el contenido ya codificado a escribir. Solo aparece si
    /// `Settings.paste_confirm` está activo; si no, el pegado escribe directo.
    Paste {
        ext: String,
        bytes: Vec<u8>,
    },
}

/// El modal de operaciones activo (uno a la vez).
#[derive(Clone, Debug)]
pub enum OpDialog {
    /// Confirmar borrado de `sources` (a papelera o permanente).
    ConfirmDelete {
        sources: Vec<PathBuf>,
        permanent: bool,
    },
    /// Conflicto por-ítem (ops-B): el motor preguntó por el choque en `prompt`.
    Conflict {
        op_index: usize,
        prompt: ConflictPrompt,
    },
    /// Pedir un nombre (nuevo archivo/carpeta, renombrar). `dir` es dónde se crea.
    NameInput {
        purpose: NamePurpose,
        dir: PathBuf,
        buf: String,
    },
    /// Retomar operaciones journaleadas tras un cierre inesperado.
    Resume { items: Vec<OpJournal> },
}

/// Una operación en curso o terminada.
pub struct ActiveOp {
    /// Canal de mensajes del motor; `None` cuando terminó.
    pub rx: Option<Receiver<OpMsg>>,
    /// Extremo de envío de decisiones de conflicto (ops-B).
    pub conflict_tx: Sender<ConflictDecision>,
    pub token: CancellationToken,
    pub label: String,
    pub progress: Option<OpProgress>,
    pub summary: Option<OpSummary>,
    /// `true` si ya se lanzó al motor; `false` si está en cola esperando turno.
    pub started: bool,
    /// Plan en espera (modo cola): se lanza cuando le toca el turno.
    pub pending: Option<(OpPlan, OpKind, ConflictPolicy)>,
    /// Id del journal (Copy/Move/Delete-permanente); se borra al terminar.
    pub journal_id: Option<String>,
    /// El request original, para construir el undo al terminar (None si no se registra).
    pub request: Option<OpRequest>,
    /// Si el motor está esperando una decisión de conflicto, el prompt pendiente.
    pub awaiting_conflict: Option<ConflictPrompt>,
}

/// Modo de ejecución de operaciones múltiples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpsMode {
    /// Todas en paralelo (cada una su hilo).
    Parallel,
    /// Una por vez (las siguientes esperan en cola).
    Queue,
}

/// Estado completo de operaciones de archivo.
pub struct OpsCtrl {
    pub active_ops: Vec<ActiveOp>,
    pub pending_dialog: Option<OpDialog>,
    pub undo_history: Vec<UndoEntry>,
    pub next_undo_id: u64,
    /// Rutas marcadas como "cortadas" (se pintan atenuadas hasta pegar/cancelar).
    pub cut_set: HashSet<PathBuf>,
    pub ops_mode: OpsMode,
    pub config_dir: PathBuf,
    /// Secuencia para ids de journal únicos en la sesión (`op-N`).
    next_journal_seq: u64,
}

impl OpsCtrl {
    pub fn new(config_dir: PathBuf) -> OpsCtrl {
        OpsCtrl {
            active_ops: Vec::new(),
            pending_dialog: None,
            undo_history: Vec::new(),
            next_undo_id: 1,
            cut_set: HashSet::new(),
            ops_mode: OpsMode::Parallel,
            config_dir,
            next_journal_seq: 1,
        }
    }

    // --- Clipboard interno (corte visual) ---

    /// Copia `paths` al portapapeles del SO; limpia la marca de corte.
    pub fn set_copy(&mut self, paths: &[PathBuf]) {
        let _ = naygo_platform::clipboard::write_files(paths, false);
        self.cut_set.clear();
    }

    /// Corta `paths`: escribe al portapapeles con efecto MOVE y los marca como cortados.
    pub fn set_cut(&mut self, paths: &[PathBuf]) {
        let _ = naygo_platform::clipboard::write_files(paths, true);
        self.cut_set = paths.iter().cloned().collect();
    }

    /// Limpia la marca de corte (tras pegar, o al cancelar con Esc).
    pub fn clear_cut(&mut self) {
        self.cut_set.clear();
    }

    /// ¿La ruta está marcada como cortada?
    pub fn is_cut(&self, path: &Path) -> bool {
        self.cut_set.contains(path)
    }

    // --- Lanzar y drenar operaciones ---

    /// ¿Algún source de `req` chocaría con un archivo ya existente en el destino? Pre-check
    /// para decidir si el motor debe preguntar (Ask) o no (Overwrite directo). Solo aplica a
    /// Copy/Move (los Create/Rename los maneja su propia ruta).
    pub fn first_collision(&self, req: &OpRequest) -> bool {
        let Some(dest) = req.dest_dir.as_ref() else {
            return false;
        };
        if !matches!(req.kind, OpKind::Copy | OpKind::Move) {
            return false;
        }
        req.sources.iter().any(|s| {
            s.file_name()
                .map(|n| dest.join(n).exists())
                .unwrap_or(false)
        })
    }

    /// Lanza una operación. `record_undo` indica si registrar el deshacer al terminar.
    /// La papelera (Delete to_trash) se hace directo (atómica, fuera del motor) — el
    /// llamador debe refrescar el panel tras esto.
    pub fn start_op(&mut self, req: OpRequest, label: String, record_undo: bool) {
        // Al comenzar una operación nueva, descartar las terminadas del panel (sus
        // resúmenes ya se vieron); las en curso/en cola se conservan.
        self.prune_finished();
        // Papelera: atómica, sin motor.
        if matches!(req.kind, OpKind::Delete { to_trash: true }) {
            let _ = naygo_platform::trash::move_to_trash(&req.sources);
            // No se ofrece deshacer-a-papelera (recuperable desde Windows).
            return;
        }

        let plan = match naygo_core::ops::plan(&req) {
            Ok(p) => p,
            Err(_) => return, // error de planificación: se ignora discreto (TODO: avisar)
        };

        // Pre-check de conflicto: si choca, el motor pregunta por ítem (Ask); si no, va directo.
        let conflict = if self.first_collision(&req) {
            ConflictPolicy::Ask
        } else {
            ConflictPolicy::Overwrite
        };

        // Modo cola: si hay otra corriendo, encolar sin spawnear.
        if self.ops_mode == OpsMode::Queue && self.any_running() {
            let (conflict_tx, _crx) = std::sync::mpsc::channel::<ConflictDecision>();
            self.active_ops.push(ActiveOp {
                rx: None,
                conflict_tx,
                token: CancellationToken::new(),
                label,
                progress: None,
                summary: None,
                started: false,
                pending: Some((plan, req.kind.clone(), conflict)),
                journal_id: None,
                request: record_undo.then_some(req),
                awaiting_conflict: None,
            });
            return;
        }

        self.spawn_op(
            plan,
            req.kind.clone(),
            conflict,
            label,
            record_undo.then_some(req),
        );
    }

    /// Spawnea el motor para un plan ya resuelto y agrega la `ActiveOp`. Crea un journal
    /// (para retomar tras un cierre inesperado) en Copy/Move/Delete-permanente.
    fn spawn_op(
        &mut self,
        plan: OpPlan,
        kind: OpKind,
        conflict: ConflictPolicy,
        label: String,
        request: Option<OpRequest>,
    ) {
        let token = CancellationToken::new();
        let (conflict_tx, conflict_rx) = std::sync::mpsc::channel::<ConflictDecision>();
        // Journal solo para operaciones largas y deshacibles-por-retomar.
        let (journal, journal_id) = if Self::journalable(&kind) {
            let id = format!("op-{}", self.next_journal_seq);
            self.next_journal_seq += 1;
            let j = OpJournal::new(id.clone(), kind.clone(), conflict, plan.clone());
            (Some(JournalWriter::new(&self.config_dir, j)), Some(id))
        } else {
            (None, None)
        };
        let (rx, _h) = engine::spawn(plan, kind, conflict, token.clone(), conflict_rx, journal);
        self.active_ops.push(ActiveOp {
            rx: Some(rx),
            conflict_tx,
            token,
            label,
            progress: None,
            summary: None,
            started: true,
            pending: None,
            journal_id,
            request,
            awaiting_conflict: None,
        });
    }

    /// ¿La operación amerita journal (es larga y se puede retomar)?
    fn journalable(kind: &OpKind) -> bool {
        matches!(
            kind,
            OpKind::Copy | OpKind::Move | OpKind::Delete { to_trash: false }
        )
    }

    /// ¿Hay alguna op realmente corriendo (con canal vivo)?
    fn any_running(&self) -> bool {
        self.active_ops.iter().any(|o| o.started && o.rx.is_some())
    }

    /// Drena los mensajes de todas las ops en curso. Abre el modal de conflicto si alguna
    /// op lo pide. Registra el undo de las que terminan. Lanza la siguiente de la cola si
    /// nada corre. Devuelve true si TODO está en reposo (para apagar el timer).
    pub fn pump_ops(&mut self) -> bool {
        for i in 0..self.active_ops.len() {
            if self.active_ops[i].rx.is_none() {
                continue;
            }
            // Drenar sin bloquear hacia variables locales (sin retener el préstamo de rx
            // mientras mutamos otros campos de la misma ActiveOp).
            let mut finished: Option<OpSummary> = None;
            let mut last_progress: Option<OpProgress> = None;
            let mut new_conflict: Option<ConflictPrompt> = None;
            if let Some(rx) = self.active_ops[i].rx.as_ref() {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        OpMsg::Progress(p) => last_progress = Some(p),
                        OpMsg::Conflict(prompt) => new_conflict = Some(prompt),
                        OpMsg::Done(s) | OpMsg::Cancelled(s) => {
                            finished = Some(s);
                            break;
                        }
                        OpMsg::Failed(_) => {
                            finished = Some(OpSummary::default());
                            break;
                        }
                    }
                }
            }
            if let Some(p) = last_progress {
                self.active_ops[i].progress = Some(p);
            }
            if let Some(prompt) = new_conflict {
                self.active_ops[i].awaiting_conflict = Some(prompt);
            }
            if let Some(summary) = finished {
                // Registrar el deshacer si corresponde.
                if let Some(req) = self.active_ops[i].request.take() {
                    if let Some(actions) = undo::build_undo(&req, &summary) {
                        if !actions.is_empty() {
                            let id = self.next_undo_id;
                            self.next_undo_id += 1;
                            self.undo_history.push(UndoEntry {
                                id,
                                label: self.active_ops[i].label.clone(),
                                when_epoch_secs: now_epoch_secs(),
                                actions,
                                undone: false,
                            });
                            // Tope de 100 entradas (descartar las más viejas).
                            if self.undo_history.len() > 100 {
                                self.undo_history.remove(0);
                            }
                        }
                    }
                }
                // Borrar el journal: la op terminó, ya no hay que retomarla.
                if let Some(jid) = self.active_ops[i].journal_id.take() {
                    journal::remove(&self.config_dir, &jid);
                }
                self.active_ops[i].summary = Some(summary);
                self.active_ops[i].rx = None;
            }
        }

        // Abrir el modal de conflicto si alguna op lo espera y no hay otro modal abierto.
        if self.pending_dialog.is_none() {
            if let Some(idx) = self
                .active_ops
                .iter()
                .position(|o| o.awaiting_conflict.is_some())
            {
                let prompt = self.active_ops[idx].awaiting_conflict.clone().unwrap();
                self.pending_dialog = Some(OpDialog::Conflict {
                    op_index: idx,
                    prompt,
                });
            }
        }

        // Si nada corre y hay una en cola, lanzarla.
        if !self.any_running() {
            if let Some(idx) = self
                .active_ops
                .iter()
                .position(|o| !o.started && o.pending.is_some())
            {
                let (plan, kind, conflict) = self.active_ops[idx].pending.take().unwrap();
                let label = self.active_ops[idx].label.clone();
                let request = self.active_ops[idx].request.take();
                // Quitar el placeholder en cola y spawnear de verdad.
                self.active_ops.remove(idx);
                self.spawn_op(plan, kind, conflict, label, request);
            }
        }

        self.active_ops.iter().all(|o| o.rx.is_none())
    }

    /// Resuelve el conflicto pendiente de la op `op_index` con la acción dada.
    pub fn resolve_conflict(&mut self, op_index: usize, action: ConflictAction, apply_all: bool) {
        if let Some(op) = self.active_ops.get_mut(op_index) {
            let _ = op.conflict_tx.send(ConflictDecision { action, apply_all });
            op.awaiting_conflict = None;
        }
        self.pending_dialog = None;
    }

    /// Cancela la op `op_index`.
    pub fn cancel_op(&mut self, op_index: usize) {
        if let Some(op) = self.active_ops.get(op_index) {
            op.token.cancel();
        }
    }

    /// Quita las ops terminadas (sin canal y con resumen ya mostrado). El llamador decide
    /// cuándo podar (p. ej. al ocultar el panel).
    pub fn prune_finished(&mut self) {
        self.active_ops
            .retain(|o| o.rx.is_some() || !o.started || o.pending.is_some());
    }

    // --- Datos para la UI (modal activo + filas de progreso) ---

    /// Datos planos del modal activo para Slint. `kind`: 0=ninguno 1=borrado 2=conflicto
    /// 3=nombre 4=pegar. (Resume=5 lo agrega la fase de journal.)
    pub fn dialog_vm(&self) -> OpDialogVmData {
        match &self.pending_dialog {
            Some(OpDialog::ConfirmDelete { sources, permanent }) => OpDialogVmData {
                kind: 1,
                del_count: sources.len() as i32,
                del_permanent: *permanent,
                ..Default::default()
            },
            Some(OpDialog::Conflict { prompt, .. }) => OpDialogVmData {
                kind: 2,
                conflict_name: prompt
                    .existing
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                ..Default::default()
            },
            Some(OpDialog::NameInput { purpose, buf, .. }) => OpDialogVmData {
                kind: 3,
                name_title: match purpose {
                    NamePurpose::NewFile => "Nuevo archivo".to_string(),
                    NamePurpose::NewDir => "Nueva carpeta".to_string(),
                    NamePurpose::Paste { ext, .. } => format!("Pegar como nombre.{ext}"),
                },
                name_value: buf.clone(),
                name_valid: naygo_core::ops::names::is_valid_name(buf),
                ..Default::default()
            },
            Some(OpDialog::Resume { .. }) => OpDialogVmData {
                kind: 5,
                ..Default::default()
            },
            _ => OpDialogVmData::default(),
        }
    }

    /// Filas del modal "retomar" (id + etiqueta) si está activo; vacío si no.
    pub fn resume_rows(&self) -> Vec<(String, String)> {
        match &self.pending_dialog {
            Some(OpDialog::Resume { items }) => {
                items.iter().map(|j| (j.id.clone(), j.label())).collect()
            }
            _ => Vec::new(),
        }
    }

    /// Filas del panel de progreso (una por op activa o terminada con resumen).
    pub fn op_rows(&self) -> Vec<OpRowData> {
        self.active_ops
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let running = o.rx.is_some();
                let percent = match &o.progress {
                    Some(p) if p.bytes_total > 0 => {
                        (p.bytes_done as f32 / p.bytes_total as f32) * 100.0
                    }
                    _ if o.summary.is_some() => 100.0,
                    _ => 0.0,
                };
                let status = if !o.started {
                    "en cola".to_string()
                } else if let Some(s) = &o.summary {
                    format!(
                        "hecho: {} copiados, {} saltados, {} fallidos",
                        s.count_done(),
                        s.count_skipped(),
                        s.count_failed()
                    )
                } else if o.awaiting_conflict.is_some() {
                    "esperando decisión…".to_string()
                } else {
                    "en curso…".to_string()
                };
                OpRowData {
                    index: i as i32,
                    label: o.label.clone(),
                    percent,
                    status,
                    running,
                }
            })
            .collect()
    }

    // --- Aplicar decisiones de los modales ---

    /// Actualiza el texto del campo de nombre mientras el usuario escribe (revalida).
    pub fn name_changed(&mut self, value: String) {
        if let Some(OpDialog::NameInput { buf, .. }) = &mut self.pending_dialog {
            *buf = value;
        }
    }

    /// Confirma el modal de nombre: crea archivo/carpeta o renombra. Devuelve true si
    /// arrancó una op (para reactivar el timer).
    pub fn name_confirm(&mut self) -> bool {
        let Some(OpDialog::NameInput { purpose, dir, buf }) = self.pending_dialog.take() else {
            return false;
        };
        if !naygo_core::ops::names::is_valid_name(&buf) {
            // Reabrir el modal con el valor inválido (no debería pasar: el botón se inhabilita).
            self.pending_dialog = Some(OpDialog::NameInput { purpose, dir, buf });
            return false;
        }
        let (req, label) = match purpose {
            NamePurpose::NewFile => (naygo_core::ops::create(dir, buf, false), "Nuevo archivo"),
            NamePurpose::NewDir => (naygo_core::ops::create(dir, buf, true), "Nueva carpeta"),
            NamePurpose::Paste { ext, bytes } => {
                // El pegado escribe el archivo directo (escritura chica y local), igual que el
                // camino sin confirmación. No pasa por el engine de ops.
                let path = dir.join(format!("{buf}.{ext}"));
                let _ = std::fs::write(&path, &bytes);
                return true;
            }
        };
        self.start_op(req, label.to_string(), true);
        true
    }

    /// Cancela el modal activo (botón Cancelar o Esc).
    pub fn dialog_cancel(&mut self) {
        self.pending_dialog = None;
    }

    /// Confirma el borrado pendiente: lanza la op. Devuelve true si arrancó algo.
    pub fn delete_confirm(&mut self) -> bool {
        let Some(OpDialog::ConfirmDelete { sources, permanent }) = self.pending_dialog.take()
        else {
            return false;
        };
        let req = naygo_core::ops::delete(sources, !permanent);
        let label = if permanent {
            "Eliminar permanente"
        } else {
            "Enviar a papelera"
        };
        self.start_op(req, label.to_string(), true);
        true
    }

    // --- Journal: retomar operaciones tras un cierre inesperado ---

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
        let (rx, _h) = engine::spawn(
            resume.plan,
            journal.kind.clone(),
            journal.conflict,
            token.clone(),
            conflict_rx,
            Some(writer),
        );
        self.active_ops.push(ActiveOp {
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

/// Datos planos del modal activo (espejo de `OpDialogVm` de Slint).
#[derive(Clone, Debug, Default)]
pub struct OpDialogVmData {
    pub kind: i32,
    pub del_count: i32,
    pub del_permanent: bool,
    pub conflict_name: String,
    pub name_title: String,
    pub name_value: String,
    pub name_valid: bool,
    pub paste_name: String,
    pub paste_is_image: bool,
}

/// Datos planos de una fila del panel de progreso (espejo de `OpRowVm` de Slint).
#[derive(Clone, Debug)]
pub struct OpRowData {
    pub index: i32,
    pub label: String,
    pub percent: f32,
    pub status: String,
    pub running: bool,
}

/// Segundos desde la época Unix (para el timestamp del UndoEntry).
fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(c: &mut OpsCtrl) {
        for _ in 0..4000 {
            let done = c.pump_ops();
            if done && !c.active_ops.is_empty() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn cut_set_marca_y_limpia() {
        let mut c = OpsCtrl::new(std::env::temp_dir());
        c.cut_set.insert(PathBuf::from("C:/x/a.txt"));
        assert!(c.is_cut(Path::new("C:/x/a.txt")));
        c.clear_cut();
        assert!(!c.is_cut(Path::new("C:/x/a.txt")));
    }

    #[test]
    fn first_collision_detecta_choque() {
        let tmp = tempfile::tempdir().unwrap();
        let dst = tmp.path().join("dst");
        std::fs::create_dir(&dst).unwrap();
        std::fs::write(dst.join("a.txt"), b"x").unwrap();
        let c = OpsCtrl::new(tmp.path().to_path_buf());
        let req = naygo_core::ops::transfer(false, vec![tmp.path().join("a.txt")], dst.clone());
        assert!(c.first_collision(&req), "a.txt ya existe en dst");
        let req2 = naygo_core::ops::transfer(false, vec![tmp.path().join("z.txt")], dst);
        assert!(!c.first_collision(&req2));
    }

    #[test]
    fn start_op_copia_y_pump_la_completa() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("a.txt");
        std::fs::write(&src, b"hola").unwrap();
        let dst = tmp.path().join("dst");
        std::fs::create_dir(&dst).unwrap();
        let mut c = OpsCtrl::new(tmp.path().to_path_buf());
        let req = naygo_core::ops::transfer(false, vec![src], dst.clone());
        c.start_op(req, "Copiar".to_string(), true);
        drain(&mut c);
        assert!(dst.join("a.txt").exists());
    }

    #[test]
    fn paste_confirm_escribe_con_el_nombre_elegido() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = OpsCtrl::new(tmp.path().to_path_buf());
        // Modal de confirmación de pegado: nombre propuesto "pegado", ext "txt", bytes.
        c.pending_dialog = Some(OpDialog::NameInput {
            purpose: NamePurpose::Paste {
                ext: "txt".into(),
                bytes: b"hola pegado".to_vec(),
            },
            dir: tmp.path().to_path_buf(),
            buf: "pegado".into(),
        });
        // El usuario edita el nombre y confirma.
        c.name_changed("mi_nota".into());
        assert!(c.name_confirm(), "el confirm escribe el archivo");
        let dest = tmp.path().join("mi_nota.txt");
        assert!(
            dest.exists(),
            "se creó con el nombre elegido + la extensión"
        );
        assert_eq!(std::fs::read(&dest).unwrap(), b"hola pegado");
        assert!(c.pending_dialog.is_none(), "el modal se cerró");
    }

    #[test]
    fn copia_registra_undo() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("a.txt");
        std::fs::write(&src, b"hola").unwrap();
        let dst = tmp.path().join("dst");
        std::fs::create_dir(&dst).unwrap();
        let mut c = OpsCtrl::new(tmp.path().to_path_buf());
        c.start_op(
            naygo_core::ops::transfer(false, vec![src], dst.clone()),
            "Copiar".into(),
            true,
        );
        drain(&mut c);
        assert_eq!(c.undo_history.len(), 1, "la copia registra un undo");
        assert!(!c.undo_history[0].label.is_empty());
    }

    #[test]
    fn conflicto_se_resuelve_con_overwrite() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("a.txt");
        std::fs::write(&src, b"NUEVO").unwrap();
        let dst = tmp.path().join("dst");
        std::fs::create_dir(&dst).unwrap();
        std::fs::write(dst.join("a.txt"), b"VIEJO").unwrap();
        let mut c = OpsCtrl::new(tmp.path().to_path_buf());
        c.start_op(
            naygo_core::ops::transfer(false, vec![src], dst.clone()),
            "Copiar".into(),
            true,
        );
        let mut resolved = false;
        for _ in 0..4000 {
            c.pump_ops();
            if !resolved {
                if let Some(OpDialog::Conflict { op_index, .. }) = c.pending_dialog.clone() {
                    c.resolve_conflict(op_index, ConflictAction::Overwrite, false);
                    resolved = true;
                }
            }
            if c.active_ops.iter().all(|o| o.summary.is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(resolved, "se abrió y resolvió el conflicto");
        assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "NUEVO");
    }

    #[test]
    fn cola_lanza_la_segunda_al_terminar_la_primera() {
        let tmp = tempfile::tempdir().unwrap();
        for n in ["a.txt", "b.txt"] {
            std::fs::write(tmp.path().join(n), b"x").unwrap();
        }
        let d1 = tmp.path().join("d1");
        let d2 = tmp.path().join("d2");
        std::fs::create_dir(&d1).unwrap();
        std::fs::create_dir(&d2).unwrap();
        let mut c = OpsCtrl::new(tmp.path().to_path_buf());
        c.ops_mode = OpsMode::Queue;
        c.start_op(
            naygo_core::ops::transfer(false, vec![tmp.path().join("a.txt")], d1.clone()),
            "Copiar a".into(),
            false,
        );
        c.start_op(
            naygo_core::ops::transfer(false, vec![tmp.path().join("b.txt")], d2.clone()),
            "Copiar b".into(),
            false,
        );
        // Solo una arrancó; la otra quedó en cola.
        assert_eq!(c.active_ops.iter().filter(|o| o.started).count(), 1);
        drain(&mut c);
        assert!(d1.join("a.txt").exists());
        assert!(d2.join("b.txt").exists());
    }

    #[test]
    fn una_copia_crea_journal_y_lo_borra_al_terminar() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        std::fs::create_dir(&cfg).unwrap();
        let src = tmp.path().join("a.txt");
        std::fs::write(&src, b"x").unwrap();
        let dst = tmp.path().join("dst");
        std::fs::create_dir(&dst).unwrap();
        let mut c = OpsCtrl::new(cfg.clone());
        c.start_op(
            naygo_core::ops::transfer(false, vec![src], dst.clone()),
            "Copiar".into(),
            true,
        );
        // Mientras corre, el journal existe en disco.
        let jdir = cfg.join("ops-journal");
        // (puede que ya haya terminado muy rápido; lo importante es que al final no queda)
        drain(&mut c);
        // Tras terminar, el journal se borró.
        let remaining = std::fs::read_dir(&jdir)
            .map(|rd| rd.flatten().count())
            .unwrap_or(0);
        assert_eq!(remaining, 0, "el journal se borra al completar la op");
    }

    #[test]
    fn scan_resume_detecta_journal_pendiente() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        std::fs::create_dir(&cfg).unwrap();
        // Crear un journal manualmente (simula una op interrumpida).
        let src = tmp.path().join("a.txt");
        std::fs::write(&src, b"x").unwrap();
        let dst = tmp.path().join("dst");
        std::fs::create_dir(&dst).unwrap();
        let req = naygo_core::ops::transfer(false, vec![src], dst);
        let plan = naygo_core::ops::plan(&req).unwrap();
        let j = OpJournal::new("op-test".into(), req.kind.clone(), req.conflict, plan);
        let _w = JournalWriter::new(&cfg, j); // persiste al crear
        let mut c = OpsCtrl::new(cfg);
        c.scan_resume();
        assert!(
            matches!(c.pending_dialog, Some(OpDialog::Resume { .. })),
            "scan_resume abre el modal de retomar"
        );
        assert_eq!(c.resume_rows().len(), 1);
    }
}
