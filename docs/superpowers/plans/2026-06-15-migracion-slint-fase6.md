# Fase 6 (Slint): pulido visual + paridad total + retiro de egui — Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Llevar el look de la UI Slint al nivel del de egui (íconos de color por tipo, toolbar
con íconos, columnas dinámicas con formato inteligente, rename en cadena), completar la paridad
funcional, mantener/extender los packs importables (temas+íconos+idiomas), y retirar la capa egui
— todo manteniendo el default cero-GPU e instantáneo.

**Architecture:** La lógica pura vive en `naygo-core` (icon_kind, icons [se mueve aquí], columns,
format, rename). La UI Slint la consume con un `IconCache` que decodifica cada PNG a
`slint::Image` UNA vez (clave del rendimiento). Las columnas se renderizan dinámicas desde el
`TableState` persistido por panel. El rename inline reusa el engine de ops de F3.

**Tech Stack:** Rust, Slint 1.16 (winit software), crate `image` (ya presente), `naygo-core::
{icon_kind, icons, columns, format, ops}`.

**Convenciones (OBLIGATORIAS):**
- `graphify query` antes de leer/grepear; `graphify update .` tras cambios.
- Gate antes de CADA commit: `cargo test --workspace` + `cargo clippy --workspace --all-targets
  -- -D warnings` + `cargo fmt --all -- --check`.
- Stage EXPLÍCITO (nunca `git add -A`): `CLAUDE.md` y `graphify-out/` NO se commitean.
- Commits en español (heredoc) terminando con `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Header en archivos nuevos: `// Naygo — <desc>.` / `// Copyright (c) 2026 Nicolás Groth /
  ISGroth. MIT License.`
- i18n: texto nuevo a claves (`Tr` + es.json/en.json en paridad). Colores vía `Theme`.
- Probar la GUI en ESTA máquina con binario real (Win32 `Start-Process`, `SetForegroundWindow`
  por PID, captura con PrintWindow flags=3 que NO se enmascara; computer-use para clics/teclas).
  Matar instancias previas (el HideWindow puede dejar el proceso vivo). Nicolás mide rendimiento
  en la VM.
- Trabajar en `main` (como F2–F5). Push al cierre de cada sub-fase grande, autorizado por Nicolás.

**APIs reusadas (firmas verificadas):**
- `core::icon_kind::{icon_key_for(&Entry)->IconKey, category_for_extension(&str)->FileCategory,
  IconKey, FileCategory, ActionIcon::{all,file_name}, DriveKind}`.
- `core::config::IconSet::{Flat, Fluent, Mono}` (default Flat).
- `core::columns::{TableState{columns:Vec<ColumnSpec>,filters}, ColumnSpec{kind,visible,width},
  ColumnKind{Name,Extension,Size,Modified,Created}, visible_columns()->iter, toggle_visible(kind),
  move_column(from,to), set_width(kind,f32), sort_key_of(kind)->SortKey, MIN/MAX_COLUMN_WIDTH}`.
  `FilePaneState.table: Option<TableState>` (persiste; `None` = default).
- `core::format::{human_size(u64)->String, format_time, DateFormat}` (de F4-bis).
- `core::ops::actions::rename(path, new_name)->OpRequest`; `OpsCtrl::start_op(req,label,undo)`.
- Assets de íconos a MOVER: `crates/ui/src/icons/assets.rs` → `crates/core/src/icons/mod.rs`:
  `bytes_for(IconSet,IconKey)->&'static [u8]` (PNGs embebidos de los 3 sets, fallback a unknown),
  `file_name(IconKey)->&str`, `all_keys()->Vec<IconKey>`. Es PURO (solo include_bytes! + core).
- Lógica del ciclo de rename (de egui, a portar a core): stage 0 = nombre sin ext, 1 = ext, else =
  todo. `rsplit_once('.')` con stem/ext no vacíos.

**Estructura de archivos:**
- `crates/core/src/icons/mod.rs` (mover desde egui): tabla de bytes PNG por (set,key).
- `crates/core/src/rename.rs` (nuevo): rangos del ciclo F2 (puro).
- `crates/core/src/format.rs` (mod): `SizeFormat` + `format_size`.
- `crates/ui-slint/src/icons.rs` (nuevo): `IconCache` (PNG→slint::Image, cacheado).
- `crates/ui-slint/src/bridge.rs` (mod): icono por fila, celdas por columna, tamaño formateado.
- `crates/ui-slint/src/workspace_ctrl.rs` / `main.rs` / `ui/*.slint` (mod): render dinámico,
  toolbar con íconos, rename inline, menú de columnas.
- `crates/ui-slint/src/packs.rs` (mod): packs con íconos.
- Retiro: `crates/ui` (egui) sale del workspace en 6F.

---

## Sub-fase 6A — Íconos de archivo de color (cacheados)

### Task 1: mover los assets de íconos a `naygo-core::icons`

**Files:**
- Create: `crates/core/src/icons/mod.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod icons;`)
- Modify: `crates/ui/src/icons/assets.rs` (re-exportar desde core)

