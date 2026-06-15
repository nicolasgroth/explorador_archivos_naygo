# Migración a Slint — Fase 3: operaciones + diálogos + progreso — Diseño

> Tercera fase de la migración egui→Slint. Las Fases 1 y 2 (panel navegable, multi-panel,
> docking, paneles especiales, tabs, drag-to-rearrange) están en `main` y verificadas. Esta
> fase agrega las **operaciones de archivo** (copiar/cortar/pegar, eliminar, nuevo
> archivo/carpeta, renombrar), sus **diálogos modales**, el **panel de progreso**, el
> **deshacer**, el **journal de retomar-tras-crash**, el **clipboard del SO con corte
> visual** y el **menú contextual híbrido**. Gobernada por el contrato de paridad
> (`docs/migracion-slint/CONTRATO-PARIDAD-FUNCIONAL.md`, sección J).

## Contexto: todo el motor ya existe en core/platform

La lógica de operaciones vive completa en `naygo-core` y `naygo-platform` (la usaba la capa
egui vieja). Esta fase **reescribe solo la ORQUESTACIÓN en `crates/ui-slint`**; el único
cambio a core es ops-B (conflicto por-ítem, ver §2). Reusables sin tocar:

- `core::ops::engine` — `spawn(plan, kind, conflict, token, conflict_rx, journal) ->
  (Receiver<OpMsg>, JoinHandle)`. Corre en un hilo; emite `OpMsg::{Progress, Conflict, Done,
  Cancelled, Failed}`. Cancelable por token; copia cancelada borra el parcial.
- `core::ops::{plan, names, undo, journal}` — `plan(req)`, `dedup_name`, `is_valid_name`,
  `build_undo`/`validate`/`to_requests`, `OpJournal`/`JournalWriter`/`scan`/`resume_plan`/
  `remove`.
- `core::ops` tipos: `OpKind`, `OpRequest`, `ConflictPolicy{Ask,Overwrite,Skip,Rename}`,
  `OpPlan`, `OpProgress`, `OpSummary`, `OpOutcome`, `ConflictPrompt{existing,incoming}`,
  `ConflictDecision{action,apply_all}`, `ConflictAction{Overwrite,Skip,Rename}`, `OpMsg`.
- `crates/ui/src/ops_actions.rs` es PURO y reusable tal cual: `transfer(kind_move, sources,
  dest)`, `delete(sources, to_trash)`, `rename(source, new_name)`, `create(dir, name,
  is_dir)` → arman el `OpRequest` con la `ConflictPolicy` correcta. (Se copia/mueve a
  ui-slint o se extrae a core; ver Riesgos.)
- `core::clipboard::decide_paste` + `platform::clipboard::{read, write_files}` (CF_HDROP +
  Preferred DropEffect: MOVE si cut, COPY si no).
- `platform::trash::move_to_trash(paths)` — papelera atómica (COM IFileOperation), FUERA del
  motor; se llama directo y se refresca.
- `platform::open::{open_default, open_with_dialog}`.
- `platform::context_menu::show_native_context_menu(hwnd, paths, x, y)` — menú del Shell.
- `core::keymap::Action::{Copy, Cut, Paste, Delete, DeletePermanent, Rename, NewFile, NewDir,
  CopyToOther, MoveToOther, Undo}` — ya en el keymap.

## 1. Arquitectura: módulo `ops_ctrl.rs`

Un módulo nuevo `crates/ui-slint/src/ops_ctrl.rs` con la struct `OpsCtrl`, dueña de TODO el
estado de operaciones. `WorkspaceCtrl` lo posee y le delega los gestos, pasándole la
selección (rutas) y la carpeta activa. Se separa de `workspace_ctrl.rs` porque ese ya está
grande y las ops son una responsabilidad distinta; `OpsCtrl` es testeable en aislamiento.

Regla de oro intacta: el hilo de UI nunca hace I/O. El motor corre en su hilo; el
`slint::Timer` (~30 ms, el mismo patrón de listados) drena el progreso y se apaga en reposo.

**Estado de `OpsCtrl`:**

