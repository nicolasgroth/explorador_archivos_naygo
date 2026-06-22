# Entrega B: modo de vista + resaltado + abrir en sistema — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Previsualización con modo de vista configurable por extensión (Auto/Texto/Imagen/Código+lenguaje), resaltado de sintaxis con syntect, y un botón para abrir el archivo con el programa del sistema.

**Architecture:** El modelo `ViewMode`/`CodeLang` vive en `core::preview`. El resaltado es una función pura en `core::highlight` (syntect) que corre en el worker de preview y produce líneas→segmentos coloreados; la UI los pinta como Text por segmento. La config gana dos combobox. El botón reusa `naygo_platform::open::open_default`.

**Tech Stack:** Rust workspace (naygo-core / naygo-ui-slint / naygo-platform), Slint 1.16 (render software), syntect. Build con `CARGO_BUILD_JOBS=2`. PowerShell 5.1 (sin `&&`).

> **Defensa i16 (NO romper):** el render por software de Slint revienta con glifos > ~32767px. El
> resaltado SIEMPRE opera sobre el texto YA recortado por `core::preview` (`TEXT_MAX_LINES=100`,
> `TEXT_MAX_LINE_CHARS=1000` / `clip_long_lines`). Nunca resaltar texto sin recortar.

> **Contexto verificado:** `naygo_platform::open::open_default(path: &Path) -> Result<(), ShellError>`
> ya existe y se usa (workspace_ctrl.rs:2878). El modelo actual es
> `PreviewRule { ext: String, enabled: bool, treat_as: Option<String> }` en `crates/core/src/preview.rs`.
> i18n triple: `ui/i18n.slint` (`in property <string> kebab`) + `core/src/i18n/{es,en}.json`
> (`"slint.xxx.yyy"`) + `ui-slint/src/i18n_keys.rs` (`tr.set_xxx(c.t("slint.xxx.yyy").into())`).
> NUNCA reusar un nombre de clave i18n existente.

---

### Task 1: Modelo `CodeLang` + `ViewMode` en core (sin tocar PreviewRule todavía)

**Files:**
- Modify: `crates/core/src/preview.rs`

- [ ] **Step 1: Test de `CodeLang` (as_str/from_str/all round-trip)**

Agregar al `mod tests` de `preview.rs`:
```rust
#[test]
fn codelang_round_trip_y_all() {
    for l in CodeLang::all() {
        assert_eq!(CodeLang::from_str(l.as_str()), Some(l), "round-trip de {}", l.as_str());
    }
    assert_eq!(CodeLang::from_str("noexiste"), None);
    assert!(CodeLang::all().contains(&CodeLang::Xml));
    assert!(CodeLang::all().contains(&CodeLang::Json));
}
```

- [ ] **Step 2: Run — falla (no existe CodeLang)**

Run: `cargo test -p naygo-core codelang_round_trip 2>&1 | Select-String "error|test result"`
Expected: error de compilación "cannot find type `CodeLang`".

- [ ] **Step 3: Implementar `CodeLang` y `ViewMode`**

