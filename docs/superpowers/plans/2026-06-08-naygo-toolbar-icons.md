# toolbar-icons — Estilo de toolbar + íconos reales + packs sueltos — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** La toolbar puede usar glifos (color del tema o del usuario) o íconos del set activo; los 3 sets embebidos pasan a íconos reales (de los zips); y el usuario puede sumar packs de íconos sueltos sin recompilar.

**Architecture:** `core::icon_kind` gana `IconKey::Action(ActionIcon)` (~12 acciones); `core::config` gana `ToolbarIconStyle` + `toolbar_glyph_color`, y `icon_set` pasa de enum a id string; `core::icon_set::IconSetCatalog` descubre sets sueltos (patrón `ThemeCatalog`). El `IconProvider` carga por id (embebido vía `assets.rs`, suelto desde `<config>/icons/<id>/`). La toolbar pinta glifos coloreados o `Image`s del set (tinte Mono al pintar). Los assets reales se extraen de los zips livianos a `assets/icons/{flat,fluent,mono}/`, con fallback al placeholder.

**Tech Stack:** Rust, `naygo-core`/`naygo-ui`, `eframe`/`egui` 0.34.3, `image` 0.25, `serde`. Sin dependencias nuevas. Íconos: flat-color (MIT), fluentui (MIT), lucide (ISC).

**Estado de partida (rama `feat/toolbar-icons`, desde `main`):**
- `crates/core/src/icon_kind.rs`: `pub enum IconKey { Folder, File(FileCategory), Drive(DriveKind), Unknown }` (`#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]`). `FileCategory` (10: Image,Video,Audio,Document,Code,Archive,Executable,Model3D,Font,Generic). `DriveKind`.
- `crates/ui/src/icons/mod.rs`: `IconProvider { set: IconSet, textures: HashMap<IconKey,TextureHandle>, fallback }`. `new(ctx, set: IconSet)` carga `assets::all_keys()`. `reload(ctx, set)`. `texture(key) -> &TextureHandle`. `load_texture(ctx, set, key)` → `assets::bytes_for(set,key)` → decode → texture; PNG ilegible → 1x1 transparente.
- `crates/ui/src/icons/assets.rs`: macros `png!($set:literal,$name:literal)` (`include_bytes!("../../../../assets/icons/<set>/<name>.png")`) y `set_table!($KONST,$set)` con 13 nombres (folder, file_image..file_generic, drive, unknown). `FLAT/FLUENT/MONO`. `table_for(set: IconSet)`. `bytes_for(set, key)` (cae a "unknown"). `file_name(key) -> &'static str` (Folder→"folder", Drive→"drive", Unknown→"unknown", File(cat)→"file_*"). `all_keys() -> Vec<IconKey>`. Tests: `cada_clave_tiene_bytes_no_vacios`, `clave_sin_asset_cae_a_unknown`, `cada_clave_tiene_su_propio_asset_no_solo_el_fallback` (iteran `[IconSet::Flat,Fluent,Mono]`).
- `crates/core/src/config/mod.rs`: `pub enum IconSet { Flat, Fluent, Mono }`. `Settings.icon_set: IconSet` con `#[serde(default="default_icon_set")]` (→Flat). Manual `impl Default` (`icon_set: IconSet::Flat`). Test `settings_v1_sin_idioma_cae_a_default` con JSON `…"icon_set":"Flat"…`. Test `settings_default_tiene_iconos_flat…` (`s.icon_set == IconSet::Flat`). Otro test `icon_set: IconSet::Mono` (~356). Patrón aditivo + manual Default + `read_json`/`write_json`.
- `crates/core/src/theme/mod.rs`: `ThemeCatalog::load(dir, _active) -> ThemeCatalog` (embebidos por id + `read_dir(dir/themes)` para sueltos). `available() -> &[ThemeId]`. Patrón a replicar.
- `crates/ui/src/settings_window/appearance.rs`: `use naygo_core::config::IconSet;` + 3 `selectable_value(&mut app.settings.icon_set, IconSet::X, label)` (~18-20).
- `crates/ui/src/toolbar.rs`: `fn icon_button(ui, icon: &str, tip: &str, enabled: bool) -> bool` (`ui.add_enabled(enabled, egui::Button::new(icon)).on_hover_text(tip).clicked()`). Botones: `◀ ▶ ▲ ⟳ ⧉ ✂ 📋 🗑 🗋 🗀 ➕ ⚙` (~52-130). Recibe `app: &mut NaygoApp`.
- `crates/ui/src/app.rs`: `IconProvider::new(&cc.egui_ctx, settings.icon_set)` (~317); `self.icons.reload(ui.ctx(), set)` (~2263). `self.config_dir`. `self.settings`. `ActiveTheme` (theme_apply) con `accent()/text(...)/to_color32`.
- `assets/icons/`: `{flat,fluent,mono}/*.png` = placeholders (13 c/u). Zips livianos presentes: `flat-color-icons-master.zip` (551KB), `fluentui-emoji-main.zip` (146MB), `lucide-main.zip` (4.6MB). NO usar `material-design-icons-master.zip` (4.7GB) ni tabler/vscode. `.gitignore` tiene `assets/icons/*.zip` (zips NO se commitean).
- `crates/ui/src/bin/gen_icons.rs`: generador de placeholders (se conserva como fallback).
- `naygo_core::theme::ThemeColor` (hex serializable).

