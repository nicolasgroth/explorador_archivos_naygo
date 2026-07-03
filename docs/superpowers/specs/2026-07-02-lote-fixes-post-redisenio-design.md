# Lote de fixes post-rediseño — Diseño

> Autor: Nicolás Groth <ngroth@gmail.com> · ISGroth · 2026 · MIT
> Fecha: 2026-07-02

## Contexto

Tras validar en la VM el rediseño del panel de operaciones (rama `feat/ajustes-post-030`),
Nicolás reportó 8 hallazgos. Uno es un **bug de pérdida de datos** (prioridad absoluta); el resto
son bugs de comportamiento y mejoras. Este lote va en la MISMA rama. Cada punto se diseñó a partir
de una investigación previa (5 subagentes de exploración; hallazgos citados abajo con archivo:línea).

Fuera de alcance de este lote (decidido con Nicolás): (a) formatos Office VIEJOS binarios OLE
(DOC/XLS/PPT pre-2007) — se difieren por complejidad y crates puro-Rust pobres; (b) extender el
lenguaje visual del panel de operaciones a otros paneles — se discutirá en una conversación aparte.

## Fase 1 — Bug crítico: copiar con Ctrl movía los archivos (pérdida de datos)

**Síntoma**: arrastrar archivos entre dos paneles con **Ctrl** presionado (cursor mostraba "+" de
copiar), en el MISMO disco, los MOVÍA en vez de copiarlos. El historial lo reportaba mal.

**Causa raíz (confirmada)**: el drop OLE solo transmite el estado de **Shift**, nunca **Ctrl**.
- `crates/platform/src/drop_target.rs:228` — `let move_ = (grfkeystate.0 & MK_SHIFT.0) != 0;`.
  `MK_CONTROL` no se importa (línea 120) ni se usa.
- Durante `DoDragDrop` (bucle modal del SO) la app NO recibe eventos de teclado, así que
  `ctrl_down`/`shift_down` del controlador quedan **congelados en false**
  (`workspace_ctrl/input.rs:109-152`). El código ya lo reconoce para Shift (comentario en
  `ops.rs:281-285`) y lo resuelve pasando `move_hint` desde el OLE, pero **NO hizo lo mismo para
  Ctrl**.
- Resultado: Ctrl+arrastre en mismo disco → `move_hint=false` + `ctrl=false` (stale) +
  `same_drive=true` → `decide_drop_action(false,false,true)` = **Move** (`core/dnd.rs:21-31`, lógica
  correcta pero alimentada con datos stale) → el motor hace `fs::rename` (`ops/engine.rs:496`).
  El cursor mostró "+" porque `effect_for` (`drop_target.rs:143`) pinta COPY sin Shift, pero el
  payload nunca transmitió que se pidió copia.

**Diseño del fix** (simétrico al de Shift, sin inventar nada):
1. En `drop_target.rs`: importar `MK_CONTROL`; leer `copy_forced = (grfkeystate.0 & MK_CONTROL.0) != 0`
   en el `Drop`. Añadir `copy_forced: bool` al `DropPayload` (junto a `move_`). En `effect_for`
   (que decide el cursor en `DragOver`), Ctrl debe dar `DROPEFFECT_COPY` explícito (ya lo hace por
   defecto, pero dejarlo explícito para coherencia con el payload).
2. En `main.rs` (donde se recibe el drop, ~1403): pasar `payload.copy_forced` a `drop_at`.
3. En `workspace_ctrl/ops.rs` `drop_at`: la decisión pasa a considerar el **grfKeyState real del
   OLE** por sobre el estado stale del teclado. Regla final (prioridad):
   `move_hint` (Shift del OLE) → Move; **`copy_forced` (Ctrl del OLE) → Copy**; luego la regla de
   `decide_drop_action` (mismo-disco → Move; distinto → Copy). Es decir: si el OLE dice Shift, mueve;
   si dice Ctrl, copia; si ninguno, aplica la heurística de disco. Los flags stale `ctrl`/`shift`
   dejan de decidir el caso del arrastre (siguen valiendo para otros llamadores no-drag si los hay,
   pero para el drop manda el OLE).
