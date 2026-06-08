# ops-B — Journal en disco + retomar tras crash — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persistir un journal de cada operación larga en curso para, tras un crash, ofrecer al reabrir Naygo retomar desde donde quedó (con revalidación estricta) o descartar — reutilizando el motor de ops-A.

**Architecture:** `core::ops::journal` define el journal serializable (`OpJournal` sobre el `OpPlan` ya serializable), un `JournalWriter` con throttle (best-effort, no frena la op), `scan` (detección al arrancar) y `resume_plan` (poda a pendientes + revalidación de huellas, puro). El motor de ops-A gana un parámetro opcional de journal. La UI muestra un modal al arrancar y dispara retomar (= ejecutar el plan podado) o descartar.

**Tech Stack:** Rust, `naygo-core` / `naygo-ui`, `eframe`/`egui` 0.34.3, `serde`/`serde_json`, `std::time`. `tempfile` (dev) para tests. Sin dependencias de terceros nuevas.

**Estado de partida (rama base `feat/ops-b`, desde `main` con ops-A):**
- `naygo_core::ops::mod`: `OpKind`, `ConflictPolicy`, `OpRequest`, `OpStep { from: Option<PathBuf>, to: PathBuf, bytes: u64, is_dir: bool }`, `OpPlan { steps: Vec<OpStep>, total_bytes: u64, total_files: usize }`, `OpProgress`, `OpSummary { items, bytes_done, elapsed_secs }`, `OpOutcome`, `OpMsg`, `ConflictDecision`. TODOS `Serialize`/`Deserialize` (excepto `PlanError`). `plan(&OpRequest) -> Result<OpPlan, PlanError>`. `dedup_name`. submódulos `mod.rs`/`names.rs`/`plan.rs`/`engine.rs`.
- `naygo_core::ops::engine`:
  - `pub fn spawn(plan: OpPlan, kind: OpKind, conflict: ConflictPolicy, token: CancellationToken, conflict_rx: Receiver<ConflictDecision>) -> (Receiver<OpMsg>, JoinHandle<()>)` — spawnea hilo, llama `run_plan`, envía `Done`/`Cancelled` final.
  - `pub fn run_plan(plan: &OpPlan, kind: &OpKind, conflict: ConflictPolicy, token: &CancellationToken, tx: &Sender<OpMsg>, _conflict_rx: &Receiver<ConflictDecision>) -> OpSummary` — loop `for step in &plan.steps` con `files_done` (NO trackea índice de paso hoy — esta fase añade `.enumerate()`).
  - `fn exec_step(step, kind, conflict, token) -> (PathBuf, OpOutcome, u64, bool)` (la 4ª = `counts_as_file`).
- `naygo_core::config`: `portable_dir() -> PathBuf`; patrón `read_json`/`write_json(&dir.join("..."))`. `Settings` sin cambios en ops-B.
- `naygo-ui::app::NaygoApp`: `ActiveOp { rx, token, label, progress, summary, started, pending: Option<(OpPlan, OpKind, ConflictPolicy)> }` (línea 43). `start_op(&mut self, req: OpRequest, label: String)` (línea 410): papelera `Delete{to_trash:true}` interceptada → `platform::trash` directo + refresh + return; else `plan(&req)`, modo cola/paralelo, `let (rx,_h) = ops::spawn(plan, req.kind, req.conflict, token.clone(), crx)` (442) y la cola drena con otro `ops::spawn` (499). `pump_ops` drena `OpMsg`, `Done|Cancelled` setea `summary`+`just_finished` (472-484), refresca panel activo. `prune_finished_ops` (518). `config_dir` (privado). `self.settings`. `tr(key)`.
- `naygo-ui::ops_dialogs`: tiene `confirm_delete`, `conflict`, `name_input` (modales egui::Modal). `PendingDialog` enum en app.rs.
- `naygo-ui::ops_panel`: panel + resumen.
- i18n: `crates/core/src/i18n/{es,en}.json` planos; `I18n::t`.

**Prerequisito:** toolchain Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo. `cargo fmt --all -- --check` (este workspace requiere `--all`). Binario `--bin naygo`.