**Prerequisito:** Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo. Para descomprimir zips: PowerShell `Expand-Archive -Path <zip> -DestinationPath <tmp>` (a un TEMP fuera del repo). `cargo fmt --all -- --check`. Bash: NO `cd /d`.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. Header de 2 líneas en NUEVOS. `core` NUNCA importa egui/windows (`image` sí, es puro). Tolerante (asset faltante→fallback, sin panics). Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt antes de cada commit. SIEMPRE `cargo fmt --all`. **Los zips NO se commitean.** Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/toolbar-icons`. NO cambiar de rama.

**SECUENCIA / blast radius:** El cambio de `icon_set` enum→String (Task 3) toca config + appearance + IconProvider + el JSON de migración del test + el bin gen_icons. Para contenerlo: el enum `IconSet {Flat,Fluent,Mono}` SE CONSERVA internamente (lo usa `assets.rs` para los embebidos); solo `Settings.icon_set` y el catálogo pasan a id string, con un mapeo id↔enum para los embebidos. Tasks 1-2 (icon_kind, ActionIcon) son additivos. Task 3 (config+catalog) es el cross-cutting. Tasks 4-5 (assets reales + IconProvider por id) habilitan los packs. Tasks 6-7 (toolbar render + settings UI). Task 8 (extracción de assets reales). Task 9 cierre. `IconKey::Action` (Task 1) puede volver no-exhaustivos los `match` sobre IconKey (en `assets::file_name`) → manejar en Task 1.

**Alcance:** ENTRA: ActionIcon/IconKey::Action, ToolbarIconStyle+glyph_color, icon_set→id+catálogo de sets sueltos, IconProvider por id, render toolbar glifos/pack, settings, assets reales (3 sets), NOTICE, i18n. NO ENTRA: material-design/tabler/vscode, tinte Flat/Fluent, commitear zips, shell-B.

---

## Estructura de archivos

```
crates/core/src/
├── icon_kind.rs     # + ActionIcon + IconKey::Action(ActionIcon)
├── icon_set.rs      # NUEVO: IconSetCatalog (patrón ThemeCatalog)
├── config/mod.rs    # + ToolbarIconStyle + toolbar_glyph_color; icon_set: String + migración
├── lib.rs           # + pub mod icon_set;
└── i18n/{es,en}.json # + settings.toolbar.*

crates/ui/src/
├── icons/assets.rs  # file_name brazo Action; set_table! + action_* ; bytes_for_id(id,key) embebido
├── icons/mod.rs     # IconProvider por id (embebido/suelto) + config_dir
├── toolbar.rs       # icon_button: Glyphs(color)/Pack(Image, tint Mono)
├── settings_window/appearance.rs  # selector Glifos/Pack + color-picker; set list del catálogo
└── (gen_icons.rs conservado)

assets/icons/
├── {flat,fluent,mono}/  # PNGs REALES (tipo + action_*), reemplazan placeholders
└── NOTICE.md        # NUEVO: licencias (flat-color MIT, fluentui MIT, lucide ISC)
```

---

## Task 1: `core::icon_kind` — ActionIcon + IconKey::Action

**Files:**
- Modify: `crates/core/src/icon_kind.rs`

- [ ] **Step 1: Tests (TDD)**

Add to the `#[cfg(test)] mod tests` of icon_kind.rs:
```rust
    #[test]
    fn action_icon_all_son_12_con_file_name_unico() {
        let all = ActionIcon::all();
        assert_eq!(all.len(), 12);
        let mut names: Vec<&str> = all.iter().map(|a| a.file_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 12, "cada acción tiene un nombre de archivo único");
        // Todos empiezan con "action_".
        assert!(all.iter().all(|a| a.file_name().starts_with("action_")));
    }

    #[test]
    fn icon_key_action_es_copy_hashable() {
        use std::collections::HashSet;
        let mut s = HashSet::new();
        s.insert(IconKey::Action(ActionIcon::Copy));
        assert!(s.contains(&IconKey::Action(ActionIcon::Copy)));
        assert!(!s.contains(&IconKey::Action(ActionIcon::Cut)));
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core icon_kind` → ERROR: `ActionIcon` no existe.

- [ ] **Step 3: Implementar**

In `crates/core/src/icon_kind.rs`, add:
```rust
/// Ícono de una acción de la barra de herramientas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionIcon {
    Back,
    Forward,
    Up,
    Refresh,
    Copy,
    Cut,
    Paste,
    Delete,
    NewFile,
    NewFolder,
    AddPane,
    Settings,
}

impl ActionIcon {
    /// Todas las acciones (para precargar el atlas).
    pub fn all() -> &'static [ActionIcon] {
        use ActionIcon::*;
        &[
            Back, Forward, Up, Refresh, Copy, Cut, Paste, Delete, NewFile, NewFolder,
            AddPane, Settings,
        ]
    }

    /// Nombre de archivo (sin extensión) del ícono en un set, p. ej. "action_back".
    pub fn file_name(self) -> &'static str {
        use ActionIcon::*;
        match self {
            Back => "action_back",
            Forward => "action_forward",
            Up => "action_up",
            Refresh => "action_refresh",
            Copy => "action_copy",
            Cut => "action_cut",
            Paste => "action_paste",
            Delete => "action_delete",
            NewFile => "action_new_file",
            NewFolder => "action_new_folder",
            AddPane => "action_add_pane",
            Settings => "action_settings",
        }
    }
}
```
Add the variant to `IconKey`: after `Unknown`, add `Action(ActionIcon),`. (Keep the derives.)

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core icon_kind` → PASS.
Run: `cargo build -p naygo-core` → compiles. NOTE: `IconKey` adding a variant may break exhaustive matches elsewhere in core — `cargo build -p naygo-core` will flag them. The `icon_key_for` fn in icon_kind.rs returns specific variants (not a match ON IconKey), so it's fine. If a match breaks, add the `Action` arm.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/icon_kind.rs
git commit -m "feat(core): IconKey::Action + ActionIcon (12 íconos de toolbar)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `ActionIcon` (12)/`all()`/`file_name()`/`IconKey::Action` EXACTOS (Tasks 4-7 depend).

---

## Task 2: `assets.rs` — file_name brazo Action + nombres de acción en las tablas

**Files:**
- Modify: `crates/ui/src/icons/assets.rs`

NOTE: `IconKey::Action` (Task 1) makes `file_name(key)` in assets.rs non-exhaustive → fix it. Also the embedded set tables gain the 12 `action_*` entries (the PNGs will exist after Task 8; until then they're placeholders generated in Task 8's prep OR the `include_bytes!` would fail to compile). To avoid a compile break before the PNGs exist, this task FIRST generates placeholder `action_*` PNGs (extending gen_icons), THEN wires the table. See Step 1.

- [ ] **Step 1: Generar placeholders action_* (para que include_bytes! compile)**

Modify `crates/ui/src/bin/gen_icons.rs`: in its `icon_specs()` (or wherever it lists names), add the 12 `action_*` names with a placeholder color each (reuse the rectangle-drawing; any color). Run `cargo run -p naygo-ui --bin gen_icons` to write `assets/icons/{flat,fluent,mono}/action_*.png` (placeholders). This guarantees the `include_bytes!` of Step 3 compiles; Task 8 replaces them with real icons.
(If gen_icons.rs structure differs, just ensure 12 `action_*.png` placeholder files exist in each of the 3 set dirs — a tiny script or manual generation is fine. Verify with `ls assets/icons/flat/action_*.png`.)

- [ ] **Step 2: file_name brazo Action + tablas**

Modify `crates/ui/src/icons/assets.rs`:
a) `file_name(key)` — add the arm:
```rust
        IconKey::Action(a) => a.file_name(),