- [ ] **Step 1: Orientar**

Run: `graphify query "icons assets bytes_for file_name all_keys include_bytes set_table IconSet"`.

- [ ] **Step 2: Crear `crates/core/src/icons/mod.rs`**

Copiar el contenido de `crates/ui/src/icons/assets.rs` (la tabla completa de `set_table!` con
los 28 nombres × 3 sets, `file_name`, `table_for`, `bytes_for`, `all_keys` y sus tests).
CAMBIO ÚNICO: la ruta de los `include_bytes!`. Desde `crates/core/src/icons/` a `assets/icons/`
son 3 niveles: `../../../assets/icons/`. La macro queda:

```rust
macro_rules! png {
    ($set:literal, $name:literal) => {
        include_bytes!(concat!("../../../assets/icons/", $set, "/", $name, ".png")) as &'static [u8]
    };
}
```

Cabecera del archivo:
```rust
// Naygo — tabla de assets embebidos: (IconSet, IconKey) → bytes PNG. Pura, reusable por toda UI.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
use crate::config::IconSet;
use crate::icon_kind::{ActionIcon, FileCategory, IconKey};
```
(El resto del cuerpo idéntico al de egui, pero con `crate::` en vez de `naygo_core::`.)

- [ ] **Step 3: Registrar el módulo**

En `crates/core/src/lib.rs` agregar `pub mod icons;` (orden alfabético entre los `pub mod`).

- [ ] **Step 4: Re-exportar desde egui (no romper la capa egui hasta 6F)**

Reemplazar TODO el cuerpo de `crates/ui/src/icons/assets.rs` por:
```rust
// Naygo — assets de íconos: ahora viven en naygo-core::icons (compartidos con la UI Slint).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
pub use naygo_core::icons::{all_keys, bytes_for, file_name};
```

- [ ] **Step 5: Verificar que los tests viajaron y pasan**

Run: `cargo test -p naygo-core icons::`. Esperado: PASS (los 3 tests:
`cada_clave_tiene_bytes_no_vacios`, `clave_sin_asset_cae_a_unknown`,
`cada_clave_tiene_su_propio_asset_no_solo_el_fallback`).

- [ ] **Step 6: Gate + commit**

```bash
cargo build --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/core/src/icons/mod.rs crates/core/src/lib.rs crates/ui/src/icons/assets.rs
git commit -F - <<'EOF'
refactor(core): mover los assets de íconos (PNGs de los 3 sets) a naygo-core::icons

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 2: `IconCache` en ui-slint (PNG → slint::Image cacheado)

**Files:**
- Create: `crates/ui-slint/src/icons.rs`
- Modify: `crates/ui-slint/src/main.rs` (`mod icons;`)

- [ ] **Step 1: Orientar**

Run: `graphify query "preview image decode slint Image from_rgba8 SharedPixelBuffer crate image rgba"`.

- [ ] **Step 2: Crear `icons.rs`**

```rust
// Naygo — cache de íconos: PNG embebido → slint::Image, decodificado UNA sola vez. Clave del
// rendimiento: el render por software no re-rasteriza; clonar un slint::Image comparte el buffer.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
use naygo_core::config::IconSet;
use naygo_core::icon_kind::IconKey;
use slint::{Image, SharedPixelBuffer};
use std::collections::HashMap;

/// Decodifica y cachea los íconos a `slint::Image` por (set, clave). El set activo lo fija la UI.
pub struct IconCache {
    map: HashMap<(IconSet, IconKey), Image>,
    /// Set activo (el que devuelve `get`). La UI lo cambia con `set_active`.
    active: IconSet,
}

impl IconCache {
    pub fn new(active: IconSet) -> IconCache {
        IconCache { map: HashMap::new(), active }
    }

    /// Cambia el set activo (al cambiarlo en Configuración). No borra el cache: las claves del
    /// set nuevo se decodifican on-demand; las viejas quedan (memoria despreciable, 28 PNGs).
    pub fn set_active(&mut self, set: IconSet) {
        self.active = set;
    }

    pub fn active(&self) -> IconSet {
        self.active
    }

    /// El `slint::Image` del ícono `key` en el set activo. Lo decodifica si falta y lo cachea.
    /// Si el PNG es ilegible, devuelve una imagen vacía (no crashea).
    pub fn get(&mut self, key: IconKey) -> Image {
        let set = self.active;
        if let Some(img) = self.map.get(&(set, key)) {
            return img.clone();
        }
        let img = decode(naygo_core::icons::bytes_for(set, key));
        self.map.insert((set, key), img.clone());
        img
    }
}

