# Vista profunda (modo recursivo en el panel) — Diseño

> Bloque 4 del trabajo de mejoras post-VM. Independiente de los bloques 1-2 (versionado,
> docs) ya cerrados, y del bloque 3 (copiar nombres/rutas, que ya estaba implementado).

**Objetivo:** un modo de vista dentro del panel de archivos que, además del contenido de la
carpeta actual, muestra todos los archivos y carpetas de las subcarpetas (recursivo), en una
lista plana con sangría por profundidad y ruta relativa, para tener el contenido completo de
un árbol sin navegar carpeta por carpeta.

**Autoría:** Nicolás Groth / ISGroth, 2026, MIT.

**Decisiones tomadas con el usuario:**
- Es un **modo de vista dentro del panel existente**, no un panel aparte.
- **Lista plana** con sangría por profundidad + ruta relativa (no árbol expandible).
- Desciende por **todos los niveles**; activación por **toggle** (botón en la barra del panel
  + opción de menú); no es estado pegajoso (navegar a otra carpeta vuelve a vista normal).
- **Streaming incremental + cancelable, SIN tope** (a diferencia de la búsqueda, que corta en
  `MAX_HITS = 5000`).
- Doble clic: carpeta → sale del modo y navega a ella; archivo → lo abre. **Operaciones
  normales** (selección múltiple, copiar nombres/rutas, copiar/mover/eliminar) sobre los ítems
  reales (paths absolutos).
- Orden y filtros actúan sobre la **lista completa como tabla plana**; la sangría/ruta relativa
  es solo presentación.

---

## Contexto actual (lo que se reutiliza — no rehacer)

- **`core::search`** (`crates/core/src/search.rs`) ya tiene el recorrido recursivo ideal:
  - `search_walk(root, query, lister: &dyn Fn(&Path) -> ListResult, token, tx)` — núcleo PURO
    con **pila propia** (sin recursión de stack, soporta árboles profundos), **no desciende a
    symlinks/junctions**, marca `partial` si hay carpetas ilegibles, chequea `token` entre
    directorios.
  - `lister` es **inyectable** → los tests pasan un closure y no tocan el FS (patrón a copiar).
  - `SearchMsg::{Hit(Entry), Progress{dirs_scanned}, Done{partial,hit_cap}, Cancelled}`.
  - `spawn_search(root, query, token) -> (Receiver<SearchMsg>, JoinHandle)`.
  - `MAX_HITS = 5000` corta la búsqueda — la vista profunda NO usará ese tope.
  - `fs_lister` y `DirEntryRaw`/`ListResult` ya existen (privados a search).
- **`core::listing`**: `entry_from_path` construye un `Entry` desde una ruta (lo usa search).
- **`Entry`** (`core::fs_model`) tiene path absoluto, nombre, tipo, tamaño, fecha, etc.
- **UI**: el panel de archivos tiene modo *normal* y modo *búsqueda*. El listado se consume por
  streaming con `.poll()` + `CancellationToken` (`crates/ui-slint/src/listing.rs`). Las filas
  son `RowData` (Slint) con columnas dinámicas, selección, orden y filtros. `selected_paths()`
  y `context_targets()` devuelven paths absolutos.

---

## Arquitectura — visión general

La vista profunda es un **tercer modo de listado** del panel. Reparte responsabilidades por
capa (regla de las 3 capas):

- **core**: `deep_listing.rs` (nuevo) — el walk recursivo que emite cada entrada con su
  `depth` y `rel_path`. Lógica pura y testeable, sin UI ni Windows.
- **ui-slint**: estado de modo en el panel, consumo por streaming del canal, sangría en la
  fila, toggle en la barra, y el ruteo de doble clic. Sin lógica de recorrido.

### Sección 1 — core: `deep_listing.rs`

Módulo nuevo `crates/core/src/deep_listing.rs`. Es el gemelo de `search` sin filtro de nombre
y con metadatos de jerarquía.