**Convenciones (CLAUDE.md):** código en inglés; comentarios/commits en español OK. Header de 2 líneas en archivos NUEVOS. `core` NUNCA importa egui/windows. UI nunca hace I/O en el hilo de UI (el journal lo escribe el worker). Build limpio + tests + clippy `--workspace --all-targets -- -D warnings` antes de cada commit. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/ops-b`. NO cambiar de rama.

**Alcance:** ENTRA: `core::ops::journal` (FileFingerprint/OpJournal/JournalWriter/scan/resume_plan/remove), integración con el motor (param opcional), modal de retomar, app.rs (journal al lanzar + scan al arrancar + retomar/descartar), i18n. NO ENTRA: journaling de papelera, conflicto per-file, paste inteligente/shell/watcher/sizing.

---

## Estructura de archivos

```
crates/core/src/ops/
├── journal.rs   # NUEVO: FileFingerprint, OpJournal, JournalWriter, ResumePlan, scan, resume_plan, journal_path, remove
├── engine.rs    # spawn/run_plan + parámetro Option<&JournalWriter>; record tras cada paso (índice)
├── mod.rs       # + pub mod journal; re-exports
crates/core/src/i18n/{es,en}.json  # + claves del modal de retomar
crates/ui/src/
├── ops_dialogs.rs  # + resume_dialog (modal)
├── app.rs          # journal al lanzar; borrar al terminar; scan al arrancar; modal retomar
```

---

## Task 1: `core::ops::journal` — FileFingerprint + OpJournal (tipos + serde)

**Files:**
- Create: `crates/core/src/ops/journal.rs`
- Modify: `crates/core/src/ops/mod.rs`

- [ ] **Step 1: Crear `journal.rs` con tipos + tests de fingerprint/serde**

Create `crates/core/src/ops/journal.rs`:

```rust
// Naygo — journal en disco de operaciones para retomar tras un crash (puro + FS).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Persiste el estado de una operación larga en `<config_dir>/ops-journal/<id>.json`
//! a medida que avanza (throttle, best-effort). Al arrancar, `scan` detecta las
//! interrumpidas y `resume_plan` poda el plan a lo pendiente revalidando que los
//! orígenes no cambiaron. El motor de ops-A se reutiliza tal cual.

use super::{ConflictPolicy, OpKind, OpPlan, OpStep};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
            .map(|s| s.from.as_ref().and_then(|p| FileFingerprint::of(p)))
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
    use std::fs;

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
        fs::write(&f, b"hola").unwrap(); // 4 bytes
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
```

- [ ] **Step 2: Declarar el submódulo + re-export**

Modify `crates/core/src/ops/mod.rs`: añadir `pub mod journal;` junto a los otros `pub mod`. Re-export (junto a los existentes): `pub use journal::{FileFingerprint, OpJournal};`

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core ops::journal` → 4 tests PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio. (NOTA: `FileFingerprint::of` con `.and_then(|p| FileFingerprint::of(p))` — clippy puede sugerir `.and_then(FileFingerprint::of)`; aplicar la forma que clippy acepte.)

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/ops/journal.rs crates/core/src/ops/mod.rs
git commit -m "feat(core): journal de operaciones — FileFingerprint + OpJournal (tipos)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `journal` — JournalWriter con throttle + scan + remove

**Files:**
- Modify: `crates/core/src/ops/journal.rs`
- Modify: `crates/core/src/ops/mod.rs`

- [ ] **Step 1: Tests (TDD)**

Añadir al `#[cfg(test)] mod tests` de journal.rs:

```rust
    use std::time::{Duration, Instant};

    #[test]
    fn writer_primer_record_escribe() {
        let dir = tempfile::tempdir().unwrap();
        let (plan, _s) = sample_plan(dir.path());
        let j = OpJournal::new("w1".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        let mut w = JournalWriter::new(dir.path(), j);
        let now = Instant::now();
        w.record(1, now);
        // El archivo existe y refleja done_through=1.
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
        w.record(1, t0); // primer write: persiste done_through=1
        w.record(2, t0 + Duration::from_millis(100)); // dentro del throttle → NO persiste
        let back: OpJournal =
            serde_json::from_str(&fs::read_to_string(journal_path(dir.path(), "w2")).unwrap()).unwrap();
        assert_eq!(back.done_through, 1, "el 2º record dentro del umbral no se persiste");
        // Pasado el umbral sí.
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
        w.record(2, t0 + Duration::from_millis(50)); // throttled, no escribió
        w.flush(); // fuerza
        let back: OpJournal =
            serde_json::from_str(&fs::read_to_string(journal_path(dir.path(), "w3")).unwrap()).unwrap();
        assert_eq!(back.done_through, 2);
    }

    #[test]
    fn scan_lee_journals_e_ignora_corruptos() {
        let dir = tempfile::tempdir().unwrap();
        let jdir = dir.path().join("ops-journal");
        fs::create_dir_all(&jdir).unwrap();
        // Uno válido.
        let (plan, _s) = sample_plan(dir.path());
        let j = OpJournal::new("ok".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        fs::write(jdir.join("ok.json"), serde_json::to_string(&j).unwrap()).unwrap();
        // Uno corrupto.
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
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core ops::journal`
Expected: ERROR de compilación — `JournalWriter`/`journal_path`/`scan`/`remove` no existen.

- [ ] **Step 3: Implementar (en journal.rs, antes del mod tests)**

```rust
use std::time::Instant;

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
        let mut w = JournalWriter {
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
```