/// Decodifica bytes PNG a un `slint::Image` RGBA. Imagen vacía si falla.
fn decode(bytes: &[u8]) -> Image {
    match image::load_from_memory_with_format(bytes, image::ImageFormat::Png) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            let buf = SharedPixelBuffer::clone_from_slice(rgba.as_raw(), w, h);
            Image::from_rgba8(buf)
        }
        Err(_) => Image::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::icon_kind::IconKey;

    #[test]
    fn get_cachea_la_misma_clave() {
        let mut c = IconCache::new(IconSet::Flat);
        let a = c.get(IconKey::Folder);
        let b = c.get(IconKey::Folder);
        // El size es estable y no vacío (el PNG de carpeta existe).
        assert_eq!(a.size(), b.size());
        assert!(a.size().width > 0);
    }
}
```

- [ ] **Step 3: Registrar el módulo**

En `main.rs` agregar `mod icons;` junto a los demás `mod`.

- [ ] **Step 4: Test**

Run: `cargo test -p naygo-ui-slint icons::`. Esperado: PASS.

- [ ] **Step 5: Gate + commit**

```bash
git add crates/ui-slint/src/icons.rs crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(slint): IconCache — PNG embebido → slint::Image cacheado (6A)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 3: WorkspaceCtrl posee el IconCache; las filas traen su ícono

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (campo `icons: IconCache`)
- Modify: `crates/ui-slint/src/bridge.rs` (`PlainRow.icon`, resolver por entry)
- Modify: `crates/ui-slint/ui/types.slint` (`RowData.icon: image`)
- Modify: `crates/ui-slint/src/main.rs` (`to_row_data` mapea el icono)
- Modify: `crates/ui-slint/ui/file-panel.slint` (pintar `Image` en vez del emoji)

- [ ] **Step 1: Orientar**

Run: `graphify query "WorkspaceCtrl new_in icons rows_of rows_from_view PlainRow RowData to_row_data file-panel emoji folder"`.

- [ ] **Step 2: Campo `icons` en WorkspaceCtrl**

Agregar `pub icons: crate::icons::IconCache` al struct; init en `new_in` con
`crate::icons::IconCache::new(/* del config */ )`. Como `ConfigCtrl` ya está construido en
`new_in`, usar `IconCache::new(config_dir...)` NO — usar el set de settings: tras crear `config`,
`IconCache::new(config.settings.icon_set)`. Ajustar el orden en `new_in` para tener `config`
antes de `icons`.

- [ ] **Step 3: `PlainRow.icon` + resolver en `rows_from_view`**

En `bridge.rs`, `PlainRow` gana `pub icon: slint::Image`. `rows_from_view` recibe un closure
`icon_of: &mut dyn FnMut(&Entry) -> slint::Image` y lo llama por fila:
```rust
icon: icon_of(e),
```
(Como `IconCache::get` es `&mut`, el closure es `FnMut`. `rows_from_view` ya recibe varios
closures; agregar este al final.)

- [ ] **Step 4: `RowData.icon` en Slint**

En `types.slint`, `RowData` gana `icon: image,`.

- [ ] **Step 5: `rows_of` pasa el resolver**

En `WorkspaceCtrl::rows_of`, antes de llamar `rows_from_view`, no se puede pedir prestado
`self.icons` mutable y `self` inmutable a la vez. Solución: `rows_of` toma `&mut self`
(cambiar la firma) y captura `&mut self.icons` en el closure:
```rust
pub fn rows_of(&mut self, id: PaneId, highlight_secs: u64, now: std::time::Instant) -> Vec<PlainRow> {
    let date_format = self.config.settings.date_format;
    let tz = naygo_platform::time::local_utc_offset_secs();
    // Tomar los datos inmutables ANTES del préstamo mutable de icons.
    let cut: Vec<...> // si hace falta, precomputar is_cut/is_fresh en vectores
    // Reestructurar: extraer la lista de entries + flags a structs planas, luego mapear con icons.
    ...
}
```
NOTA: si el doble préstamo (self.ops.is_cut + self.icons + self.watchers) complica, precomputar
en `rows_of` un `Vec<(Entry-ref-data, is_cut, is_fresh)>` y mapear los iconos en una segunda
pasada con `self.icons.get(icon_key_for(e))`. Reordenar para que el préstamo mutable de `icons`
sea el último. Mantener `rows_from_view` recibiendo los iconos ya resueltos (un `Vec<Image>`
paralelo) en vez de un closure, si eso evita el conflicto de préstamos.

- [ ] **Step 6: Mapear en `to_row_data` + pintar en Slint**

`to_row_data` copia `icon: r.icon`. En `file-panel.slint`, reemplazar el `Text { text: (row.is-dir
? "📁 " : "") + row.name; }` por una fila con `Image { source: row.icon; width: 16px; height:
16px; }` + el `Text` del nombre al lado.

- [ ] **Step 7: Actualizar callers de `rows_of`/`rows_from_view`**

`sync_rows` en main.rs llama `c.rows_of(...)` con `c = ctrl.borrow()` inmutable → cambiar a
`ctrl.borrow_mut()` (rows_of ahora es `&mut`). Los tests de `rows_from_view` en bridge.rs:
pasar el `Vec<Image>` de iconos (o `slint::Image::default()` para cada fila en el test).

- [ ] **Step 8: Build + verificación viva**

Build, lanzar, capturar con PrintWindow: las filas muestran íconos de color por tipo (carpeta
amarilla, .pdf, .png, .rs, etc.) en vez del emoji.

- [ ] **Step 9: Gate + commit**