En `crates/core/src/preview.rs`, cerca del `PreviewRule` (antes de él), agregar:
```rust
/// Lenguaje para el resaltado de sintaxis (set curado). `as_str` es la clave estable de
/// serialización y el nombre que `core::highlight` mapea a la gramática de syntect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeLang {
    Xml, Json, Html, Css, JavaScript, C, Cpp, Java, Python, Rust, Sql, Bash,
    Markdown, Yaml, Toml, Ini,
}

impl CodeLang {
    pub fn as_str(self) -> &'static str {
        match self {
            CodeLang::Xml => "xml", CodeLang::Json => "json", CodeLang::Html => "html",
            CodeLang::Css => "css", CodeLang::JavaScript => "javascript", CodeLang::C => "c",
            CodeLang::Cpp => "cpp", CodeLang::Java => "java", CodeLang::Python => "python",
            CodeLang::Rust => "rust", CodeLang::Sql => "sql", CodeLang::Bash => "bash",
            CodeLang::Markdown => "markdown", CodeLang::Yaml => "yaml", CodeLang::Toml => "toml",
            CodeLang::Ini => "ini",
        }
    }
    pub fn from_str(s: &str) -> Option<CodeLang> {
        CodeLang::all().into_iter().find(|l| l.as_str() == s)
    }
    pub fn all() -> [CodeLang; 16] {
        [CodeLang::Xml, CodeLang::Json, CodeLang::Html, CodeLang::Css, CodeLang::JavaScript,
         CodeLang::C, CodeLang::Cpp, CodeLang::Java, CodeLang::Python, CodeLang::Rust,
         CodeLang::Sql, CodeLang::Bash, CodeLang::Markdown, CodeLang::Yaml, CodeLang::Toml,
         CodeLang::Ini]
    }
}

/// Modo de previsualización forzado por extensión. `Auto` = clasificación por extensión.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ViewMode {
    #[default]
    Auto,
    Text,
    Image,
    Code(CodeLang),
}
```

- [ ] **Step 4: Run — pasa**

Run: `cargo test -p naygo-core codelang_round_trip 2>&1 | Select-String "test result"`
Expected: `test result: ok`.

- [ ] **Step 5: Commit**

```
git add crates/core/src/preview.rs
git commit -m "feat(core): CodeLang + ViewMode para el preview"
```

---

### Task 2: `PreviewRule.view` reemplaza `treat_as` + migración

**Files:**
- Modify: `crates/core/src/preview.rs`
- Modify: `crates/core/src/config/mod.rs` (si construye PreviewRule con treat_as)

- [ ] **Step 1: Test de migración legacy**

Agregar al `mod tests`:
```rust
#[test]
fn migra_treat_as_a_view_mode() {
    // alias a lenguaje conocido -> Code
    assert_eq!(rule_from_legacy("sif", true, Some("xml")).view, ViewMode::Code(CodeLang::Xml));
    // alias a imagen -> Image
    assert_eq!(rule_from_legacy("raw", true, Some("png")).view, ViewMode::Image);
    // alias a extensión de texto -> Text
    assert_eq!(rule_from_legacy("foo", true, Some("txt")).view, ViewMode::Text);
    // sin alias -> Auto
    assert_eq!(rule_from_legacy("md", true, None).view, ViewMode::Auto);
    // alias desconocido -> Auto
    assert_eq!(rule_from_legacy("zz", true, Some("zzz")).view, ViewMode::Auto);
}
```

- [ ] **Step 2: Run — falla**

Run: `cargo test -p naygo-core migra_treat_as 2>&1 | Select-String "error|test result"`
Expected: error "cannot find function `rule_from_legacy`" / field `view`.

- [ ] **Step 3: Cambiar `PreviewRule` y agregar `rule_from_legacy`**

En `PreviewRule`, reemplazar el campo `treat_as: Option<String>` por `view: ViewMode`:
```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewRule {
    pub ext: String,
    pub enabled: bool,
    #[serde(default)]
    pub view: ViewMode,
}
```
Agregar la función de migración (mapea el alias viejo al nuevo modo):
```rust
/// Construye una regla desde el formato viejo (`treat_as` como alias de extensión).
/// alias lenguaje conocido -> Code; alias imagen -> Image; alias texto -> Text; resto -> Auto.
pub fn rule_from_legacy(ext: &str, enabled: bool, treat_as: Option<&str>) -> PreviewRule {
    let view = match treat_as {
        None => ViewMode::Auto,
        Some(a) => {
            let a = a.trim().trim_start_matches('.').to_lowercase();
            if let Some(l) = CodeLang::from_str(&a) {
                ViewMode::Code(l)
            } else if IMAGE_EXTENSIONS.contains(&a.as_str()) || SVG_EXTENSIONS.contains(&a.as_str()) {
                ViewMode::Image
            } else if DEFAULT_TEXT_EXTENSIONS.contains(&a.as_str()) {
                ViewMode::Text
            } else {
                ViewMode::Auto
            }
        }
    };
    PreviewRule { ext: ext.to_string(), enabled, view }
}
```

