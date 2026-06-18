# Versionado, CHANGELOG, instalador actualizable y "Novedades" — Diseño

> Bloque 1 de un trabajo mayor. Bloques posteriores (cada uno con su propio ciclo):
> (2) docs al día, (3) copiar nombres/rutas, (4) vista profunda. Pendiente aparte en
> backlog: argumentos de línea de comandos para `naygo.exe`.

**Objetivo:** dar a Naygo un sistema de versionado controlado por commits, un CHANGELOG
como historia única de cambios, un instalador que actualiza instalaciones existentes sin
perder configuración, y una sección "Novedades" en el "Acerca de" que muestra las mejoras
de la versión instalada.

**Autoría:** Nicolás Groth / ISGroth, 2026, MIT.

---

## Estado actual (lo que YA existe — no rehacer)

- **Versión = fuente única** en `Cargo.toml` raíz (`[workspace.package] version = "0.1.0"`),
  heredada por los crates. El binario la lee con `env!("CARGO_PKG_VERSION")` y la muestra
  en el "Acerca de" (`main.rs:1480` → `cfg.set_app_version(...)`) y en el log
  (`logging.rs:35`).
- **`scripts/build-release.ps1`** ya lee la versión del Cargo.toml, compila release, arma
  el ZIP portable `Naygo-<v>-portable.zip` y genera el instalador inyectando
  `/DMyAppVersion=<v>`.
- **`installer/naygo.iss`** ya tiene `AppId` fijo (GUID estable), `AppVersion={#MyAppVersion}`,
  salida `Naygo-<v>-setup`, binarios en `{app}`, config del usuario en `%APPDATA%`
  (intacta en updates), y entradas de registro opcionales para "Abrir con" / menú
  contextual que pasan `"%1"`/`"%V"` (la carpeta) al exe.
- **`ctx_copy_path` / `ctx_copy_names`** (workspace_ctrl.rs) ya copian rutas absolutas /
  nombres con extensión de la selección — relevante para el bloque 3, no para este.
- **No hay** tags git, **no hay** CHANGELOG.md.

## Estado objetivo (lo que este bloque agrega)

1. Tag inicial `v0.1.0` + `CHANGELOG.md` con la entrada 0.1.0 que resume lo construido.
2. `scripts/bump.ps1`: infiere el nivel SemVer de los commits y publica una versión.
3. `core/src/changelog.rs`: parser puro que extrae las notas de una versión del CHANGELOG.
4. "Novedades" en el "Acerca de": viñetas de la versión instalada, desde el CHANGELOG
   embebido con `include_str!`.
5. `naygo.iss`: cierre de la app en ejecución durante el update (2 líneas).

---

## Sección 1 — Fuente única de versión y flujo

La versión vive solo en `Cargo.toml` raíz. Flujo de publicación:

```
1. Commits con prefijos convencionales:  feat(x): … / fix: … / refactor: …
2. scripts\bump.ps1 [-Level x] [-DryRun] [-Push]
     → infiere nivel desde el último tag, sube Cargo.toml, mueve "Sin publicar" del
       CHANGELOG a "X.Y.Z — fecha", commitea "chore(release): vX.Y.Z", crea tag vX.Y.Z,
       (con -Push) empuja commit y tags.
3. scripts\build-release.ps1
     → compila release + portable + instalador, leyendo la versión YA fijada.
```

`bump.ps1` (cambia versión) y `build-release.ps1` (compila) quedan **separados**.

**Arranque:** como no hay tags, el primer paso del plan crea el tag `v0.1.0` sobre el
estado actual y escribe a mano la entrada `0.1.0` del CHANGELOG.

## Sección 2 — `bump.ps1`

Script PowerShell, no interactivo por defecto.

**Algoritmo:**
1. Última versión = `git describe --tags --abbrev=0 --match "v*"`; si no hay tags, base
   `0.1.0` (y se considera "primer release": no aborta por falta de commits previos).
2. Commits desde el tag (`git log <tag>..HEAD --format=%s%n%b` por commit). Clasificar el
   *subject*:
   - `^(feat)(\(.+\))?!?:` → minor (o major si lleva `!`)
   - `^(fix)(\(.+\))?!?:` → patch (o major si lleva `!`)
   - cuerpo contiene `BREAKING CHANGE` → major
   - cualquier otro tipo (`docs|chore|refactor|style|test|build|perf|ci|revert`) → sin nivel
