# Naygo — Árbol de directorios real (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-06. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

El panel "Carpetas" (`PanePurpose::Tree`) hoy es un esqueleto: muestra la carpeta
del panel `Files` activo (con un ícono de unidad) y un botón "subir". Nicolás quiere
el **árbol expandible de verdad**, estilo Directory Opus / Explorer: raíces =
unidades, ramas colapsables, lazy-load por streaming al expandir, y navegación del
panel activo al clicar una carpeta.

**Premisa rectora:** respuesta rápida y fluida + bajo consumo. El hilo de UI NUNCA
hace I/O; todo el listado corre en workers async cancelables que se comunican por
canal (igual que el file panel). El árbol arranca instantáneo (solo enumera unidades)
y lista hijos únicamente cuando el usuario expande una rama o el auto-reveal lo pide.

### Decisiones tomadas en el brainstorm

1. **Raíces = unidades + carpeta actual resaltada.** El árbol lista todas las
   unidades del equipo como raíces; la rama de la carpeta del panel activo se
   expande/revela y resalta automáticamente (auto-sync).
2. **Solo carpetas.** El árbol muestra ÚNICAMENTE directorios (no archivos). Más
   liviano y legible; los archivos se ven en el file panel.
3. **Lazy-load async por streaming.** Al expandir, un worker lista las subcarpetas
   en streaming (cancelable); el hilo de UI no bloquea.
4. **Clic en flecha expande; clic en nombre navega.** El triángulo ▶/▼ expande/
   colapsa; clic en el NOMBRE navega el panel activo a esa carpeta.
5. **Resaltado visual (modo B):** la carpeta activa se marca con una **barra azul a
   la izquierda + fondo gris tenue** (no fondo sólido).
6. **Cargando: spinner girando** junto al nombre de la rama + fila "cargando…" tenue
   debajo, mientras llegan las subcarpetas en streaming.
7. **Estados vacía/error:** carpeta sin subcarpetas → pierde el triángulo, muestra
   "(sin subcarpetas)" tenue. Fallo (permiso/disco) → "⚠ acceso denegado" en rojo
   discreto. Nunca crashea.
8. **Auto-reveal en cascada completo:** cuando el panel activo cambia de carpeta (por
   teclado, doble clic, atrás/adelante, etc.), el árbol abre automáticamente TODOS
   los niveles necesarios hasta la carpeta actual, sin límite de profundidad, la
   resalta y hace scroll a ella.
9. **Colapsar conserva los hijos** ya cargados (no re-lista al reabrir).
10. **Paneles independientes:** el árbol es un panel más (`PanePurpose::Tree`, ya
    existe), componible/dockable como los demás. No es un sidebar fijo.

### Qué entra

- Modelo del árbol en `core` (módulo nuevo `tree`): nodos, estados, inserción de
  hijos en streaming, cálculo de la cadena de auto-reveal, búsqueda por path. Puro,
  testeable.
- Enumeración de unidades en `platform` (función nueva `drives()`).
- Variante "solo directorios" del worker de `listing`.
- Reescritura de `tree_panel.rs`: render recursivo, workers por nodo (`tree_listings`),
  expandir/colapsar, navegar, spinner, resaltado modo B, scroll al revelar.
- Auto-sync: detectar cambio de carpeta del panel activo → auto-reveal en cascada.
- Claves i18n nuevas (ES + EN): "cargando…", "(sin subcarpetas)", "acceso denegado".

### Qué NO entra

- Refresco manual de una rama / watcher de cambios (feature aparte; el watcher es
  otra sub-fase del backlog).
- Menú contextual nativo, arrastrar carpetas, renombrar desde el árbol (fases `ops`).
- Mostrar archivos en el árbol (decisión: solo carpetas).
- Favoritos/marcadores en el árbol, breadcrumbs.
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

Idea rectora: estado del árbol en `core` (testeable, sin egui) + render en `ui` +
I/O en workers async. Reutiliza canales, `CancellationToken`, `IconKey`,
`PaneRequest` ya existentes.

### Capa `core` — módulo nuevo `tree`

```rust
/// Estado de carga de un nodo del árbol.
pub enum NodeState { Collapsed, Loading, Loaded, Empty, Error }

/// Un nodo del árbol = una carpeta (o unidad). Solo carpetas.
pub struct TreeNode {
    pub path: PathBuf,
    pub name: String,            // nombre visible (carpeta) o etiqueta de unidad
    pub drive_kind: Option<DriveKind>, // Some(..) si es una raíz (unidad)
    pub expanded: bool,
    pub state: NodeState,
    pub children: Option<Vec<TreeNode>>, // None = nunca expandida (lazy)
}

/// El árbol completo: las raíces (unidades) + qué carpeta está activa.
pub struct DirTree {
    pub roots: Vec<TreeNode>,
    pub active_path: Option<PathBuf>, // la carpeta del panel activo (resaltada)
    pub reveal_to: Option<PathBuf>,   // pendiente de scroll/auto-reveal
}
```

Operaciones puras (testeables, sin I/O ni egui):

