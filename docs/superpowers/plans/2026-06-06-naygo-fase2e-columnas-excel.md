# Fase 2E — Columnas estilo Excel + panel activo — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convertir el file panel de Naygo en una tabla rica estilo Excel: cada columna con un desplegable que combina ordenar, filtrar (en vivo) y mostrar/ocultar/reordenar columnas; multi-filtro con AND; indicadores de columna activa; y marcar visualmente el panel activo.

**Architecture:** El modelo de columnas (`ColumnKind`/`ColumnSpec`/`TableState`) y los filtros (`ColumnFilter` + `matches`) viven en `naygo-core` (puros, testeados). `FilePaneState` gana un `table: TableState` (reemplaza el `text_filter` reservado, con migración). La UI pinta las columnas visibles en orden, aplica filtrar→ordenar en memoria, y muestra un desplegable (`column_menu`) cuya lógica con estado se extrae a funciones puras; las interacciones se difieren como `TableAction` y `NaygoApp` las aplica tras pintar. El panel activo se marca pintando su barra de título en color de acento.

**Tech Stack:** Rust, `naygo-core` / `naygo-ui`, `eframe`/`egui` 0.34.3, `serde`. Sin dependencias nuevas. Reutiliza `SortKey`/`SortSpec`/`sort_entries` (fase 2D).

**Estado de partida (en `main`, rama base `feat/fase2e-columnas-excel`):**
- `naygo_core::fs_model`: `Entry { name: String, path: PathBuf, kind: EntryKind, size: Option<u64>, modified: Option<SystemTime>, created: Option<SystemTime>, hidden: bool }`. `EntryKind { Directory, File, Other }`. `Entry::is_dir()`. `SortKey { Name, Extension, Size, Modified, Created, Kind }`. `SortSpec { key, ascending, dirs_first }` (Copy, serde). `ViewMode { Details, List, Icons }`.
- `naygo_core::sort::sort_entries(&mut [Entry], &SortSpec)`.
- `naygo_core::workspace::file_pane`: `FilePaneState { current_dir, entries: Vec<Entry>, sort: SortSpec, view: ViewMode, focused: Option<usize>, selected: Vec<usize>, history: NavHistory, show_dirs: bool, text_filter: Option<String> }`. `FilePanePersist { current_dir, sort, view, show_dirs, text_filter: Option<String> }`. Methods `new`, `to_persist`, `from_persist`, `navigate_to`, `focused_entry`. Tests reference `text_filter`.
- `naygo_core::lib`: re-exports incl. `pub use fs_model::{Entry, EntryKind, PaneState, SortKey, SortSpec, ViewMode};`.
- `naygo-ui::panes::file_panel::show(ui, workspace, id, pending, icons, show_parent_entry, i18n)`: pinta una `Grid` de 3 columnas FIJAS (Nombre/Tamaño/Modificado) con encabezados clicables vía `header_label(ui, title, sort, key) -> Response` (▲/▼), fila ".." con `IconKey::Folder`, filas vía `icon_row(...)`, `format_size`/`format_modified`. Acumula `clicked`/`activated`/`header_clicked`, los procesa tras la Grid (re-sort con `sort_entries`).
- `naygo-ui::docking`: `NaygoTabViewer { workspace, status, pending, icons, show_parent_entry, i18n, trees, tree_actions, tree_revealed }`; `fn title()` da el título del tab; `fn ui()` despacha por `PanePurpose`. `PaneRequest { NavigateTo, Activate }`.
- `naygo-ui::app`: `NaygoApp` con `workspace`, `dock_state`, etc. `ui()` construye el viewer, corre `DockArea::...show_inside`, procesa `pending` y `tree_actions`. `workspace.active_id() -> Option<PaneId>`.
- i18n: `crates/core/src/i18n/{es,en}.json` (objeto plano clave→texto). `I18n::t(&self, key) -> &str`. Claves existentes: `col.name`, `col.size`, `col.modified`, etc.

**Prerequisito:** toolchain Rust en PATH. PowerShell: anteponer `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo en PowerShell. Binario `--bin naygo`. Verificar `$LASTEXITCODE`.

**Convenciones (CLAUDE.md):** código en inglés; comentarios/commits en español OK. Header de 2 líneas en archivos NUEVOS:
```
// Naygo — <descripción breve>
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```
`core` NUNCA importa egui/windows/image. Build limpio + tests + clippy antes de cada commit. Footer de commit:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/fase2e-columnas-excel` (desde main). NO cambiar de rama.

**Alcance:** ENTRA: `core::columns`, `core::filter`, `table: TableState` en file_pane (+migración), `ui::column_menu` (modo B, filtro en vivo, controles por tipo), `ui::table_actions`, file_panel pinta columnas dinámicas + indicadores + pipeline filtrar→ordenar, realce del panel activo (barra de título de acento), i18n, "sin coincidencias". NO ENTRA: filtro global de barra, agrupar/pivotar, nuevas ColumnKind más allá de las 5, arrastre pixel-perfect de anchos.

---

## Estructura de archivos

```
crates/core/src/
├── filter.rs              # NUEVO: ColumnFilter, matches(), extension_counts()
├── columns.rs             # NUEVO: ColumnKind, ColumnSpec, TableState (+ops puras), sort_key_of
├── workspace/file_pane.rs # text_filter → table: TableState (+migración en from_persist)
├── lib.rs                 # + pub mod filter; pub mod columns; re-exports
└── i18n/{es,en}.json      # + claves del menú/filtros/sin-coincidencias

crates/ui/src/
├── table_actions.rs       # NUEVO: TableAction (acciones diferidas del menú)
├── column_menu.rs         # NUEVO: desplegable modo B (ordenar/filtrar/columnas), funciones puras
├── panes/file_panel.rs    # columnas visibles dinámicas; indicadores; pipeline filtrar→ordenar
├── docking.rs             # realce barra de título del panel activo (en title()/chrome)
├── app.rs                 # procesar TableAction; pasar active_id para el realce
└── main.rs                # + mod table_actions; mod column_menu;
```

---

## Task 1: `core::filter` — ColumnFilter + matches (AND)

**Files:**
- Create: `crates/core/src/filter.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear `filter.rs` con los tipos, `matches`, y tests**

Create `crates/core/src/filter.rs`:

```rust
// Naygo — filtros de columna del file panel (puros, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Filtros por columna y su combinación (AND). `matches` decide si un `Entry`
//! pasa TODOS los filtros activos. Puro y testeable; el recorrido lo hace la UI
//! sobre las entries en memoria (lineal, barato).

use crate::columns::ColumnKind;
use crate::fs_model::Entry;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

/// Filtro de una columna, según su tipo de dato.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnFilter {
    /// Nombre contiene una subcadena.
    Text { contains: String, case_sensitive: bool },
    /// Extensión dentro del conjunto marcado (minúsculas; "" = sin extensión).
    Extensions(BTreeSet<String>),
    /// Tamaño en bytes dentro de [min, max] (None = sin límite ese lado).
    SizeRange { min: Option<u64>, max: Option<u64> },
    /// Fecha (modificación o creación, según la columna) dentro de [from, to].
    DateRange { from: Option<SystemTime>, to: Option<SystemTime> },
}

