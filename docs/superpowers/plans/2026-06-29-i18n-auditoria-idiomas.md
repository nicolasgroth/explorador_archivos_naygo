# Auditoría i18n + idiomas nuevos — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mover a clave i18n todo texto visible hardcodeado (sobre todo en preview.rs y archive_tree) y agregar 8 idiomas (pt/fr/de/it completos + zh/ja/ko/hi experimentales), con parity estricto.

**Architecture:** Parte 1 — auditoría: cada `Payload::Message("...")` de preview.rs pasa a `Payload::Message(c.t("preview.err.*"))` (el worker recibe los textos ya resueltos); archive_tree recibe un `ArchiveLabels` con los textos traducidos. Parte 2 — los 8 JSON nuevos se embeben, el selector los lista por nombre nativo (`lang.<code>`) con marca experimental para CJK/hindi, y un parity test exige que todos tengan las claves de es.json. Parte 3 — traducir las ~800 claves por idioma.

**Tech Stack:** Rust, JSON i18n, Slint.

---

## Contexto y convenciones

- Rama: `feat/iconos-personalizables`.
- ⚠️ **REGLA GIT (STRICT):** SOLO `git add <rutas explícitas>` + `git commit`. PROHIBIDO `git reset/restore/checkout/stash/clean`, `git commit -a`/`-am`, `git add -A`/`add .`. 2 archivos ajenos (`CLAUDE.md`, `crates/core/src/favorites.rs`) NO se tocan ni stagean. Si el árbol parece mal, PARAR y reportar.
- Header SPDX en archivos nuevos. Comentarios/commits español NEUTRAL, NUNCA voseo.
- Build limpio + clippy + tests antes de commit. Tests core: `cargo test -p naygo-core`. ui: `cargo test -p naygo-ui-slint`, `cargo build -p naygo-ui-slint --bins`.

## Hechos verificados

- i18n: `crates/core/src/i18n/mod.rs` embebe es/en con `include_str!("es.json")` / `en.json` (~líneas 65-66). `available: Vec<LangId>`. Fallback: activo → ES → la clave. `I18n::t(key) -> String` (el accesor; en ui-slint es `c.config.t(...)` o `c.t(...)`).
- Nombre nativo de idioma: clave `lang.<code>` en los JSON (`"lang.es": "Español"`, `"lang.en": "English"`).
- El worker de preview (`crates/ui-slint/src/preview.rs`) produce `Payload::Message(String)` con ~20 literales españoles hardcodeados (líneas 234,276,287,376,392,410,413,416,419,422,445,449,451,454,461,464,475,480,518,520). El worker corre en un hilo SIN acceso al `I18n`; los textos deben PASARSE ya resueltos.
- `crates/core/src/archive_tree.rs` tiene strings ES en `render_archive_tree` (líneas ~102,111,115).
- `crates/ui-slint/ui/splash.slint:30` tiene `text: "Explorador de archivos"` hardcodeado.
- Parity test actual: `es_en_tienen_las_mismas_claves` (i18n/mod.rs:164).
- Los `in property <string> X: "default"` de `ui/i18n.slint` son defaults del cableado Tr (se sobrescriben desde Rust) — NO son hardcodes a arreglar.

## Mapa de archivos

**Crear:**
- `crates/core/src/i18n/{pt,fr,de,it,zh,ja,ko,hi}.json` — catálogos nuevos.

**Modificar:**
- `crates/core/src/archive_tree.rs` — `ArchiveLabels` + `render_archive_tree` parametrizado.
- `crates/ui-slint/src/preview.rs` — los `Payload::Message` reciben texto resuelto (struct `PreviewLabels` o pasar el I18n al worker).
- `crates/core/src/i18n/mod.rs` — embeber los 8 + parity test ampliado + test de placeholders.
- `crates/core/src/i18n/es.json`, `en.json` — claves nuevas de la auditoría + `lang.*` de los 8 idiomas + `lang.experimental_suffix`.
- `crates/ui-slint/src/main.rs` o donde se arma el selector — marca experimental.
- `crates/ui-slint/ui/splash.slint` — usar Tr en vez del literal.

