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
        /// Id ESTABLE de la op que espera la decisión (no su posición en `active_ops`). El
        /// vector se puede reordenar mientras el modal está abierto, así que guardar el id
        /// garantiza que la resolución vaya a la op correcta.
        op_id: u64,
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
    /// Id estable y único en la sesión. NO cambia aunque el vector `active_ops` se reordene
    /// (poda de terminadas, avance de la cola). Es la clave con la que la UI identifica una
    /// op: los botones Pausar/Reanudar/Saltar/Cancelar resuelven la op por este id, no por su
    /// posición en el vector (que es volátil entre el render y el clic del usuario).
    pub id: u64,
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
    /// Cuántos orígenes se omitieron al RETOMAR porque cambiaron/desaparecieron desde el
    /// journal (0 en una op normal). Se muestra en el estado para no perderlos en silencio.
    pub resume_skipped: usize,
    /// Instante del primer progreso recibido (arranque efectivo de la transferencia). Se usa
    /// para el transcurrido, la velocidad media y la ETA. `None` hasta el primer `Progress`.
    pub started_at: Option<std::time::Instant>,
    /// Última muestra (instante, bytes_done) para derivar la velocidad instantánea entre
    /// dos `Progress` y, de ahí, el pico.
    pub last_sample: Option<(std::time::Instant, u64)>,
    /// Velocidad pico observada (bytes/s): el máximo de las velocidades instantáneas.
    pub peak_speed: u64,
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
    /// Secuencia para ids ESTABLES de operación (campo `ActiveOp::id`). A diferencia de la
    /// posición en `active_ops`, este id no cambia al reordenar el vector, así que es la clave
    /// segura para que los botones del panel afecten siempre la op correcta.
    next_op_id: u64,
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
            next_op_id: 1,
        }
    }

    /// Reserva el próximo id estable de operación (monótono, único en la sesión).
    fn alloc_op_id(&mut self) -> u64 {
        let id = self.next_op_id;
        self.next_op_id += 1;
        id
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
            let id = self.alloc_op_id();
            self.active_ops.push(ActiveOp {
                id,
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
                resume_skipped: 0,
                started_at: None,
                last_sample: None,
                peak_speed: 0,
            });
            return;
        }

        let id = self.alloc_op_id();
        self.spawn_op(
            id,
            plan,
            req.kind.clone(),
            conflict,
            label,
            record_undo.then_some(req),
        );
    }

    /// Spawnea el motor para un plan ya resuelto y agrega la `ActiveOp`. Crea un journal
    /// (para retomar tras un cierre inesperado) en Copy/Move/Delete-permanente.
    ///
    /// `id` es el id ESTABLE de la op: al lanzar una op nueva, el llamador reserva uno fresco
    /// con `alloc_op_id()`; al promover una op desde la cola, pasa el id del placeholder para
    /// CONSERVARLO (el usuario que la veía en cola la sigue refiriendo igual tras arrancar).
    fn spawn_op(
        &mut self,
        id: u64,
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
            id,
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
            resume_skipped: 0,
            started_at: None,
            last_sample: None,
            peak_speed: 0,
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
                let now = std::time::Instant::now();
                let op = &mut self.active_ops[i];
                // Primer progreso = arranque efectivo de la transferencia.
                if op.started_at.is_none() {
                    op.started_at = Some(now);
                }
                // Velocidad instantánea = Δbytes / Δt respecto de la última muestra; el pico
                // es el máximo de todas. (La media y la ETA se derivan al vuelo en op_rows.)
                if let Some((prev_t, prev_bytes)) = op.last_sample {
                    let dt = now.duration_since(prev_t).as_secs_f64();
                    if dt > 0.0 && p.bytes_done >= prev_bytes {
                        let inst = ((p.bytes_done - prev_bytes) as f64 / dt) as u64;
                        op.peak_speed = op.peak_speed.max(inst);
                    }
                }
                op.last_sample = Some((now, p.bytes_done));
                op.progress = Some(p);
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
                let op_id = self.active_ops[idx].id;
                self.pending_dialog = Some(OpDialog::Conflict { op_id, prompt });
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
                // CONSERVAR el id estable del placeholder: la op que el usuario veía en cola debe
                // seguir refiriéndose con el mismo id al arrancar (si no, un clic en su botón ya
                // no la encontraría). Por eso se reusa `id` en vez de reservar uno nuevo.
                let id = self.active_ops[idx].id;
                // Quitar el placeholder en cola y spawnear de verdad.
                self.active_ops.remove(idx);
                self.spawn_op(id, plan, kind, conflict, label, request);
            }
        }

        self.active_ops.iter().all(|o| o.rx.is_none())
    }

    /// Resuelve el conflicto pendiente de la op identificada por `op_id` (id ESTABLE, no
    /// posición) con la acción dada. Se busca por id porque el vector se puede haber reordenado
    /// mientras el modal estaba abierto; resolver por posición mandaría la decisión a otra op.
    pub fn resolve_conflict(&mut self, op_id: u64, action: ConflictAction, apply_all: bool) {
        if let Some(op) = self.active_ops.iter_mut().find(|o| o.id == op_id) {
            let _ = op.conflict_tx.send(ConflictDecision { action, apply_all });
            op.awaiting_conflict = None;
        }
        self.pending_dialog = None;
    }

    /// Cancela TODA la operación identificada por `op_id` desde el diálogo de conflicto, SIN
    /// decidir el choque pendiente. Espeja `cancel_op` (cancela el token) más la limpieza que
    /// hace `resolve_conflict` (borra `awaiting_conflict` y cierra el modal), pero NO envía una
    /// `ConflictDecision`. El worker está bloqueado esperando la decisión con
    /// `recv_timeout(50ms)` y, entre timeouts, revisa el token (ver `engine::exec_copy_step`):
    /// al cancelarlo despierta dentro de ~50ms y aborta limpio (devuelve `Skipped`). Por eso
    /// basta cancelar el token; no hace falta cerrar el sender ni mandar una variante especial.
    /// Resuelve por id ESTABLE (no posición) por la misma razón que `cancel_op`/`resolve_conflict`.
    pub fn cancel_conflict(&mut self, op_id: u64) {
        if let Some(op) = self.active_ops.iter_mut().find(|o| o.id == op_id) {
            op.token.cancel();
            op.awaiting_conflict = None;
        }
        self.pending_dialog = None;
    }

    /// Cancela la op identificada por `op_id` (id ESTABLE de `ActiveOp`, no posición). Se
    /// resuelve por id porque el vector `active_ops` se reordena entre el render del panel y
    /// el clic del usuario (poda de terminadas, avance de la cola): un índice posicional
    /// cancelaría otra op real. Si la op ya no existe, no hace nada.
    pub fn cancel_op(&mut self, op_id: i32) {
        if let Some(op) = self.active_ops.iter().find(|o| o.id as i32 == op_id) {
            op.token.cancel();
        }
    }

    /// Pausa la op identificada por `op_id` (el motor se detiene en el siguiente
    /// `wait_if_paused`). Se resuelve por id estable, no por posición (ver `cancel_op`).
    pub fn pause_op(&mut self, op_id: i32) {
        if let Some(op) = self.active_ops.iter().find(|o| o.id as i32 == op_id) {
            op.token.pause();
        }
    }

    /// Reanuda la op identificada por `op_id` si estaba pausada. Se resuelve por id estable,
    /// no por posición (ver `cancel_op`).
    pub fn resume_op(&mut self, op_id: i32) {
        if let Some(op) = self.active_ops.iter().find(|o| o.id as i32 == op_id) {
            op.token.resume();
        }
    }

    /// Saltar el archivo en curso en vivo: PENDIENTE. El motor no soporta hoy abortar el
    /// archivo actual a mitad de copia y continuar con el siguiente (el `CancellationToken`
    /// solo cancela la op entera). Implementarlo bien requiere soporte del motor (una señal
    /// "skip-current" análoga a pause/cancel). Se deja como no-op para no introducir un
    /// mecanismo frágil; la UI puede ofrecerlo deshabilitado hasta entonces. La firma recibe
    /// el id estable por consistencia con los demás handlers.
    pub fn skip_op(&mut self, _op_id: i32) {
        // No-op intencional: ver el comentario de arriba.
    }

    /// Tope de operaciones terminadas que se conservan como historial reciente.
    const HISTORY_CAP: usize = 20;

    /// Poda las ops terminadas viejas, conservando como historial las `HISTORY_CAP` más
    /// recientes (con su resumen). Las en curso y las en cola se conservan siempre. El
    /// llamador decide cuándo podar (al iniciar una op nueva).
    pub fn prune_finished(&mut self) {
        // Una op cuenta como "terminada" (historial) si ya no tiene canal y arrancó alguna vez.
        let is_finished = |o: &ActiveOp| o.rx.is_none() && o.started && o.pending.is_none();
        let finished_total = self.active_ops.iter().filter(|o| is_finished(o)).count();
        if finished_total <= Self::HISTORY_CAP {
            return;
        }
        // Hay que descartar las más viejas: se conservan las últimas HISTORY_CAP terminadas
        // (las del final del vector, que es el orden de llegada) y todas las activas/en cola.
        let mut to_drop = finished_total - Self::HISTORY_CAP;
        self.active_ops.retain(|o| {
            if is_finished(o) && to_drop > 0 {
                to_drop -= 1;
                false
            } else {
                true
            }
        });
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

    /// Filas del panel de progreso (una por op activa, en cola o terminada/historial).
    /// Calcula al vuelo el transcurrido, la velocidad media (bytes_done/elapsed), la velocidad
    /// pico (acumulada en el poll) y la ETA (bytes restantes / velocidad media). Los tamaños se
    /// formatean con `SizeFormat::Auto` (el OpsCtrl no tiene acceso a Settings).
    pub fn op_rows(&self) -> Vec<OpRowData> {
        use naygo_core::format::{format_duration, format_size, format_speed, SizeFormat};
        const FMT: SizeFormat = SizeFormat::Auto;
        self.active_ops
            .iter()
            .map(|o| {
                let running = o.rx.is_some();
                let in_queue = !o.started;
                let paused = o.token.is_paused();

                let (bytes_done, bytes_total, files_done, files_total, current_file) =
                    match &o.progress {
                        Some(p) => (
                            p.bytes_done,
                            p.bytes_total,
                            p.files_done as i32,
                            p.files_total as i32,
                            p.current
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        ),
                        None => (0, 0, 0, 0, String::new()),
                    };

                let percent = if bytes_total > 0 {
                    (bytes_done as f32 / bytes_total as f32) * 100.0
                } else if o.summary.is_some() {
                    100.0
                } else {
                    0.0
                };

                // Transcurrido + velocidad media + ETA, derivados de `started_at`.
                let elapsed_secs = o
                    .started_at
                    .map(|t| t.elapsed().as_secs_f64())
                    .unwrap_or(0.0);
                let avg_speed = if elapsed_secs > 0.0 {
                    (bytes_done as f64 / elapsed_secs) as u64
                } else {
                    0
                };
                let eta = if avg_speed > 0 && bytes_total > bytes_done {
                    format_duration((bytes_total - bytes_done) / avg_speed)
                } else {
                    String::new()
                };

                // 0=en curso 1=en cola 2=historial (terminada con resumen).
                let kind = if o.summary.is_some() {
                    2
                } else if in_queue {
                    1
                } else {
                    0
                };

                let status = if in_queue {
                    "en cola".to_string()
                } else if let Some(s) = &o.summary {
                    let mut t = format!(
                        "hecho: {} copiados, {} saltados, {} fallidos",
                        s.count_done(),
                        s.count_skipped(),
                        s.count_failed()
                    );
                    // Al RETOMAR: avisar los orígenes que cambiaron/desaparecieron y se omitieron
                    // (antes se descartaban en silencio).
                    if o.resume_skipped > 0 {
                        t.push_str(&format!(
                            " · {} omitidos por cambios al retomar",
                            o.resume_skipped
                        ));
                    }
                    t
                } else if paused {
                    "pausada".to_string()
                } else if o.awaiting_conflict.is_some() {
                    "esperando decisión…".to_string()
                } else {
                    "en curso…".to_string()
                };

                OpRowData {
                    // `index` lleva el id ESTABLE de la op (no su posición): la UI lo guarda y lo
                    // devuelve a los handlers, que resuelven la op por id. Así un clic siempre
                    // afecta a la op correcta aunque el vector se reordene entre render y clic.
                    index: o.id as i32,
                    label: o.label.clone(),
                    percent,
                    status,
                    running,
                    paused,
                    bytes_done: format_size(bytes_done, FMT),
                    bytes_total: format_size(bytes_total, FMT),
                    files_done,
                    files_total,
                    current_file,
                    speed: format_speed(avg_speed),
                    speed_peak: format_speed(o.peak_speed),
                    eta,
                    elapsed: format_duration(elapsed_secs as u64),
                    kind,
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
            resume_skipped,
            started_at: None,
            last_sample: None,
            peak_speed: 0,
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
/// Los campos de tamaño/velocidad/tiempo vienen ya formateados como String (listos para la UI).
#[derive(Clone, Debug)]
pub struct OpRowData {
    pub index: i32,
    pub label: String,
    pub percent: f32,
    pub status: String,
    pub running: bool,
    pub paused: bool,
    pub bytes_done: String,
    pub bytes_total: String,
    pub files_done: i32,
    pub files_total: i32,
    pub current_file: String,
    pub speed: String,
    pub speed_peak: String,
    pub eta: String,
    pub elapsed: String,
    /// 0=en curso 1=en cola 2=historial.
    pub kind: i32,
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
                if let Some(OpDialog::Conflict { op_id, .. }) = c.pending_dialog.clone() {
                    c.resolve_conflict(op_id, ConflictAction::Overwrite, false);
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

    /// Construye una `ActiveOp` mínima "en curso" con el id dado, sin tocar el motor. Sirve para
    /// probar la resolución por id tras reordenar el vector, aislada de canales/hilos reales.
    fn fake_active_op(id: u64) -> ActiveOp {
        let (conflict_tx, _crx) = std::sync::mpsc::channel::<ConflictDecision>();
        let (_dummy_tx, rx) = std::sync::mpsc::channel::<OpMsg>();
        ActiveOp {
            id,
            rx: Some(rx),
            conflict_tx,
            token: CancellationToken::new(),
            label: format!("op {id}"),
            progress: None,
            summary: None,
            started: true,
            pending: None,
            journal_id: None,
            request: None,
            awaiting_conflict: None,
            resume_skipped: 0,
            started_at: None,
            last_sample: None,
            peak_speed: 0,
        }
    }

    #[test]
    fn cancelar_por_id_afecta_la_op_correcta_tras_reordenar() {
        // El bug original: los botones del panel resolvían la op por su POSICIÓN en `active_ops`.
        // Si el vector se reordena entre el render y el clic (poda de terminadas, avance de la
        // cola), cancelar la op N cancelaba otra transferencia real. Con id estable, el clic
        // siempre va a la op correcta.
        let mut c = OpsCtrl::new(std::env::temp_dir());
        // Tres ops con ids 10, 20, 30 en las posiciones 0, 1, 2.
        c.active_ops.push(fake_active_op(10));
        c.active_ops.push(fake_active_op(20));
        c.active_ops.push(fake_active_op(30));
        // Clonar los tokens (comparten estado por Arc) para verificar después del reordenamiento.
        let tok10 = c.active_ops[0].token.clone();
        let tok20 = c.active_ops[1].token.clone();
        let tok30 = c.active_ops[2].token.clone();

        // Reordenar: quitar la primera (como hace `prune_finished`/`pump_ops`). Ahora la op 20
        // está en la posición 0 y la 30 en la 1. Un índice posicional viejo apuntaría mal.
        c.active_ops.remove(0);
        assert_eq!(c.active_ops[0].id, 20);
        assert_eq!(c.active_ops[1].id, 30);

        // Cancelar la op 30 POR ID: debe cancelar la 30 y solo la 30.
        c.cancel_op(30);
        assert!(tok30.is_cancelled(), "cancela la op 30 (la pedida)");
        assert!(
            !tok20.is_cancelled(),
            "NO toca la op 20 (vecina tras reordenar)"
        );
        assert!(
            !tok10.is_cancelled(),
            "NO toca la op 10 (ya quitada del vector)"
        );

        // Pausar la op 20 POR ID: debe pausar la 20 y solo la 20.
        c.pause_op(20);
        assert!(tok20.is_paused(), "pausa la op 20 (la pedida)");
        assert!(!tok30.is_paused(), "NO pausa la op 30");

        // Reanudar la op 20 POR ID.
        c.resume_op(20);
        assert!(!tok20.is_paused(), "reanuda la op 20");
    }

    #[test]
    fn cancelar_id_inexistente_no_hace_nada() {
        // Un id que ya no existe (op terminada y podada) es un no-op silencioso, sin pánico.
        let mut c = OpsCtrl::new(std::env::temp_dir());
        c.active_ops.push(fake_active_op(7));
        let tok = c.active_ops[0].token.clone();
        c.cancel_op(999);
        assert!(
            !tok.is_cancelled(),
            "un id desconocido no cancela ninguna op"
        );
    }

    #[test]
    fn resolve_conflict_por_id_va_a_la_op_correcta_tras_reordenar() {
        // El conflicto guarda el id estable de la op en el modal; resolverlo debe mandar la
        // decisión a ESA op aunque el vector se haya reordenado mientras el modal estaba abierto.
        let mut c = OpsCtrl::new(std::env::temp_dir());
        let mut op_a = fake_active_op(100);
        let mut op_b = fake_active_op(200);
        // Ambas esperan decisión; capturar el receptor de la decisión de cada una.
        let (tx_a, rx_a) = std::sync::mpsc::channel::<ConflictDecision>();
        let (tx_b, rx_b) = std::sync::mpsc::channel::<ConflictDecision>();
        op_a.conflict_tx = tx_a;
        op_b.conflict_tx = tx_b;
        op_a.awaiting_conflict = None;
        op_b.awaiting_conflict = None;
        c.active_ops.push(op_a);
        c.active_ops.push(op_b);

        // Reordenar: quitar la primera. La op 200 queda en la posición 0.
        c.active_ops.remove(0);
        assert_eq!(c.active_ops[0].id, 200);

        // Resolver el conflicto de la op 200 POR ID.
        c.resolve_conflict(200, ConflictAction::Overwrite, false);
        // La op 200 recibió su decisión; la 100 (ya quitada) no recibió nada por su canal.
        assert!(rx_b.try_recv().is_ok(), "la op 200 recibe la decisión");
        assert!(
            rx_a.try_recv().is_err(),
            "la op 100 NO recibe decisión ajena"
        );
    }

    #[test]
    fn cancel_conflict_cancela_el_token_sin_decidir() {
        // Cancelar toda la operación desde el modal de conflicto: el token de ESA op queda
        // cancelado (el worker bloqueado en `recv_timeout` lo verá y abortará), el conflicto
        // pendiente se limpia y el modal se cierra. Crucial: NO se envía ninguna `ConflictDecision`
        // por el canal (cancelar ≠ decidir el choque).
        let mut c = OpsCtrl::new(std::env::temp_dir());
        let mut op = fake_active_op(42);
        let (tx, rx) = std::sync::mpsc::channel::<ConflictDecision>();
        op.conflict_tx = tx;
        op.awaiting_conflict = Some(ConflictPrompt {
            existing: PathBuf::from("C:/dst/a.txt"),
            incoming: PathBuf::from("C:/src/a.txt"),
        });
        let token = op.token.clone();
        c.active_ops.push(op);
        // El modal de conflicto está abierto apuntando a esta op.
        c.pending_dialog = Some(OpDialog::Conflict {
            op_id: 42,
            prompt: ConflictPrompt {
                existing: PathBuf::from("C:/dst/a.txt"),
                incoming: PathBuf::from("C:/src/a.txt"),
            },
        });

        c.cancel_conflict(42);

        assert!(token.is_cancelled(), "el token de la op quedó cancelado");
        assert!(
            c.active_ops[0].awaiting_conflict.is_none(),
            "el conflicto pendiente se limpió"
        );
        assert!(c.pending_dialog.is_none(), "el modal se cerró");
        assert!(
            rx.try_recv().is_err(),
            "NO se envió ninguna decisión por el canal (cancelar no decide el choque)"
        );
    }

    #[test]
    fn cancel_conflict_id_inexistente_solo_cierra_el_modal() {
        // Un id que ya no existe (op terminada/podada) no debe entrar en pánico: solo cierra el
        // modal sin tocar otras ops.
        let mut c = OpsCtrl::new(std::env::temp_dir());
        c.active_ops.push(fake_active_op(7));
        let tok = c.active_ops[0].token.clone();
        c.pending_dialog = Some(OpDialog::Conflict {
            op_id: 999,
            prompt: ConflictPrompt {
                existing: PathBuf::from("C:/x/a.txt"),
                incoming: PathBuf::from("C:/y/a.txt"),
            },
        });
        c.cancel_conflict(999);
        assert!(!tok.is_cancelled(), "no toca la op 7 (id distinto)");
        assert!(c.pending_dialog.is_none(), "igual cierra el modal");
    }

    #[test]
    fn id_estable_se_conserva_al_avanzar_la_cola() {
        // En modo cola, la 2ª op queda como placeholder con un id. Al terminar la 1ª y avanzar
        // la cola (`pump_ops` la respawnea), el placeholder debe CONSERVAR su id: el usuario que
        // la veía en cola la sigue refiriendo con el mismo id en el panel.
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
        // El id del placeholder en cola (la op que aún no arrancó).
        let queued_id = c
            .active_ops
            .iter()
            .find(|o| !o.started)
            .map(|o| o.id)
            .expect("hay una op en cola");
        drain(&mut c);
        // Tras avanzar la cola, debe seguir existiendo una op con ESE mismo id (la que arrancó).
        assert!(
            c.active_ops.iter().any(|o| o.id == queued_id),
            "el id del placeholder se conserva al respawnear desde la cola"
        );
        assert!(d2.join("b.txt").exists(), "la 2ª op se completó");
    }
}