- `DirTree::from_drives(drives: &[DriveInfo]) -> DirTree` — crea las raíces Collapsed.
- `node_at_mut(&mut self, path) -> Option<&mut TreeNode>` — busca por path.
- `begin_loading(path)` — marca un nodo `Loading`, `expanded = true`.
- `push_child(path, child_dir)` — inserta una subcarpeta (llegada del worker) en su
  padre, manteniéndolas ordenadas por nombre (case-insensitive, como el file panel).
  Descarta lo que no sea carpeta (el worker solo emite carpetas, pero la API es
  defensiva).
- `finish_loading(path, outcome)` — `Loaded` / `Empty` / `Error` según resultado.
- `collapse(path)` — `expanded = false`, conserva `children`.
- `set_active(path)` — fija `active_path` y `reveal_to = Some(path)`.
- `reveal_chain(path) -> Vec<PathBuf>` — dada la carpeta actual, devuelve la cadena
  de ancestros (desde la raíz/unidad hasta el path) que deben estar expandidos. El
  consumidor (ui) lanza un worker por cada nivel aún no cargado.

El árbol **solo guarda carpetas**: cualquier hijo que llegue se asume directorio
(garantizado por el worker solo-directorios).

### Capa `platform` — enumeración de unidades (nuevo)

```rust
pub struct DriveInfo { pub path: PathBuf, pub kind: DriveKind, pub label: String }

/// Lista las unidades lógicas del sistema. Tolerante: una unidad que no responde
/// se incluye igual (su expansión dará Error), no aborta la enumeración.
pub fn drives() -> Vec<DriveInfo>;
```

Implementación Windows: `GetLogicalDriveStringsW` (crate `windows`, feature
`Win32_Storage_FileSystem` / `Win32_System_*` según el símbolo). Mapea cada unidad a
`DriveKind` (ya existe en `core::icon_kind`) — fija/red/removible/desconocida (la
clasificación fina con `GetDriveTypeW` es opcional; si no, `DriveKind::Unknown`).
`cfg(not(windows))`: stub que devuelve `vec![]` o la raíz `/` para que compile.

### Capa `core::listing` — variante solo-directorios

El worker actual (`list_into` / `entry_from_dirent`) emite todos los tipos. Se añade
un modo "solo directorios" SIN duplicar el cuerpo:

- Opción elegida: parámetro `DirsOnly(bool)` (o un enum `ListingFilter`) en una
  función hermana `spawn_listing_filtered(dir, token, filter)`; `spawn_listing`
  delega con el filtro "todos". El loop salta las entradas que no son `Directory`
  cuando el filtro es solo-directorios. Un solo punto de cambio, sin copiar lógica.

### Capa `ui` — `tree_panel.rs` reescrito + workers por nodo

- El file panel tiene UN worker por panel (`HashMap<PaneId, PaneListing>` en
  `NaygoApp`). El árbol necesita VARIOS a la vez. Se modela un
  `HashMap<PathBuf, TreeListing>` (worker por path expandido), drenado cada frame con
  un `pump_tree` análogo a `pump_all`.
- Dónde vive el estado del árbol y sus workers: como los paneles Tree son
  componibles (puede haber más de uno), el estado es **por panel**: un
  `HashMap<PaneId, DirTree>` en `NaygoApp` (junto a `listings`). Los workers también
  van por panel-y-path: `HashMap<PaneId, HashMap<PathBuf, TreeListing>>` (o una clave
  compuesta `(PaneId, PathBuf)`). Cruzan frames, así que el loop los drena. El
  `tree_panel::show` recibe el `&mut DirTree` de SU panel + un acumulador de acciones
  del árbol (estilo `PaneRequest`) para no atar el render al I/O. Un `DirTree` para un
  `PaneId` Tree se crea de forma perezosa la primera vez que ese panel se pinta
  (enumerando unidades entonces).
- **Acciones del árbol** (acumuladas durante el pintado, ejecutadas después, como
  `PaneRequest`): `Expand(path)`, `Collapse(path)`, `Navigate(path)` (→ traducido a
  `PaneRequest::NavigateTo` sobre el panel activo). Esto evita préstamos conflictivos
  igual que el patrón actual.
- **Render recursivo:** pinta cada nodo con su indentación, triángulo (si tiene o
  podría tener hijos), ícono (de `IconKey`: `Drive(kind)` para raíces, `Folder` para
  carpetas — abierta/cerrada según `expanded`), nombre. Resaltado modo B en el nodo
  cuyo `path == active_path`. Spinner en nodos `Loading`. Filas tenues para
  `Empty`/`Error`. Scroll a `reveal_to` cuando esté presente (luego se limpia).
- **Repaint:** el árbol solicita repaint mientras haya algún `tree_listing` activo
  (mismo criterio que `any_listing_active`).

### Lo que NO cambia

File panel, docking, persistencia de layout, toolbar. El árbol es un panel más.

---

## 3. Flujo de datos y ciclo de vida

**Arranque del árbol:** `platform::drives()` → `DirTree::from_drives` → raíces
Collapsed. Instantáneo, sin I/O de contenido.