```bash
git add crates/ui-slint/src/workspace_ctrl.rs crates/ui-slint/src/bridge.rs crates/ui-slint/ui/types.slint crates/ui-slint/src/main.rs crates/ui-slint/ui/file-panel.slint
git commit -F - <<'EOF'
feat(slint): íconos de color por tipo de archivo en las filas (cacheados) (6A)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 4: selector de set de íconos en Configuración

**Files:**
- Modify: `crates/ui-slint/src/config_ctrl.rs` (`set_icon_set`)
- Modify: `crates/ui-slint/ui/types.slint` (`SettingsVm.icon-set: int`)
- Modify: `crates/ui-slint/ui/config-window.slint` (combo en Apariencia)
- Modify: `crates/ui-slint/src/main.rs` (wiring + refrescar el cache)
- Modify: i18n (clave `slint.cfg.icon_set` + `Tr.cfg-icon-set`)

- [ ] **Step 1: `set_icon_set` en ConfigCtrl**

```rust
/// Set de íconos: 0=Flat 1=Fluent 2=Mono. Persiste.
pub fn set_icon_set(&mut self, idx: i32) {
    use naygo_core::config::IconSet::*;
    self.settings.icon_set = match idx { 1 => Fluent, 2 => Mono, _ => Flat };
    self.save();
}
```
(OJO: `Settings.icon_set` es `IconSet` enum, NO `String` — verificar el tipo real; si es String,
adaptar.)

- [ ] **Step 2: SettingsVm + combo + wiring + i18n**

`SettingsVm` gana `icon-set: int`; `build_settings_vm` lo llena (match IconSet→int). Combo en
Apariencia con `[Flat, Fluent, Monocromo]` (reusar claves existentes `settings.icons.flat/fluent/
mono` del catálogo). Callback `set-date-format`-style → `cfg-set-icon-set` → `on_cfg_set_icon_set`
en main.rs que llama `config.set_icon_set(i)` Y `ctrl.borrow_mut().icons.set_active(nuevo)` + refresh.

- [ ] **Step 3: Build + verificación viva**

Cambiar el set en Configuración → las filas repintan con el set nuevo al instante.

- [ ] **Step 4: Gate + commit**

```bash
git commit -F - <<'EOF'
feat(slint): selector de set de íconos (Flat/Fluent/Mono) en Configuración (6A)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 6B — Toolbar con íconos + pulido liviano

### Task 5: toolbar con íconos de acción

**Files:**
- Modify: `crates/ui-slint/ui/app-window.slint` (ToolBtn con Image)
- Modify: `crates/ui-slint/ui/types.slint` (si hace falta un struct de botón con icon)
- Modify: `crates/ui-slint/src/main.rs` (proveer los Image de acción a la UI)

- [ ] **Step 1: Orientar**

Run: `graphify query "app-window ToolBtn toolbar label tip icon ActionIcon Up Panel Swap settings"`.

- [ ] **Step 2: Exponer los íconos de acción a Slint**

Como los botones son fijos (Subir/+/Panel/Swap/Clonar/Tabs/Config), agregar propiedades `image`
al AppWindow: `in property <image> ic-up; ic-add; ic-panel; ...` y setearlas en main.rs con
`ctrl.borrow_mut().icons.get(IconKey::Action(ActionIcon::Up))`, etc. (una vez al arrancar y al
cambiar de set).

- [ ] **Step 3: ToolBtn muestra ícono + texto (respeta icon_only)**

`ToolBtn` gana `in property <image> icon;`. Pinta `Image { source: icon; width:16px; height:16px;}`
+ el `Text` del label (oculto si `Settings.icon_only`). Mantener el tooltip.

- [ ] **Step 4: Build + verificación viva**

La toolbar muestra íconos (con texto al lado por defecto). Tooltips OK.

- [ ] **Step 5: Gate + commit**

```bash
git commit -F - <<'EOF'
feat(slint): toolbar con íconos de acción (cacheados) + tooltips (6B)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 6: íconos en árbol / favoritos / disco + pulido de espaciado

**Files:**
- Modify: `crates/ui-slint/src/bridge.rs` (TreeRow/NavRow ganan icono o usan el de tipo)
- Modify: `crates/ui-slint/ui/{tree,favorites}-panel.slint`, `file-panel.slint`

- [ ] **Step 1: Orientar**

Run: `graphify query "tree-panel favorites NavRow TreeRow drive folder emoji icon spacing row height"`.

- [ ] **Step 2: Íconos en árbol y favoritos**

`TreeRow` gana `icon: image` (Drive→IconKey::Drive, carpeta→Folder). Favoritos/recientes usan
Folder. Pintar `Image` en sus .slint en vez de 💾📁.

- [ ] **Step 3: Pulido de espaciado (sin animaciones)**

Ajustar padding de filas (p. ej. 24px de alto, gap del ícono), padding de la toolbar, el header.
Usar tokens de `Theme`. NADA de animaciones.

- [ ] **Step 4: Build + verificación viva**

Árbol con íconos de disco/carpeta; favoritos con íconos; aspecto más limpio.

- [ ] **Step 5: Gate + commit**

```bash
git commit -F - <<'EOF'
feat(slint): íconos en árbol/favoritos/disco + pulido de espaciado liviano (6B)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 6C — Columnas dinámicas + formato inteligente