NOTA: `record` persiste en el PRIMER llamado siempre (`last_write None` → due). Eso hace que el test `writer_primer_record_escribe` pase y que el `done_through=1` quede en disco. El `new` también persiste el inicial (done_through=0). El throttle aplica a partir del segundo `record`.

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core ops::journal` → 9 tests PASS (4 de Task 1 + 5 nuevos).
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 5: Re-export + commit**

Modify `crates/core/src/ops/mod.rs`: ampliar el re-export: `pub use journal::{journal_path, remove, scan, FileFingerprint, JournalWriter, OpJournal};`

```bash
git add crates/core/src/ops/journal.rs crates/core/src/ops/mod.rs
git commit -m "feat(core): JournalWriter (throttle) + scan + remove

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `journal` — resume_plan (poda + revalidación estricta)

**Files:**
- Modify: `crates/core/src/ops/journal.rs`
- Modify: `crates/core/src/ops/mod.rs`

- [ ] **Step 1: Tests (TDD)**

Añadir al `mod tests`:

```rust
    #[test]
    fn resume_plan_poda_a_pendientes() {
        let dir = tempfile::tempdir().unwrap();
        // Plan de 3 archivos.
        let mk = |n: &str, b: u64| OpStep { from: Some(dir.path().join(n)), to: dir.path().join("dst").join(n), bytes: b, is_dir: false };
        for (n, c) in [("a", "aa"), ("b", "bbb"), ("c", "cccc")] {
            fs::write(dir.path().join(n), c.as_bytes()).unwrap();
        }
        let plan = OpPlan { steps: vec![mk("a",2), mk("b",3), mk("c",4)], total_bytes: 9, total_files: 3 };
        let mut j = OpJournal::new("p1".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        j.done_through = 1; // "a" ya hecho
        let r = resume_plan(&j);
        // Pendientes: b, c (índices 1,2). Sin cambios → ambos entran.
        assert_eq!(r.plan.steps.len(), 2);
        assert_eq!(r.plan.total_bytes, 7);
        assert_eq!(r.plan.total_files, 2);
        assert!(r.skipped_changed.is_empty());
    }

    #[test]
    fn resume_plan_salta_origen_cambiado() {
        let dir = tempfile::tempdir().unwrap();
        let src_b = dir.path().join("b");
        let mk = |n: &str, b: u64| OpStep { from: Some(dir.path().join(n)), to: dir.path().join("dst").join(n), bytes: b, is_dir: false };
        fs::write(dir.path().join("a"), b"aa").unwrap();
        fs::write(&src_b, b"bbb").unwrap();
        let plan = OpPlan { steps: vec![mk("a",2), mk("b",3)], total_bytes: 5, total_files: 2 };
        let j = OpJournal::new("p2".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        // CAMBIAR b después de crear el journal (distinto tamaño).
        fs::write(&src_b, b"bbbbbbbb").unwrap();
        let r = resume_plan(&j);
        // a OK (entra), b cambió (a skipped).
        assert_eq!(r.plan.steps.len(), 1);
        assert_eq!(r.skipped_changed, vec![src_b]);
    }

    #[test]
    fn resume_plan_salta_origen_ausente() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a");
        let mk = OpStep { from: Some(src.clone()), to: dir.path().join("dst").join("a"), bytes: 2, is_dir: false };
        fs::write(&src, b"aa").unwrap();
        let plan = OpPlan { steps: vec![mk], total_bytes: 2, total_files: 1 };
        let j = OpJournal::new("p3".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        fs::remove_file(&src).unwrap(); // origen desaparece
        let r = resume_plan(&j);
        assert!(r.plan.steps.is_empty());
        assert_eq!(r.skipped_changed, vec![src]);
    }

    #[test]
    fn resume_plan_pasos_sin_from_siempre_entran() {
        let dir = tempfile::tempdir().unwrap();
        // Un paso de crear carpeta (from None) + un archivo.
        let dir_step = OpStep { from: None, to: dir.path().join("dst").join("sub"), bytes: 0, is_dir: true };
        let src = dir.path().join("a"); fs::write(&src, b"aa").unwrap();
        let file_step = OpStep { from: Some(src), to: dir.path().join("dst").join("sub").join("a"), bytes: 2, is_dir: false };
        let plan = OpPlan { steps: vec![dir_step, file_step], total_bytes: 2, total_files: 1 };
        let j = OpJournal::new("p4".into(), OpKind::Copy, ConflictPolicy::Overwrite, plan);
        let r = resume_plan(&j); // done_through=0 → ambos pendientes
        assert_eq!(r.plan.steps.len(), 2); // la carpeta (sin from) siempre entra
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core ops::journal` → ERROR: `resume_plan`/`ResumePlan` no existen.

- [ ] **Step 3: Implementar**

Añadir a journal.rs (antes del mod tests):

