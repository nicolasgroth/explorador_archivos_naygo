# Naygo — Fase 2E: columnas estilo Excel + panel activo (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-06. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

El file panel muestra columnas fijas (Nombre/Tamaño/Modificado) con orden por clic en
el encabezado (fase 2D). Nicolás quiere convertirlo en una **tabla rica estilo Excel**:
cada columna con un desplegable que combina **ordenar**, **filtrar** y **mostrar/
ocultar/reordenar columnas**. El core ya tiene `SortKey::Extension` y `SortKey::Created`
listos (2D) esperando UI.

Además, durante el brainstorm Nicolás pidió un pulido transversal: **marcar
visualmente cuál panel está activo** (hoy se distingue poco; con dual-pane + árbol +
inspector es clave saber a dónde van el teclado y las acciones).

**Premisa rectora:** respuesta rápida y fluida + bajo consumo. Ordenar y mostrar/
ocultar columnas operan sobre datos ya en RAM. Filtrar es un recorrido lineal en
memoria (milisegundos aun con decenas de miles de archivos), recalculado solo cuando
cambia un filtro — barato incluso filtrando en vivo.

### Decisiones tomadas en el brainstorm

1. **Alcance: las 3 capacidades completas** — el desplegable ordena, filtra y gestiona
   columnas.
2. **Modelo de columnas extensible** — `ColumnKind` enum + `ColumnSpec` en un `Vec`
   (el orden del Vec = orden visual). Arranca con las 5 que el core ya ordena
   (Nombre, Extensión, Tamaño, Modificado, Creación); agregar una futura = sumar
   variante + extractor, sin rehacer la UI.
3. **Desplegable modo B (compacto)** — ordenar asc/desc directo arriba; "Filtrar…" y
   "Columnas…" abren sub-paneles; opción "Quitar filtro de esta columna".
4. **Filtro en vivo** — la lista se filtra al instante mientras el usuario escribe o
   marca (no hay botón "Aplicar").
5. **Controles de filtro por tipo de dato:**
   - **Nombre (texto)**: caja "contiene", con opción "distinguir mayúsculas".
   - **Extensión/Tipo**: lista de tipos con checkboxes + búsqueda + contador por tipo
     (cuántos archivos hay de cada uno, calculado de las entries actuales). Incluye
     una entrada "(sin extensión)".
   - **Tamaño**: rango mín–máx con unidad (KB/MB/GB).
   - **Modificado/Creación (fecha)**: rango desde–hasta.
6. **Multi-filtro = intersección (Y)** — varias columnas filtradas a la vez se
   combinan con AND (estilo Excel): un archivo se muestra si cumple TODOS los filtros
   activos.
7. **Indicador de columna activa** — un encabezado con orden activo muestra ▲/▼; con
   filtro activo muestra un ícono de embudo; la columna se ve resaltada. Se nota de un
   vistazo que la lista está ordenada/filtrada por esa columna.
8. **Panel activo (pulido transversal)** — la **barra de título** del panel activo se
   pinta en color de acento (modo B). No mueve el contenido; el panel inactivo vuelve
   a neutro.
9. **Filtros persisten por panel** — no se limpian al navegar de carpeta; el panel
   recuerda su configuración (columnas + filtros), persistida en disco.
10. **Nombre no ocultable** — siempre visible (evita una tabla sin columnas).

### Qué entra

- `core::columns`: `ColumnKind`, `ColumnSpec`, `TableState` (columnas + filtros).
- `core::filter`: `ColumnFilter` (4 tipos) + `matches(entry, &filters) -> bool` (AND).
- `FilePaneState`/`FilePanePersist`: campo `table: TableState` (reemplaza el
  `text_filter: Option<String>` reservado; migración → `ColumnFilter::Text`).
- `ui::file_panel`: pinta columnas visibles en orden, con indicadores; aplica
  filtro+orden en memoria.
- `ui::column_menu` (nuevo): desplegable modo B (ordenar/filtrar/columnas), filtro en
  vivo, controles por tipo; lógica con estado extraída a funciones puras.
- `ui`: realce del panel activo (barra de título de acento).
- i18n nuevas (ES + EN): textos del menú, filtros, "sin coincidencias", etc.

### Qué NO entra

- Filtro global de barra (la búsqueda es por columna). 
- Agrupar/pivotar/fórmulas (no somos Excel; solo ordenar/filtrar/columnas).
- Redimensionar columnas con el mouse a nivel pixel-perfect si complica (ancho
  persistido sí; el arrastre fino puede ser básico). 
