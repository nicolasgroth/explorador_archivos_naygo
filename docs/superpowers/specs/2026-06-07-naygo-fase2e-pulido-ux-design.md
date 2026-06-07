# Naygo — Pulido de UX de la Fase 2E (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-07. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Tras probar la Fase 2E (columnas estilo Excel), Nicolás pidió 4 mejoras de UX. Esta
sub-fase de **pulido** las agrupa. Son cambios de UI: NO tocan el modelo de datos de
2E (`TableState`/`ColumnFilter`/`sort`), reutilizan el `column_menu` y el render
existentes, y no agregan dependencias.

**Premisa rectora:** respuesta rápida y fluida. Todo es pintado/interacción por frame,
trivial en costo.

### Las 4 mejoras (decisiones tomadas en el brainstorm)

1. **Fila completa como zona activa** (file panel **y** árbol). Hoy el clic/selección
   solo reacciona sobre el nombre. Decisión: clic en CUALQUIER celda de la fila (o el
   espacio a la derecha, a la misma altura) selecciona esa fila; doble clic en
   cualquier parte navega/activa. Estilo Explorer/Directory Opus.
2. **Indicador de drop al reordenar columnas**: al arrastrar un encabezado, mostrar una
   **línea de inserción vertical azul** en el borde donde la columna quedaría al soltar.
   La columna arrastrada se atenúa y egui muestra la etiqueta "fantasma" (ya provisto).
3. **Ícono de filtro activo = `≡`** (tres líneas, en color de acento) en lugar del
   `⏷` actual, que no gustó. El indicador de orden `▲/▼` no cambia.
4. **Clic derecho sobre un encabezado abre el MISMO menú de columna** que el `▾` (clic
   izquierdo). Reutiliza `column_menu::show_menu` tal cual (con sus submenús Filtrar/
   Columnas, modo B). Sin menú nuevo.

### Qué entra

- `file_panel.rs`: fila completa clicable vía `TableRow::response()`; línea de drop al
  arrastrar; glifo de filtro `≡`; apertura del menú con clic derecho.
- `tree_panel.rs`: fila de nodo completa clicable (clic en cualquier parte navega; el
  triángulo ▶/▼ conserva su propia zona para expandir/colapsar).
- i18n: sin claves nuevas (no hay texto nuevo; ≡/▲▼ y la línea son símbolos/pintado).

### Qué NO entra

- No cambia `core` (modelo de columnas/filtros/orden intacto).
- No agrega utilidades nuevas al menú (se descartó "quitar todos los filtros" /
  "restablecer columnas"; el clic derecho replica el menú existente).
- No toca la persistencia, el watcher, los temas (2C-ii), ni el listado.
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

Todo vive en la capa `ui`. `core` no cambia. APIs egui 0.34.3 verificadas contra la
fuente del registry:
- `egui::Response::secondary_clicked()` (response.rs:204) — clic derecho.
- `egui::Response::dnd_hover_payload::<usize>()` (487) y `contains_pointer()` (314) —
  saber sobre qué encabezado está el cursor durante el arrastre.
- `egui::Painter::vline(x, y_range, stroke)` (painter.rs:378) — línea de inserción.
- `egui_extras::TableRow::response()` (egui_extras table.rs:1355) — `Response` de la
  fila completa.

### #1 Fila completa — `file_panel.rs`

El cuerpo usa `egui_extras::TableBuilder` con `body.rows(...)`, y en cada fila (`row`)
se llama `row.col(...)` por columna. Hoy el clic se detecta solo en la celda del nombre
(`icon_row` de la 1ª columna). Cambio:
- Tras pintar todas las celdas de una fila de entry, leer `row.response()` (cubre la
  fila completa). Usar `.clicked()` para seleccionar (setear `focused` a la posición de
  vista de esa fila) y `.double_clicked()` para navegar/activar (carpeta → navegar;
  archivo → status "abrir pendiente"). Esto reemplaza/complementa la detección por
  celda. Las celdas se siguen pintando con su contenido; el ícono ya no necesita
  `Sense::click()` propio para que la fila funcione (pero puede conservarse, inofensivo).
- La fila `..` (DisplayRow::Parent): igual, `row.response().clicked() || double_clicked()`
  → navegar al padre (mantiene el "sube con un clic" actual).
- La selección visual sigue usando `row.set_selected(selected)` (de 2E).

### #1 Fila completa — `tree_panel.rs`

