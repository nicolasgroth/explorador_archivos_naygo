# Naygo — Fase 2B: Íconos + entrada ".." (plan de implementación)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reemplazar los glifos de texto (`[D]`/`[?]`) del panel de archivos por **íconos reales cacheados en GPU** (3 sets embebidos seleccionables: Flat/Fluent/Mono), agregar la **fila virtual `..`** opcional (estilo Commander), e íconos de unidad en el árbol — sin costo de rendimiento por archivo en el listado.

**Architecture:** `naygo-core` gana `icon_kind` (lógica pura: `Entry`/extensión → `IconKey` semántica vía `HashMap` O(1)) + `IconSet`/`show_parent_entry` en `config`. `naygo-ui` gana `icons` (`IconProvider`: decodifica los PNG embebidos del set activo UNA vez con el crate `image`, los sube a textura egui y los cachea por `IconKey`; `reload` al cambiar set). El file panel dibuja `Image::new(handle).fit_to_exact_size(16px)` por fila + la fila `..` (UI pura, foco separado de los índices a `entries`, type-ahead la ignora). Los assets PNG iniciales se generan programáticamente (formas de color por categoría) y se documentan para reemplazo futuro por packs profesionales.

**Tech Stack:** Rust, `naygo-core`, `eframe`/`egui` 0.34.3, crate `image` 0.25 (MIT/Apache, decodifica PNG → RGBA), `include_bytes!` para embeber assets. Sin `egui_extras` (no existe en 0.34) ni resvg/SVG en runtime.

**Estado de partida (2A, en `main`/rama base):**
- `naygo-core`: `cancel`, `fs_model` (`Entry { name, path, kind: EntryKind, size, modified, hidden }`, `EntryKind::{Directory,File,Other}`, `Entry::is_dir()`), `sort`, `listing`, `workspace` (Workspace, FilePaneState, etc.), `config` (`Settings { version, bar_position: BarPosition, icon_only: bool }`, `BarPosition`, tolerante).
- `naygo-ui`: `app.rs` (`NaygoApp { workspace, dock_state, listings, settings, templates, config_dir, status, typeahead_buf }`), `docking.rs` (`NaygoTabViewer`, `PaneRequest::{NavigateTo, Activate}`), `panes/file_panel.rs` (Grid con `selectable_label`, `kind_glyph` → `[D]`/`[?]`, clone de entries por frame), `panes/tree_panel.rs`, `panes/inspector_panel.rs`, `toolbar.rs` (barra + `settings_button` con menú ⚙), `templates_menu.rs`, `input.rs`, `typeahead.rs`, `dock_translate.rs`.
- egui 0.34.3 API verificada: `ctx.load_texture(name, ColorImage, TextureOptions) -> TextureHandle`; `egui::ColorImage::from_rgba_unmultiplied([w,h], &rgba)` (panics si `w*h*4 != len`); render no-deprecado `ui.add(egui::Image::new(&handle).fit_to_exact_size(egui::vec2(16.0,16.0)))`; `Image::tint(Color32)` existe. `egui::TextureHandle`, `egui::TextureOptions::LINEAR`.

**Prerequisito:** toolchain Rust en PATH. En cada comando PowerShell, prepend `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. Nunca `2>&1` con cargo. Verificar `$LASTEXITCODE`.

**Alcance (qué entra / qué NO):**
- ENTRA: `core::icon_kind` (`IconKey`, `FileCategory`, `DriveKind`, `category_for_extension` O(1), `icon_key_for`), `config` (`IconSet`, `Settings.icon_set`, `Settings.show_parent_entry`), generación de assets PNG propios para 3 sets, `ui::icons` (`IconProvider`: carga/cachea/reload), íconos en el file panel (reemplazan glifos) + fila `..` opcional con foco/type-ahead correctos, ícono de unidad en el árbol, selector de set + toggle fila `..` en ⚙, dependencia `image`.
- NO ENTRA (futuro): íconos reales del Shell de Windows (hueco), detección fina de tipo de unidad, miniaturas, animaciones, i18n (2C), temas/color sets completos (2C — el mono usa tint con un color fijo en 2B), escalado fino de íconos (2C), persistencia del reacomodo del dock (deuda 2A).

---

## Estructura de archivos

```
crates/core/src/
├── lib.rs                  # + re-exports de icon_kind
├── icon_kind.rs            # NUEVO: IconKey, FileCategory, DriveKind, category_for_extension, icon_key_for
├── config/mod.rs           # MODIFICAR: + IconSet; Settings { ..., icon_set, show_parent_entry }
└── ...                     # resto sin cambios

crates/ui/src/
├── icons/
│   ├── mod.rs              # NUEVO: IconProvider (carga PNG embebidos → texturas, reload, texture())
│   └── assets.rs           # NUEVO: tabla IconKey×IconSet → &'static [u8] (include_bytes!)
├── panes/file_panel.rs     # MODIFICAR: dibuja ícono por fila; fila ".."; quita glifos; foco con fila ".."
├── panes/tree_panel.rs     # MODIFICAR: ícono de unidad genérico
├── toolbar.rs              # MODIFICAR: en ⚙, selector de set + toggle fila ".."
├── app.rs                  # MODIFICAR: IconProvider en NaygoApp; reload al cambiar set; pasa provider a paneles
├── docking.rs              # MODIFICAR: el TabViewer pasa &IconProvider y settings a los paneles
└── ...

assets/icons/
├── flat/   fluent/   mono/ # PNG generados (un PNG por IconKey por set)
├── README.md              # cómo se generaron y cómo reemplazar por packs reales
└── LICENSES.md            # atribución (los iniciales son propios/CC0; packs reales al iterar)

crates/ui/build_assets.rs   # (o un binario/script de generación) — genera los PNG iniciales
```

**Por qué así:** `core::icon_kind` es pura y testeable (qué ícono, no cómo se ve). `ui::icons` encapsula GPU/decodificación. El file panel solo dibuja texturas cacheadas. La generación de assets se aísla en un script para poder reemplazarlos sin tocar lógica.

**Decisión técnica fijada:** PNG pre-rasterizados embebidos con `include_bytes!` + crate `image` para decodificar a RGBA → `ColorImage::from_rgba_unmultiplied` → `ctx.load_texture`. NO se usa `egui_extras` (no existe en 0.34) ni resvg/SVG en runtime. Los PNG iniciales los genera un script propio (formas de color); se reemplazan luego por packs profesionales (Flat Color Icons MIT, Fluent Emoji MIT, Lucide ISC / Tabler MIT — verificar al iterar).

---

## Task 1: `core::icon_kind` — claves de ícono y categorías

**Files:**
- Create: `crates/core/src/icon_kind.rs`
- Modify: `crates/core/src/lib.rs`
- Test: módulo `#[cfg(test)]` en `icon_kind.rs`

