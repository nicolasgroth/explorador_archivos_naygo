# Naygo — Fase ops-B: journal en disco + retomar tras crash (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-08. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Segunda fase del sprint de funcionalidad, **encima de ops-A** (ya mergeada). ops-A
entregó: `core::ops` (modelo serializable `OpKind`/`OpRequest`/`OpStep`/`OpPlan`/
`OpProgress`/`OpSummary`/`OpOutcome`; `plan()` puro; `engine::spawn`/`run_plan` motor
worker que copia por buffers, cancelable, emite `OpMsg` por canal), `platform::trash`,
Settings de ops, panel de operaciones, diálogos, clipboard interno, resumen+exportar.

ops-B agrega **durabilidad**: persistir un journal de cada operación larga en curso para
que, si Naygo se cae (o el SO/energía), al reabrir se ofrezca **retomar desde donde
quedó** (o descartar). El modelo de ops-A ya es `Serialize`/`Deserialize` — fue pensado
para esto.

**Premisa rectora:** el journal NO debe frenar la operación (escritura con throttle,
best-effort en el worker, nunca en el hilo de UI). La UI nunca bloquea. Tolerante:
journal corrupto se ignora. Revalidación estricta al retomar (nunca copiar datos
inconsistentes).

### Decisiones tomadas en el brainstorm

1. **Presentación del retomar: MODAL al arrancar.** Al reabrir, si hay operaciones
   interrumpidas, un modal las lista (label + progreso "124/200") con Retomar/Descartar
   por operación (y, si hay varias, "retomar todas"/"descartar todas"). Bloquea hasta
   decidir — imposible de ignorar.
