# Naygo — Fase watcher: vigilar la carpeta visible + detectar dispositivos (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-08. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Quinta fase del sprint, sobre ops-A/ops-B/paste-inteligente/shell-A (mergeadas). El módulo
`watcher` figuraba como "hueco futuro" en el spec original; esta fase lo construye.

Dos subsistemas **técnicamente distintos**, hechos en una fase pero **deliberadamente
aislados** para que el riesgo de uno no afecte al otro:

- **(W) Watcher de carpeta:** detectar altas/bajas/cambios/renames en las carpetas
  visibles y reflejarlos en el listado EN VIVO (sin refresco manual); lo recién aparecido
  se RESALTA. Vía crate `notify`.
- **(D) Detección de dispositivos:** reaccionar al instante cuando se enchufa/quita una
  unidad (pendrive), en vez del re-escaneo periódico de shell-A. Vía una ventana
  message-only Win32 que escucha `WM_DEVICECHANGE`.

### Decisiones tomadas en el brainstorm

1. **Una fase, dos subsistemas aislados** (W y D). D no toca el HWND de eframe.
2. **Mecanismo W: crate `notify`** (maduro, MIT/Apache, envuelve ReadDirectoryChangesW y
   maneja las ráfagas/overlapped I/O). **Mecanismo D: Win32 directo** (notify no cubre
   WM_DEVICECHANGE).
3. **Vigilar TODOS los paneles de archivos visibles** (un watcher por panel; se re-apunta
   al navegar; se suelta al cerrar el panel).
4. **Resaltado estilo A:** fondo de fila teñido + nombre en color, desde un **token de tema
   nuevo** (`highlight`). Combina en todos los temas.
5. **Duración del resaltado: configurable, default "hasta interactuar"** con el panel
   (clic/navegar/mover foco). Otros modos: "desvanecer a los N s" (con repaint temporizado)
   y "fijo hasta refrescar".
6. **Debounce ~300 ms:** los eventos del SO se coalescen; una copia de 100 archivos = 1-2
   actualizaciones, no 100.
7. **2 modos de inserción de lo nuevo, configurable:**
   - **"ordenado"** (default): el nuevo entra y la vista lo ordena en su lugar según la
     columna activa, resaltado.
   - **"al final":** las entradas resaltadas (las nuevas) se fijan al FINAL de la vista
     mientras siguen resaltadas (orden estable), por encima del orden de columna; al
     limpiarse el resaltado vuelven a su posición ordenada natural.
8. **Resaltado como estado de presentación en `FilePaneState`** (un `highlighted`), NO se
   toca `Entry` (modelo de datos puro).
9. **D reusa la maquinaria de discos de shell-A:** al detectar un cambio de volumen, dispara
   `start_disk_scan()` de inmediato; el re-escaneo periódico de shell-A queda como respaldo.

### Qué entra

- `platform::dir_watch`: `notify` envuelto, eventos coalescidos (~300 ms) → `DirEvent`.
- `platform::device_watch`: ventana message-only Win32, `WM_DEVICECHANGE` → `DeviceEvent`.
- `core::listing::apply_dir_events`: merge incremental puro de eventos al `Vec<Entry>`.
- `core::theme`: token `highlight` (additivo, tolerante).
- `core::workspace::FilePaneState`: `highlighted` (estado de resaltado).
- `core::config`: Settings de watcher (duración del resaltado, modo de inserción).
- `ui`: watchers por panel + pump; render del resaltado estilo A + modo "al final"; pump de
  dispositivos → `start_disk_scan`; opciones en Configuración.
- i18n ES/EN.

### Qué NO entra

- Vigilancia recursiva de subárboles (solo la carpeta visible de cada panel, no sus hijos).
- Pausar el watcher al minimizar (optimización futura).
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

Dos subsistemas aislados que comparten solo el patrón "worker → canal mpsc → pump por
frame" (el mismo de listing/ops).

### (W) Watcher de carpeta