### Task 7: `SizeFormat` configurable en core

**Files:**
- Modify: `crates/core/src/format.rs`
- Modify: `crates/core/src/config/mod.rs` (`Settings.size_format`)

- [ ] **Step 1: Orientar**

Run: `graphify query "format human_size SizeFormat Settings size_format bytes kb mb auto"`.

- [ ] **Step 2: `SizeFormat` + `format_size` (TDD)**

En `format.rs`:
```rust
/// Cómo se muestra el tamaño en la columna. Configurable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SizeFormat {
    /// Unidad automática legible (default): 512 B / 1.5 KB / 20.2 MB.
    #[default]
    Auto,
    /// Siempre en bytes, con separadores de miles: "21.250.048".
    Bytes,
    /// Siempre en KB (1 decimal): "20.2 KB".
    Kb,
    /// Siempre en MB (1 decimal): "20.2 MB".
    Mb,
}

/// Formatea `bytes` según `fmt`.
pub fn format_size(bytes: u64, fmt: SizeFormat) -> String {
    match fmt {
        SizeFormat::Auto => human_size(bytes),
        SizeFormat::Bytes => {
            // Separador de miles con punto (convención es-CL).
            let s = bytes.to_string();
            let mut out = String::new();
            for (i, c) in s.chars().rev().enumerate() {
                if i > 0 && i % 3 == 0 { out.push('.'); }
                out.push(c);
            }
            out.chars().rev().collect()
        }
        SizeFormat::Kb => format!("{:.1} KB", bytes as f64 / 1024.0),
        SizeFormat::Mb => format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)),
    }
}
```
Test:
```rust
#[test]
fn size_format_variantes() {
    assert_eq!(format_size(512, SizeFormat::Auto), "512 B");
    assert_eq!(format_size(1536, SizeFormat::Auto), "1.5 KB");
    assert_eq!(format_size(21_250_048, SizeFormat::Bytes), "21.250.048");
    assert_eq!(format_size(1024 * 1024, SizeFormat::Kb), "1024.0 KB");
    assert_eq!(format_size(1024 * 1024, SizeFormat::Mb), "1.0 MB");
}
```
Run: `cargo test -p naygo-core size_format_variantes`. Esperado: PASS.

- [ ] **Step 3: Campo en Settings**

`Settings.size_format: SizeFormat` con `#[serde(default)]`; agregarlo a los 2 literales (Default
y el test). Build workspace para confirmar que egui (que usa `..Default`) no rompe.

- [ ] **Step 4: Gate + commit**

```bash
git commit -F - <<'EOF'
feat(core): SizeFormat configurable (Auto/Bytes/KB/MB) + format_size (6C)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 8: render dinámico de columnas + alineación + selector

**Files:**
- Modify: `crates/ui-slint/src/bridge.rs` (celdas por columna del TableState)
- Modify: `crates/ui-slint/ui/types.slint` (`CellVm`/`ColumnVm`)
- Modify: `crates/ui-slint/ui/file-panel.slint` (header + filas iteran columnas)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (columnas del panel + toggle/move/width)
- Modify: `crates/ui-slint/src/main.rs` (wiring)

- [ ] **Step 1: Orientar**

Run: `graphify query "TableState visible_columns ColumnSpec ColumnKind file-panel header HeaderCell columns width to_row_data cells"`.

- [ ] **Step 2: Modelo de columnas para Slint**

`types.slint`: `ColumnVm { kind: int, label: string, width: length, align-right: bool }` y la fila
`RowData` lleva `cells: [string]` (en el ORDEN de las columnas visibles) además del `icon` y `name`.
WorkspaceCtrl expone `columns_of(id) -> Vec<ColumnVm-data>` (de `table.visible_columns()`, con su
label i18n y alineación: Size→derecha, resto izquierda) y `rows_of` produce `cells` en ese orden
(Name ya va con su ícono aparte; las demás celdas son strings: Extension/Size[fmt]/Modified/Created).

- [ ] **Step 3: file-panel.slint dinámico**

El header itera `columns` (HeaderCell por columna, clic = ordenar por esa columna, arrastrar el
borde = resize → `column-resize(kind, w)`, arrastrar el header = reordenar → `column-move(from,to)`).
Las filas iteran: 1ª celda = ícono+nombre; resto = `cells[i]` con la alineación de la columna.

- [ ] **Step 4: Menú de columnas (agregar/quitar)**

Un botón "Columnas…" (o en el menú contextual del header) abre una lista de toggles
Name/Extension/Size/Modified/Created (Name deshabilitado). `column-toggle(kind)` →
`TableState::toggle_visible`. Persiste (el table ya se serializa por panel).

- [ ] **Step 5: Wiring en WorkspaceCtrl/main**

`WorkspaceCtrl`: `column_toggle(id,kind)`, `column_move(id,from,to)`, `column_resize(id,kind,w)`
sobre `table` del panel (crear `table` default si `None`). Tras cada cambio, re-ordenar/re-pintar.
`size_format` de settings alimenta el formateo de la celda Size.

- [ ] **Step 6: Build + verificación viva**

Agregar la columna "Creado", quitar "Extensión", reordenar arrastrando, cambiar el formato de
tamaño → todo se refleja y persiste al reabrir.

- [ ] **Step 7: Gate + commit**

```bash
git commit -F - <<'EOF'
feat(slint): columnas dinámicas (agregar/quitar/reordenar/resize) + tamaño configurable (6C)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 6D — Rename inline + en cadena