- Nuevas `ColumnKind` más allá de las 5 del core (el modelo las admite a futuro, pero
  2E no agrega atributos/fecha-acceso).
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

Idea rectora: modelo de columnas/filtros en `core` (puro, testeable); la UI pinta y
aplica. Todo en memoria. Reutiliza `SortKey`/`SortSpec` (2D) y `sort_entries`.

### Capa `core` — `columns.rs` + `filter.rs` (nuevos)

```rust
/// Qué columna. Extensible: agregar variante + su extractor a futuro.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColumnKind { Name, Extension, Size, Modified, Created }

/// Una columna de la tabla: qué es, si se ve, su ancho. Orden del Vec = orden visual.
#[derive(Clone, Serialize, Deserialize)]
pub struct ColumnSpec { pub kind: ColumnKind, pub visible: bool, pub width: f32 }

/// Filtro de una columna (según su tipo de dato).
#[derive(Clone, Serialize, Deserialize)]
pub enum ColumnFilter {
    Text { contains: String, case_sensitive: bool },          // Name
    Extensions(std::collections::BTreeSet<String>),           // Extension
    SizeRange { min: Option<u64>, max: Option<u64> },         // Size (bytes)
    DateRange { from: Option<SystemTime>, to: Option<SystemTime> }, // Modified/Created
}

/// Estado de tabla de un panel: columnas + filtros activos por columna.
#[derive(Clone, Serialize, Deserialize)]
pub struct TableState {
    pub columns: Vec<ColumnSpec>,
    pub filters: std::collections::BTreeMap<ColumnKind, ColumnFilter>, // AND
}
```

- `TableState::default()` → las 5 columnas (Nombre/Extensión/Tamaño/Modificado visibles;
  Creación oculta por defecto), sin filtros.
- `ColumnKind` ↔ `SortKey` mapeo 1:1 (reutiliza `sort_entries`).
- Operaciones puras de `TableState`: `toggle_visible(kind)` (Nombre no ocultable),
  `move_column(from, to)`, `set_width(kind, w)` (clamp mín/máx), `set_filter(kind, f)`,
  `clear_filter(kind)`, `visible_columns() -> impl Iterator`.
- **`filter::matches(entry: &Entry, filters: &BTreeMap<ColumnKind, ColumnFilter>) ->
  bool`**: el entry pasa si cumple TODOS los filtros (AND). Cada variante:
  - `Text` → `entry.name` contiene la subcadena (respeta `case_sensitive`).
  - `Extensions` → la extensión del path (minúsculas; "" = "(sin extensión)") está en
    el set. Set vacío = no filtra (muestra todo).
  - `SizeRange` → `entry.size` dentro de [min, max] (None = sin límite ese lado).
    Carpetas (size None) quedan fuera si hay filtro de tamaño activo (decisión:
    filtrar por tamaño implica archivos).
  - `DateRange` → `entry.modified`/`entry.created` (según la columna) dentro de
    [from, to].
- **Contadores por tipo** (para el filtro de extensión): helper
  `extension_counts(entries: &[Entry]) -> BTreeMap<String, usize>` (puro).

### Capa `core::workspace::file_pane`

- `FilePaneState` y `FilePanePersist`: reemplazar `text_filter: Option<String>` por
  `table: TableState`. Migración al cargar: si un persist viejo trae `text_filter =
  Some(s)`, se traduce a `table.filters[Name] = ColumnFilter::Text { contains: s,
  case_sensitive: false }`. (serde con `#[serde(default)]` para retro-compat.)

### Capa `ui`

- **`file_panel.rs`**: pinta las **columnas visibles** de `table.columns` en su orden.
  Pipeline por frame (todo en RAM): filtrar (`matches`) → ordenar (`sort_entries`) →
  pintar filas. Cada encabezado: título + ▲/▼ (si ordena) + embudo (si filtra) +
  ▾ (abre el menú). Resaltado de la columna activa. Si la vista queda vacía por
  filtro: aviso discreto "sin coincidencias".
- **`column_menu.rs` (nuevo)**: el desplegable modo B. Ordenar asc/desc directo;
  "Filtrar…" abre el sub-panel del tipo correspondiente (texto/checkboxes+búsqueda/
  rango), filtrando EN VIVO; "Columnas…" abre el sub-panel de mostrar/ocultar/
  reordenar; "Quitar filtro". La **lógica con estado** (qué `TableState`/`SortSpec`
  resulta de cada interacción) se extrae a **funciones puras testeables**; el render
  solo dibuja y acumula acciones (patrón `PaneRequest`).
