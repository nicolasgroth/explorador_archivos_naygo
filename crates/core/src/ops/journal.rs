// Naygo — journal en disco de operaciones para retomar tras un crash (puro + FS).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Persiste el estado de una operación larga en `<config_dir>/ops-journal/<id>.json`
//! a medida que avanza (throttle, best-effort). Al arrancar, `scan` detecta las
//! interrumpidas y `resume_plan` poda el plan a lo pendiente revalidando que los
//! orígenes no cambiaron. El motor de ops-A se reutiliza tal cual.

use super::{ConflictPolicy, OpKind, OpPlan};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

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

/// Intervalo mínimo entre escrituras del journal (throttle).
const THROTTLE: std::time::Duration = std::time::Duration::from_millis(500);

/// Ruta del journal de la operación `id`.
pub fn journal_path(config_dir: &Path, id: &str) -> PathBuf {
    config_dir.join("ops-journal").join(format!("{id}.json"))
}

/// Borra el journal de `id` (al completar/descartar). Best-effort.
pub fn remove(config_dir: &Path, id: &str) {
    let _ = std::fs::remove_file(journal_path(config_dir, id));
}

/// Lee todas las operaciones interrumpidas de `<config_dir>/ops-journal/*.json`.
/// Ignora archivos corruptos. Orden: por id (estable).
pub fn scan(config_dir: &Path) -> Vec<OpJournal> {
    let jdir = config_dir.join("ops-journal");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&jdir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(txt) = std::fs::read_to_string(&path) {
                    if let Ok(j) = serde_json::from_str::<OpJournal>(&txt) {
                        out.push(j);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Escribe el journal a disco con throttle. Best-effort: si el write falla, no
/// propaga (la operación no debe romperse por el journal).
pub struct JournalWriter {
    config_dir: PathBuf,
    journal: OpJournal,
    last_write: Option<Instant>,
}

impl JournalWriter {
    /// Crea el escritor y persiste el journal inicial inmediatamente.
    pub fn new(config_dir: &Path, journal: OpJournal) -> JournalWriter {
        let w = JournalWriter {
            config_dir: config_dir.to_path_buf(),
            journal,
            last_write: None,
        };
        w.persist();
        w
    }

    /// Actualiza `done_through` y persiste si pasó el throttle (o es el primero).
    pub fn record(&mut self, done_through: usize, now: Instant) {
        self.journal.done_through = done_through;
        let due = match self.last_write {
            None => true,
            Some(prev) => now.duration_since(prev) >= THROTTLE,
        };
        if due {
            self.persist();
            self.last_write = Some(now);
        }
    }

    /// Fuerza una escritura (al terminar la operación).
    pub fn flush(&mut self) {
        self.persist();
    }

    /// El id del journal (para borrarlo al terminar).
    pub fn id(&self) -> &str {
        &self.journal.id
    }

    fn persist(&self) {
        let path = journal_path(&self.config_dir, &self.journal.id);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(txt) = serde_json::to_string(&self.journal) {
            let _ = std::fs::write(&path, txt);
        }
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

    use std::time::{Duration, Instant};

    #[test]
    fn writer_primer_record_escribe() {
        let dir = tempfile::tempdir().unwrap();
        let (plan, _s) = sample_plan(dir.path());
        let j = OpJournal::new("w1".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        let mut w = JournalWriter::new(dir.path(), j);
        let now = Instant::now();
        w.record(1, now);
        let path = journal_path(dir.path(), "w1");
        let txt = fs::read_to_string(&path).unwrap();
        let back: OpJournal = serde_json::from_str(&txt).unwrap();
        assert_eq!(back.done_through, 1);
    }

    #[test]
    fn writer_throttle_no_reescribe_dentro_del_umbral() {
        let dir = tempfile::tempdir().unwrap();
        let (plan, _s) = sample_plan(dir.path());
        let j = OpJournal::new("w2".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        let mut w = JournalWriter::new(dir.path(), j);
        let t0 = Instant::now();
        w.record(1, t0);
        w.record(2, t0 + Duration::from_millis(100));
        let back: OpJournal =
            serde_json::from_str(&fs::read_to_string(journal_path(dir.path(), "w2")).unwrap()).unwrap();
        assert_eq!(back.done_through, 1, "el 2º record dentro del umbral no se persiste");
        w.record(3, t0 + Duration::from_millis(600));
        let back2: OpJournal =
            serde_json::from_str(&fs::read_to_string(journal_path(dir.path(), "w2")).unwrap()).unwrap();
        assert_eq!(back2.done_through, 3);
    }

    #[test]
    fn writer_flush_fuerza_persistencia() {
        let dir = tempfile::tempdir().unwrap();
        let (plan, _s) = sample_plan(dir.path());
        let j = OpJournal::new("w3".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        let mut w = JournalWriter::new(dir.path(), j);
        let t0 = Instant::now();
        w.record(1, t0);
        w.record(2, t0 + Duration::from_millis(50));
        w.flush();
        let back: OpJournal =
            serde_json::from_str(&fs::read_to_string(journal_path(dir.path(), "w3")).unwrap()).unwrap();
        assert_eq!(back.done_through, 2);
    }

    #[test]
    fn scan_lee_journals_e_ignora_corruptos() {
        let dir = tempfile::tempdir().unwrap();
        let jdir = dir.path().join("ops-journal");
        fs::create_dir_all(&jdir).unwrap();
        let (plan, _s) = sample_plan(dir.path());
        let j = OpJournal::new("ok".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        fs::write(jdir.join("ok.json"), serde_json::to_string(&j).unwrap()).unwrap();
        fs::write(jdir.join("bad.json"), b"{ no es json").unwrap();
        let found = scan(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "ok");
    }

    #[test]
    fn remove_borra_el_journal() {
        let dir = tempfile::tempdir().unwrap();
        let (plan, _s) = sample_plan(dir.path());
        let j = OpJournal::new("r1".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        let mut w = JournalWriter::new(dir.path(), j);
        w.record(1, Instant::now());
        assert!(journal_path(dir.path(), "r1").exists());
        remove(dir.path(), "r1");
        assert!(!journal_path(dir.path(), "r1").exists());
    }
}