**`platform::dir_watch` (módulo nuevo):**
```rust
/// Cambio normalizado en una carpeta vigilada.
#[derive(Clone, Debug, PartialEq)]
pub enum DirEvent {
    Created(PathBuf),
    Removed(PathBuf),
    Modified(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

/// Handle de un watcher activo. Al dropearse, detiene el watcher (libera el handle del SO).
pub struct WatchHandle { /* contiene el notify::Watcher + el hilo de coalescing */ }

/// Empieza a vigilar `dir` (no recursivo). Los eventos crudos de notify se normalizan a
/// `DirEvent` y se coalescen ~300 ms; se emiten en lote por `tx`. Tolerante: si notify no
/// puede observar (red, permiso), el handle queda inerte (no crashea; el panel no se
/// auto-actualiza, pero sigue usable).
pub fn watch(dir: &Path, tx: Sender<Vec<DirEvent>>) -> WatchHandle;
```
- Usa `notify` (recommended watcher) + un debouncer (el propio `notify-debouncer-full`, o un
  coalescing thread propio con ~300 ms). Normaliza `notify::EventKind` → `DirEvent`
  (Create→Created, Remove→Removed, Modify→Modified, Modify::Name/rename→Renamed o
  Removed+Created si notify no entrega el par).
- `WatchHandle` Drop detiene el watcher y el hilo de coalescing.

**`core::listing::apply_dir_events` (núcleo, puro):**
```rust
/// Metadata mínima para construir/actualizar un `Entry` (inyectable para test).
pub struct EntryMeta { pub kind: EntryKind, pub size: Option<u64>, pub modified: Option<SystemTime>, pub created: Option<SystemTime>, pub hidden: bool }

/// Aplica `events` a `entries` SIN re-listar. `read_meta` produce la metadata de una ruta
/// (en producción, del FS; en tests, un closure). Devuelve las rutas NUEVAS (para resaltar).
pub fn apply_dir_events(
    entries: &mut Vec<Entry>,
    events: &[DirEvent],
    read_meta: &dyn Fn(&Path) -> Option<EntryMeta>,
) -> Vec<PathBuf>;
```
- Created → si `read_meta` da metadata y la ruta no está ya, construye `Entry` e inserta;
  agrega a "nuevas". Removed → quita por path. Modified → si existe, actualiza
  size/modified/created. Renamed{from,to} → renombra el Entry (path+name) o, si `from` no
  está, trata `to` como Created. Idempotente (un Created de algo ya presente no duplica).
- El re-ordenar/re-filtrar la vista lo hace el código de `FilePaneState` tras el merge.
- Para construir un `Entry` desde una ruta, se extrae/reusa la lógica de `entry_from_dirent`
  (hoy toma `&DirEntry`) a una variante que tome `&Path` + metadata (compartida por listing
  y por el merge), de modo que un Entry creado por el watcher sea idéntico al del listado.

**`core::workspace::FilePaneState` (campo nuevo):**
```rust
    /// Rutas resaltadas como "recién aparecidas" (estado de presentación; no es parte del
    /// modelo de datos del listado). El render las tiñe; la interacción/refresh las limpia.
    pub highlighted: std::collections::HashSet<PathBuf>,
```
(serde: `#[serde(default, skip)]` o no-serializado — es estado efímero de sesión, no se
persiste. Verificar cómo se (de)serializa `FilePaneState`: si se persiste en el workspace,
`highlighted` se marca `#[serde(skip)]` para no guardarlo.) Métodos: `clear_highlight()`,
`is_highlighted(&Path)`.

### (D) Detección de dispositivos

**`platform::device_watch` (módulo nuevo, AISLADO):**
```rust
pub enum DeviceEvent { DrivesChanged }
pub struct DeviceWatchHandle { /* hilo + hwnd message-only */ }

/// Arranca un hilo con una ventana message-only (HWND_MESSAGE) que escucha WM_DEVICECHANGE
/// (DBT_DEVICEARRIVAL / DBT_DEVICEREMOVECOMPLETE de volúmenes) y emite `DrivesChanged` por
/// `tx`. NO toca el HWND de eframe. Al dropear el handle, postea WM_QUIT y une el hilo.
/// Tolerante: si la ventana no se crea, el handle queda inerte (los discos siguen con el
/// re-escaneo periódico de shell-A).
pub fn watch(tx: Sender<DeviceEvent>) -> DeviceWatchHandle;
```
- `#[cfg(windows)]`: `RegisterClassW` + `CreateWindowExW(HWND_MESSAGE)` + bucle
  `GetMessageW`/`DispatchMessageW`; el wndproc detecta `WM_DEVICECHANGE` con
  `DBT_DEVICEARRIVAL`/`DBT_DEVICEREMOVECOMPLETE` y envía `DrivesChanged`. `#[cfg(not(windows))]`
  stub: handle inerte.