---

## Fase 1 — Auditoría: archive_tree parametrizado

### Task 1: `ArchiveLabels` en core::archive_tree

**Files:**
- Modify: `crates/core/src/archive_tree.rs`

- [ ] **Step 1: Write the failing test**

Adaptar los tests existentes y agregar uno. En `crates/core/src/archive_tree.rs`, agregar el struct y cambiar la firma. El test:

```rust
#[test]
fn render_usa_los_labels_dados() {
    let labels = ArchiveLabels {
        files: "file(s)".into(),
        folders: "folder(s)".into(),
        uncompressed: "uncompressed".into(),
        more_entries: "… and more entries".into(),
        and_more: "… and {n} more".into(),
    };
    let entries = vec![ArchiveEntry { path: "a.txt".into(), is_dir: false, size: 5 }];
    let summary = ArchiveSummary { files: 1, dirs: 0, total_uncompressed: 5, truncated: false, total_entries: 1 };
    let out = render_archive_tree(&entries, &summary, "x.zip", SizeFormat::Auto, &labels);
    assert!(out.contains("file(s)"));
    assert!(out.contains("uncompressed"));
    assert!(!out.contains("archivo"), "no debe haber español hardcodeado");
}
```

- [ ] **Step 2: Run, confirm fail**

Run: `cargo test -p naygo-core render_usa_los_labels`
Expected: FAIL — `ArchiveLabels` no existe; `render_archive_tree` tiene otra firma.

- [ ] **Step 3: Implement**

En `archive_tree.rs`, agregar el struct:
```rust
/// Textos traducidos para el encabezado/pie del árbol. La capa de UI los pasa ya resueltos
/// (core no conoce el catálogo i18n). `and_more` es una plantilla con `{n}`.
#[derive(Clone, Debug)]
pub struct ArchiveLabels {
    pub files: String,
    pub folders: String,
    pub uncompressed: String,
    pub more_entries: String,
    pub and_more: String,
}
```
Cambiar la firma de `render_archive_tree` para recibir `labels: &ArchiveLabels` y usar los labels:
```rust
pub fn render_archive_tree(
    entries: &[ArchiveEntry],
    summary: &ArchiveSummary,
    name: &str,
    size_fmt: SizeFormat,
    labels: &ArchiveLabels,
) -> String {
    let mut out = String::new();
    out.push_str(name);
    out.push('\n');
    out.push_str(&format!(
        "{} {}, {} {} · {} {}\n",
        summary.files, labels.files, summary.dirs, labels.folders,
        format_size(summary.total_uncompressed, size_fmt), labels.uncompressed,
    ));
    out.push_str("──────────────────────────────\n");
    let root = build_tree(entries);
    render_children(&root.children, "", &mut out, size_fmt);
    if summary.truncated {
        let extra = summary.total_entries.saturating_sub(entries.len());
        if extra > 0 {
            out.push_str(&format!("\n{}\n", labels.and_more.replace("{n}", &extra.to_string())));
        } else {
            out.push_str(&format!("\n{}\n", labels.more_entries));
        }
    }
    out
}
```
Actualizar los tests existentes de archive_tree para pasar un `ArchiveLabels` de prueba (con textos en español como antes, p.ej. `files: "archivo(s)"`, etc., para que las aserciones `contains("archivo")` sigan teniendo sentido — o ajusta las aserciones).

- [ ] **Step 4: Run, confirm pass**

