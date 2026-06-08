# Naygo — Fase ops-A: operaciones de archivo (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-07. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Primera fase del sprint de **funcionalidad** (ops-A → ops-B journal → paste inteligente
→ platform/shell → watcher → sizing). Hasta ahora Naygo navega, ve y configura, pero NO
opera archivos. Esta fase agrega el set completo de operaciones.

**Premisa rectora (no negociable):** el hilo de UI NUNCA hace I/O. Toda operación larga
corre en un worker async que se comunica por canal (mpsc) y recibe un
`CancellationToken` (ya existe `core::cancel`). Toda op es **cancelable** y aborta limpio
(una copia cancelada borra el parcial). El filesystem es hostil (permisos, rutas que
desaparecen, discos de red, disco lleno): `Result` tipado, errores comunicados al panel,
nunca panic.

### Operaciones (set completo)

Copiar, mover, eliminar (a papelera de Windows **o** permanente), renombrar, crear
carpeta, crear archivo vacío. Las largas (copiar/mover/eliminar de muchos) muestran
progreso; las cortas (renombrar/crear) son instantáneas.

### Decisiones tomadas en el brainstorm

1. **Disparadores (las 4 vías):**
   - Atajos estándar: Ctrl+C / Ctrl+X / Ctrl+V (copiar/cortar/pegar **entre paneles de
     Naygo** vía clipboard interno), Supr (a papelera), Shift+Supr (permanente), F2
     (renombrar), Ctrl+N (nuevo archivo) / Ctrl+Shift+N (nueva carpeta).
   - Botones en la toolbar.
   - Menú contextual PROPIO de Naygo (clic derecho sobre archivos). (El menú contextual
     NATIVO de Windows es la fase platform/shell posterior.)
   - Entre paneles estilo Commander: F5 copiar / F6 mover (del panel activo al otro).
2. **Política de conflictos:** diálogo **Sobrescribir / Saltar / Renombrar** con opción
   **"aplicar a todos"** (para ops de muchos archivos). Renombrar = `archivo (2).ext`.
3. **Confirmación de borrado:** **configurable**. Default: papelera NO confirma
   (recuperable), permanente SÍ confirma (irreversible). Un toggle en config puede
   forzar confirmación también para la papelera.
4. **Panel de operaciones** (acoplado abajo, **oculto por defecto, aparece al operar**):
   - **Compacto por defecto + botón expandir** a vista detallada por op.
   - Detallado muestra: velocidad lectura/escritura, bytes transferidos (KB/MB/GB),
     **barra animada ligada a los bytes reales** (no spinner falso), ETA, archivo actual.
   - **Lista de cola** visible, cada op cancelable individualmente (incluso antes de
     empezar).
   - Modo **encolar (default) / paralelo**, configurable. (Paralelo: N workers; útil para
     muchos archivos chicos; en un HDD suele ser más lento — por eso cola es el default.)
   - Configurable: panel acoplado / diálogo modal / siempre visible.
5. **Resumen final** (configurable mostrarlo): hecho / omitido / con error, totales
   (archivos, bytes, tiempo) + **Ver detalle** + **Exportar** a archivo (.txt/.csv).
6. **Recuperación NIVEL SIMPLE** (en ops-A): si se cancela o algo falla a mitad, el
   resumen lista hecho/pendiente/fallado y se puede **reintentar u omitir** los
   pendientes EN ESA sesión. El journal en disco + retomar-tras-crash es **ops-B**
   (fase siguiente, fuera de ops-A).
7. **Clipboard interno** de Naygo (Ctrl+C/X/V): guarda `{paths, cut}` en memoria, para
   copiar/mover entre paneles. El portapapeles del SO (texto/imagen/etc.) es la fase
   **paste inteligente** siguiente, NO ops-A.

### Qué NO entra en ops-A

