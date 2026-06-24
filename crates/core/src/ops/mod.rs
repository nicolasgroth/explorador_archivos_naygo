// Naygo — operaciones de archivo: modelo de tipos (puro, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Tipos que describen una operación de archivo (copiar/mover/eliminar/renombrar/
//! crear), su plan (pasos + bytes), los mensajes del motor (progreso/conflicto/fin)
//! y el resumen. La planificación (`plan`) y la ejecución (`engine`) viven en sus
//! propios submódulos. Todo el modelo es serializable (útil para el journal de ops-B).

pub mod actions;
pub mod engine;
pub mod journal;
pub mod names;
pub mod plan;
pub mod plan_async;
pub mod undo;

pub use actions::{
    batch_rename, create, delete, parse_new_folders, rename, transfer, FolderSpec, NewFolderError,
};
pub use engine::{run_plan, spawn};
pub use journal::{
    journal_path, remove, resume_plan, scan, FileFingerprint, JournalWriter, OpJournal, ResumePlan,
};
pub use names::{dedup_name, is_valid_name};
pub use plan::{plan, PlanError};
pub use plan_async::{spawn_plan, PlanMsg};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Qué operación se pide.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpKind {
    Copy,
    Move,
    Delete {
        to_trash: bool,
    },
    Rename {
        new_name: String,
    },
    /// Renombrar en lote (R3): `new_names[i]` es el nombre nuevo de `sources[i]`.
    /// Cada paso renombra dentro de su propia carpeta. UNA sola op → UNA entrada
    /// en el journal/historial (deshacible como un paso).
    BatchRename {
        new_names: Vec<String>,
    },
    CreateDir {
        name: String,
    },
    CreateFile {
        name: String,
    },
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
    /// Carpetas destino a BORRAR (`remove_dir_all`) antes de copiar, por una decisión
    /// "Reemplazar" en un conflicto de carpeta. El motor las procesa al inicio de `run_plan`
    /// (cancelable, error tipado). Vacío en el caso normal. `#[serde(default)]` para que los
    /// journals viejos (sin este campo) sigan deserializando.
    #[serde(default)]
    pub pre_delete: Vec<PathBuf>,
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

/// Qué hacer cuando una CARPETA de origen ya existe (como carpeta) en el destino. A
/// diferencia de `ConflictAction` (que es por ARCHIVO), esto se decide UNA vez por carpeta,
/// antes de copiar, sobre el árbol completo de ese origen. `Cancel` no es una variante: se
/// modela cancelando la op (igual que cancelar un conflicto por archivo).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FolderDecision {
    /// Copiar dentro de la carpeta existente; los archivos que choquen disparan el conflicto
    /// por archivo (Ask). Es el comportamiento por defecto: no toca el plan.
    Merge,
    /// Borrar la carpeta destino entera (`remove_dir_all`) y copiar limpio. DESTRUCTIVO.
    Replace,
    /// No copiar esa carpeta: se excluyen del plan todos los pasos bajo su destino.
    Skip,
}

/// Un conflicto de carpeta detectado en el pre-check: una carpeta de origen cuyo destino
/// `dest.join(nombre)` YA existe como carpeta. `name` es para mostrar en el diálogo;
/// `dest_root` es la ruta destino exacta a borrar (Replace) o filtrar (Skip).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderConflict {
    /// Nombre de la carpeta (para el texto del diálogo).
    pub name: String,
    /// Origen (la carpeta que se está copiando/moviendo).
    pub source: PathBuf,
    /// Destino exacto que ya existe: `dest_dir.join(name)`.
    pub dest_root: PathBuf,
}