- [ ] **Step 4: Actualizar constructores de PreviewRule**

Buscar todos los `PreviewRule {` en el repo y reemplazar `treat_as: None` por `view: ViewMode::Auto`, y cualquier `treat_as: Some("x")` por `view: ViewMode::Code(...)`/etc. Sitios conocidos: `default_preview_rules`, `rules_from_csv`, los tests existentes de `classify_rules`. Comando para encontrarlos:
```
git grep -n "treat_as" -- crates/
```
Reemplazar cada uno. En los tests viejos de `classify_rules` que usaban `treat_as: Some("xml")` (alias a otra ext), cambiarlos a `view: ViewMode::Code(CodeLang::Xml)` y ajustar la expectativa (sigue dando `PreviewKind::Text`).

- [ ] **Step 5: `classify_rules` respeta `ViewMode`**

Modificar `classify_rules` para que, hallada la regla habilitada, decida por su `view`:
```rust
match &rule.view {
    ViewMode::Auto => kind_of_extension(&ext),       // como hoy
    ViewMode::Text => PreviewKind::Text,
    ViewMode::Image => PreviewKind::Image,
    ViewMode::Code(_) => PreviewKind::Text,           // el lenguaje se lee aparte (Task 3/4)
}
```
(El `kind_of_extension` actual se mantiene para `Auto`.) Agregar un getter para el lenguaje:
```rust
/// Si la regla de `path` fuerza un lenguaje de código, devuelve cuál (para el resaltado).
pub fn code_lang_for(path: &std::path::Path, rules: &[PreviewRule]) -> Option<CodeLang> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    rules.iter().find(|r| r.ext == ext && r.enabled).and_then(|r| match r.view {
        ViewMode::Code(l) => Some(l),
        _ => None,
    })
}
```

- [ ] **Step 6: Run tests + build core**

Run: `cargo test -p naygo-core preview 2>&1 | Select-String "test result"`
Expected: `test result: ok` (todos los de preview, incluidos los migrados).

- [ ] **Step 7: Commit**

```
git add crates/core/src/preview.rs crates/core/src/config/mod.rs
git commit -m "feat(core): PreviewRule.view (ViewMode) reemplaza treat_as + migracion"
```

---

### Task 3: `core::highlight` con syntect

**Files:**
- Modify: `crates/core/Cargo.toml` (dep syntect)
- Create: `crates/core/src/highlight.rs`
- Modify: `crates/core/src/lib.rs` (declarar `pub mod highlight;`)

- [ ] **Step 1: Agregar syntect a core**

En `crates/core/Cargo.toml`, sección `[dependencies]`, agregar:
```toml
# Resaltado de sintaxis para la vista previa de código. Set de gramáticas/temas embebido.
# `default-features = false` + `default-syntaxes`/`default-themes` para traer el set por
# defecto sin el backend de regex `onig` (usar `regex-fancy`, puro Rust, sin C).
syntect = { version = "5", default-features = false, features = ["default-syntaxes", "default-themes", "regex-fancy"] }
```
> Nota: si al compilar falta algún símbolo (p. ej. `parsing`/`html`), ajustar features. El
> objetivo es: gramáticas + temas embebidos + regex puro Rust (sin `onig`/C). Verificar el set
> de features real de la versión 5 de syntect al compilar (Step 4).

- [ ] **Step 2: Test de `highlight` (líneas + degradación)**

