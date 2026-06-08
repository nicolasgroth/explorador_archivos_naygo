# Naygo — Fase sizing: tamaño de carpeta bajo demanda (F3) (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-08. Estado: aprobado (brainstorm previo), listo para plan.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Fase del sprint sobre ops-A/B, paste, shell-A, watcher y atajos-configurables (mergeadas).
Hoy las carpetas muestran `size: None` (la columna Tamaño vacía) porque calcular el tamaño
total es caro. Esta fase lo calcula **bajo demanda con F3**: suma recursiva, async,
cancelable. F3→`Action::ComputeSize` YA está en el keymap (fase atajos); `apply_action`
tiene el arm `ComputeSize => {}` no-op esperando — esta fase lo implementa.

El diseño se brainstormeó completo antes de la fase de atajos; aquí se documenta.

### Decisiones tomadas en el brainstorm

1. **Alcance:** F3 calcula el tamaño de las CARPETAS seleccionadas (o la enfocada si no hay
   selección múltiple). Cada carpeta muestra su total en la columna Tamaño; el status
   muestra el total agregado.
2. **Progreso:** parcial EN VIVO — el total acumulado va subiendo en la columna (con
   throttle para no parpadear) y se fija al terminar.
3. **Sin caché:** el tamaño calculado se guarda en `Entry.size` de esa carpeta y vive hasta
   el próximo re-listado. No hay estructura de caché (un caché con invalidación por watcher
   sería traicionero porque el watcher no ve cambios en subcarpetas profundas → mostraría
   tamaños viejos).
4. **Symlinks/junctions:** NO se siguen (evita loops infinitos y doble conteo).
5. **Permiso denegado** en una subcarpeta: se salta y se suma lo accesible; el resultado se
   marca **parcial** (si hubo accesos denegados).
6. **Opción de Configuración** `size_no_subdirs` (default `false`): si está activa, el
   cálculo NO baja a subdirectorios (suma solo el primer nivel) — más barato.
7. **Esc cancela** los cálculos de sizing en curso (las carpetas quedan con su parcial).
8. **F5 (refrescar)** re-lista la carpeta (Entry.size → None) y RE-CALCULA automáticamente
   el tamaño de las carpetas que ya lo tenían (acotado, cancelable con Esc, respeta el flag
   de subdirs). Para saber cuáles tenían tamaño, se guarda por panel un set de paths
   "con tamaño calculado".

### Qué entra

- `core::sizing`: worker recursivo cancelable + `SizeMsg`; manejo de symlink-skip / permiso
  → parcial; flag `recursive`.
- `core::config`: Settings `size_no_subdirs`.
- `ui`: `Action::ComputeSize` implementado (resuelve carpetas objetivo, lanza jobs);
  `pump_sizing` (drena parciales/total → `Entry.size` + marca parcial → repaint); Esc
  cancela; F5 recalcula lo que tenía; opción en Configuración; i18n.
- i18n ES/EN.

### Qué NO entra

- Caché persistente de tamaños / invalidación por watcher.
- Calcular todas las carpetas visibles a la vez (solo selección/enfocada).
- Seguir symlinks (se saltan).
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

### Capa `core::sizing` (módulo nuevo)

```rust
/// Mensaje del worker de cálculo de tamaño de una carpeta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SizeMsg {
    /// Acumulado parcial (bytes) mientras avanza (throttled por el worker).
    Progress { bytes: u64 },
    /// Terminó. `total` = bytes sumados; `partial` = hubo accesos denegados/errores saltados.
    Done { total: u64, partial: bool },
    /// Cancelado por el usuario; `bytes` = lo acumulado hasta el corte.
    Cancelled { bytes: u64 },
}

/// Lanza el cálculo del tamaño de `dir` en un worker. `recursive` = bajar a subcarpetas.
/// Emite `Progress` (con throttle ~150 ms) y un mensaje final por el canal devuelto.
/// Cancelable vía `token` (chequeado entre entradas). Tolerante: symlinks/junctions NO se
/// siguen; subcarpetas/archivos ilegibles se saltan marcando `partial`.
pub fn spawn_dir_size(
    dir: PathBuf,
    recursive: bool,
    token: CancellationToken,
) -> std::sync::mpsc::Receiver<SizeMsg>;
```
- El worker recorre `dir` con `std::fs::read_dir` (patrón de `ops/plan.rs`). Por cada
  entrada: lee `symlink_metadata` (NO sigue el link); si es symlink/reparse → se salta; si
  es archivo → suma `len()`; si es carpeta y `recursive` → la encola para recorrer (pila
  propia, no recursión de stack, para no desbordar en árboles profundos). Una entrada
  ilegible (permiso) → `partial = true`, se salta.