```rust
/// Una entrada del recorrido profundo: la entrada normal + su lugar en el árbol.
#[derive(Debug, Clone, PartialEq)]
pub struct DeepEntry {
    /// La entrada en sí (path absoluto, nombre, tipo, tamaño, fecha…).
    pub entry: Entry,
    /// Profundidad relativa a la raíz: 0 = hijo directo de la raíz, 1 = nieto, etc.
    pub depth: u32,
    /// Ruta relativa a la raíz, con separadores del SO (p. ej. "2025\\enero\\informe.pdf").
    pub rel_path: String,
}

/// Mensajes del worker de listado profundo hacia la UI. Espeja a `SearchMsg` pero sin
/// `hit_cap` (no hay tope): el recorrido emite TODO hasta agotarse o ser cancelado.
#[derive(Debug, Clone, PartialEq)]
pub enum DeepMsg {
    Entry(DeepEntry),
    Progress { dirs_scanned: usize },
    Done { partial: bool },
    Cancelled,
}

/// Lanza el listado profundo bajo `root` en un worker. La UI drena el receptor frame a
/// frame. Cancelable vía `token`.
pub fn spawn_deep_listing(
    root: PathBuf,
    token: CancellationToken,
) -> (Receiver<DeepMsg>, JoinHandle<()>);

/// Núcleo PURO: recorre el árbol bajo `root` con pila propia, emitiendo CADA entrada (no
/// filtra). `lister` produce las entradas de un directorio (en prod lee el FS; en tests, un
/// closure). Chequea `token` entre directorios. NO desciende a symlinks. Sin tope.
fn deep_walk(
    root: &Path,
    lister: &dyn Fn(&Path) -> ListResult,
    token: &CancellationToken,
    tx: &Sender<DeepMsg>,
);
```

**Reutilización sin duplicar:** `DirEntryRaw`, `ListResult` y `fs_lister` viven hoy en
`search.rs` como privados. Para que `deep_listing` los use sin copiarlos, se hacen
`pub(crate)` en `search.rs` (cambio mínimo, no altera su comportamiento) y `deep_listing` los
importa. `entry_from_path` ya es accesible (lo usa search). El `rel_path` se calcula con
`path.strip_prefix(root)` y el `depth` por el número de componentes de ese rel_path menos uno.
`deep_walk` lleva en su pila `(PathBuf, depth)` para conocer la profundidad de cada nivel.

**Sin tope** (decisión del usuario): no hay `MAX_HITS`. El control de tamaño es el streaming +
la cancelación, igual que el resto de operaciones largas de Naygo.

### Sección 2 — ui-slint: estado de modo, streaming y consumo

- El estado del panel de archivos gana un modo *profundo* junto a *normal* y *búsqueda*. Al
  activarlo sobre la carpeta actual: se crea un `CancellationToken` nuevo, se lanza
  `spawn_deep_listing(dir, token)`, y el `.poll()` del panel drena `DeepMsg` y vuelca las
  entradas en el `VecModel` de filas (igual que el listado normal vuelca `Entry`).
- Cada fila profunda guarda, además de lo normal, su `depth` (para la sangría) y usa el
  `rel_path` para que el origen sea visible.
- **Cancelación:** `Esc`, el toggle (apagarlo), navegar a otra carpeta o cerrar el panel
  cancelan el token y abortan el worker. El patrón de "un token por activación" (ya usado en
  búsqueda) evita workers huérfanos y filas que llegan tarde.
- **No pegajoso:** navegar a otra carpeta vuelve a modo normal (no se mantiene profundo).
- El hilo de UI nunca hace I/O: solo consume del canal.

### Sección 3 — ui-slint: fila, sangría, orden/filtros, selección, operaciones

- **`RowData` (types.slint)** gana `depth: int` (default 0). En vista normal y búsqueda
  `depth` es 0 → sin cambios visuales. En profundo, la fila se sangra `depth * paso` (paso
  fijo, p. ej. 14px) en la columna de nombre. **Por defecto, la columna de nombre muestra el
  `rel_path`** (ruta relativa a la raíz), para que el origen sea inequívoco; la sangría lo
  refuerza visualmente. (Si en el visto bueno visual Nicolás prefiere ver solo el nombre del
  archivo con la carpeta de origen tenue, es un ajuste menor de render — pero el plan parte de
  mostrar `rel_path`.)
- **Orden y filtros:** los `sort_entries`/`ColumnFilter` existentes operan sobre la lista
  profunda completa como tabla plana (orden global por la columna elegida; filtro global). La
  sangría no participa del orden (es presentación). No se implementa orden jerárquico.
- **Selección y operaciones:** sin cambios en su lógica. `selected_paths()`/`context_targets()`
  ya devuelven paths absolutos, así que copiar nombres/rutas y copiar/mover/eliminar funcionan
  sobre los ítems reales vengan de la subcarpeta que vengan.
- **Doble clic / Enter:** carpeta → sale del modo profundo y navega a esa carpeta (vista
  normal); archivo → abrir con su programa por defecto (comportamiento ya existente).

