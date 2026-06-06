# Naygo — Fase 2D: Pulidos de listado (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-06. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Tras ver Naygo corriendo con dual-pane, íconos y configuración bilingüe (fases 1,
2A, 2B, 2C-i), Nicolás pidió varios ajustes. Esta sub-fase **2D** agrupa los
**pulidos rápidos del listado** (los más chicos y satisfactorios), antes del árbol
real, el watcher y los temas:

- La **fila `..`** se ve como una entrada de directorio normal (estilo Total
  Commander), no "tenue"/separada.
- **Ordenamiento por columnas clicables** en el panel de archivos (clic en el
  encabezado ordena, toggle asc/desc, con indicador).
- **Nuevos criterios de orden**: extensión y fecha de creación (además de los
  existentes nombre / tamaño / fecha de modificación).

**Premisa rectora:** respuesta rápida y fluida. Ordenar re-ordena el `Vec` en
memoria (no re-lista el disco); el criterio se aplica con la función pura
`sort_entries` ya existente. Capturar la fecha de creación es una lectura más de
metadata que el worker de `listing` ya hace para `modified` — sin costo extra
perceptible.

### Qué entra en 2D

- Fila `..` con ícono de carpeta normal + mismo estilo/alineación que las demás
  filas (indistinguible visualmente de una carpeta; sigue siendo la primera fila y
  sube al activarla con un clic).
- `Entry.created: Option<SystemTime>` + captura en `listing`.
- `SortKey::Extension` y `SortKey::Created` + su implementación en `sort_entries`.
- Encabezados de columna clicables (Nombre / Tamaño / Modificado) → ordenar +
  toggle asc/desc + indicador de dirección (▲/▼).
- Menú contextual del encabezado (clic derecho): "Ordenar por" con los 5 criterios
  (Nombre / Extensión / Tamaño / Fecha modificación / Fecha creación) + dirección +
  "Carpetas primero". Cubre los criterios sin columna propia (extensión, creación).
- Textos nuevos a i18n (ES + EN).

### Qué NO entra en 2D

Árbol de directorios real (sub-fase aparte, siguiente); watcher / detección de
archivos nuevos (sub-fase aparte); temas / color sets / packs (2C-ii); columnas
visibles adicionales para extensión/creación (se acceden por el menú, no se agrega
una columna nueva al layout); cambiar el `SortKey::Kind` existente (queda como
está, no expuesto en UI). Nunca: reproducción de media, edición.

---

## 2. Arquitectura

Idea rectora intacta: `core` decide el ordenamiento (puro, testeable); `ui` pinta y
captura el clic. La fila `..` es UI pura (no una `Entry`).

### Capa `core`

- **`fs_model::Entry`**: gana `created: Option<SystemTime>` (junto a `modified`).
  Es un campo más; los constructores de test y `entry_from_dirent` lo rellenan.
- **`fs_model::SortKey`**: gana `Extension` y `Created`. (Ya tiene `Name`, `Size`,
  `Modified`, `Kind`.)
- **`sort::sort_entries`**: añade los brazos del match:
  - `SortKey::Extension` → comparar la extensión del `path` (minúsculas,
    case-insensitive); sin extensión ordena como cadena vacía.
  - `SortKey::Created` → comparar `created` (`Option<SystemTime>`, igual que
    `modified`).
  Mantiene `dirs_first` y la estabilidad (sort_by) ya existentes.
- **`listing::entry_from_dirent`**: captura `metadata.created().ok()` para `created`
  (análogo a `modified`). Tolerante: si el SO no lo entrega, `None`.

### Capa `ui` — `panes/file_panel.rs`

- **Fila `..` normal**: usar `IconKey::Folder` (no un ícono especial) y el mismo
  `icon_row(...)` que las entries, con el nombre `".."`. Misma alineación y estilo.
  Sigue siendo la primera fila (cuando hay padre y la opción está activa) y emite
  `NavigateTo { parent }` con un clic. Sin atenuado ni separación visual.
- **Encabezados clicables**: cada encabezado (Nombre/Tamaño/Modificado) se pinta con
  un widget clicable (p. ej. `ui.selectable_label` o un `Button` plano) que muestra
  el título + un indicador ▲/▼ si es la columna activa. Al clicar:
  - Si la columna ya es la activa (`sort.key` coincide) → invertir `sort.ascending`.
  - Si no → activar esa `SortKey` con `ascending = true`.
  - Tras el cambio, re-ordenar las entries del panel con `sort_entries` (sin
    re-listar). La selección/foco se preserva razonablemente (ver "edge cases").
- **Menú contextual del encabezado** (clic derecho sobre la zona de encabezados):
  un menú "Ordenar por" con los 5 criterios, "Ascendente/Descendente", y "Carpetas
  primero" (toggle de `dirs_first`). Aplica igual que el clic de columna + re-sort.