/// Extensión de un `Entry` en minúsculas; "" si no tiene.
pub fn entry_extension(entry: &Entry) -> String {
    entry
        .path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// `true` si `entry` pasa TODOS los filtros activos (AND). Sin filtros = pasa.
pub fn matches(entry: &Entry, filters: &BTreeMap<ColumnKind, ColumnFilter>) -> bool {
    filters.iter().all(|(kind, f)| match_one(entry, *kind, f))
}

/// Evalúa un único filtro contra un entry.
fn match_one(entry: &Entry, kind: ColumnKind, f: &ColumnFilter) -> bool {
    match f {
        ColumnFilter::Text { contains, case_sensitive } => {
            if contains.is_empty() {
                return true;
            }
            if *case_sensitive {
                entry.name.contains(contains.as_str())
            } else {
                entry.name.to_lowercase().contains(&contains.to_lowercase())
            }
        }
        ColumnFilter::Extensions(set) => {
            // Conjunto vacío = no filtra (muestra todo).
            if set.is_empty() {
                return true;
            }
            set.contains(&entry_extension(entry))
        }
        ColumnFilter::SizeRange { min, max } => {
            // Sin tamaño (carpetas) → fuera si hay filtro de tamaño activo.
            let Some(size) = entry.size else {
                return false;
            };
            min.map(|m| size >= m).unwrap_or(true) && max.map(|m| size <= m).unwrap_or(true)
        }
        ColumnFilter::DateRange { from, to } => {
            let value = match kind {
                ColumnKind::Created => entry.created,
                _ => entry.modified, // Modified (y cualquier otra columna de fecha)
            };
            let Some(t) = value else {
                return false;
            };
            from.map(|f| t >= f).unwrap_or(true) && to.map(|f| t <= f).unwrap_or(true)
        }
    }
}

/// Cuenta cuántos entries hay por extensión (para el filtro de tipos). "" = sin
/// extensión.
pub fn extension_counts(entries: &[Entry]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for e in entries {
        *counts.entry(entry_extension(e)).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_model::EntryKind;
    use std::path::PathBuf;
    use std::time::Duration;

    fn entry(name: &str, size: Option<u64>) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind: if size.is_some() { EntryKind::File } else { EntryKind::Directory },
            size,
            modified: None,
            created: None,
            hidden: false,
        }
    }

    fn no_filters() -> BTreeMap<ColumnKind, ColumnFilter> {
        BTreeMap::new()
    }

    #[test]
    fn sin_filtros_pasa_todo() {
        assert!(matches(&entry("a.txt", Some(1)), &no_filters()));
    }

    #[test]
    fn text_contains_case_insensitive() {
        let mut f = no_filters();
        f.insert(
            ColumnKind::Name,
            ColumnFilter::Text { contains: "INFORME".into(), case_sensitive: false },
        );
        assert!(matches(&entry("informe_final.pdf", Some(1)), &f));
        assert!(!matches(&entry("notas.txt", Some(1)), &f));
    }

    #[test]
    fn text_contains_case_sensitive() {
        let mut f = no_filters();
        f.insert(
            ColumnKind::Name,
            ColumnFilter::Text { contains: "Informe".into(), case_sensitive: true },
        );
        assert!(matches(&entry("Informe.pdf", Some(1)), &f));
        assert!(!matches(&entry("informe.pdf", Some(1)), &f));
    }

    #[test]
    fn extensions_set_vacio_pasa_todo() {
        let mut f = no_filters();
        f.insert(ColumnKind::Extension, ColumnFilter::Extensions(BTreeSet::new()));
        assert!(matches(&entry("a.txt", Some(1)), &f));
    }

    #[test]
    fn extensions_marca_tipos() {
        let mut set = BTreeSet::new();
        set.insert("pdf".to_string());
        set.insert("".to_string()); // sin extensión
        let mut f = no_filters();
        f.insert(ColumnKind::Extension, ColumnFilter::Extensions(set));
        assert!(matches(&entry("doc.pdf", Some(1)), &f));
        assert!(matches(&entry("LEEME", Some(1)), &f)); // sin extensión
        assert!(!matches(&entry("img.jpg", Some(1)), &f));
    }

    #[test]
    fn size_range_bordes_y_carpetas_fuera() {
        let mut f = no_filters();
        f.insert(ColumnKind::Size, ColumnFilter::SizeRange { min: Some(10), max: Some(100) });
        assert!(matches(&entry("a", Some(10)), &f)); // borde inferior
        assert!(matches(&entry("b", Some(100)), &f)); // borde superior
        assert!(!matches(&entry("c", Some(9)), &f));
        assert!(!matches(&entry("d", Some(101)), &f));
        assert!(!matches(&entry("dir", None), &f)); // carpeta fuera
    }

    #[test]
    fn date_range_modified() {
        let base = SystemTime::UNIX_EPOCH;
        let mut e = entry("a", Some(1));
        e.modified = Some(base + Duration::from_secs(50));
        let mut f = no_filters();
        f.insert(
            ColumnKind::Modified,
            ColumnFilter::DateRange {
                from: Some(base + Duration::from_secs(10)),
                to: Some(base + Duration::from_secs(100)),
            },
        );
        assert!(matches(&e, &f));
        let mut e2 = entry("b", Some(1));
        e2.modified = Some(base + Duration::from_secs(5));
        assert!(!matches(&e2, &f));
    }

    #[test]
    fn multi_filtro_es_interseccion_and() {
        let mut set = BTreeSet::new();
        set.insert("pdf".to_string());
        let mut f = no_filters();
        f.insert(ColumnKind::Extension, ColumnFilter::Extensions(set));
        f.insert(
            ColumnKind::Name,
            ColumnFilter::Text { contains: "informe".into(), case_sensitive: false },
        );
        assert!(matches(&entry("informe.pdf", Some(1)), &f)); // cumple ambos
        assert!(!matches(&entry("informe.txt", Some(1)), &f)); // falla extensión
        assert!(!matches(&entry("otro.pdf", Some(1)), &f)); // falla nombre
    }

    #[test]
    fn extension_counts_cuenta_por_tipo() {
        let entries = vec![
            entry("a.txt", Some(1)),
            entry("b.txt", Some(1)),
            entry("c.pdf", Some(1)),
            entry("LEEME", Some(1)),
        ];
        let counts = extension_counts(&entries);
        assert_eq!(counts.get("txt"), Some(&2));
        assert_eq!(counts.get("pdf"), Some(&1));
        assert_eq!(counts.get(""), Some(&1));
    }
}
```

- [ ] **Step 2: Declarar el módulo + re-export**

Modify `crates/core/src/lib.rs`:
- Tras `pub mod fs_model;` añadir `pub mod filter;` (alfabético: filter va antes de fs_model; colocar `pub mod filter;` antes de `pub mod fs_model;`). NOTA: `filter.rs` usa `crate::columns::ColumnKind`, que se crea en la Tarea 2. Para que la Tarea 1 compile SOLA, define `ColumnKind` en la Tarea 2 ANTES de que filter lo use… pero filter ya lo importa. SOLUCIÓN: en esta tarea, declarar TAMBIÉN `pub mod columns;` y crear un `columns.rs` MÍNIMO con solo el enum `ColumnKind` (el resto de columns se completa en la Tarea 2). Ver Step 3.
- Re-export tras los existentes: `pub use filter::{matches, ColumnFilter};`

- [ ] **Step 3: Crear `columns.rs` mínimo (solo `ColumnKind`) para que filter compile**

Create `crates/core/src/columns.rs` (mínimo; se amplía en Tarea 2):

```rust
// Naygo — modelo de columnas del file panel (puro, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Define qué columnas existen y el estado de tabla de un panel (qué columnas se
//! ven, en qué orden, su ancho) más los filtros activos. Puro y testeable.

use serde::{Deserialize, Serialize};

/// Qué columna. Extensible: agregar variante + su extractor a futuro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ColumnKind {
    Name,
    Extension,
    Size,
    Modified,
    Created,
}
```

Modify `crates/core/src/lib.rs`: añadir `pub mod columns;` (antes de `pub mod filter;`) y `pub use columns::ColumnKind;`.

NOTA: `ColumnKind` deriva `Ord`/`PartialOrd` porque es clave de un `BTreeMap` (`TableState.filters`).

- [ ] **Step 4: Verificar**

Run: `cargo test -p naygo-core filter` → 9 tests PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/filter.rs crates/core/src/columns.rs crates/core/src/lib.rs
git commit -m "feat(core): filtros de columna (ColumnFilter, matches AND, extension_counts)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `core::columns` — ColumnSpec + TableState + ops puras

**Files:**
- Modify: `crates/core/src/columns.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Escribir los tests (primero, TDD)**

Append a `crates/core/src/columns.rs` un `#[cfg(test)] mod tests` con:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tiene_las_cinco_columnas_creacion_oculta() {
        let t = TableState::default();
        assert_eq!(t.columns.len(), 5);
        // Nombre, Extensión, Tamaño, Modificado visibles; Creación oculta.
        let visible: Vec<ColumnKind> = t.visible_columns().map(|c| c.kind).collect();
        assert_eq!(
            visible,
            vec![ColumnKind::Name, ColumnKind::Extension, ColumnKind::Size, ColumnKind::Modified]
        );
        assert!(t.filters.is_empty());
    }

    #[test]
    fn toggle_visible_oculta_y_muestra() {
        let mut t = TableState::default();
        t.toggle_visible(ColumnKind::Size);
        assert!(!t.columns.iter().find(|c| c.kind == ColumnKind::Size).unwrap().visible);
        t.toggle_visible(ColumnKind::Size);
        assert!(t.columns.iter().find(|c| c.kind == ColumnKind::Size).unwrap().visible);
    }

    #[test]
    fn nombre_no_se_puede_ocultar() {
        let mut t = TableState::default();
        t.toggle_visible(ColumnKind::Name);
        assert!(
            t.columns.iter().find(|c| c.kind == ColumnKind::Name).unwrap().visible,
            "Nombre siempre visible"
        );
    }

    #[test]
    fn move_column_reordena() {
        let mut t = TableState::default();
        // Mover Size (idx 2) al frente (idx 0).
        t.move_column(2, 0);
        assert_eq!(t.columns[0].kind, ColumnKind::Size);
    }

    #[test]
    fn set_width_clampa() {
        let mut t = TableState::default();
        t.set_width(ColumnKind::Name, 5.0); // bajo el mínimo
        let w = t.columns.iter().find(|c| c.kind == ColumnKind::Name).unwrap().width;
        assert!(w >= MIN_COLUMN_WIDTH, "se respeta el ancho mínimo");
        t.set_width(ColumnKind::Name, 5000.0); // sobre el máximo
        let w = t.columns.iter().find(|c| c.kind == ColumnKind::Name).unwrap().width;
        assert!(w <= MAX_COLUMN_WIDTH, "se respeta el ancho máximo");
    }

    #[test]
    fn set_y_clear_filter() {
        use crate::filter::ColumnFilter;
        let mut t = TableState::default();
        t.set_filter(
            ColumnKind::Name,
            ColumnFilter::Text { contains: "x".into(), case_sensitive: false },
        );
        assert!(t.filters.contains_key(&ColumnKind::Name));
        t.clear_filter(ColumnKind::Name);
        assert!(!t.filters.contains_key(&ColumnKind::Name));
    }

    #[test]
    fn sort_key_of_mapea_columna_a_sortkey() {
        use crate::fs_model::SortKey;
        assert_eq!(sort_key_of(ColumnKind::Name), SortKey::Name);
        assert_eq!(sort_key_of(ColumnKind::Extension), SortKey::Extension);
        assert_eq!(sort_key_of(ColumnKind::Size), SortKey::Size);
        assert_eq!(sort_key_of(ColumnKind::Modified), SortKey::Modified);
        assert_eq!(sort_key_of(ColumnKind::Created), SortKey::Created);
    }

    #[test]
    fn round_trip_serde() {
        use crate::filter::ColumnFilter;
        let mut t = TableState::default();
        t.toggle_visible(ColumnKind::Created); // mostrar creación
        t.set_filter(
            ColumnKind::Name,
            ColumnFilter::Text { contains: "doc".into(), case_sensitive: false },
        );
        let json = serde_json::to_string(&t).unwrap();
        let back: TableState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }
}
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core columns`
Expected: ERROR de compilación (faltan `ColumnSpec`, `TableState`, `MIN/MAX_COLUMN_WIDTH`, `sort_key_of`, métodos).

- [ ] **Step 3: Implementar en `columns.rs`**

Añadir a `crates/core/src/columns.rs` (tras el enum `ColumnKind`, antes de los tests):

```rust
use crate::filter::ColumnFilter;
use crate::fs_model::SortKey;
use std::collections::BTreeMap;

