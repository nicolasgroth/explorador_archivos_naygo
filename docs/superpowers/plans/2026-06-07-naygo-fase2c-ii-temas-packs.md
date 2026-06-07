# Fase 2C-ii — Temas / color sets / packs — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sistema de temas (color sets) intercambiables en caliente con paleta propia completa, 4 temas de fábrica + temas/packs soltables (patrón i18n), selector de tarjetas en Configuración, y el acento de 2E saliendo del tema activo.

**Architecture:** El tema es DATOS en `naygo-core` (`theme` module: `Theme`/`ThemeColor`/`ThemeCatalog`, puro y testeado, mismo patrón que i18n). La `ui` traduce `Theme`→`egui::Visuals` y lo aplica al `Context` (`theme_apply.rs`), con un `ActiveTheme` compartido que los paneles leen (como `icons`/`i18n`). Hot-swap por frame igual que el icon set.

**Tech Stack:** Rust, `naygo-core` / `naygo-ui`, `serde`/`serde_json`, `eframe`/`egui` 0.34.3. Sin dependencias nuevas.

**Estado de partida (rama base `feat/fase2c-ii-temas-packs`, desde `main`):**
- `naygo_core::config::Settings { version, bar_position, icon_only, icon_set: IconSet, show_parent_entry, language: LangId }`. Campos aditivos usan `#[serde(default = "fn")]`. `IconSet { Flat, Fluent, Mono }`. `default_icon_set/show_parent_entry/language` helpers. `config::{portable_dir() -> PathBuf, load_settings(&Path) -> Settings, save_settings(&Path, &Settings)}`.
- `naygo_core::i18n` PATRÓN A IMITAR: `LangId(pub String)` con `::new(&str)`/`as_str()`. `I18n::load(dir: &Path, lang: &LangId)`: inserta catálogos embebidos (`include_str!("es.json")`/en), luego `read_dir(dir.join("lang"))` mergea `*.json` sueltos (id = file_stem), arma `available: Vec<LangId>` ordenado, resuelve `active` con fallback. `set_language`, `active_lang`, `available()`. Tests en `#[cfg(test)]` usan `tempfile`.
- `naygo_core::lib`: `pub mod i18n; ...`; re-exports `pub use i18n::{pick_default_language, I18n, LangId};`.
- `naygo-ui::app::NaygoApp`: tiene `icons: IconProvider`, `i18n: I18n`, `settings: Settings`. `new(cc)` carga settings/i18n/icons. `ui()` (~line 610): hot-swap de íconos `if self.icons.set() != self.settings.icon_set { self.icons.reload(ui.ctx(), set); }`; y de idioma `if self.i18n.active_lang() != self.settings.language { self.i18n.set_language(...); }`. Construye `NaygoTabViewer { workspace, status, pending, icons, show_parent_entry, i18n, trees, tree_actions, tree_revealed, table_actions }` y corre `DockArea`.
- `naygo-ui::settings_window::appearance::show(ui, app)`: ya tiene un placeholder de tema: `ui.label(app.tr("settings.theme")); let placeholder = app.tr("settings.theme.placeholder"); ui.label(RichText::new(placeholder).weak());` — ESTO se reemplaza por el selector de tarjetas.
- `naygo-ui` colores hardcoded a reemplazar (acento `0x2f81f7`): `docking.rs` `title()` (panel activo); `panes/file_panel.rs` (línea de drop `0x2f81f7`; ≡ filtro; `row.set_selected` usa el visuals de egui — ese saldrá del tema automáticamente al setear `selection.bg_fill`); `panes/tree_panel.rs` (resaltado nodo activo: fondo `0x37,0x37,0x3d` + barra `0x3b82f6`/`0x2f81f7`; error `0xe0..`).
- `naygo-ui::app::NaygoApp::tr(key) -> String`, `i18n_available()`. `cc.egui_ctx` disponible en `new`.

**Prerequisito:** toolchain Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo en PowerShell. Binario `--bin naygo`. Verificar `$LASTEXITCODE`.

**Convenciones (CLAUDE.md):** código en inglés; comentarios/commits en español OK. Header de 2 líneas en archivos NUEVOS. `core` NUNCA importa egui/windows. Build limpio + tests + clippy antes de cada commit. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/fase2c-ii-temas-packs`. NO cambiar de rama.

**Alcance:** ENTRA: `core::theme` (Theme/ThemeColor/ThemeCatalog + 4 embebidos + sueltos), `core::theme::pack` (Pack/PackCatalog + embebidos + sueltos), `Settings.theme`, `ui::theme_apply` (Visuals + ActiveTheme), selector de tarjetas + packs en Apariencia, hot-swap, reemplazar hardcoded de 2E, i18n. NO ENTRA: recolorear íconos, editor in-app, selector de acento aparte, validación de contraste.

---

## Estructura de archivos

```
crates/core/src/
├── theme/
│   ├── mod.rs                # ThemeColor, ThemeBase, ThemeId, Theme, ThemeCatalog
│   ├── pack.rs               # Pack, PackCatalog
│   ├── builtin/{dark-blue,dark-teal,light,high-contrast}.json
│   └── packs/{dark-blue,dark-teal,light,high-contrast}.json
├── config/mod.rs             # + Settings.theme (serde default)
├── lib.rs                    # + pub mod theme; re-exports
└── i18n/{es,en}.json         # + claves UI del selector

crates/ui/src/
├── theme_apply.rs            # NUEVO: ActiveTheme, to_color32, apply(Theme,&Context)
├── settings_window/appearance.rs  # selector de tarjetas (tema) + sección packs
├── app.rs                    # + ActiveTheme + hot-swap del tema; pasar &ActiveTheme a paneles
├── docking.rs                # title() panel activo usa accent del tema
├── panes/file_panel.rs       # línea de drop / ≡ usan tokens del tema
├── panes/tree_panel.rs       # resaltado nodo activo / error usan tokens del tema
└── main.rs                   # + mod theme_apply;
```

---

## Task 1: `core::theme` — ThemeColor (hex) + ThemeBase + ThemeId

**Files:**
- Create: `crates/core/src/theme/mod.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear `theme/mod.rs` con ThemeColor/ThemeBase/ThemeId + tests**

Create `crates/core/src/theme/mod.rs`:

```rust
// Naygo — sistema de temas (color sets): datos puros, sin egui ni Windows.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Un `Theme` es una paleta de colores con nombre + base claro/oscuro. Los temas se
//! cargan de JSON embebidos y de archivos sueltos (patrón i18n), son tolerantes a
//! campos faltantes, y se aplican en caliente. Puro y testeable; la traducción a
//! `egui::Visuals` vive en la capa `ui`.

use serde::{Deserialize, Serialize};

/// Base visual de la que se derivan los neutros (la capa ui parte de
/// `Visuals::light()`/`dark()` según esto).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeBase {
    Light,
    Dark,
}

/// Un color RGB. Se serializa como cadena hex "#rrggbb".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ThemeColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        ThemeColor { r, g, b }
    }

    /// Parsea "#rrggbb" o "rrggbb" (case-insensitive). `None` si es inválido.
    pub fn from_hex(s: &str) -> Option<ThemeColor> {
        let h = s.strip_prefix('#').unwrap_or(s);
        if h.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&h[0..2], 16).ok()?;
        let g = u8::from_str_radix(&h[2..4], 16).ok()?;
        let b = u8::from_str_radix(&h[4..6], 16).ok()?;
        Some(ThemeColor { r, g, b })
    }

    /// Formatea como "#rrggbb".
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

// Serde: como cadena hex.
impl Serialize for ThemeColor {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<ThemeColor, D::Error> {
        let s = String::deserialize(d)?;
        ThemeColor::from_hex(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("color hex inválido: {s}")))
    }
}

/// Identificador estable de un tema (su nombre-clave, p. ej. el file_stem del JSON).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThemeId(pub String);

impl ThemeId {
    pub fn new(id: &str) -> Self {
        ThemeId(id.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_hex_con_y_sin_almohadilla() {
        assert_eq!(ThemeColor::from_hex("#2f81f7"), Some(ThemeColor::new(0x2f, 0x81, 0xf7)));
        assert_eq!(ThemeColor::from_hex("2f81f7"), Some(ThemeColor::new(0x2f, 0x81, 0xf7)));
        assert_eq!(ThemeColor::from_hex("#2F81F7"), Some(ThemeColor::new(0x2f, 0x81, 0xf7)));
    }

    #[test]
    fn from_hex_invalido_es_none() {
        assert_eq!(ThemeColor::from_hex("#xyz"), None);
        assert_eq!(ThemeColor::from_hex("#12345"), None);
        assert_eq!(ThemeColor::from_hex(""), None);
    }

    #[test]
    fn to_hex_round_trip() {
        let c = ThemeColor::new(0x12, 0xab, 0xff);
        assert_eq!(c.to_hex(), "#12abff");
        assert_eq!(ThemeColor::from_hex(&c.to_hex()), Some(c));
    }

    #[test]
    fn serde_color_es_cadena_hex() {
        let c = ThemeColor::new(0x2f, 0x81, 0xf7);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"#2f81f7\"");
        let back: ThemeColor = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn serde_base_lowercase() {
        assert_eq!(serde_json::to_string(&ThemeBase::Dark).unwrap(), "\"dark\"");
        let b: ThemeBase = serde_json::from_str("\"light\"").unwrap();
        assert_eq!(b, ThemeBase::Light);
    }
}
```

- [ ] **Step 2: Declarar el módulo + re-export**

Modify `crates/core/src/lib.rs`:
- Añadir `pub mod theme;` en orden alfabético (tras `pub mod sort;` y antes de `pub mod tree;` — READ para ubicar: el orden actual es `... sort; theme va entre sort y tree`). En realidad alfabético: `sort` < `theme` < `tree`, así que `pub mod theme;` va entre ellos.
- Re-export: `pub use theme::{Theme, ThemeBase, ThemeColor, ThemeId};` (Theme se define en Task 2; para esta tarea re-exporta solo lo que existe: `pub use theme::{ThemeBase, ThemeColor, ThemeId};` y se amplía en Task 2).

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core theme` → 5 tests PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/theme/mod.rs crates/core/src/lib.rs
git commit -m "feat(core): ThemeColor (hex) + ThemeBase + ThemeId

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `core::theme` — Theme (paleta) + tolerancia + ThemeCatalog + 4 embebidos

**Files:**
- Modify: `crates/core/src/theme/mod.rs`
- Create: `crates/core/src/theme/builtin/dark-blue.json`, `dark-teal.json`, `light.json`, `high-contrast.json`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear los 4 JSON de tema embebidos**

Create `crates/core/src/theme/builtin/dark-blue.json`:
```json
{
  "name": "Dark Blue",
  "base": "dark",
  "accent": "#2f81f7",
  "panel_bg": "#1e1e1e",
  "row_bg": "#1e1e1e",
  "row_alt_bg": "#232323",
  "text": "#d4d4d4",
  "text_dim": "#9b9b9b",
  "selection_bg": "#37373d",
  "active_bar": "#2f81f7",
  "error": "#e06c5b",
  "border": "#3a3a3a"
}
```
Create `crates/core/src/theme/builtin/dark-teal.json`:
```json
{
  "name": "Dark Teal",
  "base": "dark",
  "accent": "#1abc9c",
  "panel_bg": "#15201e",
  "row_bg": "#15201e",
  "row_alt_bg": "#1a2522",
  "text": "#cfe0db",
  "text_dim": "#9fb8b1",
  "selection_bg": "#21302c",
  "active_bar": "#1abc9c",
  "error": "#e06c5b",
  "border": "#25352f"
}
```
Create `crates/core/src/theme/builtin/light.json`:
```json
{
  "name": "Light",
  "base": "light",
  "accent": "#1769e0",
  "panel_bg": "#fbfbfb",
  "row_bg": "#fbfbfb",
  "row_alt_bg": "#f4f4f4",
  "text": "#1f1f1f",
  "text_dim": "#6b6b6b",
  "selection_bg": "#dcebff",
  "active_bar": "#1769e0",
  "error": "#c0392b",
  "border": "#dddddd"
}
```
Create `crates/core/src/theme/builtin/high-contrast.json`:
```json
{
  "name": "High Contrast",
  "base": "dark",
  "accent": "#ffb300",
  "panel_bg": "#000000",
  "row_bg": "#000000",
  "row_alt_bg": "#0c0c0c",
  "text": "#f2f2f2",
  "text_dim": "#bdbdbd",
  "selection_bg": "#2a2200",
  "active_bar": "#ffb300",
  "error": "#ff5252",
  "border": "#333333"
}
```

- [ ] **Step 2: Escribir los tests del Theme/Catalog (TDD)**

Añadir al `#[cfg(test)] mod tests` de `theme/mod.rs`:
```rust
    #[test]
    fn theme_round_trip_serde() {
        let t = Theme {
            name: "Test".into(),
            base: ThemeBase::Dark,
            accent: ThemeColor::new(1, 2, 3),
            panel_bg: ThemeColor::new(4, 5, 6),
            row_bg: ThemeColor::new(7, 8, 9),
            row_alt_bg: ThemeColor::new(10, 11, 12),
            text: ThemeColor::new(13, 14, 15),
            text_dim: ThemeColor::new(16, 17, 18),
            selection_bg: ThemeColor::new(19, 20, 21),
            active_bar: ThemeColor::new(22, 23, 24),
            error: ThemeColor::new(25, 26, 27),
            border: ThemeColor::new(28, 29, 30),
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn theme_tolera_campos_faltantes() {
        // Solo name + base; el resto cae al default del base (no falla).
        let json = r#"{ "name": "Min", "base": "dark" }"#;
        let t: Theme = Theme::from_json(json).expect("parsea con defaults");
        assert_eq!(t.name, "Min");
        assert_eq!(t.base, ThemeBase::Dark);
        // accent cae al default oscuro (no es transparente/cero arbitrario).
        // Verificamos que es ALGÚN color (no panic) — el default exacto lo fija el helper.
        let _ = t.accent;
    }

    #[test]
    fn catalog_tiene_los_cuatro_embebidos() {
        let cat = ThemeCatalog::load(std::path::Path::new("Z:/no/existe"), &ThemeCatalog::default_id());
        let ids: Vec<&str> = cat.available().iter().map(|i| i.as_str()).collect();
        for id in ["dark-blue", "dark-teal", "light", "high-contrast"] {
            assert!(ids.contains(&id), "falta el tema embebido {id}");
        }
    }

    #[test]
    fn catalog_default_es_dark_blue() {
        assert_eq!(ThemeCatalog::default_id(), ThemeId::new("dark-blue"));
    }

    #[test]
    fn catalog_get_id_desconocido_cae_al_default() {
        let cat = ThemeCatalog::load(std::path::Path::new("Z:/no/existe"), &ThemeCatalog::default_id());
        let t = cat.get(&ThemeId::new("no-existe"));
        // Debe devolver el default (dark-blue), no panic.
        assert_eq!(t.name, "Dark Blue");
    }

    #[test]
    fn embebidos_parsean_sin_panic() {
        // Cargar el catálogo no debe perder ningún embebido por parseo fallido.
        let cat = ThemeCatalog::load(std::path::Path::new("Z:/no/existe"), &ThemeCatalog::default_id());
        assert_eq!(cat.get(&ThemeId::new("light")).base, ThemeBase::Light);
        assert_eq!(cat.get(&ThemeId::new("high-contrast")).name, "High Contrast");
    }
```