- AISLAMIENTO: vive en su propio hilo y su propia ventana oculta; un fallo aquí no afecta la
  ventana principal de eframe ni el watcher de carpeta.

### Capa `ui`

- Estado: por cada panel de archivos, un `WatchHandle` (en un `HashMap<PaneId, WatchHandle>`
  análogo a `listings`/`trees`); creado al listar una carpeta, recreado al navegar, soltado
  al cerrar el panel. Un `Sender<Vec<DirEvent>>` etiquetado por PaneId (o un canal por panel).
- `pump_watchers` (por frame): drena los lotes de `DirEvent` de cada panel →
  `apply_dir_events` sobre las entries de ese pane → agrega rutas nuevas a
  `highlighted` → recomputa la vista (orden+filtro de 2E, + "al final" si el modo lo pide) →
  repaint.
- `pump_devices` (por frame): drena `DeviceEvent::DrivesChanged` → `self.start_disk_scan()`.
- **Limpieza del resaltado** según `Settings.highlight_duration`: "hasta interactuar" → en
  `handle_input`/clic sobre el panel, `pane.clear_highlight()`; "desvanecer Ns" → guardar el
  `Instant` por ruta (un `HashMap<PathBuf, Instant>` en vez de set; o un timestamp de lote) y
  quitar al expirar (repaint mientras haya resaltados); "fijo" → limpiar solo al re-listar.
- **Render estilo A** en el file panel: para una fila cuyo path está en `highlighted`, pintar
  el fondo con el token `highlight` del tema (teñido suave) y el nombre con un color derivado.
- **Modo "al final"**: la función de vista (que ya ordena+filtra) aplica, si
  `Settings.new_items_at_end`, un paso estable que mueve las filas resaltadas al final.

### Capa `core::theme`

`Theme` gana `highlight: ThemeColor` (token nuevo). Additivo y tolerante: el deserializador
(que ya usa `Option<ThemeColor>` por campo) cae a un default si falta; los 4 temas embebidos
reciben un valor sensato (un verde/acento tenue por tema). La Serialize manual incluye el
campo nuevo.

### Capa `core::config`

`Settings` gana (serde `#[serde(default = "fn")]`, additivo, CONFIG_VERSION sigue 1):
`highlight_duration: HighlightDuration { UntilInteract, FadeSeconds(u32), UntilRefresh }`
(default `UntilInteract`) y `new_items_at_end: bool` (default `false` = ordenado).

### Lo que NO cambia

El motor de ops, el listing base, el árbol, los discos de shell-A (solo se les añade el
disparo inmediato), la tabla de 2E (se le añade el paso opcional "al final" y el tinte).

---

## 3. Flujo de datos

**Carpeta:** panel muestra `dir` → `dir_watch::watch(dir, tx)` → WatchHandle por panel.
notify → eventos crudos → normaliza+coalesce ~300 ms → lote de `DirEvent` por canal →
`pump_watchers` → `apply_dir_events(&mut entries, &lote, read_meta)` → rutas nuevas →
`highlighted` → recomputar vista (+"al final" si aplica) → repaint. Limpieza del resaltado
según el modo configurado.

**Dispositivos:** arranque → `device_watch::watch(tx)` → ventana message-only en su hilo.
WM_DEVICECHANGE → `DrivesChanged` → `pump_devices` → `start_disk_scan()` (shell-A) → árbol y
strip de discos al instante.

---

## 4. Manejo de errores / casos límite

- **Carpeta vigilada desaparece** → notify emite remove del root o falla → el panel conserva
  su listado; al interactuar, el re-listing da error manejado (flujo hostil existente). Sin
  crash.
- **Disco de red / latencia** → notify puede no entregar eventos (límite del SO) →
  degradación elegante: no auto-actualiza, pero F5/renavegar sí. Documentado.
