# Vista previa: modo de vista + resaltado de sintaxis + abrir en el sistema — diseño

> Spec de diseño. Fecha: 2026-06-21. Autor: Nicolás Groth / ISGroth.
> "Entrega B" del bloque de mejoras del preview (la "Entrega A" fueron 3 bugs ya corregidos).

## Objetivo

Tres mejoras del panel de Vista previa:

1. **Modo de vista configurable por extensión**: en Configuración → Previsualización, cambiar
   el campo de texto libre "Tratar como" por un combobox de **tipo de vista**
   (Automático / Texto / Imagen / Código) y, cuando es "Código", un segundo combobox de
   **lenguaje** (XML, JSON, HTML, CSS, JavaScript, C, C++, Java, Python, Rust, SQL, Bash,
   Markdown, YAML, TOML, INI).
2. **Resaltado de sintaxis** del preview de texto cuando el modo es Código, con **syntect**
   (set curado de gramáticas), corriendo en el worker; el resultado son líneas → segmentos
   coloreados que la UI pinta como texto real.
3. **Botón "abrir con el programa del sistema"** en la barra del PreviewPanel
   (ShellExecute / la capacidad de abrir archivos que Naygo ya tiene).

## Restricciones del proyecto (relevantes)

- Render por **software** de Slint: el `Text` pinta un solo color por elemento; el resaltado
  multicolor se logra con **un elemento Text por segmento** (no rasterizado).
- Límite **i16 (~32767px)** por coordenada de glifo en el render por software: ya mitigado con
  el tope de líneas (`TEXT_MAX_LINES=100`) y el recorte por línea (`TEXT_MAX_LINE_CHARS=1000` /
  `clip_long_lines`) en `core::preview`. El resaltado opera sobre el texto YA recortado → la
  defensa se mantiene, ningún glifo cae fuera de rango.
- El hilo de UI nunca hace I/O ni trabajo pesado: syntect corre en el **worker** de preview
  (que ya lee el archivo en background con debounce + cancelación).
- Español neutral, i18n triple (i18n.slint + es.json/en.json `slint.*` + i18n_keys.rs) para
  todo texto nuevo, sin reusar nombres de claves. Build con `CARGO_BUILD_JOBS=2`.

## Componente 1 — Modelo de datos (core)

`crates/core/src/preview.rs`. El `PreviewRule` actual es
`{ ext: String, enabled: bool, treat_as: Option<String> }`, donde `treat_as` es un alias de
extensión (texto libre). Se reemplaza `treat_as` por un modo de vista explícito:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeLang {
    Xml, Json, Html, Css, JavaScript, C, Cpp, Java, Python, Rust, Sql, Bash,
    Markdown, Yaml, Toml, Ini,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    Auto,                 // clasificación por extensión actual (lo de hoy)
    Text,                 // forzar texto plano (sin resaltado)
    Image,                // forzar imagen
    Code(CodeLang),       // forzar resaltado de código con esa gramática
}

pub struct PreviewRule {
    pub ext: String,
    pub enabled: bool,
    pub view: ViewMode,   // reemplaza treat_as
}
```

- `CodeLang` tiene helpers `as_str()` / `from_str()` (clave estable para serializar y para el
  nombre de gramática de syntect) y `all()` (para poblar el combobox).
- **Migración** (`treat_as` viejo → `ViewMode`): al deserializar un settings.json previo, un
  `treat_as: Some("xml")` se mapea a `ViewMode::Code(Xml)` si el alias es un lenguaje conocido,
  a `ViewMode::Image` si el alias es una extensión de imagen, a `ViewMode::Text` si es de texto,
  y a `ViewMode::Auto` en cualquier otro caso o si `treat_as` es `None`. Se implementa con un
  `#[serde(...)]` tolerante o una función `rule_from_legacy(ext, enabled, treat_as)`.
- `classify_rules` (la función que decide el `PreviewKind` de una ruta) pasa a respetar el
  `ViewMode`: `Auto` → comportamiento actual; `Text`/`Image`/`Code(_)` → fuerzan el tipo.
  `Code(lang)` devuelve un `PreviewKind::Text` con el lenguaje asociado (ver componente 2).

## Componente 2 — Resaltado (core, función pura + worker)

### Función pura en core

`crates/core/src/highlight.rs` (módulo nuevo). syntect vive aquí.

```rust
/// Un segmento de texto coloreado dentro de una línea.
pub struct HlSpan { pub text: String, pub color: (u8, u8, u8) }
/// Una línea = lista de segmentos.
pub struct HlLine { pub spans: Vec<HlSpan> }

/// Resalta `text` (YA recortado por truncate_text/clip_long_lines) como `lang`.
/// Devuelve una línea por cada `\n`. Si syntect falla o el lenguaje no se reconoce,
/// devuelve cada línea como un único span con color por defecto (degradación, nunca panic).
pub fn highlight(text: &str, lang: CodeLang) -> Vec<HlLine>;
```

- syntect: `SyntaxSet` + `ThemeSet` cargados una vez (lazy `OnceLock`). Set de gramáticas
  **curado** (~16 lenguajes), no el dump completo, para acotar peso. Tema de código embebido
  fijo (p. ej. `base16-ocean.dark` o `InspiredGitHub`) → colores independientes del tema de
  Naygo; el fondo del panel sigue siendo el del tema activo.
- Opera sobre el texto recortado (≤100 líneas, ≤1000 chars/línea) → barato y sin riesgo i16.
- Mapea cada `(Style, &str)` de syntect a `HlSpan { text, color: rgb }`.