Crear `crates/core/src/highlight.rs` con el test primero:
```rust
// Naygo — resaltado de sintaxis para la vista previa (syntect). Puro: sin UI ni Windows.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use crate::preview::CodeLang;

/// Un segmento de texto coloreado dentro de una línea.
#[derive(Clone, Debug, PartialEq)]
pub struct HlSpan {
    pub text: String,
    pub color: (u8, u8, u8),
}

/// Una línea resaltada = varios segmentos.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct HlLine {
    pub spans: Vec<HlSpan>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resalta_json_en_lineas_con_segmentos() {
        let src = "{\n  \"a\": 1\n}";
        let lines = highlight(src, CodeLang::Json);
        assert_eq!(lines.len(), 3, "una HlLine por línea de entrada");
        // Alguna línea tiene más de un segmento (clave/valor con colores distintos).
        assert!(lines.iter().any(|l| l.spans.len() >= 2));
        // El texto concatenado de cada línea reconstruye la línea original (sin el \n).
        let rejoined: String = lines.iter()
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect::<Vec<_>>().join("\n");
        assert_eq!(rejoined, src);
    }

    #[test]
    fn degrada_sin_panic_con_texto_vacio_y_raro() {
        assert!(highlight("", CodeLang::Rust).is_empty() || highlight("", CodeLang::Rust).len() <= 1);
        // Texto que no es del lenguaje: no panica, devuelve líneas.
        let lines = highlight("esto no es rust válido <<<", CodeLang::Rust);
        assert_eq!(lines.len(), 1);
    }
}
```

- [ ] **Step 3: Implementar `highlight`**

En el mismo archivo (antes del `mod tests`):
```rust
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<syntect::highlighting::Theme> = OnceLock::new();

fn syntaxes() -> &'static SyntaxSet {
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}
fn theme() -> &'static syntect::highlighting::Theme {
    THEME.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        // Tema de código embebido fijo (oscuro, legible sobre el fondo del panel).
        ts.themes["base16-ocean.dark"].clone()
    })
}

/// Nombre de la gramática de syntect para cada `CodeLang` (token de búsqueda por extensión).
fn syntax_token(lang: CodeLang) -> &'static str {
    match lang {
        CodeLang::Xml => "xml", CodeLang::Json => "json", CodeLang::Html => "html",
        CodeLang::Css => "css", CodeLang::JavaScript => "js", CodeLang::C => "c",
        CodeLang::Cpp => "cpp", CodeLang::Java => "java", CodeLang::Python => "py",
        CodeLang::Rust => "rs", CodeLang::Sql => "sql", CodeLang::Bash => "sh",
        CodeLang::Markdown => "md", CodeLang::Yaml => "yaml", CodeLang::Toml => "toml",
        CodeLang::Ini => "ini",
    }
}

/// Resalta `text` (debe venir YA recortado por core::preview) como `lang`. Una HlLine por línea.
/// Si la gramática no se encuentra o algo falla, degrada a una línea de un span gris por línea.
pub fn highlight(text: &str, lang: CodeLang) -> Vec<HlLine> {
    let ss = syntaxes();
    let syntax = ss
        .find_syntax_by_extension(syntax_token(lang))
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let mut h = HighlightLines::new(syntax, theme());
    let mut out = Vec::new();
    for line in LinesWithEndings::from(text) {
        let spans = match h.highlight_line(line, ss) {
            Ok(ranges) => ranges
                .into_iter()
                .map(|(style, piece): (Style, &str)| HlSpan {
                    text: piece.trim_end_matches('\n').to_string(),
                    color: (style.foreground.r, style.foreground.g, style.foreground.b),
                })
                .filter(|s| !s.text.is_empty())
                .collect(),
            Err(_) => vec![HlSpan {
                text: line.trim_end_matches('\n').to_string(),
                color: (200, 200, 200),
            }],
        };
        out.push(HlLine { spans });
    }
    out
}
```

- [ ] **Step 4: Declarar el módulo y compilar**