2. **Revalidación ESTRICTA.** Al retomar, para cada paso pendiente se revalida la huella
   del origen (tamaño + fecha de modificación) contra la guardada en el journal. Si
   cambió o desapareció → ese archivo se SALTA y se reporta en el resumen ("cambió,
   omitido"). Los no afectados se retoman normal. Nunca se reusa progreso parcial de un
   archivo cuyo origen cambió.
3. **Granularidad: tras cada archivo completado, con throttle (~500 ms).** El motor
   actualiza el journal al terminar un archivo, pero a lo sumo una escritura cada
   ~500 ms (no escribe 50 veces si 50 archivos chicos terminan en un segundo). Al
   retomar se reanuda desde el último archivo registrado.
4. **Ubicación: un JSON por operación** en `<config_dir>/ops-journal/<id>.json` (patrón
   i18n/temas). Se borra al completar (`Done`) o al cancelar/descartar.

### Qué entra en ops-B

- `core::ops::journal`: `OpJournal`, `FileFingerprint`, `JournalWriter` (throttle),
  `scan()` (detección al arrancar), `resume_plan()` (poda + revalidación), borrado.
- Integración con `engine`: parámetro opcional de journal en `spawn`/`run_plan` (sin
  journal = comportamiento de ops-A intacto).
- `ui`: al lanzar op larga, crear journal con un `id` (timestamp UI); al terminar/
  cancelar, borrarlo. Al arrancar, `scan` → modal de retomar; retomar = `resume_plan` +
  `start_op` del plan podado reusando el id; descartar = borrar journal.
- Modal de retomar en `ops_dialogs.rs`.

### Qué NO entra en ops-B

- Journaling de la papelera (es atómica vía platform; no hay "a medias" que retomar).
- Cambios al conflicto per-file (sigue el modelo conflicto-por-request de ops-A; el
  retomar usa la política ya guardada en el journal).
- Paste inteligente, platform/shell, watcher, sizing (fases siguientes del sprint).
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

Idea rectora: el journal es DATOS serializables en `core` (ya lo son `OpPlan`/`OpStep`)
+ un escritor con throttle que el motor invoca + lógica pura de detección/revalidación
al arrancar (testeable). La UI muestra el modal y dispara retomar/descartar. El motor de
ops-A se reutiliza: retomar = ejecutar un plan podado a los pasos pendientes.

### Capa `core::ops::journal` (submódulo nuevo)

```rust
/// Huella de un archivo de origen al planificar, para revalidar al retomar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFingerprint {
    pub len: u64,
    pub mtime_secs: u64, // segundos epoch de la fecha de modificación; 0 si no disponible
}

/// Lo que se persiste de una operación en curso.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpJournal {
    pub id: String,                 // id único (timestamp+contador, inyectado por la UI)
    pub kind: OpKind,
    pub conflict: ConflictPolicy,
    pub plan: OpPlan,               // pasos completos (ya serializable)
    pub done_through: usize,        // cantidad de pasos completados (los índices < ya están)
    /// Huella del origen por cada paso que tiene `from` (alineado por índice de paso).
    /// `None` para pasos sin origen (crear) o cuando no se pudo leer la metadata.
    pub source_fingerprints: Vec<Option<FileFingerprint>>,
}

/// Resultado de planificar un retomar: el plan podado a pendientes + los saltados.
pub struct ResumePlan {
    pub plan: OpPlan,               // solo los pasos pendientes que revalidaron OK
    pub skipped_changed: Vec<PathBuf>, // orígenes que cambiaron/desaparecieron → reportar
}
```

- **`FileFingerprint::of(path) -> Option<FileFingerprint>`**: lee `metadata` (len +
  mtime en segundos epoch). `None` si no se puede leer (origen ausente).
- **`OpJournal::new(id, kind, conflict, plan) -> OpJournal`** (puro): calcula las
  fingerprints de los orígenes (leyendo metadata), `done_through = 0`.
- **`JournalWriter`**: envuelve la ruta `<dir>/ops-journal/<id>.json` + el `OpJournal` +
  el instante del último write. `record(done_through, now: Instant)` actualiza
  `done_through` y persiste **solo si** pasaron ≥ `THROTTLE` (~500 ms) desde el último
  write (o si es el primero). `flush()` fuerza un write (al terminar). Escritura
  best-effort: si falla, no propaga error (la op no se rompe). El `now` se inyecta para
  testear el throttle sin reloj real.
- **`scan(config_dir) -> Vec<OpJournal>`**: lee todos los `*.json` de `ops-journal/`;
  cada uno que parsea = una op interrumpida; los corruptos se ignoran (se pueden borrar).
  Puro salvo el `read_dir`/`read_to_string`.
- **`resume_plan(journal: &OpJournal) -> ResumePlan`** (puro, lee metadata para
  revalidar): toma los pasos con índice `>= done_through` (pendientes); para cada uno con
  `from`, compara `FileFingerprint::of(from)` con la guardada — si coincide, el paso entra
  al plan podado; si difiere o es `None` (cambió/ausente), va a `skipped_changed`. Los
  pasos sin `from` (crear carpeta) siempre entran. Devuelve `ResumePlan`. Recalcula
  `total_bytes`/`total_files` del plan podado.
- **`journal_path(config_dir, id) -> PathBuf`** y **`remove(config_dir, id)`**: borrar el
  journal (al completar/descartar).

### Integración con `engine` (ops-A)

- `engine::spawn` gana un parámetro `journal: Option<JournalWriter>` (o un wrapper
  `spawn_journaled`). `run_plan` ya itera pasos con un contador de `files_done`/índice;
  tras completar un paso (no-dir, Done) llama `journal.record(idx+1, Instant::now())`.
  Al terminar, `journal.flush()`. Sin journal (`None`) = comportamiento de ops-A intacto.
- **Retomar** = `spawn` de `resume_plan(journal).plan` con el MISMO `id` (un
  `JournalWriter` reabierto sobre ese id, para que siga registrando y se borre al
  terminar). El motor no distingue "retomar"; solo ejecuta un plan.

### Capa `ui`

- `app.rs`: al lanzar copy/move/delete-permanente (NO papelera), generar un `id`
  (timestamp UI: `SystemTime::now` vive en la UI, no en core), crear `JournalWriter`,
  pasarlo a `spawn`. Al `Done`/`Cancelled` de esa op, `journal::remove(config_dir, id)`.
  Guardar el `id` en el `ActiveOp` para poder borrarlo.
- **Al arrancar** (en `NaygoApp::new` o primer frame de `ui()`): `journal::scan(config_dir)`
  → si no vacío, `pending_resume: Vec<OpJournal>` que dispara el modal de retomar.
- `ops_dialogs.rs`: `resume_dialog(ctx, i18n, &[OpJournal]) -> Option<ResumeChoice>` donde
  `ResumeChoice { id: String, action: ResumeAction { Resume, Discard } }` (o "todas").
  Lista cada op (label derivado de kind+dest + "done_through/total"). 
- Retomar → `resume_plan(journal)` → si quedan pasos, `start_op` del plan podado reusando
  el id (nuevo `JournalWriter` sobre ese id) + mostrar los `skipped_changed` en el
  resumen; si no quedan (todo ya estaba hecho) → borrar journal. Descartar → borrar journal.

### Capa `core::config`

Sin cambios de Settings (no hay opción nueva). El `id` de la op lo genera la UI.

### Lo que NO cambia

El motor de copia, el panel, el clipboard, los diálogos de conflicto/confirmación de
ops-A. Solo se añade el parámetro opcional de journal al motor y el modal de retomar.

---

## 3. Flujo de datos

**Normal (con journal):** UI lanza op larga → genera `id` → `OpJournal::new` (fingerprints
de orígenes) → escribe journal inicial → `spawn(plan, kind, conflict, token, Some(writer))`.
El motor, tras cada archivo, `writer.record(done_through, now)` (throttle ~500 ms). Al
`Done`/`Cancelled` → UI borra `<id>.json`.

**Retomar (tras crash):** al arrancar, `scan` lee los `<id>.json` que el crash NO borró →
modal. Retomar → `resume_plan` revalida fingerprints de los pendientes → plan podado
(saltando cambiados/ausentes → resumen) → `start_op` reusando el `id` → journal sigue →
al terminar se borra. Descartar → borrar `<id>.json` (no se toca nada; lo ya copiado
queda).

---

## 4. Manejo de errores / casos límite

- **Journal corrupto / JSON inválido** → `scan` lo ignora (y puede borrarlo). Las demás
  interrumpidas se ofrecen igual. No crashea.
- **Origen cambió/desapareció al retomar** → revalidación estricta: ese paso se salta y
  se reporta; el resto se retoma.
- **Archivo en curso al crashear** (no registrado, `idx >= done_through`) → se re-copia
  desde cero al retomar (sobrescribe el parcial). Nunca se asume completo un archivo no
  registrado.
- **Cancelar (no crash)** → journal borrado al `Cancelled` (cancelar = decisión del
  usuario, no interrupción a retomar).
- **Crash durante el retomar** → el journal sigue con su `done_through` avanzado; el
  siguiente arranque vuelve a ofrecer retomar. Idempotente.
- **Papelera** → atómica vía platform, NO journaled (no hay "a medias").
- **Disco del journal lleno/no escribible** → `record`/`flush` fallan silencioso
  (best-effort); la op continúa sin journal (peor caso: no se puede retomar esa op).
- **Throttle pierde el último archivo antes del crash** → al retomar se re-copia
  (idempotente). Se pierde a lo sumo <500 ms de progreso registrado.

---

## 5. Testing

- **`core::ops::journal`** (el grueso, puro con tempfile): round-trip serde de
  `OpJournal`; `FileFingerprint::of` (len+mtime; `None` si ausente); `JournalWriter`
  throttle (inyectando `now`: dos `record` dentro del umbral → un solo write; pasado el
  umbral → write); `scan` (varios journals, ignora el corrupto); `resume_plan` (poda a
  pendientes desde `done_through`; revalidación: origen igual → entra, origen con
  tamaño/fecha distinta → a `skipped_changed`, origen ausente → a `skipped_changed`;
  pasos sin `from` siempre entran; totales recalculados); `remove` borra el archivo.
- **Integración motor** (tempfile): op journaled completa borra su journal; una
  interrumpida simulada (journal con `done_through` a medias) + `resume_plan` retoma solo
  lo pendiente; un origen modificado entre journal y resume se salta.
- **UI** (modal de retomar, scan al arrancar): validación manual; la lógica (scan/resume/
  revalidate/throttle) está en core, testeada.

Meta de siempre: build limpio + tests + clippy antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/ops/
├── journal.rs   # NUEVO: FileFingerprint, OpJournal, JournalWriter, scan, resume_plan, remove
├── engine.rs    # + parámetro Option<JournalWriter> en spawn/run_plan (record tras cada archivo)
├── mod.rs       # + pub mod journal; re-exports
└── ...

crates/core/src/i18n/{es,en}.json  # + claves del modal de retomar

crates/ui/src/
├── ops_dialogs.rs  # + resume_dialog (modal de retomar/descartar)
├── app.rs          # journal al lanzar; borrar al terminar; scan al arrancar; modal
└── ...
```

---

## 7. Dependencias

Ninguna nueva. `serde`/`serde_json` (journal), `std::time` (throttle/mtime/id). El id de
la op lo genera la UI con `SystemTime::now` (no en core).

---

## Fuera de alcance (recordatorio)

Journaling de papelera, conflicto per-file, paste inteligente, platform/shell, watcher,
sizing. Nunca: reproducción de media, edición de archivos.