```rust
/// Resultado de planificar un retomar: el plan podado a pendientes + los saltados.
pub struct ResumePlan {
    /// Solo los pasos pendientes que revalidaron OK.
    pub plan: OpPlan,
    /// Orígenes que cambiaron/desaparecieron desde el journal → reportar en el resumen.
    pub skipped_changed: Vec<PathBuf>,
}

/// Construye el plan a ejecutar al retomar `journal`: toma los pasos con índice
/// `>= done_through` (pendientes); revalida la huella de cada origen contra la del
/// journal. Coincide → el paso entra; difiere o el origen desapareció → va a
/// `skipped_changed`. Los pasos sin `from` (crear) siempre entran. Recalcula totales.
pub fn resume_plan(journal: &OpJournal) -> ResumePlan {
    let mut steps: Vec<OpStep> = Vec::new();
    let mut skipped_changed: Vec<PathBuf> = Vec::new();
    let mut total_bytes = 0u64;
    let mut total_files = 0usize;

    for (idx, step) in journal.plan.steps.iter().enumerate() {
        if idx < journal.done_through {
            continue; // ya hecho
        }
        match &step.from {
            None => {
                // Crear carpeta/archivo sin origen: siempre entra.
                steps.push(step.clone());
                if !step.is_dir {
                    total_files += 1;
                }
            }
            Some(from) => {
                let recorded = journal.source_fingerprints.get(idx).and_then(|f| f.clone());
                let current = FileFingerprint::of(from);
                if current.is_some() && current == recorded {
                    steps.push(step.clone());
                    total_bytes += step.bytes;
                    if !step.is_dir {
                        total_files += 1;
                    }
                } else {
                    // Cambió o desapareció → saltar y reportar.
                    skipped_changed.push(from.clone());
                }
            }
        }
    }

    ResumePlan {
        plan: OpPlan { steps, total_bytes, total_files },
        skipped_changed,
    }
}
```

NOTA: `current == recorded` compara `Option<FileFingerprint>`. Si `recorded` era `None` (no se pudo leer al planificar) y ahora `current` es `Some`, NO coinciden → se salta (conservador, correcto). Si ambos `None` (ausente entonces y ahora) → `current.is_some()` es false → se salta. La condición `current.is_some() && current == recorded` cubre ambos: solo entra si HAY huella actual Y coincide con la registrada.

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core ops::journal` → 13 PASS (9 + 4 nuevos).
Run: `cargo test -p naygo-core` → verde.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → limpio.

- [ ] **Step 5: Re-export + commit**

Modify `crates/core/src/ops/mod.rs`: ampliar: `pub use journal::{journal_path, remove, resume_plan, scan, FileFingerprint, JournalWriter, OpJournal, ResumePlan};`

```bash
git add crates/core/src/ops/journal.rs crates/core/src/ops/mod.rs
git commit -m "feat(core): resume_plan (poda a pendientes + revalidación estricta)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Integrar el journal en el motor (engine)

**Files:**
- Modify: `crates/core/src/ops/engine.rs`

Objetivo: `spawn`/`run_plan` aceptan un `Option<JournalWriter>`; tras cada paso completado, `record(idx+1, Instant::now())`; al terminar, `flush()`. Sin journal = ops-A intacto.

- [ ] **Step 1: Test de integración (TDD)**

En el `#[cfg(test)] mod tests` de engine.rs, añadir (reusa el patrón `run` existente; este test usa el journal):

```rust
    #[test]
    fn run_plan_con_journal_actualiza_done_through() {
        use super::super::journal::{journal_path, JournalWriter, OpJournal};
        let dir = tempfile::tempdir().unwrap();
        // 2 archivos a copiar.
        for (n, c) in [("a", "aa"), ("b", "bbb")] {
            fs::write(dir.path().join(n), c.as_bytes()).unwrap();
        }
        let dest = dir.path().join("dst");
        fs::create_dir(&dest).unwrap();
        let req = OpRequest {
            kind: OpKind::Copy,
            sources: vec![dir.path().join("a"), dir.path().join("b")],
            dest_dir: Some(dest.clone()),
            conflict: ConflictPolicy::Overwrite,
        };
        let p = plan(&req).unwrap();
        let cfg = dir.path(); // usamos el tempdir como "config_dir"
        let journal = OpJournal::new("eng1".into(), req.kind.clone(), req.conflict, p.clone());
        let mut writer = JournalWriter::new(cfg, journal);
        let token = CancellationToken::new();
        let (tx, _rx) = mpsc::channel();
        let (_ctx, crx) = mpsc::channel();
        let _summary = run_plan(&p, &req.kind, req.conflict, &token, &tx, &crx, Some(&mut writer));
        writer.flush();
        // El journal en disco refleja que se completaron los 2 pasos.
        let back: OpJournal =
            serde_json::from_str(&fs::read_to_string(journal_path(cfg, "eng1")).unwrap()).unwrap();
        assert_eq!(back.done_through, 2);
        assert!(dest.join("a").exists() && dest.join("b").exists());
    }
```