- [ ] **Step 1: Escribir el módulo con tests (TDD; impl dada)**

Create `crates/core/src/icon_kind.rs`:

```rust
// Naygo — clasificación semántica de íconos (lógica pura, sin GPU ni assets).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Mapea un `Entry` (o una extensión) a una **clave de ícono** semántica
//! (`IconKey`), no a un archivo de imagen. La UI traduce la clave al asset del set
//! activo. Puro y testeable: misma extensión → misma clave; sin tocar disco/GPU.

use crate::fs_model::{Entry, EntryKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Categoría semántica de un archivo, derivada de su extensión.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileCategory {
    Image,
    Video,
    Audio,
    Document,
    Code,
    Archive,
    Executable,
    Model3D,
    Font,
    Generic,
}

/// Tipo de unidad de disco. En 2B solo se usa el genérico; la detección real es
/// de `platform` (fase futura).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DriveKind {
    Fixed,
    Removable,
    Network,
    Optical,
    Unknown,
}

/// Clave semántica de un ícono. La UI la traduce al asset del set activo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconKey {
    /// Carpeta normal.
    Folder,
    /// La fila virtual "..".
    ParentDir,
    /// Archivo de una categoría.
    File(FileCategory),
    /// Unidad de disco.
    Drive(DriveKind),
    /// Fallback genérico (tipo no clasificable).
    Unknown,
}

/// Tabla extensión (minúsculas, sin punto) → categoría. Construida una sola vez.
fn extension_table() -> &'static HashMap<&'static str, FileCategory> {
    static TABLE: OnceLock<HashMap<&'static str, FileCategory>> = OnceLock::new();
    TABLE.get_or_init(|| {
        use FileCategory::*;
        let pairs: &[(&str, FileCategory)] = &[
            // Imagen
            ("png", Image), ("jpg", Image), ("jpeg", Image), ("gif", Image),
            ("bmp", Image), ("webp", Image), ("tiff", Image), ("svg", Image),
            ("ico", Image),
            // Video
            ("mp4", Video), ("mkv", Video), ("avi", Video), ("mov", Video),
            ("webm", Video), ("wmv", Video),
            // Audio
            ("mp3", Audio), ("wav", Audio), ("flac", Audio), ("ogg", Audio),
            ("m4a", Audio), ("aac", Audio),
            // Documento
            ("pdf", Document), ("doc", Document), ("docx", Document),
            ("xls", Document), ("xlsx", Document), ("ppt", Document),
            ("pptx", Document), ("odt", Document), ("rtf", Document),
            ("md", Document),
            // Código
            ("rs", Code), ("py", Code), ("js", Code), ("ts", Code),
            ("c", Code), ("cpp", Code), ("h", Code), ("java", Code),
            ("go", Code), ("rb", Code), ("toml", Code), ("json", Code),
            ("yaml", Code), ("yml", Code), ("xml", Code), ("html", Code),
            ("css", Code), ("sh", Code),
            // Archivo comprimido
            ("zip", Archive), ("rar", Archive), ("7z", Archive),
            ("tar", Archive), ("gz", Archive), ("xz", Archive), ("bz2", Archive),
            // Ejecutable
            ("exe", Executable), ("msi", Executable), ("bat", Executable),
            ("cmd", Executable), ("com", Executable),
            // Modelo 3D
            ("stl", Model3D), ("obj", Model3D), ("3mf", Model3D),
            ("step", Model3D), ("stp", Model3D), ("gcode", Model3D),
            ("fbx", Model3D), ("blend", Model3D),
            // Fuente
            ("ttf", Font), ("otf", Font), ("woff", Font), ("woff2", Font),
        ];
        pairs.iter().copied().collect()
    })
}

/// Categoría de una extensión (case-insensitive). Desconocida → `Generic`.
pub fn category_for_extension(ext: &str) -> FileCategory {
    let lower = ext.to_ascii_lowercase();
    extension_table()
        .get(lower.as_str())
        .copied()
        .unwrap_or(FileCategory::Generic)
}

/// Clave de ícono para un `Entry`.
pub fn icon_key_for(entry: &Entry) -> IconKey {
    match entry.kind {
        EntryKind::Directory => IconKey::Folder,
        EntryKind::File => {
            let ext = entry
                .path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            IconKey::File(category_for_extension(ext))
        }
        EntryKind::Other => IconKey::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_model::EntryKind;
    use std::path::PathBuf;

    fn file(path: &str) -> Entry {
        Entry {
            name: path.into(),
            path: PathBuf::from(path),
            kind: EntryKind::File,
            size: Some(1),
            modified: None,
            hidden: false,
        }
    }

    #[test]
    fn extension_a_categoria() {
        assert_eq!(category_for_extension("stl"), FileCategory::Model3D);
        assert_eq!(category_for_extension("jpg"), FileCategory::Image);
        assert_eq!(category_for_extension("zip"), FileCategory::Archive);
        assert_eq!(category_for_extension("exe"), FileCategory::Executable);
        assert_eq!(category_for_extension("rs"), FileCategory::Code);
    }

    #[test]
    fn extension_es_case_insensitive() {
        assert_eq!(category_for_extension("JPG"), FileCategory::Image);
        assert_eq!(category_for_extension("Stl"), FileCategory::Model3D);
    }

    #[test]
    fn extension_desconocida_es_generic() {
        assert_eq!(category_for_extension("xyzabc"), FileCategory::Generic);
        assert_eq!(category_for_extension(""), FileCategory::Generic);
    }

    #[test]
    fn icon_key_de_carpeta_y_archivo() {
        let dir = Entry {
            name: "docs".into(),
            path: PathBuf::from("C:/docs"),
            kind: EntryKind::Directory,
            size: None,
            modified: None,
            hidden: false,
        };
        assert_eq!(icon_key_for(&dir), IconKey::Folder);
        assert_eq!(
            icon_key_for(&file("modelo.stl")),
            IconKey::File(FileCategory::Model3D)
        );
        assert_eq!(
            icon_key_for(&file("sin_extension")),
            IconKey::File(FileCategory::Generic)
        );
    }

    #[test]
    fn icon_key_de_other_es_unknown() {
        let other = Entry {
            name: "raro".into(),
            path: PathBuf::from("C:/raro"),
            kind: EntryKind::Other,
            size: None,
            modified: None,
            hidden: false,
        };
        assert_eq!(icon_key_for(&other), IconKey::Unknown);
    }
}
```