### Worker (ui-slint)

`crates/ui-slint/src/preview.rs`. Cuando el `PreviewKind` resuelto trae un `CodeLang`, tras
leer y recortar el texto, el worker llama a `core::highlight::highlight(text, lang)` y mete el
resultado en el `Payload`. El payload de texto gana un campo opcional de líneas resaltadas.

## Componente 3 — Contrato hacia la UI (PreviewVm)

`crates/ui-slint/ui/types.slint` y el armado del VM en `src/main.rs`/`preview.rs`:

```slint
struct HlSpanVm { text: string, color: color }
struct HlLineVm { spans: [HlSpanVm] }

// PreviewVm gana:
//   highlighted: bool          // true = pintar hl-lines; false = el `text` plano de hoy
//   hl-lines: [HlLineVm]
```

- Modo texto plano: `highlighted=false`, se usa el `text` actual (un solo color), con el
  `word-wrap` que ya tiene.
- Modo código: `highlighted=true`, `hl-lines` poblado.

## Componente 4 — Render (preview-panel.slint)

Dentro del `ScrollView` del `mode 1` (texto):

```slint
if root.view.highlighted: VerticalLayout {
    for line in root.view.hl-lines: HorizontalLayout {
        for seg in line.spans: Text {
            text: seg.text;
            color: seg.color;
            font-family: "Consolas";
            // sin wrap: cada línea ya viene recortada por core; el ScrollView da scroll.
        }
    }
}
if !root.view.highlighted: Text { /* el Text único de hoy, con word-wrap */ }
```

- Cada línea es un `HorizontalLayout` de `Text` por segmento → texto real, nítido, seleccionable.
- Las líneas ya vienen recortadas desde core → ningún glifo cae fuera de i16.
- Rendimiento: ≤100 líneas × pocos segmentos = cientos de elementos como máximo, aceptable.

## Componente 5 — Configuración (config-window.slint, sección Previsualización)

Cada fila de regla pasa de `[ext] [toggle] [TextInput treat_as]` a:
`[ext] [toggle Vista previa] [ComboBox ViewMode] [ComboBox CodeLang — visible solo si ViewMode==Código]`.

- ComboBox 1 (ViewMode): "Automático" / "Ver como texto" / "Ver como imagen" / "Ver como código".
- ComboBox 2 (CodeLang): solo visible/habilitado cuando el 1 está en "Ver como código"; lista
  los ~16 lenguajes. Si se oculta, no descuadra la fila (placeholder o colapsa).
- "Agregar una extensión": la fila de alta usa los mismos combobox.
- Callbacks nuevos hacia el controlador: setear el `ViewMode`/`CodeLang` de una regla por ext;
  el `ConfigCtrl` persiste igual que hoy.
- Migración visual: las reglas viejas se muestran ya convertidas (Auto por defecto).

## Componente 6 — Botón "abrir con el sistema"

`preview-panel.slint`: un botón (ícono Path, sin glifo de fuente) en la barra de título del
panel, visible cuando hay un archivo en preview (`view.mode != 0`). Callback `open-in-system()`
→ propagado a `app-window.slint` → `main.rs` → la función que ya hace `ShellExecute`/abrir
archivo con el programa por defecto, sobre la ruta del archivo previsualizado. Tooltip i18n
"Abrir con el programa predeterminado".

Requiere que el `PreviewVm` (o un property del panel) exponga la **ruta** del archivo
previsualizado para que el callback sepa qué abrir.

## Dependencias

`crates/core/Cargo.toml`: `syntect` con features acotadas (sin las que arrastran dumps
innecesarios; idealmente `default-features = false` + lo mínimo para parsear + tema). Verificar
en el plan qué features dan el set curado con el menor peso. Todas las licencias de syntect y su
árbol son permisivas (MIT) — se sumarán al `THIRD-PARTY-NOTICES.md` (regenerar con cargo license).

## Manejo de errores / casos borde

- syntect falla, gramática ausente, o texto no-UTF8 → `highlight` degrada a líneas de un span
  con color por defecto. Nunca panic.
- Archivo binario forzado a "código" → el lossy + recorte ya lo manejan; sale texto raro, sin crash.
- Sin archivo seleccionado (`mode 0`) → el botón "abrir" se oculta.
- Archivo grande → el tope de líneas/recorte ya acota lo que se resalta (solo lo visible del preview).

## Pruebas

- `core::highlight`: tests puros — dado un fragmento conocido + `CodeLang`, devuelve N líneas;
  respeta el tope de líneas; degrada sin panic con lenguaje/encoding inválido; un keyword/string
  conocido sale en un span separado.
- `core::preview`: migración `treat_as`→`ViewMode` (varios casos); `classify_rules` respeta el
  `ViewMode` forzado.
- UI/worker/config: verificación de compilación + visual de Nicolás en la VM.
- Gate: `cargo fmt`, `cargo test` (3 crates), `cargo clippy --workspace --all-targets -- -D warnings`,
  barrido de voseo, `graphify update .`.

## Fuera de alcance

- Tema de código mapeado al tema de Naygo (se usa un tema de código fijo embebido).
- Resaltado en el preview de PDF (el texto extraído de PDF sigue como texto plano).
- Edición en el preview (Naygo no edita; el botón abre el editor del sistema).
- Selección/copia desde el preview resaltado más allá de lo que Slint ofrece por defecto.
- Números de línea (se puede sumar después si se quiere).