NOTA: el test pasa `Some(&mut writer)` como nuevo último parámetro de `run_plan`. Los tests EXISTENTES de engine que llaman `run_plan(&p, &kind, conflict, &token, &tx, &crx)` deben actualizarse a `run_plan(&p, &kind, conflict, &token, &tx, &crx, None)`. Buscar todos los llamados a `run_plan` en el `mod tests` y añadir `, None`. Igual el helper `run(req)` si llama `run_plan`.

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core ops::engine`
Expected: ERROR de compilación — `run_plan` toma 7 args, no 6 (los tests viejos + el nuevo no compilan hasta cambiar la firma).

- [ ] **Step 3: Implementar el parámetro de journal**

Modify `crates/core/src/ops/engine.rs`:

a) `use` al tope: `use super::journal::JournalWriter;`

b) Cambiar la firma de `run_plan` para añadir `journal: Option<&mut JournalWriter>` como ÚLTIMO parámetro, y registrar tras cada paso. Reemplazar el cuerpo del loop por una versión con índice:
```rust
pub fn run_plan(
    plan: &OpPlan,
    kind: &OpKind,
    conflict: ConflictPolicy,
    token: &CancellationToken,
    tx: &Sender<OpMsg>,
    _conflict_rx: &Receiver<ConflictDecision>,
    mut journal: Option<&mut JournalWriter>,
) -> OpSummary {
    let start = std::time::Instant::now();
    let mut summary = OpSummary::default();
    let mut files_done = 0usize;

    for (idx, step) in plan.steps.iter().enumerate() {
        if token.is_cancelled() {
            break;
        }

        let _ = tx.send(OpMsg::Progress(OpProgress {
            bytes_done: summary.bytes_done,
            bytes_total: plan.total_bytes,
            files_done,
            files_total: plan.total_files,
            current: step.to.clone(),
        }));

        let (record_path, outcome, bytes_added, counts_as_file) =
            exec_step(step, kind, conflict, token);
        summary.bytes_done += bytes_added;
        if counts_as_file && matches!(outcome, OpOutcome::Done) {
            files_done += 1;
        }
        summary.items.push((record_path, outcome));

        // Journal: el paso `idx` quedó procesado → done_through = idx + 1 (throttled).
        if let Some(w) = journal.as_deref_mut() {
            w.record(idx + 1, std::time::Instant::now());
        }
    }

    summary.elapsed_secs = start.elapsed().as_secs_f64();
    summary
}
```

c) Cambiar `spawn` para aceptar y pasar el journal. Como el `JournalWriter` debe vivir en el hilo del worker y persistir al final, `spawn` toma `Option<JournalWriter>` por valor (lo mueve al hilo), hace `flush()` al terminar y borra el journal:
```rust
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
        // Al terminar (Done o Cancelled), el journal ya no es necesario: la UI lo
        // borra. Aquí solo hacemos un flush final por si el throttle dejó algo sin
        // persistir (no es estrictamente necesario porque la UI borra, pero deja el
        // estado coherente si la UI no llega a borrar).
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
```

NOTA: el borrado del journal al terminar lo hace la UI (Task 7), no `spawn` — porque `spawn` no conoce el `config_dir` de forma independiente del writer, y la UI ya maneja el ciclo de vida de la op. `spawn` solo hace flush. La UI, al recibir `Done`/`Cancelled`, llama `journal::remove(config_dir, id)`.

- [ ] **Step 4: Actualizar los call sites de run_plan en los tests existentes**

En el `mod tests` de engine.rs, todos los `run_plan(&p, &kind, conflict, &token, &tx, &crx)` → añadir `, None`. (El helper `run(req)` y el test de cancelación.)

- [ ] **Step 5: Correr — pasan**

Run: `cargo test -p naygo-core ops::engine` → todos PASS (existentes + el nuevo).
Run: `cargo test -p naygo-core` → verde.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → limpio.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/ops/engine.rs
git commit -m "feat(core): el motor registra el journal tras cada paso (param opcional)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: i18n — claves del modal de retomar (ES + EN)

**Files:**
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: Claves en es.json** (insertar en punto válido):
```json
  "resume.title": "Operaciones interrumpidas",
  "resume.body": "Naygo se cerró mientras estas operaciones estaban en curso:",
  "resume.resume": "Retomar",
  "resume.discard": "Descartar",
  "resume.resume_all": "Retomar todas",
  "resume.discard_all": "Descartar todas",
  "resume.progress": "{done} de {total} archivos",
  "resume.skipped_changed": "{n} archivo(s) omitido(s) porque cambiaron desde la interrupción"
```

- [ ] **Step 2: Mismas en en.json:**
```json
  "resume.title": "Interrupted operations",
  "resume.body": "Naygo closed while these operations were in progress:",
  "resume.resume": "Resume",
  "resume.discard": "Discard",
  "resume.resume_all": "Resume all",
  "resume.discard_all": "Discard all",
  "resume.progress": "{done} of {total} files",
  "resume.skipped_changed": "{n} file(s) skipped because they changed since the interruption"
