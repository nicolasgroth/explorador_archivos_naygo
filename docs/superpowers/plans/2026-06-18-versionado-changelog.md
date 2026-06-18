# Versionado, CHANGELOG, "Novedades" e instalador in-place — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dar a Naygo versionado por commits convencionales (`bump.ps1`), un `CHANGELOG.md` como historia única, una sección "Novedades" en el "Acerca de" alimentada del CHANGELOG embebido, y un instalador que actualiza in-place sin perder configuración.

**Architecture:** La versión sigue viviendo solo en `Cargo.toml` raíz. Un parser puro en `naygo-core` (`changelog.rs`) extrae las notas de una versión; `ui-slint` embebe el CHANGELOG con `include_str!` y lo muestra en el "Acerca de". `bump.ps1` (PowerShell) infiere el nivel SemVer de los commits y publica (Cargo + CHANGELOG + tag, push opt-in). Dos líneas en `naygo.iss` cierran la app durante el update.

**Tech Stack:** Rust workspace (naygo-core / naygo-platform / naygo-ui-slint), Slint 1.16 (render software), PowerShell, Inno Setup. Build SIEMPRE con `CARGO_BUILD_JOBS=2` (deps SVG/PDF crashean i-slint-compiler con alta paralelización). Gate por tarea de código: `cargo fmt --all` + `cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform` + `cargo clippy --workspace --all-targets -- -D warnings`.

**Spec:** `docs/superpowers/specs/2026-06-18-versionado-changelog-design.md`

**Convenciones del repo (recordatorio):**
- Español neutral SIN voseo en todo (código, comentarios, docs, UI). 
- Header en cada archivo nuevo: `// Naygo — <descr>` + `// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.`
- i18n triple: `i18n.slint` (`in property <string> kebab-case`) + `es.json`/`en.json` (`"slint.xxx.yyy"`) + `i18n_keys.rs` (`tr.set_xxx(c.t("slint.xxx.yyy").into())`).
- NO commitear: `CLAUDE.md`, `assets/icons/otros/`, `graphify-out/`.
- NO hacer `git push` (lo hace Nicolás). Los commits de cada tarea quedan locales.
- Mensajes de commit terminan con `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Tras tocar código: `graphify update .` para refrescar el grafo (sin costo API).

---

## File Structure

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `CHANGELOG.md` (raíz) | Historia única de cambios (Keep a Changelog, español). | Crear |
| `crates/core/src/changelog.rs` | Parser puro: extraer notas de una versión. + tests. | Crear |
| `crates/core/src/lib.rs` | Exponer `pub mod changelog;`. | Modificar |
| `scripts/bump.ps1` | Inferir nivel SemVer de commits y publicar versión. | Crear |
| `crates/ui-slint/ui/types.slint` | Struct `NewsSection { category, items }`. | Modificar |
| `crates/ui-slint/ui/config-window.slint` | Propiedad `release-notes` + UI "Novedades" en Acerca de. | Modificar |
| `crates/ui-slint/ui/i18n.slint` | Claves `about-news-title`, `about-no-notes`. | Modificar |
| `crates/core/src/i18n/es.json`, `en.json` | Traducciones de esas claves. | Modificar |
| `crates/ui-slint/src/i18n_keys.rs` | Setters de esas claves. | Modificar |
| `crates/ui-slint/src/main.rs` | Embeber CHANGELOG, parsear, `set_release_notes`. | Modificar |
| `installer/naygo.iss` | `CloseApplications=yes` + `RestartApplications=no`. | Modificar |

**Orden de tareas:** primero el parser en core (testeable, base de todo), luego el CHANGELOG real, luego la UI de Novedades, luego el instalador, luego `bump.ps1`, y al final el tag v0.1.0 (manual, lo hace Nicolás).

---

## Task 1: Parser de CHANGELOG en core (`changelog.rs`)

Lógica pura, sin dependencias nuevas. Es el corazón testeable del bloque.

**Files:**
- Create: `crates/core/src/changelog.rs`
- Modify: `crates/core/src/lib.rs` (agregar `pub mod changelog;`)

- [ ] **Step 1: Crear el archivo con tipos + función vacía (que compile)**

Crear `crates/core/src/changelog.rs` con este contenido inicial:

```rust
// Naygo — parser del CHANGELOG: extrae las notas de una versión concreta.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Parser mínimo de un CHANGELOG con formato "Keep a Changelog":
//! encabezados de versión `## [X.Y.Z] — fecha`, subsecciones `### Categoría`
//! y viñetas `- …`. Sin dependencias: parseo línea a línea. Tolerante a
//! formatos imperfectos (nunca hace panic; ante algo inesperado devuelve
//! `None` o secciones vacías). Lo consume la UI para la sección "Novedades".

