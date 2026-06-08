// Naygo — operaciones de archivo: modelo de tipos (puro, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Tipos que describen una operación de archivo (copiar/mover/eliminar/renombrar/
//! crear), su plan (pasos + bytes), los mensajes del motor (progreso/conflicto/fin)
//! y el resumen. La planificación (`plan`) y la ejecución (`engine`) viven en sus
//! propios submódulos. Todo el modelo es serializable (útil para el journal de ops-B).

pub mod engine;
pub mod journal;
pub mod names;
pub mod plan;

pub use engine::{run_plan, spawn};
pub use journal::{journal_path, remove, scan, FileFingerprint, JournalWriter, OpJournal};
pub use names::{dedup_name, is_valid_name};
pub use plan::{plan, PlanError};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Qué operación se pide.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpKind {
    Copy,
    Move,
    Delete { to_trash: bool },
    Rename { new_name: String },
    CreateDir { name: String },
    CreateFile { name: String },
}

/// Qué hacer ante un nombre que ya existe en el destino.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictPolicy {
    /// Preguntar a la UI (emite `OpMsg::Conflict` y espera la decisión).
    Ask,
    Overwrite,
    Skip,
    Rename,
}

/// Solicitud de operación, armada por la UI desde la selección.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpRequest {
    pub kind: OpKind,
    pub sources: Vec<PathBuf>,
    /// Carpeta destino (Copy/Move). `None` para Delete/Rename/Create.
    pub dest_dir: Option<PathBuf>,
    pub conflict: ConflictPolicy,
}

/// Un paso concreto del plan: copiar/mover un archivo, o crear una carpeta.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpStep {
    /// Origen (None para crear carpeta vacía en el destino).
    pub from: Option<PathBuf>,
    pub to: PathBuf,
    pub bytes: u64,
    pub is_dir: bool,
}

/// Plan completo de una operación: pasos + totales.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpPlan {
    pub steps: Vec<OpStep>,
    pub total_bytes: u64,
    pub total_files: usize,
}

/// Progreso emitido por el motor mientras trabaja.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpProgress {
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub files_done: usize,
    pub files_total: usize,
    pub current: PathBuf,
}

/// Petición de decisión de conflicto que el motor manda a la UI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConflictPrompt {
    pub existing: PathBuf,
    pub incoming: PathBuf,
}

/// Decisión que la UI devuelve al motor ante un conflicto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictDecision {
    pub action: ConflictAction,
    /// Aplicar esta decisión a todos los conflictos siguientes de la op.
    pub apply_all: bool,
}

/// Acción concreta de un conflicto resuelto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictAction {
    Overwrite,
    Skip,
    Rename,
}

/// Resultado por archivo, para el resumen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OpOutcome {
    Done,
    Skipped,
    Failed(String),
}

/// Resumen de una operación terminada (o cancelada).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpSummary {
    /// (ruta destino, resultado) por archivo procesado.
    pub items: Vec<(PathBuf, OpOutcome)>,
    pub bytes_done: u64,
    pub elapsed_secs: f64,
}

impl OpSummary {
    pub fn count_done(&self) -> usize {
        self.items
            .iter()
            .filter(|(_, o)| matches!(o, OpOutcome::Done))
            .count()
    }
    pub fn count_skipped(&self) -> usize {
        self.items
            .iter()
            .filter(|(_, o)| matches!(o, OpOutcome::Skipped))
            .count()
    }
    pub fn count_failed(&self) -> usize {
        self.items
            .iter()
            .filter(|(_, o)| matches!(o, OpOutcome::Failed(_)))
            .count()
    }
}

/// Mensajes que el motor emite hacia la UI por el canal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OpMsg {
    Progress(OpProgress),
    Conflict(ConflictPrompt),
    Done(OpSummary),
    Cancelled(OpSummary),
    /// Error fatal que impide siquiera empezar (p. ej. plan inválido).
    Failed(String),
}
