# Naygo — Multi-selección estilo Explorer — Diseño

> Marcar varios archivos: clic / Ctrl+clic / Shift+clic, rectángulo (rubber-band) desde
> espacio vacío, y selección por teclado. Las acciones (copiar/cortar/eliminar/tamaño/
> menú nativo) pasan a operar sobre toda la selección.

Autor: Nicolás Groth / ISGroth (Chile), 2026, MIT.

## Contexto y punto de partida

- `crates/core/src/workspace/file_pane.rs`: `FilePaneState` ya tiene
  `pub focused: Option<usize>` (ancla/foco) y `pub selected: Vec<usize>` (RESERVADO,
  hoy siempre vacío). Ambos viven en espacio de VISTA (posición en `view_indices()`,
  que respeta el filtro), NO en `entries` directo. `focused_view_entry()` mapea
  foco→entry. `view_indices()` da el orden visible.
- `crates/ui/src/panes/file_panel.rs`: el render usa `let focused = f.focused;` y
  `row.set_selected(focused == Some(i))` (~268-275) — hoy SOLO pinta el foco como
  "seleccionado"; `selected` no se usa. El TableBuilder tiene `.sense(Sense::click())`
  (~175). Los clics se difieren: `clicked = Some(i)` (~323), `double_clicked` →
  `activated`, `secondary_clicked`/context_menu → `context_focus`. Hay `dnd_drag_source`
  (~206) ya presente para futuro drag&drop. Las acciones del menú empujan `Action::*`
  a `ops_actions`.
- `crates/ui/src/app.rs`: `fn selected_paths(&self) -> Vec<PathBuf>` (~1486) YA tiene la
  rama de multi-selección: `if !f.selected.is_empty() { ...mapea f.selected (pos vista)
  →entries... } else { f.focused_view_entry() }`. Hoy la rama multi nunca corre (selected
  vacío). Al poblar `selected`, copiar/cortar/eliminar/tamaño la usan SOLAS. `apply_action`
  (~1344) procesa los `Action`. `handle_input(ctx)` procesa teclado/atajos. El clic se
  procesa tras pintar (drenando `clicked`/`activated`/`context_focus`).
- `crates/core/src/keymap.rs`: `Action` enum (sin payload). Hoy: MoveUp/Down, Activate,
  Open, Copy, Cut, Paste, Delete, ComputeSize, etc. La selección por teclado puede sumar
  acciones (SelectAll, etc.) o manejarse en handle_input según convenga.
- La barra de estado: `self.status: String` (se setea en varios lados).
- egui 0.34: `Sense::click_and_drag()` para detectar arrastre; `Response.drag_started()`/
  `dragged()`/`drag_released()`; `ui.input(|i| i.modifiers.ctrl/shift)`; `Response.rect`
  para geometría; `ui.painter().rect_stroke`/`rect_filled` para el rectángulo.

## Decisiones tomadas (brainstorm 2026-06-09, con companion visual)

- **Rectángulo (rubber-band)**: estilo **B clásico** — borde PUNTEADO azul (acento del
  tema), relleno casi imperceptible. Se dibuja arrastrando desde ESPACIO VACÍO.
- **Modificadores (estilo Windows clásico)**:
  - Clic simple → selecciona solo ese ítem (limpia), fija el foco ahí.
  - Ctrl+clic → toggle de ese ítem, mantiene el resto, mueve el foco a él.
  - Shift+clic → rango desde el foco (ancla) hasta el clic (reemplaza la selección por
    el rango; el ancla NO se mueve).
  - Rectángulo sin modificador → reemplaza con lo que toca. **Ctrl+arrastrar → SUMA**.
- **Arrastrar — modelo HÍBRIDO** (decidido tras repensar el caso de Windows):
  - **Clic simple en CUALQUIER celda** (nombre, tamaño, tipo, extensión, fecha) →
    selecciona esa fila (con Ctrl/Shift). Toda la fila responde al clic, como hoy.
  - **Arrastrar empezando sobre la celda del NOMBRE** → NO dibuja rectángulo; se reserva
    para el drag&drop interno futuro (mover/copiar el archivo).
  - **Arrastrar empezando sobre las OTRAS celdas de una fila** (tamaño/tipo/extensión/
    fecha) **o sobre el espacio vacío** → dibuja el rectángulo de selección.
  - Esto da más superficie para iniciar el rectángulo (todas las celdas no-nombre + el
    vacío) sin chocar con el drag&drop del nombre. NO es exactamente Windows (que trata
    toda la fila como el archivo), es una mejora deliberada de usabilidad en tabla densa.