```text
active_ops: Vec<ActiveOp>          // ops en curso/terminadas
pending_dialog: Option<OpDialog>   // UN modal a la vez
undo_history: Vec<UndoEntry>       // se MOVERÁ desde WorkspaceCtrl (hoy vacío en 2b)
next_undo_id: u64
cut_set: HashSet<PathBuf>          // rutas "cortadas" (corte visual)
pending_resume: Vec<OpJournal>     // ops a retomar (al arrancar la app)
config_dir: PathBuf                // para journal
```

`ActiveOp { rx: Option<Receiver<OpMsg>>, conflict_tx: Sender<ConflictDecision>, token,
label, progress: Option<OpProgress>, summary: Option<OpSummary>, started: bool,
pending: Option<(OpPlan, OpKind, ConflictPolicy)>, journal_id: Option<String>,
request: Option<OpRequest>, awaiting_conflict: Option<ConflictPrompt> }`.

`OpDialog` (enum, un modal a la vez):
- `ConfirmDelete { sources: Vec<PathBuf>, permanent: bool }`
- `Conflict { op_index: usize, prompt: ConflictPrompt }`  // ops-B, por-ítem
- `NameInput { purpose: NamePurpose, buf: String }`  // NewFile/NewDir/Rename
- `PastePreview { dir, name, ext, is_image, dims, fmt }`
- `Resume { items: Vec<OpJournal> }`

## 2. Cambio en core: conflicto por-ítem (ops-B)

Hoy el motor trata `Ask` como `Overwrite` y nunca emite `Conflict`. Se extiende
`engine.rs` para resolución INTERACTIVA por ítem:

- Al procesar un paso cuyo destino EXISTE y la política efectiva es `Ask`: emite
  `OpMsg::Conflict(ConflictPrompt{existing, incoming})` y **bloquea** en
  `conflict_rx.recv()` hasta recibir `ConflictDecision{action, apply_all}`.
- `action`: `Overwrite` reemplaza, `Skip` deja `OpOutcome::Skipped`, `Rename` usa
  `dedup_name`.
- `apply_all=true`: el motor memoriza `action` y la aplica a TODOS los choques siguientes sin
  volver a emitir `Conflict` (cambia su política efectiva interna a `Overwrite`/`Skip`/
  `Rename`).
- Cancelación durante la espera: mientras espera la decisión, si el token se cancela aborta
  limpio (usar `recv_timeout` en un bucle corto que chequea el token, o cerrar el canal).
- Si la política inicial NO es `Ask` (ya resuelta antes de spawn, o undo con `Skip`), el
  comportamiento es el de hoy (no pregunta).

Tests puros en core: choque único → cada acción produce el resultado esperado; `apply_all`
no vuelve a preguntar; cancelación durante la espera aborta. Se simula `conflict_rx` con un
canal que responde de inmediato.

**Pre-check de conflicto (en la UI, antes de spawn):** se conserva `first_collision` (lógica
de egui, se replica en `ops_ctrl`): si NO hay choques, se spawnea con `conflict=Overwrite`
(rápido, sin preguntar). Si HAY al menos un choque, se spawnea con `conflict=Ask` para que el
motor pregunte por ítem. Esto evita preguntar cuando no hace falta y habilita ops-B cuando sí.

## 3. Flujo de una operación

1. Gesto (tecla o menú) → `OpsCtrl` arma el `OpRequest` con `ops_actions::*`.
2. `Delete{to_trash:true}` → `platform::trash::move_to_trash` directo + refrescar + registrar
   undo (`TrashCreated` no aplica a papelera; la papelera es recuperable por el usuario desde
   Windows — NO se journalea ni se ofrece deshacer-a-papelera, igual que egui). Return.
3. `plan(&req)` → si `Err(PlanError)`, mostrar error discreto y return.
4. Pre-check: `first_collision` → fija `conflict = Ask` (hay choque) u `Overwrite` (no).
5. Si Copy/Move/Delete-permanente → `JournalWriter::new(config_dir, OpJournal::new(...))`.
6. Cola: si `ops_mode == Queue` y hay otra corriendo → push `ActiveOp{ started:false,
   pending:Some(...) }`. Si no → `engine::spawn(...)` con un `conflict_tx`/`conflict_rx`
   real, push `ActiveOp{ rx:Some, started:true }`.
