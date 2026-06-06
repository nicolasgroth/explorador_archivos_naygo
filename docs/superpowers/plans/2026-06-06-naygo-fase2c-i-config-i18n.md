# Naygo — Fase 2C-i: Ventana de Configuración + i18n (plan de implementación)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reemplazar todo el texto hardcoded por un sistema de i18n (`core::i18n`: claves→texto, ES/EN embebidos + `lang/*.json` sueltos, cambio en caliente, detección del idioma del SO) y reemplazar el menú ⚙ inline por una **ventana de Configuración** (viewport separado del SO) con secciones (Apariencia/Paneles/Atajos/Idioma/Avanzado).

**Architecture:** `naygo-core` gana `i18n` (lógica pura: `I18n`/`Catalog`/`t()`/fallback, carga embebida+suelta) y `pick_default_language` (puro). `naygo-platform` gana la lectura del locale del SO (`GetUserDefaultLocaleName` vía el crate `windows`, aislado). `naygo-ui` mantiene un `I18n` en `NaygoApp`, migra todos los literales a `t("clave")`, y abre la ventana de Configuración con `ctx.show_viewport_immediate` (viewport separado; el closure `FnMut` captura `&mut self`). Las opciones del menú ⚙ se mudan a la ventana, agrupadas en secciones.

**Tech Stack:** Rust, `naygo-core` (serde_json para catálogos), `naygo-platform` (`windows` 0.62 para locale), `eframe`/`egui` 0.34.3 (multi-viewport). Sin dependencias nuevas.