3. Nivel = máximo encontrado (major > minor > patch). **Fallback:** hay commits pero
   ninguno aportó nivel → **patch**. No hay commits nuevos desde el tag → abortar con
   "nada que versionar" (salvo que no hubiera tag previo: entonces versiona la base).
4. **Flags:** `-Level patch|minor|major` (salta inferencia), `-DryRun` (muestra y no toca
   nada), `-Push` (tras commit+tag hace `git push` y `git push --tags`).
5. **Aplicar** (salvo `-DryRun`): calcular X.Y.Z → reescribir la línea `version = "..."`
   de `[workspace.package]` en `Cargo.toml` → actualizar CHANGELOG (sección 3) → refrescar
   `Cargo.lock` con `cargo update --workspace --offline` (los crates `naygo-*` heredan la
   versión del workspace; esto actualiza sus entradas en el lock sin tocar la red) → `git
   add Cargo.toml Cargo.lock CHANGELOG.md` → `git commit -m "chore(release): vX.Y.Z"` →
   `git tag vX.Y.Z`. Si `cargo update` no estuviera disponible o fallara offline, el lock
   se regenera igual en el `build-release.ps1` posterior; el commit de release puede omitir
   `Cargo.lock` en ese caso (no es bloqueante).
6. **Push:** solo con `-Push`. Sin el flag, imprime el recordatorio `git push && git push
   --tags`. (Claude nunca ejecuta push por su cuenta; el flag lo dispara el usuario.)

**Salvaguardas:**
- Si el working tree está sucio al empezar → abortar (no versionar a medias).
- Validar que exista exactamente una línea `version = "..."` bajo `[workspace.package]`
  antes de reescribir; si no, abortar.
- Validar formato del nuevo tag (`vX.Y.Z`) y que no exista ya.

**Funciones internas (para testear la lógica pura sin git):**
- `Get-BumpLevel([string[]] $subjects, [string[]] $bodies) -> 'major'|'minor'|'patch'|$null`
- `Step-Version([string] $current, [string] $level) -> string` (ej. `0.1.0`,`minor`→`0.2.0`)

## Sección 3 — CHANGELOG.md + parser en core

**Formato** (Keep a Changelog, español neutral). `bump.ps1` renombra `## [Sin publicar]`
a `## [X.Y.Z] — YYYY-MM-DD` (la fecha la genera PowerShell) y crea un `## [Sin publicar]`
vacío arriba.

```markdown
# Changelog

Todas las novedades de Naygo. Formato basado en Keep a Changelog; versiona con SemVer.

## [Sin publicar]

## [0.1.0] — 2026-06-18
### Añadido
- Navegación tipo Commander: paneles dinámicos, dual-pane, atrás/adelante (+ botones del mouse).
- …
```

Categorías válidas: **Añadido, Cambiado, Obsoleto, Eliminado, Corregido, Seguridad**.

**Parser** — `crates/core/src/changelog.rs` (sin dependencias nuevas, parseo por líneas):

```rust
pub struct ReleaseNotes {
    pub version: String,
    pub date: Option<String>,
    pub sections: Vec<NoteSection>,
}
pub struct NoteSection {
    pub category: String,      // "Añadido", "Corregido", …
    pub items: Vec<String>,    // viñetas sin el "- " inicial
}

/// Extrae las notas de `version` del texto de un CHANGELOG.
/// `version` se compara contra el contenido entre corchetes del encabezado `## [..]`.
/// Devuelve `None` si no hay un bloque para esa versión.
pub fn release_notes(changelog: &str, version: &str) -> Option<ReleaseNotes>;
```

Reglas de parseo: un bloque empieza en una línea `## [<v>]` (con `— fecha` opcional);
subsecciones en `### <categoría>`; viñetas en líneas que empiezan con `- `; el bloque
termina en el siguiente `## ` o fin de texto. Tolerante: categorías ausentes, espacios
extra, CHANGELOG malformado → `None` o secciones vacías, **nunca panic**.

**Tests en core** (es lógica pura — corazón del crate testeable):
- extrae la versión pedida e ignora otras;
- agrupa viñetas por categoría, en orden;
- versión inexistente → `None`;
- CHANGELOG vacío / sin `##` → `None`;
- bloque con encabezado pero sin viñetas → `sections` vacío, no panic;
- fecha presente y ausente.

**Embebido** en `crates/ui-slint/src/`:
```rust
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md"); // raíz del repo
```
(La ruta exacta se valida al implementar; `include_str!` garantiza que el archivo exista
en build time.)

