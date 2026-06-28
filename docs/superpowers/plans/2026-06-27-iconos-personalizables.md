# Íconos personalizables — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reemplazar los 3 sets de íconos casi-idénticos por 5 sets de fábrica con personalidad real, hacer que los tintables se tiñan al color del tema, y agregar una sección de Configuración donde el usuario personaliza el ícono de cada objeto (mezclando sets + PNG propios) e importa/exporta sets en un archivo `.naygoset`.

**Architecture:** La lógica vive en `core` (testeable sin UI ni Windows): modelo de overrides (`IconSource`), resolución con fallback tolerante, y empaquetado `.naygoset` (zip). La UI Slint agrega una categoría "Íconos" al diálogo de Configuración y extiende el `IconCache` para teñir por máscara alfa. Un binario generador (build-time) rasteriza los SVG de las librerías a PNG. Se migra el `enum IconSet` viejo a IDs de string (data-driven).

**Tech Stack:** Rust, Slint (UI), serde/serde_json, `zip` (deflate), `image` (PNG), `resvg`/`usvg`/`tiny-skia` (rasterizar SVG en el generador).

---

## Convenciones del proyecto (leer antes de empezar)

- Header en cada archivo nuevo:
  ```rust
  // Naygo — <descripción breve>.
  // Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
  ```
- Nombres en inglés en el código; comentarios y commits en español neutral (tú/impersonal, **nunca** voseo).
- `core` no conoce egui ni Windows ni Slint. El hilo de UI nunca hace I/O.
- Tolerancia a fallos: el filesystem es hostil; `Result` tipado, nunca panic por I/O.
- Build limpio + `clippy` + tests verdes antes de cada commit. Correr clippy uno mismo (no confiar en subagentes).
- Tests de `core`: `cargo test -p naygo-core`. Build de UI bin-only: `cargo build -p naygo-ui-slint --bins`.

## Mapa de archivos

**Crear:**
- `crates/core/src/icon_source.rs` — `IconSource` (Builtin/UserPng), key↔string, serde del mapa de overrides.
- `crates/core/src/icon_pack.rs` — export/import `.naygoset` (zip + manifest), tolerante.
- `crates/core/src/bin/gen_icons.rs` — generador build-time: SVG → PNG (máscara alfa / color) + manifest por set.
- `assets/icons/lucide/`, `tabler/`, `material/`, `flat-color/` — PNG generados + `manifest.json`. (`mono/` se regenera.)

**Modificar:**
- `crates/core/src/config/mod.rs` — `Settings.icon_overrides`, migración del enum viejo, default `lucide`.
- `crates/core/src/icons/mod.rs` — sets data-driven, `resolve_bytes` con overrides + spec de tinte, quitar `enum IconSet` como fuente de verdad.
- `crates/core/src/icon_set.rs` — catálogo con 5 sets de fábrica + `tintable`.
- `crates/core/src/lib.rs` — exportar `icon_source`, `icon_pack`.
- `crates/core/Cargo.toml` — agregar `zip`.
- `crates/ui-slint/src/icons.rs` — `IconCache` por set efectivo + tinte por máscara alfa.
- `crates/ui-slint/src/config_ctrl.rs` — setters de override, import/export.
- `crates/ui-slint/src/main.rs` — handlers de la pestaña Íconos, llenar el VM.
- `crates/ui-slint/ui/config-window.slint` — categoría "Íconos" (sidebar + sección).
- `crates/ui-slint/ui/types.slint` — campos del VM para overrides.
- `crates/core/src/i18n/es.json`, `en.json` — claves nuevas.
- `crates/ui-slint/src/i18n_keys.rs` — cableado de claves.

---

## Fase 1 — Modelo de overrides en `core`

### Task 1: `IconSource` y serialización de la clave de override

**Files:**
- Create: `crates/core/src/icon_source.rs`
- Modify: `crates/core/src/lib.rs` (agregar `pub mod icon_source;`)

- [ ] **Step 1: Write the failing test**

En `crates/core/src/icon_source.rs`:

```rust
// Naygo — fuente de un ícono para una clave: un set embebido/pack, o un PNG propio.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `IconSource` describe de dónde sale el ícono de un objeto cuando el usuario lo
//! sobrescribe: `Builtin` apunta a otro set (por id); `UserPng` a un PNG propio bajo
//! `<config_dir>/icons/_user/`. Además, la conversión `IconKey` ↔ string estable
//! (`"action_back"`, `"file_image"`, `"drive"`, …) usada como clave del mapa de
//! overrides en `settings.json`. Puro y testeable.

use crate::icon_kind::IconKey;
use serde::{Deserialize, Serialize};

/// De dónde sale el ícono de un objeto sobrescrito por el usuario.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IconSource {
    /// Un ícono de otro set (id de fábrica o pack suelto) para la misma clave.
    Builtin { set_id: String },
    /// Un PNG propio del usuario, ruta relativa a `<config_dir>/icons/_user/`.
    UserPng { rel_path: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_source_serde_round_trip() {
        let a = IconSource::Builtin { set_id: "material".into() };
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"kind":"builtin","set_id":"material"}"#);
        let back: IconSource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, a);

        let b = IconSource::UserPng { rel_path: "ab12.png".into() };
        let json = serde_json::to_string(&b).unwrap();
        let back: IconSource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn key_string_round_trip_todas_las_claves() {
        for key in crate::icons::all_keys() {
            let s = key_to_string(key);
            let back = key_from_string(&s).expect("clave válida");
            assert_eq!(back, key, "round-trip falló para {s}");
        }
    }

    #[test]
    fn key_from_string_desconocida_es_none() {
        assert!(key_from_string("no_existe").is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core icon_source`
Expected: FAIL — `key_to_string`/`key_from_string` no existen, módulo no declarado.

- [ ] **Step 3: Write minimal implementation**

Agregar a `crates/core/src/icon_source.rs` (antes del `mod tests`):

```rust
use crate::icon_kind::{ActionIcon, DriveKind, FileCategory};

/// Clave estable de string para una `IconKey` (la misma que el nombre de archivo del
/// asset). Reutiliza `icons::file_name`.
pub fn key_to_string(key: IconKey) -> String {
    crate::icons::file_name(key).to_string()
}

/// Inversa de `key_to_string`. `None` si el string no corresponde a ninguna clave.
pub fn key_from_string(s: &str) -> Option<IconKey> {
    use FileCategory::*;
    let direct = match s {
        "folder" => Some(IconKey::Folder),
        "unknown" => Some(IconKey::Unknown),
        "drive" => Some(IconKey::Drive(DriveKind::Unknown)),
        "file_image" => Some(IconKey::File(Image)),
        "file_video" => Some(IconKey::File(Video)),
        "file_audio" => Some(IconKey::File(Audio)),
        "file_document" => Some(IconKey::File(Document)),
        "file_code" => Some(IconKey::File(Code)),
        "file_archive" => Some(IconKey::File(Archive)),
        "file_executable" => Some(IconKey::File(Executable)),
        "file_model3d" => Some(IconKey::File(Model3D)),
        "file_font" => Some(IconKey::File(Font)),
        "file_generic" => Some(IconKey::File(Generic)),
        _ => None,
    };
    if direct.is_some() {
        return direct;
    }
    ActionIcon::all()
        .iter()
        .find(|a| a.file_name() == s)
        .map(|a| IconKey::Action(*a))
}
```

Agregar a `crates/core/src/lib.rs` junto a los demás `pub mod`:

```rust
pub mod icon_source;
```

> Nota: `key_to_string` colapsa todas las variantes `Drive(_)` a `"drive"` (el asset es único); el round-trip de `Drive` vuelve como `Drive(Unknown)`. `all_keys()` incluye `Drive(Unknown)`, así que el test pasa. Correcto: la app pinta un solo asset `drive` para cualquier `DriveKind`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core icon_source`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/icon_source.rs crates/core/src/lib.rs
git commit -m "feat(core): IconSource + conversión IconKey<->string para overrides"
```

---