Modify `crates/core/src/lib.rs` — añadir `pub mod icon_kind;` (orden alfabético, tras `pub mod fs_model;` o donde calce) y, tras los `pub use`:

```rust
pub use icon_kind::{category_for_extension, icon_key_for, DriveKind, FileCategory, IconKey};
```

- [ ] **Step 2: Correr los tests**

Run: `cargo test -p naygo-core icon_kind`
Expected: PASS (5 tests).

Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/icon_kind.rs crates/core/src/lib.rs
git commit -m "feat(core): icon_kind — Entry/extensión → IconKey semántica (mapa O(1))

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `config` — `IconSet` y campos de Settings

**Files:**
- Modify: `crates/core/src/config/mod.rs`
- Test: ampliar `#[cfg(test)]` de `config/mod.rs`

- [ ] **Step 1: Añadir `IconSet` y los campos a `Settings`**

Modify `crates/core/src/config/mod.rs`:

(a) Tras `BarPosition`, añadir:

```rust
/// Qué set de íconos usa la app. Flat es el default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IconSet {
    /// Multicolor plano (default).
    Flat,
    /// Estilo Fluent (Microsoft).
    Fluent,
    /// Monocromo temable (Lucide/Tabler).
    Mono,
}
```

(b) Añadir los dos campos a `Settings` (tras `icon_only`):

```rust
    /// Set de íconos activo.
    pub icon_set: IconSet,
    /// Mostrar la fila virtual ".." al tope del panel de archivos.
    pub show_parent_entry: bool,
```

(c) Actualizar `Default for Settings`:

```rust
impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: CONFIG_VERSION,
            bar_position: BarPosition::Top,
            icon_only: true,
            icon_set: IconSet::Flat,
            show_parent_entry: true,
        }
    }
}
```

- [ ] **Step 2: Actualizar el test de round-trip y añadir uno de default**

En el `#[cfg(test)]` de `config/mod.rs`, el test `settings_round_trip` construye un `Settings` literal — actualizarlo para incluir los campos nuevos:

```rust
    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let s = Settings {
            version: CONFIG_VERSION,
            bar_position: BarPosition::Side,
            icon_only: false,
            icon_set: IconSet::Mono,
            show_parent_entry: false,
        };
        save_settings(dir.path(), &s);
        assert_eq!(load_settings(dir.path()), s);
    }
```

Y añadir:

```rust
    #[test]
    fn settings_default_tiene_iconos_flat_y_fila_padre_on() {
        let s = Settings::default();
        assert_eq!(s.icon_set, IconSet::Flat);
        assert!(s.show_parent_entry);
    }
```

NOTA: el test `settings_version_incompatible_da_default` escribe un JSON literal con `version:999` y SIN los campos nuevos — como su `version` no coincide, se descarta antes de deserializar el resto, así que sigue pasando. El test `settings_corrupto_da_default_sin_panic` también sigue válido. Si algún test de `config` que deserializa un `Settings` completo se rompe por los campos nuevos, actualízalo. (El JSON de `version:999` no deserializa a `Settings` porque faltan campos, pero la rama de versión incompatible lo captura por `version` primero — verifica el orden: `read_json` deserializa primero; si faltan campos `serde` falla → `None` → default. Resultado final: default igual. Confirmar que el test pasa; si no, el comportamiento sigue siendo "default sin panic", ajustar el assert si hace falta.)

- [ ] **Step 2b: Re-export de `IconSet`**

Modify `crates/core/src/lib.rs` — en el `pub use config::{...}` existente, añadir `IconSet`:
```rust
pub use config::{BarPosition, IconSet, Settings};
```

- [ ] **Step 3: Correr los tests**

Run: `cargo test -p naygo-core config`
Expected: PASS (los previos actualizados + el nuevo). Si `settings_version_incompatible_da_default` falla, lee el error: el comportamiento correcto sigue siendo caer a default; ajusta el test para reflejar que un JSON sin los campos nuevos también da default (sin panic).

Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/config/mod.rs crates/core/src/lib.rs
git commit -m "feat(core): config gana IconSet y Settings.{icon_set, show_parent_entry}

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Generar los assets PNG iniciales (set propio simple)

Genera programáticamente un PNG por `IconKey` por set. Set propio = sin dudas de
licencia, reemplazable luego por packs profesionales. Formas geométricas de color
distintas por categoría; el set "mono" en escala de gris/un color.

**Files:**
- Create: `crates/ui/src/bin/gen_icons.rs` (binario generador, se corre una vez)
- Modify: `crates/ui/Cargo.toml` (dep `image` + dev/bin)
- Create: `assets/icons/{flat,fluent,mono}/*.png` (salida del generador)
- Create: `assets/icons/README.md`

- [ ] **Step 1: Añadir la dependencia `image` a `naygo-ui`**

Modify `crates/ui/Cargo.toml` — en `[dependencies]` añadir:
```toml
image = { version = "0.25", default-features = false, features = ["png"] }
```
(`default-features = false` + solo `png` mantiene el peso bajo: solo necesitamos PNG.)

- [ ] **Step 2: Escribir el generador**

Create `crates/ui/src/bin/gen_icons.rs`:

```rust
// Naygo — generador de los PNG iniciales de íconos (set propio, reemplazable).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Genera un PNG 32x32 por IconKey por set bajo assets/icons/{flat,fluent,mono}/.
// Los íconos son formas de color simples por categoría — placeholders de calidad
// suficiente para validar la infraestructura; se reemplazan luego por packs
// profesionales (ver assets/icons/README.md). Correr con:
//   cargo run -p naygo-ui --bin gen_icons

use image::{Rgba, RgbaImage};
use std::path::Path;

const SIZE: u32 = 32;

/// Dibuja un rectángulo relleno (con borde) de un color y lo guarda.
fn make_icon(path: &Path, fill: [u8; 4], mono: bool) {
    let mut img = RgbaImage::new(SIZE, SIZE);
    let color = if mono {
        // En el set mono, usar un gris claro uniforme (se teñirá en la UI).
        [200, 200, 200, 255]
    } else {
        fill
    };
    for y in 0..SIZE {
        for x in 0..SIZE {
            // Margen transparente de 3px; cuerpo relleno; esquinas levemente vacías.
            let border = x < 3 || y < 3 || x >= SIZE - 3 || y >= SIZE - 3;
            let px = if border {
                Rgba([0, 0, 0, 0])
            } else {
                Rgba(color)
            };
            img.put_pixel(x, y, px);
        }
    }
    img.save(path).expect("guardar PNG");
}

/// (nombre de archivo, color RGBA) por cada IconKey. El nombre debe coincidir con
/// el que `assets.rs` espera (ver Tarea 4).
fn icon_specs() -> Vec<(&'static str, [u8; 4])> {
    vec![
        ("folder", [255, 196, 0, 255]),       // ámbar
        ("parent", [180, 180, 180, 255]),     // gris (la fila "..")
        ("file_image", [76, 175, 80, 255]),   // verde
        ("file_video", [156, 39, 176, 255]),  // púrpura
        ("file_audio", [233, 30, 99, 255]),   // rosa
        ("file_document", [33, 150, 243, 255]),// azul
        ("file_code", [0, 188, 212, 255]),    // cian
        ("file_archive", [121, 85, 72, 255]), // marrón
        ("file_executable", [96, 125, 139, 255]), // gris azulado
        ("file_model3d", [255, 87, 34, 255]), // naranja
        ("file_font", [63, 81, 181, 255]),    // índigo
        ("file_generic", [158, 158, 158, 255]),// gris
        ("drive", [69, 90, 100, 255]),        // gris oscuro
        ("unknown", [120, 120, 120, 255]),    // gris medio
    ]
}

fn main() {
    for set in ["flat", "fluent", "mono"] {
        let dir = Path::new("assets/icons").join(set);
        std::fs::create_dir_all(&dir).expect("crear dir");
        let mono = set == "mono";
        for (name, color) in icon_specs() {
            let path = dir.join(format!("{name}.png"));
            make_icon(&path, color, mono);
        }
        println!("generado set: {set}");
    }
    println!("listo. {} íconos x 3 sets.", icon_specs().len());
}
```

NOTA: para Fase 2B los tres sets se ven IGUAL (mismas formas; el "mono" en gris). Es intencional: el objetivo es validar la INFRAESTRUCTURA de set intercambiable; los packs profesionales reales (distintos por set) se sueltan al iterar sin tocar código. La diferencia visible entre sets llega con los assets reales.

- [ ] **Step 3: Correr el generador y verificar que crea los PNG**

Run: `cargo run -p naygo-ui --bin gen_icons`
Expected: imprime "generado set: flat/fluent/mono" y "listo". Verificar que existen p. ej. `assets/icons/flat/folder.png`, `assets/icons/mono/file_model3d.png`, etc. (14 archivos × 3 sets = 42 PNG).

Verify (PowerShell): `(Get-ChildItem assets/icons -Recurse -Filter *.png).Count` → 42.

- [ ] **Step 4: Escribir el README de assets**

Create `assets/icons/README.md`:

```markdown
# Íconos de Naygo

Los PNG aquí son un **set inicial propio** generado por
`crates/ui/src/bin/gen_icons.rs` (formas de color simples por categoría). Son
placeholders de licencia limpia (propios) para validar la infraestructura de
íconos. Los tres sets (flat/fluent/mono) comparten forma por ahora; el "mono" va en
gris para teñirse con el tema.

## Reemplazar por packs profesionales (a futuro)

Colocar un PNG por cada nombre de archivo de abajo en cada carpeta de set. Tamaño
recomendado 32×32 (o 16/32/48). Packs libres recomendados (verificar licencia):

- **Flat Color Icons** (icons8) — MIT — github.com/icons8/flat-color-icons
- **Fluent Emoji** (Microsoft) — MIT — github.com/microsoft/fluentui-emoji
- **Lucide** (ISC) / **Tabler** (MIT) — para el set mono — github.com/lucide-icons/lucide
- **VS Code Icons** — MIT — github.com/vscode-icons/vscode-icons (excelentes por tipo de archivo)

## Nombres de archivo esperados (uno por IconKey)

folder, parent, file_image, file_video, file_audio, file_document, file_code,
file_archive, file_executable, file_model3d, file_font, file_generic, drive, unknown
```

- [ ] **Step 5: Commit (incluye los PNG generados)**

```bash
git add crates/ui/Cargo.toml crates/ui/src/bin/gen_icons.rs assets/icons/
git commit -m "feat(ui): generador y set inicial de íconos PNG (propio, reemplazable)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

NOTA: los PNG SÍ se versionan (son assets del binario). El `.gitignore` no los ignora (solo ignora `/target`).

---

## Task 4: `ui::icons` — `IconProvider` (carga/cachea texturas)

**Files:**
- Create: `crates/ui/src/icons/assets.rs`
- Create: `crates/ui/src/icons/mod.rs`
- Modify: `crates/ui/src/main.rs` (declarar `mod icons;`)
- Test: módulo `#[cfg(test)]` en `assets.rs` (la parte pura: tabla clave→bytes)

- [ ] **Step 1: Tabla `IconKey`×`IconSet` → bytes embebidos**

Create `crates/ui/src/icons/assets.rs`:

```rust
// Naygo — tabla de assets embebidos: (IconSet, IconKey) → bytes PNG.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Embebe los PNG de los tres sets con `include_bytes!`. `bytes_for` da los bytes
//! del ícono para un set y una clave; si la clave no tiene asset propio, cae a
//! `unknown` (genérico), que siempre existe. Función pura y testeable.

use naygo_core::config::IconSet;
use naygo_core::icon_kind::{FileCategory, IconKey};

/// Nombre de archivo (sin extensión) para una `IconKey`. Debe coincidir con lo que
/// generó `gen_icons.rs`.
fn file_name(key: IconKey) -> &'static str {
    match key {
        IconKey::Folder => "folder",
        IconKey::ParentDir => "parent",
        IconKey::Drive(_) => "drive",
        IconKey::Unknown => "unknown",
        IconKey::File(cat) => match cat {
            FileCategory::Image => "file_image",
            FileCategory::Video => "file_video",
            FileCategory::Audio => "file_audio",
            FileCategory::Document => "file_document",
            FileCategory::Code => "file_code",
            FileCategory::Archive => "file_archive",
            FileCategory::Executable => "file_executable",
            FileCategory::Model3D => "file_model3d",
            FileCategory::Font => "file_font",
            FileCategory::Generic => "file_generic",
        },
    }
}

/// Macro: embebe el PNG de un set+nombre. La ruta es relativa a este archivo
/// fuente (crates/ui/src/icons/) → sube a la raíz del repo a assets/icons/.
macro_rules! png {
    ($set:literal, $name:literal) => {
        include_bytes!(concat!(
            "../../../../assets/icons/",
            $set,
            "/",
            $name,
            ".png"
        )) as &'static [u8]
    };
}

/// Bytes PNG para todas las claves de un set, en una tabla nombre→bytes.
fn table_for(set: IconSet) -> &'static [(&'static str, &'static [u8])] {
    match set {
        IconSet::Flat => FLAT,
        IconSet::Fluent => FLUENT,
        IconSet::Mono => MONO,
    }
}

/// Lista de los 14 nombres con sus bytes embebidos, por set.
macro_rules! set_table {
    ($konst:ident, $set:literal) => {
        const $konst: &[(&str, &[u8])] = &[
            ("folder", png!($set, "folder")),
            ("parent", png!($set, "parent")),
            ("file_image", png!($set, "file_image")),
            ("file_video", png!($set, "file_video")),
            ("file_audio", png!($set, "file_audio")),
            ("file_document", png!($set, "file_document")),
            ("file_code", png!($set, "file_code")),
            ("file_archive", png!($set, "file_archive")),
            ("file_executable", png!($set, "file_executable")),
            ("file_model3d", png!($set, "file_model3d")),
            ("file_font", png!($set, "file_font")),
            ("file_generic", png!($set, "file_generic")),
            ("drive", png!($set, "drive")),
            ("unknown", png!($set, "unknown")),
        ];
    };
}

set_table!(FLAT, "flat");
set_table!(FLUENT, "fluent");
set_table!(MONO, "mono");

/// Bytes PNG del ícono para `set`+`key`; cae a `unknown` si no hay asset (no falla).
pub fn bytes_for(set: IconSet, key: IconKey) -> &'static [u8] {
    let name = file_name(key);
    let table = table_for(set);
    table
        .iter()
        .find(|(n, _)| *n == name)
        .or_else(|| table.iter().find(|(n, _)| *n == "unknown"))
        .map(|(_, b)| *b)
        .unwrap_or(&[])
}

/// Todas las claves que la app pinta (para precargar el atlas).
pub fn all_keys() -> Vec<IconKey> {
    use FileCategory::*;
    let mut v = vec![
        IconKey::Folder,
        IconKey::ParentDir,
        IconKey::Drive(naygo_core::icon_kind::DriveKind::Unknown),
        IconKey::Unknown,
    ];
    for cat in [
        Image, Video, Audio, Document, Code, Archive, Executable, Model3D, Font,
        Generic,
    ] {
        v.push(IconKey::File(cat));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_clave_tiene_bytes_no_vacios() {
        for set in [IconSet::Flat, IconSet::Fluent, IconSet::Mono] {
            for key in all_keys() {
                assert!(
                    !bytes_for(set, key).is_empty(),
                    "asset vacío para {:?}/{:?}",
                    set,
                    key
                );
            }
        }
    }

    #[test]
    fn clave_sin_asset_cae_a_unknown() {
        // Drive(Fixed) usa el mismo "drive"; aquí verificamos que una clave mapeada
        // siempre da bytes (no vacío) y que el fallback existe.
        let b = bytes_for(IconSet::Flat, IconKey::Drive(naygo_core::icon_kind::DriveKind::Fixed));
        assert!(!b.is_empty());
    }
}
```

VERIFICACIÓN DE RUTA: el `include_bytes!` usa una ruta relativa al archivo `assets.rs` (`crates/ui/src/icons/assets.rs`). Para llegar a `assets/icons/` en la raíz del repo: `../../../../assets/icons/` (sube icons → src → ui → crates → raíz). CONFIRMA contando los niveles desde `crates/ui/src/icons/assets.rs`: icons(1) src(2) ui(3) crates(4) → raíz. Son 4 `../`. Si el build falla con "file not found", ajusta el número de `../` y reporta el valor correcto.

- [ ] **Step 2: El `IconProvider`**

Create `crates/ui/src/icons/mod.rs`:

```rust
// Naygo — IconProvider: decodifica y cachea los íconos del set activo en GPU.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Carga los PNG embebidos del set activo UNA vez, los sube a textura egui y los
//! cachea por `IconKey`. Pintar una fila = referenciar una textura ya cargada
//! (cero decodificación por frame). Cambiar de set = `reload` (operación única).

pub mod assets;

use naygo_core::config::IconSet;
use naygo_core::icon_kind::IconKey;
use std::collections::HashMap;

/// Dueño de las texturas del set activo.
pub struct IconProvider {
    set: IconSet,
    textures: HashMap<IconKey, egui::TextureHandle>,
    /// Textura de respaldo (la de `Unknown`), garantizada presente.
    fallback: egui::TextureHandle,
}

impl IconProvider {
    /// Carga el set `set` en el contexto `ctx`.
    pub fn new(ctx: &egui::Context, set: IconSet) -> Self {
        let fallback = load_texture(ctx, set, IconKey::Unknown);
        let mut textures = HashMap::new();
        for key in assets::all_keys() {
            textures.insert(key, load_texture(ctx, set, key));
        }
        IconProvider { set, textures, fallback }
    }

    /// El set actualmente cargado.
    pub fn set(&self) -> IconSet {
        self.set
    }

    /// Recarga el atlas para `set` (operación única, no por-frame).
    pub fn reload(&mut self, ctx: &egui::Context, set: IconSet) {
        if set == self.set {
            return;
        }
        *self = IconProvider::new(ctx, set);
    }

    /// Textura cacheada para `key`; cae al fallback si no está.
    pub fn texture(&self, key: IconKey) -> &egui::TextureHandle {
        self.textures.get(&key).unwrap_or(&self.fallback)
    }
}

/// Decodifica el PNG embebido de `set`+`key` y lo sube como textura. Si el PNG es
/// ilegible, sube una textura 1x1 transparente (nunca crashea).
fn load_texture(ctx: &egui::Context, set: IconSet, key: IconKey) -> egui::TextureHandle {
    let bytes = assets::bytes_for(set, key);
    let color_image = decode_png(bytes).unwrap_or_else(|| {
        tracing::warn!("ícono ilegible para {:?}/{:?}; usando vacío", set, key);
        egui::ColorImage::new([1, 1], egui::Color32::TRANSPARENT)
    });
    let name = format!("icon_{:?}_{:?}", set, key);
    ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
}

/// Decodifica bytes PNG a `ColorImage` RGBA, o `None` si falla.
fn decode_png(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}
```

NOTA API (verificada): `egui::ColorImage::new([1,1], Color32::TRANSPARENT)` crea una imagen sólida 1x1 — confirma su firma en egui 0.34.3 (`ColorImage::new(size, fill: Color32)`); si difiere, usa `ColorImage::from_rgba_unmultiplied([1,1], &[0,0,0,0])`. `TextureOptions::LINEAR` existe; si no, usa `TextureOptions::default()`.