- **Acciones del menú** (diferidas, estilo `PaneRequest`): `SetSort`, `SetFilter`,
  `ClearFilter`, `ToggleColumn`, `MoveColumn`, `SetColumnWidth`. `NaygoApp` las aplica
  tras pintar.
- **Panel activo**: en el chrome del tab/panel, pintar la barra de título del panel
  activo (`workspace.active_id()`) en color de acento. Independiente del modelo de
  columnas.

### Lo que NO cambia

Árbol, listado, docking (salvo el realce del activo). El orden por clic en encabezado
(2D) se absorbe en el nuevo desplegable (clic en ▾ → ordenar).

---

## 3. Flujo de datos

Pipeline de visualización por frame (memoria pura, el hilo de UI no toca disco):
1. El worker llena `entries` (sin cambios).
2. **Filtrar** (si hay filtros): `entries.iter().filter(|e| filter::matches(e,
   &table.filters))`.
3. **Ordenar**: `sort_entries` con el `SortSpec` vigente.
4. **Pintar** columnas visibles en orden + indicadores.

Filtro en vivo: cada tecla/checkbox actualiza `table.filters` y el pipeline recalcula
al instante (lineal, imperceptible).

Interacciones del menú → acciones diferidas → `NaygoApp` muta `TableState`/`SortSpec`
tras pintar → persiste por panel al guardar.

Panel activo: al cambiar `active_id` (clic/Tab), la barra de título del nuevo activo
se pinta en acento; el anterior a neutro.

---

## 4. Manejo de errores / casos límite

- **0 resultados por filtro** → lista vacía + aviso discreto "sin coincidencias" +
  embudo visible (para entender por qué y poder quitarlo). No es error.
- **Ocultar todas las columnas** → Nombre no es ocultable (siempre visible).
- **Contadores por tipo** → de las entries actuales; se actualizan en vivo si la
  carpeta sigue listando.
- **Filtro activo + navegar** → los filtros persisten por panel (no se limpian); el
  embudo sigue visible.
- **Persist con `ColumnKind`/filtro desconocido** → tolerante: se ignora, sin pánico.
- **Ancho de columna** → clamp mín/máx (no desaparece ni desborda).
- **Filtro de tamaño/fecha sobre carpetas** → carpetas (size/fecha ausente) quedan
  fuera cuando ese filtro está activo; consistente y sin pánico.

---

## 5. Testing

- **`core::filter`** (el grueso): `matches` por tipo — Text (contains, case sens/insens),
  Extensions (set, "sin extensión", set vacío = todo), SizeRange (bordes, None en un
  lado, carpetas fuera), DateRange (bordes). **Intersección Y** de varios filtros.
  `extension_counts`.
- **`core::columns`/`TableState`**: toggle_visible (Nombre no ocultable), move_column,
  set_width (clamp), set_filter/clear_filter, visible_columns; round-trip serde
  (columnas + filtros); migración `text_filter` → `ColumnFilter::Text`.
- **Funciones puras del menú** (interacción → nuevo TableState/SortSpec): sin egui.
- **UI** (desplegable, indicadores, contadores, realce panel activo, "sin
  coincidencias"): validación manual; lógica con estado extraída a core/puras.

Meta de siempre: build limpio + tests + clippy antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── columns.rs             # NUEVO: ColumnKind, ColumnSpec, TableState (+ops puras)
├── filter.rs              # NUEVO: ColumnFilter, matches(), extension_counts()
├── workspace/file_pane.rs # text_filter → table: TableState (+ migración)
├── lib.rs                 # + pub mod columns; pub mod filter; re-exports
└── i18n/{es,en}.json      # + claves del menú/filtros/sin-coincidencias

crates/ui/src/
├── panes/file_panel.rs    # columnas visibles en orden; indicadores; pipeline filtrar+ordenar
├── column_menu.rs         # NUEVO: desplegable modo B (ordenar/filtrar/columnas), filtro en vivo
├── table_actions.rs       # NUEVO (o dentro de column_menu): acciones diferidas del menú
├── docking.rs / app.rs    # realce del panel activo (barra de título de acento)
└── main.rs                # + mod column_menu; (+ table_actions si va aparte)
```

---

## 7. Dependencias

Sin dependencias nuevas. Todo es std + lo ya usado (`serde`, egui). `SortKey`/
`SortSpec`/`sort_entries` ya existen (2D).

---

## Fuera de alcance (recordatorio)

Filtro global de barra, agrupar/pivotar/fórmulas, nuevas ColumnKind (atributos/fecha
acceso), arrastre pixel-perfect de anchos si complica. Watcher y temas/packs van por
su cuenta (2C-ii en paralelo). Nunca: reproducción de media, edición de archivos.