Run: `cargo test -p naygo-core archive_tree`
Expected: PASS (todos, adaptados).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/archive_tree.rs
git commit -m "refactor(core): render_archive_tree recibe ArchiveLabels (textos traducibles, no hardcode)"
```

---

### Task 2: Claves i18n del archive_tree + preview.rs arma los labels

**Files:**
- Modify: `crates/core/src/i18n/es.json`, `en.json`
- Modify: `crates/ui-slint/src/preview.rs`

- [ ] **Step 1: Claves i18n**

es.json (junto a otras `archive.*` o `preview.*`):
```json
"archive.files": "archivo(s)",
"archive.folders": "carpeta(s)",
"archive.uncompressed": "sin comprimir",
"archive.more_entries": "… y más entradas",
"archive.and_more": "… y {n} más",
```
en.json:
```json
"archive.files": "file(s)",
"archive.folders": "folder(s)",
"archive.uncompressed": "uncompressed",
"archive.more_entries": "… and more entries",
"archive.and_more": "… and {n} more",
```
Validar JSON + parity: `cargo test -p naygo-core i18n`.

- [ ] **Step 2: preview.rs arma ArchiveLabels**

El worker corre en un hilo sin I18n. La forma limpia: pasar los labels al worker cuando se lanza. READ cómo `read_archive_listing`/`build_payload` reciben datos (rules, auto_highlight). El I18n vive en el controller (hilo de UI). Opciones:
- (a) Pasar un `ArchiveLabels` (resuelto en el hilo de UI) al worker junto con `rules`. Requiere threadear el struct por el canal/closure del worker.
- (b) Como `ArchiveLabels` es solo 5 strings, resolverlos en el hilo de UI cuando se construye el job y pasarlos.

READ `preview.rs` para ver cómo se lanza el worker (`start`, el `thread::spawn`, qué captura). Lo más simple: cuando el hilo de UI prepara el job (donde ya tiene acceso a `c.t(...)`), construir el `ArchiveLabels` y pasarlo al worker (capturarlo en el closure del spawn). Si el worker ya recibe `rules: Vec<PreviewRule>`, agregar `labels: ArchiveLabels` al mismo punto.

Implementación: en la función del worker que llama `render_archive_tree`, pasar el `labels` recibido. En el sitio del hilo de UI que lanza el worker, construir:
```rust
let labels = naygo_core::archive_tree::ArchiveLabels {
    files: c.t("archive.files"),
    folders: c.t("archive.folders"),
    uncompressed: c.t("archive.uncompressed"),
    more_entries: c.t("archive.more_entries"),
    and_more: c.t("archive.and_more"),
};
```
y threadearlo hasta `read_archive_listing`. Adapta a la estructura real del worker (lee el archivo primero). Si threadear el struct por todo el worker es muy invasivo, una alternativa aceptable: dejar que `read_archive_listing` reciba `&ArchiveLabels` como parámetro y propagarlo desde `build_payload` (que ya recibe `rules`). El job que se le pasa al worker debe incluir los labels.

> Este task tiene trabajo de plomería real. Si al leer el worker ves que pasar el struct es complejo, repórtalo y proponemos la variante más limpia. NO hardcodees los textos de vuelta.

- [ ] **Step 3: Compilar + tests**

Run: `cargo build -p naygo-ui-slint --bins` + `cargo test -p naygo-ui-slint preview`
Expected: compila + tests verdes (los tests de preview que verifican el árbol ahora reciben labels en español, así que las aserciones `contains("archivo")` siguen valiendo si los tests pasan labels ES; ajustar si hace falta).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/src/preview.rs
git commit -m "feat(i18n): archive_tree usa claves i18n (preview de comprimidos traducible)"
```

---

### Task 3: i18n de los Payload::Message de preview.rs

**Files:**
- Modify: `crates/core/src/i18n/es.json`, `en.json`
- Modify: `crates/ui-slint/src/preview.rs`

- [ ] **Step 1: Claves i18n (los ~12 mensajes únicos)**

Los literales se repiten; las claves únicas son: `preview.err.not_previewable` ("No previsualizable"), `preview.err.archive_bad` ("Archivo comprimido inválido o dañado"), `preview.err.read` ("No se pudo leer"), `preview.err.image_big` ("Imagen muy grande"), `preview.err.cancelled` ("Cancelado"), `preview.err.decode` ("No se pudo decodificar"), `preview.err.svg_big` ("SVG muy grande"), `preview.err.svg_bad` ("SVG inválido"), `preview.err.rasterize` ("No se pudo rasterizar"), `preview.err.pdf_big` ("PDF muy grande").
Agregar todas a es.json + en.json (EN: "Not previewable", "Invalid or damaged archive", "Could not read", "Image too large", "Cancelled", "Could not decode", "SVG too large", "Invalid SVG", "Could not rasterize", "PDF too large"). Validar parity.