El render recursivo pinta, por nodo, una fila con: indentación, triángulo (si tiene
hijos), ícono, y el nombre como `selectable_label`. Hoy solo el nombre navega. Cambio:
- Envolver el contenido de la fila (ícono + nombre, NO el triángulo) en un único
  elemento que sensa clic (mismo patrón `icon_row` del file panel: unir las respuestas
  con `Response::union` y sensar clic), de modo que clic en cualquier parte de esa zona
  emita `TreeAction::Navigate(path)`. El triángulo ▶/▼ mantiene su propia `Label` con
  `Sense::click()` para expandir/colapsar (no debe navegar). El resaltado del nodo
  activo (barra azul + fondo) no cambia.

### #2 Línea de inserción — `file_panel.rs`

Durante el render de los encabezados (cada uno ya es `dnd_drag_source` con payload =
índice real de la columna en `table.columns`):
- Si hay un arrastre en curso, para cada encabezado consultar
  `header_response.dnd_hover_payload::<usize>()`; si el cursor está sobre ese encabezado
  (`contains_pointer()`), determinar el borde de inserción (izquierdo del encabezado
  bajo el cursor) y pintar una `vline` azul (`ui.painter().vline(x, y_range, Stroke)`)
  en ese borde. Solo pintado; la lógica de `MoveColumn` (al soltar) no cambia.
- Decisión simple para 2E: la línea se pinta en el borde IZQUIERDO del encabezado sobre
  el que está el cursor (inserción "antes de esta columna"). Es suficiente y claro.

### #3 Glifo de filtro — `file_panel.rs`

En `column_header`, donde hoy se añade `" ⏷"` al label cuando la columna tiene filtro
activo, cambiar el glifo por `" ≡"` (en color de acento, como el resto del indicador).
Cambio mínimo, sin lógica.

### #4 Clic derecho abre el menú — `file_panel.rs`

En `column_header`, el `▾` ya abre `Popup::menu` con clic izquierdo. Añadir: si el
`Response` del encabezado registra `secondary_clicked()`, abrir el MISMO popup
(`Popup::menu` anclado al mismo response, o togglear su estado por el mismo `popup_id`).
Reutiliza `column_menu::show_menu` sin cambios. Verificar la forma de abrir el popup por
clic derecho en egui 0.34 (`Popup::menu` decide por la respuesta; si hace falta, abrir
manualmente vía memoria del popup_id).

---

## 3. Manejo de errores / casos límite

- Arrastre que termina fuera de cualquier encabezado → no se pinta línea y no hay
  `MoveColumn` (ya tolerado por 2E).
- Fila vacía / lista filtrada vacía ("sin coincidencias") → no hay filas de entry que
  clicar; la fila de aviso no es seleccionable. Sin cambios.
- Árbol: clic en el triángulo NO debe navegar (solo expandir/colapsar); clic en el resto
  de la fila navega. Zonas separadas, sin ambigüedad.
- Clic derecho fuera de un encabezado (sobre una fila de datos) → NO abre el menú de
  columna (eso sería un menú contextual de archivo, fuera de alcance). Solo los
  encabezados responden al clic derecho en esta sub-fase.

---

## 4. Testing

- Es interacción/pintado de egui → validación principalmente MANUAL.
- Si al implementar la línea de inserción surge una función pura útil (p. ej. "dado el
  conjunto de bordes de columna y la X del cursor, ¿en qué borde va la línea?"),
  extraerla a una fn pura y testearla. No forzar tests sobre el render.
- Regресión: confirmar que el doble clic sigue navegando, el type-ahead y las flechas
  siguen funcionando (operan sobre la vista, sin cambio), y que el menú ▾ sigue abriendo
  con clic izquierdo además del derecho.

Meta de siempre: build limpio + tests + clippy antes de cada commit.

---

## 5. Estructura de archivos (incremental)

```
crates/ui/src/
├── panes/file_panel.rs   # fila completa (TableRow::response); línea de drop (vline);
│                         # glifo de filtro ≡; clic derecho abre el menú de columna
├── panes/tree_panel.rs   # fila de nodo completa clicable (triángulo conserva su zona)
└── column_menu.rs        # sin cambios (se reutiliza show_menu para el clic derecho)
```

No hay cambios en `core` ni en i18n.

---

## 6. Dependencias

Ninguna nueva. Todo con egui 0.34.3 / egui_extras 0.34.3 ya presentes. APIs usadas
verificadas contra la fuente.

---

## Fuera de alcance (recordatorio)

Menú contextual sobre filas de datos (archivos/carpetas), "quitar todos los filtros" /
"restablecer columnas", watcher, temas/packs (2C-ii). Nunca: reproducción de media,
edición de archivos.