- Chequea `token.is_cancelled()` entre entradas; al cancelar, emite `Cancelled { bytes }` y
  retorna.
- Throttle del `Progress`: emite el acumulado a lo sumo cada ~150 ms (o cada N entradas),
  usando un `Instant` interno; el último `Progress` y el `Done` reflejan el total real.
- **Parte pura testeable:** una fn `dir_size_walk(root, recursive, lister, is_symlink,
  token, on_progress) -> (u64, bool)` donde el "lister" y el "is_symlink" se inyectan
  (en producción, FS real; en tests, closures sobre un árbol simulado). Devuelve
  `(total, partial)`. El `spawn_dir_size` la envuelve en el hilo + canal. Esto permite
  testear la suma recursiva, el no-seguir-symlink y el flag partial sin FS real; y también
  con tempfile para el camino real.

### Capa `core::config`

`Settings` gana `size_no_subdirs: bool` (default `false`), patrón aditivo
`#[serde(default = "default_size_no_subdirs")]` + la fn default. CONFIG_VERSION sigue 1.

### Capa `ui`

- **Estado en `NaygoApp`:** `size_jobs: HashMap<(PaneId, PathBuf), SizeJob>` donde `SizeJob
  { rx: Receiver<SizeMsg>, token: CancellationToken }`; y `sized_paths: HashMap<PaneId,
  HashSet<PathBuf>>` (paths que tienen un tamaño calculado, para el recálculo de F5). Un
  `size_partial: HashSet<PathBuf>` (o un flag por path) para marcar los parciales en el
  render.
- **`Action::ComputeSize`** (reemplaza el no-op): resuelve las carpetas objetivo
  (`selected_paths` filtradas a dirs; o la entry enfocada si es dir y no hay selección
  múltiple). Por cada carpeta: crea un `CancellationToken`, `spawn_dir_size(dir,
  !settings.size_no_subdirs, token)`, guarda el `SizeJob` en `size_jobs[(pane, dir)]`.
- **`pump_sizing` (por frame):** drena los `SizeMsg` de cada job. `Progress`/`Done` →
  escribe el valor en el `Entry.size` de esa carpeta (busca por path en `entries` del pane)
  → si `Done.partial`, agrega el path a `size_partial`; al `Done`/`Cancelled` saca el job y
  agrega el path a `sized_paths[pane]`. Repaint mientras haya jobs. El render existente
  (`format_size` → `human_size(entry.size)`) muestra el número automáticamente.
- **Marca de parcial:** la columna Tamaño de un path en `size_partial` muestra un sufijo
  (p. ej. "1.2 GB *" o "1.2 GB (parcial)") — un pequeño cambio en cómo el file panel pinta
  la celda Size para esos paths (o se concatena en el valor). i18n para el sufijo.
- **Esc:** además de cancelar el listado (CancelListing ya existe), cancela los tokens de
  TODOS los `size_jobs` activos. (El binding de Esc es CancelListing; en `apply_action` para
  CancelListing, además de cancelar el listing, cancelar los size_jobs.)
- **F5 (refrescar):** `refresh_pane` re-lista (Entry.size → None implícito al re-listar). Tras
  re-listar, para cada path en `sized_paths[pane]` que siga siendo una carpeta, re-lanzar
  `ComputeSize` para ese path (acotado a lo que tenía, cancelable, respeta el flag). Limpiar
  `size_partial` de ese pane al re-listar.
- **Config:** opción `size_no_subdirs` en Configuración (sección Avanzado o Paneles).
- **request_repaint** mientras `!size_jobs.is_empty()` para drenar parciales sin input.