```
(import `ActionIcon` if needed: `use naygo_core::icon_kind::{ActionIcon, FileCategory, IconKey};`)
b) `set_table!` macro — add the 12 action rows so each set table embeds them:
```rust
            ("action_back", png!($set, "action_back")),
            ("action_forward", png!($set, "action_forward")),
            ("action_up", png!($set, "action_up")),
            ("action_refresh", png!($set, "action_refresh")),
            ("action_copy", png!($set, "action_copy")),
            ("action_cut", png!($set, "action_cut")),
            ("action_paste", png!($set, "action_paste")),
            ("action_delete", png!($set, "action_delete")),
            ("action_new_file", png!($set, "action_new_file")),
            ("action_new_folder", png!($set, "action_new_folder")),
            ("action_add_pane", png!($set, "action_add_pane")),
            ("action_settings", png!($set, "action_settings")),
```
c) `all_keys()` — add the action keys so the atlas preloads them:
```rust
    for a in naygo_core::icon_kind::ActionIcon::all() {
        v.push(IconKey::Action(*a));
    }
```

- [ ] **Step 3: Verificar**

Run: `cargo build -p naygo-ui` → compiles (include_bytes! finds the placeholder PNGs from Step 1).
Run: `cargo test -p naygo-ui icons::assets` → the existing tests (`cada_clave_tiene_su_propio_asset…`) now also cover the action keys (all_keys includes them) → PASS.
Run: `cargo clippy -p naygo-ui --all-targets -- -D warnings` → clean.
Run: `cargo fmt --all`.

- [ ] **Step 4: Commit (incluye los placeholders action_* generados)**
```
git add crates/ui/src/icons/assets.rs crates/ui/src/bin/gen_icons.rs assets/icons/flat/action_*.png assets/icons/fluent/action_*.png assets/icons/mono/action_*.png
git commit -m "feat(ui): claves action_* en las tablas de íconos (placeholders)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
(VERIFY no `.zip` is staged: `git status` — only PNGs + .rs.)

---

## Task 3: `core::config` — ToolbarIconStyle + glyph_color + icon_set→id string

**Files:**
- Modify: `crates/core/src/config/mod.rs`
- Modify: `crates/ui/src/icons/mod.rs` (IconProvider takes id — minimal shim here)
- Modify: `crates/ui/src/settings_window/appearance.rs`
- Modify: `crates/ui/src/app.rs`

This is the cross-cutting task: `Settings.icon_set` becomes a `String` id. The `IconSet` enum stays for the embedded sets (assets.rs). A helper maps embedded id ↔ enum.

- [ ] **Step 1: Tests (TDD) en config**