/// Una subsección de notas (una categoría con sus viñetas).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteSection {
    /// Nombre de la categoría tal cual aparece tras `### ` (p. ej. "Añadido").
    pub category: String,
    /// Viñetas de la categoría, sin el "- " inicial.
    pub items: Vec<String>,
}

/// Notas de una versión del CHANGELOG.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseNotes {
    /// Versión tal cual aparece entre corchetes en `## [..]`.
    pub version: String,
    /// Fecha si el encabezado la incluye tras un guion (p. ej. "2026-06-18").
    pub date: Option<String>,
    /// Subsecciones en el orden en que aparecen.
    pub sections: Vec<NoteSection>,
}

/// Extrae del texto de un CHANGELOG el bloque de la versión `version`.
///
/// Busca un encabezado `## [<version>]` (la coincidencia es por el contenido
/// EXACTO entre corchetes). Devuelve `None` si no existe tal bloque. El bloque
/// termina en el siguiente `## ` o al final del texto.
pub fn release_notes(changelog: &str, version: &str) -> Option<ReleaseNotes> {
    let mut lines = changelog.lines();
    // Encontrar el encabezado de la versión pedida.
    let header = lines.by_ref().find(|l| {
        version_in_header(l).is_some_and(|v| v == version)
    })?;
    let date = date_in_header(header);

    let mut sections: Vec<NoteSection> = Vec::new();
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("## ") {
            break; // empezó la siguiente versión
        }
        if let Some(cat) = trimmed.strip_prefix("### ") {
            sections.push(NoteSection {
                category: cat.trim().to_string(),
                items: Vec::new(),
            });
        } else if let Some(item) = trimmed.strip_prefix("- ") {
            if let Some(last) = sections.last_mut() {
                last.items.push(item.trim().to_string());
            }
            // Una viñeta antes de cualquier `### ` se ignora (formato raro).
        }
    }

    Some(ReleaseNotes {
        version: version.to_string(),
        date,
        sections,
    })
}

/// Si la línea es un encabezado `## [algo] …`, devuelve `algo` (lo de dentro de
/// los corchetes). Si no, `None`.
fn version_in_header(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("## ")?;
    let rest = rest.trim_start();
    let inner = rest.strip_prefix('[')?;
    let end = inner.find(']')?;
    Some(inner[..end].trim())
}

/// Extrae la fecha de un encabezado de versión, si viene tras un guion
/// (acepta "—" o "-"). Devuelve el texto tras el guion, recortado.
fn date_in_header(line: &str) -> Option<String> {
    // Tomar lo que viene después del `]`.
    let after = line.split_once(']')?.1.trim();
    // Quitar un guion inicial (em-dash o normal) y espacios.
    let after = after
        .trim_start_matches('—')
        .trim_start_matches('-')
        .trim();
    if after.is_empty() {
        None
    } else {
        Some(after.to_string())
    }
}
```

- [ ] **Step 2: Exponer el módulo en lib.rs**

En `crates/core/src/lib.rs`, agregar la línea en orden alfabético, entre `pub mod cancel;` y `pub mod cli;`:

```rust
pub mod cancel;
pub mod changelog;
pub mod cli;
```

- [ ] **Step 3: Agregar los tests al final de `changelog.rs`**

Pegar al final de `crates/core/src/changelog.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Changelog

## [Sin publicar]

## [0.2.0] — 2026-07-01
### Añadido
- Vista profunda recursiva.
- Copiar rutas absolutas.
### Corregido
- Fuga de z-order en el encabezado.