4. El `is_move` resultante alimenta tanto la ejecución como el `label`/registro del historial, así
   que arreglar la decisión arregla también el texto del historial (que decía mal el tipo).

**Testing**: test unitario de la tabla de decisión del drop con las 4 combinaciones
(ninguno/Shift/Ctrl/ambos) × (mismo disco / distinto disco), aseverando Copy vs Move esperado.
Debe cubrir explícitamente el caso del bug: Ctrl + mismo disco = **Copy**. Si la función de decisión
es `decide_drop_action` en core, extenderla o añadir una capa que combine (move_hint, copy_forced,
decide_drop_action) y testear ESA capa.

**Nota de seguridad**: este es un fix de pérdida de datos; se implementa PRIMERO y se verifica en la
VM antes que el resto del lote.

## Fase 2 — Historial: fecha del registro + orden (nuevos arriba)

Hay DOS historiales visibles (Nicolás mostró ambos):
- **Panel de operaciones** ("Historial reciente"): filas `OpRowVm` con `kind==2`, generadas desde
  `active_ops` terminadas (con `summary`) en `ops_ctrl.rs` (~1516).
- **Panel "Historial de acciones"** (undo): `HistRow`/`UndoEntry`, armado en `bridge.rs:499`
  (`history_rows`). Su struct ya contempla un "cuándo" (comentario `bridge.rs:475`) — verificar si
  ya expone fecha y solo falta mostrarla, o si hay que añadirla.

**Fecha**:
- El motor de ops usa `started_at: Option<Instant>` (reloj MONÓTONO — NO sirve para mostrar fecha
  de calendario). Para la fecha del registro se necesita un timestamp de **wall-clock** (epoch
  segundos, i64). Añadir a la op / entrada de historial un campo de "cuándo terminó" en epoch
  (obtenido con `SystemTime::now()` al finalizar), y mostrarlo formateado con la función existente
  `core::format::format_time(Option<i64>, DateFormat)` (`format.rs:121`), respetando el
  `DateFormat` de la config (como las fechas de la columna "Modificado").
- Añadir el campo de fecha (string ya formateado o epoch) al `OpRowVm` y al `HistRow`, y pintarlo en
  cada fila de ambos paneles, discreto (`Theme.text-dim`, tamaño pequeño), alineado a la derecha o
  bajo la etiqueta.

**Orden (nuevos arriba)**:
- Panel de operaciones: las filas de historial (`kind==2`) deben quedar con la más RECIENTE arriba.
  Hoy salen en orden de inserción; invertir solo el tramo de historial al construir las filas.
- Panel de acciones (undo): `undo_history` se llena con `push` y se poda con `remove(0)` a 100
  entradas (`ops_ctrl.rs:934-943`); `history_rows` las recorre en ese orden. Invertir el orden de
  presentación (más reciente primero) sin cambiar la semántica del undo (Ctrl+Z sigue deshaciendo
  la última acción).
- CUIDADO: invertir el orden de PRESENTACIÓN, no el de la estructura interna que usa el undo. Si el
  undo depende del orden del `Vec`, solo invertir en la capa de filas (`history_rows`/construcción
  de `OpRowVm`), no en `undo_history` mismo.

**Testing**: test de que, dadas N ops/entradas, la primera fila del historial corresponde a la más
reciente; y de que la fecha se formatea con la clave/función correcta (comparar contra la salida de
`format_time`, no un literal — CI en inglés).

## Fase 3 — Path-bar: seleccionar todo + alinear a la izquierda al editar

Al hacer clic en la barra de ruta para editarla a mano, el texto debe: (a) aparecer TODO
seleccionado (borrar/reemplazar escribiendo), y (b) alineado a la IZQUIERDA (hoy centrado).