Add to config tests:
```rust
    #[test]
    fn toolbar_defaults_y_round_trip() {
        let mut s = Settings::default();
        assert_eq!(s.toolbar_icon_style, ToolbarIconStyle::Glyphs);
        assert!(s.toolbar_glyph_color.is_none());
        s.toolbar_icon_style = ToolbarIconStyle::Pack;
        s.toolbar_glyph_color = Some(crate::theme::ThemeColor::new(0xe0, 0xa0, 0x30));
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.toolbar_icon_style, ToolbarIconStyle::Pack);
        assert_eq!(back.toolbar_glyph_color, s.toolbar_glyph_color);
    }

    #[test]
    fn icon_set_id_default_es_flat() {
        let s = Settings::default();
        assert_eq!(s.icon_set, "flat");
    }

    #[test]
    fn icon_set_migra_del_enum_viejo() {
        // Un settings.json viejo serializaba el enum: "icon_set":"Flat".
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"icon_set":"Flat"}"#,
        ).unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.icon_set, "flat");
    }

    #[test]
    fn icon_set_id_desconocido_cae_a_flat() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"icon_set":"no-existe-xyz"}"#,
        ).unwrap();
        let s = load_settings(dir.path());
        // id desconocido se normaliza a flat al cargar (o queda y el catálogo lo resuelve;
        // ver Step 3 — aquí esperamos "flat" tras la normalización en load_settings).
        assert_eq!(s.icon_set, "flat");
    }
```
NOTE: update the EXISTING tests `settings_default_tiene_iconos_flat…` (`s.icon_set == IconSet::Flat` → `s.icon_set == "flat"`) and the `IconSet::Mono` test (~356) (→ `"mono"`), and `settings_v1_sin_idioma_cae_a_default` keeps its `"icon_set":"Flat"` JSON (migration handles it). Fix those to compile.

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core config` → ERROR (ToolbarIconStyle missing; icon_set type mismatch).

- [ ] **Step 3: Implementar en config/mod.rs**

a) Enum:
```rust
/// Estilo de los íconos de la barra de herramientas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolbarIconStyle {
    /// Glifos Unicode (liviano; default).
    Glyphs,
    /// Íconos del set activo (pack).
    Pack,
}
```
b) `Settings`: change `pub icon_set: IconSet` → `pub icon_set: String`. Keep its `#[serde(default = "default_icon_set")]`. Add:
```rust
    #[serde(default = "default_toolbar_icon_style")]
    pub toolbar_icon_style: ToolbarIconStyle,
    #[serde(default)]
    pub toolbar_glyph_color: Option<crate::theme::ThemeColor>,
```
c) Defaults:
```rust
fn default_icon_set() -> String { "flat".to_string() }
fn default_toolbar_icon_style() -> ToolbarIconStyle { ToolbarIconStyle::Glyphs }
```
(`toolbar_glyph_color` uses `#[serde(default)]` → None.)
d) Manual `impl Default`: `icon_set: "flat".into(), toolbar_icon_style: ToolbarIconStyle::Glyphs, toolbar_glyph_color: None,`.
e) **Migración** del enum viejo + normalización: in `load_settings`, after reading the Settings, normalize `icon_set`: if it's the capitalized enum form (`"Flat"/"Fluent"/"Mono"`) map to `"flat"/"fluent"/"mono"`; the resulting id is left as-is otherwise. Since serde will deserialize `"icon_set":"Flat"` into the String field as `"Flat"` (a plain string), add a normalize step:
```rust
// En load_settings, tras obtener `s`:
s.icon_set = normalize_icon_set_id(&s.icon_set);
```
```rust
/// Normaliza el id de set: acepta el enum viejo capitalizado y los ids conocidos;
/// cualquier otro id (incl. un suelto inexistente) cae a "flat". Los packs sueltos
/// válidos se validan contra el catálogo en la UI; aquí solo migramos el formato viejo.
fn normalize_icon_set_id(id: &str) -> String {
    match id {
        "Flat" | "flat" => "flat".to_string(),
        "Fluent" | "fluent" => "fluent".to_string(),
        "Mono" | "mono" => "mono".to_string(),
        other => other.to_string(), // un id suelto se conserva; la UI valida contra el catálogo
    }
}
```
WAIT — the test `icon_set_id_desconocido_cae_a_flat` expects unknown→"flat". But a loose pack id is ALSO unknown to `normalize_icon_set_id`. Resolution: `normalize` keeps unknown ids as-is (a loose pack), and the UI (Task 6) falls back to "flat" if the id isn't in the catalog at load time. So CHANGE that test to expect the id preserved, OR have the catalog do the fallback. SIMPLER + matches spec ("id desconocido → flat" happens via the catalog/IconProvider, not load_settings): make `normalize_icon_set_id` only migrate the capitalized enum forms and lowercase them; keep everything else. Then in the test `icon_set_id_desconocido_cae_a_flat`, the id stays "no-existe-xyz" after load, and the FALLBACK to flat happens when IconProvider can't find it. UPDATE that test to assert the catalog/provider resolves unknown→flat instead (move it to Task 4/5 where the catalog exists), OR keep load_settings normalizing unknown→flat but then a loose pack id would wrongly reset. DECISION: load_settings only migrates the 3 enum forms; unknown ids are preserved; the "unknown→flat" guarantee is the catalog's job (Task 4). REMOVE `icon_set_id_desconocido_cae_a_flat` from this task (it belongs to Task 4's catalog tests). Keep the migration test (`"Flat"→"flat"`) and the default test here.

- [ ] **Step 4: Repuntar los call sites del enum (compilan)**

- `crates/ui/src/settings_window/appearance.rs`: the 3 `selectable_value(&mut app.settings.icon_set, IconSet::X, ...)` now compare a `String`. TEMPORARY shim to keep it compiling THIS task (Task 6 rewrites it to the catalog): change to `selectable_value(&mut app.settings.icon_set, "flat".to_string(), l_flat)` etc. (selectable_value works on `String` with `==`). Remove `use ...IconSet;` if now unused.
- `crates/ui/src/app.rs`: `IconProvider::new(&cc.egui_ctx, settings.icon_set)` (~317) and `self.icons.reload(ui.ctx(), set)` (~2263) now pass a `String`/`&str`. Add a minimal `IconProvider::new(ctx, set_id: &str, config_dir)` shim — but that's Task 4. To keep THIS task compiling, do the minimal: change `IconProvider`'s `new`/`reload` to take `set_id: &str` and internally map to the enum (Task 4 adds the loose-pack path). i.e. a tiny version of Task 4's signature now. SEE Step 5.

- [ ] **Step 5: IconProvider acepta id string (shim mínimo, embebidos)**

Modify `crates/ui/src/icons/mod.rs` minimally so it compiles with a string id, embedded-only for now (Task 4 adds loose packs + config_dir):
```rust
/// Mapea un id de set embebido a su enum; ids desconocidos → Flat (fallback).
fn embedded_set(id: &str) -> naygo_core::config::IconSet {
    use naygo_core::config::IconSet;
    match id {
        "fluent" => IconSet::Fluent,
        "mono" => IconSet::Mono,
        _ => IconSet::Flat,
    }
}
```
Change `IconProvider { set: IconSet, ... }` to store `set_id: String`, and `new(ctx, set_id: &str)` / `reload(ctx, set_id: &str)` use `embedded_set(set_id)` to load via `assets::bytes_for`. `set()` returns `&str`. Update app.rs call sites to pass `&settings.icon_set` / `&set_id`.
(Task 4 will add `config_dir` + the loose-pack branch; for now embedded-only via the mapper. This keeps the tree compiling after the type change.)

- [ ] **Step 6: Verificar**

Run: `cargo build --workspace` → compiles. `cargo test --workspace` → green (config migration tests; the icons assets tests). `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.

- [ ] **Step 7: Commit**
```
git add crates/core/src/config/mod.rs crates/ui/src/icons/mod.rs crates/ui/src/settings_window/appearance.rs crates/ui/src/app.rs
git commit -m "feat(core): ToolbarIconStyle + toolbar_glyph_color; icon_set como id (migración del enum)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `ToolbarIconStyle`/`toolbar_glyph_color`/`icon_set: String` EXACTOS (Tasks 4-7 depend).

---

## Task 4: `core::icon_set::IconSetCatalog` — embebidos + sueltos