- **Feedback visual (los 3)**:
  1. Barra de estado: "N seleccionados · <tamaño sumado>" (suma lo conocido; carpetas
     sin tamaño calculado no suman hasta F3).
  2. Foco/ancla distinguible: borde punteado además del resaltado azul.
  3. Menú contextual: encabezado "N seleccionados" arriba.
- **Acciones**: copiar/cortar/eliminar/tamaño(F3)/menú-nativo operan sobre TODA la
  selección (vía `selected_paths()` que ya soporta la rama multi).
- **Teclado (incluido)**: Shift+flechas extiende el rango desde el ancla, Ctrl+A todo,
  Espacio toggle del foco.

## Componentes

### 1. Lógica de selección pura (`core::workspace::file_pane` o módulo nuevo `selection`)

Funciones PURAS sobre `FilePaneState` (o sobre `selected`/`focused` + el largo de la
vista), testeables sin UI. Todas operan en espacio de VISTA (posiciones en
`view_indices()`), clamp al rango válido. API (nombres a fijar en el plan):
- `select_single(pos)` → `selected = [pos]`, `focused = Some(pos)`, `anchor = pos`.
- `select_toggle(pos)` (Ctrl) → agrega/quita `pos` de `selected`, `focused = Some(pos)`.
- `select_range(anchor, pos)` (Shift) → `selected = anchor..=pos` (orden normalizado),
  `focused = Some(pos)`; el `anchor` se mantiene.
- `select_rect(pos_set, additive)` → reemplaza (o suma si `additive`) con el conjunto de
  posiciones que el rectángulo tocó.
- `select_all(view_len)` → `selected = 0..view_len`.
- `move_focus_extend(delta, shift)` (teclado) → mueve el foco; con shift extiende el
  rango desde el ancla.
- Helpers: `is_selected(pos)`, `selection_count()`.
NOTA sobre el ANCLA: hoy `FilePaneState` tiene `focused` pero NO un `anchor` explícito.
Shift+clic necesita un ancla estable (el punto desde donde se extiende el rango, que NO
se mueve al hacer Shift+clic). Agregar `pub anchor: Option<usize>` a `FilePaneState`
(efímero, NO se persiste — como `focused`). Clic simple / Ctrl+clic fijan
`anchor = pos`; Shift+clic usa `anchor` y NO lo cambia.
Tests fuertes: cada operación, normalización de rango (anchor > pos y viceversa), clamp a
la vista, toggle que vacía, respeto del filtro (posiciones de vista, no de entries).

### 2. UI — input de selección (`file_panel.rs` + `app.rs`)

- En el render de cada fila: pintar `row.set_selected(f.is_selected(i))` (en vez de solo
  `focused == Some(i)`). El foco/ancla: además del resaltado, un borde punteado
  (`ui.painter().rect_stroke` con el acento del tema) sobre la fila `focused`.
- Diferir el clic CON sus modificadores: en vez de `clicked = Some(i)`, capturar
  `clicked = Some((i, modifiers))` (ctrl/shift leídos del input). Procesar tras pintar
  llamando la función pura correspondiente (single/toggle/range).
- **Rubber-band**: el área del panel (fondo, donde NO hay fila) usa
  `Sense::click_and_drag()`. Al `drag_started()` sobre espacio vacío, guardar el punto
  inicial; durante `dragged()`, calcular el rect (inicial→actual) y pintarlo punteado
  (`rect_stroke` dashed — egui 0.34 soporta `Stroke` + un patrón, o emular con segmentos;
  verificar). Calcular qué filas intersecta el rect (cada fila conoce su `Response.rect`)
  → posiciones de vista. Al `drag_released()`, aplicar `select_rect(pos_set, ctrl)`.
  CLAVE: el arrastre que empieza SOBRE una fila NO dispara el rubber-band (esa fila tiene
  su propio `Sense::click()`/`dnd_drag_source`); el rubber-band es del fondo del panel.
  Verificar cómo distinguir "fondo" de "fila" en el TableBuilder (quizá el `Sense` del
  área scrolleable detrás de la tabla, o un `interact` sobre el rect del panel menos las
  filas). Esto es lo técnicamente más delicado — el plan debe resolver la mecánica exacta.
- Teclado en `handle_input`/`apply_action`: Shift+flechas → `move_focus_extend(±1, true)`;
  flechas sin shift → mover foco + `select_single` del nuevo foco (comportamiento actual
  de MoveUp/Down pero poblando selected); Ctrl+A → `select_all`; Espacio → toggle del
  foco. Ctrl+A como Action nueva (SelectAll) en el keymap (configurable) o atajo fijo —
  decidir en el plan.

### 3. Feedback (`file_panel.rs` / `app.rs` / menú)

