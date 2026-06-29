# Informe de auditoría — Naygo

**Para:** Nicolás Groth (ISGroth)
**Fecha:** 2026-06-29
**Método:** auditoría multi-agente (5 dimensiones: corrección core, robustez platform, regla-de-oro UI, ajuste de texto i18n, optimización) con verificación adversarial de cada hallazgo.
**Alcance:** 27 hallazgos verificados (cada uno sobrevivió a un verificador que intentó refutarlo).

> Nota: dos de los hallazgos de UI (CfgBtn, recorte de texto en config) ya fueron
> corregidos en la rama `fix/ajustes-postmerge` ANTES de cerrar este informe — el
> verificador los refutó porque leyó el código ya arreglado.

---

## 1. Resumen ejecutivo

El código está **mayormente sano**. **No hay un solo hallazgo Critical**: ningún bug de
corrupción garantizada, crash en flujo normal, ni pérdida de datos en el camino feliz. La
arquitectura de tres capas, la caché de vista por firma y el modo de bajo consumo (timer que
duerme en reposo) están bien resueltos y desactivan por sí solos varias "tormentas de CPU por
frame" que un análisis superficial habría reportado como graves.

Lo que sí queda es real: **dos guardas de seguridad case-sensitive** (una en el camino de
borrado, con riesgo de pérdida de datos en un escenario angosto), **una lectura fuera de rango
del portapapeles** (entrada hostil), **un emparejamiento frágil del undo de Move** con
carpetas, y **dos violaciones de la regla de oro** (I/O de disco síncrono en el hilo de UI por
tick). El resto son micro-optimizaciones y recortes de texto cosméticos en alemán.

**Recuento por severidad:** 0 Critical · 4 Important · 7 Minor · 11 Optimization · (i18n/UI).

---

## 2. Bugs a corregir

### Important (4)

**I-1 · `read_text` lee el portapapeles sin acotar → out-of-bounds read**
`crates/platform/src/clipboard.rs:382-386`. `while *ptr.add(len) != 0 { len += 1; }` escanea
el `HGLOBAL` del `CF_UNICODETEXT` sin cota contra `GlobalSize`. Otro proceso puede poner un
bloque sin NUL terminador: el bucle lee memoria adyacente (info leak) o segfaultea — viola "la
app NUNCA cae". La ruta DIB sí se blindó; ésta se olvidó.
**Fix:** acotar con `GlobalSize(hglobal)/size_of::<u16>()`. Mismo PR: blindar
`read_preferred_drop_effect` (`clipboard.rs:194`, `read_unaligned` de u32 sin chequear tamaño).

**I-2 · `pre_delete`: la guarda anti-borrado del origen es case-sensitive**
`crates/core/src/ops/engine.rs:117-121` (+ `mod.rs:298`). `from == target || from.starts_with(target)`
compara case-sensitive, pero Windows es case-insensitive. Si el usuario elige "Reemplazar"
sobre un destino que es el origen con otra capitalización, `remove_dir_all(target)` borra el
árbol de origen. Trigger angosto (acción deliberada con destino case-variante).
**Fix:** case-fold ambos lados antes de comparar, en las dos guardas. (El único con riesgo de
pérdida de datos — cerrar junto con M-1.)

**I-3 · El undo de Move puede emparejar mal con carpetas o nombres ambiguos**
`crates/core/src/ops/undo.rs:61-91`. El brazo de Move empareja sources 1:1 contra `done` por
nombre+posición, pero una carpeta source produce un step por descendiente → `done` se
desincroniza y emite `MoveBack` a rutas equivocadas. Alcanzable en cada Move de carpeta real.
**Fix:** provenance explícita por step desde el engine (registrar `src → destino-final`) en
vez de re-derivar por nombre+orden. Agregar tests de carpeta y mezcla ambigua.