/// Detecta los conflictos de CARPETA de un `OpRequest`: orígenes que son directorios cuyo
/// destino `dest_dir.join(nombre)` ya existe como directorio. Un origen archivo cuyo destino
/// existe NO es un conflicto de carpeta (lo maneja el conflicto por archivo). Solo aplica a
/// Copy/Move; para otros tipos devuelve vacío. Es puro salvo por consultar el FS (igual que
/// `first_collision` en la UI).
pub fn folder_conflicts(req: &OpRequest) -> Vec<FolderConflict> {
    let Some(dest) = req.dest_dir.as_ref() else {
        return Vec::new();
    };
    if !matches!(req.kind, OpKind::Copy | OpKind::Move) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for src in &req.sources {
        if !src.is_dir() {
            continue;
        }
        let Some(name) = src.file_name() else {
            continue;
        };
        let dest_root = dest.join(name);
        if dest_root.is_dir() {
            out.push(FolderConflict {
                name: name.to_string_lossy().into_owned(),
                source: src.clone(),
                dest_root,
            });
        }
    }
    out
}

/// Aplica una `FolderDecision` sobre el conflicto de carpeta cuyo destino es `dest_root`,
/// MUTANDO el plan en su lugar:
/// - `Merge`: no toca nada (el plan ya copia dentro de la existente).
/// - `Skip`: excluye todos los pasos cuyo destino cae bajo `dest_root` (no se copia ese
///   subárbol) y recalcula los totales del plan.
/// - `Replace`: agrega `dest_root` a `plan.pre_delete` (el motor lo borra con `remove_dir_all`
///   ANTES de copiar, de forma cancelable y con error tipado). No quita pasos: tras borrar, la
///   copia repuebla el destino con SOLO el contenido del origen.
///
/// `dest_root` es la ruta destino exacta (`dest_dir.join(nombre)`): solo se filtra/borra ESE
/// destino, nunca el origen ni otra carpeta.
pub fn apply_folder_decision(plan: &mut OpPlan, dest_root: &PathBuf, decision: FolderDecision) {
    match decision {
        FolderDecision::Merge => {}
        FolderDecision::Skip => {
            plan.steps.retain(|s| !s.to.starts_with(dest_root));
            recompute_totals(plan);
        }
        FolderDecision::Replace => {
            if !plan.pre_delete.contains(dest_root) {
                plan.pre_delete.push(dest_root.clone());
            }
        }
    }
}