## [0.1.0] — 2026-06-18
### Añadido
- Navegación tipo Commander.
";

    #[test]
    fn extrae_la_version_pedida_e_ignora_otras() {
        let n = release_notes(SAMPLE, "0.2.0").expect("debe encontrar 0.2.0");
        assert_eq!(n.version, "0.2.0");
        assert_eq!(n.date.as_deref(), Some("2026-07-01"));
        assert_eq!(n.sections.len(), 2);
        assert_eq!(n.sections[0].category, "Añadido");
        assert_eq!(
            n.sections[0].items,
            vec![
                "Vista profunda recursiva.".to_string(),
                "Copiar rutas absolutas.".to_string()
            ]
        );
        assert_eq!(n.sections[1].category, "Corregido");
        assert_eq!(n.sections[1].items, vec!["Fuga de z-order en el encabezado.".to_string()]);
    }

    #[test]
    fn version_inexistente_devuelve_none() {
        assert!(release_notes(SAMPLE, "9.9.9").is_none());
    }

    #[test]
    fn changelog_vacio_o_sin_encabezados_devuelve_none() {
        assert!(release_notes("", "0.1.0").is_none());
        assert!(release_notes("texto suelto sin secciones", "0.1.0").is_none());
    }

    #[test]
    fn bloque_sin_vinetas_da_secciones_vacias_sin_panic() {
        let cl = "## [1.0.0]\n### Añadido\n";
        let n = release_notes(cl, "1.0.0").expect("encuentra 1.0.0");
        assert_eq!(n.date, None);
        assert_eq!(n.sections.len(), 1);
        assert!(n.sections[0].items.is_empty());
    }

    #[test]
    fn no_se_mezclan_vinetas_de_la_version_siguiente() {
        // 0.1.0 es el último bloque: solo su categoría/viñeta, nada de 0.2.0.
        let n = release_notes(SAMPLE, "0.1.0").expect("encuentra 0.1.0");
        assert_eq!(n.sections.len(), 1);
        assert_eq!(n.sections[0].items, vec!["Navegación tipo Commander.".to_string()]);
    }
}
```

- [ ] **Step 4: Correr los tests y verificar que pasan**

```
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core changelog
```
Esperado: PASS (5 tests del módulo `changelog`).

- [ ] **Step 5: Gate + commit**

```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all; cargo clippy -p naygo-core --all-targets -- -D warnings
```
Esperado: sin warnings. Luego:

```bash
git add crates/core/src/changelog.rs crates/core/src/lib.rs
git commit -F - <<'EOF'
feat(core): parser de CHANGELOG para la seccion "Novedades"

changelog::release_notes(texto, version) extrae las notas de una version
(categorias + vinetas) de un CHANGELOG estilo Keep a Changelog. Parseo puro
linea a linea, sin dependencias, tolerante a formatos imperfectos. Con tests.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 2: Crear `CHANGELOG.md` con la entrada 0.1.0

**Files:**
- Create: `CHANGELOG.md` (raíz del repo)

- [ ] **Step 1: Crear `CHANGELOG.md`**

Crear el archivo en la raíz con este contenido (resume lo ya construido; categorías en español):

```markdown
# Changelog

Todas las novedades de Naygo se documentan en este archivo.

El formato se basa en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto sigue [Versionado Semántico](https://semver.org/lang/es/).

## [Sin publicar]

## [0.1.0] — 2026-06-18
### Añadido
- Navegación de archivos tipo Commander: paneles dinámicos acoplables, dual-pane,
  ir atrás/adelante (incluidos los botones laterales del mouse).
- Árbol de carpetas con expansión incremental, revelado hasta la carpeta activa y
  navegación por teclado.
- Listado por streaming incremental y cancelable; el filesystem hostil (red caída,
  permisos, rutas que desaparecen) no tumba la app.
- Columnas dinámicas estilo planilla: ordenar, filtrar por tipo de columna y
  reordenar arrastrando.
- Operaciones de archivo entre paneles (copiar, mover, eliminar) con cola opcional,
  progreso y cancelación.
- Renombrado en línea y en cadena, y ventana de renombrado por lotes.
- Búsqueda recursiva por nombre en la carpeta y sus subcarpetas.
- Previsualización liviana: imágenes, SVG (rasterizado), PDF (texto y metadatos),
  texto/código y listado de contenido de archivos ZIP.
- Cálculo de tamaño de carpetas bajo demanda.
- Barra de unidades de disco con espacio libre/total y porcentaje usado; ícono
  propio para unidades USB y expulsión segura desde un menú.
- Detección de discos duros externos USB como extraíbles (por tipo de bus).
- Integración con Windows: menú contextual del shell, "Abrir con", watcher de
  carpeta, detección de dispositivos, arrastrar y soltar, ícono de bandeja y
  arranque opcional con el sistema.
- Internacionalización (español e inglés) y temas intercambiables en caliente, con
  galería de selección y packs de usuario.
- Configuración como ventana nativa: apariencia, atajos, previsualización, plantilla
  de tabla, opciones avanzadas y sección "Acerca de".
- Distribución como ejecutable portable e instalador (Inno Setup) con CRT estático.
```