```

- [ ] **Step 3: Verificar + commit**

Run: `cargo test -p naygo-core i18n` → parity PASS. `cargo test -p naygo-core` → verde.
```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "i18n: claves del modal de retomar operaciones (ES/EN)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: `ops_dialogs::resume_dialog` — modal de retomar

**Files:**
- Modify: `crates/ui/src/ops_dialogs.rs`

- [ ] **Step 1: Añadir el modal**

VERIFICAR egui 0.34.3 `egui::Modal` (ya usado en ops_dialogs.rs para confirm_delete/conflict — seguir el mismo patrón). Añadir a `ops_dialogs.rs`:

```rust
/// Decisión del usuario sobre las operaciones interrumpidas.
pub enum ResumeChoice {
    /// Retomar la operación con este id.
    Resume(String),
    /// Descartar la operación con este id.
    Discard(String),
    /// Retomar todas las pendientes.
    ResumeAll,
    /// Descartar todas.
    DiscardAll,
}

/// Modal de retomar: lista las operaciones interrumpidas (id + label + progreso) con
/// Retomar/Descartar por op, y Retomar todas / Descartar todas. Devuelve `Some(choice)`
/// cuando el usuario actúa, `None` mientras sigue abierto. `items` = (id, label, done, total).
pub fn resume_dialog(
    ctx: &egui::Context,
    i18n: &naygo_core::i18n::I18n,
    items: &[(String, String, usize, usize)],
) -> Option<ResumeChoice> {
    let mut choice = None;
    egui::Modal::new(egui::Id::new("resume_ops_modal")).show(ctx, |ui| {
        ui.set_max_width(440.0);
        ui.heading(i18n.t("resume.title"));
        ui.label(i18n.t("resume.body"));
        ui.add_space(6.0);
        for (id, label, done, total) in items {
            ui.group(|ui| {
                ui.label(egui::RichText::new(label).strong());
                let prog = i18n
                    .t("resume.progress")
                    .replace("{done}", &done.to_string())
                    .replace("{total}", &total.to_string());
                ui.label(egui::RichText::new(prog).weak());
                ui.horizontal(|ui| {
                    if ui.button(i18n.t("resume.resume")).clicked() {
                        choice = Some(ResumeChoice::Resume(id.clone()));
                    }
                    if ui.button(i18n.t("resume.discard")).clicked() {
                        choice = Some(ResumeChoice::Discard(id.clone()));
                    }
                });
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if items.len() > 1 {
                if ui.button(i18n.t("resume.resume_all")).clicked() {
                    choice = Some(ResumeChoice::ResumeAll);
                }
                if ui.button(i18n.t("resume.discard_all")).clicked() {
                    choice = Some(ResumeChoice::DiscardAll);
                }
            }
        });
    });
    choice
}
```

VERIFICAR: `egui::Modal::new(Id).show(ctx, closure)` firma real en 0.34 (cómo lo usa `confirm_delete`); `ui.group`, `ui.set_max_width`, `RichText::weak/strong`. Adaptar a lo que el archivo ya usa. NO manejar Esc para cerrar sin decisión (el modal de retomar debe forzar una decisión — no hay should_close; el usuario elige por op o "todas"). Si el patrón existente usa should_close, para este modal ignorarlo (no cerrar sin elegir).

- [ ] **Step 2: Verificar + commit**

Run: `cargo build -p naygo-ui` → compila. `cargo clippy -p naygo-ui --all-targets -- -D warnings`. (`resume_dialog`/`ResumeChoice` sin uso hasta Task 7 → `#[allow(dead_code)]` temporal si bloquea, con comentario "consumido en Task 7".) `cargo fmt --all`.

```bash
git add crates/ui/src/ops_dialogs.rs
git commit -m "feat(ui): modal de retomar operaciones interrumpidas

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: app.rs — journal al lanzar, scan al arrancar, retomar/descartar

**Files:**
- Modify: `crates/ui/src/app.rs`

Tarea de integración. Conecta el journal al ciclo de vida de las ops y el modal al arranque.

- [ ] **Step 1: ActiveOp lleva el journal id; start_op crea el journal**

Modify `crates/ui/src/app.rs`:
a) `use`: `use naygo_core::ops::journal::{self, JournalWriter, OpJournal};`
b) En `struct ActiveOp`, añadir `pub journal_id: Option<String>,` (None para ops sin journal — p. ej. papelera no llega aquí, pero rename/create cortas tampoco necesitan; journaleamos copy/move/delete-permanente).
c) Inicializar `journal_id` en TODOS los `ActiveOp { ... }` literales (poner `journal_id: None` donde no aplica; se setea abajo para las journaled).
d) En `start_op`, tras obtener `plan` y ANTES de spawnear (en la rama que sí spawnea, línea ~442), decidir si journalear: journalear si `kind` es Copy/Move o `Delete{to_trash:false}`. Generar el id con timestamp UI:
```rust
        // ¿Journalear esta op? Copy/Move/Delete-permanente (las largas y recuperables).
        let journaled = matches!(
            req.kind,
            OpKind::Copy | OpKind::Move | OpKind::Delete { to_trash: false }
        );
        let journal = if journaled {
            let id = self.next_journal_id();
            let j = OpJournal::new(id.clone(), req.kind.clone(), req.conflict, plan.clone());
            Some((id, JournalWriter::new(&self.config_dir, j)))
        } else {
            None
        };