- [ ] **Step 2: preview.rs usa las claves**

El worker no tiene I18n. Igual que los labels del archive (Task 2): resolver estos mensajes en el hilo de UI y pasarlos al worker, O — más simple dado que son mensajes de error de muestra — pasar un struct `PreviewMessages` (resuelto en UI) al worker. READ cómo el worker produce `Payload::Message`; lo más limpio: que el worker devuelva una VARIANTE TIPADA de error (un enum `PreviewError { NotPreviewable, Read, Cancelled, ImageBig, ... }`) en vez de un String, y que el hilo de UI traduzca ese enum a texto con `c.t(...)` al convertir `Payload` → `PreviewVm`.

> Esta es la opción ARQUITECTÓNICAMENTE CORRECTA: el worker (core de UI, sin i18n) emite errores TIPADOS; la traducción ocurre en el hilo de UI que sí tiene el catálogo. Cambiar `Payload::Message(String)` por `Payload::Message(PreviewError)` (un enum nuevo en preview.rs), y donde se convierte Payload→PreviewVm, mapear el enum a `c.t("preview.err.<x>")`.

Implementar:
```rust
/// Motivo por el que no hay vista previa (lo traduce el hilo de UI con i18n).
#[derive(Clone, Debug)]
pub enum PreviewError {
    NotPreviewable, ArchiveBad, Read, ImageBig, Cancelled, Decode, SvgBig, SvgBad, Rasterize, PdfBig,
}
```
Cambiar todos los `Payload::Message("texto".to_string())` por `Payload::Message(PreviewError::X)`. En el punto donde `Payload::Message(m)` se convierte a `ViewCache::Message`/`PreviewVm` (busca `Payload::Message(m) =>`), traducir: `let texto = c.t(preview_error_key(m)); ...` con un helper `fn preview_error_key(e: &PreviewError) -> &'static str` que mapea cada variante a su clave. Necesitas `c` (I18n) en ese punto — está en el hilo de UI, sí lo tiene.

> Si el sitio de conversión Payload→VM no tiene fácil acceso al I18n, repórtalo; pero debería tenerlo (es el hilo de UI). Adapta.

- [ ] **Step 3: Compilar + tests**

