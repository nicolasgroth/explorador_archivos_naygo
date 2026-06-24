# Operaciones de archivo robustas — Diseño

> Naygo (explorador Rust + Slint, render software, Windows). Autor: Nicolás Groth / ISGroth.
> Fecha: 2026-06-23. Origen: incidente real al copiar una carpeta grande que ya existía en el destino
> (la app se congeló "analizando", y el diálogo de conflicto no dejaba cancelar).

Cuatro cambios independientes que endurecen las operaciones de archivo (copiar/mover) y completan el
toolbar. El motor de copia (`core::ops::engine`) está SANO (tests lo prueban); el trabajo es de la
fase de planificación, la UI de conflictos y el toolbar.

---

## Problema 1 — Escaneo previo en segundo plano (no congelar la app)

### Diagnóstico
Al copiar/mover, `OpsCtrl::start_op` llama a `naygo_core::ops::plan()` de forma **síncrona en el hilo
de UI** (`ops_ctrl.rs:206`). Para una carpeta grande, `expand()` (`plan.rs:191`) recorre todo el árbol
con `read_dir`/`metadata` antes de spawnear el worker → el hilo de UI queda **congelado** (la ventana
no repinta) y no hay progreso ni forma de cancelar. Viola la regla de oro del proyecto (el hilo de UI
nunca hace I/O de disco).

### Solución
Mover la planificación a un worker, siguiendo el molde ya existente de `search::spawn_search`
(`search.rs:111`) y `deep_listing::spawn_deep_listing` (`deep_listing.rs:47`): recorren árboles en un
hilo, con `CancellationToken`, emitiendo mensajes incrementales por canal.

- **Core**: nueva `ops::spawn_plan(req, token) -> Receiver<PlanMsg>` (en `plan.rs` o un módulo nuevo
  `plan_async`). `PlanMsg::Progress { files, bytes }` emitido con throttle (~cada 100 ms o cada N
  archivos) durante el walk; `PlanMsg::Done(Plan)` al terminar; respeta `token.is_cancelled()` para
  abortar el walk limpio. La fn pura `expand`/`plan` existente se reusa internamente (no se duplica la
  lógica), solo se la envuelve para emitir y chequear cancelación.
- **UI (`ops_ctrl.rs`)**: `start_op` deja de llamar `plan()` inline. En su lugar registra una
  operación en estado **"Calculando…"** y spawnea `spawn_plan`. `pump_ops` drena `PlanMsg`: actualiza
  el panel con "Calculando… N archivos, M GB" y, al `Done`, hace el pre-check de conflicto
  (`first_collision`) y procede a `spawn_op` con el plan ya hecho. Si llega un conflicto de carpeta
  (Problema 3) se resuelve ahí.
- **Cancelación**: la operación "Calculando…" tiene su `CancellationToken` desde el inicio; el botón
  Cancelar del panel la aborta aunque aún no haya empezado a copiar.

### UI
El panel de Operaciones muestra la fase "Calculando…" con el contador de archivos/bytes escaneados y
el botón Cancelar activo. Al terminar el escaneo, transiciona a la fase de copia normal (barra %,
velocidad, ETA) que ya existe.

---

## Problema 2 — Poder cancelar TODO desde el diálogo de conflicto (BUG)

### Diagnóstico
El modal de conflicto (`op-dialogs.slint:131-137`, `kind==2`) solo tiene Saltar / Renombrar /
Sobrescribir. No hay Cancelar. El clic en el velo = Saltar (`:64`); Escape ignora `kind==2`
(`:218`). Peor: el velo `#000000aa` cubre toda la ventana y **tapa el botón Cancelar del panel de
operaciones**, así que el usuario queda atrapado: ninguna salida aborta la operación.

A nivel motor, cancelar SÍ funciona aun con un conflicto pendiente (`engine.rs:289-292` retorna
`Skipped` si el token está cancelado mientras espera la decisión). El bug es puramente de UI: falta el
camino para disparar ese cancel.

### Solución
- **`op-dialogs.slint`**: agregar botón **"Cancelar todo"** al modal de conflicto (`kind==2`) que
  emita un callback nuevo `conflict-cancel()`. Incluir `kind==2` en el manejo de **Escape** del
  FocusScope (`:218`) → `conflict-cancel()`. Cambiar el clic-en-velo del conflicto para que cancele
  en vez de saltar (decisión: cerrar por fuera un conflicto = cancelar la operación, más seguro que
  saltar silenciosamente).
- **`ops_ctrl.rs`**: nuevo `cancel_conflict(op_id)` (junto a `resolve_conflict`, `:434`) que hace
  `op.token.cancel()` + limpia `awaiting_conflict`/`pending_dialog` + cierra el modal. (Hoy
  `resolve_conflict` siempre manda una `ConflictDecision`; falta la vía que cancela sin decidir.)
- **`main.rs`**: cablear `on_conflict_cancel` → `ops.cancel_conflict(op_id)`.
- La copia cancelada borra su parcial (comportamiento del motor ya existente).

---

## Problema 3 — Conflicto a nivel de CARPETA (mejora)