```
Y al spawnear, pasar el writer y guardar el id:
```rust
        let (journal_id, writer) = match journal {
            Some((id, w)) => (Some(id), Some(w)),
            None => (None, None),
        };
        let (rx, _h) = ops::spawn(plan, req.kind, req.conflict, token.clone(), crx, writer);
        self.active_ops.push(ActiveOp {
            rx: Some(rx), token, label, progress: None, summary: None,
            started: true, pending: None, journal_id,
        });
```
NOTA: la rama de COLA (pending) guarda `(plan, kind, conflict)` y spawnea luego en `pump_ops`. Para journalear la op encolada, hay que crear el journal cuando se LANCE (en pump_ops), no al encolar — o crear el journal al encolar y reusarlo. SIMPLIFICACIÓN para ops-B: journalear solo las que se lanzan inmediatamente (start_now). Las encoladas se journalean cuando pump_ops las spawnea — para eso, en pump_ops, al spawnear una pendiente, crear el journal igual que arriba (mismo bloque). Implementar el journal-create también en el drain de cola de pump_ops (el otro `ops::spawn`, línea ~499), guardando el journal_id en ese ActiveOp. Si resulta repetitivo, extraer un helper `fn make_journal(&self, kind, conflict, plan) -> Option<(String, JournalWriter)>`.

Añadir el helper de id (timestamp + contador, UI puede usar SystemTime):
```rust
    /// Genera un id único para un journal de operación (timestamp + nanos).
    fn next_journal_id(&self) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("op-{now}")
    }
```

- [ ] **Step 2: Borrar el journal al terminar (Done/Cancelled)**

En `pump_ops`, donde se maneja `OpMsg::Done(s) | OpMsg::Cancelled(s)` (línea ~472), tras setear el summary, borrar el journal de esa op si tiene id. Como ahí estás en `&mut op`, necesitas `self.config_dir` — captura el `config_dir` antes del loop (es PathBuf, clónalo a un local `let cfg = self.config_dir.clone();`) y dentro del match: `if let Some(id) = &op.journal_id { journal::remove(&cfg, id); }`.

- [ ] **Step 3: Scan al arrancar + estado de retomar**

a) En `struct NaygoApp`, añadir `pending_resume: Vec<OpJournal>,` (las interrumpidas a ofrecer).
b) En `NaygoApp::new`, tras cargar config: `let pending_resume = journal::scan(&config_dir);` y añadirlo al literal del struct. (scan es barato; lee la carpeta una vez.)
c) En `ui()`, ANTES o como parte del manejo de diálogos: si `!self.pending_resume.is_empty()`, mostrar el modal `ops_dialogs::resume_dialog(ctx, &self.i18n, &items)` donde `items` se arma de `pending_resume` (id, label vía `OpJournal::label()`, done_through, total = plan.steps.len() o total_files). Procesar el `ResumeChoice`:
   - `Resume(id)` → tomar ese journal de `pending_resume`, `journal::resume_plan(&j)`; si `r.plan.steps` no vacío → reusar el id: crear un `JournalWriter` nuevo sobre ese id con un OpJournal del plan podado (o re-`OpJournal::new(id, kind, conflict, r.plan)`), spawnear vía un `start_resumed_op` análogo a start_op pero con el plan ya hecho + el id dado; si vacío → `journal::remove(&config_dir, &id)` (nada que hacer). Mostrar `r.skipped_changed` en el status/resumen (`resume.skipped_changed` con {n}).
   - `Discard(id)` → `journal::remove(&config_dir, &id)`, quitarlo de pending_resume.
   - `ResumeAll`/`DiscardAll` → aplicar a todos.
   - Quitar de `pending_resume` lo procesado; cuando quede vacío, el modal desaparece.

   Para retomar, añadir un método:
```rust
    /// Retoma una operación desde un plan ya podado, reusando el `id` de journal.
    fn start_resumed_op(&mut self, id: String, kind: OpKind, conflict: ConflictPolicy, plan: OpPlan, label: String) {
        let token = CancellationToken::new();
        let (_ctx, crx) = std::sync::mpsc::channel();
        let j = OpJournal::new(id.clone(), kind.clone(), conflict, plan.clone());
        let writer = JournalWriter::new(&self.config_dir, j);
        let (rx, _h) = ops::spawn(plan, kind, conflict, token.clone(), crx, Some(writer));
        self.active_ops.push(ActiveOp {
            rx: Some(rx), token, label, progress: None, summary: None,
            started: true, pending: None, journal_id: Some(id),
        });
    }
