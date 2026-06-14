# Plan — Migración a Slint, Fase 2b: paneles especiales

> Sub-fase de la Fase 2. La 2a (splits multi-panel) está completa en `slint-fase2`
> (con el fix de modelos estables). La 2b agrega los CINCO paneles especiales con su
> contenido real, reutilizando todo lo que ya vive en `naygo-core` y `naygo-platform`.
> Gobernada por `docs/migracion-slint/CONTRATO-PARIDAD-FUNCIONAL.md`.

## Objetivo

Que el workspace pueda contener, además de paneles `Files`, los paneles:
- **Tree** (árbol de carpetas, lazy + worker solo-directorios)
- **History** (historial de deshacer, lista + validación)
- **Favorites** (favoritos + recientes, clic navega, clic derecho quita)
- **Inspector** (propiedades del ítem enfocado del panel activo)
- **Preview** (texto/imagen del archivo enfocado, worker + debounce 150 ms)

Y un menú **▾** en la barra superior para agregar cada tipo (divide el leaf activo,
igual que el botón "+").

## Principios (heredados)

- Core/platform intactos. Solo se agrega capa UI en `crates/ui-slint`.
- I/O SIEMPRE en workers (hilos + canales `mpsc`); el hilo de Slint nunca lee disco.
- Modelos Slint ESTABLES mutados in situ (lección de 2a): nunca recrear el VecModel de
  un `for` en cada frame.
- Bridges puros y testeables en core (igual que `bridge.rs` de F1): el estado del panel
  → filas/nodos planos, sin tocar tipos generados por Slint.
- Cancelación universal: cada worker recibe un `CancellationToken`.

## Estado compartido nuevo en `WorkspaceCtrl`

Hoy `WorkspaceCtrl` tiene: `ws: Workspace`, `keymap`, `listings: HashMap<PaneId, Listing>`,
typeahead, modificadores. Se agrega (espejando el `NaygoApp` de egui, pero modular):

- `trees: HashMap<PaneId, DirTree>` — un árbol por panel Tree (lazy).
- `tree_listings: HashMap<(PaneId, PathBuf), TreeListing>` — worker solo-dirs por rama.
- `favorites: naygo_core::favorites::Favorites` — global (persistencia diferida a F4/config).
- `recents: naygo_core::recent_dirs::RecentDirs` — global; se empuja al navegar.
- `undo_history: Vec<UndoEntry>` + `next_undo_id: u64` — sesión (se llenará de verdad en F3
  cuando existan operaciones; en 2b el panel se pinta vacío con su UI lista).
- `preview: PreviewState` — debounce + worker + resultado (texto/imagen).

Nota de alcance honesta: en 2b NO hay operaciones de archivo todavía (son F3), así que el
**Historial** se renderiza con su UI completa pero la lista estará vacía hasta F3. Se deja
el cableado (build_undo/validate/to_requests) listo para que F3 solo tenga que empujar
entradas. Esto se documenta para no dar falsa sensación de completitud.

## Piezas a construir (orden de implementación)

### Paso 0 — Estado compartido + andamiaje de render
- Ampliar `WorkspaceCtrl` con los campos de arriba (sin lógica aún).
- `PaneVm` en Slint gana un campo `purpose: int` (0=Files,1=Tree,2=Inspector,3=History,
  4=Favorites,5=Preview) para que el `for` elija qué componente pintar.
- En `app-window.slint`, dentro del `for p in root.panes`, un selector por `p.purpose`
  (un `if` por tipo) que instancia el componente correcto en el mismo rect.
- Menú ▾: botón en la barra que despliega 6 ítems (Files/Tree/Inspector/History/
  Favorites/Preview); cada uno llama `add-pane-of(purpose:int)`.
- `add_pane_of(purpose)` en el ctrl: como `add_pane_split` pero con el purpose dado; los
  no-Files no arrancan listing; Tree inicializa su DirTree desde `drives()`.

### Paso 1 — Inspector (el más simple, sin worker)
- `bridge::inspector_info(&FilePaneState) -> Option<InspectorInfo>` (puro, tests):
  nombre, tipo, ruta, tamaño humano, modificado, creado. Lee `focused_view_entry`.
- Componente `inspector-panel.slint`: grid etiqueta/valor; vacío si no hay foco.
- En `sync_rows`, para cada panel Inspector, setear su `InspectorVm` desde el panel
  Files activo. Modelo estable (una struct, no lista).

### Paso 2 — Favoritos + Recientes (sin worker)
- `bridge::favorite_rows(&Favorites) -> Vec<FavRow>` y `recent_rows(&RecentDirs)`.
- Componente `favorites-panel.slint`: sección Favoritos (clic navega, clic derecho/×
  quita) + sección Recientes (clic navega). Modelos estables por panel.
- Callbacks: `fav-navigate(path:string)`, `fav-remove(path:string)`,
  `recent-navigate(path:string)`. En el ctrl: navegar el panel Files activo + arrancar
  su listing; quitar del favorites.
- Empujar a `recents` en cada navegación (double-click dir, go_up, tree click, fav click).