**Files:**
- Create: `crates/core/src/icon_set.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear icon_set.rs con tests (TDD)**

Create `crates/core/src/icon_set.rs`:
```rust
// Naygo — catálogo de sets de íconos: embebidos + packs sueltos del usuario.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lista los sets de íconos disponibles: los 3 embebidos (flat/fluent/mono) más los
//! packs sueltos descubiertos en `<config_dir>/icons/<nombre>/`. Patrón análogo a
//! `theme::ThemeCatalog`. Puro salvo el `read_dir` de descubrimiento.

use std::path::Path;

/// Un set de íconos disponible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconSetInfo {
    /// Id estable (flat/fluent/mono o el nombre de la carpeta suelta).
    pub id: String,
    /// Etiqueta a mostrar (para embebidos, capitalizada; para sueltos, el nombre).
    pub label: String,
    /// `true` si es uno de los 3 embebidos.
    pub builtin: bool,
}

/// Catálogo de sets disponibles.
pub struct IconSetCatalog {
    sets: Vec<IconSetInfo>,
}

impl IconSetCatalog {
    /// Construye el catálogo: 3 embebidos + sueltos de `<dir>/icons/<nombre>/`.
    /// Tolerante: si `read_dir` falla, solo los embebidos.
    pub fn load(dir: &Path) -> IconSetCatalog {
        let mut sets = vec![
            IconSetInfo { id: "flat".into(), label: "Flat".into(), builtin: true },
            IconSetInfo { id: "fluent".into(), label: "Fluent".into(), builtin: true },
            IconSetInfo { id: "mono".into(), label: "Mono".into(), builtin: true },
        ];
        let icons_dir = dir.join("icons");
        if let Ok(entries) = std::fs::read_dir(&icons_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        // No duplicar un id embebido.
                        if !["flat", "fluent", "mono"].contains(&name) {
                            sets.push(IconSetInfo {
                                id: name.to_string(),
                                label: name.to_string(),
                                builtin: false,
                            });
                        }
                    }
                }
            }
        }
        IconSetCatalog { sets }
    }

    /// Los sets disponibles (embebidos primero, luego sueltos en orden de descubrimiento).
    pub fn available(&self) -> &[IconSetInfo] {
        &self.sets
    }

    /// ¿Existe un set con este id?
    pub fn contains(&self, id: &str) -> bool {
        self.sets.iter().any(|s| s.id == id)
    }

    /// Resuelve un id a uno válido: si no existe, cae a "flat".
    pub fn resolve(&self, id: &str) -> String {
        if self.contains(id) {
            id.to_string()
        } else {
            "flat".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embebidos_siempre_presentes() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert!(cat.contains("flat") && cat.contains("fluent") && cat.contains("mono"));
        assert_eq!(cat.available().len(), 3);
    }

    #[test]
    fn descubre_packs_sueltos() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons").join("mi-pack")).unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert!(cat.contains("mi-pack"));
        let info = cat.available().iter().find(|s| s.id == "mi-pack").unwrap();
        assert!(!info.builtin);
    }

    #[test]
    fn icons_dir_ausente_solo_embebidos() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(cat.available().len(), 3);
    }

    #[test]
    fn resolve_desconocido_cae_a_flat() {
        let dir = tempfile::tempdir().unwrap();
        let cat = IconSetCatalog::load(dir.path());
        assert_eq!(cat.resolve("no-existe"), "flat");
        assert_eq!(cat.resolve("mono"), "mono");
    }

    #[test]
    fn pack_suelto_no_duplica_id_embebido() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("icons").join("flat")).unwrap();
        let cat = IconSetCatalog::load(dir.path());
        // "flat" sigue siendo uno solo (embebido).
        assert_eq!(cat.available().iter().filter(|s| s.id == "flat").count(), 1);
    }
}
```

- [ ] **Step 2: Declarar el módulo**

Modify `crates/core/src/lib.rs`: add `pub mod icon_set;`.

- [ ] **Step 3: Verificar + commit**

Run: `cargo test -p naygo-core icon_set` → 5 PASS. `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.
```
git add crates/core/src/icon_set.rs crates/core/src/lib.rs
git commit -m "feat(core): IconSetCatalog (embebidos + packs sueltos)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `IconSetCatalog`/`IconSetInfo`/`load`/`available`/`contains`/`resolve` EXACTOS (Tasks 5-6 depend).

---

## Task 5: `IconProvider` — cargar sets sueltos desde disco

**Files:**
- Modify: `crates/ui/src/icons/mod.rs`
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: IconProvider carga embebido o suelto por id**

Modify `crates/ui/src/icons/mod.rs`. `IconProvider::new`/`reload` gain `config_dir`. For an embedded id → `assets::bytes_for(embedded_set(id), key)` (existing path). For a loose id → read `<config_dir>/icons/<id>/<file_name(key)>.png` from disk; if missing/unreadable → fallback (unknown of "flat"). Concretely:
```rust
impl IconProvider {
    pub fn new(ctx: &egui::Context, set_id: &str, config_dir: &std::path::Path) -> Self {
        let loader = TextureLoader { set_id: set_id.to_string(), config_dir: config_dir.to_path_buf() };
        let fallback = loader.load(ctx, IconKey::Unknown);
        let mut textures = HashMap::new();
        for key in assets::all_keys() {
            textures.insert(key, loader.load(ctx, key));
        }
        IconProvider { set_id: set_id.to_string(), textures, fallback }
    }

    pub fn reload(&mut self, ctx: &egui::Context, set_id: &str, config_dir: &std::path::Path) {
        if set_id == self.set_id {
            return;
        }
        *self = IconProvider::new(ctx, set_id, config_dir);
    }

    pub fn set(&self) -> &str { &self.set_id }

    pub fn texture(&self, key: IconKey) -> &egui::TextureHandle {
        self.textures.get(&key).unwrap_or(&self.fallback)
    }
}