- La lógica "qué SortSpec resulta de clicar la columna X" se extrae a una **función
  pura testeable** (recibe el SortSpec actual + la columna clicada, devuelve el
  nuevo SortSpec) para no atar la decisión al render.
- El `SortSpec` vive en `FilePaneState` (ya se persiste vía `FilePanePersist`).

### Mapeo columna ↔ SortKey

| Encabezado visible | SortKey |
|---|---|
| Nombre | `Name` |
| Tamaño | `Size` |
| Modificado | `Modified` |
| (menú) Extensión | `Extension` |
| (menú) Fecha de creación | `Created` |

---

## 3. Modelo de interacción

- **Ordenar por columna**: clic en "Nombre"/"Tamaño"/"Modificado" → ordena por esa
  columna ascendente; segundo clic en la misma → descendente; indicador ▲/▼ en la
  activa.
- **Ordenar por extensión o fecha de creación**: clic derecho en los encabezados →
  "Ordenar por → Extensión / Fecha de creación", o cualquiera de los 5; ahí también
  Ascendente/Descendente y "Carpetas primero".
- **Fila `..`**: primera fila del panel (si hay padre y la opción está on), con
  ícono de carpeta normal; un clic sube al directorio padre.
- El criterio elegido persiste por panel (al cerrar/reabrir Naygo se conserva).

---

## 4. Manejo de errores / edge cases

Filosofía del proyecto:
- `created` ausente (el SO no lo entrega, o un FS sin ese dato) → `None`; al ordenar
  por creación, los `None` se agrupan de forma consistente (igual que `modified`
  hoy). No crashea.
- Extensión inexistente (archivos sin punto, carpetas) → se trata como cadena vacía
  al comparar; consistente y sin panic.
- Re-ordenar tras un clic de columna mientras el panel está listando: el sort se
  aplica sobre lo que haya; cuando el listado termine, `pump_one` re-ordena con el
  `SortSpec` vigente (ya lo hace). El foco (`focused: Option<usize>`) puede quedar
  apuntando a otra fila tras re-ordenar — aceptable; opción: re-fijar foco a 0 o
  preservar el ítem enfocado por path (mejora menor, decidir en el plan; lo simple
  es dejar el índice y que `focused_entry()` siga siendo seguro vía `.get()`).
- La fila `..` nunca participa del ordenamiento ni del foco/type-ahead (sigue siendo
  UI pura, como en 2B).

---

## 5. Testing

- **`sort` (core)**: tests nuevos para `SortKey::Extension` (asc/desc,
  case-insensitive, sin-extensión como vacío) y `SortKey::Created` (asc/desc, `None`
  consistente), respetando `dirs_first`.
- **`fs_model`**: `Entry` con `created`; el round-trip donde aplique.
- **Función pura de clic-de-columna** (en ui): dado un SortSpec y una columna
  clicada, el nuevo SortSpec es correcto (misma columna→invierte ascending; otra→
  activa con ascending=true). Testeable sin egui.
- **`listing`**: la captura de `created` no se testea fácil (depende del FS); el
  resto del motor sigue con sus tests.
- UI (encabezados clicables, menú contextual, fila `..` normal) → validación manual;
  la lógica con estado extraída a funciones puras.

Meta de siempre: build limpio + tests + clippy antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── fs_model.rs            # + Entry.created; + SortKey::{Extension, Created}
├── sort.rs                # + brazos Extension/Created en sort_entries
├── listing.rs             # + captura metadata.created() en entry_from_dirent
└── ...

crates/ui/src/
├── panes/file_panel.rs    # fila ".." normal; encabezados clicables; menú de orden
├── sort_ui.rs (nuevo, opcional)  # función pura: clic de columna → nuevo SortSpec
│                          # (o vivir dentro de file_panel si es chico)
├── docking.rs             # pasa lo necesario al file_panel (ya pasa i18n, workspace)
└── ...

crates/core/src/i18n/{es,en}.json  # + claves del menú de orden
```

---

## 7. Dependencias

Sin dependencias nuevas. `std::fs::Metadata::created()` ya está en std (devuelve
`io::Result<SystemTime>`; puede no estar soportado en algún FS → `.ok()` → `None`).
egui ya provee menús contextuales (`response.context_menu(...)`) y los widgets
clicables.

> A confirmar en el plan: si la función de clic-de-columna vive en un archivo nuevo
> (`sort_ui.rs`) o dentro de `file_panel.rs`; y la API exacta de `context_menu` en
> egui 0.34.3 (verificar contra la fuente antes de implementar el menú).

---

## Fuera de alcance (recordatorio — NO en 2D)

Árbol real, watcher / detección de archivos nuevos, temas/color sets/packs (2C-ii),
columnas visibles para extensión/creación, multi-ventana, deuda del dock (2A).
Nunca: reproducción de media, edición de archivos.