En `crates/core/src/lib.rs`, agregar `pub mod highlight;` junto a los otros `pub mod`.
Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core highlight 2>&1 | Select-String "error|test result"`
Expected: compila; `test result: ok`. Si falla por features de syntect, ajustar el Step 1 (p. ej. el método `load_defaults_newlines`/`load_defaults` requiere la feature de dumps embebidos; el `highlight_line` requiere `parsing`+`highlighting`).

- [ ] **Step 5: Commit**

```
git add crates/core/Cargo.toml crates/core/src/highlight.rs crates/core/src/lib.rs Cargo.lock
git commit -m "feat(core): modulo highlight con syntect (set+tema embebidos)"
```

---

### Task 4: Worker de preview produce las líneas resaltadas

**Files:**
- Modify: `crates/ui-slint/src/preview.rs`

**Contexto:** el worker lee el archivo y arma un `Payload` (Text/Image/Message). Hay que: (a) que
el `Payload::Text` lleve un campo opcional `highlighted: Option<Vec<HlLine>>`; (b) cuando la
clasificación de la ruta da un `CodeLang` (vía `core::preview::code_lang_for`), tras recortar el
texto, llamar `core::highlight::highlight(texto_recortado, lang)` y guardarlo.

- [ ] **Step 1: Ampliar el `Payload::Text`**

Localizar el enum `Payload` en `preview.rs`. Añadir el campo a la variante de texto:
```rust
// antes: Text { text: String, truncated: bool }
Text { text: String, truncated: bool, highlighted: Option<Vec<naygo_core::highlight::HlLine>> },
```
Actualizar TODAS las construcciones de `Payload::Text { ... }` en el archivo (read_text, read_pdf, los mensajes de error que devuelven texto) agregando `highlighted: None` salvo donde se resalte.

- [ ] **Step 2: Resaltar en la lectura de texto**

En la función que lee texto (la que usa `truncate_text`, ~`read_text`), tras obtener el
`TruncatedText`, resaltar si la ruta fuerza un lenguaje. La función necesita acceso a las reglas
de preview (las recibe ya, o pasarle el `CodeLang` resuelto). Patrón:
```rust
let t = naygo_core::preview::truncate_text(&buf, hit_cap);
let highlighted = lang.map(|l| naygo_core::highlight::highlight(&t.text, l));
Payload::Text { text: t.text, truncated: t.truncated, highlighted }
```
Donde `lang: Option<CodeLang>` se calcula con `core::preview::code_lang_for(path, &rules)` en el
punto donde el worker ya conoce la ruta y las reglas. Si el worker no tiene las reglas a mano,
pasarlas al spawn del job (revisar cómo se arranca el worker; las reglas viven en Settings).

- [ ] **Step 3: Build**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-String "error|Finished"`
Expected: `Finished`. (Errores típicos: faltó `highlighted: None` en alguna construcción de `Payload::Text`.)

- [ ] **Step 4: Commit**

```
git add crates/ui-slint/src/preview.rs
git commit -m "feat(ui): worker de preview resalta el codigo (core::highlight)"
```

---

### Task 5: Contrato hacia la UI — PreviewVm con líneas resaltadas

**Files:**
- Modify: `crates/ui-slint/ui/types.slint`
- Modify: `crates/ui-slint/src/main.rs` y/o `src/preview.rs` (armado del PreviewVm)

- [ ] **Step 1: Structs en types.slint**

Junto a `PreviewVm` en `types.slint`, agregar:
```slint
export struct HlSpanVm { text: string, color: color }
export struct HlLineVm { spans: [HlSpanVm] }
```
Y a `PreviewVm` agregar dos campos:
```slint
    highlighted: bool,
    hl-lines: [HlLineVm],
```

- [ ] **Step 2: Poblar el PreviewVm desde el Payload**

En el punto donde se construye el `PreviewVm` a partir del resultado del worker (buscar
`PreviewVm {` en main.rs/preview.rs), mapear:
```rust
let (highlighted, hl_lines) = match &payload {
    Payload::Text { highlighted: Some(lines), .. } => (
        true,
        lines.iter().map(|l| HlLineVm {
            spans: ModelRc::new(VecModel::from(
                l.spans.iter().map(|s| HlSpanVm {
                    text: s.text.clone().into(),
                    color: slint::Color::from_rgb_u8(s.color.0, s.color.1, s.color.2),
                }).collect::<Vec<_>>()
            )),
        }).collect::<Vec<_>>(),
    ),
    _ => (false, Vec::new()),
};
// setear vm.highlighted = highlighted; vm.hl_lines = ModelRc::new(VecModel::from(hl_lines));
```
Ajustar a cómo el código arma hoy el VM (mode/text/image). El `mode` sigue siendo 1 (texto)
cuando hay resaltado; `highlighted` decide qué se pinta.

