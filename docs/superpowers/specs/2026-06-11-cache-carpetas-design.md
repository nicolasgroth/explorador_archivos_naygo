# Caché de carpetas visitadas + recientes — Diseño

> Fase aprobada por Nicolás (2026-06-11): "el caché e historial de las carpetas
> anteriores creo que es muy importante". Primera de la serie
> caché → path-bar+favoritos. Apunta directo a la prioridad nº1 del proyecto:
> velocidad de navegación percibida.

## Objetivo

Volver a una carpeta ya visitada pinta su contenido **al instante** desde un
caché en memoria, mientras un listado normal corre por detrás y corrige
cualquier diferencia (*stale-while-revalidate*). Atrás/adelante se vuelven
instantáneos. Además queda registrado un historial global de carpetas
recientes (persistente) que alimentará el menú del botón atrás ahora y el
autocompletado/panel de favoritos en la fase siguiente.

## Comportamiento (UX)

- Navegar a una carpeta **cacheada**: el contenido aparece en el MISMO frame
  (sin parpadeo a vacío), y el listado streaming de siempre corre por detrás;
  si algo cambió, la vista se corrige al llegar (como hoy). El spinner de
  "listando" se muestra discreto igual (la revalidación es real I/O).
- Navegar a una carpeta **no cacheada**: exactamente como hoy (streaming).
- La selección/foco NO se preserva entre visitas (como hoy); solo el contenido.
- Botones atrás/adelante: mismo mecanismo (instantáneos si la carpeta sigue
  cacheada). **Clic derecho** sobre atrás/adelante abre un menú con el
  historial de navegación del panel activo (saltar N pasos de una vez).
- Cero datos viejos persistentes: el caché vive SOLO en memoria (se pierde al
  cerrar; los `recents` sí persisten, pero son solo rutas).

## Core (`crates/core/src/listing_cache.rs`, puro)

```rust
pub struct ListingCache { /* HashMap<PathBuf, CachedListing> + orden LRU */ }
pub struct CachedListing { pub entries: Vec<Entry>, pub cached_at_epoch: u64 }

impl ListingCache {
    pub fn new(max_dirs: usize, max_total_entries: usize) -> Self;
    pub fn get(&mut self, dir: &Path) -> Option<&CachedListing>; // toca el LRU
    pub fn put(&mut self, dir: PathBuf, entries: Vec<Entry>);    // evict LRU si excede
    pub fn invalidate(&mut self, dir: &Path);
    pub fn clear(&mut self);
}
```

- Límites por defecto: **50 carpetas / 50 000 entries totales** (≈ ~20 MB peor
  caso). El que se exceda primero desaloja por LRU. Una carpeta más grande que
  el tope total simplemente no se cachea.
- Tests: hit/miss, LRU (orden de desalojo), tope por entries, invalidate,
  put reemplaza.

### Recientes (`crates/core/src/recent_dirs.rs`, puro)

```rust
pub struct RecentDirs { /* Vec<PathBuf>, MRU, cap 30, dedup */ }
impl RecentDirs { push(dir), list() -> &[PathBuf], remove_missing(), (de)serialize }
```

- Persistencia en `<config>/recents.json` (guardado con debounce al navegar,
  como el resto del estado). Carga tolerante (archivo corrupto → lista vacía).

## Integración (app)

- `NaygoApp` posee `listing_cache: ListingCache` (una, compartida entre paneles)
  y `recent_dirs: RecentDirs`.
- En `start_listing(id, dir)`: si `cache.get(dir)` → poblar `f.entries` de
  inmediato (la vista pinta este frame) y lanzar el listado streaming normal.
  El pump existente reemplaza entries al llegar los resultados frescos
  (semántica actual, sin cambios).
- Al COMPLETAR un listado (Done del pump): `cache.put(dir, entries_finales)`.
  Un listado cancelado o con error NO escribe el caché.
- Watcher: un evento de cambio en `dir` → `invalidate(dir)` (el refresh que ya
  dispara el watcher la re-cachea al completar).
- Cambio de disco/unidad desaparecida: el listado fresco falla como hoy; la
  entrada cacheada se invalida al fallar la revalidación.
- Navegación exitosa → `recent_dirs.push(dir)`.

## Configuración

- Sección Avanzado: "Caché de carpetas (carpetas máx.)" con 0 = desactivado
  (default 50). i18n ES/EN. El cambio aplica en caliente (recrear el caché).

## Fuera de alcance (fases siguientes)

- Panel/sección "Recientes" visible (va con el panel Favoritos).
- Autocompletado del path (fase path-bar; consumirá `RecentDirs`).
- Preservar selección/scroll por carpeta (posible mejora futura).
- Persistir el caché de listados a disco.

## Riesgos y mitigación

- **Datos obsoletos visibles unos ms**: aceptado a propósito (es el punto del
  stale-while-revalidate); la revalidación corre SIEMPRE.
- **Memoria**: topes duros por dirs y entries totales + LRU; configurable.
- **Coherencia con ops** (rename/move/delete): las ops ya refrescan el panel
  → el listado fresco re-escribe el caché; el watcher invalida ante cambios
  externos. No hay camino que deje el caché viejo sin revalidación al volver
  a mostrarse (mostrar = revalidar siempre).