Modify `crates/ui/src/main.rs` — añadir `mod icons;` junto a los otros `mod`.

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-ui assets` (los 2 tests de la tabla pura) → PASS.
Run: `cargo build -p naygo-ui` → compila (el provider aún no se usa; puede haber dead-code de `IconProvider`/`reload`/`texture`/`all_keys` — son `pub` en un crate binario, así que clippy puede marcarlos. Si `cargo clippy -p naygo-ui -- -D warnings` se queja de dead-code, NO lo resuelvas aún: la Tarea 5 los consume. Implementa Tareas 4 y 5 juntas antes de clippy estricto, o añade un `#[allow(dead_code)]` temporal a nivel de `icons/mod.rs` con comentario "consumido en Tarea 5". Reporta lo que clippy diga.)

Run: `cargo build -p naygo-ui` debe pasar. Reporta warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/icons/ crates/ui/src/main.rs
git commit -m "feat(ui): IconProvider — decodifica y cachea los íconos del set activo en GPU

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Integrar el `IconProvider` en la app y pintar íconos en el file panel

**Files:**
- Modify: `crates/ui/src/app.rs` (campo `icons: IconProvider`; reload al cambiar set; pasar a paneles)
- Modify: `crates/ui/src/docking.rs` (el viewer lleva `&IconProvider` + `&Settings` y los pasa)
- Modify: `crates/ui/src/panes/file_panel.rs` (dibujar ícono por fila; quitar glifos)

- [ ] **Step 1: Añadir `IconProvider` a `NaygoApp`**

Modify `crates/ui/src/app.rs`:

(a) Import: `use crate::icons::IconProvider;`. (b) Añadir campo a `NaygoApp`:
```rust
    icons: IconProvider,
```
(c) En `NaygoApp::new`, tras construir `settings`, crear el provider con el `ctx`
del `CreationContext` (`_cc.egui_ctx`):
```rust
        let icons = IconProvider::new(&_cc.egui_ctx, settings.icon_set);
```
(renombra `_cc` a `cc` en la firma de `new` ya que ahora se usa).
Inicializa `icons` en el struct literal de `NaygoApp { ... }`.

(d) En `logic()` (o al inicio de `ui()`), recargar el provider si el set de settings
cambió (p. ej. el usuario lo cambió en ⚙). Añadir al comienzo de `ui()` antes de
pintar:
```rust
        // Si el set de íconos cambió en Settings, recargar el atlas (una vez).
        if self.icons.set() != self.settings.icon_set {
            let set = self.settings.icon_set;
            self.icons.reload(ui.ctx(), set);
        }
```

- [ ] **Step 2: Pasar el provider y settings al TabViewer**

Modify `crates/ui/src/docking.rs`:

(a) Añadir campos a `NaygoTabViewer`:
```rust
    pub icons: &'a crate::icons::IconProvider,
    pub show_parent_entry: bool,
```
(b) En el método `ui` del `TabViewer`, pasar `self.icons` y `self.show_parent_entry`
a `file_panel::show`, y `self.icons` a `tree_panel::show`. Las firmas nuevas:
```rust
            Some(PanePurpose::Files) => crate::panes::file_panel::show(
                ui,
                self.workspace,
                id,
                self.pending,
                self.icons,
                self.show_parent_entry,
            ),
            Some(PanePurpose::Tree) => {
                crate::panes::tree_panel::show(ui, self.workspace, self.pending, self.icons)
            }
```

(c) En `app.rs::ui`, al construir el viewer, pasar los campos nuevos:
```rust
            let mut viewer = crate::docking::NaygoTabViewer {
                workspace: &mut self.workspace,
                status: &mut self.status,
                pending: &mut pending,
                icons: &self.icons,
                show_parent_entry: self.settings.show_parent_entry,
            };
```

- [ ] **Step 3: Dibujar el ícono en el file panel y quitar los glifos**

Modify `crates/ui/src/panes/file_panel.rs` — cambiar la firma de `show` y el render
de cada fila. Reemplaza el archivo por:

```rust
// Naygo — panel de archivos: vista Detalle (columnas) con íconos sobre FilePaneState.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta las entradas del panel `id` en columnas, cada una con su ícono de tipo
//! (textura cacheada del set activo). Respeta `show_dirs`. Si `show_parent_entry`
//! y hay padre, pinta una fila ".." arriba (UI pura, no una Entry). Clic
//! selecciona; doble clic / Enter sobre carpeta o ".." navega. No hace I/O.

use crate::docking::PaneRequest;
use crate::icons::IconProvider;
use naygo_core::fs_model::Entry;
use naygo_core::icon_kind::{icon_key_for, IconKey};
use naygo_core::workspace::{PaneId, Workspace};

const ICON_SIZE: f32 = 16.0;

pub fn show(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    id: PaneId,
    pending: &mut Vec<PaneRequest>,
    icons: &IconProvider,
    show_parent_entry: bool,
) {
    let Some(pane) = workspace.pane(id) else {
        return;
    };
    let Some(f) = pane.files.as_ref() else {
        return;
    };
    let focused = f.focused;
    let show_dirs = f.show_dirs;
    let current_dir = f.current_dir.clone();
    let entries: Vec<Entry> = f.entries.clone();

    // ¿Mostrar la fila ".."? Solo si la opción está activa y hay carpeta padre.
    let parent = if show_parent_entry {
        current_dir.parent().map(|p| p.to_path_buf())
    } else {
        None
    };

    ui.horizontal(|ui| {
        ui.monospace(current_dir.display().to_string());
    });
    ui.separator();

    let mut clicked: Option<usize> = None;
    let mut activated: Option<usize> = None;
    let mut parent_activated = false;

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new(("file_grid", id.0))
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Nombre");
                ui.strong("Tamaño");
                ui.strong("Modificado");
                ui.end_row();

                // Fila ".." (si corresponde).
                if parent.is_some() {
                    let resp = icon_row(ui, icons, IconKey::ParentDir, "..", false);
                    if resp.double_clicked() || resp.clicked() {
                        // Un clic en ".." navega directo (no hay nada que seleccionar).
                        parent_activated = true;
                    }
                    ui.label("");
                    ui.label("");
                    ui.end_row();
                }

                for (i, entry) in entries.iter().enumerate() {
                    if !show_dirs && entry.is_dir() {
                        continue;
                    }
                    let selected = focused == Some(i);
                    let key = icon_key_for(entry);
                    let resp = icon_row(ui, icons, key, &entry.name, selected);
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                    if resp.double_clicked() {
                        activated = Some(i);
                    }
                    ui.label(format_size(entry));
                    ui.label(format_modified(entry));
                    ui.end_row();
                }
            });
    });

    // Navegación al padre por la fila "..".
    if parent_activated {
        if let Some(dir) = parent {
            pending.push(PaneRequest::Activate { id });
            pending.push(PaneRequest::NavigateTo { id, dir });
        }
    }
    if let Some(i) = clicked {
        if let Some(f) = workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.focused = Some(i);
        }
        pending.push(PaneRequest::Activate { id });
    }
    if let Some(i) = activated {
        if let Some(entry) = entries.get(i) {
            if entry.is_dir() {
                pending.push(PaneRequest::Activate { id });
                pending.push(PaneRequest::NavigateTo {
                    id,
                    dir: entry.path.clone(),
                });
            }
        }
    }
}

/// Pinta una fila "[ícono] nombre" como un único elemento seleccionable/clicable.
/// Devuelve el `Response` de la fila (clic/doble clic).
fn icon_row(
    ui: &mut egui::Ui,
    icons: &IconProvider,
    key: IconKey,
    name: &str,
    selected: bool,
) -> egui::Response {
    // Un grupo horizontal clicable: ícono + texto, con resaltado si selected.
    let inner = ui.horizontal(|ui| {
        let tex = icons.texture(key);
        ui.add(
            egui::Image::new(tex)
                .fit_to_exact_size(egui::vec2(ICON_SIZE, ICON_SIZE)),
        );
        ui.selectable_label(selected, name)
    });
    inner.inner
}

fn format_size(entry: &Entry) -> String {
    match entry.size {
        Some(bytes) => human_size(bytes),
        None => String::new(),
    }
}

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

/// PROVISIONAL: segundos epoch hasta tener i18n (fase 2C).
fn format_modified(entry: &Entry) -> String {
    use std::time::UNIX_EPOCH;
    match entry.modified.and_then(|t| t.duration_since(UNIX_EPOCH).ok()) {
        Some(d) => format!("{}", d.as_secs()),
        None => String::new(),
    }
}
```