- Barra de estado: tras procesar selección, si `selection_count() > 1` (o >=1), setear
  `self.status` = i18n "N seleccionados · <human_size de la suma de tamaños conocidos>"
  (reusar `core::format::human_size`; sumar `entry.size` de los seleccionados; carpetas
  con size desconocido no suman). Cuando la selección vuelve a 0/1, el status normal.
- Menú contextual: si hay multi-selección, un encabezado deshabilitado "N seleccionados"
  arriba (i18n). Las acciones ya operan sobre `selected_paths()`.
- Foco/ancla: borde punteado (ver arriba).

## Verificación (reparto)

- **El agente**: tests PUROS fuertes de la lógica de selección (todos los casos:
  single/toggle/range/rect/all/teclado, normalización, clamp, filtro); build/clippy/fmt;
  i18n parity de las claves nuevas.
- **Nicolás (visual/manual)**: arrastrar el rectángulo desde vacío selecciona; Ctrl/Shift
  combinan bien; el contador y el tamaño sumado se ven; el foco se distingue; las acciones
  (copiar/eliminar/tamaño/menú nativo) afectan a todos los seleccionados; arrastrar sobre
  una fila NO dibuja rectángulo.

## Fuera de alcance (explícito)

- **Drag&drop interno** entre paneles (mover/copiar arrastrando) — fase futura; esta fase
  RESERVA el arrastre-sobre-fila para eso (no lo implementa).
- Inline rename (F2 en la fila), feedback "cortado" atenuado, agrupar — backlog.
- Arrastre que auto-scrollea la lista al llegar al borde — nice-to-have futuro (mencionar
  como deuda si el rect no auto-scrollea).

## Notas de riesgo / cuidado para el plan

- **Rubber-band: dónde arranca** (lo más delicado, modelo HÍBRIDO): el rectángulo se
  dibuja al arrastrar desde (a) el espacio vacío debajo de la última fila, O (b) cualquier
  celda de una fila que NO sea la del NOMBRE. Arrastrar sobre la celda del NOMBRE se
  reserva para drag&drop futuro. Mecánica a resolver en el plan: durante el render,
  guardar el rect de la celda-nombre de cada fila (la columna Name conoce su rect); al
  detectar `drag_started()`, mirar dónde cayó el punto inicial — si está sobre alguna
  celda-nombre → no rubber-band (futuro drag); si está sobre otra celda o el vacío →
  rubber-band. Alternativa más simple si distinguir por columna es frágil con el
  TableBuilder: un `ui.interact(panel_rect, id, Sense::click_and_drag())` de fondo que
  capture el arrastre salvo cuando el punto inicial cae sobre una celda-nombre (guardar
  solo los rects de las celdas-nombre, no de toda la fila). El clic SIMPLE en cualquier
  celda sigue seleccionando la fila (eso lo maneja el `Sense::click()` de la fila, que es
  independiente del arrastre de fondo). Verificar contra egui 0.34 / egui_extras que el
  click-de-fila y el drag-de-fondo coexisten sin robarse el evento.
- **Intersección rect↔filas**: cada fila conoce su `Response.rect` (coords de pantalla);
  guardar esos rects durante el render para intersectar con el rect del rubber-band y
  derivar las posiciones de vista tocadas.
- **Borde punteado en egui 0.34**: `rect_stroke` con patrón dasheado — verificar si egui
  0.34 expone dashes nativos (`Stroke` no los tiene; puede requerir `Shape::dashed_line`
  o pintar segmentos). Si es complejo, un borde sólido fino distinto del relleno también
  distingue el foco — pero el companion mostró punteado; intentar punteado primero.
- **`anchor` nuevo en FilePaneState**: efímero (no persistir), igual que `focused`.
  Verificar que el bridge de persistencia (FilePanePersist) NO lo serialice.
- **selected en espacio de vista**: al re-listar / re-filtrar / re-ordenar, las posiciones
  de vista cambian — la selección debería limpiarse o re-mapearse. DECISIÓN simple: al
  re-listar (navegación) o cambiar filtro/orden, LIMPIAR la selección (selected/anchor) —
  es lo que hace Windows al cambiar de carpeta. Documentar; re-mapear es over-engineering.
- **Tamaño sumado en el status**: usar `entry.size` ya conocido; carpetas sin F3 no suman
  (no disparar cálculos por seleccionar — eso rompería bajo consumo).
- **Teclado**: las flechas hoy mueven el foco (MoveUp/Down) — al poblar selected, flecha
  sin shift debe hacer select_single del nuevo foco (no dejar multi-selección pegada).
  Ctrl+A: decidir Action nueva vs atajo fijo (preferir Action configurable, ya hay keymap
  + test de paridad i18n de acciones).
- **selected_paths() ya listo**: NO reescribir esa rama; solo poblar `selected`.