### Task 9: rangos del ciclo F2 en core (puro)

**Files:**
- Create: `crates/core/src/rename.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod rename;`)

- [ ] **Step 1: `rename_selection` (TDD)**

```rust
// Naygo — selección del rename inline (ciclo F2): qué parte del nombre se selecciona. Puro.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

/// Rango `(inicio, fin)` en CHARS a seleccionar en el editor de rename, según la etapa del
/// ciclo F2: 0 = nombre sin extensión, 1 = solo extensión, 2+ = todo. Sin extensión válida
/// (carpetas, dotfiles tipo ".gitignore") cualquier etapa selecciona todo.
pub fn rename_selection(text: &str, stage: u8) -> (usize, usize) {
    let total = text.chars().count();
    let split = text
        .rsplit_once('.')
        .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty());
    match (stage, split) {
        (0, Some((stem, _))) => (0, stem.chars().count()),
        (1, Some((stem, _))) => (stem.chars().count() + 1, total),
        _ => (0, total),
    }
}
```
Test:
```rust
#[cfg(test)]
mod tests {
    use super::rename_selection;
    #[test]
    fn ciclo_nombre_ext_todo() {
        assert_eq!(rename_selection("foto.png", 0), (0, 4));   // "foto"
        assert_eq!(rename_selection("foto.png", 1), (5, 8));   // "png"
        assert_eq!(rename_selection("foto.png", 2), (0, 8));   // todo
        // Sin extensión válida → siempre todo.
        assert_eq!(rename_selection("carpeta", 0), (0, 7));
        assert_eq!(rename_selection(".gitignore", 0), (0, 10));
    }
}
```
Run: `cargo test -p naygo-core rename::`. Esperado: PASS.

- [ ] **Step 2: Registrar + gate + commit**

`pub mod rename;` en lib.rs.
```bash
git commit -F - <<'EOF'
feat(core): rename_selection — rangos del ciclo F2 (nombre/ext/todo), puro (6D)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 10: rename inline + en cadena en la UI Slint

**Files:**
- Modify: `crates/ui-slint/ui/file-panel.slint` (editor inline en la celda Name)
- Modify: `crates/ui-slint/ui/app-window.slint` (estado de rename + callbacks)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (commit rename + avanzar foco)
- Modify: `crates/ui-slint/src/main.rs` (wiring; F2 abre, ↑/↓ encadena)

- [ ] **Step 1: Orientar**

Run: `graphify query "on_key Rename op_rename pending_dialog NameInput file_pane focused move_focus ops actions rename start_op"`.

- [ ] **Step 2: Estado de rename (AppWindow)**

`app-window.slint`: `in-out property <int> rename-pane: -1; in property <int> rename-pos; in
property <string> rename-text; in property <int> rename-stage;` La fila en `rename-pos` del panel
`rename-pane` muestra un `LineEdit` en la celda Name en vez del texto.

- [ ] **Step 3: Abrir con F2 / menú**

`WorkspaceCtrl::on_key` (acción `Rename`) y el menú contextual → en vez de abrir el modal
`NameInput`, setear `rename_requested = Some((pane, pos, stage=0))`. La UI lo consume (como el
`take_edit_path_request` de la path-bar): abre el editor con el nombre y aplica
`rename_selection(name, 0)` para el rango inicial. (Mantener el modal NameInput SOLO para
"nuevo archivo/carpeta", no para renombrar.)

- [ ] **Step 4: Commit + encadenar**

`WorkspaceCtrl::rename_commit(pane, pos, new_name) -> bool`: arma `ops::actions::rename(path,
new_name)` y `ops.start_op(...)`. `rename_chain(pane, pos, new_name, dir: i32) -> Option<usize>`:
confirma el actual y devuelve la nueva posición (pos+dir, clamp a la vista); la UI reabre el editor
ahí con stage=0 (nombre sin extensión, decisión de Nicolás). Enter sin moverse confirma y cierra.
Esc cancela.

- [ ] **Step 5: Slint del editor**

El `LineEdit` de la celda Name: `accepted` → `rename-commit`; `key-pressed` Esc → cancelar; ↑/↓ →
`rename-chain(dir)`. Tras commit/chain, la UI aplica la selección con `rename_selection` (expuesto
por un helper de WorkspaceCtrl `rename_selection_for(name, stage) -> (i32,i32)` que envuelve el de
core) — Slint no setea selección de texto programática fácil; alternativa: pre-seleccionar el
LineEdit con `select-all` y, si la etapa es "solo nombre", confiar en que el usuario reescribe (o
usar el rango si la API de LineEdit lo permite; si no, documentar la limitación y seleccionar todo).

- [ ] **Step 6: Test del controlador**

```rust
#[test]
fn rename_chain_avanza_y_confirma() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    std::fs::write(tmp.path().join("b.txt"), b"y").unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    // Renombrar la fila 0 y encadenar hacia abajo → devuelve pos 1.
    let next = c.rename_chain(id, 0, "a2.txt".into(), 1);
    assert_eq!(next, Some(1));
    assert!(drain(&mut c)); // la op de rename corre
    assert!(c.rows_of(id, 8, std::time::Instant::now()).iter().any(|r| r.name == "a2.txt"));
}
```
Run: `cargo test -p naygo-ui-slint rename_chain_avanza_y_confirma`. Esperado: PASS.

- [ ] **Step 7: Build + verificación viva**

F2 sobre un archivo abre el editor; Enter renombra; con el editor abierto, ↓ confirma y abre el
siguiente. (Si la preselección por etapa no es factible en LineEdit, dejar constancia.)

- [ ] **Step 8: Gate + commit**

```bash
git commit -F - <<'EOF'
feat(slint): rename inline (ciclo F2) + rename en cadena con ↑/↓ (6D)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 6E — Packs de usuario con íconos