/// Ancho mínimo/máximo de una columna (px lógicos).
pub const MIN_COLUMN_WIDTH: f32 = 40.0;
pub const MAX_COLUMN_WIDTH: f32 = 1200.0;

/// Una columna de la tabla: qué es, si se ve, su ancho. El ORDEN del Vec en
/// `TableState.columns` es el orden visual.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub kind: ColumnKind,
    pub visible: bool,
    pub width: f32,
}

/// Estado de tabla de un panel: columnas (orden/visibilidad/ancho) + filtros AND.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableState {
    pub columns: Vec<ColumnSpec>,
    pub filters: BTreeMap<ColumnKind, ColumnFilter>,
}

impl Default for TableState {
    fn default() -> Self {
        let col = |kind, visible, width| ColumnSpec { kind, visible, width };
        TableState {
            columns: vec![
                col(ColumnKind::Name, true, 240.0),
                col(ColumnKind::Extension, true, 90.0),
                col(ColumnKind::Size, true, 90.0),
                col(ColumnKind::Modified, true, 140.0),
                col(ColumnKind::Created, false, 140.0),
            ],
            filters: BTreeMap::new(),
        }
    }
}

impl TableState {
    /// Itera las columnas visibles en orden visual.
    pub fn visible_columns(&self) -> impl Iterator<Item = &ColumnSpec> {
        self.columns.iter().filter(|c| c.visible)
    }

    /// Alterna la visibilidad de una columna. Nombre nunca se oculta.
    pub fn toggle_visible(&mut self, kind: ColumnKind) {
        if kind == ColumnKind::Name {
            return; // Nombre siempre visible.
        }
        if let Some(c) = self.columns.iter_mut().find(|c| c.kind == kind) {
            c.visible = !c.visible;
        }
    }

    /// Mueve la columna del índice `from` al índice `to` (reordena el Vec).
    pub fn move_column(&mut self, from: usize, to: usize) {
        if from >= self.columns.len() || to >= self.columns.len() || from == to {
            return;
        }
        let c = self.columns.remove(from);
        self.columns.insert(to, c);
    }

    /// Fija el ancho de una columna, con clamp a [MIN, MAX].
    pub fn set_width(&mut self, kind: ColumnKind, width: f32) {
        if let Some(c) = self.columns.iter_mut().find(|c| c.kind == kind) {
            c.width = width.clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
        }
    }

    /// Establece (o reemplaza) el filtro de una columna.
    pub fn set_filter(&mut self, kind: ColumnKind, filter: ColumnFilter) {
        self.filters.insert(kind, filter);
    }

    /// Quita el filtro de una columna.
    pub fn clear_filter(&mut self, kind: ColumnKind) {
        self.filters.remove(&kind);
    }
}

/// Mapea una columna a su `SortKey` (1:1).
pub fn sort_key_of(kind: ColumnKind) -> SortKey {
    match kind {
        ColumnKind::Name => SortKey::Name,
        ColumnKind::Extension => SortKey::Extension,
        ColumnKind::Size => SortKey::Size,
        ColumnKind::Modified => SortKey::Modified,
        ColumnKind::Created => SortKey::Created,
    }
}
```

- [ ] **Step 4: Re-exports**

Modify `crates/core/src/lib.rs`: cambiar `pub use columns::ColumnKind;` por:
```rust
pub use columns::{sort_key_of, ColumnKind, ColumnSpec, TableState};
```

- [ ] **Step 5: Correr los tests — pasan**

Run: `cargo test -p naygo-core columns` → todos PASS.
Run: `cargo test -p naygo-core` → todo verde.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/columns.rs crates/core/src/lib.rs
git commit -m "feat(core): modelo de columnas (ColumnSpec, TableState, ops puras, sort_key_of)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `file_pane` — `table: TableState` con migración de `text_filter`

**Files:**
- Modify: `crates/core/src/workspace/file_pane.rs`

- [ ] **Step 1: Escribir el test de migración (primero)**

En el `#[cfg(test)] mod tests` de `file_pane.rs`, añadir:

```rust
    #[test]
    fn migracion_text_filter_a_table_name_filter() {
        use crate::columns::ColumnKind;
        use crate::filter::ColumnFilter;
        // Un persist "viejo" con text_filter = Some(..) debe migrar a un filtro de
        // Nombre en la tabla.
        let persist = FilePanePersist {
            current_dir: p("C:/a"),
            sort: SortSpec::default(),
            view: ViewMode::default(),
            show_dirs: true,
            text_filter: Some("informe".into()),
            table: None,
        };
        let s = FilePaneState::from_persist(persist);
        let f = s.table.filters.get(&ColumnKind::Name).expect("filtro de nombre migrado");
        assert_eq!(
            *f,
            ColumnFilter::Text { contains: "informe".into(), case_sensitive: false }
        );
    }

    #[test]
    fn persist_nuevo_usa_table_directamente() {
        use crate::columns::TableState;
        let mut table = TableState::default();
        table.toggle_visible(ColumnKind_for_test());
        let persist = FilePanePersist {
            current_dir: p("C:/a"),
            sort: SortSpec::default(),
            view: ViewMode::default(),
            show_dirs: true,
            text_filter: None,
            table: Some(table.clone()),
        };
        let s = FilePaneState::from_persist(persist);
        assert_eq!(s.table, table);
    }

    // helper para el test de arriba (evita import largo inline)
    #[allow(non_snake_case)]
    fn ColumnKind_for_test() -> crate::columns::ColumnKind {
        crate::columns::ColumnKind::Created
    }
```

NOTA: el `FilePanePersist` gana un campo `table: Option<TableState>` (Option para retro-compat: persists viejos no lo traen → `None` → se migra desde `text_filter`). `text_filter` SE MANTIENE en el persist (deprecado, solo lectura para migrar); se deja de escribir.

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core file_pane`
Expected: ERROR de compilación — `FilePaneState` no tiene `table`; `FilePanePersist` no tiene `table`.

- [ ] **Step 3: Implementar**

Modify `crates/core/src/workspace/file_pane.rs`:

a) En los `use`, asegurar: `use crate::columns::TableState;` y `use crate::columns::ColumnKind;` y `use crate::filter::ColumnFilter;`.

b) En `struct FilePaneState`, reemplazar `pub text_filter: Option<String>,` por:
```rust
    /// Estado de tabla: columnas (orden/visibilidad/ancho) + filtros por columna.
    pub table: TableState,
```

c) En `struct FilePanePersist`, MANTENER `text_filter` (deprecado) y AÑADIR `table`:
```rust
    /// DEPRECADO: filtro de texto plano (fases previas). Solo se LEE para migrar a
    /// `table`. Ya no se escribe.
    #[serde(default)]
    pub text_filter: Option<String>,
    /// Estado de tabla. `None` en persists viejos → se migra desde `text_filter`.
    #[serde(default)]
    pub table: Option<TableState>,
```

d) En `FilePaneState::new`, reemplazar `text_filter: None,` por `table: TableState::default(),`.

e) En `to_persist`, reemplazar `text_filter: self.text_filter.clone(),` por:
```rust
            text_filter: None, // ya no se escribe; se migró a table
            table: Some(self.table.clone()),
```

f) En `from_persist`, reemplazar `s.text_filter = p.text_filter;` por la lógica de migración:
```rust
        s.table = match p.table {
            Some(t) => t,
            None => {
                // Migración: persist viejo con text_filter → filtro de Nombre.
                let mut t = TableState::default();
                if let Some(text) = p.text_filter {
                    if !text.is_empty() {
                        t.set_filter(
                            ColumnKind::Name,
                            ColumnFilter::Text { contains: text, case_sensitive: false },
                        );
                    }
                }
                t
            }
        };
```

g) El test existente `nuevo_apunta_su_historial_a_la_carpeta` referencia `s.text_filter.is_none()`. Cambiar esa aserción por `assert!(s.table.filters.is_empty());`.

- [ ] **Step 4: Correr los tests — pasan**

Run: `cargo test -p naygo-core file_pane` → PASS (incl. los 2 nuevos de migración).
Run: `cargo test -p naygo-core` → todo verde.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/workspace/file_pane.rs
git commit -m "feat(core): file_pane usa TableState (migra text_filter → filtro de Nombre)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: i18n — claves de 2E (ES + EN)

**Files:**
- Modify: `crates/core/src/i18n/es.json`
- Modify: `crates/core/src/i18n/en.json`

- [ ] **Step 1: Añadir claves en `es.json`**

Modify `crates/core/src/i18n/es.json` — insertar tras `"col.modified": "...",` (que tiene coma y hay claves después):
```json
  "col.extension": "Extensión",
  "col.created": "Creación",
  "menu.sort_asc": "Ordenar ascendente",
  "menu.sort_desc": "Ordenar descendente",
  "menu.filter": "Filtrar…",
  "menu.columns": "Columnas…",
  "menu.clear_filter": "Quitar filtro de esta columna",
  "filter.name_contains": "El nombre contiene:",
  "filter.case_sensitive": "Distinguir mayúsculas",
  "filter.search_type": "buscar tipo…",
  "filter.no_extension": "(sin extensión)",
  "filter.size_from": "Desde:",
  "filter.size_to": "Hasta:",
  "filter.date_from": "Desde:",
  "filter.date_to": "Hasta:",
  "filter.clear": "Limpiar",
  "filter.none": "Ninguno",
  "table.no_matches": "Sin coincidencias",
```

- [ ] **Step 2: Añadir las mismas claves en `en.json`**

Modify `crates/core/src/i18n/en.json` — insertar tras `"col.modified": "...",`:
```json
  "col.extension": "Extension",
  "col.created": "Created",
  "menu.sort_asc": "Sort ascending",
  "menu.sort_desc": "Sort descending",
  "menu.filter": "Filter…",
  "menu.columns": "Columns…",
  "menu.clear_filter": "Clear filter on this column",
  "filter.name_contains": "Name contains:",
  "filter.case_sensitive": "Case sensitive",
  "filter.search_type": "search type…",
  "filter.no_extension": "(no extension)",
  "filter.size_from": "From:",
  "filter.size_to": "To:",
  "filter.date_from": "From:",
  "filter.date_to": "To:",
  "filter.clear": "Clear",
  "filter.none": "None",
  "table.no_matches": "No matches",