- [ ] **Step 2: Verificar que el archivo es UTF-8 y se lee bien**

```
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core changelog
```
Esperado: PASS (los tests siguen verdes; este paso solo confirma que no rompimos nada). El consumo real del archivo se valida en la Task 4.

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -F - <<'EOF'
docs: CHANGELOG.md inicial con la entrada 0.1.0

Historia unica de cambios (Keep a Changelog, espanol). La entrada 0.1.0 resume
lo construido hasta hoy. La seccion "Sin publicar" acumula lo nuevo; bump.ps1 la
movera a la version correspondiente.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 3: i18n de las claves de "Novedades"

Tres capas. Las claves: `about-news-title` ("Novedades de esta versión") y `about-no-notes` ("Sin notas para esta versión").

**Files:**
- Modify: `crates/ui-slint/ui/i18n.slint`
- Modify: `crates/core/src/i18n/es.json`
- Modify: `crates/core/src/i18n/en.json`
- Modify: `crates/ui-slint/src/i18n_keys.rs`

- [ ] **Step 1: Defaults en `i18n.slint`**

Buscar la línea `in property <string> about-repo:` (en el bloque de claves `about-*`) y agregar justo después:

```slint
    in property <string> about-news-title: "Novedades de esta versión";
    in property <string> about-no-notes: "Sin notas para esta versión.";
```

- [ ] **Step 2: Claves en `es.json`**

Buscar `"slint.about.repo"` y agregar después:

```json
  "slint.about.news_title": "Novedades de esta versión",
  "slint.about.no_notes": "Sin notas para esta versión.",
```

- [ ] **Step 3: Claves en `en.json`**

Buscar `"slint.about.repo"` y agregar después:

```json
  "slint.about.news_title": "What's new in this version",
  "slint.about.no_notes": "No notes for this version.",
```

- [ ] **Step 4: Setters en `i18n_keys.rs`**

Buscar `tr.set_about_repo(` y agregar después:

```rust
    tr.set_about_news_title(c.t("slint.about.news_title").into());
    tr.set_about_no_notes(c.t("slint.about.no_notes").into());
```

- [ ] **Step 5: Verificar que compila (las claves existen y casan)**

```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
```
Esperado: compila. (Si `set_about_news_title`/`set_about_no_notes` no existieran, fallaría; existen porque Slint genera el setter de cada `in property`.)

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/ui/i18n.slint crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/src/i18n_keys.rs
git commit -F - <<'EOF'
feat(i18n): claves para la seccion "Novedades" del Acerca de

about-news-title y about-no-notes en las tres capas (i18n.slint, es/en.json,
i18n_keys.rs).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 4: Sección "Novedades" en el Acerca de (Slint + main.rs)

Define el struct del modelo, la propiedad, la UI y el wiring que parsea el CHANGELOG embebido.

**Files:**
- Modify: `crates/ui-slint/ui/types.slint` (struct `NewsSection`)
- Modify: `crates/ui-slint/ui/config-window.slint` (propiedad + UI)
- Modify: `crates/ui-slint/src/main.rs` (embeber + parsear + set)

- [ ] **Step 1: Struct `NewsSection` en `types.slint`**

Agregar al final de `crates/ui-slint/ui/types.slint`:

```slint
// Una subsección de "Novedades" del Acerca de: una categoría y sus viñetas.
export struct NewsSection {
    category: string,
    items: [string],
}
```

- [ ] **Step 2: Importar el struct y declarar la propiedad en `config-window.slint`**

En `crates/ui-slint/ui/config-window.slint`, localizar el bloque de `import { … } from "types.slint";` (cerca del inicio). Si `types.slint` ya está importado, agregar `NewsSection` a la lista de símbolos importados; si no hay import de `types.slint`, agregar una línea:

```slint
import { NewsSection } from "types.slint";
```

Luego, junto a las otras `in property` de la ventana de config (cerca de `in property <[PreviewRuleVm]> preview-rules;`, ~línea 167), agregar:

```slint
    in property <[NewsSection]> release-notes;
```

- [ ] **Step 3: UI de "Novedades" dentro del Acerca de (cat 8)**

En `config-window.slint`, dentro del `if root.cat == 8: VerticalLayout { … }`, DESPUÉS del bloque del botón del repo (el `HorizontalLayout { alignment: center; Rectangle { … repo-touch … } }` que termina cerca de la línea 799) y ANTES de cerrar ese `VerticalLayout`, insertar:

```slint
                                // Separador y sección "Novedades de esta versión".
                                Rectangle { height: 8px; }
                                Text {
                                    text: Tr.about-news-title;
                                    color: Theme.text;
                                    font-weight: 700;
                                    horizontal-alignment: center;
                                }
                                // Si no hay notas para esta versión, una línea discreta.
                                if root.release-notes.length == 0: Text {
                                    text: Tr.about-no-notes;
                                    color: Theme.text-dim;
                                    horizontal-alignment: center;
                                }
                                // Una subsección por categoría, con sus viñetas.
                                for sec in root.release-notes: VerticalLayout {
                                    spacing: 2px;
                                    Text {
                                        text: sec.category;
                                        color: Theme.accent;
                                        font-weight: 700;
                                    }
                                    for it in sec.items: Text {
                                        text: "• " + it;
                                        color: Theme.text-dim;
                                        wrap: word-wrap;
                                    }
                                }
```

- [ ] **Step 4: Embeber el CHANGELOG y rellenar el modelo en `main.rs`**

En `crates/ui-slint/src/main.rs`:

(a) Cerca del inicio del archivo (junto a otros `const`/`use`), agregar la constante:

```rust
/// CHANGELOG embebido en build time: fuente de la sección "Novedades" del Acerca de.
/// Ruta relativa desde este archivo (crates/ui-slint/src/) hasta la raíz del repo.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");
```

(b) Localizar la línea `cfg.set_app_version(env!("CARGO_PKG_VERSION").into());` (~1480). Justo DESPUÉS, agregar:

```rust
        // Sección "Novedades": parsear el CHANGELOG embebido y volcar las notas de la
        // versión actual. Se setea una sola vez (no cambia en runtime).
        {
            let notes = naygo_core::changelog::release_notes(CHANGELOG, env!("CARGO_PKG_VERSION"));
            let sections: Vec<NewsSection> = notes
                .map(|n| {
                    n.sections
                        .into_iter()
                        .map(|s| NewsSection {
                            category: s.category.into(),
                            items: ModelRc::new(VecModel::from(
                                s.items
                                    .into_iter()
                                    .map(SharedString::from)
                                    .collect::<Vec<_>>(),
                            )),
                        })
                        .collect()
                })
                .unwrap_or_default();
            cfg.set_release_notes(ModelRc::new(VecModel::from(sections)));
        }
```

(c) Asegurar que `NewsSection` esté en el `use` de tipos generados por Slint. Buscar dónde se importan los tipos del .slint (p. ej. una línea con `RowData, ColumnVm, …` o `use crate::...`/`slint::include_modules!`). Si los tipos se usan sin import explícito (vía `include_modules!` que los trae al scope del módulo), no hace falta nada. Si hay una lista explícita de tipos importados que incluye `RowData`, agregar `NewsSection` a esa lista.

> Nota: `ModelRc`, `VecModel`, `SharedString` ya están importados en main.rs
> (`use slint::{Model, ModelRc, SharedPixelBuffer, SharedString, TimerMode, VecModel};`).

- [ ] **Step 5: Compilar**

```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
```
Esperado: compila sin errores. Si falla por `NewsSection` no encontrado en main.rs, aplicar el import del Step 4(c) según cómo el archivo traiga los demás structs (mismo patrón que `RowData`).

- [ ] **Step 6: Gate completo**

```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform; cargo clippy --workspace --all-targets -- -D warnings
```
Esperado: tests PASS, clippy sin warnings.

- [ ] **Step 7: Verificación visual (Nicolás)**