**I-4 · `pane_dir_missing` hace `read_dir` bloqueante en el hilo de UI por tick**
`crates/ui-slint/src/workspace_ctrl.rs:830-835` (def); `main.rs:514,1455` (uso). `read_dir(&dir).is_err()`
síncrono, sin caché ni timeout, una vez por panel dentro de `sync_rows`, en cada tick mientras
hay actividad. Sobre un share de red caído, cada tick puede bloquear el hilo de UI segundos.
**Fix:** sacar la comprobación del hilo de UI; cachear "missing" y recalcular solo ante eventos
reales (watcher de dispositivos, navegación, reintento). Tratar junto con O-10
(`pane_has_existing_ancestor`, mismo patrón).

### Minor (7)

- **M-1 · `is_inside`/`DestInsideSource` case-sensitive** (`plan.rs:178-181,297-299`) — hermano
  de I-2 en planificación. Peor caso real: anidamiento redundante de un nivel (NO recursión
  infinita — `plan`/`expand` solo leen un árbol estático). Mismo helper case-insensitive.
- **M-2 · Eventos de watcher obsoletos inyectan fila fantasma tras navegar**
  (`workspace_ctrl.rs:734-760`) — eventos encolados de la carpeta anterior (misma PaneId) se
  aplican tras repoblar. Cosmético, se auto-cura. Fix: filtrar por `parent()==current_dir` o
  epoch por panel.
- **M-3 · `close_tab` suelta el `Listing` sin cancelarlo** (`workspace_ctrl.rs:2538-2543`) —
  inconsistente con `close_pane`. Trabajo desperdiciado acotado a ~una entrada. Fix de 1 línea
  (`l.cancel()`), idealmente `impl Drop` para `Listing`.
- **M-4 · `is_valid_name` acepta espacios/puntos finales que Windows recorta**
  (`names.rs:15-25`) — preview engañoso (dos filas "Ok" que en disco son el mismo nombre). El
  consecuente "pérdida silenciosa" está sobreestimado (runtime detecta la colisión). Fix:
  rechazar/trim último char espacio o `.`.
- **M-5 · Encabezados de PDF en el preview hardcodeados en español**
  (`ui-slint/src/preview.rs:595-607`) — "PDF de {n} página(s)", "(No se pudo extraer el texto)"
  salen siempre en español. Fix: 2-3 claves i18n cableadas por `PreviewMessages`.
- **M-6 · `archive_tree` oculta hijos si un componente aparece primero como archivo**
  (`archive_tree.rs:56-77`) — precondición patológica (archiver estándar nunca la emite),
  impacto en preview de solo lectura. Fix: forzar `is_dir=true` al descender a un nodo con hijos.
- **M-7 · La vista profunda clona TODOS los `deep_items` en cada `sync_rows`**
  (`workspace_ctrl.rs:1176-1180`) — clona miles de entries ~33×/seg durante streaming. Modo
  opt-in, transitorio. Fix: destructure disjunto (patrón ya usado en el camino normal) e iterar
  por referencia.

---

## 3. Mejoras de UI / i18n (recortes de texto, idiomas largos)

Todas cosméticas, afectan solo rótulos largos (alemán). **Conviene resolver en bloque.**

| # | Dónde | Qué pasa | Fix |
|---|---|---|---|
| UI-1 | `op-dialogs.slint:355-380` (4 botones del conflicto) + DlgBtn | en alemán los `min-width` chocan en la tarjeta de 560px; el `Text` (sin `overflow:elide`) se recorta o sangra | `overflow:elide` al Text; o tarjeta más ancha / wrap 2×2 |
| UI-2 | `app-window.slint:268-283` (MenuItem) | rótulo largo desborda el menú (sin `clip`/elide) | `overflow:elide` + `horizontal-stretch` al Text; ancho de menú adaptable |
| (✅) | `config-window.slint` CfgBtn | **ya corregido** en esta rama (crece con el texto) | — |

---

## 4. Optimizaciones propuestas (por beneficio/esfuerzo)

> El timer de 30 ms **duerme en reposo**, así que casi ninguna es un drenaje sostenido — muerden
> solo en ventanas de actividad (streaming, ops, resaltados, modal). Eso baja la urgencia.