struct TextureLoader { set_id: String, config_dir: std::path::PathBuf }
impl TextureLoader {
    fn load(&self, ctx: &egui::Context, key: IconKey) -> egui::TextureHandle {
        let color_image = self.color_image_for(key).unwrap_or_else(|| {
            egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0])
        });
        let name = format!("icon_{}_{:?}", self.set_id, key);
        ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
    }
    fn color_image_for(&self, key: IconKey) -> Option<egui::ColorImage> {
        let builtin = matches!(self.set_id.as_str(), "flat" | "fluent" | "mono");
        if builtin {
            decode_png(assets::bytes_for(embedded_set(&self.set_id), key))
        } else {
            // Pack suelto: leer <config>/icons/<id>/<file_name>.png; fallback al unknown embebido.
            let path = self
                .config_dir
                .join("icons")
                .join(&self.set_id)
                .join(format!("{}.png", assets::file_name(key)));
            let bytes = std::fs::read(&path).ok();
            match bytes.as_deref().and_then(decode_png) {
                Some(img) => Some(img),
                None => decode_png(assets::bytes_for(naygo_core::config::IconSet::Flat, IconKey::Unknown)),
            }
        }
    }
}
```
NOTE: `assets::file_name` must be `pub` (it likely is `pub(crate)` or private — make it `pub` in assets.rs so the loader can build the loose path; or expose a small helper). `decode_png` already exists (make it usable here). `embedded_set` from Task 3. Adapt to the real `assets` visibility — keep it minimal.

- [ ] **Step 2: Actualizar app.rs call sites**

`IconProvider::new(&cc.egui_ctx, &settings.icon_set, &config_dir)` (~317) — `config_dir` is in scope there (it's loaded before). `self.icons.reload(ui.ctx(), &set_id, &self.config_dir)` (~2263) — pass the new set id + config_dir. (The `set` var there comes from the appearance selector; it's now a String.)

- [ ] **Step 3: Verificar**

Run: `cargo build --workspace` → compiles. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.
MANUAL (optional): a loose pack works — create `<config>/icons/test/folder.png` (any PNG), select it; folder icon comes from disk, others fall back. (Headless OK to skip; logic is covered.)

- [ ] **Step 4: Commit**
```
git add crates/ui/src/icons/mod.rs crates/ui/src/icons/assets.rs crates/ui/src/app.rs
git commit -m "feat(ui): IconProvider carga sets embebidos o packs sueltos por id

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: UI — appearance: selector de set del catálogo + estilo toolbar + color-picker

**Files:**
- Modify: `crates/ui/src/settings_window/appearance.rs`
- Modify: `crates/ui/src/app.rs` (IconSetCatalog en NaygoApp si conviene)
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: i18n (ambos, idénticas)**

ES: `"settings.toolbar.section": "Barra de herramientas"`, `"settings.toolbar.style": "Estilo de íconos"`, `"settings.toolbar.glyphs": "Glifos"`, `"settings.toolbar.pack": "Pack de íconos"`, `"settings.toolbar.glyph_color": "Color de los glifos"`, `"settings.toolbar.use_theme_color": "Usar color del tema"`.
EN: `"settings.toolbar.section": "Toolbar"`, `"settings.toolbar.style": "Icon style"`, `"settings.toolbar.glyphs": "Glyphs"`, `"settings.toolbar.pack": "Icon pack"`, `"settings.toolbar.glyph_color": "Glyph color"`, `"settings.toolbar.use_theme_color": "Use theme color"`.
(READ both json; keep keys identical — parity test.)

- [ ] **Step 2: appearance.rs — set selector del catálogo + estilo + color**

Modify `crates/ui/src/settings_window/appearance.rs`:
a) Set selector: replace the 3 hardcoded `selectable_value(IconSet::X)` with a loop over `IconSetCatalog::load(&app.config_dir).available()` (or a cached catalog on NaygoApp). For each `info`, `selectable_value(&mut app.settings.icon_set, info.id.clone(), &info.label)`. When the selection changes, reload the IconProvider (the app already reloads on change — verify; if reload is driven elsewhere by comparing `icons.set() != settings.icon_set`, ensure that comparison still triggers). Use the builtin labels via i18n (`settings.icons.flat/fluent/mono`) and loose labels as-is.
b) Toolbar style section: heading `settings.toolbar.section`; a segmented selector `settings.toolbar.glyphs`/`settings.toolbar.pack` via `selectable_value(&mut app.settings.toolbar_icon_style, ToolbarIconStyle::Glyphs/Pack, label)`.
c) Glyph color (shown when style is Glyphs, or always): a `color_edit_button_srgba`. Since `toolbar_glyph_color` is `Option<ThemeColor>`, present: a checkbox/button "usar color del tema" that sets it to `None`; when Some, a `color_edit_button_srgba(&mut color32)` editing the color (convert ThemeColor↔Color32 via `theme_apply::to_color32` and back). Concretely:
```rust
    // Color de glifos.
    let mut use_theme = app.settings.toolbar_glyph_color.is_none();
    if ui.checkbox(&mut use_theme, app.tr("settings.toolbar.use_theme_color")).changed() {
        app.settings.toolbar_glyph_color = if use_theme {
            None
        } else {
            Some(naygo_core::theme::ThemeColor::new(0x2f, 0x81, 0xf7)) // arranca con el acento
        };
    }
    if let Some(tc) = app.settings.toolbar_glyph_color {
        let mut c = egui::Color32::from_rgb(tc.r, tc.g, tc.b); // ajustar a los campos reales de ThemeColor
        if ui.color_edit_button_srgba(&mut c).changed() {
            app.settings.toolbar_glyph_color =
                Some(naygo_core::theme::ThemeColor::new(c.r(), c.g(), c.b()));
        }
    }
```
VERIFY `ThemeColor` field/accessor names (r/g/b? a `to_rgb()`? a `from_hex`/`new(r,g,b)`?) — adapt. `color_edit_button_srgba(&mut Color32)` is the egui 0.34 API; verify.

- [ ] **Step 3: Verificar**

Run: `cargo build --workspace`; `cargo test --workspace` → green (i18n parity); `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all`.