### Lo que NO cambia

El motor de ops, el listing, el watcher, el árbol. El render de la columna Size solo gana el
sufijo de parcial. `Entry.size` ya es `Option<u64>` (se rellena).

---

## 3. Flujo de datos

**F3:** `ComputeSize` → carpetas objetivo (selección dirs / enfocada) → por cada una
`spawn_dir_size(dir, recursive=!size_no_subdirs, token)` → `SizeJob` en `size_jobs`.
**Worker:** recorre (pila propia, no sigue symlinks, salta ilegibles→partial, chequea token)
→ `Progress{bytes}` (throttle ~150ms) → `Done{total,partial}` o `Cancelled{bytes}`.
**pump_sizing (frame):** drena → escribe `Entry.size` de la carpeta + marca partial → al
final saca el job + registra en `sized_paths` → repaint. Status: total agregado.
**Esc:** cancela los tokens de los size_jobs. **F5:** re-lista + re-lanza ComputeSize de los
paths que estaban en `sized_paths`.

## 4. Manejo de errores / casos límite

- **Symlink/junction de carpeta** → no se entra (symlink_metadata + skip). Sin loops, sin
  doble conteo.
- **Subcarpeta/archivo sin permiso o que desaparece** → se salta, `partial = true`; sigue.
- **Carpeta vacía** → total 0.
- **Cancelación a mitad** → `Cancelled { bytes }`; el `Entry.size` queda con el último
  parcial (visible, marcado parcial). No se borra (útil ver "al menos esto").
- **F3 sobre un archivo** (no dir) → ignorado (el archivo ya tiene size).
- **F3 sobre una carpeta ya en cálculo** → cancelar el job viejo y relanzar (relanzar es lo
  intuitivo; evita dos jobs sobre el mismo path).
- **Disco de red lento** → el worker tarda pero la UI no bloquea; Esc lo corta.
- **Árbol muy profundo** → pila propia (no recursión de stack) evita stack overflow.

## 5. Testing

- **`core::sizing::dir_size_walk`** (puro, lister/is_symlink inyectados): suma recursiva de
  un árbol conocido = total esperado; modo no-recursivo suma solo primer nivel; un
  symlink/junction de carpeta NO se sigue (no suma su contenido); una entrada ilegible
  marca `partial = true` y no aborta; carpeta vacía = 0; token cancelado temprano corta.
- **`spawn_dir_size`** con tempfile (smoke en core): crear un árbol real chico, sumar, total
  correcto; cancelación produce `Cancelled`.
- **Throttle del Progress** (si el `now` es inyectable): dos progresos dentro del umbral no
  duplican emisión; si no, smoke.
- **`core::config`**: round-trip de `size_no_subdirs`.
- **UI**: validación manual (F3 sobre carpeta grande → parcial sube → total; Esc corta; F5
  limpia+recalcula; selección múltiple; opción no-subdirs; sufijo parcial en una carpeta con
  subdir protegido).

Meta: build + tests + clippy + fmt verde antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── sizing.rs        # NUEVO: SizeMsg, spawn_dir_size, dir_size_walk (puro) + tests
├── config/mod.rs    # + size_no_subdirs
├── lib.rs           # + pub mod sizing;
└── i18n/{es,en}.json # + sufijo parcial + (opcional) status de cálculo + settings

crates/ui/src/
├── app.rs           # Action::ComputeSize (real); size_jobs/sized_paths/size_partial; pump_sizing;
│                    #   Esc cancela sizing; F5 recalcula; request_repaint
├── panes/file_panel.rs  # sufijo "(parcial)" en la celda Size para paths en size_partial
└── settings_window/  # opción size_no_subdirs
```

---

## 7. Dependencias

Ninguna nueva. `CancellationToken` (core::cancel), `human_size` (core::format), `std::thread`/
`mpsc`. Sin chrono.

---

## Fuera de alcance (recordatorio)

Caché de tamaños, invalidación por watcher, calcular todo el panel, seguir symlinks. Nunca:
reproducción de media, edición de archivos.