Compilar release y abrir el "Acerca de" para confirmar que "Novedades" se ve bien (título + categorías + viñetas de la versión actual). Este paso es visto bueno visual de Nicolás; no marcar la tarea como cerrada del todo hasta su OK.

```
$env:CARGO_BUILD_JOBS = "2"; cargo build --release -p naygo-ui-slint
```

- [ ] **Step 8: Commit**

```bash
git add crates/ui-slint/ui/types.slint crates/ui-slint/ui/config-window.slint crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(about): seccion "Novedades de esta version" desde el CHANGELOG embebido

El Acerca de muestra las vinetas de la version instalada, agrupadas por
categoria. El CHANGELOG.md se embebe con include_str! y se parsea con
core::changelog::release_notes; si no hay notas, una linea discreta.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 5: Instalador actualizable in-place

**Files:**
- Modify: `installer/naygo.iss`

- [ ] **Step 1: Agregar cierre de la app en `[Setup]`**

En `installer/naygo.iss`, dentro de la sección `[Setup]` (tras la línea `UninstallDisplayIcon={app}\{#MyAppExe}` o en cualquier punto de `[Setup]`), agregar:

```
; Si Naygo está corriendo durante un update, ofrecer cerrarlo antes de reemplazar
; el .exe (evita el error "archivo en uso"). No reiniciar la app automáticamente.
CloseApplications=yes
RestartApplications=no
```

- [ ] **Step 2: Verificar que el .iss compila (si hay Inno instalado)**

```
$env:CARGO_BUILD_JOBS = "2"; powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```
Esperado: genera `dist\Naygo-0.1.0-portable.zip` y, si Inno está instalado, `dist\Naygo-0.1.0-setup.exe` sin errores de ISCC. Si Inno no está, el script avisa y genera solo el portable (no es bloqueante para esta tarea: el cambio del .iss es sintácticamente trivial).

- [ ] **Step 3: Commit**

```bash
git add installer/naygo.iss
git commit -F - <<'EOF'
build(installer): cerrar Naygo en uso durante el update (in-place)

CloseApplications=yes + RestartApplications=no: si la app esta abierta al
instalar una version nueva, Inno la cierra antes de reemplazar el .exe. El
resto del comportamiento de update (mismo AppId, config en %APPDATA% intacta)
ya estaba.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 6: Script `bump.ps1`

PowerShell. Funciones puras separadas para poder probarlas; el flujo git las usa.

**Files:**
- Create: `scripts/bump.ps1`

- [ ] **Step 1: Crear `scripts/bump.ps1`**

Crear el archivo con este contenido:

```powershell
# Naygo — versionado: infiere el nivel SemVer de los commits y publica una version.
# Copyright (c) 2026 Nicolas Groth / ISGroth. MIT License.
#
# Uso:
#   scripts\bump.ps1                 # infiere patch/minor/major de los commits
#   scripts\bump.ps1 -Level minor    # fuerza el nivel
#   scripts\bump.ps1 -DryRun         # muestra que haria, sin tocar nada
#   scripts\bump.ps1 -Push           # tras commit+tag, hace git push y push --tags
#
# Reglas (Conventional Commits) desde el ultimo tag vX.Y.Z:
#   feat: -> minor   fix: -> patch   "BREAKING CHANGE" o "!" -> major
#   si hay commits pero ninguno aporta nivel -> patch (fallback)
#   si no hay commits nuevos y ya habia tag -> no versiona (avisa)
# NO hace push salvo -Push. Pensado para correrlo Nicolas; Claude no lo ejecuta.

[CmdletBinding()]
param(
    [ValidateSet('patch', 'minor', 'major')]
    [string]$Level,
    [switch]$DryRun,
    [switch]$Push
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$cargoPath = Join-Path $repo "Cargo.toml"
$changelogPath = Join-Path $repo "CHANGELOG.md"

# --- Funciones puras (testeables sin git) ---

# Decide el nivel a partir de los "subjects" y "bodies" de los commits.
# Devuelve 'major'|'minor'|'patch'|$null (null = ningun commit aporto nivel).
function Get-BumpLevel {
    param([string[]]$Subjects, [string[]]$Bodies)
    $level = $null
    for ($i = 0; $i -lt $Subjects.Count; $i++) {
        $s = $Subjects[$i]
        $b = if ($i -lt $Bodies.Count) { $Bodies[$i] } else { "" }
        $isBreaking = ($s -match '^[a-z]+(\(.+\))?!:') -or ($b -match 'BREAKING CHANGE')
        if ($isBreaking) { return 'major' }
        if ($s -match '^feat(\(.+\))?:') { if ($level -ne 'minor') { $level = 'minor' } }
        elseif ($s -match '^fix(\(.+\))?:') { if ($null -eq $level) { $level = 'patch' } }
    }
    return $level
}

# Sube una version "X.Y.Z" segun el nivel. Devuelve la nueva "X.Y.Z".
function Step-Version {
    param([string]$Current, [string]$BumpLevel)
    if ($Current -notmatch '^(\d+)\.(\d+)\.(\d+)$') {
        throw "Version actual con formato inesperado: '$Current'"
    }
    $maj = [int]$Matches[1]; $min = [int]$Matches[2]; $pat = [int]$Matches[3]
    switch ($BumpLevel) {
        'major' { $maj++; $min = 0; $pat = 0 }
        'minor' { $min++; $pat = 0 }
        'patch' { $pat++ }
        default { throw "Nivel invalido: '$BumpLevel'" }
    }
    return "$maj.$min.$pat"
}

# --- Flujo principal ---

# 0) Working tree limpio (no versionar a medias).
$dirty = git -C $repo status --porcelain
if ($dirty -and -not $DryRun) {
    throw "El working tree tiene cambios sin commitear. Haz commit o stash antes de versionar."
}

# 1) Version actual desde Cargo.toml (la linea de [workspace.package]).
$cargoRaw = Get-Content $cargoPath -Raw
if ($cargoRaw -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
    throw "No pude leer la version de Cargo.toml."
}
$current = $Matches[1]

# 2) Ultimo tag vX.Y.Z (si hay).
$lastTag = (git -C $repo tag --list "v*" --sort=-v:refname | Select-Object -First 1)
$hasTag = [bool]$lastTag

# 3) Determinar el nivel.
if ($Level) {
    $bump = $Level
} else {
    if ($hasTag) {
        $range = "$lastTag..HEAD"
    } else {
        $range = "HEAD"  # primer release: considerar todos los commits
    }
    $subjects = @(git -C $repo log $range --format="%s")
    $bodies = @(git -C $repo log $range --format="%b")
    if ($subjects.Count -eq 0 -and $hasTag) {
        Write-Host "Nada que versionar: no hay commits nuevos desde $lastTag."
        return
    }
    $inferred = Get-BumpLevel -Subjects $subjects -Bodies $bodies
    if ($null -eq $inferred) { $inferred = 'patch' }  # fallback
    $bump = $inferred
}

# 4) Nueva version.
$new = Step-Version -Current $current -BumpLevel $bump
$newTag = "v$new"
if (git -C $repo tag --list $newTag) {
    throw "El tag $newTag ya existe."
}
$today = (Get-Date -Format "yyyy-MM-dd")

Write-Host "Version actual : $current"
Write-Host "Nivel          : $bump"
Write-Host "Nueva version  : $new   (tag $newTag, fecha $today)"

if ($DryRun) {
    Write-Host "[DryRun] No se modifico nada."
    return
}

# 5) Reescribir version en Cargo.toml (solo la primera ocurrencia, la del workspace).
$cargoNew = [regex]::Replace(
    $cargoRaw,
    '(?m)^(\s*version\s*=\s*")[^"]+(")',
    "`${1}$new`${2}",
    1
)
Set-Content -Path $cargoPath -Value $cargoNew -Encoding utf8 -NoNewline

# 6) Mover "## [Sin publicar]" -> "## [X.Y.Z] - fecha" y crear un "Sin publicar" vacio.
$cl = Get-Content $changelogPath -Raw
if ($cl -notmatch '## \[Sin publicar\]') {
    throw "No encontre '## [Sin publicar]' en el CHANGELOG."
}
$replacement = "## [Sin publicar]`r`n`r`n## [$new] - $today"
$cl = $cl -replace '## \[Sin publicar\]', $replacement
Set-Content -Path $changelogPath -Value $cl -Encoding utf8

# 7) Refrescar el lock (los crates naygo-* heredan la version del workspace).
try {
    cargo update --workspace --offline 2>$null
} catch {
    Write-Warning "cargo update fallo (offline); el lock se regenerara al compilar."
}

# 8) Commit + tag.
git -C $repo add Cargo.toml Cargo.lock CHANGELOG.md
git -C $repo commit -m "chore(release): $newTag"
git -C $repo tag $newTag
Write-Host "Commit y tag $newTag creados."

# 9) Push solo con -Push.
if ($Push) {
    git -C $repo push
    git -C $repo push --tags
    Write-Host "Push hecho (commit + tags)."
} else {
    Write-Host "Recuerda publicar:  git push && git push --tags"
}
```

- [ ] **Step 2: Probar las funciones puras en seco**

Correr este bloque (carga las funciones del script y verifica la inferencia sin tocar git):

```powershell
. scripts\bump.ps1 -DryRun  # carga funciones; -DryRun aborta sin cambios tras imprimir

# Verificaciones manuales de la logica pura:
Get-BumpLevel -Subjects @("feat: x","docs: y") -Bodies @("","")   # -> minor
Get-BumpLevel -Subjects @("fix: x") -Bodies @("")                  # -> patch
Get-BumpLevel -Subjects @("feat!: x") -Bodies @("")               # -> major
Get-BumpLevel -Subjects @("chore: x") -Bodies @("")              # -> (vacio/null)
Step-Version -Current "0.1.0" -BumpLevel "minor"                  # -> 0.2.0
Step-Version -Current "0.1.9" -BumpLevel "patch"                  # -> 0.1.10
```
Esperado: los valores entre comentarios. (Si `. scripts\bump.ps1` ejecuta el flujo en vez de solo cargar, usar `-DryRun` como arriba: imprime y retorna antes de modificar nada.)

- [ ] **Step 3: Probar el flujo completo en seco**

```powershell
scripts\bump.ps1 -DryRun
```
Esperado: imprime "Version actual / Nivel / Nueva version" y "[DryRun] No se modifico nada." Sin cambios en `git status`.

- [ ] **Step 4: Commit**

```bash
git add scripts/bump.ps1
git commit -F - <<'EOF'
build(release): script bump.ps1 (versionado por commits convencionales)

Infiere patch/minor/major desde el ultimo tag (feat->minor, fix->patch,
BREAKING/!->major; fallback patch), actualiza Cargo.toml, mueve "Sin publicar"
del CHANGELOG a la version con fecha, commitea y crea tag vX.Y.Z. Flags -Level,
-DryRun y -Push (opt-in; no empuja por defecto).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 7: Tag inicial v0.1.0 (acción de Nicolás)

Esto crea el punto de partida del versionado. NO lo hace Claude (implica un tag que normalmente acompaña un push). Queda documentado para que Nicolás lo ejecute.

- [ ] **Step 1: Crear el tag sobre el estado actual**

Cuando todo el bloque esté commiteado y Nicolás esté listo:

```bash
git tag -a v0.1.0 -m "Naygo 0.1.0"
git push && git push --tags
```

- [ ] **Step 2: Verificar**

```bash
git describe --tags --abbrev=0   # -> v0.1.0
scripts\bump.ps1 -DryRun         # ahora calcula el siguiente nivel desde v0.1.0
```

---

## Verificación final del bloque

- [ ] Gate completo verde:
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform; cargo clippy --workspace --all-targets -- -D warnings
```
- [ ] `graphify update .` para refrescar el grafo.
- [ ] Visto bueno visual de Nicolás de la sección "Novedades" en el Acerca de.
- [ ] (Nicolás) Tag v0.1.0 y push.
- [ ] (Nicolás, opcional) Probar update in-place: instalar 0.1.0, configurar algo, bumpear a 0.1.1, compilar instalador, instalar encima y confirmar que actualiza sin duplicar entrada y conserva config.

---

## Notas de cierre

- Al terminar, sugerir a Nicolás el siguiente bloque: **bloque 2 (docs al día + limpieza de voseo, incl. `build-release.ps1` línea ~89)**, luego bloque 3 (copiar nombres/rutas) y bloque 4 (vista profunda). Backlog aparte: argumentos de CLI para `naygo.exe` (ya existe `core::cli` como base).
- Actualizar la memoria de proyecto con el estado del bloque al cerrarlo.
```