### Task 2: `icon_overrides` en `Settings` + migración del enum viejo

**Files:**
- Modify: `crates/core/src/config/mod.rs` (struct `Settings`, `default_icon_set`, `normalize_icon_set_id`, tests)

- [ ] **Step 1: Write the failing test**

Agregar a los tests de `crates/core/src/config/mod.rs`:

```rust
#[test]
fn icon_set_default_es_lucide() {
    assert_eq!(Settings::default().icon_set, "lucide");
    assert!(Settings::default().icon_overrides.is_empty());
}

#[test]
fn migra_flat_a_flat_color_y_fluent_a_lucide() {
    let dir = tempfile::tempdir().unwrap();
    for s in ["lucide", "flat-color"] {
        std::fs::create_dir_all(dir.path().join("icons").join(s)).unwrap();
    }
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{"version":1,"bar_position":"Top","icon_only":false,"icon_set":"flat"}"#,
    )
    .unwrap();
    let s = load_settings(dir.path());
    assert_eq!(s.icon_set, "flat-color");

    std::fs::write(
        dir.path().join("settings.json"),
        br#"{"version":1,"bar_position":"Top","icon_only":false,"icon_set":"fluent"}"#,
    )
    .unwrap();
    let s = load_settings(dir.path());
    assert_eq!(s.icon_set, "lucide");
}

#[test]
fn overrides_persisten_round_trip() {
    use crate::icon_source::IconSource;
    let dir = tempfile::tempdir().unwrap();
    let mut s = Settings::default();
    s.icon_overrides
        .insert("folder".into(), IconSource::Builtin { set_id: "material".into() });
    save_settings(dir.path(), &s);
    let back = load_settings(dir.path());
    assert_eq!(back.icon_overrides.get("folder"),
        Some(&IconSource::Builtin { set_id: "material".into() }));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core icon_set_default_es_lucide overrides_persisten migra_flat`
Expected: FAIL — campo `icon_overrides` no existe, default sigue siendo `flat`.

- [ ] **Step 3: Write minimal implementation**

En `crates/core/src/config/mod.rs`:

1. Agregar el campo al struct `Settings` (junto a `icon_set`, ~línea 127):

```rust
    /// Overrides por objeto sobre el set base: clave estable (`"action_back"`,
    /// `"file_image"`, …) → fuente del ícono. Vacío = solo el set base.
    #[serde(default)]
    pub icon_overrides: std::collections::BTreeMap<String, crate::icon_source::IconSource>,
```

2. Inicializarlo en el `Default` de `Settings` (~línea 438):

```rust
            icon_overrides: std::collections::BTreeMap::new(),
```

3. Cambiar `default_icon_set` (~línea 332) de `"flat"` a `"lucide"`:

```rust
fn default_icon_set() -> String {
    "lucide".to_string()
}
```

4. Extender `normalize_icon_set_id` (~línea 544):

```rust
fn normalize_icon_set_id(id: &str) -> String {
    match id {
        "Flat" | "flat" => "flat-color".to_string(),
        "Fluent" | "fluent" => "lucide".to_string(),
        "Mono" => "mono".to_string(),
        other => other.to_string(),
    }
}
```

> Actualiza los tests existentes de migración (líneas ~841-905): los que afirmaban `icon_set == "flat"` como default ahora afirman `"lucide"`; los de `"Flat"→"flat"` ahora son `"Flat"→"flat-color"`. Mismo commit.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core config`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/config/mod.rs
git commit -m "feat(core): Settings.icon_overrides + migración flat->flat-color, fluent->lucide, default lucide"
```

---

## Fase 2 — Catálogo de 5 sets + resolución con overrides

### Task 3: Catálogo data-driven con 5 sets de fábrica y `tintable`

**Files:**
- Modify: `crates/core/src/icon_set.rs` (`IconSetInfo`, `IconSetCatalog::load`, `resolve`)

- [ ] **Step 1: Write the failing test**

Reemplazar/expandir los tests de `crates/core/src/icon_set.rs`:

```rust
#[test]
fn cinco_sets_de_fabrica_presentes() {
    let dir = tempfile::tempdir().unwrap();
    let cat = IconSetCatalog::load(dir.path());
    for id in ["lucide", "tabler", "material", "flat-color", "mono"] {
        assert!(cat.contains(id), "falta el set de fábrica {id}");
    }
    assert_eq!(cat.available().len(), 5);
}

#[test]
fn tintable_correcto_por_set() {
    let dir = tempfile::tempdir().unwrap();
    let cat = IconSetCatalog::load(dir.path());
    let by = |id: &str| cat.available().iter().find(|s| s.id == id).unwrap().tintable;
    assert!(by("lucide"));
    assert!(by("tabler"));
    assert!(by("material"));
    assert!(by("mono"));
    assert!(!by("flat-color")); // trae su propio color
}

#[test]
fn resolve_desconocido_cae_a_lucide() {
    let dir = tempfile::tempdir().unwrap();
    let cat = IconSetCatalog::load(dir.path());
    assert_eq!(cat.resolve("no-existe"), "lucide");
    assert_eq!(cat.resolve("material"), "material");
}

#[test]
fn pack_suelto_importado_es_tintable_false_por_defecto() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("icons").join("mi-pack")).unwrap();
    let cat = IconSetCatalog::load(dir.path());
    let info = cat.available().iter().find(|s| s.id == "mi-pack").unwrap();
    assert!(!info.builtin);
    assert!(!info.tintable); // los packs del usuario no se tiñen salvo que su manifest lo diga
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core icon_set`
Expected: FAIL — solo hay 3 sets, no existe campo `tintable`, resolve cae a `"flat"`.

- [ ] **Step 3: Write minimal implementation**

En `crates/core/src/icon_set.rs`:

1. Agregar `tintable` a `IconSetInfo`:

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconSetInfo {
    pub id: String,
    pub label: String,
    pub builtin: bool,
    /// Si los íconos se tiñen al color del tema (línea/máscara) o traen su color (flat-color).
    pub tintable: bool,
}
```

2. Reescribir el vec de embebidos en `load` con los 5 sets:

```rust
let mut sets = vec![
    IconSetInfo { id: "lucide".into(),     label: "Lucide".into(),     builtin: true, tintable: true },
    IconSetInfo { id: "tabler".into(),     label: "Tabler".into(),     builtin: true, tintable: true },
    IconSetInfo { id: "material".into(),   label: "Material".into(),   builtin: true, tintable: true },
    IconSetInfo { id: "flat-color".into(), label: "Flat Color".into(), builtin: true, tintable: false },
    IconSetInfo { id: "mono".into(),       label: "Mono".into(),       builtin: true, tintable: true },
];
let factory_ids = ["lucide", "tabler", "material", "flat-color", "mono"];
```

3. En el descubrimiento de packs sueltos, cambiar el filtro `!["flat","fluent","mono"]` por `!factory_ids.contains(&name)`, y crear el `IconSetInfo` con `tintable: false` (un pack del usuario no se tiñe por defecto; en Fase 4 se lee del manifest si está):

```rust
if !factory_ids.contains(&name) {
    sets.push(IconSetInfo {
        id: name.to_string(),
        label: name.to_string(),
        builtin: false,
        tintable: false,
    });
}
```

4. En `resolve`, cambiar el fallback de `"flat"` a `"lucide"`:

```rust
pub fn resolve(&self, id: &str) -> String {
    if self.contains(id) { id.to_string() } else { "lucide".to_string() }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core icon_set`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/icon_set.rs
git commit -m "feat(core): catálogo de 5 sets de fábrica con flag tintable; fallback a lucide"
```

---

### Task 4: `resolve_bytes` con overrides + quitar el `enum IconSet`

**Files:**
- Modify: `crates/core/src/icons/mod.rs` (sets data-driven, `resolve_with_overrides`, mantener `bytes_for`)

> El `enum IconSet { Flat, Fluent, Mono }` deja de ser la fuente de verdad. Las tablas embebidas pasan a indexarse por id de string. Los 5 sets se embeben con `include_bytes!` igual que hoy.

- [ ] **Step 1: Write the failing test**

Agregar a los tests de `crates/core/src/icons/mod.rs`:

```rust
#[test]
fn resolve_con_override_builtin_usa_el_set_indicado() {
    use crate::icon_source::IconSource;
    use std::collections::BTreeMap;
    let mut ov: BTreeMap<String, IconSource> = BTreeMap::new();
    ov.insert("folder".into(), IconSource::Builtin { set_id: "material".into() });
    let dir = std::path::Path::new("");
    // override: folder sale de material; base lucide
    let with = resolve_with_overrides("lucide", &ov, IconKey::Folder, dir);
    let material_folder = bytes_for_id("material", IconKey::Folder);
    assert_eq!(with, material_folder);
    // sin override: copy sale del base (lucide)
    let copy = resolve_with_overrides("lucide", &ov, IconKey::Action(crate::icon_kind::ActionIcon::Copy), dir);
    assert_eq!(copy, bytes_for_id("lucide", IconKey::Action(crate::icon_kind::ActionIcon::Copy)));
}

#[test]
fn resolve_override_userpng_inexistente_cae_a_unknown() {
    use crate::icon_source::IconSource;
    use std::collections::BTreeMap;
    let mut ov: BTreeMap<String, IconSource> = BTreeMap::new();
    ov.insert("folder".into(), IconSource::UserPng { rel_path: "no-existe.png".into() });
    let dir = std::path::Path::new("");
    let bytes = resolve_with_overrides("lucide", &ov, IconKey::Folder, dir);
    assert!(!bytes.is_empty()); // cae a unknown embebido, nunca vacío
}

#[test]
fn cada_set_de_fabrica_cubre_las_33_claves() {
    for set in ["lucide", "tabler", "material", "flat-color", "mono"] {
        for key in all_keys() {
            assert!(!bytes_for_id(set, key).is_empty(), "asset vacío {set}/{:?}", key);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core icons`
Expected: FAIL — `resolve_with_overrides` y `bytes_for_id` no existen; faltan tablas de los 5 sets.

- [ ] **Step 3: Write minimal implementation**

En `crates/core/src/icons/mod.rs`:

1. Reemplazar `set_table!(FLAT…)`/`FLUENT`/`MONO` por las 5 tablas nuevas (mismo macro `set_table!`, mismas 33 entradas, distinto literal de carpeta):

```rust
set_table!(LUCIDE, "lucide");
set_table!(TABLER, "tabler");
set_table!(MATERIAL, "material");
set_table!(FLAT_COLOR, "flat-color");
set_table!(MONO, "mono");
```

2. Reemplazar `table_for(set: IconSet)` por una función por id de string:

```rust
/// Tabla de bytes embebidos para un id de set de fábrica; `None` si no es de fábrica.
fn table_for_id(set_id: &str) -> Option<&'static [(&'static str, &'static [u8])]> {
    match set_id {
        "lucide" => Some(LUCIDE),
        "tabler" => Some(TABLER),
        "material" => Some(MATERIAL),
        "flat-color" => Some(FLAT_COLOR),
        "mono" => Some(MONO),
        _ => None,
    }
}
```

3. Agregar `bytes_for_id` (reemplaza `bytes_for(IconSet, …)`):

```rust
/// Bytes PNG embebidos del ícono `key` para un set de fábrica `set_id`; cae a
/// `unknown` si la clave no tiene asset. Para sets no-fábrica devuelve `&[]`.
pub fn bytes_for_id(set_id: &str, key: IconKey) -> Vec<u8> {
    let name = file_name(key);
    match table_for_id(set_id) {
        Some(table) => table
            .iter()
            .find(|(n, _)| *n == name)
            .or_else(|| table.iter().find(|(n, _)| *n == "unknown"))
            .map(|(_, b)| b.to_vec())
            .unwrap_or_default(),
        None => Vec::new(),
    }
}
```

4. Reescribir `resolve_bytes` como `resolve_with_overrides` (mantén `resolve_bytes` simple llamando a la nueva con overrides vacíos, para no romper llamadores):

```rust
use crate::icon_source::IconSource;
use std::collections::BTreeMap;

/// Resuelve el ícono `key` aplicando overrides sobre el set base. Nunca vacío.
pub fn resolve_with_overrides(
    base_set: &str,
    overrides: &BTreeMap<String, IconSource>,
    key: IconKey,
    config_dir: &std::path::Path,
) -> Vec<u8> {
    // 1) override para esta clave
    if let Some(src) = overrides.get(file_name(key)) {
        match src {
            IconSource::Builtin { set_id } => {
                let b = resolve_set_bytes(set_id, key, config_dir);
                if !b.is_empty() { return b; }
            }
            IconSource::UserPng { rel_path } => {
                let p = config_dir.join("icons").join("_user").join(rel_path);
                if let Ok(bytes) = std::fs::read(&p) {
                    if !bytes.is_empty() { return bytes; }
                }
            }
        }
    }
    // 2) set base
    let b = resolve_set_bytes(base_set, key, config_dir);
    if !b.is_empty() { return b; }
    // 3) fallback embebido
    bytes_for_id("lucide", IconKey::Unknown)
}

/// Bytes de un set por id: embebido si es de fábrica, o pack suelto en disco.
fn resolve_set_bytes(set_id: &str, key: IconKey, config_dir: &std::path::Path) -> Vec<u8> {
    if table_for_id(set_id).is_some() {
        return bytes_for_id(set_id, key);
    }
    let path = config_dir
        .join("icons").join(set_id)
        .join(format!("{}.png", file_name(key)));
    std::fs::read(&path).unwrap_or_default()
}

/// Compat: resolución sin overrides (set base solo). Usada donde aún no hay overrides.
pub fn resolve_bytes(set_id: &str, key: IconKey, config_dir: &std::path::Path) -> Vec<u8> {
    let empty = BTreeMap::new();
    resolve_with_overrides(set_id, &empty, key, config_dir)
}
```

5. Eliminar `embedded_set`, `table_for(IconSet)` y el `match set` sobre el enum. El `enum IconSet` puede quedar en `config` por compat de serde viejo, pero `icons/mod.rs` ya no lo usa. Borrar los tests viejos que iteraban `[IconSet::Flat, …]` y reemplazarlos por el test nuevo `cada_set_de_fabrica_cubre_las_33_claves`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core icons`
Expected: PASS.
> ⚠️ Este task NO compila hasta que existan los PNG de los 5 sets en `assets/icons/<set>/` (los crea la Fase 3). Si trabajas estrictamente en orden, **haz la Fase 3 antes que el Step 4 de este task** (el `include_bytes!` falla si falta el archivo). Alternativa: ejecuta primero el generador de Fase 3, luego compila aquí. Coordina con quien ejecute el plan.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/icons/mod.rs
git commit -m "feat(core): resolve_with_overrides (override por objeto) + sets indexados por id; retiro del enum IconSet en icons"
```

---

## Fase 3 — Generador de los 5 sets de fábrica (build-time)

> **Orden de ejecución:** este generador debe correrse (y sus PNG quedar commiteados)
> **antes** de compilar el Step 4 del Task 4, porque `icons/mod.rs` los embebe con
> `include_bytes!`. En la práctica: implementa el generador (Tasks 5-6), córrelo para
> producir los assets, commitea los PNG, y recién ahí compila la Fase 2.

### Task 5: Esqueleto del generador + tabla de mapeo SVG→IconKey

**Files:**
- Create: `crates/core/src/bin/gen_icons.rs`
- Modify: `crates/core/Cargo.toml` (deps de generación, solo para el bin)

> Decisión: el generador es un binario de `naygo-core` (`src/bin/gen_icons.rs`), no un
> crate xtask aparte (mantiene el workspace en 4 miembros). Las deps de rasterizado
> (`resvg`/`usvg`/`tiny-skia`) son pesadas pero **solo** las usa este bin; se marcan
> opcionales tras una feature `gen-icons` para no inflar el build normal.

- [ ] **Step 1: Escribir la tabla de mapeo + un test de cobertura**

En `crates/core/src/bin/gen_icons.rs`:

```rust
// Naygo — generador build-time de los sets de íconos de fábrica (SVG -> PNG).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// No forma parte del binario de la app. Se ejecuta a mano para regenerar
// `assets/icons/<set>/`. Lee los SVG de las librerías descomprimidas y rasteriza
// cada IconKey a PNG: máscara alfa blanca para sets tintables, color para flat-color.
// Idempotente; nunca toca el config del usuario.

/// Una entrada de mapeo: clave estable de Naygo -> ruta del SVG dentro de la librería.
struct Map { key: &'static str, rel: &'static str }

/// Mapeo por set: (id, carpeta_fuente, tintable, &[Map]).
struct SetSpec { id: &'static str, src_root: &'static str, tintable: bool, maps: &'static [Map] }

// NOTA: rellenar las 33 entradas por set. Ejemplo (lucide), las demás análogas.
const LUCIDE: &[Map] = &[
    Map { key: "action_back",   rel: "arrow-left.svg" },
    Map { key: "action_forward",rel: "arrow-right.svg" },
    Map { key: "action_up",     rel: "arrow-up.svg" },
    Map { key: "action_refresh",rel: "refresh-cw.svg" },
    Map { key: "action_copy",   rel: "copy.svg" },
    Map { key: "action_cut",    rel: "scissors.svg" },
    Map { key: "action_paste",  rel: "clipboard.svg" },
    Map { key: "action_delete", rel: "trash-2.svg" },
    Map { key: "action_new_file",  rel: "file-plus.svg" },
    Map { key: "action_new_folder",rel: "folder-plus.svg" },
    Map { key: "action_add_pane",  rel: "columns-2.svg" },
    Map { key: "action_swap_panes",rel: "arrow-left-right.svg" },
    Map { key: "action_clone_path",rel: "copy-plus.svg" },
    Map { key: "action_new_window",rel: "app-window.svg" },
    Map { key: "action_settings",  rel: "settings.svg" },
    Map { key: "action_tabs",      rel: "layout-panel-top.svg" },
    Map { key: "action_layouts",   rel: "layout-grid.svg" },
    Map { key: "action_terminal",  rel: "terminal.svg" },
    Map { key: "action_eject",     rel: "eject.svg" },
    Map { key: "action_panel",     rel: "panel-left.svg" },
    Map { key: "folder",        rel: "folder.svg" },
    Map { key: "drive",         rel: "hard-drive.svg" },
    Map { key: "unknown",       rel: "file.svg" },
    Map { key: "file_image",    rel: "image.svg" },
    Map { key: "file_video",    rel: "film.svg" },
    Map { key: "file_audio",    rel: "music.svg" },
    Map { key: "file_document", rel: "file-text.svg" },
    Map { key: "file_code",     rel: "file-code.svg" },
    Map { key: "file_archive",  rel: "file-archive.svg" },
    Map { key: "file_executable",rel: "file-cog.svg" },
    Map { key: "file_model3d",  rel: "box.svg" },
    Map { key: "file_font",     rel: "type.svg" },
    Map { key: "file_generic",  rel: "file.svg" },
];

#[cfg(test)]
mod tests {
    use super::*;
    // Las 33 claves que la app pinta (espejo de icons::all_keys, por nombre).
    const ALL_KEYS: &[&str] = &[
        "folder","drive","unknown",
        "file_image","file_video","file_audio","file_document","file_code",
        "file_archive","file_executable","file_model3d","file_font","file_generic",
        "action_back","action_forward","action_up","action_refresh","action_copy",
        "action_cut","action_paste","action_delete","action_new_file","action_new_folder",
        "action_add_pane","action_swap_panes","action_clone_path","action_new_window",
        "action_settings","action_tabs","action_layouts","action_terminal","action_eject","action_panel",
    ];

    fn assert_cubre(maps: &[Map], set: &str) {
        for k in ALL_KEYS {
            assert!(maps.iter().any(|m| m.key == *k), "{set}: falta mapeo de {k}");
        }
        assert_eq!(maps.len(), ALL_KEYS.len(), "{set}: sobran/faltan mapeos");
    }

    #[test]
    fn lucide_cubre_todas_las_claves() { assert_cubre(LUCIDE, "lucide"); }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core --bin gen_icons`
Expected: FAIL al compilar si `--bin gen_icons` aún no está declarado, o el test falla si el mapeo está incompleto. Completa LUCIDE hasta que pase.

- [ ] **Step 3: Completar mapeos de los otros 4 sets**

Añadir `TABLER`, `MATERIAL`, `FLAT_COLOR`, `MONO` (mono reusa los SVG de lucide pero teñidos gris) con sus 33 `Map` cada uno, y un test `assert_cubre` por set. Los nombres de archivo varían por librería: para Tabler usar `icons/outline/*.svg`; para Material `src/<categoría>/<nombre>/materialicons/24px.svg`; para Flat Color los `svg/*.svg`. Donde una librería no tenga un ícono exacto, elegir el más cercano (documentar la elección en un comentario junto al `Map`).

Marcar en `Cargo.toml`:

```toml
[[bin]]
name = "gen_icons"
required-features = ["gen-icons"]

[features]
gen-icons = ["dep:resvg", "dep:usvg", "dep:tiny-skia"]

[dependencies]
# … existentes …
resvg = { version = "0.47", optional = true }
usvg = { version = "0.47", optional = true }
tiny-skia = { version = "0.12", optional = true }
zip = { version = "2", default-features = false, features = ["deflate"] }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core --bin gen_icons --features gen-icons`
Expected: PASS (5 tests `*_cubre_todas_las_claves`).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/bin/gen_icons.rs crates/core/Cargo.toml
git commit -m "feat(gen): esqueleto del generador de íconos + mapeo SVG->IconKey de los 5 sets"
```

---

### Task 6: Rasterizado (máscara alfa / color) + escritura de assets

**Files:**
- Modify: `crates/core/src/bin/gen_icons.rs` (función `main`, rasterizar y escribir)

- [ ] **Step 1: Implementar el rasterizado**

Agregar la lógica en `gen_icons.rs`. Para cada `SetSpec`:
- Cargar el SVG con `usvg::Tree::from_data`.
- Renderizar con `resvg` a un `tiny_skia::Pixmap` a 48×48 (@2x de 24).
- Si `tintable`: forzar el color a **blanco opaco** conservando el alfa (la app teñirá). Implementación: parsear el SVG fijando `fill`/`stroke` a `#FFFFFF` antes de rasterizar (manipular el `usvg::Tree` o pre-procesar el texto SVG con un reemplazo de `currentColor`/`fill`/`stroke`).
- Si no tintable (flat-color): rasterizar tal cual.
- Guardar como PNG en `assets/icons/<id>/<key>.png` con el crate `image` (ya disponible).
- Escribir `assets/icons/<id>/manifest.json`: `{ "id", "label", "tintable", "version":1, "license" }`.

```rust
fn main() {
    let repo_assets = std::path::Path::new("assets/icons");
    let svg_root = std::path::Path::new("assets/icons"); // donde están las carpetas descomprimidas/clonadas
    for spec in all_specs() {
        let out_dir = repo_assets.join(spec.id);
        std::fs::create_dir_all(&out_dir).expect("crear dir de set");
        for m in spec.maps {
            let svg_path = svg_root.join(spec.src_root).join(m.rel);
            let png = rasterize(&svg_path, spec.tintable, 48)
                .unwrap_or_else(|e| panic!("rasterizar {}: {e}", svg_path.display()));
            std::fs::write(out_dir.join(format!("{}.png", m.key)), png).expect("escribir png");
        }
        write_manifest(&out_dir, spec);
        eprintln!("set '{}' generado ({} íconos)", spec.id, spec.maps.len());
    }
}
```

> `rasterize(path, tintable, size) -> Result<Vec<u8>, String>` y `write_manifest` se
> implementan en este mismo archivo. El reemplazo de color para tintables: leer el
> texto del SVG y sustituir atributos `fill="..."`/`stroke="..."` (que no sean `none`)
> y `currentColor` por `#FFFFFF`, luego rasterizar. Probar con un SVG de muestra.

- [ ] **Step 2: Ejecutar el generador**

Asegúrate de tener las carpetas fuente descomprimidas (los SVG de lucide/tabler/material/flat-color) bajo `assets/icons/<src_root>/`. Luego:

Run: `cargo run -p naygo-core --bin gen_icons --features gen-icons`
Expected: imprime "set 'lucide' generado (33 íconos)" … para los 5; crea `assets/icons/<set>/*.png` + `manifest.json`.

- [ ] **Step 3: Verificación manual visual**

Abrir 4-5 PNG generados de distintos sets y confirmar: los tintables son blancos sobre transparente (se verán al teñir), flat-color conserva color. Comparar un par a 24px que se distingan claramente entre sets.

- [ ] **Step 4: Compilar la Fase 2 ahora que existen los assets**

Run: `cargo test -p naygo-core icons icon_set`
Expected: PASS — ahora `include_bytes!` encuentra los PNG y los tests de Task 3/4 pasan.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/bin/gen_icons.rs assets/icons/lucide assets/icons/tabler assets/icons/material assets/icons/flat-color assets/icons/mono
git commit -m "feat(gen): rasterizado SVG->PNG (máscara alfa/color) + assets de los 5 sets regenerados"
```

> Limpieza: en un commit aparte, borrar `assets/icons/flat/`, `assets/icons/fluent/`,
> `assets/icons/otros/` (reemplazados) y los `.zip`/carpetas fuente que ya no se
> embeben, si Nicolás confirma que no los quiere en el repo. Preguntar antes de borrar.

---

## Fase 4 — Import/export `.naygoset`

### Task 7: Serializar/deserializar el manifest del set

**Files:**
- Create: `crates/core/src/icon_pack.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod icon_pack;`)

- [ ] **Step 1: Write the failing test**

En `crates/core/src/icon_pack.rs`:

```rust
// Naygo — empaquetado de sets de íconos: export/import del archivo .naygoset (zip).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Un `.naygoset` es un zip con `manifest.json` + `icons/` (los PNG que el set
//! aporta). Export toma el set efectivo (base + overrides) y lo empaqueta autocontenido;
//! import lo valida y lo copia a `<config_dir>/icons/<nombre>/`. Tolerante a archivos
//! corruptos: una entrada inválida no aborta la importación.

use crate::icon_source::IconSource;
use serde::{Deserialize, Serialize};

/// Una entrada del manifest: qué objeto y de dónde sale su ícono.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverrideEntry {
    pub key: String,
    pub source: IconSource,
}

