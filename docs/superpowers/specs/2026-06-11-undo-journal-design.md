# R2 — Deshacer (journal + Ctrl+Z + panel Historial) — Diseño

> Autor: Nicolás Groth / ISGroth — 2026-06-11. MIT License.
> Fase 2 de 3 del plan rename+undo aprobado por Nicolás.

## Qué es

Deshacer renombrar/mover/copiar/crear como en Windows (Ctrl+Z), con un **historial
visible** donde además se puede deshacer un paso puntual — con validación previa, que
fue la condición acordada para permitir deshacer fuera de orden.

## Diseño

### core: `ops/undo.rs` (lógica pura + validación con FS, patrón de `journal.rs`)
- `UndoAction`: `MoveBack { now, back_to }` (sirve para mover Y renombrar de vuelta) ·
  `TrashCreated { path }` (deshacer copia/creación = mandar lo creado a PAPELERA,
  nunca borrado permanente).
- `UndoEntry { id: u64, label, when_epoch_secs, actions: Vec<UndoAction>, undone: bool }`.
- `build_undo(req: &OpRequest, summary: &OpSummary) -> Option<Vec<UndoAction>>` (PURA,
  testeable): Rename → MoveBack; Move → empareja `sources` con los items `Done` del
  summary (los destinos REALES, robusto ante conflict-rename); Copy/CreateDir/
  CreateFile → TrashCreated de los `Done`. Delete → `None` (restaurar de papelera
  queda para una fase posterior).
- `validate(actions) -> Result<(), String>`: MoveBack exige que `now` exista, que el
  padre de `back_to` exista y que `back_to` NO esté ocupado; TrashCreated exige que
  `path` exista. Es la guarda que hace seguro el deshacer selectivo.
- `to_requests(actions) -> Vec<OpRequest>`: agrupa en OpRequests normales (Move/Rename
  de vuelta con `ConflictPolicy::Skip`; Delete a papelera) → el deshacer corre por el
  MISMO motor de ops (progreso, cancelación y panel gratis).

### app: captura + Ctrl+Z
- `ActiveOp` gana `request: Option<OpRequest>` (None = no registrar deshacer, usado
  por las ops que SON un deshacer → sin redo en v1, sin bucles).
- Al completar (`OpMsg::Done`): `build_undo` → `UndoEntry` a `undo_history`
  (Vec, tope 100, más nuevo al final). Canceladas también (lo hecho parcial es
  igualmente deshacible: los `Done` del summary).
- `Action::Undo` (keymap, default **Ctrl+Z**, editable en Atajos): toma la entrada
  más nueva no-deshecha, `validate` → si ok lanza `to_requests` con etiqueta
  "Deshacer: {label}" y la marca `undone`; si no, status con el motivo.
- i18n ES/EN: `action.undo`, `undo.done`, `undo.invalid`, `undo.nothing`.

### UI: panel Historial (PanePurpose::History)
- Nuevo tipo de panel (mismo mecanismo que Tree/Inspector): lista las entradas (más
  nueva arriba) con etiqueta, hora, nº de ítems y estado (deshecha/atascada).
- Botón "Deshacer" por entrada: `validate` al pintar → deshabilitado con tooltip del
  motivo si el inverso ya no aplica (la protección acordada contra dependencias).
- Botón "Deshacer hasta aquí": deshace en orden seguro (de la más nueva a esa).
- Se abre desde la toolbar (botón junto a "agregar panel") o desde plantillas.
- Historial en memoria de la sesión (v1); el diseño deja la puerta a persistirlo.

## Tests
core: build_undo por cada OpKind (incl. emparejado con conflict-rename y Skipped
excluidos), validate con tempfile (ok / now ausente / back_to ocupado), to_requests
agrupando por carpeta de destino. UI: verificación en vivo.
