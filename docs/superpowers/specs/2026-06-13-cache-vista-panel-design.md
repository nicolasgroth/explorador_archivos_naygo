# Caché de la vista del panel (filtrado+orden) — Diseño (2026-06-13)

> Tras llevar Naygo a una VM sin GPU, hacer clic en un directorio o arrastrar el
> borde de una columna disparaba ~79% CPU. El diagnóstico (medido) descartó la GPU:
> el costo real es que el panel RECALCULA la vista (clonar+filtrar+ordenar TODAS las
> entries) en CADA frame. Este diseño cachea esa vista y la recalcula solo al cambiar.

## Contexto y medición

`file_panel::show` corre cada frame que se pinta el panel y, por frame:
- clona `f.entries` completo (`all_entries`),
- calcula `extension_counts` sobre todas,
- clona otra vez para `view` y la **re-ordena entera**.

En `C:\Windows\System32` (4845 entries, cada `Entry` = `String` + `PathBuf` + campos):
a 60fps durante un drag de columna eso son ~580k allocaciones/seg + ~290k comparaciones
de sort/seg, **redundantes** (el resultado es idéntico entre frames si nada cambió).
Ese es el 79% — CPU pura, no rasterizado. Escala con el tamaño de la carpeta.

Además, `core::FilePaneState::view_indices()` recalcula los índices por su cuenta (lo
usan teclado/selección), duplicando el trabajo.

## Decisión

Cachear la **vista** (índices filtrados+ordenados) en `core::FilePaneState`, recalculada
SOLO cuando cambian sus inputs. Beneficia al render Y al teclado, y es testeable sin egui.

## Modelo (core::FilePaneState)

Inputs que determinan la vista: `entries` (contenido), `sort`, `table.filters`,
`group_new_at_end`, `highlighted` (solo importa si `group_new_at_end`).

```rust
// Caché de la vista: los índices ya calculados + una "época" de invalidación.
struct ViewCache {
    epoch: u64,          // coincide con `view_epoch` cuando el caché es válido
    indices: Vec<usize>, // índices en `entries`, filtrados y ordenados
}
// en FilePaneState:
view_epoch: u64,                  // se incrementa cuando cambia cualquier input
view_cache: RefCell<Option<ViewCache>>,  // efímero, no se persiste ni se clona-compara
```

- **Invalidación EXPLÍCITA por época**: cualquier mutación de un input llama a
  `invalidate_view()` (incrementa `view_epoch`). Puntos a instrumentar: todo lo que
  toca `entries` (el `pump_one` del listado, `pump_watchers`, `apply_dir_events`,
  `enter`/`navigate_to` que limpia entries), `set_sort`/cambio de `sort`,
  `set_filter`/`clear_filter` (TableState), toggle de `group_new_at_end`, y los cambios
  de `highlighted` (insert/clear) SOLO si `group_new_at_end` (si no, no afecta el orden).
- `view_indices()` (sigue `&self`): si `view_cache` tiene la época actual, devuelve un
  clon de sus `indices`; si no, recalcula (la lógica actual de `view_indices_ordered`),
  guarda en el `RefCell` con la época actual, y devuelve. El `RefCell` permite el
  recompute perezoso bajo `&self` sin cambiar la firma pública.
- `Clone` de `FilePaneState`: el `view_cache` se clona como `None` (no copiar el caché;
  se reconstruye al primer uso). `view_epoch` puede copiarse o resetear a 0; lo más
  simple: `#[derive(Clone)]` con `view_cache` reconstruido vacío vía un `Clone` manual o
  marcando el campo para reset. (Se hace un `Clone` manual mínimo o se envuelve para que
  no rompa derive.)

**Por qué época y no comparar inputs:** comparar `filters`/`sort`/`len(entries)` cada
frame es barato, pero el contenido de `entries` cambia durava el streaming sin cambiar
`len` de forma fiable; una época incrementada en cada mutación es O(1), inequívoca y no
depende de heurísticas frágiles.

## Cambios en la UI

`file_panel::show` deja de clonar+ordenar. En vez de construir `view: Vec<Entry>`:
- pide `let view_idx = f.view_indices();` (cacheado) — `Vec<usize>` (índices en `entries`).
- las celdas leen `&f.entries[real]` por índice (no se clona la lista).
- `extension_counts` (para el menú de filtro): sólo se necesita cuando el menú de columna
  está ABIERTO, no cada frame. Calcularlo perezoso al abrir el menú, o cachearlo junto a
  la vista. v1: calcularlo solo si algún popup de columna está activo (barato la mayoría
  del tiempo). Si complica, cachear `ext_counts` con la misma época.

Préstamos: hoy se clona porque el closure de `body.rows` necesita los datos sin chocar
con `&mut workspace`. Con índices, se clona SOLO el `Vec<usize>` (barato) y un slice de
`entries` por referencia donde se pueda; si el borrow checker obliga, clonar `entries`
una vez al entrar a `show` se evita pasando los índices + leyendo de un `&[Entry]`
tomado antes del `TableBuilder`. Detalle de implementación a resolver en el plan.

## Tests (core)

- `view_indices` cacheada: dos llamadas seguidas sin mutar devuelven lo mismo y la 2ª no
  recalcula (verificable con un contador de recomputes detrás del RefCell en test, o
  comprobando que tras mutar `sort` el resultado cambia y antes no).
- Invalidación: cambiar `sort` → la vista refleja el nuevo orden; agregar una entry →
  aparece; cambiar filtro → filtra; toggle `group_new_at_end` → reordena.
- `Clone` no arrastra caché viejo (clonar, mutar el original, el clon recalcula bien).

## Fuera de alcance

- Cap de FPS del listado a 30fps: YA aplicado (commit en este branch).
- Virtualización extra del render: egui_extras ya virtualiza las filas; el costo era el
  recompute, no el pintado de filas.
- Cachear `extension_counts` si resulta innecesario (se evalúa en el plan).