```
   NOTA: `OpJournal::new` recalcula fingerprints del plan podado (los pendientes), lo cual es correcto (las huellas actuales pasan a ser la referencia desde el retomar). `done_through` arranca en 0 para el plan podado.

d) Manejar el modal de retomar con prioridad sobre otros: si hay `pending_resume`, mostrarlo; egui Modal bloquea. Asegurar que el resto de `ui()` (toolbar, dock) se sigue pintando detrás (el modal es overlay) pero los triggers de teclado se pueden suprimir mientras el modal está (como ya se hace con `pending_dialog`).

- [ ] **Step 4: Quitar allows + verificar**

Quitar `#[allow(dead_code)]` de `resume_dialog`/`ResumeChoice`.
Run: `cargo build -p naygo-ui` → compila (resolver borrows: `config_dir.clone()` a local para el loop de pump_ops; el modal lee `&self.pending_resume` y `&self.i18n` mientras `ctx` es `&` — sin conflicto con `&mut self` si se estructura como en pending_dialog).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt --all`.

App-start: copiar muchos archivos → se crea `<config>/ops-journal/op-<ts>.json`; al terminar se borra. Si matas Naygo a media copia y reabres → modal "Operaciones interrumpidas" con Retomar/Descartar; Retomar reanuda los pendientes (saltando los que cambiaron, reportados); Descartar borra el journal.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/app.rs
git commit -m "feat(ui): journal de ops (crear/borrar) + scan y modal de retomar al arrancar

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: README**

Modify `README.md` — bloque de estado:
```markdown
> **Estado:** Fase ops-B (journal + retomar tras crash) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-08-naygo-ops-b-design.md`](docs/superpowers/specs/2026-06-08-naygo-ops-b-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-08-naygo-ops-b.md`](docs/superpowers/plans/2026-06-08-naygo-ops-b.md).
> Operaciones de archivo (ops-A) y bloque visual completos.
```
(READ el bloque actual y reemplazarlo.)

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → verde (core: ops::journal + engine integración; ui).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo fmt --all -- --check` → limpio.
Run: `cargo build --release -p naygo-ui` → release compila.
App-start manual: provocar journal + retomar (matar a mitad, reabrir).

- [ ] **Step 3: Commit y push**

```bash
git add README.md
git commit -m "chore: actualizar estado del README (fase ops-B)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/ops-b
```

---

## Self-review (cobertura del spec)

| Requisito del spec ops-B | Tarea(s) |
|---|---|
| FileFingerprint (len+mtime, of()) | 1 |
| OpJournal (serde, new con fingerprints, label) | 1 |
| JournalWriter throttle (record/flush, now inyectable, best-effort) | 2 |
| scan (ignora corruptos) | 2 |
| journal_path / remove | 2 |
| resume_plan (poda a pendientes + revalidación estricta + skipped_changed + totales) | 3 |
| Integración motor (param opcional, record tras cada paso, flush) | 4 |
| i18n del modal | 5 |
| Modal de retomar (lista, Retomar/Descartar/todas) | 6 |
| Journal al lanzar (copy/move/delete-perm; id timestamp UI) | 7 |
| Borrar journal al Done/Cancelled | 7 |
| Scan al arrancar + modal + retomar/descartar | 7 |
| Retomar reusa id + muestra skipped_changed | 7 |
| Papelera NO journaleada | 7 (journaled = solo Copy/Move/Delete{to_trash:false}) |

**Notas de riesgo:**
- **Firma de run_plan/spawn** (Task 4): añadir el param de journal rompe los call sites existentes en los tests de engine Y en app.rs (los 2 `ops::spawn` de start_op + cola). Task 4 actualiza los tests; Task 7 actualiza app.rs (los 2 spawn pasan el writer o None). Verificar que NO quede ningún `ops::spawn`/`run_plan` con la aridad vieja (grep).
- **Cola + journal** (Task 7): journalear también las ops que arrancan desde la cola (el 2º `ops::spawn` en pump_ops) — crear el journal al spawnear la pendiente, no al encolar. Helper `make_journal` para no duplicar.
- **Borrows en pump_ops** (Task 7): `journal::remove(&cfg, id)` dentro del loop sobre `&mut self.active_ops` → clonar `config_dir` a un local `cfg` antes del loop.
- **egui Modal** (Task 6): seguir el patrón EXACTO que ya usan confirm_delete/conflict en ops_dialogs.rs (no inventar otra forma). El modal de retomar NO cierra sin decisión.
- **resume_plan recalcula fingerprints al retomar** (`OpJournal::new` sobre el plan podado): correcto — el nuevo journal del retomar usa el estado actual como referencia, y done_through=0 sobre el plan podado.
- **Throttle test determinista** (Task 2): el `now: Instant` se inyecta, así que el test no depende del reloj real (usa `t0 + Duration`).
```