NOTA SOBRE EL FOCO Y TYPE-AHEAD: en 2B la fila `..` se activa por **clic/doble
clic** (mouse). El foco por teclado (↑↓) y el type-ahead siguen operando sobre
`entries` exactamente como en 2A — **NO** se modifica `focused: Option<usize>` para
incluir la fila `..`. Esto cumple el invariante del spec (no romper índices ni
type-ahead): la fila `..` es alcanzable por mouse; subir por teclado sigue siendo
`Backspace`/`Alt+←`/botón del mouse (ya existen). Llevar el foco de teclado a la
fila `..` (que requeriría un `focused_row` distinto de `Option<usize>`) se pospone:
NO es necesario para el valor de 2B y evita tocar la navegación de teclado probada.
Si en el futuro se quiere ↑ hasta `..`, será un cambio acotado en `FilePaneState`.

NOTA API: `egui::Image::new(tex)` acepta `&TextureHandle` (o `(TextureId, Vec2)`).
Verifica que `Image::new(&handle)` compile en 0.34.3; si requiere
`Image::new((handle.id(), size))`, usa esa forma. `fit_to_exact_size` existe.

- [ ] **Step 4: Compilar y verificar**

Run: `cargo build -p naygo-ui` → compila. Warnings verbatim.
Run: `cargo clippy --workspace -- -D warnings` → limpio (ahora el provider se usa; el dead-code de la Tarea 4 desaparece — si quedó un `#[allow(dead_code)]` temporal en `icons/mod.rs`, quítalo).
Run: `cargo test --workspace` → core + ui verdes.
Run: `cargo fmt`.

App-start + verificación manual:
`$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; $p = Start-Process -FilePath "cargo" -ArgumentList "run","-p","naygo-ui" -PassThru -WindowStyle Hidden; Start-Sleep -Seconds 30; if (-not $p.HasExited) { "STILL RUNNING (good)"; $p.Kill() } else { "EXITED code $($p.ExitCode)" }`

Manual (si hay display): cada fila muestra un ícono de color por tipo (carpeta ámbar, .stl naranja, etc.) en vez de `[D]`; la fila `..` aparece arriba con su ícono y al doble clic sube al padre.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/app.rs crates/ui/src/docking.rs crates/ui/src/panes/file_panel.rs
git commit -m "feat(ui): íconos por fila en el file panel + fila '..' (UI pura)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Ícono de unidad en el árbol + selector de set / toggle ".." en ⚙

**Files:**
- Modify: `crates/ui/src/panes/tree_panel.rs` (ícono de unidad)
- Modify: `crates/ui/src/toolbar.rs` (menú ⚙: set de íconos + toggle fila "..")

- [ ] **Step 1: Ícono de unidad en el tree panel**

Modify `crates/ui/src/panes/tree_panel.rs` — la firma gana `icons: &IconProvider` y
pinta un ícono de unidad junto a la ubicación. Reemplaza por:

```rust
// Naygo — panel de árbol (esqueleto de Fase 2A/2B): ubicación + ícono de unidad.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Esqueleto: muestra la carpeta del panel `Files` activo (con un ícono de unidad)
//! y permite subir. El árbol expandible real es trabajo posterior.

use crate::docking::PaneRequest;
use crate::icons::IconProvider;
use naygo_core::icon_kind::{DriveKind, IconKey};
use naygo_core::workspace::Workspace;

pub fn show(
    ui: &mut egui::Ui,
    workspace: &mut Workspace,
    pending: &mut Vec<PaneRequest>,
    icons: &IconProvider,
) {
    let active = workspace.active_id();
    let dir = workspace.active_files().map(|f| f.current_dir.clone());

    ui.horizontal(|ui| {
        let tex = icons.texture(IconKey::Drive(DriveKind::Unknown));
        ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(16.0, 16.0)));
        ui.label("Ubicación actual");
    });
    if let Some(d) = &dir {
        ui.monospace(d.display().to_string());
    } else {
        ui.label("—");
    }
    ui.separator();
    if ui.button("⬆ Subir un nivel").clicked() {
        if let (Some(id), Some(d)) = (active, dir) {
            if let Some(parent) = d.parent() {
                pending.push(PaneRequest::NavigateTo {
                    id,
                    dir: parent.to_path_buf(),
                });
            }
        }
    }
}
```

- [ ] **Step 2: Selector de set de íconos + toggle ".." en el menú ⚙**