```

NOTA: ambos catálogos deben tener EXACTAMENTE el mismo set de claves. READ ambos archivos primero para confirmar que `col.modified` tiene coma y hay claves después (debería; `col.size`/`col.modified` ya existen).

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core i18n` → PASS (parity ES/EN).
Run: `cargo test -p naygo-core` → verde.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "i18n: claves de columnas/filtros estilo Excel (ES/EN)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: `ui::table_actions` — acciones diferidas del menú

**Files:**
- Create: `crates/ui/src/table_actions.rs`
- Modify: `crates/ui/src/main.rs`

- [ ] **Step 1: Crear el enum con un test**

Create `crates/ui/src/table_actions.rs`:

```rust
// Naygo — acciones del menú de columna, acumuladas al pintar y aplicadas después.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Igual que `PaneRequest`/`TreeAction`: el render del menú de columna no muta el
//! estado; acumula `TableAction`s que `NaygoApp` procesa tras pintar.

use naygo_core::columns::ColumnKind;
use naygo_core::filter::ColumnFilter;
use naygo_core::fs_model::SortSpec;

/// Una acción pedida desde el menú/encabezado de columna.
#[derive(Clone, Debug, PartialEq)]
pub enum TableAction {
    /// Cambiar el orden del panel.
    SetSort(SortSpec),
    /// Establecer/reemplazar el filtro de una columna.
    SetFilter(ColumnKind, ColumnFilter),
    /// Quitar el filtro de una columna.
    ClearFilter(ColumnKind),
    /// Alternar visibilidad de una columna.
    ToggleColumn(ColumnKind),
    /// Mover una columna del índice `from` al `to`.
    MoveColumn(usize, usize),
    /// Fijar el ancho de una columna.
    SetColumnWidth(ColumnKind, f32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_action_es_comparable() {
        let a = TableAction::ClearFilter(ColumnKind::Name);
        assert_eq!(a, TableAction::ClearFilter(ColumnKind::Name));
        assert_ne!(a, TableAction::ToggleColumn(ColumnKind::Name));
    }
}
```

- [ ] **Step 2: Declarar el módulo**

Modify `crates/ui/src/main.rs` — añadir `mod table_actions;` (alfabético: tras `mod sort_ui;`, antes de `mod templates_menu;` — READ el archivo para ubicar).

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-ui table_action` → 1 test PASS.
Run: `cargo clippy -p naygo-ui --all-targets -- -D warnings`.
NOTA: `TableAction` no se consume hasta la Tarea 6/7 → `dead_code`. Si clippy bloquea, añadir `#[allow(dead_code)]` sobre el enum con comentario `// consumido en Tareas 6-7`, a quitar en la Tarea 7.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/table_actions.rs crates/ui/src/main.rs
git commit -m "feat(ui): enum TableAction (acciones diferidas del menú de columna)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: `ui::column_menu` — funciones puras de interacción

**Files:**
- Create: `crates/ui/src/column_menu.rs`
- Modify: `crates/ui/src/main.rs`

Esta tarea crea la LÓGICA PURA del menú (qué acción resulta de cada interacción), testeable sin egui. El render egui viene en la Tarea 7.

- [ ] **Step 1: Crear el archivo con funciones puras + tests**

Create `crates/ui/src/column_menu.rs`:

```rust
// Naygo — lógica pura del menú de columna (qué acción produce cada interacción).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! El render del desplegable (Tarea siguiente) llama a estas funciones puras para
//! decidir qué `TableAction` emitir. Separar la decisión del dibujo las hace
//! testeables sin egui.

use crate::table_actions::TableAction;
use naygo_core::columns::{sort_key_of, ColumnKind};
use naygo_core::filter::ColumnFilter;
use naygo_core::fs_model::SortSpec;

/// Acción al pedir "ordenar" por una columna en una dirección.
pub fn sort_action(kind: ColumnKind, ascending: bool, dirs_first: bool) -> TableAction {
    TableAction::SetSort(SortSpec {
        key: sort_key_of(kind),
        ascending,
        dirs_first,
    })
}

/// Acción al cambiar el texto del filtro de Nombre. Texto vacío → quitar filtro.
pub fn name_filter_action(contains: &str, case_sensitive: bool) -> TableAction {
    if contains.is_empty() {
        TableAction::ClearFilter(ColumnKind::Name)
    } else {
        TableAction::SetFilter(
            ColumnKind::Name,
            ColumnFilter::Text {
                contains: contains.to_string(),
                case_sensitive,
            },
        )
    }
}

/// Acción al cambiar el conjunto de extensiones marcadas. Vacío → quitar filtro.
pub fn extensions_filter_action(selected: std::collections::BTreeSet<String>) -> TableAction {
    if selected.is_empty() {
        TableAction::ClearFilter(ColumnKind::Extension)
    } else {
        TableAction::SetFilter(ColumnKind::Extension, ColumnFilter::Extensions(selected))
    }
}

/// Acción al fijar un rango de tamaño. Ambos None → quitar filtro.
pub fn size_filter_action(min: Option<u64>, max: Option<u64>) -> TableAction {
    if min.is_none() && max.is_none() {
        TableAction::ClearFilter(ColumnKind::Size)
    } else {
        TableAction::SetFilter(ColumnKind::Size, ColumnFilter::SizeRange { min, max })
    }
}

/// Convierte un valor + unidad (KB/MB/GB) a bytes. Para los controles de tamaño.
pub fn to_bytes(value: f64, unit: SizeUnit) -> u64 {
    let mult = match unit {
        SizeUnit::Kb => 1024.0,
        SizeUnit::Mb => 1024.0 * 1024.0,
        SizeUnit::Gb => 1024.0 * 1024.0 * 1024.0,
    };
    (value * mult).max(0.0) as u64
}

/// Unidad de tamaño para los controles de filtro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeUnit {
    Kb,
    Mb,
    Gb,
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::fs_model::SortKey;
    use std::collections::BTreeSet;

    #[test]
    fn sort_action_usa_sortkey_de_la_columna() {
        let a = sort_action(ColumnKind::Extension, true, true);
        match a {
            TableAction::SetSort(spec) => {
                assert_eq!(spec.key, SortKey::Extension);
                assert!(spec.ascending);
                assert!(spec.dirs_first);
            }
            _ => panic!("esperaba SetSort"),
        }
    }

    #[test]
    fn name_filter_vacio_quita_filtro() {
        assert_eq!(name_filter_action("", false), TableAction::ClearFilter(ColumnKind::Name));
    }

    #[test]
    fn name_filter_con_texto_setea() {
        let a = name_filter_action("doc", true);
        assert_eq!(
            a,
            TableAction::SetFilter(
                ColumnKind::Name,
                ColumnFilter::Text { contains: "doc".into(), case_sensitive: true }
            )
        );
    }

    #[test]
    fn extensions_vacio_quita_filtro() {
        assert_eq!(
            extensions_filter_action(BTreeSet::new()),
            TableAction::ClearFilter(ColumnKind::Extension)
        );
    }

    #[test]
    fn size_ambos_none_quita_filtro() {
        assert_eq!(
            size_filter_action(None, None),
            TableAction::ClearFilter(ColumnKind::Size)
        );
    }

    #[test]
    fn to_bytes_convierte_unidades() {
        assert_eq!(to_bytes(1.0, SizeUnit::Kb), 1024);
        assert_eq!(to_bytes(2.0, SizeUnit::Mb), 2 * 1024 * 1024);
        assert_eq!(to_bytes(1.0, SizeUnit::Gb), 1024 * 1024 * 1024);
    }
}
```

- [ ] **Step 2: Declarar el módulo**

Modify `crates/ui/src/main.rs` — añadir `mod column_menu;` (alfabético: tras `mod app;`/los primeros; ubicar entre `mod app;` y `mod dock_translate;` → en realidad alfabéticamente `column_menu` va tras `app` y antes de `dock_translate`. READ y ubicar correctamente).

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-ui column_menu` → 6 tests PASS.
Run: `cargo clippy -p naygo-ui --all-targets -- -D warnings`.
NOTA: estas fns no se usan hasta la Tarea 7 → `dead_code`. Si clippy bloquea, `#[allow(dead_code)]` por fn (o `#![allow(dead_code)]` al tope del módulo) con comentario `// consumido en la Tarea 7 (render del menú)`, a quitar en la Tarea 7.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/column_menu.rs crates/ui/src/main.rs
git commit -m "feat(ui): lógica pura del menú de columna (acciones de orden/filtro)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: `file_panel` — columnas dinámicas, indicadores, pipeline filtrar→ordenar, desplegable

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/ui/src/column_menu.rs` (añadir el render del desplegable)
- Modify: `crates/ui/src/docking.rs` (pasar tabla + acumular TableAction)
- Modify: `crates/ui/src/app.rs` (procesar TableAction)

Esta es la tarea de integración grande. Produce la tabla rica funcional. VERIFICAR egui 0.34.3 API contra `C:\Users\ngrot\.cargo\registry\src\index.crates.io-*\egui-0.34.3\` antes de usar widgets (patrón de fases previas).

- [ ] **Step 1: `docking.rs` — pasar la tabla del panel y recoger TableActions**

Modify `crates/ui/src/docking.rs`:

a) Añadir al `struct NaygoTabViewer`:
```rust
    pub table_actions: &'a mut Vec<(naygo_core::workspace::PaneId, crate::table_actions::TableAction)>,