Run: `cargo build -p naygo-ui-slint --bins` + `cargo test -p naygo-ui-slint preview`
Expected: compila + verde. Los tests que afirman `Payload::Message(_)` siguen valiendo (la variante cambió de String a enum, ajustar el `matches!` si hace falta).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/src/preview.rs
git commit -m "refactor(ui): errores de preview tipados (PreviewError) traducidos por i18n"
```

---

### Task 4: splash.slint + barrido final de hardcodes

**Files:**
- Modify: `crates/ui-slint/ui/splash.slint`, `crates/ui-slint/ui/i18n.slint`, `crates/ui-slint/src/i18n_keys.rs`
- Modify: `crates/core/src/i18n/es.json`, `en.json`

- [ ] **Step 1: splash a i18n**

`splash.slint:30` tiene `text: "Explorador de archivos"`. Agregar `Tr.splash-subtitle` (`in property <string> splash-subtitle: "Explorador de archivos";` en i18n.slint) + setter en i18n_keys.rs (`tr.set_splash_subtitle(c.t("splash.subtitle").into())`) + claves `splash.subtitle` en es.json ("Explorador de archivos") / en.json ("File explorer"). Cambiar el literal en splash.slint por `Tr.splash-subtitle`.
> OJO: el splash puede mostrarse ANTES de que i18n cargue. Si es así, el default del `in property` ("Explorador de archivos") es el fallback — aceptable. Verifica que el splash reciba el Tr; si el splash no tiene acceso a Tr (ventana separada), deja el default y documenta que el splash usa el idioma por defecto (es un subtítulo de 1 segundo). Usa tu criterio.

- [ ] **Step 2: barrido final**

Correr un grep de verificación para detectar literales españoles visibles que queden:
```bash
grep -rn 'Message("[A-ZÁÉ]\|text: "[A-ZÁÉ][a-záéíóú]' crates --include="*.rs" --include="*.slint" | grep -v "test\|i18n.slint\|//"
```
Revisar cada resultado: si es texto visible al usuario, moverlo a clave; si es un default de Tr, un ejemplo de ruta, o no-visible, dejarlo (documentar por qué). El `file-panel.slint:508` placeholder `"C:\\…"` es un ejemplo de ruta, NO traducir (es ilustrativo, igual en todo idioma).

- [ ] **Step 3: Compilar + commit**

Run: `cargo build -p naygo-ui-slint --bins` + `cargo test -p naygo-core i18n`.
```bash
git add crates/ui-slint/ui/splash.slint crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(i18n): subtítulo del splash por clave + barrido final de hardcodes"
```

---

## Fase 2 — Infraestructura de los 8 idiomas

### Task 5: Crear los 8 JSON (copia de en.json) + embeber + lang.* + experimental

**Files:**
- Create: `crates/core/src/i18n/{pt,fr,de,it,zh,ja,ko,hi}.json`
- Modify: `crates/core/src/i18n/mod.rs`, `es.json`, `en.json`

- [ ] **Step 1: Crear los 8 JSON como copia de en.json**

Copiar `en.json` a cada uno de los 8 archivos nuevos (placeholder: parten en inglés para que compilen y el parity pase; se traducen en Fase 3). 
```bash
for l in pt fr de it zh ja ko hi; do cp crates/core/src/i18n/en.json crates/core/src/i18n/$l.json; done
```

- [ ] **Step 2: Agregar las claves `lang.<code>` + sufijo experimental a TODOS los JSON**

En es.json, en.json Y los 8 nuevos (parity: la clave existe en todos con el mismo valor — el nombre nativo NO se traduce):
```json
"lang.pt": "Português",
"lang.fr": "Français",
"lang.de": "Deutsch",
"lang.it": "Italiano",
"lang.zh": "中文",
"lang.ja": "日本語",
"lang.ko": "한국어",
"lang.hi": "हिन्दी",
"lang.experimental_suffix": " (experimental)",
```
(es.json y en.json ya tienen `lang.es`/`lang.en`; agregar también esos a los 8 nuevos para parity total.)
> Como los 8 nuevos son copia de en.json, agregar estas claves a en.json ANTES de copiar (Step 1) las propaga. Mejor orden: agregar las lang.* a es.json + en.json PRIMERO, validar parity es↔en, LUEGO copiar en.json a los 8. Reordena los steps si conviene.

- [ ] **Step 3: Embeber los 8 en mod.rs**

En `crates/core/src/i18n/mod.rs`, junto a `const ES_JSON` / `EN_JSON` (~65-66), agregar:
```rust
const PT_JSON: &str = include_str!("pt.json");
const FR_JSON: &str = include_str!("fr.json");
const DE_JSON: &str = include_str!("de.json");
const IT_JSON: &str = include_str!("it.json");
const ZH_JSON: &str = include_str!("zh.json");
const JA_JSON: &str = include_str!("ja.json");
const KO_JSON: &str = include_str!("ko.json");
const HI_JSON: &str = include_str!("hi.json");
```
Y donde se registran los catálogos embebidos (busca dónde se usan ES_JSON/EN_JSON para construir el catálogo + `available`), agregar los 8 con sus códigos. Verás un punto que hace algo como `catalogs.insert("es", Catalog::from_json("es", ES_JSON))` y un `available` con los LangId. Agregar los 8 ahí.

- [ ] **Step 4: Compilar + parity (los 8 == es porque son copia de en, y en == es)**

Run: `cargo build -p naygo-core` + `cargo test -p naygo-core i18n`
Expected: compila; el parity es↔en sigue verde. (El parity de los 8 nuevos se agrega en Task 6.)

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/i18n/pt.json crates/core/src/i18n/fr.json crates/core/src/i18n/de.json crates/core/src/i18n/it.json crates/core/src/i18n/zh.json crates/core/src/i18n/ja.json crates/core/src/i18n/ko.json crates/core/src/i18n/hi.json crates/core/src/i18n/mod.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(i18n): 8 idiomas nuevos embebidos (placeholder en inglés) + nombres nativos"
```