7. `pump_ops` (en el Timer): drena cada `rx`:
   - `Progress(p)` → `op.progress = Some(p)`.
   - `Conflict(prompt)` → `op.awaiting_conflict = Some(prompt)`; abre
     `OpDialog::Conflict{op_index, prompt}` (si no hay otro modal).
   - `Done(s)|Cancelled(s)` → `build_undo(&req, &s)` → si `Some`, push `UndoEntry`;
     `journal::remove`; `op.summary = Some(s)`; `op.rx = None`; refrescar paneles afectados.
   - `Failed(e)` → marcar error en la op.
   - Tras drenar: si nada corre y hay `pending`, spawnear la siguiente de la cola.
8. El usuario resuelve el modal de conflicto → `ConflictDecision` se envía por
   `op.conflict_tx`; el motor continúa.

## 4. Diálogos (componentes Slint nuevos)

Overlay con velo (mismo patrón que el menú ▾ de 2b), UN modal a la vez, manejado por
`pending_dialog`. Cada uno: componente `.slint` + bridge de datos + callback de decisión.

- **`confirm-delete-dialog.slint`** — "Eliminar N elementos" + (papelera | permanente) →
  Eliminar / Cancelar. Permanente con texto de advertencia rojo.
- **`conflict-dialog.slint`** (ops-B) — "Ya existe «nombre»" → Sobrescribir / Saltar /
  Renombrar + checkbox "Aplicar a todos". Esc = Saltar.
- **`name-dialog.slint`** — título i18n (Nuevo archivo / Nueva carpeta / Renombrar) + campo
  de texto; OK deshabilitado si `is_valid_name` falla. Enter confirma, Esc cancela.
- **`paste-preview-dialog.slint`** — pegar texto/imagen del portapapeles: nombre editable +
  (si imagen) formato → Crear / Cancelar.
- **`resume-dialog.slint`** — al arrancar, si `scan(config_dir)` halló journals: lista (label,
  done/total) → Retomar / Descartar (por ítem o todas).

Cada modal expone su estado a Slint vía un struct (`*Vm`) y devuelve la decisión por callback
al `OpsCtrl`.

## 5. Panel de progreso

Overlay inferior, sobre la barra de estado (no roba área de los paneles). Modelo estable de
filas (`OpRowVm`): por op → etiqueta, % (de `bytes_done/bytes_total`), estado (en cola /
copiando… / hecho: N copiados, M saltados, K fallidos), botón ✕ cancelar. Se muestra solo si
`active_ops` no está vacío. Cancelar = `op.token.cancel()`. Las terminadas se conservan si
`settings.show_op_summary`, si no se podan. El Timer mantiene vivo el panel mientras haya ops
corriendo o con resumen visible.

## 6. Clipboard + corte visual

- `Copy` → `platform::clipboard::write_files(paths, cut=false)`; `cut_set.clear()`.
- `Cut` → `write_files(paths, cut=true)`; `cut_set = paths`. Las filas en `cut_set` se pintan
  ATENUADAS (flag `cut: bool` nuevo en `RowData`/`PlainRow`; el bridge lo marca consultando
  `cut_set`).
- `Paste` → `clipboard::read()` → `core::clipboard::decide_paste(content, dest)` → `PastePlan`:
  - Archivos (CF_HDROP) → `OpRequest` Copy/Move según el drop effect → flujo §3.
  - Texto/imagen → `OpDialog::PastePreview` → crear el archivo (worker corto).
- Al pegar OK o `Esc` → `cut_set.clear()`.

## 7. Menú contextual híbrido (clic derecho)

`context-menu.slint` propio (overlay posicionado en el clic) con las acciones de Naygo: Abrir,
Abrir con…, Copiar, Cortar, Pegar, Renombrar, Eliminar, Copiar ruta, + separador + **"Más
opciones de Windows…"**. Esta última invoca
`platform::context_menu::show_native_context_menu(hwnd, paths, x_pantalla, y_pantalla)`.

El HWND se obtiene del backend winit de Slint vía `window().window_handle()` +
`raw-window-handle` (feature ya disponible; se aísla en un helper `fn naygo_hwnd(window) ->
isize`). El menú nativo es modal/bloqueante del Shell; tras volver, refrescar el panel. Las
coords de pantalla = posición del clic + posición de la ventana.

Riesgo acotado: si no se puede obtener el HWND (otro backend), "Más opciones de Windows…" se
oculta; el resto del menú propio funciona igual.