- [ ] **Step 3: Build**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-String "error|Finished"`
Expected: `Finished`.

- [ ] **Step 4: Commit**

```
git add crates/ui-slint/ui/types.slint crates/ui-slint/src/main.rs crates/ui-slint/src/preview.rs
git commit -m "feat(ui): PreviewVm lleva las lineas resaltadas (hl-lines)"
```

---

### Task 6: Render del código resaltado en preview-panel.slint

**Files:**
- Modify: `crates/ui-slint/ui/preview-panel.slint`

- [ ] **Step 1: Pintar por segmentos cuando highlighted**

En el `mode == 1` (texto), dentro del `ScrollView`, reemplazar el `Text` único por una rama
condicional:
```slint
if root.view.highlighted: VerticalLayout {
    for line in root.view.hl-lines: HorizontalLayout {
        for seg in line.spans: Text {
            text: seg.text;
            color: seg.color;
            font-family: "Consolas";
            // sin wrap: cada línea ya viene recortada por core (defensa i16).
        }
        // Espaciador para que líneas vacías ocupen alto:
        Rectangle { }
    }
}
if !root.view.highlighted: Text {
    text: root.view.text;
    color: Theme.text;
    font-family: "Consolas";
    wrap: word-wrap;
}
```

- [ ] **Step 2: Build + verificar que el `.slint` compila**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-String "error|Finished"`
Expected: `Finished`. Si Slint 1.16 no acepta algo de la sintaxis (p. ej. `for` anidado dentro de `HorizontalLayout`), ajustar manteniendo: una fila por línea, un Text por segmento.

- [ ] **Step 3: Commit**

```
git add crates/ui-slint/ui/preview-panel.slint
git commit -m "feat(ui): render del codigo resaltado por segmentos en el preview"
```

---

### Task 7: Botón "abrir con el programa del sistema"

**Files:**
- Modify: `crates/ui-slint/ui/preview-panel.slint` (botón + callback)
- Modify: `crates/ui-slint/ui/app-window.slint` (propagar callback + ruta)
- Modify: `crates/ui-slint/src/main.rs` (handler → open_default)
- Modify: i18n triple (tooltip)

- [ ] **Step 1: i18n del tooltip**

`ui/i18n.slint`: `in property <string> preview-open-tip: "Abrir con el programa predeterminado";`
`core/src/i18n/es.json`: `"slint.preview.open_tip": "Abrir con el programa predeterminado",`
`core/src/i18n/en.json`: `"slint.preview.open_tip": "Open with the default program",`
`ui-slint/src/i18n_keys.rs`: `tr.set_preview_open_tip(c.t("slint.preview.open_tip").into());`
(Verificar que `preview-open-tip` no choca con una clave existente.)

- [ ] **Step 2: Botón + callback + ruta en preview-panel.slint**

`PreviewPanel` necesita exponer la ruta del archivo previsualizado. Agregar a `PreviewVm`
(types.slint) un campo `path: string` poblado en el armado del VM (la ruta del archivo en
preview, "" si ninguno). En `preview-panel.slint`:
```slint
callback open-in-system();
```
En la barra de título del panel, un botón (ícono Path, no glifo) visible cuando `root.view.mode != 0`:
`TouchArea { clicked => { root.open-in-system(); } }` con su tooltip `Tr.preview-open-tip`.

- [ ] **Step 3: Propagar en app-window.slint**

Donde se instancia `PreviewPanel { ... }`, cablear `open-in-system() => { root.preview-open(p.path); }`
y declarar en AppWindow `callback preview-open(string);` (pasando la ruta del VM del panel).

- [ ] **Step 4: Handler en main.rs**

```rust
ui.on_preview_open(move |path| {
    if !path.is_empty() {
        let _ = naygo_platform::open::open_default(std::path::Path::new(path.as_str()));
    }
});
```
(Colocar junto a los otros `ui.on_...`; `open_default` ya está en uso en el crate.)