- **Seleccionar todo**: añadir `self.select-all()` al `init` del `LineEdit` de edición del path
  (`file-panel.slint:512`, hoy `init => { self.focus(); }`). Patrón YA usado en el rename inline
  (`file-panel.slint:672`: `init => { self.focus(); self.select-all(); }`).
- **Alinear izquierda**: el `LineEdit` centra por defecto (estilo fluent de Slint 1.16). Envolverlo
  en un `HorizontalLayout { padding-left: 4px; ... LineEdit { horizontal-stretch: 1; ... } }` (como
  los breadcrumbs), o usar la propiedad de alineación de texto si la versión la soporta. No romper
  el commit (Enter) / cancel (Esc) / cierre por pérdida de foco existentes.

**Testing**: es UI declarativa; se valida visualmente en la VM. Sin test unitario nuevo.

## Fase 4 — Metadata ampliada (audio, Office moderno, PDF)

Sobre el sistema `core::metadata` ya existente (trait `MetadataProvider` = `extensions()` +
`read(&Path) -> Vec<MetadataField>`; registro en `ensure_core_providers`; tolerante, nunca panic;
`MetadataField { label_key, value }` con label_key = clave i18n). Todos los providers nuevos van en
`core` (son puro-Rust) y se registran en `ensure_core_providers`. Se muestran en Propiedades y
Preview (worker ya cableado en `workspace_ctrl/meta.rs`).

**Audio** — crate `lofty` (pura-Rust, MIT/Apache, ~1.2 MB, lee solo la cabecera, multi-formato):
- Extensiones: `mp3, flac, ogg, m4a, wav` (lofty las cubre todas con una sola API).
- Campos técnicos: duración (HH:MM:SS), bitrate (kbps), sample rate (Hz), canales (mono/estéreo/…).
- Campos de contenido (ID3/tags), si existen: artista, título, álbum, año. Los campos vacíos NO se
  muestran (el provider omite los que vengan vacíos).
- Nuevo `audio_meta.rs` en core. Añadir `lofty` a `crates/core/Cargo.toml` (verificar versión/licencia
  al implementar; confirmar que NO arrastra dependencias nativas/C).

**Office moderno** — un SOLO provider compartido (docx/xlsx/pptx son ZIP+XML, misma estructura):
- Extensiones: `docx, xlsx, pptx`.
- Leer con `zip` (ya presente) el `docProps/core.xml` (autor `dc:creator`, título `dc:title`, fechas
  `dcterms:created`/`dcterms:modified`) y `docProps/app.xml` (por tipo: palabras/páginas para docx,
  nº de hojas para xlsx, nº de diapositivas para pptx). Parsear el XML con `quick-xml` (ya transitivo;
  añadirlo como dependencia EXPLÍCITA de core si hace falta).
- Campos: autor, título, fecha de modificación, y el contador propio del tipo (palabras / hojas /
  diapositivas). Omitir los vacíos.
- Nuevo `office_meta.rs` en core. SIN dependencias nuevas más allá de exponer `quick-xml`.
- Tolerante: un ZIP corrupto o sin `docProps` devuelve vacío, nunca panic.

**PDF** — crate puro-Rust a CONFIRMAR en el plan (candidata: `lopdf`, MIT; evaluar que sea puro-Rust
y su peso; si `lopdf` resultara pesada o con deps dudosas, evaluar `pdf` u otra, o diferir PDF):
- Extensión: `pdf`.
- Campos: número de páginas, autor, título (del diccionario `/Info` y el catálogo de páginas).
- Nuevo `pdf_meta.rs` en core. Añadir la crate elegida a `crates/core/Cargo.toml`.
- Si al implementar NINGUNA crate puro-Rust de PDF cumple la política (puro-Rust, sin C, MIT/Apache)
  con peso razonable, DIFERIR PDF y dejar constancia (no bloquear el resto del lote por PDF).

**i18n**: claves nuevas en los 10 idiomas para cada etiqueta:
`meta.duration, meta.bitrate, meta.sample_rate, meta.channels, meta.artist, meta.title, meta.album,
meta.year, meta.author, meta.modified, meta.word_count, meta.sheet_count, meta.slide_count,
meta.pages`. (Reutilizar `meta.title` para audio y office/pdf.) Respetar el test de paridad i18n.