**Expandir (clic ▶):** nodo → `Loading` (spinner); se lanza worker solo-directorios
para ese path en `tree_listings`. `pump_tree` drena cada frame; las subcarpetas
llegan en streaming y se insertan ordenadas. Al terminar: `Loaded` / `Empty` /
`Error`.

**Colapsar (clic ▼):** `expanded = false`, conserva hijos (no re-lista).

**Navegar (clic en el nombre):** acción `Navigate(path)` → `PaneRequest::NavigateTo`
sobre el panel activo → `NaygoApp` navega y lista (mecanismo existente). El árbol no
hace nada especial; el auto-sync reacciona.

**Auto-sync / auto-reveal:** cada frame, el árbol compara la carpeta del panel activo
con `active_path`. Si cambió: `set_active(nueva)`, calcula `reveal_chain(nueva)` y
lanza un worker por cada nivel de la cadena aún no cargado (cascada completa, sin
límite de profundidad). A medida que cada nivel termina, el siguiente ya puede
expandir. Se resalta la carpeta y se hace scroll a ella (`reveal_to`). Si un nivel
falla (permiso/disco), la cascada se detiene ahí discretamente.

**Cambios rápidos de carpeta:** al cambiar `active_path`, los workers de reveal en
vuelo que ya no sirven se cancelan vía su token (igual que `start_listing` cancela el
anterior).

**Premisa de fluidez:** nada toca disco en el hilo de UI; N ramas cargan en paralelo
(N workers); repaint solo mientras haya workers activos.

---

## 4. Manejo de errores / casos límite

- Disco caído / unidad sin responder → se lista igual; al expandir, `Error`. No
  cuelga (worker async + cancelación).
- Permiso denegado → `Error` discreto ("⚠ acceso denegado").
- Carpeta sin subcarpetas → `Empty` (pierde triángulo, "(sin subcarpetas)").
- Carpeta que desaparece estando expandida → al re-expandir da error; el nodo viejo
  no crashea (su path ya no resuelve).
- Auto-reveal a ruta inexistente (disco desconectado) → la cascada para en el último
  nivel que resuelve; sin pánico.
- Symlinks/junctions cíclicos → el árbol es lazy (solo expande lo que el usuario
  abre); el auto-reveal sigue una cadena FINITA (la ruta actual). No hay recursión
  automática infinita.
- Navegación rápida repetida → workers de reveal obsoletos cancelados por token.

---

## 5. Testing

- **`core::tree`** (el grueso, sin egui): inserción de hijos en streaming + orden;
  transiciones de estado (Collapsed→Loading→Loaded/Empty/Error); conservar hijos al
  colapsar; `reveal_chain` (cadena correcta de ancestros, incluida raíz/unidad);
  descarte/normalización; `node_at_mut` por path; `set_active`/`reveal_to`.
- **`core::listing` solo-directorios**: tempdir con archivos + carpetas mezclados →
  el worker filtrado emite SOLO carpetas; el no-filtrado sigue emitiendo todo (no
  romper tests existentes).
- **`platform::drives()`**: smoke test — en Windows al menos una unidad con path no
  vacío; en `cfg(not(windows))`, el stub compila y devuelve algo coherente.
- **`ui`** (render recursivo, spinner, scroll, resaltado): validación manual; la
  lógica con estado está extraída a `core` y testeada.

Meta de siempre: build limpio + tests + clippy antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── tree.rs                # NUEVO: DirTree, TreeNode, NodeState, reveal_chain, etc. (puro)
├── listing.rs             # + variante solo-directorios (spawn_listing_filtered / flag)
├── lib.rs                 # + pub mod tree; re-exports
└── ...

crates/platform/src/
├── drives.rs              # NUEVO: DriveInfo, drives() (GetLogicalDriveStringsW; stub no-win)
├── lib.rs                 # + pub mod drives;
└── ...

crates/ui/src/
├── panes/tree_panel.rs    # REESCRITO: render recursivo, acciones, spinner, resaltado B, scroll
├── tree_actions.rs        # (opcional) acciones del árbol (Expand/Collapse/Navigate), puras
├── app.rs                 # + HashMap<PaneId,DirTree> + tree_listings + pump_tree + auto-sync + repaint
├── docking.rs             # pasa &mut DirTree + acumulador de acciones al tree_panel
└── ...

crates/core/src/i18n/{es,en}.json  # + tree.loading, tree.empty, tree.access_denied
```

---

## 7. Dependencias

Sin dependencias nuevas de terceros. `platform` usa el crate `windows` (ya presente)
con la feature necesaria para `GetLogicalDriveStringsW` (y opcionalmente
`GetDriveTypeW`). Todo lo demás es std + lo ya usado.

---

## Fuera de alcance (recordatorio)

Refresco manual / watcher (sub-fase aparte), menú contextual nativo, drag&drop,
mostrar archivos en el árbol, favoritos/breadcrumbs en el árbol, clasificación fina
de tipo de unidad si complica. Nunca: reproducción de media, edición de archivos.