## Sección 4 — "Novedades" en el "Acerca de" (UI Slint)

En `config-window.slint`, categoría **Acerca de** (ya tiene versión/autor/licencia/stack +
botón al repo), agregar una sección **Novedades de esta versión**:

- Propiedad nueva en la ventana: `in property [{category: string, items: [string]}]
  release-notes;` (modelo Slint).
- En `main.rs`, tras `set_app_version`, llamar `naygo_core::changelog::release_notes(
  CHANGELOG, env!("CARGO_PKG_VERSION"))`, mapear a un `VecModel` y `set_release_notes(...)`.
- Render dentro del scroll existente del "Acerca de": encabezado `Novedades — v{version}`,
  y por cada sección un subtítulo (categoría) + las viñetas. Estilo vía `Theme`, nada
  hardcodeado.
- Si `release_notes` → `None` (o lista vacía): mostrar una línea discreta con la clave
  i18n `about-no-notes` ("Sin notas para esta versión").
- **i18n triple** para los textos fijos: `about-news-title` ("Novedades de esta versión"),
  `about-no-notes`. Las viñetas vienen del CHANGELOG tal cual (no se traducen).

Alcance (YAGNI): solo la versión instalada; sin navegación a versiones anteriores.

**Verificación:** la extracción queda cubierta por tests de core. El render va a visto
bueno visual de Nicolás (que la sección se vea bien dentro del "Acerca de").

## Sección 5 — Instalador actualizable in-place

El `naygo.iss` ya cumple casi todo (AppId fijo, versión inyectada, config en %APPDATA%).
**Único cambio:** en `[Setup]`, agregar para un update limpio cuando la app está abierta:

```
CloseApplications=yes
RestartApplications=no
```

Con esto, si Naygo corre durante la instalación, Inno ofrece cerrarlo antes de reemplazar
el `.exe` (evita "archivo en uso"). El resto del comportamiento de update ya es correcto:
mismo `AppId` → actualiza en vez de duplicar; `%APPDATA%` intacto → conserva config/temas/
layout; uninstaller borra solo `{app}` (no toca config).

**Verificación:** compilar el instalador (lo hace `build-release.ps1`); la prueba de
actualización real sobre una instalación previa (que actualice sin duplicar y conserve
config) la confirma Nicolás en un equipo/VM, acorde al requisito de correr en equipos
limpios.

---

## Archivos tocados

| Archivo | Acción |
|---|---|
| `CHANGELOG.md` | **Crear** (entrada 0.1.0 + "Sin publicar"). |
| `scripts/bump.ps1` | **Crear** (inferencia + bump + tag + push opt-in). |
| `crates/core/src/changelog.rs` | **Crear** (parser + tests). |
| `crates/core/src/lib.rs` | Exponer `pub mod changelog;`. |
| `crates/ui-slint/src/main.rs` | Embeber CHANGELOG, llamar al parser, `set_release_notes`. |
| `crates/ui-slint/ui/config-window.slint` | Sección "Novedades" en el "Acerca de". |
| `crates/ui-slint/ui/i18n.slint` | Claves `about-news-title`, `about-no-notes`. |
| `crates/core/src/i18n/es.json`, `en.json` | Traducciones de esas claves. |
| `crates/ui-slint/src/i18n_keys.rs` | Setters de esas claves. |
| `installer/naygo.iss` | `CloseApplications=yes` + `RestartApplications=no`. |

## Fuera de alcance (este bloque)

- Reescritura de README/GUÍA/BUILD/DISTRIBUTION y barrido de voseo (bloque 2).
- Copiar nombres/rutas (bloque 3). Vista profunda (bloque 4).
- Argumentos de CLI para `naygo.exe` (backlog).
- Chequeo de versión online / auto-update por red (descartado por ahora).
- Branch protection de GitHub (instructivo aparte en `docs/PROTEGER-RAMA-MAIN.md`).

## Riesgos y mitigaciones

- **Ruta de `include_str!`**: si la relativa falla, no compila → se ve de inmediato; se
  ajusta la ruta. Garantía, no riesgo silencioso.
- **Inferencia de nivel equivocada**: cubierta por tests de las funciones puras de bump.
- **`bump.ps1` sobre repo sucio**: abortado por salvaguarda.
- **Codificación del CHANGELOG**: usar UTF-8; el parser no asume ASCII (categorías con
  acentos). El `.json` ya es UTF-8.