- [ ] **Step 5: Build**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-String "error|Finished"`
Expected: `Finished`.

- [ ] **Step 6: Commit**

```
git add crates/ui-slint/ui/preview-panel.slint crates/ui-slint/ui/app-window.slint crates/ui-slint/ui/types.slint crates/ui-slint/ui/i18n.slint crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/src/i18n_keys.rs crates/ui-slint/src/main.rs
git commit -m "feat(ui): boton abrir con el programa del sistema en el preview"
```

---

### Task 8: Configuración — doble combobox (ViewMode + CodeLang)

**Files:**
- Modify: `crates/ui-slint/ui/config-window.slint` (sección Previsualización, ~660)
- Modify: `crates/ui-slint/ui/types.slint` (VM de la fila de regla, si aplica)
- Modify: `crates/ui-slint/src/main.rs` / `config_ctrl.rs` (callbacks de set)
- Modify: i18n triple (etiquetas de los modos)

- [ ] **Step 1: i18n de los modos de vista**

Agregar claves (triple) sin reusar nombres:
- `preview-view-auto` / `slint.preview.view_auto` → ES "Automático" / EN "Automatic"
- `preview-view-text` / `slint.preview.view_text` → ES "Ver como texto" / EN "View as text"
- `preview-view-image` / `slint.preview.view_image` → ES "Ver como imagen" / EN "View as image"
- `preview-view-code` / `slint.preview.view_code` → ES "Ver como código" / EN "View as code"
- `preview-lang` / `slint.preview.lang` → ES "Lenguaje" / EN "Language"
Setear las 5 en `i18n_keys.rs`.

- [ ] **Step 2: Exponer modo+lenguaje por fila en el VM de config**

Buscar el struct que modela cada fila de regla de preview en types.slint (el que hoy lleva
`ext`, `enabled`, `treat_as`). Reemplazar `treat_as: string` por:
```slint
    view-index: int,     // 0=Auto 1=Texto 2=Imagen 3=Código
    lang-index: int,     // índice en CodeLang::all() (solo si view-index==3)
```
El armado del VM (en el ConfigCtrl / build_settings_vm) mapea `ViewMode` → `view-index`/`lang-index`.

- [ ] **Step 3: Render del doble combobox**

En la sección Previsualización, reemplazar el `TextInput` de `treat_as` por:
```slint
ComboBox {
    model: [Tr.preview-view-auto, Tr.preview-view-text, Tr.preview-view-image, Tr.preview-view-code];
    current-index: rule.view-index;
    selected => { root.set-view-mode(rule.ext, self.current-index); }
}
if rule.view-index == 3: ComboBox {
    model: root.lang-names;   // property <[string]> con los nombres de CodeLang::all()
    current-index: rule.lang-index;
    selected => { root.set-view-lang(rule.ext, self.current-index); }
}
```
Declarar en ConfigWindow: `callback set-view-mode(string, int);`, `callback set-view-lang(string, int);`,
y `in property <[string]> lang-names;` (poblada desde Rust con `CodeLang::all().map(as_str)` o nombres legibles).
La fila de "Agregar una extensión" usa los mismos combobox.

- [ ] **Step 4: Callbacks en main.rs → ConfigCtrl**

```rust
cfg_win.on_set_view_mode(move |ext, idx| {
    ctrl.borrow_mut().config.set_preview_view_mode(ext.as_str(), idx);   // método nuevo en ConfigCtrl
    // refrescar el SettingsVm
});
cfg_win.on_set_view_lang(move |ext, idx| {
    ctrl.borrow_mut().config.set_preview_view_lang(ext.as_str(), idx);
});
```
En `config_ctrl.rs`, `set_preview_view_mode(ext, idx)` traduce idx→`ViewMode` (0=Auto,1=Text,2=Image,
3=Code(lang actual o Xml por defecto)) sobre la regla de esa ext y persiste; `set_preview_view_lang(ext, idx)`
fija `ViewMode::Code(CodeLang::all()[idx])`. Ambos guardan (`save()`).

- [ ] **Step 5: Build**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-String "error|Finished"`
Expected: `Finished`.

