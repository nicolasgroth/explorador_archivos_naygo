// Naygo — journal en disco de operaciones para retomar tras un crash (puro + FS).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Persiste el estado de una operación larga en `<config_dir>/ops-journal/<id>.json`
//! a medida que avanza (throttle, best-effort). Al arrancar, `scan` detecta las
//! interrumpidas y `resume_plan` poda el plan a lo pendiente revalidando que los
//! orígenes no cambiaron. El motor de ops-A se reutiliza tal cual.

use super::{ConflictPolicy, OpKind, OpPlan};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Huella de un archivo de origen al planificar, para revalidar al retomar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFingerprint {
    pub len: u64,
    /// Segundos epoch de la fecha de modificación; 0 si no disponible.
    pub mtime_secs: u64,
}

impl FileFingerprint {
    /// Lee la huella de `path`. `None` si no se puede leer la metadata (origen ausente).
    pub fn of(path: &Path) -> Option<FileFingerprint> {
        let meta = std::fs::metadata(path).ok()?;
        let len = meta.len();
        let mtime_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Some(FileFingerprint { len, mtime_secs })
    }
}

/// Estado persistido de una operación en curso.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpJournal {
    pub id: String,
    pub kind: OpKind,
    pub conflict: ConflictPolicy,
    pub plan: OpPlan,
    /// Cantidad de pasos completados: los índices `< done_through` ya están hechos.
    pub done_through: usize,
    /// Huella del origen por paso (alineado por índice). `None` para pasos sin `from`
    /// (crear) o cuando no se pudo leer la metadata al planificar.
    pub source_fingerprints: Vec<Option<FileFingerprint>>,
}

impl OpJournal {
    /// Crea un journal nuevo (done_through=0), calculando la huella de cada origen.
    pub fn new(id: String, kind: OpKind, conflict: ConflictPolicy, plan: OpPlan) -> OpJournal {
        let source_fingerprints = plan
            .steps
            .iter()
            .map(|s| s.from.as_deref().and_then(FileFingerprint::of))
            .collect();
        OpJournal { id, kind, conflict, plan, done_through: 0, source_fingerprints }
    }

    /// Etiqueta corta para mostrar en el modal (kind + destino raíz).
    pub fn label(&self) -> String {
        let dest = self
            .plan
            .steps
            .first()
            .map(|s| s.to.display().to_string())
            .unwrap_or_default();
        let verb = match self.kind {
            OpKind::Copy => "Copiar",
            OpKind::Move => "Mover",
            OpKind::Delete { .. } => "Eliminar",
            OpKind::Rename { .. } => "Renombrar",
            OpKind::CreateDir { .. } | OpKind::CreateFile { .. } => "Crear",
        };
        format!("{verb} → {dest}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::OpStep;
    use std::fs;
    use std::path::PathBuf;

    fn sample_plan(dir: &Path) -> (OpPlan, PathBuf) {
        let src = dir.join("a.txt");
        fs::write(&src, b"hola").unwrap();
        let to = dir.join("dst").join("a.txt");
        let step = OpStep { from: Some(src.clone()), to, bytes: 4, is_dir: false };
        (OpPlan { steps: vec![step], total_bytes: 4, total_files: 1 }, src)
    }

    #[test]
    fn fingerprint_of_archivo_existente() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        fs::write(&f, b"hola").unwrap();
        let fp = FileFingerprint::of(&f).unwrap();
        assert_eq!(fp.len, 4);
    }

    #[test]
    fn fingerprint_of_ausente_es_none() {
        assert!(FileFingerprint::of(Path::new("Z:/no/existe/x.txt")).is_none());
    }

    #[test]
    fn journal_new_calcula_fingerprints() {
        let dir = tempfile::tempdir().unwrap();
        let (plan, _src) = sample_plan(dir.path());
        let j = OpJournal::new("id1".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        assert_eq!(j.done_through, 0);
        assert_eq!(j.source_fingerprints.len(), 1);
        assert_eq!(j.source_fingerprints[0].as_ref().unwrap().len, 4);
    }

    #[test]
    fn journal_round_trip_serde() {
        let dir = tempfile::tempdir().unwrap();
        let (plan, _src) = sample_plan(dir.path());
        let j = OpJournal::new("id1".into(), OpKind::Move, ConflictPolicy::Skip, plan);
        let json = serde_json::to_string(&j).unwrap();
        let back: OpJournal = serde_json::from_str(&json).unwrap();
        assert_eq!(back, j);
    }
}