- [ ] **Step 4: Commit**
```
git add crates/ui/src/settings_window/appearance.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): Configuración — set de íconos del catálogo + estilo toolbar + color de glifos

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: UI — toolbar render glifos/pack

**Files:**
- Modify: `crates/ui/src/toolbar.rs`

- [ ] **Step 1: icon_button bifurca glifos/pack**

Modify `crates/ui/src/toolbar.rs`. Each toolbar action needs: its glyph (current), its `ActionIcon`, its tooltip, enabled. Change `icon_button` to take both glyph + ActionIcon + the render context (style, color, icons, theme, set-is-mono). Concretely, replace the per-button `icon_button(ui, "◀", &lbl, can_back)` calls with a helper that knows the style:
```rust
fn icon_button(
    ui: &mut egui::Ui,
    glyph: &str,
    action: naygo_core::icon_kind::ActionIcon,
    tip: &str,
    enabled: bool,
    style: naygo_core::config::ToolbarIconStyle,
    glyph_color: egui::Color32,
    icons: &crate::icons::IconProvider,
    tint_mono: Option<egui::Color32>,
) -> bool {
    use naygo_core::config::ToolbarIconStyle;
    let resp = match style {
        ToolbarIconStyle::Glyphs => {
            let txt = egui::RichText::new(glyph).color(glyph_color).size(16.0);
            ui.add_enabled(enabled, egui::Button::new(txt))
        }
        ToolbarIconStyle::Pack => {
            let tex = icons.texture(naygo_core::icon_kind::IconKey::Action(action));
            let mut img = egui::Image::new(tex).fit_to_exact_size(egui::vec2(18.0, 18.0));
            if let Some(c) = tint_mono {
                img = img.tint(c);
            }
            ui.add_enabled(enabled, egui::Button::image(img))
        }
    };
    resp.on_hover_text(tip).clicked()
}
```
And the `buttons(ui, app)` fn computes once: `let style = app.settings.toolbar_icon_style;`, `let glyph_color = app.settings.toolbar_glyph_color.map(to_color32).unwrap_or_else(|| app.active_theme.accent());`, `let tint_mono = if app.icons.set() == "mono" { Some(app.active_theme.text_or_accent_color) } else { None };` (use the theme color used for mono tint — accent or text; pick one, e.g. text color via the active theme). Then each button passes its glyph + ActionIcon:
- `◀`/Back, `▶`/Forward, `▲`/Up, `⟳`/Refresh, `⧉`/Copy, `✂`/Cut, `📋`/Paste, `🗑`/Delete, `🗋`/NewFile, `🗀`/NewFolder, `➕`/AddPane, `⚙`/Settings.
VERIFY egui 0.34.3: `egui::Button::new(RichText)`, `egui::Button::image(Image)`, `egui::Image::new(&TextureHandle).fit_to_exact_size(vec2).tint(Color32)`, `add_enabled`. Adapt. Where `app.active_theme`/`app.icons` are accessed — `buttons` takes `&mut NaygoApp`, so `app.icons` (the IconProvider) and `app.active_theme` are reachable; if `icon_button` needs `&icons` while `app` is borrowed, split the borrows (read style/color/set into locals, then pass `&app.icons`).

- [ ] **Step 2: Verificar**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all`.
MANUAL: toolbar in Glyphs shows colored glyphs (theme color, or custom); switch to Pack → shows the set's action icons; Mono set tinted by theme.