- [ ] **Step 6: Commit**

```
git add crates/ui-slint/ui/config-window.slint crates/ui-slint/ui/types.slint crates/ui-slint/ui/i18n.slint crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/src/i18n_keys.rs crates/ui-slint/src/main.rs crates/ui-slint/src/config_ctrl.rs
git commit -m "feat(config): doble combobox de modo de vista + lenguaje en Previsualizacion"
```

---

### Task 9: Gate integral + THIRD-PARTY-NOTICES + CHANGELOG

**Files:**
- Modify: `THIRD-PARTY-NOTICES.md` (regenerar), `CHANGELOG.md`

- [ ] **Step 1: cargo fmt**

Run: `cargo fmt`
Expected: reformatea; `git diff --stat` solo formato.

- [ ] **Step 2: Tests de los 3 crates**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace 2>&1 | Select-String "test result"`
Expected: naygo-core (incluye highlight + preview), naygo-platform, naygo-ui-slint todos `ok`.
Verificar especialmente `es_en_tienen_las_mismas_claves` (se agregaron claves a ambos JSON).

- [ ] **Step 3: Clippy estricto**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo clippy --workspace --all-targets -- -D warnings 2>&1 | Select-Object -Last 4`
Expected: `Finished`, sin warnings.

- [ ] **Step 4: Barrido de voseo en archivos tocados**

Run:
```
git diff --name-only HEAD~8 | ForEach-Object { Select-String -Path $_ -Pattern "avis[aá]s|dec[ií]s|quer[ée]s|asegurate|hac[ée]|eleg[ií]|fijate|mir[áa]|pod[ée]s|commiteá|ejecutá|descargá" -ErrorAction SilentlyContinue }
```
Expected: sin coincidencias reales (revisar falsos positivos como "elegir"/"deshacer").

- [ ] **Step 5: Regenerar THIRD-PARTY-NOTICES (syntect MIT + su árbol)**

Run:
```
cargo license --avoid-dev-deps --avoid-build-deps --filter-platform x86_64-pc-windows-msvc --tsv > $env:TEMP\lic.tsv
```
Regenerar `THIRD-PARTY-NOTICES.md` con el mismo formato del existente (lista por licencia +
sección Slint). syntect y sus deps (onig NO, regex-fancy/fancy-regex sí) son permisivas (MIT/
Apache). Verificar que no entró ninguna licencia nueva no-permisiva; si aparece una rara, avisar.

- [ ] **Step 6: CHANGELOG**

En `## [Sin publicar]` → `### Añadido`:
```markdown
- La Vista previa resalta el código por colores (XML, JSON, HTML, CSS, JavaScript, C/C++, Java,
  Python, Rust, SQL, Bash, Markdown, YAML, TOML, INI) y se puede forzar el modo de vista por
  extensión (Automático / texto / imagen / código + lenguaje) en Configuración → Previsualización.
- Botón para abrir el archivo previsualizado con el programa predeterminado del sistema.
```

- [ ] **Step 7: Commit + graphify**

```
git add THIRD-PARTY-NOTICES.md CHANGELOG.md
git commit -m "docs: CHANGELOG + THIRD-PARTY-NOTICES (syntect) para el resaltado del preview"
graphify update .
```

---

## Verificación de Nicolás (visual, en la VM)

- Previsualizar un .json/.xml/.rs → se ve coloreado, nítido, sin caídas.
- Configuración → Previsualización: combobox de modo; al elegir "Ver como código" aparece el de
  lenguaje; forzar una extensión cualquiera a "código/JSON" y verificar que resalta.
- Botón "abrir con el sistema" abre el archivo con su programa por defecto.
- Probar un archivo de log enorme con líneas largas en modo código → NO se cae (recorte i16).
- Tema claro y oscuro: el código se lee bien sobre el fondo del panel.