**Testing**: por provider, un test con un fixture mínimo generado en el test (un mp3/wav mínimo con
lofty si la crate lo permite, o un archivo de fixture; un .docx/.xlsx/.pptx mínimo armado como zip
con un `docProps/core.xml` embebido; un pdf mínimo). Aseverar que `metadata_for` devuelve los campos
esperados comparando `label_key` contra la CLAVE (no el literal). Tolerancia: archivo inexistente o
corrupto → vacío, sin panic.

**Licencias**: actualizar `THIRD-PARTY-NOTICES.md` con las crates nuevas (lofty, pdf) — el proyecto
lo exige.

## Fase 5 — Diagnóstico dirigido (bugs que requieren reproducción)

Dos bugs NO se pudieron confirmar por lectura; se abordan con **logging temporal + reproducción por
Nicolás en la VM**, y recién con la evidencia se aplica el fix. NO se arreglan a ciegas.

**5a — Cerrar la ventana mata el proceso** (debería ir a la bandeja):
- Hipótesis principal: `should_quit_on_close(close_to_tray, tray_active)` (`tray.rs:130`) cierra si
  `tray_active=false`; los defaults son correctos (`close_to_tray=true`, `tray_enabled=true`), así
  que la sospecha es que **la creación del tray falla silenciosamente** (`tray.rs:39-113`) y deja
  `tray_active=false`.
- Instrumentar: loguear el resultado de la creación del tray (éxito/error y el error concreto), el
  valor de `tray_active`, y la rama tomada en `on_close_requested`. Nicolás reproduce (cerrar la
  ventana) y comparte el log (`naygo.log`).
- Fix según evidencia. Además, endurecer: si `close_to_tray=true` pero el tray falló, NO cerrar en
  silencio — al menos loguear claramente y, opcionalmente, mantener la app viva minimizada o avisar.

**5b — Scroll bloqueado con 469 archivos**:
- El scroll es manual (`file-panel.slint:855`): `wheel-scroll` acotado a
  `[0, rows.length*row-h - self.height]`, donde `self` es el `body-touch`. El `body-touch` se
  DESMONTA si `rename-pos != -1 || modal-open || drag-in-progress` (`file-panel.slint:721`).
- Hipótesis a distinguir con logs: (a) `body-touch` desmontado por un flag pegado (drag-in-progress
  / modal-open que no se limpió); (b) `self.height` mal calculado (0 o inflado) que hace el máximo
  de scroll 0 o inaccesible; (c) el panel estaba en modo deep/búsqueda y `rows.length` no refleja
  las filas reales; (d) algo del layout del split le da un alto raro.
- Instrumentar: loguear al recibir `scroll-event` (o exponer a un log Rust) `rows.length`, `row-h`,
  `self.height`, el máximo calculado, y los flags `rename-pos/modal-open/drag-in-progress`. Nicolás
  reproduce en ese panel de 469 y comparte. Fix según evidencia (probablemente: limpiar un flag
  pegado, o usar `list.visible-height` en vez de `self.height`, o corregir el alto del contenedor).

**Checkpoint**: esta fase tiene un paso de reproducción por Nicolás en medio. Se entrega el build
instrumentado, él reproduce, y con los logs se cierra el diagnóstico y se aplica el fix.

## Fase 6 — Autostart / bandeja: arrancar directo a la bandeja

Comportamiento deseado (elegido por Nicolás): al iniciar Windows con autostart, Naygo arranca
**directo en la bandeja del reloj** — sin ventana visible y sin botón en la barra de tareas —,
**salvo** que la última vez el usuario haya cerrado/apagado con la **ventana abierta**, en cuyo caso
restaura la ventana abierta.