---

### Task 6: Parity estricto de los 10 idiomas + test de placeholders

**Files:**
- Modify: `crates/core/src/i18n/mod.rs` (tests)

- [ ] **Step 1: Write the failing test**

En `crates/core/src/i18n/mod.rs`, agregar:
```rust
#[test]
fn todos_los_idiomas_tienen_las_claves_de_es() {
    let langs: &[(&str, &str)] = &[
        ("en", EN_JSON), ("pt", PT_JSON), ("fr", FR_JSON), ("de", DE_JSON),
        ("it", IT_JSON), ("zh", ZH_JSON), ("ja", JA_JSON), ("ko", KO_JSON), ("hi", HI_JSON),
    ];
    let es: std::collections::HashMap<String, String> = serde_json::from_str(ES_JSON).unwrap();
    let es_keys: std::collections::BTreeSet<&String> = es.keys().collect();
    for (code, json) in langs {
        let m: std::collections::HashMap<String, String> = serde_json::from_str(json).unwrap();
        let keys: std::collections::BTreeSet<&String> = m.keys().collect();
        assert_eq!(keys, es_keys, "el idioma {code} no tiene exactamente las claves de es.json");
    }
}

#[test]
fn placeholders_consistentes_entre_idiomas() {
    // Para cada clave de es.json con {x}, los demás idiomas deben tener el mismo {x}.
    use std::collections::HashMap;
    let es: HashMap<String, String> = serde_json::from_str(ES_JSON).unwrap();
    let langs: &[&str] = &[EN_JSON, PT_JSON, FR_JSON, DE_JSON, IT_JSON, ZH_JSON, JA_JSON, KO_JSON, HI_JSON];
    let extract = |s: &str| -> std::collections::BTreeSet<String> {
        // captura los {algo} del texto
        let mut out = std::collections::BTreeSet::new();
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'{' {
                if let Some(end) = s[i..].find('}') {
                    out.insert(s[i..i+end+1].to_string());
                    i += end + 1;
                    continue;
                }
            }
            i += 1;
        }
        out
    };
    for json in langs {
        let m: HashMap<String, String> = serde_json::from_str(json).unwrap();
        for (k, v_es) in &es {
            let ph_es = extract(v_es);
            if ph_es.is_empty() { continue; }
            if let Some(v) = m.get(k) {
                assert_eq!(extract(v), ph_es, "placeholders distintos en la clave {k}");
            }
        }
    }
}
```

- [ ] **Step 2: Run, confirm pass (deberían pasar)**