## 8. Atajos

En `WorkspaceCtrl::on_key`, los `Action` de ops se enrutan a `OpsCtrl`: Copy/Cut/Paste,
Delete/DeletePermanent, Rename (inline si 1 ítem; batch-rename queda para una fase de pulido,
fuera de F3), NewFile/NewDir, CopyToOther/MoveToOther (reusan el selector 1..9 de 2c-i para el
destino), Undo (deshace la última `UndoEntry` deshacible).

## 9. Integración con paneles existentes

- **Historial (2b)**: hoy se pinta vacío. Ahora `OpsCtrl::undo_history` lo alimenta de verdad;
  el botón "Deshacer" del panel Historial llama `OpsCtrl::undo_entry(id)` (valida con
  `undo::validate`, re-emite con `to_requests`). Se mueve `undo_history` de `WorkspaceCtrl` a
  `OpsCtrl` (o `OpsCtrl` lo posee y `WorkspaceCtrl` lo lee para el bridge del panel).
- **Inspector/Preview**: sin cambios.
- **Refresco tras op**: al terminar una op, refrescar los paneles Files cuya carpeta fue
  afectada (origen y destino) re-listando.

## 10. Testing

- **core (engine ops-B)**: choque → Overwrite/Skip/Rename produce el resultado esperado;
  `apply_all` no re-pregunta; cancelación durante la espera aborta limpio. Canal simulado.
- **ops_ctrl**: el gesto arma el `OpRequest` correcto; pre-check `first_collision` elige
  Ask/Overwrite; transición de estados de `active_ops` (started/pending/summary); registro de
  `UndoEntry` al `Done`; `cut_set` en cut/paste/esc; encolado en modo Queue; deshacer valida y
  re-emite.
- **Verificación viva (Claude, en esta máquina)**: con Win32 PostMessage + PrintWindow (el
  método validado en F2): copiar/mover entre paneles, borrar a papelera y permanente,
  conflicto por-ítem con "aplicar a todos", nuevo archivo/carpeta, renombrar inline, progreso
  + cancelar, deshacer desde el Historial, corte atenuado, pegar, menú derecho propio + nativo.
- **Rendimiento (Nicolás, VM)**: CPU al operar y en reposo.

## 11. Puertas

`cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` +
`cargo fmt --all -- --check` antes de cada commit. Commits en español, rutas EXPLÍCITAS (sin
`CLAUDE.md` ni `graphify-out/`). `graphify update .` tras cambios de código. Sin merge a main
hasta el visto bueno de Nicolás; la FUNCIONALIDAD la verifica Claude en esta máquina antes de
pedir nada, Nicolás solo el rendimiento en la VM.

## 12. Riesgos / decisiones

- **ops-B toca el core**: es el único cambio a `engine.rs`. Acotado y testeable; la capa egui
  vieja NO usa el conflicto interactivo (sigue pasando `conflict` ya resuelto), así que no se
  rompe (su flujo pre-resuelve el conflicto antes de spawn; el `Ask` interactivo solo lo
  dispara la capa Slint). Verificar que egui compila y sus tests pasan.
- **`ops_actions.rs` vive en `crates/ui` (egui)**: para reusarlo desde ui-slint sin depender
  de egui, se MUEVE a `core::ops::actions` (es puro: arma `OpRequest`s). Cambio mecánico;
  egui pasa a importarlo de core. (Alternativa: duplicarlo en ui-slint; se prefiere mover para
  una sola fuente de verdad.)
- **HWND del backend winit**: acopla levemente; aislado en un helper y con fallback si falla.
- **Menú nativo bloqueante**: el Shell menu es modal; mientras está abierto la app se detiene
  (igual que egui). Aceptable.
- **Batch-rename (2+ ítems)**: fuera de F3 (es `core::batch_rename`, su propia fase de pulido).
  En F3, renombrar con multiselección renombra el ítem enfocado (o se deshabilita); se
  documenta.
- **Papelera no se deshace desde Naygo**: es recuperable desde Windows; no se journalea ni se
  ofrece undo (paridad con egui).

## Fuera de alcance (otras fases)
Configuración + editor de atajos (F4); OLE drag&drop / tray / splash / watcher (F5); batch-
rename avanzado, paleta de comandos, miniaturas (pulido). Retiro de egui (F6).