| # | Qué | Beneficio | Esfuerzo | Riesgo |
|---|---|---|---|---|
| O-1 | `sync_rows` reconstruye TODAS las filas de TODOS los paneles cada tick (`main.rs:387-449`) — re-aloca decenas de miles de Strings/seg durante streaming/ops | **Alto** (mayor sumidero sostenido en VM) | Medio | Medio |
| O-2 | `rows_from_view` usa `selected.contains(&pos)` por fila → O(filas×selección), cuadrático con Select All (`bridge.rs:105`) | Medio-alto | Bajo | Bajo |
| O-3 | `local_utc_offset_secs()` hace `GetTimeZoneInformation` por panel por tick (`workspace_ctrl.rs:1162`) | Bajo | Bajo | Bajo |
| O-4 | `IconCache::get` clona `self.active` (String) por fila antes de mirar el mapa (`icons.rs:88`) | Bajo | Bajo | Bajo |
| O-5 | `category_for_extension` aloca `to_ascii_lowercase` por fila (`icon_kind.rs:239`) | Bajo | Bajo | Bajo |
| O-6 | `view_indices()` clona el `Vec<usize>` completo aunque el caller solo quiera len (`file_pane.rs:259`) | Bajo | Medio | Bajo-medio |
| O-7 | `cmp_name` aloca 2 String/comparación; `SortKey::Kind` hace `format!("{:?}")` (`sort.rs:37,49`) | Bajo | Medio | Bajo-medio |
| O-8 | `cmp_extension` aloca String por entry (`sort.rs:57`) — ruta fría | Muy bajo | Bajo | Bajo |
| O-9 | `typeahead` aloca `name.to_lowercase()` por entry (`workspace_ctrl.rs:5229`) | Muy bajo | Bajo | Bajo |
| O-10 | `pane_has_existing_ancestor` camina ancestros con `exists()` en el hilo de UI — hermano de I-4 | Bajo (con I-4) | Bajo | Bajo |
| O-11 | `close_tab`/`close_pane` no purgan `tree_listings` del miembro cerrado | Muy bajo | Bajo | Bajo |
| O-12 | `workspace_ctrl.rs` tiene 8569 líneas (god-object, 369 métodos) — viola "una responsabilidad por módulo" | Mantenibilidad | Alto | Bajo |

---

## 5. Recomendación de prioridad

**Tanda 1 — seguridad/corrección, barato, alto valor (hacer ya):**
1. **I-1** (`read_text` acotado con `GlobalSize`) + el otro hallazgo de portapapeles
   (`read_preferred_drop_effect`). Fix de pocas líneas, copia el patrón de `read_dib`.
2. **I-2 + M-1** (case-fold en las tres guardas de containment: `engine.rs`, `mod.rs:298`,
   `plan.rs`). Un helper compartido. I-2 es el único con riesgo de pérdida de datos.

**Tanda 2 — corrección con más superficie:**
3. **I-3** (provenance explícita en el undo de Move) + tests de carpeta/ambigüedad.
4. **I-4 + O-10** (sacar `pane_dir_missing` y `pane_has_existing_ancestor` del hilo de UI con
   caché/worker).

**Tanda 3 — rendimiento de verdad (al tocar la ruta de filas):**
5. **O-1 + O-2** en el mismo PR de `sync_rows` (mejor beneficio/esfuerzo de perf; atacan el peor
   caso: VM, render por software, carpeta grande). Las micro-allocs O-3..O-9 se barren de paso.

**Diferir / oportunista:** UI-1/UI-2 + M-5 (cuando toques i18n); M-2/M-3/M-4/M-6/M-7/O-11
(fixes chicos, cuando ya estés en el archivo); O-12 (split del monolito, sesión de deuda).

**Nota honesta:** ninguno de estos 27 justifica parar el desarrollo. Los cuatro Important son
angostos en su trigger. El código base resistió bien la verificación adversarial.