- [ ] **Step 3: Correr y ver fallar**

Run: `cargo test -p naygo-core theme`
Expected: ERROR de compilación — faltan `Theme`, `Theme::from_json`, `ThemeCatalog`, `ThemeCatalog::{load, default_id, available, get}`.

- [ ] **Step 4: Implementar Theme + tolerancia + ThemeCatalog**

Añadir a `theme/mod.rs` (tras `ThemeId`, antes de los tests):

```rust
use std::collections::HashMap;
use std::path::Path;

/// Paleta completa de un tema (tokens propios de Naygo).
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub base: ThemeBase,
    pub accent: ThemeColor,
    pub panel_bg: ThemeColor,
    pub row_bg: ThemeColor,
    pub row_alt_bg: ThemeColor,
    pub text: ThemeColor,
    pub text_dim: ThemeColor,
    pub selection_bg: ThemeColor,
    pub active_bar: ThemeColor,
    pub error: ThemeColor,
    pub border: ThemeColor,
}

// Serde: serializa todos los campos; al deserializar tolera faltantes vía `ThemeRaw`.
impl Serialize for Theme {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Theme", 12)?;
        st.serialize_field("name", &self.name)?;
        st.serialize_field("base", &self.base)?;
        st.serialize_field("accent", &self.accent)?;
        st.serialize_field("panel_bg", &self.panel_bg)?;
        st.serialize_field("row_bg", &self.row_bg)?;
        st.serialize_field("row_alt_bg", &self.row_alt_bg)?;
        st.serialize_field("text", &self.text)?;
        st.serialize_field("text_dim", &self.text_dim)?;
        st.serialize_field("selection_bg", &self.selection_bg)?;
        st.serialize_field("active_bar", &self.active_bar)?;
        st.serialize_field("error", &self.error)?;
        st.serialize_field("border", &self.border)?;
        st.end()
    }
}

/// Forma cruda para deserializar tolerando faltantes (cada color es opcional).
#[derive(Deserialize)]
struct ThemeRaw {
    name: Option<String>,
    base: Option<ThemeBase>,
    accent: Option<ThemeColor>,
    panel_bg: Option<ThemeColor>,
    row_bg: Option<ThemeColor>,
    row_alt_bg: Option<ThemeColor>,
    text: Option<ThemeColor>,
    text_dim: Option<ThemeColor>,
    selection_bg: Option<ThemeColor>,
    active_bar: Option<ThemeColor>,
    error: Option<ThemeColor>,
    border: Option<ThemeColor>,
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Theme, D::Error> {
        let raw = ThemeRaw::deserialize(d)?;
        let base = raw.base.unwrap_or(ThemeBase::Dark);
        let def = Theme::defaults_for(base, raw.name.clone().unwrap_or_default());
        Ok(Theme {
            name: raw.name.unwrap_or(def.name),
            base,
            accent: raw.accent.unwrap_or(def.accent),
            panel_bg: raw.panel_bg.unwrap_or(def.panel_bg),
            row_bg: raw.row_bg.unwrap_or(def.row_bg),
            row_alt_bg: raw.row_alt_bg.unwrap_or(def.row_alt_bg),
            text: raw.text.unwrap_or(def.text),
            text_dim: raw.text_dim.unwrap_or(def.text_dim),
            selection_bg: raw.selection_bg.unwrap_or(def.selection_bg),
            active_bar: raw.active_bar.unwrap_or(def.active_bar),
            error: raw.error.unwrap_or(def.error),
            border: raw.border.unwrap_or(def.border),
        })
    }
}

impl Theme {
    /// Parsea un tema desde JSON (tolerante a campos faltantes). `None` si el JSON
    /// es inválido a nivel estructural.
    pub fn from_json(text: &str) -> Option<Theme> {
        serde_json::from_str(text).ok()
    }

    /// Paleta default neutra para un `base` (relleno de campos faltantes).
    fn defaults_for(base: ThemeBase, name: String) -> Theme {
        let c = ThemeColor::new;
        match base {
            ThemeBase::Dark => Theme {
                name,
                base,
                accent: c(0x2f, 0x81, 0xf7),
                panel_bg: c(0x1e, 0x1e, 0x1e),
                row_bg: c(0x1e, 0x1e, 0x1e),
                row_alt_bg: c(0x23, 0x23, 0x23),
                text: c(0xd4, 0xd4, 0xd4),
                text_dim: c(0x9b, 0x9b, 0x9b),
                selection_bg: c(0x37, 0x37, 0x3d),
                active_bar: c(0x2f, 0x81, 0xf7),
                error: c(0xe0, 0x6c, 0x5b),
                border: c(0x3a, 0x3a, 0x3a),
            },
            ThemeBase::Light => Theme {
                name,
                base,
                accent: c(0x17, 0x69, 0xe0),
                panel_bg: c(0xfb, 0xfb, 0xfb),
                row_bg: c(0xfb, 0xfb, 0xfb),
                row_alt_bg: c(0xf4, 0xf4, 0xf4),
                text: c(0x1f, 0x1f, 0x1f),
                text_dim: c(0x6b, 0x6b, 0x6b),
                selection_bg: c(0xdc, 0xeb, 0xff),
                active_bar: c(0x17, 0x69, 0xe0),
                error: c(0xc0, 0x39, 0x2b),
                border: c(0xdd, 0xdd, 0xdd),
            },
        }
    }
}

/// Catálogo de temas: 4 embebidos + sueltos de `<config_dir>/themes/*.json`.
pub struct ThemeCatalog {
    themes: HashMap<String, Theme>,
    available: Vec<ThemeId>,
}

const DARK_BLUE_JSON: &str = include_str!("builtin/dark-blue.json");
const DARK_TEAL_JSON: &str = include_str!("builtin/dark-teal.json");
const LIGHT_JSON: &str = include_str!("builtin/light.json");
const HIGH_CONTRAST_JSON: &str = include_str!("builtin/high-contrast.json");

impl ThemeCatalog {
    /// Id del tema por defecto (Dark Blue).
    pub fn default_id() -> ThemeId {
        ThemeId::new("dark-blue")
    }

    /// Carga embebidos + sueltos de `<dir>/themes/*.json`. `active` se ignora aquí
    /// (el llamador resuelve cuál mostrar con `get`); se acepta por simetría con i18n.
    pub fn load(dir: &Path, _active: &ThemeId) -> ThemeCatalog {
        let mut themes: HashMap<String, Theme> = HashMap::new();
        // Embebidos (id = nombre de archivo sin extensión).
        for (id, json) in [
            ("dark-blue", DARK_BLUE_JSON),
            ("dark-teal", DARK_TEAL_JSON),
            ("light", LIGHT_JSON),
            ("high-contrast", HIGH_CONTRAST_JSON),
        ] {
            if let Some(t) = Theme::from_json(json) {
                themes.insert(id.to_string(), t);
            }
        }
        // Sueltos: <dir>/themes/*.json (un JSON inválido se ignora).
        let theme_dir = dir.join("themes");
        if let Ok(entries) = std::fs::read_dir(&theme_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(text) = std::fs::read_to_string(&path) {
                            if let Some(t) = Theme::from_json(&text) {
                                themes.insert(id.to_string(), t);
                            }
                        }
                    }
                }
            }
        }
        let mut available: Vec<ThemeId> = themes.keys().map(|k| ThemeId::new(k)).collect();
        available.sort_by(|a, b| a.0.cmp(&b.0));
        ThemeCatalog { themes, available }
    }

    /// Ids disponibles (ordenados).
    pub fn available(&self) -> &[ThemeId] {
        &self.available
    }

    /// Tema por id; si no existe, el default (dark-blue). Nunca panic.
    pub fn get(&self, id: &ThemeId) -> &Theme {
        self.themes
            .get(id.as_str())
            .or_else(|| self.themes.get(Self::default_id().as_str()))
            .expect("el tema default embebido siempre existe")
    }
}
```

- [ ] **Step 5: Re-exports + verificar**

Modify `crates/core/src/lib.rs`: cambiar el re-export a:
```rust
pub use theme::{Theme, ThemeBase, ThemeCatalog, ThemeColor, ThemeId};
```

Run: `cargo test -p naygo-core theme` → todos PASS (5 de Task 1 + 6 nuevos).
Run: `cargo test -p naygo-core` → verde.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/theme/ crates/core/src/lib.rs
git commit -m "feat(core): Theme (paleta completa) + ThemeCatalog + 4 temas embebidos

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `core::theme::pack` — Pack + PackCatalog + 4 packs embebidos

**Files:**
- Create: `crates/core/src/theme/pack.rs`
- Create: `crates/core/src/theme/packs/{dark-blue,dark-teal,light,high-contrast}.json`
- Modify: `crates/core/src/theme/mod.rs` (declarar `pub mod pack;`)
- Modify: `crates/core/src/lib.rs` (re-export)

- [ ] **Step 1: Crear los 4 JSON de pack embebidos**

Cada pack empareja un tema con un icon set. `IconSet` serializa como sus variantes (`Flat`/`Fluent`/`Mono`) — verificar el `#[serde]` de `IconSet` en config (es un enum simple; serde usa el nombre de variante). Create `crates/core/src/theme/packs/dark-blue.json`:
```json
{ "name": "Dark Blue", "theme": "dark-blue", "icon_set": "Flat" }
```
Create `crates/core/src/theme/packs/dark-teal.json`:
```json
{ "name": "Dark Teal", "theme": "dark-teal", "icon_set": "Flat" }
```
Create `crates/core/src/theme/packs/light.json`:
```json
{ "name": "Light", "theme": "light", "icon_set": "Mono" }
```
Create `crates/core/src/theme/packs/high-contrast.json`:
```json
{ "name": "High Contrast", "theme": "high-contrast", "icon_set": "Mono" }
```
NOTA: confirmar que `IconSet` deserializa desde `"Flat"`/`"Mono"` (READ `config/mod.rs` enum `IconSet` — si tiene `#[serde(rename_all=...)]` ajustar los JSON; si no, son los nombres de variante tal cual).

- [ ] **Step 2: Crear `pack.rs` con Pack/PackCatalog + tests (TDD)**

Create `crates/core/src/theme/pack.rs`:
```rust
// Naygo — "packs": preset que activa un tema + un set de íconos juntos.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Un `Pack` empareja un `ThemeId` con un `IconSet`. Activar un pack escribe ambos
//! ajustes (que siguen siendo independientes después). Embebidos + sueltos, patrón
//! i18n. Puro y testeable.

use crate::config::IconSet;
use crate::theme::ThemeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Un pack: nombre visible + tema + set de íconos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    pub name: String,
    pub theme: ThemeId,
    pub icon_set: IconSet,
}

/// Catálogo de packs: embebidos + sueltos de `<config_dir>/packs/*.json`.
pub struct PackCatalog {
    packs: Vec<Pack>,
}

const DARK_BLUE: &str = include_str!("packs/dark-blue.json");
const DARK_TEAL: &str = include_str!("packs/dark-teal.json");
const LIGHT: &str = include_str!("packs/light.json");
const HIGH_CONTRAST: &str = include_str!("packs/high-contrast.json");

impl PackCatalog {
    /// Carga embebidos + sueltos. JSON inválido se ignora.
    pub fn load(dir: &Path) -> PackCatalog {
        let mut packs: Vec<Pack> = Vec::new();
        for json in [DARK_BLUE, DARK_TEAL, LIGHT, HIGH_CONTRAST] {
            if let Ok(p) = serde_json::from_str::<Pack>(json) {
                packs.push(p);
            }
        }
        let pack_dir = dir.join("packs");
        if let Ok(entries) = std::fs::read_dir(&pack_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        if let Ok(p) = serde_json::from_str::<Pack>(&text) {
                            packs.push(p);
                        }
                    }
                }
            }
        }
        PackCatalog { packs }
    }

    /// Todos los packs (embebidos primero, luego sueltos en orden de lectura).
    pub fn packs(&self) -> &[Pack] {
        &self.packs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_round_trip_serde() {
        let p = Pack {
            name: "X".into(),
            theme: ThemeId::new("dark-blue"),
            icon_set: IconSet::Flat,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: Pack = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn catalog_tiene_los_cuatro_embebidos() {
        let cat = PackCatalog::load(Path::new("Z:/no/existe"));
        assert_eq!(cat.packs().len(), 4);
        let names: Vec<&str> = cat.packs().iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Dark Blue"));
        assert!(names.contains(&"High Contrast"));
    }

    #[test]
    fn pack_referencia_tema_por_id() {
        let cat = PackCatalog::load(Path::new("Z:/no/existe"));
        let db = cat.packs().iter().find(|p| p.name == "Dark Blue").unwrap();
        assert_eq!(db.theme, ThemeId::new("dark-blue"));
    }
}
```

- [ ] **Step 3: Declarar el módulo + re-export**

Modify `crates/core/src/theme/mod.rs`: añadir al inicio (tras el `//!` doc) `pub mod pack;`.
Modify `crates/core/src/lib.rs`: ampliar re-export: `pub use theme::pack::{Pack, PackCatalog};`.

- [ ] **Step 4: Verificar**

Run: `cargo test -p naygo-core pack` → 3 tests PASS.
Run: `cargo test -p naygo-core` → verde.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/theme/pack.rs crates/core/src/theme/packs/ crates/core/src/theme/mod.rs crates/core/src/lib.rs
git commit -m "feat(core): Pack + PackCatalog + 4 packs embebidos

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `Settings.theme` (serde default)

**Files:**
- Modify: `crates/core/src/config/mod.rs`

- [ ] **Step 1: Escribir el test (TDD)**

En el `#[cfg(test)] mod tests` de `config/mod.rs`, añadir:
```rust
    #[test]
    fn settings_default_theme_es_dark_blue() {
        use crate::theme::ThemeId;
        let s = Settings::default();
        assert_eq!(s.theme, ThemeId::new("dark-blue"));
    }

    #[test]
    fn settings_viejo_sin_theme_cae_al_default() {
        use crate::theme::ThemeId;
        // settings.json previo sin el campo "theme" debe cargar y caer al default.
        let json = r#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"Flat","show_parent_entry":true,"language":"es"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.theme, ThemeId::new("dark-blue"));
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core config`
Expected: ERROR de compilación — `Settings` no tiene `theme`.

- [ ] **Step 3: Implementar**

Modify `crates/core/src/config/mod.rs`:
a) En los `use`, añadir `use crate::theme::ThemeId;` (si no está).
b) En `struct Settings`, tras `language`:
```rust
    /// Tema (color set) activo. `#[serde(default)]` por retro-compat (settings viejo
    /// sin este campo cae al default).
    #[serde(default = "default_theme")]
    pub theme: ThemeId,
```
c) Añadir el helper default (junto a los otros `default_*`):
```rust
/// Default de `theme` para `#[serde(default)]`: Dark Blue.
fn default_theme() -> ThemeId {
    ThemeId::new("dark-blue")
}
```
d) En `impl Default for Settings`, añadir `theme: default_theme(),`.

- [ ] **Step 4: Verificar**

Run: `cargo test -p naygo-core config` → PASS (incl. 2 nuevos).
Run: `cargo test -p naygo-core` → verde.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/config/mod.rs
git commit -m "feat(core): Settings.theme (color set activo, serde default Dark Blue)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: i18n — claves del selector de tema/packs (ES + EN)

**Files:**
- Modify: `crates/core/src/i18n/es.json`, `crates/core/src/i18n/en.json`

- [ ] **Step 1: Añadir claves en `es.json`**

READ `es.json` para confirmar que existe `"settings.theme"` (debería, hay un placeholder) y `"settings.theme.placeholder"`. Tras la línea `"settings.theme": "...",` (o donde estén las claves settings.*), insertar:
```json
  "settings.theme.section": "Tema",
  "settings.packs.section": "Packs",
  "settings.packs.hint": "Un pack activa un tema y un set de íconos juntos.",
```
(Si `"settings.theme"` ya existe como etiqueta, reutilizarla; estas son adicionales para la sección de packs y un subtítulo. NO dupliques claves existentes.)

- [ ] **Step 2: Mismas claves en `en.json`**

Insertar en `en.json` (mismas posiciones):
```json
  "settings.theme.section": "Theme",
  "settings.packs.section": "Packs",
  "settings.packs.hint": "A pack activates a theme and an icon set together.",
```

NOTA: los NOMBRES de tema/pack (Dark Blue, etc.) NO son claves i18n — vienen del JSON del tema/pack y se muestran tal cual. Solo las etiquetas de sección se traducen.

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core i18n` → PASS (parity ES/EN).
Run: `cargo test -p naygo-core` → verde.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "i18n: claves de sección Tema/Packs (ES/EN)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: `ui::theme_apply` — ActiveTheme + Theme→Visuals + apply

**Files:**
- Create: `crates/ui/src/theme_apply.rs`
- Modify: `crates/ui/src/main.rs` (`mod theme_apply;`)

- [ ] **Step 1: Crear `theme_apply.rs`**

VERIFICAR egui 0.34.3 contra `C:\Users\ngrot\.cargo\registry\src\index.crates.io-*\egui-0.34.3\`: `egui::Visuals::dark()`/`light()`, los campos `panel_fill`, `window_fill`, `extreme_bg_color`, `selection: Selection { bg_fill, stroke }`, `override_text_color: Option<Color32>`, `hyperlink_color`, `widgets: Widgets`. `egui::Color32::from_rgb(u8,u8,u8)`. `ctx.set_visuals(Visuals)`. Ajustar el mapeo a los campos reales de 0.34.

Create `crates/ui/src/theme_apply.rs`:
```rust
// Naygo — traducción de un Theme (core) a egui::Visuals y su aplicación al Context.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `core` define la paleta (datos); aquí se convierte a `egui::Visuals` y se aplica
//! en caliente. `ActiveTheme` guarda el tema resuelto y expone sus tokens como
//! `egui::Color32` para los paneles que pintan acentos propios (selección de fila,
//! barra del panel activo, indicadores, línea de drop).

use naygo_core::theme::{Theme, ThemeBase, ThemeColor, ThemeId};

/// El tema activo resuelto + su id (para el hot-swap).
pub struct ActiveTheme {
    pub id: ThemeId,
    pub theme: Theme,
}

impl ActiveTheme {
    pub fn new(id: ThemeId, theme: Theme) -> Self {
        ActiveTheme { id, theme }
    }
    pub fn accent(&self) -> egui::Color32 {
        to_color32(self.theme.accent)
    }
    pub fn active_bar(&self) -> egui::Color32 {
        to_color32(self.theme.active_bar)
    }
    pub fn selection_bg(&self) -> egui::Color32 {
        to_color32(self.theme.selection_bg)
    }
    pub fn text_dim(&self) -> egui::Color32 {
        to_color32(self.theme.text_dim)
    }
    pub fn error(&self) -> egui::Color32 {
        to_color32(self.theme.error)
    }
}

/// Convierte un `ThemeColor` a `egui::Color32` (opaco).
pub fn to_color32(c: ThemeColor) -> egui::Color32 {
    egui::Color32::from_rgb(c.r, c.g, c.b)
}

/// Traduce el tema a `egui::Visuals` y lo aplica al contexto (hot-swap).
pub fn apply(theme: &Theme, ctx: &egui::Context) {
    let mut v = match theme.base {
        ThemeBase::Dark => egui::Visuals::dark(),
        ThemeBase::Light => egui::Visuals::light(),
    };
    v.panel_fill = to_color32(theme.panel_bg);
    v.window_fill = to_color32(theme.panel_bg);
    v.extreme_bg_color = to_color32(theme.row_alt_bg); // fondo de striped alterno
    v.selection.bg_fill = to_color32(theme.selection_bg);
    v.selection.stroke.color = to_color32(theme.accent);
    v.hyperlink_color = to_color32(theme.accent);
    v.override_text_color = Some(to_color32(theme.text));
    // Borde de widgets (no inactivo) — color de borde del tema.
    v.widgets.noninteractive.bg_stroke.color = to_color32(theme.border);
    ctx.set_visuals(v);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_color32_mapea_rgb() {
        let c = to_color32(ThemeColor::new(0x2f, 0x81, 0xf7));
        assert_eq!(c, egui::Color32::from_rgb(0x2f, 0x81, 0xf7));
    }
}
```
NOTA: si algún campo de `Visuals` no existe con ese nombre en 0.34 (p. ej. `selection.stroke` o `widgets.noninteractive.bg_stroke`), ajustar al real. El objetivo: el chrome refleja la paleta; lo mínimo imprescindible es `panel_fill`, `selection.bg_fill`, `override_text_color`, `extreme_bg_color`. Los acentos propios (barra activa, línea de drop, ≡) se leen del `ActiveTheme` en los paneles (Task 8), no dependen de Visuals.

- [ ] **Step 2: Declarar el módulo**

Modify `crates/ui/src/main.rs`: añadir `mod theme_apply;` en orden alfabético (tras `mod templates_menu;`/`mod toolbar;`, antes de `mod tree_actions;` — READ para ubicar).

- [ ] **Step 3: Verificar**

Run: `cargo build -p naygo-ui` → compila (resolver mapeo Visuals).
Run: `cargo test -p naygo-ui theme_apply` → 1 test PASS.
Run: `cargo clippy -p naygo-ui --all-targets -- -D warnings`. `ActiveTheme`/getters aún no se usan → posible `dead_code`. Si bloquea, `#[allow(dead_code)]` con comentario `// consumido en Tareas 7-8`, a quitar en Task 8.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/theme_apply.rs crates/ui/src/main.rs
git commit -m "feat(ui): theme_apply (Theme→Visuals + ActiveTheme con tokens Color32)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: `app.rs` — cargar tema, aplicar al arrancar, hot-swap, pasar a paneles

**Files:**
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/docking.rs`

- [ ] **Step 1: Estado + carga inicial en `NaygoApp`**

Modify `crates/ui/src/app.rs`:
a) `use`: añadir `use crate::theme_apply::{self, ActiveTheme}; use naygo_core::theme::ThemeCatalog;`
b) En `struct NaygoApp`, añadir junto a `i18n`: `theme_catalog: ThemeCatalog,` y `active_theme: ActiveTheme,`.
c) En `NaygoApp::new`, tras cargar settings y ANTES de construir el struct: 
```rust
        let theme_catalog = ThemeCatalog::load(&config_dir, &settings.theme);
        let active_theme = {
            let t = theme_catalog.get(&settings.theme).clone();
            ActiveTheme::new(settings.theme.clone(), t)
        };
        theme_apply::apply(&active_theme.theme, &cc.egui_ctx);
```
(`config_dir`/`settings` ya existen ahí; `cc.egui_ctx` también.) Añadir `theme_catalog,` y `active_theme,` al literal del struct.

- [ ] **Step 2: Hot-swap en `ui()`**

En `NaygoApp::ui()`, junto al hot-swap de íconos/idioma (~line 610), añadir:
```rust
        // Hot-swap del tema: si cambió el id en settings, recargar y reaplicar.
        if self.active_theme.id != self.settings.theme {
            let t = self.theme_catalog.get(&self.settings.theme).clone();
            self.active_theme = ActiveTheme::new(self.settings.theme.clone(), t);
            theme_apply::apply(&self.active_theme.theme, ui.ctx());
        }
```

- [ ] **Step 3: Pasar `&ActiveTheme` al viewer y paneles**

Modify `crates/ui/src/docking.rs`: añadir campo a `NaygoTabViewer`:
```rust
    pub theme: &'a crate::theme_apply::ActiveTheme,
```
Modify `crates/ui/src/app.rs`: en la construcción de `NaygoTabViewer { ... }`, añadir `theme: &self.active_theme,`.
PROBLEMA DE BORROW: el viewer toma `&self.active_theme` (inmutable) junto a `&mut self.workspace` — campos disjuntos, compila. Igual que `icons`/`i18n` (que ya son `&self.*`). Si el borrow falla, usar el mismo patrón que ya funciona para `icons`.

- [ ] **Step 4: Verificar (sin usar los tokens aún → Task 8 los consume en paneles)**

Run: `cargo build -p naygo-ui` → compila.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio. (Si `theme` del viewer no se usa todavía en los paneles, puede dar `dead_code`/unused field. Para evitarlo, Task 8 los consume; si quieres mantener verde entre tareas, pasa `theme` y úsalo de inmediato en al menos un panel — pero el plan separa 7 y 8. Si clippy bloquea por campo sin usar, añade un `let _ = self.theme;` temporal en docking `ui()` con comentario `// usado en Task 8`, o combina 7 y 8. Reporta.)
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start: la app arranca con el tema Dark Blue aplicado (idéntico visualmente a hoy); cambiar `settings.theme` (cuando exista el selector) reteñirá. Sin selector aún → no hay cambio visible salvo que los Visuals del tema se vean (fondos/selección ya salen del tema).

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/app.rs crates/ui/src/docking.rs
git commit -m "feat(ui): cargar/aplicar tema + hot-swap + pasar ActiveTheme a los paneles

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Reemplazar colores hardcoded de 2E por tokens del tema

**Files:**
- Modify: `crates/ui/src/docking.rs`, `crates/ui/src/panes/file_panel.rs`, `crates/ui/src/panes/tree_panel.rs`

- [ ] **Step 1: docking.rs — barra de título del panel activo**

En `docking.rs` `title()`, donde el panel activo usa `egui::Color32::from_rgb(0x2f, 0x81, 0xf7)`, reemplazar por `self.theme.accent()`:
```rust
        if self.workspace.active_id() == Some(*tab) {
            egui::RichText::new(name).color(self.theme.accent()).strong().into()
        } else {
            name.into()
        }
```

- [ ] **Step 2: file_panel.rs — línea de drop y ≡ de filtro**

`file_panel::show` y `column_header` necesitan el `&ActiveTheme`. Pasarlo: añadir un parámetro `theme: &crate::theme_apply::ActiveTheme` a `show` (y reenviarlo desde docking.rs, que ya tiene `self.theme`). En la llamada de docking.rs `Some(PanePurpose::Files) => { ... file_panel::show(ui, self.workspace, id, self.pending, self.icons, self.show_parent_entry, self.i18n, &mut local, self.theme); ... }`.
- Línea de drop: reemplazar `egui::Color32::from_rgb(0x2f, 0x81, 0xf7)` por `theme.accent()`.
- El ≡ de filtro: hoy es parte del string del título (sin color propio; hereda el text). Si se quiere en acento, pintar el ≡ aparte con `theme.accent()` — OPCIONAL; para mantenerlo simple, dejar el ≡ con el color de texto (ya cumple). Si el reviewer/visual lo pide en acento, es un ajuste menor. Para esta tarea: dejar ≡ heredando texto (sin cambio de color), y SOLO cambiar la línea de drop a `theme.accent()`.
- `column_header` necesita `theme` solo si coloreamos el ≡; como no lo coloreamos, NO hace falta pasar theme a `column_header`. Mantener su firma.

NOTA: `row.set_selected` y el striping ya salen de `Visuals` (Task 6 los setea), así que la selección de fila YA refleja el tema sin tocar file_panel.

- [ ] **Step 2b: file_panel — "sin coincidencias"** usa `ui.weak(...)` (hereda text_dim de Visuals si se setea, o el weak de egui) → sin cambio necesario.

- [ ] **Step 3: tree_panel.rs — resaltado del nodo activo + error**

`tree_panel::show`/`show_node` necesitan `&ActiveTheme`. Añadir el parámetro `theme` a `show` (y `show_node`), reenviado desde docking.rs (`tree_panel::show(ui, tree, &mut local, self.icons, self.i18n, self.theme)`).
- Resaltado del nodo activo: hoy fondo `Color32::from_rgb(0x37,0x37,0x3d)` + barra `0x2f81f7`/`0x3b82f6`. Reemplazar fondo por `theme.selection_bg()` y la barra por `theme.accent()`.
- Error (⚠ acceso denegado): hoy `Color32::from_rgb(0xe0,...)`. Reemplazar por `theme.error()`.

- [ ] **Step 4: Quitar allows + verificar**

Quitar cualquier `#[allow(dead_code)]` puesto en Tasks 6/7 sobre `ActiveTheme`/getters/campo `theme` (ahora se usan).
Run: `cargo build -p naygo-ui` → compila (resolver el threading de `theme` por las firmas de los paneles).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start: cambiar de tema (cuando el selector exista — Task 9) reteñirá barra de título del panel activo, selección de fila, línea de drop, resaltado del árbol y el error, además de fondos/striping (vía Visuals). Con Dark Blue se ve igual que hoy.

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/docking.rs crates/ui/src/panes/file_panel.rs crates/ui/src/panes/tree_panel.rs
git commit -m "feat(ui): los acentos de la UI salen del tema activo (no hardcoded)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Selector de tema (tarjetas) + sección de packs en Apariencia

**Files:**
- Modify: `crates/ui/src/settings_window/appearance.rs`
- Modify: `crates/ui/src/app.rs` (acceso al catálogo + aplicar pack)

- [ ] **Step 1: Exponer catálogos y "aplicar pack" desde NaygoApp**

Modify `crates/ui/src/app.rs`: añadir métodos a `impl NaygoApp`:
```rust
    /// Ids de tema disponibles + su Theme (para pintar las tarjetas del selector).
    pub fn theme_cards(&self) -> Vec<(naygo_core::theme::ThemeId, naygo_core::theme::Theme)> {
        self.theme_catalog
            .available()
            .iter()
            .map(|id| (id.clone(), self.theme_catalog.get(id).clone()))
            .collect()
    }

    /// Packs disponibles (para la sección de packs).
    pub fn packs(&self) -> Vec<naygo_core::theme::Pack> {
        naygo_core::theme::PackCatalog::load(&self.config_dir).packs().to_vec()
    }

    /// Activa un pack: setea tema + icon set (siguen independientes después).
    pub fn apply_pack(&mut self, pack: &naygo_core::theme::Pack) {
        self.settings.theme = pack.theme.clone();
        self.settings.icon_set = pack.icon_set;
    }
```
(`config_dir` es privado en NaygoApp pero los métodos están en el mismo impl → OK. `Pack` debe derivar `Clone` — ya lo hace.)
NOTA: cargar `PackCatalog` en cada `packs()` (cada repaint de Settings) es un `read_dir` por frame de la ventana de config. Para evitar I/O por frame, MEJOR: cargar `PackCatalog` una vez en `new` y guardarlo como `pack_catalog: PackCatalog` en NaygoApp (igual que theme_catalog), y que `packs()` lo lea. HACERLO ASÍ: añadir `pack_catalog: PackCatalog` al struct, cargarlo en `new` (`PackCatalog::load(&config_dir)`), y `packs()` devuelve `self.pack_catalog.packs().to_vec()`.

- [ ] **Step 2: Selector de tarjetas + packs en appearance.rs**

Modify `crates/ui/src/settings_window/appearance.rs`. Reemplazar el placeholder de tema (las líneas `ui.label(app.tr("settings.theme")); let placeholder = ...; ui.label(RichText::new(placeholder).weak());`) por el selector de tarjetas + la sección de packs.

VERIFICAR egui 0.34.3 para pintar las tarjetas: `ui.horizontal_wrapped(|ui|{...})`, `egui::Frame::group(ui.style())` o `Frame::none().fill(...).stroke(...)`, `ui.allocate_response`/`response.clicked()`, `Painter::rect_filled`. Usar un patrón simple: cada tarjeta es un `ui.vertical(|ui|{...})` dentro de un `Frame` clicable (con `ui.interact` sobre el rect), con 3 swatches (`painter.rect_filled` de panel_bg/selection_bg/accent) + el nombre. La activa lleva borde de acento.

Esqueleto:
```rust
    ui.label(app.tr("settings.theme.section"));
    ui.add_space(4.0);
    let current = app.settings.theme.clone();
    let cards = app.theme_cards();
    let mut chosen: Option<naygo_core::theme::ThemeId> = None;
    ui.horizontal_wrapped(|ui| {
        for (id, theme) in &cards {
            let is_active = *id == current;
            // Tarjeta: un grupo clicable con swatches + nombre.
            let resp = theme_card(ui, theme, is_active);
            if resp.clicked() {
                chosen = Some(id.clone());
            }
        }
    });
    if let Some(id) = chosen {
        app.settings.theme = id; // el hot-swap del próximo frame lo aplica
    }
    ui.add_space(10.0);

    // Packs.
    ui.label(app.tr("settings.packs.section"));
    ui.label(egui::RichText::new(app.tr("settings.packs.hint")).weak());
    ui.add_space(4.0);
    let packs = app.packs();
    let mut chosen_pack: Option<naygo_core::theme::Pack> = None;
    ui.horizontal_wrapped(|ui| {
        for pack in &packs {
            if ui.button(&pack.name).clicked() {
                chosen_pack = Some(pack.clone());
            }
        }
    });
    if let Some(p) = chosen_pack {
        app.apply_pack(&p);
    }
    ui.add_space(10.0);
```
Añadir el helper `theme_card` al final de `appearance.rs`:
```rust
/// Pinta una tarjeta de tema: 3 swatches (panel/selección/acento) + nombre. Borde de
/// acento si es el activo. Devuelve el Response clicable.
fn theme_card(ui: &mut egui::Ui, theme: &naygo_core::theme::Theme, active: bool) -> egui::Response {
    use crate::theme_apply::to_color32;
    let desired = egui::vec2(92.0, 56.0);
    let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());
    let painter = ui.painter_at(rect);
    // Fondo de la tarjeta.
    painter.rect_filled(rect, 6.0, to_color32(theme.panel_bg));
    // 3 swatches arriba.
    let sw_h = 18.0;
    let third = rect.width() / 3.0;
    for (i, c) in [theme.panel_bg, theme.selection_bg, theme.accent].iter().enumerate() {
        let x0 = rect.left() + third * i as f32;
        let sr = egui::Rect::from_min_size(egui::pos2(x0, rect.top()), egui::vec2(third, sw_h));
        painter.rect_filled(sr, 0.0, to_color32(*c));
    }
    // Nombre debajo.
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 12.0),
        egui::Align2::CENTER_CENTER,
        &theme.name,
        egui::FontId::proportional(12.0),
        to_color32(theme.text),
    );
    // Borde: acento si activo, borde tenue si no.
    let stroke = if active {
        egui::Stroke::new(2.0, to_color32(theme.accent))
    } else {
        egui::Stroke::new(1.0, to_color32(theme.border))
    };
    painter.rect_stroke(rect, 6.0, stroke, egui::StrokeKind::Inside);
    resp
}
```
VERIFICAR egui 0.34.3: `ui.allocate_exact_size(vec2, Sense) -> (Rect, Response)`; `ui.painter_at(rect) -> Painter`; `Painter::rect_filled(rect, corner_radius, color)`; `Painter::text(pos, align, text, font_id, color) -> Rect`; `Painter::rect_stroke(rect, radius, stroke, StrokeKind)` (en 0.34 puede requerir `egui::StrokeKind` o ser `rect_stroke(rect, radius, stroke)` sin kind — VERIFICAR y ajustar); `egui::Align2::CENTER_CENTER`, `egui::FontId::proportional`. Ajustar firmas al 0.34 real.

- [ ] **Step 3: Verificar**

Run: `cargo build -p naygo-ui` → compila (resolver firmas de painter).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start: abrir Configuración → Apariencia muestra las tarjetas de tema (Dark Blue activa con borde de acento) + botones de pack; clic en una tarjeta cambia el tema EN CALIENTE; clic en un pack cambia tema+íconos.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/settings_window/appearance.rs crates/ui/src/app.rs
git commit -m "feat(ui): selector de tema (tarjetas con preview) + sección de packs

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: Actualizar README**

Modify `README.md` — bloque de estado:
```markdown
> **Estado:** Fase 2C-ii (temas / color sets / packs) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-07-naygo-fase2c-ii-temas-packs-design.md`](docs/superpowers/specs/2026-06-07-naygo-fase2c-ii-temas-packs-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-07-naygo-fase2c-ii-temas-packs.md`](docs/superpowers/plans/2026-06-07-naygo-fase2c-ii-temas-packs.md).
> Fases 1, 2A, 2B, 2C-i, 2D, árbol, 2E (columnas Excel) y su pulido completas.
```
(READ el bloque actual y reemplazarlo.)

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → verde (core: theme/pack/config/i18n; ui: theme_apply).
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo fmt --check` → limpio.
Run: `cargo build --release -p naygo-ui` → release compila.
App-start manual: cambiar entre los 4 temas y un pack; verificar hot-swap de fondos/selección/acentos/árbol; reiniciar y confirmar que el tema persiste.

- [ ] **Step 3: Commit y push**

```bash
git add README.md
git commit -m "chore: actualizar estado del README (Fase 2C-ii temas/packs)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/fase2c-ii-temas-packs
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| `ThemeColor` (hex) + serde | 1 |
| `ThemeBase`, `ThemeId` | 1 |
| `Theme` (paleta completa) + tolerancia campos faltantes | 2 |
| 4 temas embebidos | 2 |
| `ThemeCatalog` (embebidos + sueltos, id desconocido→default) | 2 |
| `Pack` + `PackCatalog` (embebidos + sueltos) | 3 |
| `Settings.theme` (serde default) | 4 |
| i18n claves de sección | 5 |
| `Theme→Visuals` + `apply` | 6 |
| `ActiveTheme` + getters Color32 | 6 |
| Cargar/aplicar al arrancar + hot-swap | 7 |
| Pasar `&ActiveTheme` a paneles | 7 + 8 |
| Acento de 2E desde el tema (docking/file/tree) | 8 |
| Selector de tarjetas con preview | 9 |
| Sección de packs (activar = tema+íconos) | 9 |
| Cambio en caliente | 7 (hot-swap) + 9 (selector) |
| Persistencia del tema | 4 (Settings) — ya persiste vía config |
| Tolerancia (JSON inválido, campo faltante, id inexistente) | 2 (Theme/Catalog) + 3 (Pack) |

**Notas de riesgo:**
- Mapeo `Theme → egui::Visuals` (Task 6): verificar nombres de campos en 0.34.3 (`selection.stroke`, `widgets.noninteractive.bg_stroke`, `extreme_bg_color`, `override_text_color`). Lo imprescindible: `panel_fill`, `selection.bg_fill`, `override_text_color`, `extreme_bg_color`. Acentos propios van por `ActiveTheme` (no Visuals).
- Painter de las tarjetas (Task 9): verificar `rect_stroke`/`StrokeKind`, `painter_at`, `Painter::text`, `allocate_exact_size` en 0.34.3.
- `IconSet` serde (Task 3): confirmar cómo serializa (nombres de variante `Flat`/`Fluent`/`Mono` vs rename) y ajustar los JSON de pack.
- Borrow en app.rs (Task 7): `&self.active_theme` junto a `&mut self.workspace` — disjuntos, igual que `icons`/`i18n`. Si falla, mismo patrón existente.
- `dead_code` entre Tasks 6-7-8: `ActiveTheme`/getters/campo `theme` del viewer no se usan hasta Task 8 → allow temporal o reportar. Task 8 los consume y quita el allow.
- `PackCatalog` por frame (Task 9): cargarlo UNA vez en `new` (`pack_catalog` en el struct), no en cada `packs()` (evita read_dir por frame).
- Persistencia: `settings.theme` se guarda por el mecanismo de config existente; el catálogo se relee al arrancar (no se persiste), como i18n.
```