### Paso 3 — Historial (lectura + validación, sin worker)
- `bridge::history_rows(&[UndoEntry]) -> Vec<HistRow>` con `label`, `when`, `count`,
  `undoable: bool` (vía `undo::validate`), `reason: String` (si no es deshacible).
- Componente `history-panel.slint`: lista; botón "Deshacer" habilitado/deshabilitado
  con tooltip de motivo. (En 2b la lista está vacía; UI lista para F3.)
- Callback `undo-entry(id:int)` cableado al ctrl (no-op útil hasta F3, pero correcto).

### Paso 4 — Árbol (worker solo-directorios + lazy + reveal)
- `listing.rs`: agregar `Listing::start_dirs_only(dir)` o un `TreeListing` propio que use
  `naygo_core::listing::spawn_listing_filtered(.., ListingFilter::DirsOnly)`.
- `bridge::tree_rows(&DirTree) -> Vec<TreeRow>`: APLANA el árbol a una lista con
  `depth`, `name`, `path`, `expanded`, `has_children`, `state`, `is_drive`, `active`,
  `disk_percent` (para raíces, vía `read_space`/`DiskUsage`). PURO, tests.
- Componente `tree-panel.slint`: `for` plano con sangría `depth*14px`; ▶/▼ expande/
  colapsa; clic navega el panel Files activo; raíces con barrita de uso de disco;
  favoritos anclados arriba (reutiliza FavRow). ListView estable.
- Ctrl: `tree_expand(id,path)` (begin_loading + worker, o expand_loaded si ya cargó),
  `tree_collapse(id,path)` (cancela token + collapse), `pump_tree()` (drena → push_child
  + finish_loading), `tree_navigate(path)` (navega Files activo). `set_active` del árbol
  cuando cambia la carpeta del panel Files activo (resalta + reveal).
- El timer ahora también drena `pump_tree`; se reactiva al expandir.

### Paso 5 — Preview (worker + debounce 150 ms + texto/imagen)
- Agregar dep `image = { version = "0.25", default-features = false,
  features = ["png","jpeg","gif","bmp","webp","ico"] }` a `ui-slint/Cargo.toml`.
- `preview.rs` (nuevo en ui-slint): `PreviewState` (wanted/since/loaded/view/rx/token),
  worker que clasifica con `preview::classify_rules` y lee texto (truncate_text) o decodifica
  imagen (crate image → RGBA escalada a IMAGE_MAX_SIDE), debounce 150 ms, cancelación por
  token. Reusa la lógica del worker de egui pero produce `slint::Image` vía
  `SharedPixelBuffer::clone_from_slice` en el hilo de UI.
- Componente `preview-panel.slint`: texto monospace (+aviso de truncado) | imagen
  (ImageFit Contain) | mensaje i18n. Modelo estable.
- Ctrl: `pump_preview()` (mira el focused_view_entry del Files activo; debounce; arranca/
  drena worker). El timer lo drena mientras haya un Preview con worker en vuelo o debounce
  pendiente.

## Testing
- core/bridges puros: `inspector_info`, `favorite_rows`, `recent_rows`, `history_rows`,
  `tree_rows` (aplanado con sangría, estados, raíces con disco), clasificación de preview.
- ui-slint: que `add_pane_of` cree el purpose correcto y NO arranque listing para no-Files;
  que el árbol expanda/colapse y pueble por path; que el debounce de preview respete el
  plazo; que navegar desde favoritos/recientes/árbol arranque el listing del Files activo.
- Verificación viva (Nicolás, VM): agregar cada panel desde ▾, árbol expandible, clic
  navega, preview de un .txt y un .png, inspector refleja el foco, favoritos navegan;
  CPU bajo al interactuar y en reposo 0%.

## Puertas (antes de cada commit)
`cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings`
+ `cargo fmt --all -- --check`. Stagear rutas EXPLÍCITAS (no `git add -A`: hay un cambio
pendiente no relacionado en CLAUDE.md). Commits en español. NO merge a main hasta el visto
bueno visual + CPU de Nicolás en la VM.

## Riesgos / decisiones
- **Imágenes en render por software:** decodificar en worker (Vec<u8> RGBA), construir el
  `slint::Image` en el hilo de UI. Escalar a IMAGE_MAX_SIDE para no subir buffers gigantes.
  Se mide la CPU al previsualizar imágenes en la VM; si cuesta, se limita el tamaño.
- **Historial vacío en 2b:** es correcto (no hay ops hasta F3). Se documenta; la UI queda
  lista para que F3 solo empuje entradas.
- **Persistencia de favoritos:** se carga/guarda de verdad en F4 (config). En 2b vive en
  memoria de sesión para no adelantar el módulo de config. (Decisión: no bloquear 2b por
  persistencia; el modelo core ya serializa.)
- **reveal/scroll-to del árbol y del panel:** el `scroll-y` por panel sigue siendo el GAP
  conocido de 2a; el reveal del árbol se cablea aquí en lo posible, pero el scroll fino
  puede quedar para 2c/pulido si el render por software lo complica.