```

b) El file_panel necesita el `TableState` y las `entries` del panel — ya los obtiene de `self.workspace` dentro de `file_panel::show`, así que NO hace falta pasar la tabla aparte; basta pasar el acumulador. En el brazo `Some(PanePurpose::Files)`, cambiar la llamada a `file_panel::show` para pasar `self.table_actions` (ver Step 2 para la nueva firma).

- [ ] **Step 2: `file_panel.rs` — nueva firma + columnas dinámicas + pipeline + indicadores**

Modify `crates/ui/src/panes/file_panel.rs`. La función `show` cambia para:
- Leer `table: TableState` del panel (clonar: `let table = f.table.clone();`).
- Construir la vista: `entries` filtradas (`naygo_core::filter::matches`) y ordenadas (`sort_entries`), en memoria.
- Pintar las columnas VISIBLES en orden (`table.visible_columns()`), cada encabezado con indicadores y un ▾ que abre el menú.
- Acumular `TableAction`s y devolverlas (o empujarlas a un `&mut Vec` recibido).

Nueva firma:
```rust
pub fn show(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    id: PaneId,
    pending: &mut Vec<PaneRequest>,
    icons: &IconProvider,
    show_parent_entry: bool,
    i18n: &naygo_core::i18n::I18n,
    table_actions: &mut Vec<crate::table_actions::TableAction>,
)
```

Cuerpo (adaptar a la estructura actual; conceptual):
```rust
    let Some(pane) = workspace.pane(id) else { return; };
    let Some(f) = pane.files.as_ref() else { return; };
    let focused = f.focused;
    let show_dirs = f.show_dirs;
    let current_dir = f.current_dir.clone();
    let sort = f.sort;
    let table = f.table.clone();
    let all_entries: Vec<Entry> = f.entries.clone();

    // Pipeline en memoria: filtrar → ordenar.
    let mut entries: Vec<Entry> = if table.filters.is_empty() {
        all_entries
    } else {
        all_entries
            .into_iter()
            .filter(|e| naygo_core::filter::matches(e, &table.filters))
            .collect()
    };
    naygo_core::sort::sort_entries(&mut entries, &sort);

    // Contadores por extensión (para el menú de filtro) sobre las entries SIN filtrar
    // de esa columna — para simplicidad de 2E, usar todas las entries actuales del panel.
    let ext_counts = naygo_core::filter::extension_counts(&f.entries);

    // ... cabecera de ruta + separador (como hoy) ...

    let parent = /* igual que hoy */;

    let mut clicked: Option<usize> = None;
    let mut activated: Option<usize> = None;
    let mut parent_activated = false;

    let visible_cols: Vec<naygo_core::columns::ColumnSpec> = table.visible_columns().cloned().collect();

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new(("file_grid", id.0))
            .num_columns(visible_cols.len())
            .striped(true)
            .show(ui, |ui| {
                // Encabezados dinámicos.
                for col in &visible_cols {
                    column_header(ui, col, &table, sort, &ext_counts, i18n, table_actions);
                }
                ui.end_row();

                // Fila "..".
                if parent.is_some() {
                    let resp = icon_row(ui, icons, IconKey::Folder, "..", false);
                    if resp.double_clicked() || resp.clicked() { parent_activated = true; }
                    for _ in 1..visible_cols.len() { ui.label(""); }
                    ui.end_row();
                }

                // Filas.
                if entries.is_empty() && !table.filters.is_empty() {
                    ui.weak(i18n.t("table.no_matches"));
                    ui.end_row();
                } else {
                    for (i, entry) in entries.iter().enumerate() {
                        if !show_dirs && entry.is_dir() { continue; }
                        let selected = focused == Some(i);
                        // Primera columna visible lleva el ícono+nombre; las demás, el dato.
                        for (ci, col) in visible_cols.iter().enumerate() {
                            if ci == 0 {
                                let key = icon_key_for(entry);
                                let resp = icon_row(ui, icons, key, &cell_text(entry, col.kind), selected);
                                if resp.clicked() { clicked = Some(i); }
                                if resp.double_clicked() { activated = Some(i); }
                            } else {
                                ui.label(cell_text(entry, col.kind));
                            }
                        }
                        ui.end_row();
                    }
                }
            });
    });

    // ... procesar parent_activated / clicked / activated igual que hoy ...
```

Helpers nuevos en `file_panel.rs`:
```rust
/// Texto de una celda según la columna.
fn cell_text(entry: &Entry, kind: naygo_core::columns::ColumnKind) -> String {
    use naygo_core::columns::ColumnKind;
    match kind {
        ColumnKind::Name => entry.name.clone(),
        ColumnKind::Extension => naygo_core::filter::entry_extension(entry),
        ColumnKind::Size => format_size(entry),
        ColumnKind::Modified => format_time(entry.modified),
        ColumnKind::Created => format_time(entry.created),
    }
}

/// Encabezado de una columna: título + indicadores (▲/▼ orden, embudo filtro) + ▾ menú.
#[allow(clippy::too_many_arguments)]
fn column_header(
    ui: &mut egui::Ui,
    col: &naygo_core::columns::ColumnSpec,
    table: &naygo_core::columns::TableState,
    sort: naygo_core::fs_model::SortSpec,
    ext_counts: &std::collections::BTreeMap<String, usize>,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<crate::table_actions::TableAction>,
) {
    use naygo_core::columns::sort_key_of;
    let title = column_title(col.kind, i18n);
    let is_sorted = sort.key == sort_key_of(col.kind);
    let is_filtered = table.filters.contains_key(&col.kind);
    let mut label = title.to_string();
    if is_sorted {
        label.push_str(if sort.ascending { " ▲" } else { " ▼" });
    }
    if is_filtered {
        label.push_str(" ⏷"); // embudo (indicador de filtro activo)
    }
    // Botón-menú: clic abre el desplegable modo B.
    let resp = ui.add(egui::Button::new(egui::RichText::new(format!("{label}  ▾")).strong()).frame(false));
    let popup_id = ui.make_persistent_id(("colmenu", col.kind));
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    crate::column_menu::show_menu(ui, &resp, popup_id, col.kind, table, sort, ext_counts, i18n, actions);
}