- [ ] **Step 3: Commit**
```
git add crates/ui/src/toolbar.rs
git commit -m "feat(ui): toolbar pinta glifos (color) o íconos del pack (tinte Mono)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Assets reales — extraer de los zips (con fallback)

**Files:**
- Replace: `assets/icons/{flat,fluent,mono}/*.png` (los 26 por set con imágenes reales)
- Create: `assets/icons/NOTICE.md`

ESTA TAREA ES MANUAL Y DELICADA. Objetivo: reemplazar los placeholders por íconos reales de los zips. Tolerante: si un ícono no se logra, se deja el placeholder (la app no se rompe). NUNCA commitear un `.zip`.

- [ ] **Step 1: Descomprimir los 3 zips livianos a un TEMP fuera del repo**

PowerShell (a una carpeta temporal, NO dentro del repo):
```
$tmp = "$env:TEMP\naygo-icons"
Expand-Archive -Path "D:\Empresas\ISGroth\explorador_de_archivos\assets\icons\flat-color-icons-master.zip" -DestinationPath "$tmp\flat" -Force
Expand-Archive -Path "D:\Empresas\ISGroth\explorador_de_archivos\assets\icons\lucide-main.zip" -DestinationPath "$tmp\lucide" -Force
Expand-Archive -Path "D:\Empresas\ISGroth\explorador_de_archivos\assets\icons\fluentui-emoji-main.zip" -DestinationPath "$tmp\fluent" -Force
```
Then EXPLORE each extracted tree (Glob/Read) to find the SVG/PNG files. flat-color-icons: `*/svg/*.svg` (flat multicolor). lucide: `*/icons/*.svg` (line, monochrome, stroke=currentColor). fluentui-emoji: `*/assets/<Name>/Color/*.svg` or `.png` (emoji). Map each of the 26 names (13 type + 12 action) to the best-matching source file per set. NOTE: lucide/flat are SVG → must rasterize to PNG ~32px; fluentui has PNGs (use the 32px or 48px variants).

- [ ] **Step 2: Rasterizar/copiar a assets/icons/<set>/<name>.png**

For SVGs (flat, lucide): rasterize to 32×32 PNG. Use a small throwaway Rust step OR the `resvg`/`usvg` CLI if available OR ImageMagick (`magick convert -background none -resize 32x32 in.svg out.png`) — pick whatever is available on this machine; verify with one icon first. For Mono (lucide), the line icons are monochrome (will be tinted at paint time) — rasterize with a neutral color (e.g. white/light gray, like the current mono placeholders, so the tint works). For Fluent PNGs, resize to 32px.
Map the names (examples; choose the closest icon in each pack):
- Type: folder, drive, unknown, file_image, file_video, file_audio, file_document, file_code, file_archive, file_executable, file_model3d, file_font, file_generic.
- Action: action_back(arrow-left), action_forward(arrow-right), action_up(arrow-up / corner-up), action_refresh(refresh-cw), action_copy(copy), action_cut(scissors), action_paste(clipboard), action_delete(trash), action_new_file(file-plus), action_new_folder(folder-plus), action_add_pane(columns / plus), action_settings(settings/gear).
Write each to `assets/icons/<set>/<name>.png`, OVERWRITING the placeholder. If a given icon can't be found/rasterized cleanly, LEAVE the placeholder for that one (don't break the build) and note it.

- [ ] **Step 3: NOTICE de licencias**

Create `assets/icons/NOTICE.md`:
```markdown
# Atribución de íconos

Los íconos embebidos en Naygo provienen de proyectos de código abierto, usados bajo
sus licencias (todas permisivas, compatibles con la licencia MIT de Naygo):

- **Flat (set "flat")** — Flat Color Icons — https://github.com/icons8/flat-color-icons — MIT.
- **Fluent (set "fluent")** — Fluent UI Emoji (Microsoft) — https://github.com/microsoft/fluentui-emoji — MIT.
- **Mono (set "mono")** — Lucide — https://github.com/lucide-icons/lucide — ISC.

Solo se versionan los PNG extraídos en `assets/icons/{flat,fluent,mono}/`. Los archivos
comprimidos originales NO se incluyen en el repositorio.
```

- [ ] **Step 4: Verificar (build con los PNGs reales + zips NO staged)**

Run: `cargo build --workspace` → compiles (include_bytes! sigue resolviendo los nombres).
Run: `cargo test --workspace` → green (los tests de assets verifican que cada (set,key) tiene bytes; los PNGs reales los satisfacen).
Run: `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all -- --check`.
CRÍTICO: `git status` — confirmar que NINGÚN `.zip` está staged ni nuevo-trackeable; solo PNGs + NOTICE.md. (`git check-ignore assets/icons/material-design-icons-master.zip` debe imprimir la ruta = ignorado.)
MANUAL: lanzar la app, set Pack, ver íconos reales en toolbar + tipos de archivo; cambiar entre flat/fluent/mono.

- [ ] **Step 5: Commit (PNGs reales + NOTICE; NUNCA zips)**
```
git add assets/icons/flat/ assets/icons/fluent/ assets/icons/mono/ assets/icons/NOTICE.md
git commit -m "assets: íconos reales (flat-color/fluentui/lucide) reemplazan placeholders

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
Report exactly which icons (if any) couldn't be extracted and stayed as placeholders.

---

## Task 9: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README**

READ el bloque de estado y reemplazar:
```markdown
> **Estado:** Fase toolbar-icons (estilo de toolbar + íconos reales + packs sueltos) en
> desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-08-naygo-toolbar-icons-design.md`](docs/superpowers/specs/2026-06-08-naygo-toolbar-icons-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-08-naygo-toolbar-icons.md`](docs/superpowers/plans/2026-06-08-naygo-toolbar-icons.md).
> Operaciones (ops-A/B), paste, shell-A, watcher, atajos, sizing y bloque visual completos.
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compiles. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all -- --check` → clean. `cargo build --release -p naygo-ui` → release compiles.
CRÍTICO: `git status` limpio salvo lo intencional; ningún `.zip` trackeado (`git ls-files assets/icons/*.zip` vacío).
MANUAL end-to-end: Glifos (color tema/custom) / Pack; cambiar de set incl. uno suelto; íconos reales.

- [ ] **Step 3: Commit y push**
```
git add README.md
git commit -m "chore: actualizar estado del README (fase toolbar-icons)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/toolbar-icons
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| ActionIcon (12) + IconKey::Action + file_name | 1 |
| assets file_name brazo Action + tablas action_* | 2 |
| ToolbarIconStyle + toolbar_glyph_color | 3 |
| icon_set enum→id string + migración | 3 |
| IconSetCatalog (embebidos + sueltos) | 4 |
| IconProvider por id (embebido/suelto) | 5 |
| appearance: selector del catálogo + estilo + color-picker | 6 |
| toolbar render glifos(color)/pack(tint Mono) | 7 |
| assets reales de los zips (3 sets) + NOTICE | 8 |
| i18n | 6 |
| zips NO commiteados | 2,8,9 (git status checks) |
| material-design/tabler/vscode, tinte Flat/Fluent, shell-B FUERA | (no se tocan) |

**Notas de riesgo:**
- **icon_set enum→String** (Task 3): contenido manteniendo `IconSet` enum interno para embebidos + `embedded_set(id)`; los call sites (appearance/app/IconProvider) se repuntan; migración del enum viejo en load_settings; los tests del enum se actualizan al id. El test "desconocido→flat" se movió a IconSetCatalog (Task 4), no a load_settings.
- **IconKey::Action no-exhaustivo** (Task 1/2): `assets::file_name` gana el brazo; build lo verifica.
- **include_bytes! antes de los PNGs reales** (Task 2): se generan placeholders `action_*` primero (gen_icons) para que compile; Task 8 los reemplaza.
- **Extracción de los zips** (Task 8): manual, tolerante (ícono no logrado → placeholder, build no rompe). VERIFICAR con `git status`/`git check-ignore` que ningún `.zip` se commitee. Rasterizar SVG→PNG con la herramienta disponible (resvg/ImageMagick/un paso Rust); probar con un ícono primero.
- **Tinte Mono** (Task 7): solo si `icons.set() == "mono"`; Flat/Fluent sin tinte. El color de glifos = override o acento del tema.
- **ThemeColor API** (Task 6): verificar campos (r/g/b o to_rgb/from_hex) + `color_edit_button_srgba`/`Image::tint`/`fit_to_exact_size` en egui 0.34.3.
- **i18n parity** (Task 6): claves `settings.toolbar.*` en AMBOS json.
```