### Diagnóstico
Hoy el conflicto es siempre por archivo: `expand` aplana la carpeta a steps de archivo, los
directorios se fusionan en silencio (`engine.rs:251`) y solo se pregunta por cada archivo que choca
(mitigado por "aplicar a todos"). Copiar una carpeta de 56 GB que ya existe pregunta archivo por
archivo.

### Solución
Antes de planear/copiar, si el destino ya contiene una carpeta con el nombre de un origen que es
directorio, mostrar **una sola vez** un diálogo de conflicto de carpeta:

> La carpeta «X» ya existe en el destino. ¿Qué hacer?
> **Fusionar** · **Reemplazar** · **Saltar** · **Cancelar**

- **Fusionar**: copia dentro de la existente; los archivos que choquen disparan el conflicto por
  archivo que ya existe (política `Ask`). Es el comportamiento actual.
- **Reemplazar**: **borra la carpeta destino** y copia limpia. Operación destructiva → se hace dentro
  del worker, es cancelable, y se ejecuta como un paso previo del plan (un `OpStep` de borrado del
  directorio destino, o un `remove_dir_all` controlado por el motor antes de copiar ese subárbol).
- **Saltar**: no copia esa carpeta (continúa con los demás orígenes si los hay).
- **Cancelar**: aborta toda la operación.

Con varias carpetas en conflicto, la casilla "Aplicar a todas las carpetas" replica la decisión.

### Core / UI
- **Core**: `Plan`/`OpRequest` gana una `FolderPolicy` (Merge/Replace/Skip) por carpeta raíz en
  conflicto, o un paso de "reemplazar" (borrar destino) en el plan. `first_collision` ya detecta que
  la carpeta raíz existe (`ops_ctrl.rs:178`); se amplía para distinguir colisión de archivo vs de
  carpeta y disparar el diálogo correcto.
- **UI**: nuevo `OpDialog::FolderConflict` (análogo a `Conflict`) + su tarjeta en `op-dialogs.slint`
  con los 4 botones + casilla "aplicar a todas". El flujo: tras el escaneo (Problema 1), si hay
  colisión de carpeta → mostrar este diálogo ANTES de copiar; la decisión ajusta el plan.
- **Reemplazar** borra el destino: confirmar que el borrado es cancelable y que un fallo (permiso,
  archivo en uso) se comunica sin caer (Result tipado).

---

## Problema 4 — Botón "crear carpeta(s)" en el toolbar — YA EXISTE

### Diagnóstico
Verificado en el código: **tanto el atajo como el botón del toolbar ya existen**.
- Botón en `app-window.slint:1175-1182`: ícono `ic-new-folder`, tooltip `Tr.toolbar-new-folder-tip`,
  dispara `new-folder-toolbar()` (abre el modal de nueva carpeta multilínea en el panel activo).
- El atajo de crear carpeta existe en el keymap y es configurable.
- Soporta múltiples carpetas y anidadas (multilínea, `\`) — tarea #87.

### Solución
NADA que implementar. Solo documentar en la guía de usuario que crear carpeta(s) está disponible
desde el toolbar (ícono) y por atajo configurable. Este problema sale del alcance de implementación.

---

## Testing
- P1: `spawn_plan` emite progreso incremental y es cancelable (test con un árbol temporal: cuenta los
  PlanMsg, cancela a mitad y verifica abort). La fn pura `expand` ya está testeada.
- P2: `cancel_conflict` cancela el token y limpia el estado (test del controlador). El motor ya tiene
  `cancelar_durante_espera_aborta`.
- P3: la decisión de carpeta ajusta el plan (Merge no borra; Replace agrega el paso de borrado; Skip
  excluye el subárbol). Test de la lógica de plan. "Reemplazar" borra el destino antes de copiar
  (test con carpeta destino poblada → tras Replace, solo está el contenido del origen).
- P4: el botón dispara la misma acción que el atajo (verificación de cableado).
- UI (modales, panel "Calculando…", botón): verificación visual en la VM.

## i18n
Triple (es + en + i18n.slint + i18n_keys.rs), español neutral SIN voseo, sin reusar claves. Nuevas:
"Calculando…", "Cancelar todo", "La carpeta «{}» ya existe", "Fusionar", "Reemplazar", "Saltar"
(reusar si ya existe), "Aplicar a todas las carpetas", tooltip "Nueva carpeta".

## Orden de implementación
1. **P2 (cancelar conflicto)**: el bug más directo y de mayor impacto inmediato (desatasca al usuario).
2. **P1 (escaneo en background)**: arregla el congelamiento; la pieza más arquitectural.
3. **P3 (conflicto por carpeta)**: la mejora más grande (modelo + diálogo + borrado).
4. **P4 (botón toolbar)**: ya existe — solo se documenta en la guía.
5. Cierre: gate + CHANGELOG + guía + dist.

## Fuera de alcance (YAGNI)
- Reanudar una copia interrumpida (resume) — no se pidió.
- Verificación por hash tras copiar — no se pidió.
- Cambiar el motor de copia (está sano).