- Journal en disco / retomar-tras-crash (ops-B).
- Leer el portapapeles del SO / paste inteligente (fase siguiente).
- Menú contextual NATIVO de Windows, ShellExecute (platform/shell).
- Drag&drop COM/OLE con el SO.
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

Idea rectora: `core::ops` define el modelo + la **planificación pura** (testeable) + el
**motor (worker)** cancelable; `platform` aporta la papelera (Win32); `ui` dispara,
confirma y pinta. La UI nunca bloquea (worker + canal, patrón `listing`).

### Capa `core` — módulo nuevo `ops`

```rust
/// Qué operación.
pub enum OpKind {
    Copy,
    Move,
    Delete { to_trash: bool },
    Rename { new_name: String },
    CreateDir { name: String },
    CreateFile { name: String },
}

/// Qué hacer ante un nombre que ya existe en el destino.
pub enum ConflictPolicy { Ask, Overwrite, Skip, Rename }

/// Solicitud de operación (lo que la UI arma desde la selección).
pub struct OpRequest {
    pub kind: OpKind,
    pub sources: Vec<PathBuf>,
    pub dest_dir: Option<PathBuf>, // carpeta destino (Copy/Move); None para Delete/Create
    pub conflict: ConflictPolicy,
}

/// Un paso concreto (un archivo a copiar/mover, una carpeta a crear). Producto de la
/// planificación pura.
pub struct OpStep { pub from: Option<PathBuf>, pub to: PathBuf, pub bytes: u64, pub is_dir: bool }

/// Plan completo: pasos + totales. Lo produce `plan(&OpRequest, &dyn Fs) -> Result<OpPlan>`.
pub struct OpPlan { pub steps: Vec<OpStep>, pub total_bytes: u64, pub total_files: usize }
```

- **Planificación pura** (`plan`): expande carpetas recursivamente → `Vec<OpStep>` con
  bytes por archivo y total; resuelve nombres de conflicto (`archivo (2).ext`,
  incrementando); detecta y RECHAZA "copiar/mover una carpeta dentro de sí misma";
  decide mover mismo-volumen (rename atómico) vs cruzado (copiar+borrar). Para testear
  sin disco real, `plan` puede tomar un trait `Fs` mínimo (o trabajar sobre listas de
  entradas ya provistas); el motor usa la implementación real.
- **Motor (worker)** `spawn(plan, token, conflict_rx) -> (Receiver<OpMsg>, JoinHandle)`:
  ejecuta los pasos, copiando archivos **por buffers** (chequea el token entre bloques,
  no solo entre archivos → un archivo de GBs también es cancelable a media copia). Emite:
  ```rust
  pub enum OpMsg {
      Progress(OpProgress),     // bytes hechos, archivo actual, archivos hechos
      Conflict(ConflictPrompt), // pide decisión a la UI (cuando policy = Ask)
      Done(OpSummary),
      Error(OpError),           // error de UN paso; la op continúa con los demás
      Cancelled(OpSummary),
  }
  ```
  Cancelar borra el parcial del archivo en curso. Un error de un paso NO aborta la op:
  se registra y se sigue (el resumen lista los fallados).
- **`OpProgress`**: `{ bytes_done, bytes_total, files_done, files_total, current: PathBuf }`.
  La velocidad/ETA las calcula la UI con una ventana móvil (o el worker; decidir en el
  plan — la UI es más simple y no carga al worker).
- **`OpSummary`**: por archivo → `Done`/`Skipped`/`Failed(reason)`; totales. Para el
  resumen final + exportar.
- **Conflicto (policy = Ask):** el worker emite `Conflict(prompt)` y ESPERA por un canal
  de respuesta (`conflict_rx`) la decisión de la UI (Overwrite/Skip/Rename + apply_all).
  El worker bloquea SU hilo esperando (no el de UI); la UI responde cuando el usuario
  elige en el modal.

### Capa `platform` — papelera (nuevo)