### Task 11: import/export de temas CON íconos + carga de íconos de usuario

**Files:**
- Modify: `crates/ui-slint/src/packs.rs` (export_theme/import_zip con icons/)
- Modify: `crates/core/src/icons/mod.rs` (buscar primero en `<config_dir>/icons/`)
- Modify: `crates/ui-slint/src/icons.rs` (IconCache consulta el dir de usuario)

- [ ] **Step 1: Orientar**

Run: `graphify query "packs export_theme import_zip themes icons config_dir bytes_for IconCache user set"`.

- [ ] **Step 2: `bytes_for` con override de usuario**

En `core::icons`, agregar `bytes_for_dir(config_dir, set, key) -> Vec<u8>`: si existe
`<config_dir>/icons/<set_name>/<file_name(key)>.png` lo lee; si no, cae a los bytes embebidos
(`bytes_for`). `set_name` = "flat"/"fluent"/"mono". Mantener `bytes_for` (embebidos) para los tests.
(El `IconCache::decode` pasa por `bytes_for_dir` con el `config_dir`.)

- [ ] **Step 3: `IconCache` usa el dir de usuario**

`IconCache::new(active, config_dir: PathBuf)` guarda el dir; `decode` para una clave usa
`bytes_for_dir(&self.config_dir, set, key)`. Ajustar el caller en WorkspaceCtrl
(`IconCache::new(config.settings.icon_set, config.config_dir.clone())`).