- **Ráfaga enorme** (p. ej. 10.000 archivos) → debounce coalesce; si un lote supera un umbral
  (p. ej. > 1000 eventos), en vez del merge incremental se re-lista la carpeta entera (más
  barato) y se difde para marcar nuevas. Fallback interno.
- **Evento de un archivo que no pasa el filtro activo** → entra al `Vec` pero la vista no lo
  muestra; no se resalta visiblemente. Correcto.
- **Rename** → un `DirEvent::Renamed`; si notify lo da como remove+create, el merge lo trata
  como baja+alta (el nuevo nombre se resalta). Aceptable.
- **Panel cerrado** → su WatchHandle se dropea (libera el handle del SO).
- **device_watch falla al crear la ventana** → handle inerte; discos con re-escaneo de
  shell-A. Sin crash.

---

## 5. Testing

- **`core::listing::apply_dir_events`** (el grueso puro, `read_meta` inyectado): Created
  inserta + aparece en nuevas; Removed quita; Modified actualiza size/fecha; Renamed
  renombra (y Renamed con `from` ausente → Created); idempotencia (Created de algo presente
  no duplica); lote mixto; rutas nuevas devueltas correctas. Sin tocar disco.
- **Normalización/coalescing** (si es lógica pura separable en `dir_watch`): mapear eventos
  crudos→DirEvent y dedup en ventana. El I/O de notify se valida con smoke manual.
- **`core::theme` highlight**: deserializar un tema sin `highlight` cae al default
  (tolerante); round-trip con el campo.
- **`core::config`**: round-trip de `highlight_duration`/`new_items_at_end` (additivo).
- **Vista "al final"**: dado un set `highlighted`, la vista mueve esas filas al final de
  forma estable; sin highlighted, orden normal (test puro sobre la función de vista).
- **`platform::dir_watch` / `device_watch`**: smoke manual (crear/borrar archivo en carpeta
  vigilada → DirEvent; enchufar/quitar pendrive → DrivesChanged + árbol actualizado).
- **UI**: validación manual (copiar a un panel visible → resaltado en vivo; 3 modos de
  duración; modo "al final"; pendrive en vivo).

Meta: build + tests + clippy + fmt verde antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/platform/src/
├── dir_watch.rs     # NUEVO: notify envuelto → DirEvent (coalesce ~300ms) + WatchHandle
├── device_watch.rs  # NUEVO: ventana message-only Win32 → DeviceEvent (WM_DEVICECHANGE) + stub
├── lib.rs           # + pub mod dir_watch; pub mod device_watch;
└── Cargo.toml       # + notify (y debouncer); + features windows si faltan (Devices_*, WindowsAndMessaging)

crates/core/src/
├── listing.rs       # + apply_dir_events + EntryMeta; extraer entry_from_path (compartido)
├── theme/mod.rs     # + token highlight (Serialize manual + deserialize tolerante + 4 temas)
├── workspace/file_pane.rs  # + highlighted: HashSet<PathBuf> (#[serde(skip)]) + clear_highlight
├── config/mod.rs    # + HighlightDuration + new_items_at_end (serde default)
└── i18n/{es,en}.json # + claves de config del watcher

crates/ui/src/
├── app.rs           # watchers por panel (HashMap<PaneId, WatchHandle>) + pump_watchers + pump_devices + device_watch al arrancar + limpieza de highlight
├── panes/file_panel.rs  # render estilo A (fondo highlight + nombre) para filas en highlighted
├── (vista)          # paso "al final" opcional en el cómputo de la vista
├── theme_apply.rs   # getter highlight() del tema activo
└── settings_window/  # opciones de watcher (duración del resaltado, nuevos al final)
```

---

## 7. Dependencias

- **`notify`** (y opcionalmente `notify-debouncer-full`) — MIT/Apache, para el watcher de
  carpeta. Árbol moderado (mio, etc.). Cumple "dependencias libres".
- Crate `windows` 0.62: features para `WM_DEVICECHANGE`/message-only window
  (`Win32_UI_WindowsAndMessaging`, `Win32_System_LibraryLoader` para GetModuleHandle si hace
  falta) y para detección de volúmenes. Sin chrono.

---

## Fuera de alcance (recordatorio)

Vigilancia recursiva, pausar al minimizar, menú COM nativo (shell-B). Nunca: reproducción
de media, edición de archivos.