`fn move_to_trash(paths: &[PathBuf]) -> Result<(), TrashError>` vía Win32
`IFileOperation` (COM; la API moderna recomendada sobre `SHFileOperationW`; soporta
papelera y multi-archivo). El borrado permanente lo hace `core::ops` con `std::fs`.
`cfg(not(windows))`: stub que devuelve error "no soportado" (o borra permanente, a
decidir en el plan; para tests, stub que no toca disco).

### Capa `ui`

- **`ops_panel.rs` (nuevo)**: panel acoplado abajo. Compacto (una línea: barra+%+
  velocidad+✕) con botón expandir a detallado (velocidad/bytes/ETA/archivo actual);
  lista de cola; resumen final con Ver detalle + Exportar; barra animada ligada a bytes.
  Oculto si no hay ops y la config no es "siempre visible".
- **`ops_dialogs.rs` (nuevo)**: modal de confirmación de borrado (según config) y modal
  de conflicto (Sobrescribir/Saltar/Renombrar + aplicar-a-todos). Responden al worker
  por el canal de conflicto.
- **`app.rs`**: `active_ops: Vec<ActiveOp>` (cada uno: `OpMsg` rx + conflict tx + token +
  estado/progreso + summary); `pump_ops()` cada frame (como `pump_all`); gestión cola vs
  paralelo; clipboard interno `{paths, cut}`; conexión de disparadores. Repaint mientras
  haya ops activas.
- **`input::Action`**: gana `Copy, Cut, Paste, Delete, DeletePermanent, Rename, NewFile,
  NewDir, CopyToOther, MoveToOther`.
- **`toolbar.rs`**: botones de ops.
- **menú contextual**: un menú egui (popup) al clic derecho sobre filas del file panel,
  con las ops.
- **`config`**: `Settings` gana `ops_mode (Queue/Parallel)`, `ops_display (Panel/Modal/
  AlwaysVisible)`, `confirm_trash (bool)`, `show_op_summary (bool)`, todos
  `#[serde(default)]`.

### Lo que NO cambia

Listing, árbol, columnas, temas. El file panel gana selección-para-ops (ya tiene
`selected: Vec<usize>`) y el menú contextual.

---

## 3. Flujo de datos

Disparar (cualquier vía → mismo camino): el disparador arma una `OpRequest` (kind +
sources de la selección del panel activo, o el foco si no hay selección; dest = otro
panel para F5/F6 o la carpeta actual para pegar) → `NaygoApp` la encola/lanza → crea
`ActiveOp` + spawnea worker → el panel aparece.

Durante (cada frame, no bloquea): `pump_ops` drena canales. `Progress` → actualiza
barra/velocidad/bytes/ETA/archivo. `Conflict` → abre modal; al responder, envía la
decisión por `conflict tx` (el worker, que esperaba, continúa). `Done`/`Cancelled` →
mueve a completada + resumen. `Error` → registra el paso fallado, la op sigue.

Cola vs paralelo: cola = 1 op activa, resto en espera (cancelables); paralelo = N
workers. Configurable.

Cancelar: ✕ → `token.cancel()`; worker aborta entre archivos/bloques, borra parcial,
emite `Cancelled` con summary parcial (para reintentar/omitir en sesión).

Resumen + exportar: al terminar, `OpSummary` en el panel; Exportar escribe .txt/.csv vía
worker.

Clipboard interno: Ctrl+C/X guarda `{paths, cut}`; Ctrl+V arma OpRequest Copy/Move a la
carpeta del panel activo y limpia el "cut" tras mover.

---

## 4. Manejo de errores / casos límite

- Permiso denegado / archivo bloqueado a mitad → ese paso = `Failed`; la op CONTINÚA;
  resumen lo lista (reintentar/omitir).
- Origen/destino desaparece durante la op → error de ese paso, continúa.
- Copiar/mover carpeta dentro de sí misma → detectado en `plan` (puro), rechazado antes
  de empezar con error claro.