- [ ] **Step 4: export_theme incluye icons/**

`packs::export_theme`: además del `themes/<id>.json`, si existe `<config_dir>/icons/<id>/` mete sus
PNGs como `icons/<id>/<name>.png` en el zip. `import_zip`: una entrada `icons/<id>/<name>.png` se
extrae a `<config_dir>/icons/<id>/` (validar que es PNG por la firma, rechazar "..").

- [ ] **Step 5: Tests**

Round-trip: crear `<src>/themes/x.json` + `<src>/icons/x/folder.png` (un PNG mínimo),
`export_theme(src, "x", zip)`, `import_zip(dst, zip)` → en `dst` quedan ambos. Validar que un
`icons/x/evil.png` con bytes no-PNG se rechaza.

Run: `cargo test -p naygo-ui-slint packs::`. Esperado: PASS.

- [ ] **Step 6: Gate + commit**

```bash
git commit -F - <<'EOF'
feat(slint): packs de tema con íconos propios — export/import + carga de íconos de usuario (6E)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Sub-fase 6F — Paridad restante + retiro de egui + cierre

### Task 12: inventario de paridad + gaps

**Files:** (solo lectura + notas)

- [ ] **Step 1: Diff contra el contrato**

Leer `docs/migracion-slint/CONTRATO-PARIDAD-FUNCIONAL.md` y marcar qué ítems quedan sin
implementar en Slint (rubber-band/selección por rectángulo, Ctrl+arrastre aditivo, reordenar
columnas [hecho en 6C], cualquier atajo/gesto). Listar los gaps reales. (No commit; alimenta las
tareas siguientes.)

### Task 13: rubber-band (selección por rectángulo) + Ctrl+arrastre

**Files:**
- Modify: `crates/ui-slint/ui/file-panel.slint` (gesto de arrastre en zona vacía)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (selección por rango de filas)

- [ ] **Step 1: Orientar**

Run: `graphify query "file_pane select range rubber band rectangle selection drag empty area shift ctrl additive"`.

- [ ] **Step 2: Selección por rango (core ya lo tiene vía Shift)**

Reusar `FilePaneState` para seleccionar un rango de posiciones (el Shift+clic ya lo hace).
WorkspaceCtrl: `select_range(pane, from_pos, to_pos, additive)`.

- [ ] **Step 3: Gesto rubber-band en Slint**

En el área vacía del ListView (o un overlay), un arrastre con botón izquierdo pinta un rectángulo
(pointer-event down→move→up) y al soltar selecciona las filas cuyo rect intersecta. Pintar el
rectángulo translúcido (Theme.selection-bg con alpha). Ctrl mantiene la selección previa (aditivo).
NOTA DE RIESGO: el ListView/Flickable puede competir por el gesto; si choca, acotar el rubber-band
a la zona del header/vacía o documentar el fallback.

- [ ] **Step 4: Build + verificación viva + commit**

```bash
git commit -F - <<'EOF'
feat(slint): selección por rectángulo (rubber-band) + Ctrl+arrastre aditivo (6F)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 14: retiro de egui — el binario hereda `naygo`

**Files:**
- Modify: `Cargo.toml` (workspace: quitar `crates/ui` de members)
- Modify: `crates/ui-slint/Cargo.toml` (`[[bin]] name = "naygo"`)
- Delete: `crates/ui/` (toda la carpeta de la capa egui)
- Modify: dist scripts / `dist/` (apuntar al binario `naygo`)

- [ ] **Step 1: Orientar**

Run: `graphify query "workspace members crates ui egui bin naygo-slint name binary dist"`.

- [ ] **Step 2: Renombrar el binario**

En `crates/ui-slint/Cargo.toml`, `[[bin]] name = "naygo"` (era `naygo-slint`). Ajustar los scripts/
dist que referencian `naygo-slint.exe`.

- [ ] **Step 3: Quitar egui del workspace**

En el `Cargo.toml` raíz, quitar `"crates/ui"` de `members`. Confirmar que nada en core/platform/
ui-slint depende de `naygo-ui`. Borrar `crates/ui/`.

- [ ] **Step 4: Gate completo (sin egui)**

Run: `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets
-- -D warnings && cargo fmt --all -- --check`. Esperado: todo PASS, sin `crates/ui`.

- [ ] **Step 5: Commit**

```bash
git rm -r crates/ui
git add Cargo.toml crates/ui-slint/Cargo.toml
git commit -F - <<'EOF'
refactor: retirar la capa egui (crates/ui) — el binario Slint hereda el nombre `naygo` (6F)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 15: distribución + verificación integral + push

**Files:**
- Modify: `dist/` + scripts; `README.md` si menciona el binario.

- [ ] **Step 1: Build release + dist**

`cargo build -p naygo-ui-slint --release` → `target/release/naygo.exe`. Copiar a `dist/`.
Confirmar CRT estático/portable (ver [[project-distribucion-naygo]]).

- [ ] **Step 2: Verificación integral en vivo** (capturando):
  íconos de color por tipo; cambiar set de íconos; columnas (agregar/quitar/reordenar/resize);
  formato de tamaño; rename inline + en cadena con ↑/↓; pack de tema con íconos (export+import);
  rubber-band. + el reposo a 0% CPU.

- [ ] **Step 3: memoria + graphify + push**

Actualizar `memory/project-migracion-slint.md` (F6 completa, migración terminada); `graphify
update .`; `git push origin main`; verificar sync `0 0`.

- [ ] **Step 4: avisar a Nicolás** — migración egui→Slint COMPLETA; pedir prueba de rendimiento +
  visual en la VM; ofrecer el siguiente paso (instalador, o capas posteriores del backlog).

---

## Self-review (cobertura del spec)

- 6A íconos de color cacheados (mover assets a core + IconCache + filas + selector) → Tasks 1–4. ✓
- 6B toolbar con íconos + árbol/favoritos/disco + pulido → Tasks 5–6. ✓
- 6C columnas dinámicas (agregar/quitar/reordenar/resize) + tamaño configurable + alineación →
  Tasks 7–8. ✓
- 6D rename inline ciclo F2 + rename en cadena ↑/↓ (nombre sin ext) → Tasks 9–10. ✓
- 6E packs de tema con íconos propios + carga de íconos de usuario → Task 11. ✓
- 6F paridad restante (rubber-band, Ctrl+arrastre) + retiro de egui + binario `naygo` + dist +
  cierre → Tasks 12–15. ✓
- Principio rector (íconos cacheados una vez, sin animaciones, default liviano) → Tasks 2,3,6. ✓
- Testing puro (icons, format_size, rename_selection, packs) + vivo → cada sub-fase. ✓

Sin placeholders: cada step con código o comando. Tipos consistentes: `IconCache::{new,get,
set_active,active}`, `bytes_for`/`bytes_for_dir`/`file_name`/`all_keys`, `SizeFormat`/`format_size`,
`rename_selection`, `rename_commit`/`rename_chain`, `column_toggle`/`column_move`/`column_resize`,
`select_range` — mismos nombres en plan y self-review. Riesgos marcados (doble préstamo de `icons`
en rows_of con estrategia; preselección de LineEdit por etapa con fallback; rubber-band vs
Flickable con fallback).