/// Título i18n de una columna.
fn column_title(kind: naygo_core::columns::ColumnKind, i18n: &naygo_core::i18n::I18n) -> String {
    use naygo_core::columns::ColumnKind;
    let key = match kind {
        ColumnKind::Name => "col.name",
        ColumnKind::Extension => "col.extension",
        ColumnKind::Size => "col.size",
        ColumnKind::Modified => "col.modified",
        ColumnKind::Created => "col.created",
    };
    i18n.t(key).to_string()
}
```

Renombrar `format_modified(entry)` → un `format_time(opt: Option<SystemTime>)` reutilizable para Modified y Created (mantener el formato epoch-secs provisional existente):
```rust
fn format_time(t: Option<std::time::SystemTime>) -> String {
    use std::time::UNIX_EPOCH;
    match t.and_then(|t| t.duration_since(UNIX_EPOCH).ok()) {
        Some(d) => format!("{}", d.as_secs()),
        None => String::new(),
    }
}
```
Eliminar la antigua `format_modified` y los usos directos (ahora vía `cell_text`).

NOTA: el `header_label` y `header_clicked` de 2D se eliminan (el orden ahora va por el menú). Quitar también el uso de `crate::sort_ui::next_sort_on_header_click` en file_panel SI ya no se necesita (el menú ofrece asc/desc directos). `sort_ui.rs` puede quedar (sus tests siguen válidos) o marcarse; NO lo borres en esta tarea para no romper sus tests — déjalo, simplemente file_panel deja de llamarlo. Si clippy se queja de import sin usar, quita solo ese `use`.

- [ ] **Step 3: `column_menu.rs` — render del desplegable modo B (egui)**

Añadir a `crates/ui/src/column_menu.rs` la función de render (además de las puras de la Tarea 6). VERIFICAR contra egui 0.34.3: `egui::popup::popup_below_widget` (o `egui::Popup`), su firma y `PopupCloseBehavior`; `ui.text_edit_singleline(&mut String)`; `ui.checkbox(&mut bool, text)`; `ui.selectable_label`; `egui::ComboBox` para la unidad. Adaptar.

```rust
/// Render del desplegable de una columna (modo B): ordenar directo + sub-secciones
/// Filtrar/Columnas + quitar filtro. Empuja `TableAction`s. Filtro EN VIVO (cada
/// cambio emite una acción que se aplica el mismo frame).
#[allow(clippy::too_many_arguments)]
pub fn show_menu(
    ui: &egui::Ui,
    anchor: &egui::Response,
    popup_id: egui::Id,
    kind: ColumnKind,
    table: &naygo_core::columns::TableState,
    sort: SortSpec,
    ext_counts: &std::collections::BTreeMap<String, usize>,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<TableAction>,
) {
    egui::popup::popup_below_widget(
        ui,
        popup_id,
        anchor,
        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(220.0);
            // Ordenar (directo).
            if ui.button(format!("▲ {}", i18n.t("menu.sort_asc"))).clicked() {
                actions.push(sort_action(kind, true, sort.dirs_first));
            }
            if ui.button(format!("▼ {}", i18n.t("menu.sort_desc"))).clicked() {
                actions.push(sort_action(kind, false, sort.dirs_first));
            }
            ui.separator();
            // Filtrar (sub-sección, en vivo) — el control depende del tipo de columna.
            ui.collapsing(i18n.t("menu.filter"), |ui| {
                filter_controls(ui, kind, table, ext_counts, i18n, actions);
            });
            // Columnas (mostrar/ocultar/reordenar).
            ui.collapsing(i18n.t("menu.columns"), |ui| {
                columns_controls(ui, table, i18n, actions);
            });
            ui.separator();
            if ui.button(format!("✕ {}", i18n.t("menu.clear_filter"))).clicked() {
                actions.push(TableAction::ClearFilter(kind));
            }
        },
    );
}
```

Y dos helpers de render, `filter_controls` y `columns_controls`. IMPORTANTE para el **filtro en vivo con estado de UI**: los controles necesitan estado local entre frames (texto que se escribe, checkboxes). Usar `egui::Memory`/`ui.memory_mut` con `data` por `popup_id`+columna, o `ui.id().with(...)` + `egui::TextEdit` cuyo buffer se guarda en memoria. Patrón concreto para el texto de Nombre:

```rust
fn filter_controls(
    ui: &mut egui::Ui,
    kind: ColumnKind,
    table: &naygo_core::columns::TableState,
    ext_counts: &std::collections::BTreeMap<String, usize>,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<TableAction>,
) {
    use naygo_core::filter::ColumnFilter;
    match kind {
        ColumnKind::Name => {
            // Estado del texto en memoria (persistente entre frames).
            let id = ui.make_persistent_id(("name_filter", kind));
            let mut text: String = ui
                .memory(|m| m.data.get_temp(id))
                .or_else(|| match table.filters.get(&kind) {
                    Some(ColumnFilter::Text { contains, .. }) => Some(contains.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let mut case_id = ui.make_persistent_id(("name_case", kind));
            let mut case: bool = ui.memory(|m| m.data.get_temp(case_id)).unwrap_or(false);
            ui.label(i18n.t("filter.name_contains"));
            let changed_text = ui.text_edit_singleline(&mut text).changed();
            let changed_case = ui.checkbox(&mut case, i18n.t("filter.case_sensitive")).changed();
            if changed_text || changed_case {
                ui.memory_mut(|m| m.data.insert_temp(id, text.clone()));
                ui.memory_mut(|m| m.data.insert_temp(case_id, case));
                actions.push(name_filter_action(&text, case)); // EN VIVO
            }
            let _ = &mut case_id;
        }
        ColumnKind::Extension => {
            // Conjunto marcado en memoria; arranca del filtro actual.
            let id = ui.make_persistent_id(("ext_filter", kind));
            let mut selected: std::collections::BTreeSet<String> = ui
                .memory(|m| m.data.get_temp(id))
                .or_else(|| match table.filters.get(&kind) {
                    Some(ColumnFilter::Extensions(s)) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            let mut changed = false;
            for (ext, count) in ext_counts {
                let label = if ext.is_empty() {
                    format!("{} ({count})", i18n.t("filter.no_extension"))
                } else {
                    format!(".{ext} ({count})")
                };
                let mut on = selected.contains(ext);
                if ui.checkbox(&mut on, label).changed() {
                    if on { selected.insert(ext.clone()); } else { selected.remove(ext); }
                    changed = true;
                }
            }
            if changed {
                ui.memory_mut(|m| m.data.insert_temp(id, selected.clone()));
                actions.push(extensions_filter_action(selected)); // EN VIVO
            }
        }
        ColumnKind::Size => {
            // Rango simple (sin unidad combo para simplificar; bytes directos en KB).
            let id_min = ui.make_persistent_id(("size_min", kind));
            let id_max = ui.make_persistent_id(("size_max", kind));
            let mut min_s: String = ui.memory(|m| m.data.get_temp(id_min)).unwrap_or_default();
            let mut max_s: String = ui.memory(|m| m.data.get_temp(id_max)).unwrap_or_default();
            ui.label(i18n.t("filter.size_from"));
            let c1 = ui.text_edit_singleline(&mut min_s).changed();
            ui.label(i18n.t("filter.size_to"));
            let c2 = ui.text_edit_singleline(&mut max_s).changed();
            if c1 || c2 {
                ui.memory_mut(|m| m.data.insert_temp(id_min, min_s.clone()));
                ui.memory_mut(|m| m.data.insert_temp(id_max, max_s.clone()));
                // Interpretar como KB → bytes.
                let parse_kb = |s: &str| s.trim().parse::<f64>().ok().map(|v| to_bytes(v, SizeUnit::Kb));
                actions.push(size_filter_action(parse_kb(&min_s), parse_kb(&max_s))); // EN VIVO
            }
        }
        ColumnKind::Modified | ColumnKind::Created => {
            // Rango de fecha: para 2E, entrada simple por segundos epoch (provisional,
            // coherente con format_time provisional). Un date-picker rico es trabajo futuro.
            ui.label(i18n.t("filter.date_from"));
            ui.label("—");
            ui.weak("(rango de fecha: pendiente UI rica)");
            // NO emitir acción aquí: el control de fecha rico llega después; dejar el
            // sub-panel visible pero inerte evita un control a medias. (Ver nota.)
        }
    }
}

fn columns_controls(
    ui: &mut egui::Ui,
    table: &naygo_core::columns::TableState,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<TableAction>,
) {
    for col in &table.columns {
        let title = match col.kind {
            ColumnKind::Name => i18n.t("col.name"),
            ColumnKind::Extension => i18n.t("col.extension"),
            ColumnKind::Size => i18n.t("col.size"),
            ColumnKind::Modified => i18n.t("col.modified"),
            ColumnKind::Created => i18n.t("col.created"),
        };
        let mut vis = col.visible;
        let enabled = col.kind != ColumnKind::Name; // Nombre no ocultable
        ui.add_enabled_ui(enabled, |ui| {
            if ui.checkbox(&mut vis, title).changed() {
                actions.push(TableAction::ToggleColumn(col.kind));
            }
        });
    }
}
```

IMPORTANTE — DECISIÓN DE ALCANCE para fechas: el spec lista rango de fecha como control, pero un date-picker usable en egui es trabajo no trivial. Para 2E, el sub-panel de fecha queda VISIBLE pero INERTE (placeholder i18n "rango de fecha: pendiente UI rica") y NO emite filtro. Los filtros de Nombre/Extensión/Tamaño SÍ funcionan completos. Reportar esto como DONE_WITH_CONCERNS para que el controlador decida si quiere el date-picker en 2E o lo difiere. (El core `DateRange` ya está testeado y listo; solo falta la UI del control.) Añadir la clave i18n `filter.date_pending` = "(rango de fecha: pendiente UI rica)" / "(date range: rich UI pending)" en es/en si se usa el texto — o reutilizar un texto neutro. Para no agregar claves sobre la marcha, usar `i18n.t("filter.date_from")` + un `ui.weak("—")` sin texto hardcoded; NO hardcodear español. Si necesitas el texto placeholder, AÑADE la clave en es/en en esta tarea.

- [ ] **Step 4: `app.rs` — procesar TableActions**

Modify `crates/ui/src/app.rs`, método `ui()`:
- Declarar `let mut table_actions: Vec<(PaneId, crate::table_actions::TableAction)> = Vec::new();` junto a `pending`/`tree_actions`.
- Pasar `table_actions: &mut table_actions` al `NaygoTabViewer`.
- En el brazo Files del viewer (docking.rs Step 1), recoger las acciones locales del file_panel y taggear con el PaneId (igual patrón que tree_actions): el `file_panel::show` recibe un `&mut Vec<TableAction>` local; tras llamarlo, `for a in local { self.table_actions.push((id, a)); }`.
- Tras procesar `pending`/`tree_actions`, procesar table_actions:
```rust
        for (id, action) in table_actions {
            if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                match action {
                    crate::table_actions::TableAction::SetSort(spec) => { f.sort = spec; }
                    crate::table_actions::TableAction::SetFilter(kind, filter) => { f.table.set_filter(kind, filter); }
                    crate::table_actions::TableAction::ClearFilter(kind) => { f.table.clear_filter(kind); }
                    crate::table_actions::TableAction::ToggleColumn(kind) => { f.table.toggle_visible(kind); }
                    crate::table_actions::TableAction::MoveColumn(from, to) => { f.table.move_column(from, to); }
                    crate::table_actions::TableAction::SetColumnWidth(kind, w) => { f.table.set_width(kind, w); }
                }
            }
        }
```
(El re-sort/re-filtro ocurre solo en el render del frame siguiente, que recalcula el pipeline — no hace falta re-ordenar `entries` aquí; el pipeline es por frame.)

- [ ] **Step 5: Quitar los `#[allow(dead_code)]`**

Quitar los allow puestos en Tareas 5/6 (TableAction y column_menu puras ahora se usan).

- [ ] **Step 6: Compilar, verificar, formatear**

Run: `cargo build -p naygo-ui` → compila (resolver egui API: popup, text_edit, checkbox, memory data; borrows). 
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start (`--bin naygo`): el file panel muestra Nombre/Extensión/Tamaño/Modificado; clic en ▾ abre el menú modo B; ordenar asc/desc funciona; filtrar por Nombre (en vivo), Extensión (checkboxes + contador), Tamaño (rango KB) funciona y filtra al instante; multi-filtro = AND; columna con orden/filtro muestra ▲▼/⏷; mostrar/ocultar columnas funciona (Nombre deshabilitado); "sin coincidencias" cuando el filtro vacía la lista.

- [ ] **Step 7: Commit**

```bash
git add crates/ui/src/panes/file_panel.rs crates/ui/src/column_menu.rs crates/ui/src/docking.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): tabla rica estilo Excel (columnas dinámicas, menú, filtro en vivo)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Filtro de FECHA — date-picker de rango (Modificado/Creación)

**Files:**
- Modify: `crates/ui/src/column_menu.rs` (reemplazar el placeholder de fecha por un control real)
- Modify: `crates/core/src/i18n/{es,en}.json` (si se requieren textos nuevos)

Completa el control de filtro por rango de fecha que en la Tarea 7 quedó como placeholder inerte. El core `ColumnFilter::DateRange` ya está testeado (Tarea 1); aquí va SOLO la UI.

VERIFICAR egui 0.34.3: no hay date-picker nativo en egui core (el `egui_extras::DatePickerButton` requiere la feature `datepicker` y la dep `chrono`). DECISIÓN para no sumar dependencias pesadas: usar **tres campos numéricos año/mes/día** por extremo (Desde / Hasta), convertidos a `SystemTime` con aritmética de `std::time` (sin chrono). Es un control simple, sin dependencias nuevas, coherente con la premisa de bajo peso.

- [ ] **Step 1: Helper puro de fecha → SystemTime con test (en column_menu.rs)**

Añadir a `crates/ui/src/column_menu.rs` una función pura + tests:

```rust
/// Convierte (año, mes 1-12, día 1-31) a un `SystemTime` (medianoche UTC de ese
/// día), usando solo `std::time`. Devuelve `None` si la fecha es inválida.
/// Algoritmo de días-desde-época (proleptic Gregorian), sin dependencias externas.
pub fn ymd_to_system_time(year: i32, month: u32, day: u32) -> Option<std::time::SystemTime> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year < 1970 {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    let secs = (days as u64).checked_mul(86_400)?;
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

/// Días desde 1970-01-01 (inclusive) para una fecha civil válida. `None` si el día
/// excede los del mes. Basado en el algoritmo de Howard Hinnant (días civiles).
fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    // Validar día contra los días del mes (con bisiesto).
    let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let dim = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if day > dim[(month - 1) as usize] {
        return None;
    }
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64; // [0, 399]
    let m = month as i64;
    let d = day as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days = era * 146097 + doe - 719468; // días desde 1970-01-01
    Some(days)
}

/// Acción al fijar un rango de fecha para una columna (Modified/Created). Ambos
/// None → quitar filtro.
pub fn date_filter_action(
    kind: ColumnKind,
    from: Option<std::time::SystemTime>,
    to: Option<std::time::SystemTime>,
) -> TableAction {
    if from.is_none() && to.is_none() {
        TableAction::ClearFilter(kind)
    } else {
        TableAction::SetFilter(kind, ColumnFilter::DateRange { from, to })
    }
}
```

Tests (añadir al `mod tests` de column_menu.rs):
```rust
    #[test]
    fn ymd_epoch_es_cero_dias() {
        let t = ymd_to_system_time(1970, 1, 1).unwrap();
        assert_eq!(t, std::time::UNIX_EPOCH);
    }

    #[test]
    fn ymd_un_dia_despues() {
        let t = ymd_to_system_time(1970, 1, 2).unwrap();
        assert_eq!(t, std::time::UNIX_EPOCH + std::time::Duration::from_secs(86_400));
    }

    #[test]
    fn ymd_rechaza_fecha_invalida() {
        assert!(ymd_to_system_time(2021, 2, 29).is_none()); // 2021 no bisiesto
        assert!(ymd_to_system_time(2020, 2, 29).is_some()); // 2020 bisiesto
        assert!(ymd_to_system_time(2021, 13, 1).is_none());
        assert!(ymd_to_system_time(1969, 1, 1).is_none());
    }

    #[test]
    fn date_filter_ambos_none_quita() {
        assert_eq!(
            date_filter_action(ColumnKind::Modified, None, None),
            TableAction::ClearFilter(ColumnKind::Modified)
        );
    }
```

- [ ] **Step 2: Correr y ver fallar, luego pasar**

Run: `cargo test -p naygo-ui column_menu` → primero falla (funciones nuevas), tras implementar PASA (los 4 nuevos + los de la Tarea 6).

- [ ] **Step 3: Render del control de fecha (reemplazar el placeholder de la Tarea 7)**

En `filter_controls` de `column_menu.rs`, reemplazar el brazo `ColumnKind::Modified | ColumnKind::Created` (placeholder inerte) por un control real: tres `DragValue`/campos numéricos (año/mes/día) por extremo, en memoria por `make_persistent_id` (incluyendo el PaneId para no colisionar entre paneles), emitiendo `date_filter_action` EN VIVO cuando cambian. Esqueleto:

```rust
        ColumnKind::Modified | ColumnKind::Created => {
            let id_from = ui.make_persistent_id(("date_from", kind, pane_id));
            let id_to = ui.make_persistent_id(("date_to", kind, pane_id));
            // (y, m, d) por extremo en memoria; 0 = sin valor.
            let mut from: (i32, u32, u32) = ui.memory(|m| m.data.get_temp(id_from)).unwrap_or((0, 0, 0));
            let mut to: (i32, u32, u32) = ui.memory(|m| m.data.get_temp(id_to)).unwrap_or((0, 0, 0));
            let mut changed = false;
            ui.label(i18n.t("filter.date_from"));
            ui.horizontal(|ui| {
                changed |= ui.add(egui::DragValue::new(&mut from.0).range(0..=9999).prefix("A ")).changed();
                changed |= ui.add(egui::DragValue::new(&mut from.1).range(0..=12).prefix("M ")).changed();
                changed |= ui.add(egui::DragValue::new(&mut from.2).range(0..=31).prefix("D ")).changed();
            });
            ui.label(i18n.t("filter.date_to"));
            ui.horizontal(|ui| {
                changed |= ui.add(egui::DragValue::new(&mut to.0).range(0..=9999).prefix("A ")).changed();
                changed |= ui.add(egui::DragValue::new(&mut to.1).range(0..=12).prefix("M ")).changed();
                changed |= ui.add(egui::DragValue::new(&mut to.2).range(0..=31).prefix("D ")).changed();
            });
            if changed {
                ui.memory_mut(|m| m.data.insert_temp(id_from, from));
                ui.memory_mut(|m| m.data.insert_temp(id_to, to));
                let f = ymd_to_system_time(from.0, from.1, from.2);
                let t = ymd_to_system_time(to.0, to.1, to.2);
                actions.push(date_filter_action(kind, f, t)); // EN VIVO
            }
        }
```
NOTA: `filter_controls` debe recibir el `pane_id` para los ids persistentes (ajustar su firma y la llamada en `show_menu`/`column_header`; pasar el PaneId desde file_panel). Verificar egui 0.34.3: `egui::DragValue::new(&mut T).range(RangeInclusive)` (en 0.34 puede ser `.range(..)` o `.clamp_range(..)` — VERIFICAR y usar el correcto) y `.prefix(..)`. Si `range` no existe con ese nombre, usar el que provea 0.34.

Quitar la clave/placeholder de fecha inerte si se había añadido en la Tarea 7.

- [ ] **Step 4: Verificar**

Run: `cargo test -p naygo-ui` → verde. `cargo clippy --workspace --all-targets -- -D warnings` → limpio. `cargo fmt`.
App-start: filtrar por rango de fecha en Modificado/Creación funciona en vivo.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/column_menu.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): filtro de rango de fecha (date-picker simple sin dependencias)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Reordenar columnas por arrastre del encabezado

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`

Completa el reordenamiento de columnas por arrastre (la op `TableState::move_column` y la acción `TableAction::MoveColumn` ya existen y están testeadas; aquí va la UI de drag&drop entre encabezados).

VERIFICAR egui 0.34.3: el patrón de drag&drop — `egui::DragAndDrop`, `ui.dnd_drag_source(id, payload, |ui| {...})` y `ui.dnd_drop_zone::<Payload, _>(frame, |ui| {...})` (API de drag&drop de egui 0.34). Si esa API difiere, usar el mecanismo de 0.34 (puede ser `Response::dnd_set_drag_payload` / `Response::dnd_release_payload`). Adaptar.

- [ ] **Step 1: Hacer cada encabezado fuente y zona de drop**

En `column_header` (file_panel.rs), envolver el botón-menú del encabezado como **drag source** con payload = el índice visual de esa columna, y como **drop zone** que, al soltar, emite `TableAction::MoveColumn(from_index, to_index)`. Los índices son posiciones en `visible_cols` mapeadas a posiciones reales en `table.columns` (cuidado: `move_column` opera sobre el Vec completo `columns`, no solo visibles; convertir el índice visible al índice real buscando el `kind` en `table.columns`).

Esqueleto (adaptar a la API real de 0.34):
```rust
// dentro del loop de encabezados en file_panel::show, con el índice visible `vi`:
let col_real_index = table.columns.iter().position(|c| c.kind == col.kind).unwrap();
let dnd_id = egui::Id::new(("colhdr", id.0, col.kind));
let resp = ui.dnd_drag_source(dnd_id, col_real_index, |ui| {
    column_header(ui, col, &table, sort, &ext_counts, i18n, table_actions, id);
}).response;
if let Some(from) = resp.dnd_release_payload::<usize>() {
    let to = col_real_index;
    if *from != to {
        table_actions.push(crate::table_actions::TableAction::MoveColumn(*from, to));
    }
}
```
NOTA: esto es conceptual. VERIFICAR los nombres exactos en egui 0.34.3 (`dnd_drag_source`, `dnd_release_payload`, o el par `dnd_set_drag_payload`/`dnd_drop_zone`). El payload debe ser `Clone + Send + Sync + 'static` (un `usize` lo cumple). Mantén el render del contenido del header (título+indicadores+▾) idéntico dentro del drag source.

- [ ] **Step 2: Verificar**

Run: `cargo build -p naygo-ui` → compila. `cargo clippy --workspace --all-targets -- -D warnings` → limpio. `cargo test --workspace` → verde. `cargo fmt`.
App-start: arrastrar un encabezado sobre otro reordena las columnas; persiste por panel.

NOTA: el reordenamiento por arrastre es UI sin lógica nueva testeable (la op `move_column` ya tiene test en la Tarea 2). Validación manual.

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src/panes/file_panel.rs
git commit -m "feat(ui): reordenar columnas por arrastre del encabezado

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Anchos de columna por arrastre del borde

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`

Completa el redimensionado de columnas arrastrando el borde (la op `TableState::set_width` con clamp ya existe y está testeada; aquí va la UI).

CONTEXTO: el file_panel usa `egui::Grid`, que NO da control fino de anchos por columna con handles arrastrables. Para anchos arrastrables reales conviene migrar la tabla a **`egui_extras::TableBuilder`** (que soporta `Column::initial(w).resizable(true)` y reporta el ancho). VERIFICAR si `egui_extras` ya es dependencia de `naygo-ui` (revisar `crates/ui/Cargo.toml`); si NO está, añadir `egui_extras = "0.34"` (misma línea de versión que egui; es del mismo autor, MIT). 

DECISIÓN: migrar el cuerpo de la tabla de `Grid` a `egui_extras::TableBuilder` con columnas `resizable(true)` usando los anchos de `table.columns`. Al detectar que el usuario cambió un ancho, emitir `TableAction::SetColumnWidth(kind, nuevo_ancho)`.

- [ ] **Step 1: Asegurar la dependencia egui_extras**

Revisar `crates/ui/Cargo.toml`. Si falta, añadir bajo `[dependencies]`:
```toml
egui_extras = "0.34"
```
Run `cargo build -p naygo-ui` para que baje la dep.

- [ ] **Step 2: Migrar la tabla a TableBuilder con columnas redimensionables**

En `file_panel::show`, reemplazar el `egui::Grid` por `egui_extras::TableBuilder`. VERIFICAR egui_extras 0.34 API: `TableBuilder::new(ui).columns(...)` o `.column(Column::initial(w).resizable(true).at_least(MIN))`, `.header(height, |header| { header.col(|ui| {...}) })`, `.body(|body| { body.rows(row_h, n, |mut row| { row.col(|ui| {...}) }) })`. El ancho resultante de cada columna se obtiene del layout; para persistirlo, egui_extras reporta los anchos vía el estado de la tabla — si la API no expone el ancho directamente cada frame, una alternativa es leer el ancho asignado del `Column` tras el render. Si obtener el ancho exacto es difícil con la API de 0.34, usar el enfoque: cuando el usuario arrastra el separador, egui_extras persiste el ancho en su propio estado (id-scoped); en ese caso, leer ese estado y reflejarlo en `TableState` al cerrar/guardar. INVESTIGAR la API real y elegir el camino más limpio; documentar en el commit cuál se usó.

Mantener: encabezados con menú (Tarea 7), indicadores, fila "..", filtrar→ordenar, selección/activación de filas, "sin coincidencias".

- [ ] **Step 3: Emitir SetColumnWidth al cambiar un ancho**

Cuando el ancho de una columna cambie respecto a `table.columns[i].width`, empujar `TableAction::SetColumnWidth(kind, nuevo)`. El clamp lo hace `set_width` en core.

- [ ] **Step 4: Verificar**

Run: `cargo build -p naygo-ui` → compila. `cargo clippy --workspace --all-targets -- -D warnings` → limpio. `cargo test --workspace` → verde. `cargo fmt`.
App-start: arrastrar el borde de una columna la ensancha/encoge; el ancho persiste por panel.

NOTA: migrar Grid→TableBuilder es el cambio de mayor riesgo de 2E. Si surge un bloqueo serio con la API de anchos de egui_extras 0.34, reportar BLOCKED con el detalle; el controlador decidirá (p. ej. mantener Grid + un handle de ancho manual, o diferir solo los anchos). Las demás features de 2E NO dependen de esta tarea.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/panes/file_panel.rs crates/ui/Cargo.toml
git commit -m "feat(ui): anchos de columna redimensionables (migración a TableBuilder)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: Realce del panel activo (barra de título de acento)

**Files:**
- Modify: `crates/ui/src/docking.rs`

- [ ] **Step 1: Pintar el título del tab activo en color de acento**

Modify `crates/ui/src/docking.rs`. egui_dock pinta los títulos de tab vía `TabViewer::title()`, que devuelve `WidgetText`. Para resaltar el panel activo, usar el `active_id` del workspace: si el tab que se está titulando es el panel activo, devolver el texto en color de acento.

VERIFICAR egui 0.34.3: `egui::WidgetText` desde `RichText::new(s).color(Color32)`. egui_dock 0.19 `TabViewer::title(&mut self, tab) -> WidgetText`.

En `fn title`, tras computar el `name` del tab, envolverlo:
```rust
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        let name = /* ... cómputo actual del nombre ... */;
        let is_active = self.workspace.active_id() == Some(*tab);
        if is_active {
            egui::RichText::new(name)
                .color(egui::Color32::from_rgb(0x2f, 0x81, 0xf7))
                .strong()
                .into()
        } else {
            name.into()
        }
    }
```
NOTA: esto colorea el TEXTO del título del tab activo (aproximación de "barra de título de acento" compatible con egui_dock, que no expone fácilmente el fondo de la barra del tab por panel). Si egui_dock 0.19 permite estilizar el fondo del tab activo vía `Style`, evaluarlo; si no, el texto de acento + negrita es la señal clara y de bajo riesgo. Reportar cuál se usó.

Como el `title()` actual computa el nombre para `PanePurpose::Files` (nombre de carpeta) y usa claves i18n para Tree/Inspector, aplicar el realce a TODOS los casos (envolver el resultado final). Refactor menor: que `title()` calcule `let name: String = match purpose { ... }` y luego aplique el color si `is_active`.

- [ ] **Step 2: Verificar**

Run: `cargo build -p naygo-ui` → compila.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start: el tab del panel activo se ve en color de acento + negrita; al cambiar de panel activo (clic/Tab), el realce se mueve.

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src/docking.rs
git commit -m "feat(ui): resaltar el panel activo (título del tab en color de acento)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 12: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: Actualizar el README**

Modify `README.md` — bloque de estado:
```markdown
> **Estado:** Fase 2E (columnas estilo Excel + panel activo) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-06-naygo-fase2e-columnas-excel-design.md`](docs/superpowers/specs/2026-06-06-naygo-fase2e-columnas-excel-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-06-naygo-fase2e-columnas-excel.md`](docs/superpowers/plans/2026-06-06-naygo-fase2e-columnas-excel.md).
> Fases 1, 2A, 2B, 2C-i, 2D y el árbol de directorios completas.
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → todo verde (core: filter + columns + file_pane migración; ui: table_action + column_menu).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo fmt --check` → limpio.
Run: `cargo build --release -p naygo-ui` → release compila.
App-start manual: repasar el checklist de la Tarea 7 Step 6 + filtro de fecha (Tarea 8) + reordenar columnas por arrastre (Tarea 9) + anchos por arrastre (Tarea 10) + realce del panel activo (Tarea 11).

- [ ] **Step 3: Commit y push**

```bash
git add README.md
git commit -m "chore: actualizar estado del README (Fase 2E)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/fase2e-columnas-excel
```

---

## Self-review (cobertura del spec)

| Requisito del spec 2E | Tarea(s) |
|---|---|
| `core::filter` (ColumnFilter 4 tipos + matches AND) | 1 |
| `extension_counts` | 1 |
| `core::columns` (ColumnKind/ColumnSpec/TableState + ops) | 1 (mínimo) + 2 |
| `sort_key_of` (ColumnKind↔SortKey) | 2 |
| Nombre no ocultable | 2 (toggle_visible) |
| `table: TableState` en file_pane + migración text_filter | 3 |
| Persistencia (serde, columnas+filtros) | 2 (round_trip) + 3 (persist) |
| i18n nuevas (menú/filtros/sin-coincidencias) | 4 |
| Acciones diferidas (TableAction) | 5 |
| Lógica pura del menú (acciones de interacción) | 6 |
| Desplegable modo B (ordenar/filtrar/columnas) | 7 (column_menu render) |
| Filtro en vivo | 7 (emite acción en cada cambio) |
| Controles por tipo (texto/checkboxes+contador/rango) | 7 (filter_controls) |
| Indicadores (▲▼ orden, embudo filtro, columna resaltada) | 7 (column_header) |
| Columnas visibles dinámicas en orden | 7 (file_panel) |
| Pipeline filtrar→ordenar en memoria | 7 |
| Multi-filtro = AND | 1 (matches) + 7 (aplica todos) |
| "Sin coincidencias" | 7 |
| Mostrar/ocultar columnas | 7 (columns_controls) |
| Filtro de FECHA (date-picker de rango) | 8 |
| Reordenar columnas por arrastre | 9 |
| Anchos de columna por arrastre | 10 |
| Panel activo resaltado (barra de título acento) | 11 |

> Nota: en Tarea 7, el control de fecha se deja como placeholder inerte SOLO hasta
> la Tarea 8 (que lo reemplaza por el date-picker real). El filtro de fecha SÍ entra
> en 2E (decisión del usuario: los 3 controles finos van completos).

**Notas de riesgo:**
- egui 0.34.3: verificar `popup::popup_below_widget`/`PopupCloseBehavior`, `memory.data.get_temp/insert_temp`, `text_edit_singleline`, `checkbox`, `add_enabled_ui`, `Button::frame(false)`, `RichText::color`, `make_persistent_id`, `toggle_popup` contra la fuente del registry antes de implementar la Tarea 7 (patrón de fases previas). Si una firma cambió, adaptar.
- egui_dock 0.19 `TabViewer::title -> WidgetText` y si permite estilizar el fondo del tab activo; si no, usar texto de acento (Tarea 11).
- Estado de UI del filtro en vivo: usar `memory.data` por `make_persistent_id` para el texto/checkbox/set entre frames (incluido en el plan). Cuidar que el id sea estable por (columna, panel) para no mezclar estado entre paneles — incluir el PaneId además de la columna en TODOS los `make_persistent_id` de filtros (Tareas 7 y 8). Si dos paneles muestran la misma columna, sus filtros locales no deben colisionar.
- Tarea 8 (fecha): NO sumar `chrono`/`egui_extras::datepicker`; usar el helper puro `ymd_to_system_time` (incluido y testeado) + `DragValue`. Verificar `DragValue::range`/`clamp_range` en 0.34.
- Tarea 9 (reordenar): verificar la API de drag&drop de egui 0.34 (`dnd_drag_source`/`dnd_release_payload` o `dnd_set_drag_payload`/`dnd_drop_zone`) antes de implementar. Convertir índice visible → índice real en `table.columns` para `move_column`.
- Tarea 10 (anchos): mayor riesgo. Migrar `Grid`→`egui_extras::TableBuilder` (añadir dep `egui_extras = "0.34"` si falta). Investigar cómo 0.34 reporta el ancho redimensionado para persistirlo; si se bloquea, reportar BLOCKED (las demás features no dependen de esta).
- Migración `text_filter`: `#[serde(default)]` en los campos nuevos del persist para que settings viejos carguen sin romper (incluido).