Modify `crates/ui/src/toolbar.rs` — en `settings_button`, tras el checkbox "Solo
íconos", añadir el selector de set y el toggle de la fila "..". Necesitas
`use naygo_core::config::IconSet;` arriba. Añade dentro del `menu_button("⚙", ...)`:

```rust
        ui.separator();
        ui.label("Set de íconos");
        let mut set = app.settings.icon_set;
        if ui.radio_value(&mut set, IconSet::Flat, "Flat (color)").clicked()
            || ui.radio_value(&mut set, IconSet::Fluent, "Fluent").clicked()
            || ui.radio_value(&mut set, IconSet::Mono, "Monocromo").clicked()
        {
            app.settings.icon_set = set;
            // El reload del IconProvider ocurre al inicio de ui() cuando detecta el cambio.
        }
        ui.separator();
        let mut show_parent = app.settings.show_parent_entry;
        if ui.checkbox(&mut show_parent, "Mostrar fila ..").changed() {
            app.settings.show_parent_entry = show_parent;
        }
```

NOTA: `radio_value(&mut value, alternativa, label)` marca `value=alternativa` al
clicar y devuelve un `Response`; `.clicked()` indica selección. Asignar
`app.settings.icon_set = set` dispara el reload en el siguiente frame (Tarea 5
Step 1d). Verifica la firma de `radio_value` en egui 0.34.3; si difiere, usa
`selectable_value` (misma semántica). El cambio de set persiste vía el `save()` de
eframe ya existente.

- [ ] **Step 3: Compilar, verificar, formatear**

Run: `cargo build -p naygo-ui` → compila. Warnings verbatim.
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start check. Manual (si hay display): el menú ⚙ muestra los 3 sets (radio) y el
toggle "Mostrar fila ..". Cambiar de set recarga los íconos; ocultar la fila ".." la
quita. El árbol muestra un ícono de unidad.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/panes/tree_panel.rs crates/ui/src/toolbar.rs
git commit -m "feat(ui): ícono de unidad en el árbol; selector de set y toggle '..' en ajustes

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Cierre de fase — README, verificación final, push

**Files:**
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: Actualizar el README**

Modify `README.md` — actualizar el bloque de estado:

```markdown
> **Estado:** Fase 2B (íconos) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-05-naygo-fase2b-iconos-design.md`](docs/superpowers/specs/2026-06-05-naygo-fase2b-iconos-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-06-naygo-fase2b-iconos.md`](docs/superpowers/plans/2026-06-06-naygo-fase2b-iconos.md).
> Fases 1 (esqueleto) y 2A (layout dinámico) completas.
```

- [ ] **Step 2: Verificación final completa**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → todo verde (core: ... + icon_kind 5 + config actualizado; ui: ... + assets 2).
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo fmt --check` → limpio (si no, `cargo fmt` + incluir en commit).
Run: `cargo build --release -p naygo-ui` → release compila (autoría + CRT estático intactos; los assets PNG quedan embebidos en el .exe).
Run (PowerShell): verificar tamaño del release `"{0:N2} MB" -f ((Get-Item target/release/naygo.exe).Length / 1MB)` — reportar (subió un poco por los 42 PNG embebidos; debe seguir siendo razonable, orden ~11–13 MB).

App-start manual: la app abre con íconos de color por tipo, fila ".." arriba, y el menú ⚙ permite cambiar set y togglear la fila. Cambiar set y reabrir conserva la elección (persistencia).

- [ ] **Step 3: Commit y push**

```bash
git add README.md
git commit -m "chore: actualizar estado del README a Fase 2B

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/fase2b-iconos
```

---

## Self-review (cobertura del spec)

| Requisito del spec 2B | Tarea(s) |
|---|---|
| `core::icon_kind` (IconKey, FileCategory, DriveKind) | 1 |
| `category_for_extension` O(1) case-insensitive | 1 |
| `icon_key_for(Entry)` | 1 |
| `IconSet` + `Settings.icon_set` + `show_parent_entry` | 2 |
| 3 sets de assets PNG embebidos | 3 (generación) + 4 (embebido) |
| `IconProvider` (carga/cachea/reload, fallback) | 4 |
| Íconos en el file panel (reemplazan glifos) | 5 |
| Fila ".." opcional (UI pura, default on) | 5 |
| Foco/type-ahead no se rompen (invariante) | 5 (".." por mouse; foco de teclado intacto) |
| Recarga al cambiar set (sin costo por frame) | 5 (reload en ui()) |
| Ícono de unidad en el árbol | 6 |
| Selector de set + toggle ".." en ⚙ | 6 |
| Persistencia (icon_set, show_parent_entry) | 2 (config) + save() existente |
| Tolerancia a assets corruptos → fallback | 4 (decode_png → vacío; load_texture nunca crashea) |
| Dependencia `image` (PNG) | 3 |
| Hueco para Shell de Windows | documentado en spec; no se codifica (correcto) |

**Diferido explícitamente (NO en 2B):** íconos reales del Shell, detección fina de
unidad, miniaturas, animaciones, i18n (2C), temas/color sets completos y escalado
fino (2C), foco de teclado hacia la fila ".." (pulido posterior), persistencia del
reacomodo del dock (deuda 2A), packs de íconos profesionales (se sueltan al iterar
sobre el set propio inicial, sin tocar código).

**Notas de riesgo (API / rutas — verificar contra la fuente):**
- Ruta de `include_bytes!` en `assets.rs` (Tarea 4): contar los `../` desde
  `crates/ui/src/icons/assets.rs` hasta `assets/icons/` (son 4). Si falla el build,
  ajustar y reportar.
- `egui::Image::new(&TextureHandle)` vs `Image::new((id, size))` (Tarea 5): usar la
  forma que compile en 0.34.3.
- `ColorImage::new([1,1], Color32)` (Tarea 4): confirmar firma; alternativa
  `from_rgba_unmultiplied([1,1], &[0,0,0,0])`.
- `radio_value` vs `selectable_value` (Tarea 6): usar la que exista en 0.34.3.
- `image` 0.25 con `features=["png"]` y `default-features=false`: confirmar que
  `load_from_memory` + `to_rgba8` están disponibles con solo la feature `png` (lo
  están; `load_from_memory` infiere formato por los features activos).
- El binario `gen_icons` (Tarea 3) es parte de `naygo-ui`; correrlo no afecta el
  binario principal `naygo`. Confirmar que `[[bin]]` adicional no rompe el bin
  principal (Cargo soporta varios `src/bin/*.rs` + el `[[bin]]` principal).
```