Run: `cargo test -p naygo-core i18n`
Expected: PASS — los 8 son copia de en.json (mismas claves y placeholders que en, que == es por parity). Si falla, hay un desajuste que arreglar antes de seguir.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/i18n/mod.rs
git commit -m "test(i18n): parity estricto de los 10 idiomas + consistencia de placeholders"
```

---

### Task 7: Selector con marca experimental

**Files:**
- Modify: `crates/ui-slint/src/main.rs` (donde se arma la lista de idiomas)

- [ ] **Step 1: Implementar (sin test unitario; verificación en vivo)**

READ dónde se construye `languages: Vec<SharedString>` (main.rs ~5342) y cómo el combo muestra cada idioma. La lista hoy son los códigos o nombres. Cambiar para que cada idioma muestre su nombre nativo (`c.t(&format!("lang.{code}"))`) + el sufijo experimental para los 4 CJK/hindi:
```rust
const EXPERIMENTAL: &[&str] = &["zh", "ja", "ko", "hi"];
// por cada code disponible:
let mut label = c.t(&format!("lang.{code}"));
if EXPERIMENTAL.contains(&code) {
    label.push_str(&c.t("lang.experimental_suffix"));
}
```
> Verifica cómo el combo mapea la etiqueta mostrada al `code` que se setea en settings (si muestra el nombre pero guarda el código, hay un mapeo label↔code que mantener). Adapta. Si el combo hoy muestra el código crudo, este cambio lo mejora a nombre nativo.

- [ ] **Step 2: Compilar + verificación en vivo**

Run: `cargo build -p naygo-ui-slint --bins`, luego `cargo run -p naygo-ui-slint --bin naygo`
Verificar: el selector de idioma lista los 10 con su nombre nativo; los 4 CJK/hindi llevan " (experimental)". Cambiar a un idioma muestra la UI traducida (en inglés todavía para los 8, hasta Fase 3).

- [ ] **Step 3: Commit**

```bash
git add crates/ui-slint/src/main.rs
git commit -m "feat(ui): selector de idioma con nombre nativo + marca experimental (CJK/hindi)"
```

---

## Fase 3 — Traducciones (idioma por idioma)

> Cada task de esta fase reemplaza el contenido en-copia de un idioma por la traducción real
> de las ~800 claves. Las CLAVES y los placeholders (`{drive}`, `{n}`, etc.) NO cambian —
> solo los VALORES. Tras cada idioma: `cargo test -p naygo-core i18n` (parity + placeholders
> verdes) confirma que no se rompió ninguna clave ni placeholder.

### Task 8-15: Traducir pt / fr / de / it / zh / ja / ko / hi

Una task por idioma (8 tasks). Para cada idioma `<L>`:

- [ ] **Step 1: Traducir `crates/core/src/i18n/<L>.json`**

Reemplazar cada VALOR (no la clave) por su traducción al idioma `<L>`, partiendo del valor de
es.json (sentido) y en.json (referencia). REGLAS:
- NO tocar las claves.
- Conservar los placeholders EXACTOS (`{drive}`, `{n}`, `{path}`, `{date}`, etc.).
- NO traducir `lang.*` (nombres nativos) ni `lang.experimental_suffix` (quedan igual en todos).
- Tono neutral, consistente. Para CJK: traducción razonable; los nombres propios (Naygo,
  ISGroth) quedan igual.
- Mantener el JSON válido (comillas escapadas, sin trailing comma).

- [ ] **Step 2: Validar**

Run: `/c/Python313/python -c "import json; json.load(open('crates/core/src/i18n/<L>.json', encoding='utf-8')); print('ok')"`
Run: `cargo test -p naygo-core i18n`
Expected: JSON válido + parity + placeholders verdes.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/i18n/<L>.json
git commit -m "feat(i18n): traducción al <idioma> (<L>)"
```

(Repetir para los 8: pt=Task8, fr=9, de=10, it=11, zh=12, ja=13, ko=14, hi=15.)

---

## Fase 4 — Cierre

### Task 16: Suite + clippy + THIRD-PARTY + verificación

- [ ] **Step 1:** `cargo test -p naygo-core` y `cargo test -p naygo-ui-slint` → verdes.
- [ ] **Step 2:** `cargo clippy -p naygo-core` y `cargo clippy -p naygo-ui-slint --bins` → sin warnings.
- [ ] **Step 3:** `cargo build -p naygo-ui-slint --bins` → compila.
- [ ] **Step 4:** Verificación en VM (Nicolás): cambiar a cada idioma; latinos completos; CJK/hindi renderizan (según fuente del SO) con su marca experimental; el preview de comprimidos y los mensajes de error salen traducidos.

---

## Resumen de fases
1. **Auditoría**: archive_tree con ArchiveLabels → preview.rs errores tipados+i18n → splash + barrido.
2. **Infraestructura**: 8 JSON (copia en) + embeber + lang.* + parity estricto + selector con marca experimental.
3. **Traducciones**: pt/fr/de/it/zh/ja/ko/hi (una task c/u).
4. **Cierre**: suite + clippy + VM.