/// Recalcula `total_bytes`/`total_files` de un plan a partir de sus pasos (tras filtrar).
fn recompute_totals(plan: &mut OpPlan) {
    plan.total_bytes = plan.steps.iter().map(|s| s.bytes).sum();
    plan.total_files = plan.steps.iter().filter(|s| !s.is_dir).count();
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

#[cfg(test)]
mod folder_conflict_tests {
    use super::plan::plan;
    use super::*;
    use std::fs;

    /// Árbol de origen `carpeta/` con un archivo, y un destino que YA tiene una `carpeta/`
    /// (poblada con un archivo extra que NO está en el origen). Devuelve (tempdir, req, dest_root).
    fn escenario_conflicto_carpeta() -> (tempfile::TempDir, OpRequest, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        // Origen: carpeta/ con a.txt.
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("a.txt"), b"origen").unwrap();
        // Destino: dst/ que ya contiene carpeta/ con un archivo EXTRA (solo del destino).
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let dest_root = dest.join("carpeta");
        fs::create_dir(&dest_root).unwrap();
        fs::write(dest_root.join("extra.txt"), b"solo-en-destino").unwrap();
        let req = transfer(false, vec![src], dest);
        (dir, req, dest_root)
    }

    #[test]
    fn folder_conflicts_detecta_carpeta_existente() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let conflicts = folder_conflicts(&req);
        assert_eq!(conflicts.len(), 1, "una carpeta en conflicto");
        assert_eq!(conflicts[0].name, "carpeta");
        assert_eq!(conflicts[0].dest_root, dest_root);
    }

    #[test]
    fn folder_conflicts_ignora_archivo_que_choca() {
        // Un ARCHIVO de origen cuyo destino existe NO es conflicto de carpeta (lo maneja el
        // conflicto por archivo). folder_conflicts debe devolver vacío.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        fs::write(&src, b"nuevo").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("a.txt"), b"viejo").unwrap();
        let req = transfer(false, vec![src], dest);
        assert!(folder_conflicts(&req).is_empty());
    }

    #[test]
    fn folder_conflicts_ignora_carpeta_nueva() {
        // El destino no tiene la carpeta → no hay conflicto.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("carpeta");
        fs::create_dir(&src).unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = transfer(false, vec![src], dest);
        assert!(folder_conflicts(&req).is_empty());
    }

    #[test]
    fn merge_no_toca_el_plan() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let mut p = plan(&req).unwrap();
        let antes = p.clone();
        apply_folder_decision(&mut p, &dest_root, FolderDecision::Merge);
        assert_eq!(p, antes, "Merge deja el plan idéntico");
        assert!(p.pre_delete.is_empty());
    }

    #[test]
    fn skip_excluye_los_pasos_bajo_la_carpeta() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let mut p = plan(&req).unwrap();
        // Antes: hay pasos cuyo destino cae bajo dest_root (la carpeta y su a.txt).
        assert!(p.steps.iter().any(|s| s.to.starts_with(&dest_root)));
        let bytes_archivo = p.total_bytes; // "origen" = 6 bytes
        assert!(bytes_archivo > 0);

        apply_folder_decision(&mut p, &dest_root, FolderDecision::Skip);

        // Después: NINGÚN paso cae bajo dest_root y los totales se recalcularon a cero.
        assert!(
            !p.steps.iter().any(|s| s.to.starts_with(&dest_root)),
            "Skip excluye todo el subárbol de la carpeta"
        );
        assert_eq!(p.total_bytes, 0);
        assert_eq!(p.total_files, 0);
        assert!(p.pre_delete.is_empty(), "Skip no borra nada");
    }

    #[test]
    fn skip_no_toca_otros_origenes() {
        // Dos carpetas a copiar; solo una choca. Skip de la que choca NO debe quitar los pasos
        // de la otra.
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("x.txt"), b"xx").unwrap();
        fs::write(b.join("y.txt"), b"yyy").unwrap();
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        // Solo "a" existe en el destino.
        fs::create_dir(dest.join("a")).unwrap();
        let dest_root_a = dest.join("a");
        let req = transfer(false, vec![a, b], dest.clone());
        let mut p = plan(&req).unwrap();

        apply_folder_decision(&mut p, &dest_root_a, FolderDecision::Skip);

        // Los pasos de "b" siguen; los de "a" se fueron.
        assert!(!p.steps.iter().any(|s| s.to.starts_with(&dest_root_a)));
        assert!(p
            .steps
            .iter()
            .any(|s| s.to.starts_with(dest.join("b").as_path())));
        // total_files quedó en 1 (solo y.txt).
        assert_eq!(p.total_files, 1);
        assert_eq!(p.total_bytes, 3);
    }

    #[test]
    fn replace_agrega_el_borrado_del_destino() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let mut p = plan(&req).unwrap();
        let pasos_antes = p.steps.clone();

        apply_folder_decision(&mut p, &dest_root, FolderDecision::Replace);

        // Replace agrega EXACTAMENTE dest_root al pre_delete; NO quita pasos de copia.
        assert_eq!(p.pre_delete, vec![dest_root]);
        assert_eq!(p.steps, pasos_antes, "Replace no filtra pasos");
    }

    #[test]
    fn replace_idempotente_no_duplica() {
        let (_d, req, dest_root) = escenario_conflicto_carpeta();
        let mut p = plan(&req).unwrap();
        apply_folder_decision(&mut p, &dest_root, FolderDecision::Replace);
        apply_folder_decision(&mut p, &dest_root, FolderDecision::Replace);
        assert_eq!(p.pre_delete, vec![dest_root], "no se duplica el destino");
    }
}