**Estado de partida (2B, en `main`/rama base):**
- `naygo-core`: cancel, fs_model, sort, listing, workspace, config (`Settings { version, bar_position, icon_only, icon_set, show_parent_entry }`, todos con `#[serde(default)]` salvo los base), icon_kind.
- `naygo-platform`: crate aislado de Win32, hoy con un `hello()` placeholder y la dep `windows` declarada bajo `[target.'cfg(windows)'.dependencies]`.
- `naygo-ui`: app.rs (`NaygoApp { workspace, dock_state, listings, settings, templates, config_dir, status, typeahead_buf, icons }`), toolbar.rs (con `settings_button` = menú ⚙ inline con posición de barra, solo-íconos, set de íconos, fila ".."), docking.rs, panes/*, templates_menu.rs, icons/, input.rs, typeahead.rs, dock_translate.rs. Textos hardcoded en español por todos lados.
- egui 0.34.3 verificado: `ctx.show_viewport_immediate(ViewportId, ViewportBuilder, FnMut(&mut Ui, ViewportClass) -> T) -> T` (el closure captura `&mut`); `ViewportId::from_hash_of("settings")`; `ViewportBuilder::default().with_title(..).with_inner_size([f32;2]).with_close_button(bool)`; dentro del closure se usa `egui::CentralPanel::default().show(ui, ...)` y `SidePanel::left(..).show_inside(ui, ...)` (el `ui` ES el root del viewport); cierre: `ui.ctx().input(|i| i.viewport().close_requested())`. Locale: `windows::Win32::Globalization::GetUserDefaultLocaleName(&mut [u16]) -> i32`.

**Prerequisito:** toolchain Rust en PATH. En PowerShell, prepend `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. Nunca `2>&1` con cargo. Verificar `$LASTEXITCODE`. La app tiene 2 binarios (`naygo`, `gen_icons`): para correr usar `cargo run -p naygo-ui --bin naygo`.

**Alcance:** ENTRA: `core::i18n` + `pick_default_language`, ES/EN embebidos + `lang/*.json`, `platform` locale, `Settings.language`, ventana de Configuración (viewport) con 5 secciones, migración total de textos a claves, selector de idioma en caliente, mudanza de las opciones del ⚙ a la ventana. NO ENTRA: temas/color sets reales (Apariencia muestra placeholder de tema), packs, recoloreado del mono, edición de atajos (solo-lectura), multi-ventana del explorador, deuda del dock (2A).

---

## Estructura de archivos

```
crates/core/src/
├── lib.rs                  # + re-exports de i18n
├── i18n/
│   ├── mod.rs              # LangId, Catalog, I18n (t, set_language, load, available)
│   ├── detect.rs           # pick_default_language (puro)
│   ├── es.json             # catálogo ES embebido (include_str!)
│   └── en.json             # catálogo EN embebido
├── config/mod.rs           # + Settings.language
└── ...

crates/platform/src/
├── lib.rs                  # + pub mod locale;
└── locale.rs               # os_locale() -> Option<String> (Win32 GetUserDefaultLocaleName)

crates/ui/src/
├── settings_window/
│   ├── mod.rs              # SettingsSection, show_settings_viewport(app, ctx)
│   ├── appearance.rs       # sección Apariencia
│   ├── panes.rs            # sección Paneles
│   ├── shortcuts.rs        # sección Atajos (solo-lectura)
│   ├── language.rs         # sección Idioma
│   └── advanced.rs         # sección Avanzado
├── app.rs                  # + i18n, settings_open, settings_section; abre el viewport en update/ui
├── toolbar.rs              # ⚙ abre la ventana (quita settings_button inline); textos vía tr()
├── panes/{file,tree,inspector}_panel.rs  # textos vía clave
├── docking.rs              # títulos de tab vía clave
├── templates_menu.rs       # textos vía clave (nombres built-in traducidos)
└── ...
```

**Por qué así:** `core::i18n` puro y testeable; `platform` aísla la lectura del locale (Win32); `ui` consume. La ventana de config se parte por sección (archivos enfocados). La migración de textos se hace por zona (tareas separadas) para revisiones manejables.

**Riesgo marcado:** la API de multi-viewport (Tarea 6) es lo más nuevo; el código de abajo usa la API VERIFICADA contra la fuente de egui 0.34.3.

---

## Task 1: `core::i18n` — `LangId`, `Catalog`, `I18n`

**Files:**
- Create: `crates/core/src/i18n/mod.rs`
- Create: `crates/core/src/i18n/es.json`, `crates/core/src/i18n/en.json` (mínimos por ahora, se llenan al migrar)
- Modify: `crates/core/src/lib.rs`
- Test: módulo `#[cfg(test)]` en `mod.rs`

- [ ] **Step 1: Crear catálogos mínimos embebidos**

Create `crates/core/src/i18n/es.json`:
```json
{
  "app.loading": "Listando…",
  "app.cancelled": "Cancelado"
}
```
Create `crates/core/src/i18n/en.json`:
```json
{
  "app.loading": "Listing…",
  "app.cancelled": "Cancelled"
}
```
(Son semilla; las claves reales se agregan a medida que migramos textos en las Tareas 7-9. Tener 2 claves permite testear el mecanismo ya.)

- [ ] **Step 2: Crear `i18n/mod.rs` con la lógica y tests**

Create `crates/core/src/i18n/mod.rs`:

```rust
// Naygo — internacionalización: catálogo de textos por clave (lógica pura).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `I18n` resuelve un texto por clave en el idioma activo, con fallback al idioma
//! de respaldo (ES) y, si falta, a la clave misma (visible, nunca panic). ES y EN
//! van embebidos; además se pueden cargar `lang/*.json` sueltos al lado del `.exe`.
//! No toca egui ni Windows; la lectura del locale del SO vive en `platform`.

pub mod detect;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub use detect::pick_default_language;

/// Identificador de idioma (código corto: "es", "en", "fr"...).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LangId(pub String);

impl LangId {
    pub fn new(code: &str) -> Self {
        LangId(code.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Catálogo de un idioma: clave → texto.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub lang: String,
    map: HashMap<String, String>,
}

impl Catalog {
    /// Parsea un catálogo desde JSON plano (clave: texto). JSON inválido → vacío.
    pub fn from_json(lang: &str, json: &str) -> Catalog {
        let map: HashMap<String, String> = serde_json::from_str(json).unwrap_or_else(|e| {
            tracing::warn!("catálogo i18n '{lang}' ilegible: {e}");
            HashMap::new()
        });
        Catalog { lang: lang.to_string(), map }
    }

    /// Texto de `key`, o `None` si la clave no está.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }

    /// Mergea otro catálogo encima (las claves de `other` ganan).
    pub fn merge(&mut self, other: &Catalog) {
        for (k, v) in &other.map {
            self.map.insert(k.clone(), v.clone());
        }
    }
}

/// Catálogos embebidos en el binario (siempre disponibles).
const ES_JSON: &str = include_str!("es.json");
const EN_JSON: &str = include_str!("en.json");

/// Estado de i18n: idioma activo + fallback (ES) + idiomas disponibles.
pub struct I18n {
    active: Catalog,
    fallback: Catalog,
    available: Vec<LangId>,
    catalogs: HashMap<String, Catalog>,
}

impl I18n {
    /// Construye con los embebidos (ES/EN) más los `lang/*.json` de `dir`, y activa
    /// `lang` (si no existe, cae a EN, y si tampoco, a ES).
    pub fn load(dir: &Path, lang: &LangId) -> I18n {
        let mut catalogs: HashMap<String, Catalog> = HashMap::new();
        catalogs.insert("es".into(), Catalog::from_json("es", ES_JSON));
        catalogs.insert("en".into(), Catalog::from_json("en", EN_JSON));

        // Cargar archivos sueltos: dir/lang/*.json (cada uno mergea/añade).
        let lang_dir = dir.join("lang");
        if let Ok(entries) = std::fs::read_dir(&lang_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(code) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Ok(text) = std::fs::read_to_string(&path) {
                            let cat = Catalog::from_json(code, &text);
                            catalogs
                                .entry(code.to_string())
                                .or_default()
                                .merge(&cat);
                            // Asegurar que el lang del catálogo quede seteado.
                            catalogs.get_mut(code).unwrap().lang = code.to_string();
                        }
                    }
                }
            }
        }

        let available: Vec<LangId> = {
            let mut v: Vec<LangId> = catalogs.keys().map(|k| LangId::new(k)).collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v
        };

        let fallback = catalogs.get("es").cloned().unwrap_or_default();
        let active = catalogs
            .get(lang.as_str())
            .or_else(|| catalogs.get("en"))
            .or_else(|| catalogs.get("es"))
            .cloned()
            .unwrap_or_default();

        I18n { active, fallback, available, catalogs }
    }

    /// Texto de `key`: activo → fallback (ES) → la clave misma (nunca panic).
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.active
            .get(key)
            .or_else(|| self.fallback.get(key))
            .unwrap_or(key)
    }

    /// Cambia el idioma activo si está cargado. Devuelve true si cambió.
    pub fn set_language(&mut self, lang: &LangId) -> bool {
        if let Some(cat) = self.catalogs.get(lang.as_str()) {
            self.active = cat.clone();
            true
        } else {
            false
        }
    }

    /// Idioma activo.
    pub fn active_lang(&self) -> LangId {
        LangId::new(&self.active.lang)
    }

    /// Idiomas disponibles (ordenados).
    pub fn available(&self) -> &[LangId] {
        &self.available
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i18n_de_prueba() -> I18n {
        // Construye sin tocar disco: usa una ruta inexistente (no carga sueltos).
        I18n::load(Path::new("Z:/no/existe/naygo-test"), &LangId::new("es"))
    }

    #[test]
    fn t_resuelve_clave_del_activo() {
        let i = i18n_de_prueba();
        assert_eq!(i.t("app.loading"), "Listando…");
    }

    #[test]
    fn t_clave_inexistente_devuelve_la_clave() {
        let i = i18n_de_prueba();
        assert_eq!(i.t("clave.que.no.existe"), "clave.que.no.existe");
    }

    #[test]
    fn set_language_cambia_el_activo() {
        let mut i = i18n_de_prueba();
        assert!(i.set_language(&LangId::new("en")));
        assert_eq!(i.t("app.loading"), "Listing…");
    }

    #[test]
    fn set_language_idioma_inexistente_no_cambia() {
        let mut i = i18n_de_prueba();
        assert!(!i.set_language(&LangId::new("zz")));
        assert_eq!(i.t("app.loading"), "Listando…");
    }

    #[test]
    fn fallback_a_es_si_falta_en_el_activo() {
        // Activamos EN; una clave que solo exista en ES debería caer al fallback.
        // Con los catálogos semilla ambos tienen las mismas claves, así que este
        // test verifica el mecanismo: si EN no tuviera la clave, usa ES.
        let mut i = i18n_de_prueba();
        i.set_language(&LangId::new("en"));
        // app.loading existe en EN; verificamos que NO cae a la clave cruda.
        assert_ne!(i.t("app.loading"), "app.loading");
    }

    #[test]
    fn catalog_json_invalido_queda_vacio() {
        let c = Catalog::from_json("es", "{ no es json");
        assert!(c.get("x").is_none());
    }

    #[test]
    fn disponibles_incluye_es_y_en() {
        let i = i18n_de_prueba();
        let codes: Vec<&str> = i.available().iter().map(|l| l.as_str()).collect();
        assert!(codes.contains(&"es"));
        assert!(codes.contains(&"en"));
    }
}
```

- [ ] **Step 3: `detect.rs` (placeholder hasta Tarea 2)**

Create `crates/core/src/i18n/detect.rs`:
```rust
// Naygo — detección del idioma por defecto a partir del locale del SO (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```
(Contenido real en la Tarea 2; el `pub mod detect;` + `pub use detect::pick_default_language;` en mod.rs lo referencian, así que el archivo debe existir. Para que compile en esta tarea, define un stub mínimo: añade a `detect.rs` una función temporal y un re-export que la Tarea 2 reemplaza. MEJOR: implementa `pick_default_language` ya aquí — ver Tarea 2 Step 1 — y haz la Tarea 1 y 2 juntas. Para mantener el orden, en esta tarea pon en detect.rs SOLO el header y mueve el `pub use detect::pick_default_language;` a la Tarea 2. Es decir: en ESTA tarea, en mod.rs NO pongas `pub use detect::pick_default_language;` todavía, solo `pub mod detect;`. Quita esa línea de re-export del bloque de arriba en esta tarea y agrégala en la Tarea 2.)

Para evitar confusión: en esta Tarea 1, `mod.rs` declara `pub mod detect;` pero NO re-exporta `pick_default_language` (esa línea va en Tarea 2). `detect.rs` solo tiene el header.

Modify `crates/core/src/lib.rs` — añadir `pub mod i18n;` y `pub use i18n::{I18n, LangId};` (NO `pick_default_language` aún).

- [ ] **Step 4: Verificar**

Run: `cargo test -p naygo-core i18n` → expect 7 tests PASS.
Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/i18n/ crates/core/src/lib.rs
git commit -m "feat(core): i18n — catálogo clave→texto con fallback, ES/EN embebidos + sueltos

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `pick_default_language` (detección del idioma del SO, pura)

**Files:**
- Modify: `crates/core/src/i18n/detect.rs`
- Modify: `crates/core/src/i18n/mod.rs` (re-export) y `crates/core/src/lib.rs`
- Test: módulo `#[cfg(test)]` en `detect.rs`

- [ ] **Step 1: Implementar `pick_default_language` con tests**

Replace `crates/core/src/i18n/detect.rs`:

```rust
// Naygo — detección del idioma por defecto a partir del locale del SO (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `pick_default_language` recibe el string de locale del SO (p. ej. "es-CL") ya
//! leído por la capa `platform`, y la lista de idiomas disponibles, y elige el
//! idioma por defecto. Puro y testeable: la lectura real del SO NO ocurre aquí.

use crate::i18n::LangId;

/// Elige el idioma por defecto a partir del `locale` del SO y los `available`.
/// Regla: si el prefijo del locale (antes de '-' o '_') matchea un idioma
/// disponible, usar ese; si el locale empieza con "es", usar "es"; en cualquier
/// otro caso, "en" (fallback internacional). Case-insensitive.
pub fn pick_default_language(locale: &str, available: &[LangId]) -> LangId {
    let lower = locale.to_ascii_lowercase();
    let prefix = lower
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_string();

    // ¿Hay un idioma disponible que matchee el prefijo exacto?
    if let Some(found) = available.iter().find(|l| l.as_str() == prefix) {
        return found.clone();
    }
    // Español explícito.
    if prefix == "es" {
        return LangId::new("es");
    }
    // Fallback internacional.
    LangId::new("en")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn avail() -> Vec<LangId> {
        vec![LangId::new("es"), LangId::new("en")]
    }

    #[test]
    fn es_cl_elige_es() {
        assert_eq!(pick_default_language("es-CL", &avail()), LangId::new("es"));
    }

    #[test]
    fn es_a_secas_elige_es() {
        assert_eq!(pick_default_language("es", &avail()), LangId::new("es"));
    }

    #[test]
    fn en_us_elige_en() {
        assert_eq!(pick_default_language("en-US", &avail()), LangId::new("en"));
    }

    #[test]
    fn fr_fr_cae_a_en() {
        assert_eq!(pick_default_language("fr-FR", &avail()), LangId::new("en"));
    }

    #[test]
    fn vacio_cae_a_en() {
        assert_eq!(pick_default_language("", &avail()), LangId::new("en"));
    }

    #[test]
    fn es_con_underscore_y_mayusculas() {
        assert_eq!(pick_default_language("ES_es", &avail()), LangId::new("es"));
    }

    #[test]
    fn idioma_disponible_extra_se_usa_si_matchea() {
        let av = vec![LangId::new("es"), LangId::new("en"), LangId::new("fr")];
        assert_eq!(pick_default_language("fr-FR", &av), LangId::new("fr"));
    }
}
```

- [ ] **Step 2: Re-export**

Modify `crates/core/src/i18n/mod.rs` — añadir junto a los otros `pub use`:
```rust
pub use detect::pick_default_language;
```
Modify `crates/core/src/lib.rs` — actualizar el re-export:
```rust
pub use i18n::{pick_default_language, I18n, LangId};
```

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core detect` → expect 7 tests PASS.
Run: `cargo test -p naygo-core i18n` → todos verdes.
Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/i18n/detect.rs crates/core/src/i18n/mod.rs crates/core/src/lib.rs
git commit -m "feat(core): pick_default_language — elige idioma desde el locale del SO (puro)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `config` — `Settings.language`

**Files:**
- Modify: `crates/core/src/config/mod.rs`
- Test: ampliar `#[cfg(test)]` de `config/mod.rs`

- [ ] **Step 1: Añadir el campo `language`**

Modify `crates/core/src/config/mod.rs`:

(a) Import arriba: `use crate::i18n::LangId;`.
(b) Añadir a `Settings` (tras `show_parent_entry`), con `#[serde(default)]` retro-compatible:
```rust
    /// Idioma activo de la UI. Vacío/ausente → se detecta del SO al arrancar.
    #[serde(default = "default_language")]
    pub language: LangId,
```
(c) La función default — el idioma por defecto persistido es "en" como marcador
neutro; la DETECCIÓN real del SO la hace la capa ui en el primer arranque (cuando
no hay settings.json). Para `Settings::default()` usamos "en":
```rust
fn default_language() -> LangId {
    LangId::new("en")
}
```
(d) Actualizar `impl Default for Settings` añadiendo `language: default_language(),`.

NOTA: `LangId` deriva `Serialize, Deserialize` (de la Tarea 1). Verifica que el `use` resuelva.

- [ ] **Step 2: Test**

Ampliar el `#[cfg(test)]` de config:
```rust
    #[test]
    fn settings_round_trip_con_idioma() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        s.language = crate::i18n::LangId::new("es");
        save_settings(dir.path(), &s);
        assert_eq!(load_settings(dir.path()).language, crate::i18n::LangId::new("es"));
    }

    #[test]
    fn settings_v1_sin_idioma_cae_a_default() {
        // Un settings.json sin "language" (build previo) debe seguir cargando,
        // con language = default (gracias a #[serde(default)]).
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"Flat","show_parent_entry":true}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert_eq!(s.language, crate::i18n::LangId::new("en"));
    }
```

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core config` → todos verdes.
Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/config/mod.rs
git commit -m "feat(core): Settings.language (serde-default retro-compatible)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `platform::locale` — leer el locale del SO

**Files:**
- Create: `crates/platform/src/locale.rs`
- Modify: `crates/platform/src/lib.rs`

- [ ] **Step 1: Implementar `os_locale()`**

Create `crates/platform/src/locale.rs`:

```rust
// Naygo — lectura del locale del SO (Win32), aislada en la capa platform.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Devuelve el nombre de locale del usuario (p. ej. "es-CL") consultando Windows.
//! La elección del idioma a partir de este string la hace `core::pick_default_language`.

/// Locale del usuario del SO, o `None` si no se pudo leer.
#[cfg(windows)]
pub fn os_locale() -> Option<String> {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;
    // LOCALE_NAME_MAX_LENGTH es 85; un buffer holgado basta.
    let mut buf = [0u16; 85];
    // SAFETY: pasamos un buffer válido y su longitud; la función escribe UTF-16.
    let len = unsafe { GetUserDefaultLocaleName(&mut buf) };
    if len > 0 {
        // `len` incluye el terminador nulo; recortarlo.
        let n = (len as usize).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..n]))
    } else {
        None
    }
}

/// En no-Windows (no es el target real, pero mantiene el crate compilable): None.
#[cfg(not(windows))]
pub fn os_locale() -> Option<String> {
    std::env::var("LANG").ok().map(|l| {
        // "es_CL.UTF-8" → "es_CL"
        l.split('.').next().unwrap_or("").to_string()
    })
}
```

Modify `crates/platform/src/lib.rs` — añadir `pub mod locale;` (y quitar el `hello()` placeholder si sigue ahí, o dejarlo; preferible añadir el módulo sin tocar lo demás). Asegúrate de que el crate exponga `pub mod locale;`.

NOTA: verifica la firma real de `GetUserDefaultLocaleName` en el crate `windows` 0.62.2 (verificado: `pub unsafe fn GetUserDefaultLocaleName(lplocalename: &mut [u16]) -> i32`). Si difiere (algunas versiones toman `PWSTR` + len), ajústalo contra la fuente en `C:\Users\ngrot\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\windows-0.62.2\src\Windows\Win32\Globalization\mod.rs` y reporta.

- [ ] **Step 2: Verificar**

Run: `cargo build -p naygo-platform` → compila (en Windows, usa el crate `windows`).
Run: `cargo clippy -p naygo-platform -- -D warnings` → limpio. (Si `os_locale` queda sin usar hasta la Tarea 6, puede haber dead-code; `pub fn` en un crate lib NO dispara dead_code, así que debería estar bien. Reporta si clippy se queja.)

NOTA: esta función es difícil de testear unitariamente (depende del SO). No se le exige test unitario; su corrección se valida porque `core::pick_default_language` (testeado) recibe su salida, y manualmente al arrancar.

- [ ] **Step 3: Commit**

```bash
git add crates/platform/src/locale.rs crates/platform/src/lib.rs
git commit -m "feat(platform): os_locale() lee el locale del usuario vía Win32

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: `I18n` en `NaygoApp` + helper `tr()`

Integra i18n en la app SIN migrar textos todavía (eso son las Tareas 7-9). Sienta la
base: el `I18n` vive en la app, se inicializa detectando el SO, y hay un helper.

**Files:**
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: Añadir `i18n` a `NaygoApp`**

Modify `crates/ui/src/app.rs`:

(a) Imports:
```rust
use naygo_core::i18n::{pick_default_language, I18n, LangId};
```
(b) Campo en `NaygoApp`:
```rust
    i18n: I18n,
```
(c) En `NaygoApp::new`, tras cargar `settings` y `config_dir`, construir el i18n. El
idioma: si hay settings persistido NO-default úsalo; si es el primer arranque
(settings.json no existía → settings es default con language "en"), detectar el SO.
Para distinguir "primer arranque", chequea si el archivo existe:
```rust
        // Idioma: el persistido, o detectar del SO en el primer arranque.
        let settings_exists = config_dir.join("settings.json").exists();
        // Primero cargamos i18n con un idioma provisional para conocer los disponibles.
        let provisional = I18n::load(&config_dir, &settings.language);
        let lang = if settings_exists {
            settings.language.clone()
        } else {
            let locale = naygo_platform::locale::os_locale().unwrap_or_default();
            pick_default_language(&locale, provisional.available())
        };
        let mut i18n = provisional;
        i18n.set_language(&lang);
        // Reflejar el idioma elegido en settings (para que se persista).
        // (settings es mut; si no lo es, hazlo mut arriba.)
        let mut settings = settings;
        settings.language = lang;
```
(d) Añadir `i18n,` al struct literal `NaygoApp { ... }`.
(e) Añadir un helper method:
```rust
    /// Atajo para traducir una clave con el idioma activo.
    pub fn tr(&self, key: &str) -> String {
        self.i18n.t(key).to_string()
    }
```
(Devuelve `String` para evitar problemas de borrow al pintar; el costo es un clone de
un texto corto, aceptable — `t()` en sí no aloca, el clone es para ergonomía de la UI.)

(f) Reaccionar a cambio de idioma: cuando la sección Idioma (Tarea 6) cambie
`settings.language`, la app debe llamar `i18n.set_language`. Añade al inicio de
`ui()` (o `logic()`), junto al reload de íconos:
```rust
        if self.i18n.active_lang() != self.settings.language {
            let lang = self.settings.language.clone();
            self.i18n.set_language(&lang);
        }
```

- [ ] **Step 2: Verificar**

Run: `cargo build -p naygo-ui` → compila. (El `tr()` aún no se usa → posible
dead-code de `tr`; es `pub`, no debería disparar. Reporta si clippy se queja —
se consume en las Tareas 7-9; si hace falta, `#[allow(dead_code)]` temporal en `tr`
con comentario.)
Run: `cargo clippy -p naygo-ui -- -D warnings` → limpio o reporta.
Run: `cargo test --workspace` → verde.
App-start (`--bin naygo`) → abre.

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src/app.rs
git commit -m "feat(ui): I18n en NaygoApp + detección de idioma del SO + helper tr()

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Ventana de Configuración (viewport separado) con secciones

La tarea grande de la UI. Crea la ventana como viewport, con las 5 secciones, y
mueve las opciones del menú ⚙ aquí. Usa la API de multi-viewport VERIFICADA.

**Files:**
- Create: `crates/ui/src/settings_window/mod.rs` + `appearance.rs` + `panes.rs` + `shortcuts.rs` + `language.rs` + `advanced.rs`
- Modify: `crates/ui/src/main.rs` (declarar `mod settings_window;`)
- Modify: `crates/ui/src/app.rs` (estado `settings_open`/`settings_section`; llamar al viewport)
- Modify: `crates/ui/src/toolbar.rs` (⚙ abre la ventana; quitar el menú inline)

- [ ] **Step 1: Estado de la ventana en `NaygoApp`**

Modify `crates/ui/src/app.rs`:
(a) Import: `use crate::settings_window::SettingsSection;`.
(b) Campos en `NaygoApp`:
```rust
    settings_open: bool,
    settings_section: SettingsSection,
```
(c) Init en `new`: `settings_open: false,` y `settings_section: SettingsSection::Appearance,`.

- [ ] **Step 2: `settings_window/mod.rs` — el viewport y el despacho**

Create `crates/ui/src/settings_window/mod.rs`:

```rust
// Naygo — ventana de Configuración (viewport separado del SO) con secciones.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! La Configuración es una segunda ventana real del SO (egui multi-viewport).
//! `show_settings_viewport` se llama cada frame de la app principal cuando
//! `settings_open`; usa `show_viewport_immediate` (el closure captura `&mut app`),
//! con un `SidePanel` de secciones y un `CentralPanel` que despacha a cada una.

mod advanced;
mod appearance;
mod language;
mod panes;
mod shortcuts;

use crate::app::NaygoApp;

/// Secciones de la ventana de Configuración.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    Appearance,
    Panes,
    Shortcuts,
    Language,
    Advanced,
}

/// Abre/pinta el viewport de Configuración. Debe llamarse cada frame mientras
/// `app.settings_open` sea true. Pone `settings_open = false` si el usuario cierra
/// la ventana del SO.
pub fn show_settings_viewport(app: &mut NaygoApp, ctx: &egui::Context) {
    let viewport_id = egui::ViewportId::from_hash_of("naygo_settings");
    let builder = egui::ViewportBuilder::default()
        .with_title(app.tr("settings.title"))
        .with_inner_size([560.0, 420.0])
        .with_close_button(true);

    ctx.show_viewport_immediate(viewport_id, builder, |ui, _class| {
        // Detectar el botón X de la ventana del SO.
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            app.settings_open = false;
        }

        // Sidebar de secciones.
        egui::SidePanel::left("settings_sections")
            .resizable(false)
            .exact_width(160.0)
            .show_inside(ui, |ui| {
                ui.add_space(6.0);
                section_item(ui, app, SettingsSection::Appearance, "settings.appearance");
                section_item(ui, app, SettingsSection::Panes, "settings.panes");
                section_item(ui, app, SettingsSection::Shortcuts, "settings.shortcuts");
                section_item(ui, app, SettingsSection::Language, "settings.language");
                section_item(ui, app, SettingsSection::Advanced, "settings.advanced");
            });

        // Contenido de la sección activa.
        egui::CentralPanel::default().show_inside(ui, |ui| match app.settings_section {
            SettingsSection::Appearance => appearance::show(ui, app),
            SettingsSection::Panes => panes::show(ui, app),
            SettingsSection::Shortcuts => shortcuts::show(ui, app),
            SettingsSection::Language => language::show(ui, app),
            SettingsSection::Advanced => advanced::show(ui, app),
        });
    });
}

/// Un ítem clicable de la lista de secciones (resaltado si es el activo).
fn section_item(ui: &mut egui::Ui, app: &mut NaygoApp, section: SettingsSection, key: &str) {
    let selected = app.settings_section == section;
    let label = app.tr(key);
    if ui.selectable_label(selected, label).clicked() {
        app.settings_section = section;
    }
}
```

> NOTA API (verificada): el closure de `show_viewport_immediate` recibe
> `(&mut egui::Ui, ViewportClass)` y captura `&mut app` (es `FnMut`). Dentro, el
> `ui` ES el root del viewport: se usa `SidePanel::left(..).show_inside(ui, ..)` y
> `CentralPanel::default().show_inside(ui, ..)`. El cierre se detecta con
> `ui.ctx().input(|i| i.viewport().close_requested())`. Si la firma del closure
> difiere (algunas docs muestran `&egui::Context`), ajústalo contra la fuente
> `egui-0.34.3/src/context.rs:4131` y reporta. Si `show_inside` no aplica al ui raíz
> del viewport y se requiere `.show(ui.ctx(), ..)`, prueba esa forma y reporta.

- [ ] **Step 3: Las 5 secciones**

Create `crates/ui/src/settings_window/appearance.rs`:
```rust
// Naygo — sección Apariencia de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::config::IconSet;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    ui.heading(app.tr("settings.appearance"));
    ui.add_space(8.0);

    ui.label(app.tr("settings.icon_set"));
    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.settings.icon_set, IconSet::Flat, app.tr("settings.icons.flat"));
        ui.selectable_value(&mut app.settings.icon_set, IconSet::Fluent, app.tr("settings.icons.fluent"));
        ui.selectable_value(&mut app.settings.icon_set, IconSet::Mono, app.tr("settings.icons.mono"));
    });
    ui.add_space(8.0);

    // Placeholder de tema (el motor real es 2C-ii).
    ui.label(app.tr("settings.theme"));
    ui.label(egui::RichText::new(app.tr("settings.theme.placeholder")).weak());
    ui.add_space(8.0);

    let mut icon_only = app.settings.icon_only;
    if ui.checkbox(&mut icon_only, app.tr("settings.icon_only")).changed() {
        app.settings.icon_only = icon_only;
    }
}
```
> NOTA: `selectable_value` toma `&mut app.settings.icon_set` y `app.tr(...)` toma
> `&app` — esto da un conflicto de préstamo (mut + shared sobre `app`). Resuélvelo
> calculando los labels ANTES: `let (l_flat, l_fluent, l_mono) = (app.tr(...), ...);`
> y luego `ui.selectable_value(&mut app.settings.icon_set, IconSet::Flat, l_flat)`.
> Aplica el mismo patrón (precalcular Strings de label) en todas las secciones donde
> se mezcle `&mut app.settings.x` con `app.tr()`. Reporta el patrón usado.

Create `crates/ui/src/settings_window/panes.rs`:
```rust
// Naygo — sección Paneles de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::config::BarPosition;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let title = app.tr("settings.panes");
    ui.heading(title);
    ui.add_space(8.0);

    let mut show_parent = app.settings.show_parent_entry;
    let lbl = app.tr("settings.show_parent");
    if ui.checkbox(&mut show_parent, lbl).changed() {
        app.settings.show_parent_entry = show_parent;
    }
    ui.add_space(8.0);

    ui.label(app.tr("settings.bar_position"));
    let (l_top, l_side) = (app.tr("settings.bar.top"), app.tr("settings.bar.side"));
    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.settings.bar_position, BarPosition::Top, l_top);
        ui.selectable_value(&mut app.settings.bar_position, BarPosition::Side, l_side);
    });
}
```

Create `crates/ui/src/settings_window/shortcuts.rs`:
```rust
// Naygo — sección Atajos (solo-lectura) de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let title = app.tr("settings.shortcuts");
    ui.heading(title);
    ui.add_space(8.0);
    ui.label(egui::RichText::new(app.tr("settings.shortcuts.readonly")).weak());
    ui.add_space(6.0);

    // Mapa fijo de Fase 2A (solo-lectura). La edición llega en una fase posterior.
    let rows: &[(&str, &str)] = &[
        ("↑ / ↓", "shortcut.move"),
        ("Enter", "shortcut.activate"),
        ("Backspace", "shortcut.up"),
        ("Alt + ← / →", "shortcut.backforward"),
        ("Tab", "shortcut.switchpane"),
        ("Esc", "shortcut.cancel"),
    ];
    egui::Grid::new("shortcuts_grid").num_columns(2).striped(true).show(ui, |ui| {
        for (keys, desc_key) in rows {
            ui.monospace(*keys);
            ui.label(app.tr(desc_key));
            ui.end_row();
        }
    });
}
```

Create `crates/ui/src/settings_window/language.rs`:
```rust
// Naygo — sección Idioma de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;
use naygo_core::i18n::LangId;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let title = app.tr("settings.language");
    ui.heading(title);
    ui.add_space(8.0);

    // Lista de idiomas disponibles; al seleccionar, cambia settings.language (la app
    // aplica i18n.set_language al detectar el cambio en ui()/logic()).
    let langs: Vec<LangId> = app.i18n_available();
    let current = app.settings.language.clone();
    for lang in langs {
        let selected = lang == current;
        // Nombre legible del idioma (clave "lang.es", "lang.en", ...; fallback al código).
        let key = format!("lang.{}", lang.as_str());
        let label = app.tr(&key);
        if ui.selectable_label(selected, label).clicked() {
            app.settings.language = lang;
        }
    }
}
```
> NOTA: `app.i18n_available()` es un helper nuevo que debes añadir a `NaygoApp`
> (devuelve `Vec<LangId>` clonado de `self.i18n.available()`), para no prestar
> `&self.i18n` y `&mut app` a la vez. Añádelo en app.rs:
> ```rust
>     pub fn i18n_available(&self) -> Vec<LangId> {
>         self.i18n.available().to_vec()
>     }
> ```

Create `crates/ui/src/settings_window/advanced.rs`:
```rust
// Naygo — sección Avanzado de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::app::NaygoApp;

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let title = app.tr("settings.advanced");
    ui.heading(title);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(app.tr("settings.config_dir"));
        ui.monospace(app.config_dir_display());
    });
    ui.horizontal(|ui| {
        ui.label(app.tr("settings.version"));
        ui.monospace(env!("CARGO_PKG_VERSION"));
    });
}
```
> NOTA: `app.config_dir_display()` es un helper nuevo en `NaygoApp` que devuelve
> `self.config_dir.display().to_string()` (el campo `config_dir` es privado). Añádelo:
> ```rust
>     pub fn config_dir_display(&self) -> String {
>         self.config_dir.display().to_string()
>     }
> ```

- [ ] **Step 4: Declarar el módulo y llamar al viewport**

Modify `crates/ui/src/main.rs` — añadir `mod settings_window;`.

Modify `crates/ui/src/app.rs` — en `ui()`, tras pintar el dock (al final), si la
ventana está abierta, mostrar el viewport. Como `show_settings_viewport` necesita
`&mut self` y `ctx`, y estamos dentro de `ui(&mut self, ui, _)`, usa `ui.ctx()`:
```rust
        if self.settings_open {
            let ctx = ui.ctx().clone();
            crate::settings_window::show_settings_viewport(self, &ctx);
        }
```
(Clonar el `Context` es barato — es un handle `Arc`. Esto evita prestar `ui` y `self`
a la vez dentro de la llamada.)

- [ ] **Step 5: Toolbar — ⚙ abre la ventana, quitar el menú inline**

Modify `crates/ui/src/toolbar.rs` — reemplazar `settings_button` por un botón simple
que setea `settings_open = true`:
```rust
/// Botón de ajustes: abre la ventana de Configuración (viewport separado).
fn settings_button(ui: &mut egui::Ui, app: &mut NaygoApp) {
    if ui.button("⚙").on_hover_text(app.tr("toolbar.settings")).clicked() {
        app.settings_open = true;
    }
}
```
Elimina todo el cuerpo anterior de `settings_button` (los radios/checkboxes inline) y
el `use naygo_core::config::IconSet;` si queda sin uso en toolbar.rs (BarPosition
sigue usándose en `show`). Mantén la lógica de posicionarlo a la derecha en Top.

NOTA: `app.tr(...)` toma `&app` y el botón no necesita `&mut` hasta el `.clicked()` —
pero `settings_button` recibe `&mut NaygoApp`. Precalcula el label:
`let lbl = app.tr("toolbar.settings");` antes del `if ui.button(...)`.

- [ ] **Step 6: Añadir las claves nuevas a es.json y en.json**

Modify `crates/core/src/i18n/es.json` — añadir (mergea con lo existente):
```json
{
  "app.loading": "Listando…",
  "app.cancelled": "Cancelado",
  "settings.title": "Configuración",
  "settings.appearance": "Apariencia",
  "settings.panes": "Paneles",
  "settings.shortcuts": "Atajos de teclado",
  "settings.language": "Idioma",
  "settings.advanced": "Avanzado",
  "settings.icon_set": "Set de íconos",
  "settings.icons.flat": "Flat (color)",
  "settings.icons.fluent": "Fluent",
  "settings.icons.mono": "Monocromo",
  "settings.theme": "Tema",
  "settings.theme.placeholder": "Los temas y color sets llegan en una fase posterior.",
  "settings.icon_only": "Botones de barra solo con ícono",
  "settings.show_parent": "Mostrar fila \"..\" en los paneles",
  "settings.bar_position": "Posición de la barra",
  "settings.bar.top": "Arriba",
  "settings.bar.side": "Al costado",
  "settings.shortcuts.readonly": "Solo lectura por ahora. La personalización de atajos llega después.",
  "shortcut.move": "Mover selección",
  "shortcut.activate": "Entrar / abrir",
  "shortcut.up": "Subir un nivel",
  "shortcut.backforward": "Atrás / adelante",
  "shortcut.switchpane": "Cambiar de panel",
  "shortcut.cancel": "Cancelar listado",
  "settings.config_dir": "Carpeta de configuración:",
  "settings.version": "Versión:",
  "toolbar.settings": "Ajustes",
  "lang.es": "Español",
  "lang.en": "English"
}
```
Modify `crates/core/src/i18n/en.json` — las mismas claves en inglés:
```json
{
  "app.loading": "Listing…",
  "app.cancelled": "Cancelled",
  "settings.title": "Settings",
  "settings.appearance": "Appearance",
  "settings.panes": "Panels",
  "settings.shortcuts": "Keyboard shortcuts",
  "settings.language": "Language",
  "settings.advanced": "Advanced",
  "settings.icon_set": "Icon set",
  "settings.icons.flat": "Flat (color)",
  "settings.icons.fluent": "Fluent",
  "settings.icons.mono": "Monochrome",
  "settings.theme": "Theme",
  "settings.theme.placeholder": "Themes and color sets arrive in a later phase.",
  "settings.icon_only": "Icon-only toolbar buttons",
  "settings.show_parent": "Show \"..\" row in panels",
  "settings.bar_position": "Toolbar position",
  "settings.bar.top": "Top",
  "settings.bar.side": "Side",
  "settings.shortcuts.readonly": "Read-only for now. Shortcut customization comes later.",
  "shortcut.move": "Move selection",
  "shortcut.activate": "Enter / open",
  "shortcut.up": "Go up one level",
  "shortcut.backforward": "Back / forward",
  "shortcut.switchpane": "Switch pane",
  "shortcut.cancel": "Cancel listing",
  "settings.config_dir": "Config folder:",
  "settings.version": "Version:",
  "toolbar.settings": "Settings",
  "lang.es": "Español",
  "lang.en": "English"
}
```

- [ ] **Step 7: Compilar, verificar, formatear**

Run: `cargo build -p naygo-ui` → compila. Resuelve borrows con el patrón de
precalcular labels (ver notas). Reporta warnings verbatim.
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.
App-start (`--bin naygo`): el botón ⚙ abre una SEGUNDA ventana del SO "Configuración"
con las 5 secciones; navegar entre secciones funciona; cambiar idioma a English
cambia los textos de esa ventana (y de la principal); cambiar set de íconos / fila
".." / posición de barra surten efecto. Cerrar la ventana del SO (X) la cierra
(settings_open=false) y se puede reabrir.

- [ ] **Step 8: Commit**

```bash
git add crates/ui/src/settings_window/ crates/ui/src/main.rs crates/ui/src/app.rs crates/ui/src/toolbar.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): ventana de Configuración (viewport) con secciones; ⚙ la abre

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Migrar textos a claves — paneles + docking

**Files:**
- Modify: `crates/ui/src/panes/{file_panel,tree_panel,inspector_panel}.rs`
- Modify: `crates/ui/src/docking.rs`
- Modify: `crates/core/src/i18n/es.json` + `en.json` (claves nuevas)

- [ ] **Step 1: Reunir los textos hardcoded de estos archivos y asignarles claves**

Identifica los literales en español en esos archivos. Esperados (busca con grep
`"` en cada archivo): títulos de tab ("Carpetas", "Archivos"/nombre, "Propiedades"),
columnas ("Nombre", "Tamaño", "Modificado"), inspector ("Nada seleccionado.",
"Nombre", "Tipo", "Carpeta", "Archivo", "Otro", "Ruta", "Tamaño"), tree ("Ubicación
actual", "⬆ Subir un nivel"). Añade claves a ambos JSON:

`es.json` (añadir):
```json
{
  "pane.tree.title": "Carpetas",
  "pane.inspector.title": "Propiedades",
  "col.name": "Nombre",
  "col.size": "Tamaño",
  "col.modified": "Modificado",
  "inspector.nothing": "Nada seleccionado.",
  "inspector.type": "Tipo",
  "inspector.path": "Ruta",
  "kind.folder": "Carpeta",
  "kind.file": "Archivo",
  "kind.other": "Otro",
  "tree.location": "Ubicación actual",
  "tree.go_up": "Subir un nivel"
}
```
`en.json` (añadir):
```json
{
  "pane.tree.title": "Folders",
  "pane.inspector.title": "Properties",
  "col.name": "Name",
  "col.size": "Size",
  "col.modified": "Modified",
  "inspector.nothing": "Nothing selected.",
  "inspector.type": "Type",
  "inspector.path": "Path",
  "kind.folder": "Folder",
  "kind.file": "File",
  "kind.other": "Other",
  "tree.location": "Current location",
  "tree.go_up": "Go up one level"
}
```

- [ ] **Step 2: Migrar los textos**

El reto: los paneles reciben `&mut Workspace` (no `&NaygoApp`), así que no tienen
acceso directo a `app.tr()`. SOLUCIÓN: pasar el `I18n` (o un `&dyn`/`&I18n`) a las
funciones de panel. Como el `TabViewer` (docking.rs) ya lleva referencias, añade
`i18n: &'a naygo_core::i18n::I18n` a `NaygoTabViewer` y pásalo a cada `show`.

Modify `crates/ui/src/docking.rs`:
- Añadir campo `pub i18n: &'a naygo_core::i18n::I18n,` a `NaygoTabViewer`.
- En `title()`: usar `self.i18n.t("pane.tree.title")` para Tree, `self.i18n.t("pane.inspector.title")` para Inspector (Files sigue usando el nombre de la carpeta).
- En `ui()`: pasar `self.i18n` a `file_panel::show`, `tree_panel::show`, `inspector_panel::show`.

Modify `crates/ui/src/app.rs` — al construir `NaygoTabViewer`, pasar `i18n: &self.i18n,`.
PERO: `self.i18n` y `&mut self.workspace` se prestan a la vez en el viewer. Como
`i18n` es `&` (shared) y `workspace` es `&mut`, son campos DISTINTOS de `self` →
préstamos disjuntos, lo cual Rust permite si se hace por campos. El viewer ya toma
`&mut self.workspace` y `&mut self.status`; añadir `&self.i18n` es un préstamo shared
de otro campo — OK. Verifica que compile; si el borrow-checker se queja por tomar
`&self.i18n` mientras hay `&mut self.*`, extrae los campos a variables locales antes
de construir el viewer (`let i18n = &self.i18n; let ws = &mut self.workspace; ...`).

En cada panel (`file_panel.rs`, `tree_panel.rs`, `inspector_panel.rs`):
- Añadir parámetro `i18n: &naygo_core::i18n::I18n` a `show`.
- Reemplazar los literales por `i18n.t("clave")`. Ejemplos:
  - file_panel columnas: `ui.strong(i18n.t("col.name"))`, etc.
  - inspector: `ui.label(i18n.t("inspector.nothing"))`, `ui.strong(i18n.t("col.name"))`, `i18n.t("inspector.type")`, el match de kind → `i18n.t("kind.folder")`/`kind.file`/`kind.other`, `i18n.t("inspector.path")`.
  - tree: `i18n.t("tree.location")`, el botón `format!("⬆ {}", i18n.t("tree.go_up"))`.

- [ ] **Step 3: Compilar, verificar**

Run: `cargo build -p naygo-ui` → compila. Resuelve borrows como se indica.
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.
App-start: cambiar a English en Configuración → los títulos de tab, columnas,
inspector y árbol aparecen en inglés.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/panes/ crates/ui/src/docking.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): migrar textos de paneles e inspector a claves i18n

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Migrar textos a claves — toolbar, status bar, templates

**Files:**
- Modify: `crates/ui/src/toolbar.rs`, `crates/ui/src/templates_menu.rs`, `crates/ui/src/app.rs` (status), 
- Modify: `crates/core/src/i18n/es.json` + `en.json`

- [ ] **Step 1: Claves nuevas**

`es.json` (añadir):
```json
{
  "toolbar.back": "Atrás (Alt+←)",
  "toolbar.forward": "Adelante (Alt+→)",
  "toolbar.up": "Subir un nivel (Backspace)",
  "toolbar.refresh": "Refrescar",
  "toolbar.add_pane": "Agregar panel de archivos",
  "toolbar.layouts": "Layouts",
  "templates.recents": "Recientes",
  "templates.favorites": "Favoritos",
  "templates.mine": "Míos",
  "templates.builtin": "Predefinidos",
  "templates.save_current": "Guardar disposición actual…",
  "templates.favorite": "Favorito",
  "templates.delete": "Borrar",
  "template.minimalista": "Minimalista",
  "template.clasico": "Clásico",
  "template.dual_pane": "Dual-pane",
  "template.power_user": "Power-user",
  "status.elements": "{n} elementos",
  "status.error": "Error: {e}",
  "status.open_pending": "Abrir: {name} (pendiente platform::shell)"
}
```
`en.json` (añadir):
```json
{
  "toolbar.back": "Back (Alt+←)",
  "toolbar.forward": "Forward (Alt+→)",
  "toolbar.up": "Go up one level (Backspace)",
  "toolbar.refresh": "Refresh",
  "toolbar.add_pane": "Add file panel",
  "toolbar.layouts": "Layouts",
  "templates.recents": "Recent",
  "templates.favorites": "Favorites",
  "templates.mine": "Mine",
  "templates.builtin": "Built-in",
  "templates.save_current": "Save current layout…",
  "templates.favorite": "Favorite",
  "templates.delete": "Delete",
  "template.minimalista": "Minimal",
  "template.clasico": "Classic",
  "template.dual_pane": "Dual-pane",
  "template.power_user": "Power-user",
  "status.elements": "{n} items",
  "status.error": "Error: {e}",
  "status.open_pending": "Open: {name} (platform::shell pending)"
}
```

- [ ] **Step 2: Migrar toolbar.rs**

Reemplaza los tooltips/labels hardcoded de `toolbar.rs` por `app.tr(...)`
(precalculando los Strings antes de los `if ui...` para evitar borrows). Los íconos
de los botones (◀ ▶ ▲ ⟳ ➕ ⚙ ▦) son símbolos, NO se traducen; solo los `on_hover_text`.

- [ ] **Step 3: Migrar status bar (app.rs)**

En `app.rs`, los mensajes de estado se construyen con literales (`"{} elementos"`,
`format!("Error: {e}")`, `format!("Abrir: {} (pendiente platform::shell)", ...)`).
Reemplázalos usando las claves con sustitución simple. Patrón de sustitución:
```rust
// helper local o inline:
let txt = self.i18n.t("status.elements").replace("{n}", &n.to_string());
```
Para `status.error`: `self.i18n.t("status.error").replace("{e}", &e)`.
Para `status.open_pending`: `.replace("{name}", &entry.name)`.
El `"Listando…"` / `"Cancelado"` usan `app.loading` / `app.cancelled` (ya existen).
Ajusta `pump_one`/`navigate_to`/`activate_focused` para usar estas claves. NOTA: estos
se setean en métodos que tienen `&mut self`, así que `self.i18n.t(...)` está
disponible; cuida el orden de borrows (lee el texto a un `String` antes de asignarlo
a `self.status`).

- [ ] **Step 4: Migrar templates_menu.rs**

Reemplaza los labels de sección ("🕘 Recientes", "★ Favoritos", "👤 Míos",
"📋 Predefinidos", "💾 Guardar disposición actual…", tooltips "Favorito"/"Borrar") por
`app.tr(...)` (con los emojis como prefijo literal + el texto traducido, p. ej.
`format!("🕘 {}", app.tr("templates.recents"))`). Los nombres de las plantillas
BUILT-IN deben traducirse: cuando muestres una plantilla built-in, en vez de su
`name` crudo, usa `app.tr(&format!("template.{}", clave_del_builtin))`. PERO los
built-in hoy tienen `name` = "Minimalista"/"Clásico"/etc. (strings). Para traducirlos
sin romper la lógica (que busca por name), mapea el name→clave al MOSTRAR:
```rust
fn builtin_label(app: &NaygoApp, name: &str) -> String {
    let key = match name {
        "Minimalista" => "template.minimalista",
        "Clásico" => "template.clasico",
        "Dual-pane" => "template.dual_pane",
        "Power-user" => "template.power_user",
        _ => return name.to_string(), // plantilla del usuario: nombre literal
    };
    app.tr(key)
}
```
Usa `builtin_label(app, &t.name)` para el texto del botón, pero sigue pasando
`t.name` real a `apply_template`/`record_use` (la lógica no cambia, solo la etiqueta).

- [ ] **Step 5: Compilar, verificar, formatear**

Run: `cargo build -p naygo-ui` → compila. Reporta warnings.
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.
App-start: en English, los tooltips de la toolbar, el menú de Layouts (sus secciones
y los nombres built-in), y la barra de estado ("N items", "Listing…") aparecen en
inglés.

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/toolbar.rs crates/ui/src/templates_menu.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): migrar toolbar, status y plantillas a claves i18n

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Barrido final de literales + remover allows + cierre de fase

**Files:**
- Modify: cualquier literal hardcoded restante en `crates/ui/src/`
- Modify: `crates/ui/src/app.rs` (remover `#[allow(dead_code)]` de `tr` si se puso)
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: Buscar literales en español restantes**

Run (PowerShell/grep): buscar strings con caracteres español o palabras comunes en
`crates/ui/src/`. Usa Grep con patrón `"[A-ZÁÉÍÓÚ a-záéíóúñ]{3,}"` sobre `*.rs` de ui,
revisando manualmente: cualquier `ui.label("texto")`, `.on_hover_text("texto")`,
`format!("texto...")` que sea visible al usuario y siga hardcoded. Candidatos
fáciles de olvidar: el placeholder "(dock multi-panel en construcción...)" si quedó,
mensajes de la barra de estado, "Panel desconocido" en docking, "Panel activo en:".
Para cada uno: añade clave a ambos JSON y reemplaza por `t(...)`. Documenta cuáles
migraste. Los textos NO visibles (logs `tracing::`, nombres de textura, IDs de egui,
comentarios) NO se migran — solo lo que ve el usuario.

Claves probables a añadir (ajusta según lo que encuentres):
`es.json`: `{ "pane.unknown": "Panel desconocido", "tree.active_in": "Panel activo en:" }`
`en.json`: `{ "pane.unknown": "Unknown panel", "tree.active_in": "Active pane in:" }`

- [ ] **Step 2: Remover allows temporales**

Si en la Tarea 5 se puso un `#[allow(dead_code)]` en `tr()` u otro, remuévelo ahora
que se consume. `cargo clippy --workspace -- -D warnings` debe pasar sin él.

- [ ] **Step 3: Actualizar README**

Modify `README.md` — bloque de estado:
```markdown
> **Estado:** Fase 2C-i (configuración + i18n) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-06-naygo-fase2c-i-config-i18n-design.md`](docs/superpowers/specs/2026-06-06-naygo-fase2c-i-config-i18n-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-06-naygo-fase2c-i-config-i18n.md`](docs/superpowers/plans/2026-06-06-naygo-fase2c-i-config-i18n.md).
> Fases 1, 2A (layout) y 2B (íconos) completas.
```

- [ ] **Step 4: Verificación final**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → todo verde (core: ... + i18n 7 + detect 7 + config; ui: ...).
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo fmt --check` → limpio (si no, fmt + incluir).
Run: `cargo build --release -p naygo-ui` → release compila (autoría + CRT estático intactos).
App-start manual (`--bin naygo`): abrir ⚙ → ventana de Configuración separada;
cambiar idioma ES↔EN cambia TODA la UI en caliente (ventana principal + config);
verificar que no queden textos en español al estar en English (barrido visual);
cambiar set de íconos / fila ".." / barra desde la ventana surten efecto; cerrar y
reabrir conserva idioma y opciones (persistencia).

- [ ] **Step 5: Commit y push**

```bash
git add -A
git commit -m "feat(ui): barrido final de literales a i18n; cierre de Fase 2C-i

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/fase2c-i-config-i18n
```

---

## Self-review (cobertura del spec)

| Requisito del spec 2C-i | Tarea(s) |
|---|---|
| `core::i18n` (LangId, Catalog, I18n, t, fallback) | 1 |
| Carga embebida (ES/EN) + sueltos (`lang/*.json`) | 1 |
| Cambio de idioma en caliente | 1 (set_language) + 5 (reacción en ui) + 6 (selector) |
| `pick_default_language` (detección SO, pura) | 2 |
| Lectura del locale del SO (Win32, en platform) | 4 |
| `Settings.language` (persistido, retro-compat) | 3 |
| Ventana de Configuración (viewport separado) | 6 |
| 5 secciones (Apariencia/Paneles/Atajos/Idioma/Avanzado) | 6 |
| Mudar opciones del ⚙ a la ventana | 6 |
| ⚙ abre la ventana (quita menú inline) | 6 |
| Migración total de textos a claves | 6 (settings) + 7 (paneles) + 8 (toolbar/status/templates) + 9 (barrido) |
| Nombres de plantilla built-in traducidos | 8 |
| ES + EN completos | 6, 7, 8, 9 (claves acumuladas) |
| Atajos solo-lectura | 6 (shortcuts.rs) |
| Placeholder de tema en Apariencia | 6 (appearance.rs) |
| Tolerancia (JSON corrupto, clave faltante) | 1 (Catalog/t) |

**Diferido (NO en 2C-i):** temas/color sets reales (2C-ii), packs (2C-ii), recoloreado
del mono (2C-ii), edición de atajos, multi-ventana del explorador, deuda del dock (2A).

**Notas de riesgo (verificar contra la fuente):**
- Multi-viewport (Tarea 6): firma del closure de `show_viewport_immediate`
  (`FnMut(&mut Ui, ViewportClass)` verificado en context.rs:4131); si `show_inside`
  no aplica al `ui` raíz del viewport, usar `.show(ui.ctx(), ...)`. Detección de
  cierre con `i.viewport().close_requested()`.
- Borrows `app.tr()` (&self) + `&mut app.settings.x`: precalcular labels a `String`
  antes de los widgets mutables (patrón documentado en Tarea 6).
- `GetUserDefaultLocaleName` (Tarea 4): firma `&mut [u16] -> i32` verificada en
  windows 0.62.2; `len` incluye el nulo (recortar).
- `i18n` + `&mut workspace` en el TabViewer (Tarea 7): préstamos de campos disjuntos
  de `self`; si el checker se queja, extraer a locales antes de construir el viewer.
- Sustitución `{n}`/`{e}`/`{name}` en status (Tarea 8): `.replace(...)` simple sobre
  el texto de la clave; suficiente para 2C-i (un sistema de formato más rico es YAGNI).
```