- Mover mismo-volumen = rename atómico; entre volúmenes = copiar+borrar con progreso.
- Disco lleno a mitad → error; parcial del archivo en curso borrado; resumen lo refleja.
- Renombrar/crear con nombre que ya existe → conflicto/ error según corresponda.
- Cancelar a media copia de archivo grande → parcial borrado (no dejar basura).
- Nombre inválido (caracteres prohibidos en Windows: `\ / : * ? " < > |`) → validación
  en `plan`, error claro, no cae.
- Papelera no disponible (disco de red) → `IFileOperation` informa; se reporta / se
  ofrece permanente; sin caer.
- Exportar resumen a ruta no escribible → error discreto, no cae.

---

## 5. Testing

- **`core::ops` planificación** (el grueso, puro): expandir carpeta→pasos+bytes; conflicto
  (`archivo (2).ext`, incrementar si `(2)` también existe); detección carpeta-dentro-de-
  sí-misma; mover mismo-volumen vs cruzado; validación de nombres (caracteres prohibidos).
- **`core::ops` motor** (con `tempfile`, disco real en tempdir): copiar/mover/eliminar;
  cancelación a mitad borra parcial; conflicto Skip/Overwrite/Rename; un paso fallado no
  aborta la op; `OpSummary` correcto (hecho/omitido/error + totales).
- **`platform::move_to_trash`**: smoke en Windows (temp → papelera → ya no está en sitio);
  stub en no-Windows.
- **Funciones puras de la UI** (qué `OpRequest` produce cada disparador; cálculo de
  velocidad/ETA si se extrae): testeables sin egui.
- **UI** (panel compacto/expandir, cola, modales, animación, resumen, exportar):
  validación manual; la lógica con estado vive en core/puras.

Meta de siempre: build limpio + tests + clippy antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── ops/
│   ├── mod.rs        # OpKind, ConflictPolicy, OpRequest, OpStep, OpPlan, OpMsg, OpProgress, OpSummary
│   ├── plan.rs       # planificación pura: plan(), resolución de conflictos, validaciones
│   └── engine.rs     # motor worker: spawn(), copia por buffers, cancelación, conflict_rx
├── config/mod.rs     # + Settings.{ops_mode, ops_display, confirm_trash, show_op_summary}
├── lib.rs            # + pub mod ops; re-exports
└── i18n/{es,en}.json # + claves de ops (botones, diálogos, panel, resumen)

crates/platform/src/
├── trash.rs          # NUEVO: move_to_trash (Win32 IFileOperation; stub no-win)
├── lib.rs            # + pub mod trash;
└── (Cargo.toml)      # + features Win32 necesarias (Com, UI_Shell, etc.)

crates/ui/src/
├── ops_panel.rs      # NUEVO: panel de operaciones (compacto/expandir, cola, resumen)
├── ops_dialogs.rs    # NUEVO: modales de confirmación y conflicto
├── ops_actions.rs    # NUEVO (opcional): OpRequest desde disparadores (puro, testeable)
├── app.rs            # + active_ops + pump_ops + clipboard interno + disparadores
├── input.rs          # + variantes de Action de ops + mapeo de teclas
├── toolbar.rs        # + botones de ops
├── panes/file_panel.rs # + menú contextual (clic derecho) con las ops
└── main.rs           # + mod ops_panel; ops_dialogs; ops_actions;
```

---

## 7. Dependencias

`platform` usa el crate `windows` (ya presente) con features Win32 para COM +
`IFileOperation` (`Win32_System_Com`, `Win32_UI_Shell`, etc. — confirmar al implementar).
`core::ops` solo std. UI: egui. Exportar resumen: std (`std::fs`). Sin dependencias de
terceros nuevas.

---

## Fuera de alcance (recordatorio)

Journal en disco / retomar-tras-crash (ops-B), portapapeles del SO / paste inteligente,
menú contextual nativo / ShellExecute (platform/shell), drag&drop COM. Nunca:
reproducción de media, edición de archivos.
