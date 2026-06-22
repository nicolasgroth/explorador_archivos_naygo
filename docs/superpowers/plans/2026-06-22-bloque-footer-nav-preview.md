# Bloque footer + navegación + auto-resaltado + copia-preview — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agregar a Naygo un footer por panel configurable, botones Atrás/Adelante/Home en el toolbar, un toggle global de auto-resaltado de código, y selección+copia en el preview de texto.

**Architecture:** Lógica pura nueva en `naygo-core` (módulo `footer`, resolución de Home, parámetro `auto_highlight` en `code_lang_for`), tres campos nuevos en `Settings` (con `#[serde(default)]` para migrar), y cableado en `naygo-ui-slint` (toolbar, file-panel, preview-panel, config-window). El hilo de UI no hace I/O nuevo por pulsación: el `DiskUsage` del footer se cachea por panel.

**Tech Stack:** Rust workspace (naygo-core / naygo-platform / naygo-ui-slint), Slint 1.16 render software, serde, syntect (ya integrado). Build con `CARGO_BUILD_JOBS=2`.

**Gate (correr SIEMPRE uno mismo tras cada subagente):**
```
$env:CARGO_BUILD_JOBS = "2"
cargo fmt
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**i18n:** triple (es + en + estructura), español neutral SIN voseo. NO reutilizar nombres de claves existentes (gotcha conocido: colisión history-empty).

---

## Estructura de archivos

**Crear:**
- `crates/core/src/footer.rs` — módulo del footer (FooterField, FooterPreset, FooterData, render, tokens).

**Modificar (core):**
- `crates/core/src/lib.rs` — `pub mod footer;`.
- `crates/core/src/preview.rs:214` — `code_lang_for` gana parámetro `auto_highlight: bool`.
- `crates/core/src/config/mod.rs:119` — Settings: `footer_enabled`, `footer_preset`, `footer_custom_template`, `auto_highlight_code`, `home_dir` + sus fn default + `Default for Settings`.
- `crates/core/src/keymap.rs:102` — `Action::GoHome` + binding default `Alt+Home`.
- `crates/core/src/path_util.rs` (o helper nuevo en `footer.rs`/`config`) — `resolve_home_dir(home_dir: &str) -> PathBuf`.

**Modificar (ui-slint):**
- `crates/ui-slint/src/workspace_ctrl.rs` — `on_go_home`, `can_go_back`/`can_go_forward`, footer_text en sync_rows con caché de disco, pasar `auto_highlight` al worker de preview, `Action::GoHome` en el dispatcher.
- `crates/ui-slint/src/main.rs` — callbacks nuevos (nav home, footer, toggle config, preview selectable).
- `crates/ui-slint/src/config_ctrl.rs` — getters/setters de los Settings nuevos.
- `crates/ui-slint/ui/types.slint` — `footer_text` en PaneVm; flags de preview.
- `crates/ui-slint/ui/app-window.slint` — botones nav/home en toolbar + callbacks.
- `crates/ui-slint/ui/file-panel.slint` — barra footer al pie.
- `crates/ui-slint/ui/preview-panel.slint` — botón seleccionable + TextEdit.
- `crates/ui-slint/ui/config-window.slint` — sección footer + Home + toggle auto-resaltado en Avanzado/Previsualización.
- `crates/ui-slint/lang/*.json` (es/en) + `i18n` core — claves nuevas.

---

# FASE 1 — core::footer (lógica pura)

### Task 1: Tipos del footer (FooterField, FooterPreset, FooterData)

**Files:**
- Create: `crates/core/src/footer.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear el módulo con los tipos y el header**

En `crates/core/src/footer.rs`:

```rust
// Naygo — footer por panel: campos, plantillas y render de la barra inferior.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA del footer de cada panel (sin UI ni Windows). La UI calcula los datos
//! crudos por panel (`FooterData`) y `render` produce el string final según la plantilla.
//! Nunca falla: datos ausentes (disco no disponible) se muestran como `—`.

use crate::disk::DiskUsage;
use crate::format::{human_size, SizeFormat};
use serde::{Deserialize, Serialize};

/// Plantilla del footer. `Custom` lleva el template de tokens del usuario.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FooterPreset {
    /// "{sel}/{total} · {marked}"
    #[default]
    Compact,
    /// "{sel} de {total} sel · {marked} · {free} libres / {disk_total} ({pct})"
    Full,
    /// "{free} libres / {disk_total} ({pct})"
    DiskOnly,
    /// "{sel} de {total} sel · {marked}"
    SelectionOnly,
    /// Plantilla libre del usuario (con tokens).
    Custom(String),
}

impl FooterPreset {
    /// El template string asociado al preset (para Custom, el del usuario).
    pub fn template(&self) -> &str {
        match self {
            FooterPreset::Compact => "{sel}/{total} · {marked}",
            FooterPreset::Full => {
                "{sel} de {total} sel · {marked} · {free} libres / {disk_total} ({pct})"
            }
            FooterPreset::DiskOnly => "{free} libres / {disk_total} ({pct})",
            FooterPreset::SelectionOnly => "{sel} de {total} sel · {marked}",
            FooterPreset::Custom(t) => t.as_str(),
        }
    }
}

/// Datos crudos que la UI calcula por panel y pasa a `render`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FooterData {
    pub sel_count: usize,
    pub total_count: usize,
    pub marked_bytes: u64,
    /// `None` = disco no disponible (red caída, panel especial).
    pub disk: Option<DiskUsage>,
    pub item_count: usize,
    pub file_count: usize,
    pub dir_count: usize,
}
```

- [ ] **Step 2: Registrar el módulo**

En `crates/core/src/lib.rs`, junto a los otros `pub mod`, agregar (orden alfabético si lo siguen, si no al final del bloque):

```rust
pub mod footer;
```

- [ ] **Step 3: Compilar para verificar que el módulo entra**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-core`
Expected: compila sin errores (los tipos aún no se usan → puede haber warnings de dead_code, ok por ahora).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/footer.rs crates/core/src/lib.rs
git commit -m "feat(core): tipos del footer (FooterPreset, FooterData)"
```

---

### Task 2: `render` — sustitución de tokens

**Files:**
- Modify: `crates/core/src/footer.rs`

- [ ] **Step 1: Escribir los tests de render (fallan)**

Al final de `crates/core/src/footer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::DiskUsage;

    fn sample_with_disk() -> FooterData {
        FooterData {
            sel_count: 3,
            total_count: 12,
            marked_bytes: 4_404_019, // ~4,2 MB
            disk: Some(DiskUsage { total: 500_000_000_000, free: 112_000_000_000 }),
            item_count: 12,
            file_count: 8,
            dir_count: 4,
        }
    }

    #[test]
    fn compact_con_disco() {
        let s = render(&FooterPreset::Compact, &sample_with_disk(), SizeFormat::Auto);
        assert!(s.contains("3/12"), "esperaba sel/total: {s}");
        assert!(s.contains("MB"), "esperaba bytes marcados: {s}");
    }

    #[test]
    fn full_incluye_disco_y_pct() {
        let s = render(&FooterPreset::Full, &sample_with_disk(), SizeFormat::Auto);
        assert!(s.contains("3 de 12"), "{s}");
        assert!(s.contains("libres"), "{s}");
        assert!(s.contains('%'), "esperaba el porcentaje: {s}");
    }

    #[test]
    fn disco_none_muestra_guion() {
        let mut d = sample_with_disk();
        d.disk = None;
        let s = render(&FooterPreset::DiskOnly, &d, SizeFormat::Auto);
        assert!(s.contains('—'), "disco ausente debe dar —: {s}");
        assert!(!s.contains('%') || s.contains("—"), "{s}");
    }

    #[test]
    fn custom_token_desconocido_queda_literal() {
        let p = FooterPreset::Custom("{sel} {desconocido} {dirs}".to_string());
        let s = render(&p, &sample_with_disk(), SizeFormat::Auto);
        assert!(s.contains("{desconocido}"), "token raro se deja literal: {s}");
        assert!(s.contains("3"), "{s}");
        assert!(s.contains("4"), "dirs=4: {s}");
    }

    #[test]
    fn nunca_panica_con_total_cero() {
        let d = FooterData::default(); // todo 0, disco None
        let s = render(&FooterPreset::Full, &d, SizeFormat::Auto);
        assert!(!s.is_empty());
    }
}
```

- [ ] **Step 2: Correr los tests para verificar que fallan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core footer::`
Expected: FAIL — `render` no existe (no compila).

- [ ] **Step 3: Implementar `render`**

Antes del bloque `#[cfg(test)]` en `footer.rs`:

```rust
/// Renderiza el footer sustituyendo tokens en el template del preset. Nunca falla.
/// Tokens de disco con `disk == None` → `—`. Tokens desconocidos → se dejan literales.
pub fn render(preset: &FooterPreset, data: &FooterData, size_fmt: SizeFormat) -> String {
    let dash = "—";
    let (free, disk_total, pct) = match &data.disk {
        Some(u) => (
            human_size(u.free, size_fmt),
            human_size(u.total, size_fmt),
            format!("{}%", u.percent_used()),
        ),
        None => (dash.to_string(), dash.to_string(), dash.to_string()),
    };
    let marked = human_size(data.marked_bytes, size_fmt);

    // Sustitución token a token. Se aplican los más largos primero no es necesario aquí
    // porque ningún token es prefijo de otro salvo {total}/{total_disk}: usamos {disk_total}.
    let mut out = preset.template().to_string();
    let pairs: [(&str, String); 9] = [
        ("{sel}", data.sel_count.to_string()),
        ("{total}", data.total_count.to_string()),
        ("{marked}", marked),
        ("{free}", free),
        ("{disk_total}", disk_total),
        ("{pct}", pct),
        ("{items}", data.item_count.to_string()),
        ("{files}", data.file_count.to_string()),
        ("{dirs}", data.dir_count.to_string()),
    ];
    for (token, value) in pairs.iter() {
        out = out.replace(token, value);
    }
    out
}
```

- [ ] **Step 4: Correr los tests para verificar que pasan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core footer::`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/footer.rs
git commit -m "feat(core): render del footer con sustitución de tokens"
```

---

### Task 3: Resolución de la carpeta Home

**Files:**
- Modify: `crates/core/src/footer.rs` (helper) — o `crates/core/src/config/mod.rs` si se prefiere; el plan lo pone en `config` junto a Settings.

> Nota: `dirs` ya es dependencia del workspace (se usa para config_dir). Verificar con
> `cargo tree -p naygo-core | grep dirs`; si no está, usar `std::env::var("USERPROFILE")`.

- [ ] **Step 1: Escribir el test (falla)**

En `crates/core/src/config/mod.rs`, dentro del `#[cfg(test)] mod tests` existente (o crear uno):

```rust
#[test]
fn home_vacio_cae_al_perfil_del_usuario() {
    // Vacío → algún PathBuf no vacío (el perfil). No comparamos la ruta exacta (varía por
    // máquina), solo que NO quede vacío y que una ruta explícita se respete.
    let explicit = resolve_home_dir("D:\\Trabajo");
    assert_eq!(explicit, std::path::PathBuf::from("D:\\Trabajo"));

    let fallback = resolve_home_dir("");
    assert!(!fallback.as_os_str().is_empty(), "vacío debe resolver a una ruta");
}
```

- [ ] **Step 2: Correr el test (falla)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core home_vacio`
Expected: FAIL — `resolve_home_dir` no existe.

- [ ] **Step 3: Implementar `resolve_home_dir`**

En `crates/core/src/config/mod.rs` (función pública, junto a las helpers):

```rust
/// Resuelve la carpeta Home: si `home_dir` está vacío, usa la carpeta personal del usuario
/// (`%USERPROFILE%`); si tiene una ruta, usa esa. Pura, testeable.
pub fn resolve_home_dir(home_dir: &str) -> PathBuf {
    if !home_dir.trim().is_empty() {
        return PathBuf::from(home_dir);
    }
    // Carpeta personal del usuario. dirs::home_dir si está; si no, USERPROFILE; último
    // recurso: la raíz C:\ (nunca vacío, nunca panic).
    dirs::home_dir()
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("C:\\"))
}
```

> Si `dirs` NO está en naygo-core, quitar la primera rama y dejar solo `USERPROFILE` + fallback.

- [ ] **Step 4: Correr el test (pasa)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core home_vacio`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/config/mod.rs
git commit -m "feat(core): resolve_home_dir (vacío = perfil del usuario)"
```

---

### Task 4: `code_lang_for` gana `auto_highlight`

**Files:**
- Modify: `crates/core/src/preview.rs:214`

- [ ] **Step 1: Escribir/ajustar tests (fallan al cambiar la firma)**

En el `#[cfg(test)]` de `preview.rs`, agregar:

```rust
#[test]
fn auto_highlight_on_resalta_extension_conocida() {
    let rules = default_preview_rules(); // todas Auto
    let p = std::path::Path::new("main.rs");
    // ON: una extensión de código conocida en modo Auto se resalta sola.
    assert_eq!(code_lang_for(p, &rules, true), Some(CodeLang::Rust));
    // OFF: en Auto no se resalta.
    assert_eq!(code_lang_for(p, &rules, false), None);
}

#[test]
fn regla_forzada_manda_sobre_el_global() {
    // Una regla que fuerza Code(Json) se resalta aunque auto_highlight esté OFF.
    let rules = vec![PreviewRule { ext: "txt".into(), enabled: true, view: ViewMode::Code(CodeLang::Json) }];
    let p = std::path::Path::new("notas.txt");
    assert_eq!(code_lang_for(p, &rules, false), Some(CodeLang::Json));
}
```

- [ ] **Step 2: Correr (fallan: firma vieja)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core preview::`
Expected: FAIL de compilación (la firma tiene 2 args, los tests pasan 3).

- [ ] **Step 3: Cambiar la firma e implementar**

Reemplazar `code_lang_for` (preview.rs:214) por:

```rust
/// Lenguaje de código para el resaltado de `path`. Si una regla fuerza `Code(l)`, gana ese
/// (manda sobre el global). Si la regla es `Auto` (o no hay regla habilitada que fuerce) y
/// `auto_highlight` está activo, intenta deducir el lenguaje por la extensión conocida.
pub fn code_lang_for(
    path: &std::path::Path,
    rules: &[PreviewRule],
    auto_highlight: bool,
) -> Option<CodeLang> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext.is_empty() {
        return None;
    }
    let rule = rules.iter().find(|r| r.ext == ext && r.enabled);
    // 1) Regla que fuerza un lenguaje: manda siempre.
    if let Some(PreviewRule { view: ViewMode::Code(l), .. }) = rule {
        return Some(*l);
    }
    // 2) Global ON + (regla Auto o sin regla específica de código): deducir por extensión.
    //    Solo si la regla NO fuerza Text/Image (esos no resaltan).
    let forces_non_code = matches!(
        rule,
        Some(PreviewRule { view: ViewMode::Text, .. })
            | Some(PreviewRule { view: ViewMode::Image, .. })
    );
    if auto_highlight && !forces_non_code {
        return CodeLang::from_str(&ext);
    }
    None
}
```

- [ ] **Step 4: Arreglar los llamadores existentes**

Buscar usos viejos: `cargo build -p naygo-ui-slint` mostrará dónde se llama `code_lang_for` con 2 args. Actualizar cada uno para pasar `settings.auto_highlight_code` (tras Task 5 ese campo existe; si se hace antes, pasar `true` temporalmente y ajustar en Task 8).

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core preview::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/preview.rs
git commit -m "feat(core): code_lang_for con auto_highlight (regla forzada manda sobre el global)"
```

---

# FASE 2 — Settings nuevos + keymap

### Task 5: Settings (footer + auto_highlight + home_dir)

**Files:**
- Modify: `crates/core/src/config/mod.rs:119` (struct + defaults + `Default for Settings`)

- [ ] **Step 1: Test de round-trip + migración (falla)**

En el `#[cfg(test)]` de `config/mod.rs`:

```rust
#[test]
fn settings_nuevos_tienen_defaults() {
    let s = Settings::default();
    assert!(s.footer_enabled);
    assert_eq!(s.footer_preset, crate::footer::FooterPreset::Compact);
    assert!(s.footer_custom_template.is_empty());
    assert!(s.auto_highlight_code);
    assert!(s.home_dir.is_empty());
}

#[test]
fn settings_viejo_sin_campos_nuevos_migra_a_defaults() {
    // Un JSON v1 mínimo SIN los campos nuevos debe cargar con defaults (serde default).
    let json = r#"{"version":1,"bar_position":"Top","icon_only":false}"#;
    let s: Settings = serde_json::from_str(json).expect("debe migrar");
    assert!(s.footer_enabled);
    assert!(s.auto_highlight_code);
    assert!(s.home_dir.is_empty());
}
```

- [ ] **Step 2: Correr (falla)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core settings_nuevos settings_viejo`
Expected: FAIL — los campos no existen.

- [ ] **Step 3: Agregar los campos al struct**

En `struct Settings` (tras `recent_limit`, antes del `}` de cierre en :244):

```rust
    /// Mostrar el footer (barra inferior) en cada panel de archivos. `#[serde(default)]`
    /// retro-compat (settings viejo → true).
    #[serde(default = "default_footer_enabled")]
    pub footer_enabled: bool,
    /// Plantilla del footer (global a todos los paneles). `#[serde(default)]` retro-compat.
    #[serde(default)]
    pub footer_preset: crate::footer::FooterPreset,
    /// Template personalizado del footer (cuando `footer_preset == Custom`). `#[serde(default)]`.
    #[serde(default)]
    pub footer_custom_template: String,
    /// Resaltar automáticamente el código de extensiones conocidas en modo Auto del preview.
    /// `#[serde(default)]` retro-compat (settings viejo → true).
    #[serde(default = "default_auto_highlight_code")]
    pub auto_highlight_code: bool,
    /// Carpeta de inicio (botón Home). Vacío = carpeta personal del usuario. `#[serde(default)]`.
    #[serde(default)]
    pub home_dir: String,
```

- [ ] **Step 4: Agregar las fn default**

Junto a las otras `fn default_*` de config/mod.rs:

```rust
/// Default de `footer_enabled`: true (mostrar el footer).
fn default_footer_enabled() -> bool {
    true
}

/// Default de `auto_highlight_code`: true (resaltar código en Auto).
fn default_auto_highlight_code() -> bool {
    true
}
```

- [ ] **Step 5: Actualizar `Default for Settings`**

Buscar el `impl Default for Settings` (es donde se construye la instancia default). Agregar a la
construcción los cinco campos nuevos:

```rust
            footer_enabled: true,
            footer_preset: crate::footer::FooterPreset::Compact,
            footer_custom_template: String::new(),
            auto_highlight_code: true,
            home_dir: String::new(),
```

- [ ] **Step 6: Correr los tests (pasan)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core settings_nuevos settings_viejo`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/config/mod.rs
git commit -m "feat(core): Settings footer_* + auto_highlight_code + home_dir (migrables)"
```

---

### Task 6: `Action::GoHome` + binding Alt+Home

**Files:**
- Modify: `crates/core/src/keymap.rs:102` (enum) + donde se definen los bindings default + `Action::all()`/labels si existen.

- [ ] **Step 1: Test del binding default (falla)**

En el `#[cfg(test)]` de `keymap.rs`:

```rust
#[test]
fn alt_home_dispara_go_home() {
    let km = KeyMap::default();
    let chord = Chord::alt(KeyCode::Home);
    assert_eq!(km.action_for(&chord), Some(Action::GoHome));
}
```

- [ ] **Step 2: Correr (falla)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core alt_home`
Expected: FAIL — `Action::GoHome` no existe.

- [ ] **Step 3: Agregar la variante al enum**

En `enum Action` (tras `GoForward`, :110):

```rust
    /// Ir a la carpeta de inicio configurada (Home). Alt+Home.
    GoHome,
```

- [ ] **Step 4: Agregar el binding default**

Buscar la función que construye el `KeyMap::default()` (la que hace los `bind`/inserta los chords
default — buscar `GoForward` para ubicar el bloque de navegación). Agregar junto a GoBack/GoForward:

```rust
        map.bind(Chord::alt(KeyCode::Home), Action::GoHome);
```

> Si el editor de atajos lista acciones por nombre (labels/`Action::all()`), agregar `GoHome` ahí
> también para que aparezca en el editor. Buscar un `match` sobre `Action` que devuelva strings.

- [ ] **Step 5: Correr (pasa)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core alt_home`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/keymap.rs
git commit -m "feat(core): Action::GoHome con binding default Alt+Home"
```

---

# FASE 3 — Navegación (controlador + toolbar)

### Task 7: `can_go_back`/`can_go_forward` en NavHistory + controlador

**Files:**
- Modify: `crates/core/src/workspace/nav_history.rs` (exponer si hay anterior/siguiente)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (`on_go_home`, getters can_*)

- [ ] **Step 1: Test de can_back/can_forward en NavHistory (falla)**

En `#[cfg(test)]` de `nav_history.rs`:

```rust
#[test]
fn can_back_forward_reflejan_el_cursor() {
    let mut h = NavHistory::new(std::path::PathBuf::from("A"));
    assert!(!h.can_back());
    assert!(!h.can_forward());
    h.push(std::path::PathBuf::from("B"));
    assert!(h.can_back());
    assert!(!h.can_forward());
    h.back();
    assert!(!h.can_back());
    assert!(h.can_forward());
}
```

> Ajustar el constructor/método `push` a la firma REAL de NavHistory (ver el módulo; `new` toma
> el dir inicial, `push` agrega, `back`/`forward` mueven el cursor). Si `new` no toma argumento,
> adaptar el test.

- [ ] **Step 2: Correr (falla)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core can_back_forward`
Expected: FAIL — métodos no existen.

- [ ] **Step 3: Implementar can_back/can_forward**

En `nav_history.rs`, junto a `back`/`forward`:

```rust
    /// `true` si hay una entrada anterior a la actual (se puede ir Atrás).
    pub fn can_back(&self) -> bool {
        self.cursor > 0
    }

    /// `true` si hay una entrada posterior a la actual (se puede ir Adelante).
    pub fn can_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }
```

> Ajustar `self.cursor`/`self.entries` a los nombres REALES de los campos del struct (ver arriba
> en el módulo). El concepto: cursor>0 → hay atrás; cursor no es el último → hay adelante.

- [ ] **Step 4: Correr (pasa)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core can_back_forward`
Expected: PASS.

- [ ] **Step 5: Agregar `on_go_home` y getters al controlador**

En `crates/ui-slint/src/workspace_ctrl.rs`, junto a `on_go_back`/`on_go_forward` (:3440):

```rust
    /// Navega el panel Files activo a la carpeta Home configurada (vacío = perfil del usuario).
    pub fn on_go_home(&mut self) -> bool {
        let home = naygo_core::config::resolve_home_dir(&self.settings.home_dir);
        self.navigate_active_to(&home)
    }

    /// ¿El panel activo puede ir Atrás? (para habilitar/deshabilitar el botón del toolbar).
    pub fn can_go_back(&self) -> bool {
        self.ws
            .active_files()
            .map(|f| f.can_go_back())
            .unwrap_or(false)
    }

    /// ¿El panel activo puede ir Adelante?
    pub fn can_go_forward(&self) -> bool {
        self.ws
            .active_files()
            .map(|f| f.can_go_forward())
            .unwrap_or(false)
    }
```

> `navigate_active_to` / `f.can_go_back()`: ajustar a los métodos REALES. `on_go_back` ya hace
> `.and_then(|f| f.go_back())` sobre el panel activo (ver :3448), así que el FilePaneState ya
> tiene su NavHistory; exponer `can_go_back`/`can_go_forward` en FilePaneState delegando a su
> historial. Para navegar a Home, reusar el mismo camino que usa "ir a una ruta" (buscar cómo
> `on_go_up` o la path-bar navegan a un PathBuf y llamar a eso; debe registrar en el historial).

- [ ] **Step 6: Rutear `Action::GoHome` en el dispatcher**

En el `match action` de `on_key` (workspace_ctrl.rs:3953, junto a GoBack/GoForward):

```rust
            Action::GoHome => return self.on_go_home(),
```

- [ ] **Step 7: Gate completo (core + ui compilan)**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings
```
Expected: PASS, clippy limpio.

- [ ] **Step 8: Commit**

```bash
git add crates/core/src/workspace/nav_history.rs crates/ui-slint/src/workspace_ctrl.rs
git commit -m "feat: on_go_home + can_go_back/forward + ruteo Action::GoHome"
```

---

### Task 8: Botones Atrás/Adelante/Home en el toolbar (Slint)

**Files:**
- Modify: `crates/ui-slint/ui/app-window.slint` (componente ToolBtn nuevos + callbacks + props can_*)
- Modify: `crates/ui-slint/src/main.rs` (cablear callbacks + alimentar can_* y refrescarlos)

- [ ] **Step 1: Agregar íconos Path al ToolBtn (chevrones + casa)**

En `app-window.slint`, el componente `ToolBtn` (:75) ya tiene flags `draw-terminal`/`draw-refresh`/
etc. Agregar tres flags nuevos `draw-back`/`draw-forward`/`draw-home` con su Path (chevron izq,
chevron der, casita), siguiendo el patrón de los Path existentes (ver `draw-refresh` en :189 como
molde). Ejemplo del chevron Atrás:

```slint
    in property <bool> draw-back: false;
    // ... dentro del cuerpo, junto a los otros if draw-*:
    if root.draw-back: Path {
        width: 16px; height: 16px;
        x: (parent.width - self.width) / 2; y: (parent.height - self.height) / 2;
        stroke: root.tint; stroke-width: 1.6px;
        viewbox-width: 16; viewbox-height: 16;
        MoveTo { x: 10; y: 3; } LineTo { x: 5; y: 8; } LineTo { x: 10; y: 13; }
    }
```

(Forward = espejo: `MoveTo{x:6;y:3} LineTo{x:11;y:8} LineTo{x:6;y:13}`. Home = casita: triángulo
techo + cuadrado base.)

Y agregar a la condición del Text (:260) `&& !root.draw-back && !root.draw-forward && !root.draw-home`
para que no dibuje texto cuando es ícono.

- [ ] **Step 2: Declarar callbacks y props en AppWindow**

En `app-window.slint`, junto a `go-up` (:437):

```slint
    callback go-back();
    callback go-forward();
    callback go-home();
    in property <bool> can-go-back: false;
    in property <bool> can-go-forward: false;
```

- [ ] **Step 3: Instanciar los botones al inicio del toolbar**

En la fila del toolbar, ANTES del botón "Subir nivel" existente (buscar dónde se instancia el
ToolBtn de go-up), agregar:

```slint
        ToolBtn {
            draw-back: true; icon-only: true;
            enabled: root.can-go-back;
            tooltip: Tr.nav-back-tip;
            clicked => { root.go-back(); }
            hover-tip(t, x) => { root.hover-tip(t, x); }
        }
        ToolBtn {
            draw-forward: true; icon-only: true;
            enabled: root.can-go-forward;
            tooltip: Tr.nav-forward-tip;
            clicked => { root.go-forward(); }
            hover-tip(t, x) => { root.hover-tip(t, x); }
        }
        ToolBtn {
            draw-home: true; icon-only: true;
            tooltip: Tr.nav-home-tip;
            clicked => { root.go-home(); }
            hover-tip(t, x) => { root.hover-tip(t, x); }
        }
```

> `enabled`: si `ToolBtn` no tiene aún una prop `enabled` que atenúe y bloquee el clic, agregarla
> (in property <bool> enabled: true; el TouchArea usa `enabled: root.enabled;` y el tint baja a
> `Theme.text-dim` cuando `!enabled`). Ver cómo otros botones manejan disabled si ya existe.

- [ ] **Step 4: Cablear en main.rs**

Donde se cablean los callbacks del toolbar (buscar `on_go_up`), agregar:

```rust
    {
        let ctrl = ctrl.clone();
        let ui_weak = ui.as_weak();
        ui.on_go_back(move || {
            ctrl.borrow_mut().on_go_back();
            if let Some(ui) = ui_weak.upgrade() { refresh_all(&ui, &ctrl); }
        });
    }
    // idéntico para on_go_forward (llama on_go_forward) y on_go_home (llama on_go_home)
```

> Usar el patrón de refresco que ya use el proyecto tras navegar (la función que repinta filas +
> path-bar; buscar cómo on_go_up refresca). Tras cada navegación, actualizar `can-go-back`/
> `can-go-forward` de la AppWindow:

```rust
    ui.set_can_go_back(ctrl.borrow().can_go_back());
    ui.set_can_go_forward(ctrl.borrow().can_go_forward());
```

Estos dos `set_` deben llamarse también en el refresco general (donde se repintan los paneles), para
que los botones se habiliten/deshabiliten al navegar por cualquier vía (teclado, mouse, doble clic).

- [ ] **Step 5: i18n — claves nuevas**

En `crates/ui-slint/lang/es.json` y `en.json` (y el struct i18n de core si las claves se declaran
ahí), agregar (NO reutilizar nombres existentes):

```json
"nav-back-tip": "Atrás (Alt+←)",
"nav-forward-tip": "Adelante (Alt+→)",
"nav-home-tip": "Inicio (Alt+Inicio)"
```

en (es): "Atrás (Alt+←)" / "Adelante (Alt+→)" / "Inicio (Alt+Inicio)".
en (en): "Back (Alt+←)" / "Forward (Alt+→)" / "Home (Alt+Home)".

- [ ] **Step 6: Gate + build**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint; cargo clippy --workspace --all-targets -- -D warnings
```
Expected: compila, clippy limpio. (Verificación visual en la VM al final.)

- [ ] **Step 7: Commit**

```bash
git add crates/ui-slint/ui/app-window.slint crates/ui-slint/src/main.rs crates/ui-slint/lang/es.json crates/ui-slint/lang/en.json
git commit -m "feat(ui): botones Atrás/Adelante/Home en el toolbar"
```

---

# FASE 4 — Footer en la UI

### Task 9: `footer_text` en PaneVm + cálculo en sync_rows (con caché de disco)

**Files:**
- Modify: `crates/ui-slint/ui/types.slint` (campo `footer_text` en PaneVm)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (caché de disco por panel + armado en sync_rows)

- [ ] **Step 1: Agregar el campo al PaneVm**

En `types.slint`, en `struct PaneVm` (o como se llame la struct del panel), agregar:

```slint
    footer-text: string,
```

- [ ] **Step 2: Caché de disco por panel en el controlador**

En `workspace_ctrl.rs`, agregar un campo a la struct del controlador para cachear el `DiskUsage`
por raíz de unidad con timestamp lógico (refresco al cambiar de carpeta o cada N refrescos):

```rust
    /// Caché de espacio en disco por raíz de unidad (para el footer; evita pegarle a WinAPI
    /// en cada tick). Se refresca al cambiar la carpeta del panel.
    footer_disk_cache: std::collections::HashMap<PathBuf, naygo_core::disk::DiskUsage>,
```

Inicializarlo en el constructor (`HashMap::new()`).

- [ ] **Step 3: Helper que arma el FooterData de un panel**

En `workspace_ctrl.rs`:

```rust
    /// Arma el `FooterData` del panel Files dado, usando la caché de disco.
    fn footer_data_for(&mut self, files: &FilePaneState) -> naygo_core::footer::FooterData {
        let sel_count = files.selected.len();
        let total_count = files.view_indices().len();
        let marked_bytes: u64 = files.selected_entries().map(|e| e.size).sum();
        let (file_count, dir_count) = files.count_files_dirs(); // archivos/carpetas del listado
        let item_count = file_count + dir_count;
        // Disco: caché por la raíz de la unidad de la carpeta actual.
        let root = drive_root_of(&files.current_dir);
        let disk = self
            .footer_disk_cache
            .entry(root.clone())
            .or_insert_with(|| disk_usage(&root).unwrap_or(naygo_core::disk::DiskUsage { total: 0, free: 0 }))
            .clone()
            .into();
        let disk = if matches!(&disk, Some(d) if d.total == 0) { None } else { disk };
        naygo_core::footer::FooterData {
            sel_count, total_count, marked_bytes, disk, item_count, file_count, dir_count,
        }
    }
```

> Ajustar `files.selected_entries()`, `.size`, `.count_files_dirs()`, `drive_root_of` a lo REAL.
> Si no existen, implementarlos: `selected_entries` itera `selected` mapeando a los FsEntry;
> `count_files_dirs` cuenta sobre las filas del listado; `drive_root_of(p)` devuelve la raíz de
> unidad (p.ej. `C:\`) — en Windows, los primeros componentes (prefix + RootDir). `disk_usage`
> ya existe (:4208).

- [ ] **Step 4: Invalidar la caché de disco al cambiar de carpeta**

En el método que cambia la carpeta de un panel (navigate/enter/go_up/go_home), tras cambiar
`current_dir`, quitar la entrada de la raíz vieja del cache O simplemente limpiar todo el cache
de disco (es chico): `self.footer_disk_cache.clear();`. Así el footer relee el espacio al navegar.

- [ ] **Step 5: Setear `footer_text` en sync_rows**

En `sync_rows` (donde ya se actualizan los campos por tick del PaneVm), para cada panel Files,
calcular y asignar:

```rust
        let footer_text = if self.settings.footer_enabled {
            let data = self.footer_data_for(files);
            naygo_core::footer::render(&self.settings.footer_preset, &data, self.settings.size_format)
        } else {
            String::new()
        };
        pane_vm.footer_text = footer_text.into();
```

> Cuidar el borrow: `footer_data_for` toma `&mut self`; si `sync_rows` ya tiene `&self`, extraer
> los datos antes o reestructurar (calcular `FooterData` con un helper que tome solo lo necesario y
> el cache por parámetro). Mantener el patrón existente de sync_rows.

- [ ] **Step 6: Gate**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio.

- [ ] **Step 7: Commit**

```bash
git add crates/ui-slint/ui/types.slint crates/ui-slint/src/workspace_ctrl.rs
git commit -m "feat(ui): footer_text por panel en sync_rows con caché de disco"
```

---

### Task 10: Barra del footer en file-panel.slint

**Files:**
- Modify: `crates/ui-slint/ui/file-panel.slint`

- [ ] **Step 1: Agregar la barra al pie del panel**

Al final del `VerticalLayout` del panel (después de la tabla, antes de cerrar el layout), agregar:

```slint
    if root.pane.footer-text != "": Rectangle {
        height: 22px;
        background: Theme.row-alt-bg;
        // Borde superior fino.
        Rectangle { y: 0; height: 1px; width: parent.width; background: Theme.border; }
        Text {
            x: 8px;
            width: parent.width - 16px;
            height: parent.height;
            vertical-alignment: center;
            text: root.pane.footer-text;
            color: Theme.text-dim;
            font-size: 12px;
            overflow: elide;
        }
    }
```

> `root.pane.footer-text`: ajustar al path real del PaneVm dentro de file-panel.slint (cómo se
> accede a los otros campos del pane, p.ej. `root.pane.current-path`). Si el panel recibe el VM con
> otro nombre, usar ese.

- [ ] **Step 2: Build (verificación de que compila el .slint)**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint`
Expected: compila.

- [ ] **Step 3: Commit**

```bash
git add crates/ui-slint/ui/file-panel.slint
git commit -m "feat(ui): barra de footer al pie del file panel"
```

---

# FASE 5 — Toggle auto-resaltado + selección en preview (config + UI)

### Task 11: Pasar `auto_highlight_code` al worker de preview

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` y/o `crates/ui-slint/src/preview.rs` (donde se llama `code_lang_for`)

- [ ] **Step 1: Actualizar las llamadas a code_lang_for**

Buscar las llamadas a `code_lang_for(path, rules)` en el código de UI (preview worker / build_payload)
y pasarles el tercer argumento desde Settings:

```rust
    let lang = naygo_core::preview::code_lang_for(path, &rules, settings.auto_highlight_code);
```

> El worker de preview corre en otro hilo: asegurarse de pasarle el bool por el mensaje/clon de
> settings que ya recibe (mismo canal por el que recibe `rules`). Si hoy recibe solo `rules`,
> agregar el bool al payload del job de preview.

- [ ] **Step 2: Gate**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio.

- [ ] **Step 3: Commit**

```bash
git add crates/ui-slint/src
git commit -m "feat(ui): el worker de preview respeta auto_highlight_code"
```

---

### Task 12: Selección+copia en el preview (TextEdit read-only + toggle)

**Files:**
- Modify: `crates/ui-slint/ui/preview-panel.slint`

- [ ] **Step 1: Estado local del toggle**

En `PreviewPanel`, agregar una propiedad local:

```slint
    property <bool> selectable: false;
```

- [ ] **Step 2: Botón "seleccionar texto" en la barra (solo si highlighted)**

En la `HorizontalLayout` de la barra de título del preview (junto al botón "abrir con el sistema",
:84), agregar un botón visible solo cuando hay resaltado:

```slint
            if root.view.highlighted: Rectangle {
                width: 26px; height: 20px;
                background: sel-touch.has-hover ? Theme.selection-bg : transparent;
                border-radius: 3px;
                // Ícono simple "T" o cursor de texto con Path (reusar estilo OpenExternalIcon).
                Text { text: "T"; color: root.selectable ? Theme.accent : Theme.text-dim;
                       font-weight: 700; horizontal-alignment: center; vertical-alignment: center;
                       width: 100%; height: 100%; }
                sel-touch := TouchArea {
                    mouse-cursor: pointer;
                    clicked => { root.selectable = !root.selectable; }
                    changed has-hover => {
                        root.hover-tip(self.has-hover ? Tr.preview-select-tip : "",
                            self.absolute-position.x + self.width / 2);
                    }
                }
            }
```

- [ ] **Step 3: Render condicional del cuerpo de texto (modo 1)**

Reemplazar el bloque del modo texto (mode==1) para que respete `selectable`:

```slint
        if root.view.mode == 1: VerticalLayout {
            vertical-stretch: 1;
            spacing: 2px;
            // Coloreado (no seleccionable) — solo cuando hay resaltado Y el toggle está OFF.
            if root.view.highlighted && !root.selectable: ScrollView {
                vertical-stretch: 1;
                // ... (el render coloreado actual, sin cambios)
            }
            // Seleccionable: TextEdit read-only (texto plano, una tinta, copia nativa).
            // Se usa cuando NO hay resaltado (siempre) o cuando el toggle está ON.
            if !root.view.highlighted || root.selectable: TextEdit {
                vertical-stretch: 1;
                read-only: true;
                wrap: word-wrap;
                text: root.view.text;
                font-family: "Consolas";
            }
            if root.view.truncated: Text {
                text: Tr.preview-truncated; color: Theme.text-dim; font-size: 11px;
            }
        }
```

> Importar `TextEdit` desde `std-widgets.slint` al tope de preview-panel.slint si no está.
> El `Text` plano actual (el `if !root.view.highlighted: Text {...}`) se reemplaza por el TextEdit
> read-only de arriba (texto plano siempre seleccionable, como se decidió).

- [ ] **Step 4: i18n del tooltip del botón**

`es.json`: `"preview-select-tip": "Seleccionar/copiar texto"`. `en.json`:
`"preview-select-tip": "Select/copy text"`. (Clave nueva, no reutilizar.)

- [ ] **Step 5: Build**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint`
Expected: compila.

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/ui/preview-panel.slint crates/ui-slint/lang/es.json crates/ui-slint/lang/en.json
git commit -m "feat(ui): preview de texto seleccionable (TextEdit read-only + toggle)"
```

---

### Task 13: Sección de config — Avanzado (footer + Home) y Previsualización (toggle)

**Files:**
- Modify: `crates/ui-slint/ui/config-window.slint`
- Modify: `crates/ui-slint/src/config_ctrl.rs` (getters/setters)
- Modify: `crates/ui-slint/src/main.rs` (cablear callbacks de config)

- [ ] **Step 1: Toggle auto-resaltado en Previsualización**

En `config-window.slint`, sección Previsualización, ARRIBA de la tabla de reglas, agregar una fila
con un switch (reusar el patrón de switches/checkboxes existentes en config):

```slint
        HorizontalLayout {
            spacing: 8px;
            Text { text: Tr.cfg-auto-highlight; color: Theme.text; vertical-alignment: center; }
            // Switch existente del proyecto, enlazado a una prop in-out `auto-highlight-code`.
            ConfigSwitch { checked: root.auto-highlight-code;
                           toggled(v) => { root.set-auto-highlight-code(v); } }
        }
```

- [ ] **Step 2: Sección "Pie de panel" + "Carpeta de inicio" en Avanzado**

En la sección Avanzado, agregar:

```slint
        Text { text: Tr.cfg-footer-section; color: Theme.text; font-weight: 700; }
        ConfigSwitch { checked: root.footer-enabled;
                       toggled(v) => { root.set-footer-enabled(v); } }
        // Combo de plantillas (usar el ThemeCombo propio; el ComboBox de std es invisible en claro).
        ThemeCombo {
            model: root.footer-preset-names;   // ["Compacta","Completa","Solo disco","Solo selección","Personalizada…"]
            current-index: root.footer-preset-index;
            selected(i) => { root.set-footer-preset-index(i); }
        }
        // Editor de plantilla + tokens + preview, solo si es "Personalizada…".
        if root.footer-preset-index == 4: VerticalLayout {
            LineEdit { text: root.footer-custom-template;
                       edited(s) => { root.set-footer-custom-template(s); } }
            Text { text: Tr.cfg-footer-tokens; color: Theme.text-dim; font-size: 11px; }
            Text { text: root.footer-preview; color: Theme.text-dim; font-family: "Consolas"; }
        }

        Text { text: Tr.cfg-home-section; color: Theme.text; font-weight: 700; }
        HorizontalLayout {
            LineEdit { text: root.home-dir; placeholder-text: Tr.cfg-home-placeholder;
                       edited(s) => { root.set-home-dir(s); } }
            Button { text: Tr.cfg-browse; clicked => { root.browse-home(); } }
        }
        Text { text: Tr.cfg-home-note; color: Theme.text-dim; font-size: 11px; }
```

> Usar los componentes REALES del proyecto (ConfigSwitch/ThemeCombo/LineEdit/Button como ya se usen
> en config-window.slint; ver cómo está hecha otra fila de Avanzado y replicar el estilo). Declarar
> las props in-out y callbacks en el `export component` de la ventana de config.

- [ ] **Step 3: Getters/setters en config_ctrl.rs**

Agregar métodos que lean/escriban los Settings nuevos y persistan (siguiendo el patrón de los demás
campos de Avanzado, p.ej. cómo se setea `low_power_mode`/`recent_limit`):

```rust
    pub fn auto_highlight_code(&self) -> bool { self.settings.auto_highlight_code }
    pub fn set_auto_highlight_code(&mut self, v: bool) { self.settings.auto_highlight_code = v; self.save(); }
    pub fn footer_enabled(&self) -> bool { self.settings.footer_enabled }
    pub fn set_footer_enabled(&mut self, v: bool) { self.settings.footer_enabled = v; self.save(); }
    // footer_preset por índice (0..=4), footer_custom_template, home_dir → ídem.
```

> `self.save()`: usar el método REAL de persistencia (buscar cómo otros setters guardan). El índice
> 0..=3 mapea a los 4 presets fijos; 4 = Custom (toma `footer_custom_template`).

- [ ] **Step 4: Preview en vivo + browse + cablear en main.rs**

- `footer-preview`: en config_ctrl, un getter que renderiza con un `FooterData` de muestra fijo:
  `naygo_core::footer::render(&preset, &SAMPLE, SizeFormat::Auto)`. Recalcular cuando cambia el
  template (en el setter, actualizar la prop del UI).
- `browse-home`: callback en main.rs que abre el FileDialog nativo (reusar el que ya se usa para
  elegir carpetas, p.ej. en import/export o "elegir otra") y mete la ruta en `home_dir`.
- Cablear todos los `set_*`/getters como los callbacks de config existentes en main.rs.

- [ ] **Step 5: i18n — claves de config (nuevas)**

`es.json` / `en.json`:
```
cfg-auto-highlight   : "Resaltar código automáticamente" / "Highlight code automatically"
cfg-footer-section   : "Pie de panel" / "Panel footer"
cfg-footer-tokens    : "Tokens: {sel} {total} {marked} {free} {disk_total} {pct} {items} {files} {dirs}"
cfg-home-section     : "Carpeta de inicio (Home)" / "Home folder"
cfg-home-placeholder : "Vacío = carpeta personal del usuario" / "Empty = your user folder"
cfg-home-note        : "Vacío = carpeta personal del usuario." / "Empty = your user folder."
cfg-browse           : "Examinar…" / "Browse…"
```
Nombres de presets para el combo (claves nuevas, p.ej. `footer-preset-compact` etc.) o construir el
modelo en Rust con strings traducidos. NO reutilizar claves.

- [ ] **Step 6: Gate completo**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo fmt; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio.

- [ ] **Step 7: Commit**

```bash
git add crates/ui-slint/ui/config-window.slint crates/ui-slint/src/config_ctrl.rs crates/ui-slint/src/main.rs crates/ui-slint/lang/es.json crates/ui-slint/lang/en.json
git commit -m "feat(ui): config — footer (Avanzado) + Home + toggle auto-resaltado"
```

---

# FASE 6 — Cierre

### Task 14: CHANGELOG + docs + gate final + dist

**Files:**
- Modify: `CHANGELOG.md`, `docs/` (guía de usuario: footer, navegación, auto-resaltado, copiar preview)

- [ ] **Step 1: CHANGELOG**

En `CHANGELOG.md`, bajo la sección en curso, "Añadido":
```
- Pie de panel (footer) configurable: cada panel muestra seleccionados, bytes marcados y espacio
  del disco de su unidad. Plantilla global con presets o personalizada con tokens (Avanzado).
- Botones Atrás / Adelante / Inicio en la barra de herramientas (Inicio = Alt+Inicio; carpeta
  configurable, por defecto la carpeta personal del usuario).
- Resaltado automático de código en la vista previa (extensiones conocidas), con interruptor en
  Configuración → Previsualización.
- La vista previa de texto permite seleccionar y copiar (botón para alternar a modo seleccionable).
```

- [ ] **Step 2: Docs de usuario**

Actualizar la guía de usuario (buscar el .md de documentación de secciones) con los 4 puntos.

- [ ] **Step 3: Gate final completo**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings
```
Expected: PASS (todos los tests), clippy 100% limpio.

- [ ] **Step 4: Actualizar el grafo**

Run: `graphify update .`

- [ ] **Step 5: Commit docs**

```bash
git add CHANGELOG.md docs
git commit -m "docs: footer + navegación + auto-resaltado + copiar preview"
```

- [ ] **Step 6: Generar el dist**

Run: `$env:CARGO_BUILD_JOBS = "2"; powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1`
Expected: `dist/Naygo-0.1.0-portable.zip` + `dist/Naygo-0.1.0-setup.exe` regenerados.

(El push lo hace Nicolás. Verificación visual en la VM: footer en ambos paneles con datos propios,
botones nav habilitándose/deshabilitándose, Home navega al perfil, Alt+Home funciona, toggle
auto-resaltado on/off, botón de seleccionar texto en el preview copia con Ctrl+C.)

---

## Notas para el implementador

- **SIEMPRE correr el gate uno mismo tras cada subagente** (no confiar en su reporte): el subagente
  de la Entrega B no corrió clippy y dejó 2 errores. `cargo clippy --workspace --all-targets -D warnings`.
- **Ajustar firmas a lo REAL**: los nombres de campos/métodos marcados con "ajustar a lo real"
  (FilePaneState.selected, NavHistory.cursor/entries, navigate_active_to, ConfigSwitch/ThemeCombo)
  deben verificarse en el código antes de escribir; el plan da el concepto y el patrón, no inventa.
- **graphify antes de grep** (hook obligatorio) e incluirlo en prompts de subagentes.
- **Slint 1.16**: `HorizontalLayout` NO soporta `wrap`; `TextEdit` da selección nativa pero una
  sola tinta; las `function` sueltas en layouts van envueltas en `Rectangle{}`.
- **i18n triple, sin voseo, sin reutilizar claves.**
- Un solo dist al final (Task 14). Nicolás hace el push.