/// Manifest de un `.naygoset`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    pub author: String,
    pub base_set_id: String,
    #[serde(default)]
    pub overrides: Vec<OverrideEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let m = PackManifest {
            schema: 1,
            name: "Mi set".into(),
            author: "Nico".into(),
            base_set_id: "lucide".into(),
            overrides: vec![OverrideEntry {
                key: "folder".into(),
                source: IconSource::Builtin { set_id: "material".into() },
            }],
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: PackManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core icon_pack`
Expected: FAIL — módulo no declarado.

- [ ] **Step 3: Write minimal implementation**

Agregar `pub mod icon_pack;` a `lib.rs`. (El struct ya está completo en el Step 1.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core icon_pack`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/icon_pack.rs crates/core/src/lib.rs
git commit -m "feat(core): PackManifest del .naygoset (schema + base + overrides)"
```

---

### Task 8: Export e import del `.naygoset` (zip)

**Files:**
- Modify: `crates/core/src/icon_pack.rs` (funciones `export_pack`, `import_pack`)

- [ ] **Step 1: Write the failing test**

Agregar a los tests de `icon_pack.rs`:

```rust
#[test]
fn export_import_round_trip() {
    use std::collections::BTreeMap;
    let cfg = tempfile::tempdir().unwrap();
    // un PNG de usuario en _user
    let user_dir = cfg.path().join("icons").join("_user");
    std::fs::create_dir_all(&user_dir).unwrap();
    std::fs::write(user_dir.join("ab12.png"), b"\x89PNG\r\n\x1a\nFAKE").unwrap();

    let mut overrides: BTreeMap<String, IconSource> = BTreeMap::new();
    overrides.insert("folder".into(), IconSource::Builtin { set_id: "material".into() });
    overrides.insert("file_image".into(), IconSource::UserPng { rel_path: "ab12.png".into() });

    let out = cfg.path().join("mi.naygoset");
    export_pack(&out, "Mi set", "Nico", "lucide", &overrides, cfg.path()).unwrap();
    assert!(out.exists());

    // importar a OTRO config dir
    let cfg2 = tempfile::tempdir().unwrap();
    let imported = import_pack(&out, cfg2.path()).unwrap();
    assert_eq!(imported.base_set_id, "lucide");
    // el PNG de usuario quedó copiado bajo icons/<nombre>/
    let pack_dir = cfg2.path().join("icons").join(&imported.name);
    assert!(pack_dir.join("_user").join("ab12.png").exists());
}

#[test]
fn import_pack_corrupto_es_err_no_panic() {
    let cfg = tempfile::tempdir().unwrap();
    let bad = cfg.path().join("malo.naygoset");
    std::fs::write(&bad, b"esto no es un zip").unwrap();
    assert!(import_pack(&bad, cfg.path()).is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core icon_pack`
Expected: FAIL — `export_pack`/`import_pack` no existen.

- [ ] **Step 3: Write minimal implementation**

Agregar a `icon_pack.rs`:

```rust
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

/// Error de empaquetado (string simple; se muestra al usuario vía MessageModal).
pub type PackResult<T> = Result<T, String>;

/// Exporta el set efectivo (base + overrides) a un archivo `.naygoset`.
pub fn export_pack(
    out: &Path,
    name: &str,
    author: &str,
    base_set_id: &str,
    overrides: &BTreeMap<String, IconSource>,
    config_dir: &Path,
) -> PackResult<()> {
    let manifest = PackManifest {
        schema: 1,
        name: name.to_string(),
        author: author.to_string(),
        base_set_id: base_set_id.to_string(),
        overrides: overrides
            .iter()
            .map(|(k, s)| OverrideEntry { key: k.clone(), source: s.clone() })
            .collect(),
    };
    let file = std::fs::File::create(out).map_err(|e| format!("crear {}: {e}", out.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    zip.start_file("manifest.json", opts).map_err(|e| e.to_string())?;
    zip.write_all(&json).map_err(|e| e.to_string())?;

    // empaquetar los PNG de usuario referenciados por overrides UserPng
    for (_k, src) in overrides {
        if let IconSource::UserPng { rel_path } = src {
            let p = config_dir.join("icons").join("_user").join(rel_path);
            if let Ok(bytes) = std::fs::read(&p) {
                zip.start_file(format!("icons/_user/{rel_path}"), opts)
                    .map_err(|e| e.to_string())?;
                zip.write_all(&bytes).map_err(|e| e.to_string())?;
            }
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Importa un `.naygoset` a `<config_dir>/icons/<nombre>/`. Devuelve el manifest.
/// Tolerante: un PNG corrupto no aborta; solo el manifest ilegible/zip inválido es error.
pub fn import_pack(path: &Path, config_dir: &Path) -> PackResult<PackManifest> {
    let file = std::fs::File::open(path).map_err(|e| format!("abrir {}: {e}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("zip inválido: {e}"))?;

    // leer manifest
    let manifest: PackManifest = {
        let mut mf = archive.by_name("manifest.json").map_err(|_| "falta manifest.json".to_string())?;
        let mut s = String::new();
        mf.read_to_string(&mut s).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| format!("manifest inválido: {e}"))?
    };
    if manifest.schema != 1 {
        return Err(format!("schema no soportado: {}", manifest.schema));
    }

    let dest = config_dir.join("icons").join(&manifest.name);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    // extraer todos los icons/* (tolerante: ignora fallos por entrada)
    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) { Ok(e) => e, Err(_) => continue };
        let name = entry.name().to_string();
        if let Some(rel) = name.strip_prefix("icons/") {
            let target = dest.join(rel);
            if let Some(parent) = target.parent() { let _ = std::fs::create_dir_all(parent); }
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() {
                let _ = std::fs::write(&target, &buf);
            }
        }
    }
    Ok(manifest)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core icon_pack`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/icon_pack.rs
git commit -m "feat(core): export/import .naygoset (zip autocontenido, import tolerante)"
```

---

## Fase 5 — `IconCache` con tinte + set efectivo en la UI

### Task 9: Tinte por máscara alfa en el `IconCache`

**Files:**
- Modify: `crates/ui-slint/src/icons.rs` (cache por set efectivo + color de tinte)

> El render de Slint es por software. Teñir = recolorear la máscara alfa con el color
> del tema, una sola vez al decodificar, cacheado por `(set_id, key, tint)`.

- [ ] **Step 1: Write the failing test**

Agregar a los tests de `crates/ui-slint/src/icons.rs`:

```rust
#[test]
fn tint_recolorea_conservando_alfa() {
    // un PNG blanco semitransparente: tras teñir a rojo, el RGB es rojo y el alfa se conserva.
    let mut buf = image::RgbaImage::new(1, 1);
    buf.put_pixel(0, 0, image::Rgba([255, 255, 255, 128]));
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(buf)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let out = tint_png(png.get_ref(), (255, 0, 0));
    let img = image::load_from_memory(&out).unwrap().to_rgba8();
    let px = img.get_pixel(0, 0);
    assert_eq!(px[0], 255);
    assert_eq!(px[1], 0);
    assert_eq!(px[2], 0);
    assert_eq!(px[3], 128, "el alfa se conserva");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-ui-slint tint_recolorea`
Expected: FAIL — `tint_png` no existe.

- [ ] **Step 3: Write minimal implementation**

Agregar a `crates/ui-slint/src/icons.rs`:

```rust
/// Recolorea un PNG (máscara) al color `(r,g,b)` conservando su canal alfa.
/// Para sets tintables: el glifo blanco se vuelve del color del tema.
pub fn tint_png(bytes: &[u8], rgb: (u8, u8, u8)) -> Vec<u8> {
    let decoded = match image::load_from_memory(bytes) {
        Ok(d) => d.to_rgba8(),
        Err(_) => return bytes.to_vec(),
    };
    let (w, h) = (decoded.width(), decoded.height());
    let mut out = image::RgbaImage::new(w, h);
    for (x, y, p) in decoded.enumerate_pixels() {
        out.put_pixel(x, y, image::Rgba([rgb.0, rgb.1, rgb.2, p[3]]));
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    if image::DynamicImage::ImageRgba8(out)
        .write_to(&mut buf, image::ImageFormat::Png)
        .is_err()
    {
        return bytes.to_vec();
    }
    buf.into_inner()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-ui-slint tint_recolorea`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/src/icons.rs
git commit -m "feat(ui): tint_png — recolorear máscara alfa al color del tema"
```

---

### Task 10: `IconCache` consume set efectivo + overrides + tinte

**Files:**
- Modify: `crates/ui-slint/src/icons.rs` (`IconCache::get`, nuevos campos)

- [ ] **Step 1: Write the failing test**

Agregar a los tests de `icons.rs`:

```rust
#[test]
fn cache_aplica_override_y_tiñe() {
    use naygo_core::icon_source::IconSource;
    use std::collections::BTreeMap;
    let mut ov: BTreeMap<String, IconSource> = BTreeMap::new();
    ov.insert("folder".into(), IconSource::Builtin { set_id: "material".into() });
    let mut c = IconCache::new("lucide", std::path::PathBuf::new());
    c.set_overrides(ov);
    c.set_tint(true, (200, 100, 50)); // set tintable: aplica color
    let img = c.get(IconKey::Folder);
    assert!(img.size().width > 0); // se resolvió y decodificó sin panic
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-ui-slint cache_aplica_override`
Expected: FAIL — `set_overrides`/`set_tint` no existen; `get` no aplica overrides.

- [ ] **Step 3: Write minimal implementation**

En `crates/ui-slint/src/icons.rs`, extender `IconCache`:

```rust
// nuevos campos
pub struct IconCache {
    map: HashMap<(String, IconKey, (u8,u8,u8), bool), Image>,
    active: String,
    config_dir: PathBuf,
    overrides: std::collections::BTreeMap<String, naygo_core::icon_source::IconSource>,
    tint: (u8, u8, u8),
    tintable: bool,
}
```

```rust
pub fn set_overrides(&mut self, ov: std::collections::BTreeMap<String, naygo_core::icon_source::IconSource>) {
    self.overrides = ov;
}
/// `tintable` = el set activo se tiñe; `rgb` = color del tema.
pub fn set_tint(&mut self, tintable: bool, rgb: (u8, u8, u8)) {
    self.tintable = tintable;
    self.tint = rgb;
}
```

Reescribir `get`:

```rust
pub fn get(&mut self, key: IconKey) -> Image {
    let tint = if self.tintable { self.tint } else { (0, 0, 0) };
    let ck = (self.active.clone(), key, tint, self.tintable);
    if let Some(img) = self.map.get(&ck) {
        return img.clone();
    }
    let mut bytes = naygo_core::icons::resolve_with_overrides(
        &self.active, &self.overrides, key, &self.config_dir,
    );
    if self.tintable {
        bytes = tint_png(&bytes, self.tint);
    }
    let img = decode(&bytes);
    self.map.insert(ck, img.clone());
    img
}
```

Actualizar `IconCache::new` para inicializar los campos nuevos (`overrides` vacío, `tint` negro, `tintable` false). Quitar el `#![allow(dead_code)]` si ya no aplica.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-ui-slint cache`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/src/icons.rs
git commit -m "feat(ui): IconCache resuelve set efectivo (overrides) y tiñe sets tintables"
```

---

### Task 11: Cablear set efectivo + tinte al cambiar set/tema

**Files:**
- Modify: `crates/ui-slint/src/main.rs` (handler `on_set_icon_set`, init del cache, al cambiar tema)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs:430` (construcción inicial del cache)

- [ ] **Step 1: Implementar el cableado (sin test unitario; verificación en vivo)**

En `workspace_ctrl.rs:430` (donde se crea el `IconCache`), tras construirlo, sembrar overrides y tinte desde la config y el tema:

```rust
let mut icons = crate::icons::IconCache::new(config.settings.icon_set.clone(), config_dir.clone());
icons.set_overrides(config.settings.icon_overrides.clone());
let tintable = naygo_core::icon_set::IconSetCatalog::load(&config_dir)
    .available().iter().find(|s| s.id == config.settings.icon_set)
    .map(|s| s.tintable).unwrap_or(false);
icons.set_tint(tintable, theme_text_rgb(&config.settings.theme)); // helper que da el color de texto del tema
```

> `theme_text_rgb(theme_id) -> (u8,u8,u8)` devuelve el color con que se pintan los íconos
> (normalmente el color de texto/acento del tema). Implementar junto a donde se resuelven
> los colores del tema en la UI; si ya existe un resolvedor de `ThemeColor`, reusarlo.

En `main.rs` `on_set_icon_set` (línea ~2843), tras `c.icons.set_active(active)`, recomputar `tintable` y reaplicar overrides + tinte, e invalidar el cache (recrearlo o limpiar el `map`). Añadir el mismo recálculo de tinte en el handler de cambio de tema (buscar `set_theme`/`on_set_theme`) para que al cambiar el tema los íconos tintables se repinten.

- [ ] **Step 2: Compilar bin-only**

Run: `cargo build -p naygo-ui-slint --bins`
Expected: compila sin error.

- [ ] **Step 3: Verificación en vivo**

Run: `cargo run -p naygo-ui-slint --bin naygo`
Comprobar: cambiar de set en Configuración cambia los íconos de toolbar/paneles de inmediato y se nota; cambiar de tema retiñe los sets de línea/relleno; flat-color mantiene su color.

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl.rs
git commit -m "feat(ui): aplicar set efectivo + tinte por tema al cambiar set/tema (en caliente)"
```

---

## Fase 6 — Pestaña "Íconos" en Configuración + import/export en UI

> Esta fase es de UI Slint y se verifica en vivo (Slint no tiene tests unitarios de
> markup). Cada task compila bin-only y se prueba corriendo la app. Mantener todo
> texto por clave i18n (ES+EN), sin hardcodear.

### Task 12: VM de la grilla de íconos + claves i18n

**Files:**
- Modify: `crates/ui-slint/ui/types.slint` (struct `IconRowVm`, campos en `SettingsVm`)
- Modify: `crates/core/src/i18n/es.json`, `crates/core/src/i18n/en.json`
- Modify: `crates/ui-slint/src/i18n_keys.rs` (cableado)

- [ ] **Step 1: Definir el VM de fila**

En `crates/ui-slint/ui/types.slint`, agregar:

```slint
// Una fila de la grilla "íconos por objeto" en Configuración.
export struct IconRowVm {
    key: string,        // clave estable ("action_back", "file_image", …)
    label: string,      // nombre traducido del objeto ("Atrás", "Imagen")
    icon: image,        // ícono efectivo actual (ya teñido)
    origin: string,     // "Lucide (base)" / "Material · override"
    overridden: bool,   // tiene override
    group: int,         // 0 = acción de barra, 1 = tipo de archivo
}
```

Y a `SettingsVm` (junto a `icon-sets`):

```slint
    icon-set-tintable: bool,   // si el set base activo se tiñe
    icon-rows: [IconRowVm],     // filas de la grilla por objeto
    icon-set-labels: [string],  // etiquetas legibles de los sets disponibles (paralelo a icon-sets)
```

- [ ] **Step 2: Claves i18n**

En `crates/core/src/i18n/es.json` agregar (y el equivalente en `en.json`):

```json
"slint.cfg.cat_icons": "Íconos",
"settings.icons.section_base": "Set base",
"settings.icons.section_objects": "Íconos por objeto",
"settings.icons.section_custom": "Set personalizado",
"settings.icons.change": "Cambiar",
"settings.icons.override_badge": "personalizado",
"settings.icons.user_png": "PNG propio…",
"settings.icons.reset": "Restablecer al set base",
"settings.icons.import": "Importar set (.naygoset)…",
"settings.icons.export": "Exportar set actual…",
"settings.icons.import_ok": "Set importado correctamente.",
"settings.icons.import_err": "No se pudo importar el set.",
"settings.icons.export_ok": "Set exportado correctamente.",
"settings.icons.group_actions": "Acciones de la barra",
"settings.icons.group_files": "Tipos de archivo",
"settings.icons.set.lucide": "Lucide (línea)",
"settings.icons.set.tabler": "Tabler (línea)",
"settings.icons.set.material": "Material (relleno)",
"settings.icons.set.flat-color": "Flat Color",
"settings.icons.set.mono": "Mono"
```

en.json: traducciones equivalentes ("Icons", "Base set", "Icons per object", "Custom set", "Change", "custom", "Own PNG…", "Reset to base set", "Import set (.naygoset)…", "Export current set…", etc.).

> Eliminar/actualizar las claves viejas `settings.icons.flat`/`fluent` si ya no se usan.

En `crates/ui-slint/src/i18n_keys.rs`, agregar el cableado (patrón `set_cfg_*`), p.ej.:

```rust
tr.set_cfg_cat_icons(c.t("slint.cfg.cat_icons").into());
tr.set_cfg_icons_section_base(c.t("settings.icons.section_base").into());
// … una por cada clave nueva usada en el markup …
```

- [ ] **Step 3: Compilar**

Run: `cargo build -p naygo-ui-slint --bins`
Expected: compila (las nuevas props del VM existen, las claves se cablean).

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/ui/types.slint crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/src/i18n_keys.rs
git commit -m "feat(ui): VM de la grilla de íconos + claves i18n ES/EN de la pestaña Íconos"
```

---

### Task 13: Categoría "Íconos" en el sidebar + sección (set base + grilla)

**Files:**
- Modify: `crates/ui-slint/ui/config-window.slint` (nueva categoría `cat == N`, sidebar entry, callbacks)

- [ ] **Step 1: Agregar la categoría al sidebar**

En el sidebar de categorías (donde se listan las 9 actuales), agregar la entrada "Íconos"
(`Tr.cfg-cat-icons`) con un nuevo índice `cat`. En el contenedor condicional, agregar
`if root.cat == <N>:` con la sección. Declarar los callbacks nuevos en `ConfigWindow`:

```slint
callback set-icon-set(string);          // ya existe; reutilizar
callback open-icon-picker(string);       // key — abre el selector (Task 14); el handler llena las choices
callback set-icon-override(string, string); // (key, set_id) — override builtin
callback set-icon-override-png(string);  // key — abre file dialog para PNG propio
callback clear-icon-override(string);    // key — quitar override de un objeto
callback reset-icon-overrides();         // limpiar todos
callback import-icon-set();              // abre file dialog .naygoset
callback export-icon-set();              // abre save dialog .naygoset
```

- [ ] **Step 2: Markup de la sección**

Dentro de `if root.cat == <N>`, construir (siguiendo el patrón `Field`/`CfgBtn`/`ThemeCombo`):

1. **Set base** — un `Text` con `Tr.cfg-icons-section-base` + una fila de chips/combo con
   `root.vm.icon-set-labels` (model) y `root.vm.icon-set` (current), que llama
   `set-icon-set`.
2. **Íconos por objeto** — un `Text` con la sección + un `VerticalLayout`/`for row in
   root.vm.icon-rows`: cada fila muestra `row.icon` (Image), `row.label`, `row.origin`,
   un badge si `row.overridden`, y un `CfgBtn` "Cambiar" que abre el selector (Task 14).
   Agrupar por `row.group` con un encabezado (`group_actions` / `group_files`).
3. **Set personalizado** — tres `CfgBtn`: importar, exportar, restablecer, que llaman a
   `import-icon-set`, `export-icon-set`, `reset-icon-overrides`.

- [ ] **Step 3: Compilar**

Run: `cargo build -p naygo-ui-slint --bins`
Expected: compila (los handlers Rust se conectan en Task 15; aquí pueden quedar como
callbacks declarados aún sin lógica — Slint permite callbacks sin conectar, no crashea).

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/ui/config-window.slint
git commit -m "feat(ui): categoría Íconos en Configuración — set base + grilla por objeto + botones de set"
```

---

### Task 14: Selector de ícono (popup al pulsar "Cambiar")

**Files:**
- Modify: `crates/ui-slint/ui/config-window.slint` (popup de selección)
- Modify: `crates/ui-slint/ui/types.slint` (VM del selector si hace falta)

- [ ] **Step 1: VM del selector**

Agregar a `types.slint`:

```slint
// Una opción del selector: el mismo objeto renderizado en un set.
export struct IconChoiceVm {
    set-id: string,
    set-label: string,
    icon: image,    // el objeto en ese set (ya teñido si corresponde)
}
```

Y a `SettingsVm`:

```slint
    icon-picker-key: string,        // clave del objeto que se está editando ("" = cerrado)
    icon-picker-choices: [IconChoiceVm],
```

- [ ] **Step 2: Markup del popup**

En `config-window.slint`, un `PopupWindow`/overlay que se muestra cuando
`root.vm.icon-picker-key != ""`: título "Ícono para «label»", un `for ch in
root.vm.icon-picker-choices` con cada `ch.icon` + `ch.set-label` (clic → `set-icon-override(key, ch.set-id)`),
más una celda final "PNG propio…" (clic → `set-icon-override-png(key)`). El botón "Cambiar"
de cada fila (Task 13) setea `icon-picker-key` para abrir este popup — se hace vía un
callback `open-icon-picker(string)` manejado en Rust (Task 15) que llena `icon-picker-choices`.

- [ ] **Step 3: Compilar + verificación en vivo**

Run: `cargo build -p naygo-ui-slint --bins`
Expected: compila. (La verificación visual completa va en Task 15, cuando los handlers
rellenan los datos.)

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/ui/config-window.slint crates/ui-slint/ui/types.slint
git commit -m "feat(ui): selector de ícono por objeto (mismo objeto en cada set + PNG propio)"
```

---

### Task 15: Handlers Rust de la pestaña Íconos (overrides + import/export + file dialogs)

**Files:**
- Modify: `crates/ui-slint/src/config_ctrl.rs` (setters de override + import/export delegando a core)
- Modify: `crates/ui-slint/src/main.rs` (conectar callbacks, llenar VM, file dialogs, refresh)

- [ ] **Step 1: Métodos en `config_ctrl.rs`**

Siguiendo el patrón `set_* + save()`:

```rust
pub fn set_icon_override(&mut self, key: String, set_id: String) {
    self.settings.icon_overrides.insert(key, naygo_core::icon_source::IconSource::Builtin { set_id });
    self.save();
}
pub fn set_icon_override_png(&mut self, key: String, src_png: &std::path::Path) {
    // copiar el PNG elegido a <config_dir>/icons/_user/<hash>.png
    if let Some(rel) = copy_user_png(&self.config_dir, src_png) {
        self.settings.icon_overrides.insert(key,
            naygo_core::icon_source::IconSource::UserPng { rel_path: rel });
        self.save();
    }
}
pub fn clear_icon_override(&mut self, key: &str) {
    self.settings.icon_overrides.remove(key);
    self.save();
}
pub fn reset_icon_overrides(&mut self) {
    self.settings.icon_overrides.clear();
    self.save();
}
pub fn export_icon_set(&self, out: &std::path::Path, name: &str, author: &str) -> Result<(), String> {
    naygo_core::icon_pack::export_pack(out, name, author,
        &self.settings.icon_set, &self.settings.icon_overrides, &self.config_dir)
}
pub fn import_icon_set(&mut self, path: &std::path::Path) -> Result<String, String> {
    let m = naygo_core::icon_pack::import_pack(path, &self.config_dir)?;
    // activar el set importado como base
    self.settings.icon_set = m.name.clone();
    // aplicar sus overrides
    self.settings.icon_overrides.clear();
    for ov in m.overrides {
        self.settings.icon_overrides.insert(ov.key, ov.source);
    }
    self.save();
    Ok(m.name)
}
```

`copy_user_png(config_dir, src) -> Option<String>`: crea `<config_dir>/icons/_user/`,
copia el PNG con un nombre derivado (hash del contenido o contador), devuelve la ruta
relativa. Implementar en `config_ctrl.rs`.

- [ ] **Step 2: Conectar en `main.rs`**

Para cada callback declarado en `config-window.slint`:
- `on_set_icon_override(|key, set_id| …)` → `c.config.set_icon_override(...)`, recomputar
  el `IconCache` (overrides) e invalidar, `refresh_icons()/refresh()`, y **regenerar el VM**
  (`rebuild_icon_rows`) para actualizar la grilla (origen, badge, ícono).
- `on_open_icon_picker(|key| …)` → llenar `icon-picker-choices` resolviendo el objeto en
  cada set del catálogo (usando `IconCache`/`resolve_with_overrides` por set) y setear
  `icon-picker-key`.
- `on_set_icon_override_png(|key| …)` → abrir `FileDialog` nativo (filtro `*.png`), llamar
  `set_icon_override_png`, refrescar.
- `on_clear_icon_override`, `on_reset_icon_overrides` → análogos + refresh + rebuild VM.
- `on_import_icon_set` → `FileDialog` (`*.naygoset`), `import_icon_set`, en éxito reconstruir
  el cache + VM + catálogo de sets, mostrar `MessageModal` con `import_ok`; en error,
  `MessageModal` con `import_err`. **Nunca panic.**
- `on_export_icon_set` → `FileDialog` de guardar (sufijo `.naygoset`), `export_icon_set`,
  `MessageModal` con `export_ok`/error.

Implementar `rebuild_icon_rows(c) -> Vec<IconRowVm>`: por cada `IconKey` de
`icons::all_keys()`, construir la fila (label traducido, ícono efectivo del cache, origen
y `overridden` según `settings.icon_overrides`, `group` por tipo). Llamarla al abrir
Configuración y tras cada cambio de íconos.

- [ ] **Step 3: Compilar + verificación en vivo completa**

Run: `cargo build -p naygo-ui-slint --bins` y `cargo run -p naygo-ui-slint --bin naygo`
Comprobar el flujo completo: abrir Configuración → Íconos; cambiar set base (se nota);
cambiar el ícono de "Carpeta" a Material (badge aparece, toolbar/paneles se actualizan);
poner un PNG propio en un objeto; restablecer; exportar a `.naygoset`; importar en otra
instancia/carpeta de config y ver que aplica. Verificar avisos vía `MessageModal`.

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/src/config_ctrl.rs crates/ui-slint/src/main.rs
git commit -m "feat(ui): handlers de personalización de íconos — override por objeto, PNG propio, import/export .naygoset"
```

---

## Fase 7 — Cierre

### Task 16: i18n de los nombres de objeto + repaso final

**Files:**
- Modify: `crates/core/src/i18n/es.json`, `en.json` (labels de cada IconKey)
- Modify: `crates/ui-slint/src/i18n_keys.rs` si hace falta

- [ ] **Step 1: Claves de nombre por objeto**

Agregar una clave por objeto para la columna "label" de la grilla, p.ej.:

```json
"icons.obj.action_back": "Atrás",
"icons.obj.folder": "Carpeta",
"icons.obj.file_image": "Imagen",
"… (las 33) …"
```

y sus traducciones en `en.json`. `rebuild_icon_rows` usa `c.t("icons.obj.<key>")` para el label.

- [ ] **Step 2: Repaso de licencias (THIRD-PARTY-NOTICES)**

Verificar que `THIRD-PARTY-NOTICES.md` lista Lucide (ISC), Tabler (MIT), Material
(Apache-2.0) y Flat Color Icons (MIT). Si falta alguna, agregarla. Confirmar que el
generador copió las licencias correspondientes (o agregarlas a mano).

- [ ] **Step 3: Suite completa + clippy**

Run: `cargo test -p naygo-core` y `cargo clippy -p naygo-core -p naygo-ui-slint --bins`
Expected: todos verdes, sin warnings de clippy.
Run: `pwsh scripts/test-all.ps1` (si existe y aplica).

- [ ] **Step 4: Verificación en VM (visual)**

Como manda el proyecto: probar en VM limpia que los íconos se ven, se tiñen por tema, el
cambio de set se nota, y el import/export funciona. (Esta verificación la hace Nicolás.)

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json THIRD-PARTY-NOTICES.md
git commit -m "feat(i18n): nombres de objeto de la grilla de íconos + licencias de las librerías"
```

---

## Resumen de fases

1. **Modelo** (`IconSource`, `Settings.icon_overrides`, migración) — core, TDD.
2. **Catálogo + resolución** (5 sets, `resolve_with_overrides`, retiro del enum) — core, TDD.
3. **Generador** (SVG→PNG máscara/color, assets) — core bin, build-time. *(correr antes de compilar la Fase 2)*
4. **`.naygoset`** (manifest + export/import zip tolerante) — core, TDD.
5. **Cache + tinte** (`tint_png`, set efectivo, cableado por tema) — UI, TDD + vivo.
6. **Pestaña Íconos** (categoría, grilla, selector, handlers, import/export) — UI, vivo.
7. **Cierre** (i18n de objetos, licencias, suite + clippy + VM).