### Sección 4 — UI de activación (toggle)

- Un botón toggle en la barra del panel (ícono dibujado con `Path`, NUNCA glifo de fuente —
  render por software) que activa/desactiva el modo. Estado visual on/off (p. ej. fondo
  acento cuando está activo).
- Opción equivalente accesible desde menú (el de carpeta / zona vacía del panel, donde ya
  viven acciones del panel).
- i18n triple para el tooltip/label: `deep-view` ("Vista profunda" / "Deep view") y
  `deep-view-tip` (explica qué hace). Sin texto hardcodeado.
- Mientras el recorrido está activo y aún no termina, el botón puede ofrecer cancelar (o `Esc`
  cancela). El detalle visual del estado "cargando/cancelar" queda al implementar.

---

## Archivos tocados

| Archivo | Acción |
|---|---|
| `crates/core/src/deep_listing.rs` | **Crear** (DeepEntry, DeepMsg, spawn_deep_listing, deep_walk + tests). |
| `crates/core/src/search.rs` | `DirEntryRaw`, `ListResult`, `fs_lister` pasan a `pub(crate)` (sin cambiar comportamiento). |
| `crates/core/src/lib.rs` | `pub mod deep_listing;`. |
| `crates/ui-slint/src/listing.rs` (o equiv.) | Consumo por streaming de `DeepMsg` (gemelo del polling actual). |
| `crates/ui-slint/src/workspace_ctrl.rs` | Modo profundo en el estado del panel: activar/desactivar, token, volcar filas con depth, cancelar al navegar/apagar, ruteo de doble clic. |
| `crates/ui-slint/ui/types.slint` | `RowData` gana `depth: int`. |
| `crates/ui-slint/ui/file-panel.slint` | Sangría por `depth` en la fila; botón toggle en la barra del panel; estado on/off. |
| `crates/ui-slint/ui/i18n.slint` | `deep-view`, `deep-view-tip`. |
| `crates/core/src/i18n/es.json`, `en.json` | Traducciones de esas claves. |
| `crates/ui-slint/src/i18n_keys.rs` | Setters de esas claves. |
| `crates/ui-slint/src/main.rs` | Cablear el callback del toggle y el ruteo. |

## Testing

En **core** (lógica pura, lo importante):
- `deep_walk` sobre un árbol temporal (vía `lister` closure, sin tocar FS): emite TODAS las
  entradas de todos los niveles; `depth` correcto (0 hijos directos, 1 nietos, 2 bisnietos);
  `rel_path` correcto por nivel.
- NO desciende a symlinks (análogo al test que ya existe en search).
- `token` cancelado corta el recorrido y emite `Cancelled` (deja de emitir entradas).
- Carpeta ilegible (lister devuelve `None`) no aborta el resto → `Done { partial: true }`.
- Árbol vacío / sin subcarpetas: emite solo el nivel 0.
- Sin tope: un árbol con > 5000 entradas las emite todas (no corta como search).

Orden/filtros: reutilizan los tests existentes de `sort_entries`/`ColumnFilter` (operan sobre
entradas arbitrarias); no requieren tests nuevos salvo confirmar que `depth` no interfiere.

La parte visual (sangría, toggle on/off, doble clic que navega/abre, cancelación con Esc) va a
**visto bueno visual de Nicolás** corriendo la app.

## Fuera de alcance (YAGNI)

- Profundidad configurable (1/2/3/todos) — se decidió "todos"; se puede agregar después.
- Tope/aviso de seguridad por cantidad — se decidió sin tope (streaming + cancelar).
- Árbol expandible (acordeón) y orden jerárquico — se decidió lista plana con orden global.
- Persistir el modo profundo entre sesiones — no es pegajoso, no se persiste.

## Riesgos y mitigaciones

- **Recorrido enorme (raíz de disco):** mitigado por streaming + cancelación (Esc/toggle).
  Documentar en la guía que es cancelable.
- **Hacer `pub(crate)` en search.rs:** cambio de visibilidad mínimo; no altera la API pública
  ni el comportamiento. Si se prefiriera no tocar search, la alternativa es mover
  `DirEntryRaw`/`ListResult`/`fs_lister` a un módulo común; se elige `pub(crate)` por ser el
  cambio más pequeño.
- **`depth` en `RowData`:** default 0 → cero impacto en vista normal/búsqueda.
- **Filas que llegan tarde tras cancelar:** el token-por-activación las descarta (patrón ya
  probado en búsqueda).