**Problemas actuales**:
- `set_minimized(true)` (`main.rs:1760`) deja el botón en la BARRA DE TAREAS, no en la bandeja; y
  hay un "flash" porque Slint muestra la ventana en `run()` antes de poder minimizarla.
- No se puede `window().hide()` (decrementa el contador de ventanas de Slint y cierra la app).

**Diseño**:
1. **Persistir el estado "ventana abierta al cerrar"**: al guardar la sesión/geometría
   (`main.rs:5754`+), guardar un booleano `window_was_open_on_exit` (true si se cerró con la ventana
   visible; false si estaba en la bandeja). Ese flag decide si el próximo arranque en tray muestra o
   no la ventana.
2. **Arranque en tray sin flash ni botón de barra de tareas**: exponer en `naygo_platform::window`
   una operación Win32 sobre el HWND que: (a) quite el botón de la barra de tareas
   (`WS_EX_TOOLWINDOW` / `ITaskbarList::DeleteTab`), y (b) mantenga la ventana oculta. Aplicarla lo
   antes posible en el arranque `--tray` (en el primer `on_wake`, donde ya existe el HWND). El
   objetivo es que en modo tray NUNCA aparezca un botón en la barra de tareas y el flash sea mínimo
   o nulo. La ventana solo se muestra (y recupera su botón de barra de tareas) cuando el usuario
   abre desde el ícono de bandeja.
3. **Lógica de arranque**: si `--tray` (autostart minimizado) Y `!window_was_open_on_exit` → arrancar
   en bandeja oculto. Si `--tray` pero `window_was_open_on_exit` → restaurar la ventana abierta (el
   usuario la tenía abierta al apagar).
4. Depende de Fase 5a (el tray debe crearse bien; si el tray no existe, no tiene sentido "arrancar en
   bandeja"). Ordenar 5a antes que 6.

**Testing**: el grueso es Win32 + comportamiento de ventana, se valida en la VM. Test unitario donde
aplique: la lógica de "mostrar vs ocultar al arrancar" (`--tray` × `window_was_open_on_exit`) como
función pura testeable.

**Checkpoint**: verificación visual en la VM (arrancar con Windows, ver que no aparece ventana ni
botón de barra de tareas; cerrar con ventana abierta y reiniciar para ver que la restaura).

## Arquitectura y capas

- `core`: `dnd` (decisión de drop, extendida para copy_forced), `metadata/*` (audio/office/pdf),
  `format` (ya tiene `format_time`), un helper de timestamp epoch si hace falta.
- `platform`: `drop_target` (leer MK_CONTROL), `window` (Win32 tray/taskbar), `autostart` (ya
  existe).
- `ui-slint`: `ops_ctrl`/`bridge` (fecha + orden en las filas), `file-panel.slint` (path-bar),
  `main.rs` (cableado del drop, arranque en tray), `config`/i18n (claves nuevas).
- Regla de oro intacta: el hilo de UI no hace I/O; la lectura de metadata ya corre en el worker.

## Orden de ejecución (por prioridad y dependencia)

1. **Fase 1** (bug crítico) — primero, verificar en VM.
2. **Fase 3** (path-bar) y **Fase 2** (fecha+orden) — baratos, independientes.
3. **Fase 4** (metadata) — mediano, independiente.
4. **Fase 5** (diagnóstico con logs) — checkpoint de reproducción con Nicolás.
5. **Fase 6** (autostart/bandeja) — tras 5a; checkpoint VM.

## Riesgos y mitigaciones

- **Fase 1 es pérdida de datos**: máxima prioridad, test exhaustivo de la tabla de decisión, verificar
  en VM antes de seguir.
- **PDF**: si no hay crate puro-Rust aceptable, se difiere sin bloquear el lote.
- **Fases 5 y 6 dependen de la VM de Nicolás**: se entregan builds instrumentados/de prueba y se
  cierran con su verificación; no se dan por terminadas sin ella.
- **Undo**: al invertir el orden de presentación del historial de acciones, NO alterar la semántica
  del undo (Ctrl+Z deshace la última acción). Invertir solo en la capa de filas.
