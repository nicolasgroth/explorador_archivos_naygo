# Argumentos de línea de comandos para naygo.exe — Diseño

> Backlog pendiente desde hace varias sesiones (memoria `feature-backlog-cli-args`). Permite
> abrir Naygo en una carpeta concreta y/o con un tema/plantilla, desde un acceso directo, la
> terminal, o el "Abrir en Naygo" del menú contextual (que el instalador ya cablea con `%V`).

**Objetivo:** `naygo.exe` acepta argumentos para abrir un directorio específico, aplicar un
tema y/o una plantilla de disposición al arrancar, más `--help`/`--version`. La app nunca
falla por un argumento inválido.

**Autoría:** Nicolás Groth / ISGroth, 2026, MIT.

**Sintaxis:**
```
naygo.exe [<carpeta>] [--theme <id>] [--layout <nombre>] [--help] [--version]
```

**Decisiones tomadas con el usuario:**
- La **carpeta posicional MANDA sobre la sesión**: si es un directorio existente, se abre en el
  panel activo aunque haya sesión guardada. Sin carpeta → se respeta la sesión / arranque clásico.
- **Flags con nombre** `--theme <id>` y `--layout <nombre>`.
- El **tema/plantilla del argumento aplican SOLO para esa sesión** (no modifican la config
  guardada): al reabrir normal, vuelve el tema/disposición de siempre.
- **Args inválidos** (carpeta que no es dir, tema/plantilla inexistente) → **aviso + abrir
  igual** (la app nunca cae por un argumento malo). El aviso es un diálogo breve al arrancar.
- **`--help`/`--version`** → diálogo nativo (rfd) con el texto, sin abrir la ventana principal.
- **`--layout`** acepta plantillas **built-in y guardadas por el usuario**.

---

## Contexto actual (lo que se reutiliza — no rehacer)

- **`crates/core/src/cli.rs`** YA tiene: `first_positional(args) -> Option<&str>`,
  `resolve_initial_dir(args, is_dir) -> Option<PathBuf>` (predicado `is_dir` inyectable para
  test puro), `parse_initial_dir(args)` (usa `Path::is_dir` real). Con tests. SOLO resuelve la
  carpeta; no parsea flags.
- **`crates/ui-slint/src/main.rs:165`** arranca con `WorkspaceCtrl::new(start)` donde `start` es
  la carpeta por defecto — **NO consume `cli`**. La línea `if !load_session() {
  apply_first_run_layout() }` (agregada hace poco) maneja sesión vs arranque clásico.
- **Temas:** `naygo_core::theme::ThemeCatalog` (`get(&ThemeId)`, `available()`, `default_id()`).
  El selector de config y el cambio de tema en runtime ya aplican un `ThemeId`.
- **Plantillas:** `TemplateStore`/`LayoutTemplate` (built-in + usuario); el menú de Layouts ya
  busca y aplica una plantilla por nombre. `WorkspaceCtrl::apply_template`/equivalente existe.
- **Diálogos nativos:** `rfd::MessageDialog` ya se usa (panic hook, confirmaciones).
- **Logging:** `crate::logging::log_line`/`breadcrumb` para registrar avisos.
- **Instalador (`installer/naygo.iss`):** ya registra "Abrir con"/menú contextual que invocan
  `"{app}\naygo.exe" "%1"` y `"%V"` (la carpeta) — este diseño hace que ese arg SE USE.

---

## Sección 1 — `core::cli`: parsear a una struct (puro, testeable)

Se amplía `cli.rs` con una struct de opciones y un parser que NO valida contra catálogos (core
no los conoce); solo extrae las cadenas. La validación vive en la capa UI.

```rust
/// Argumentos de línea de comandos ya parseados. `theme`/`layout` son las cadenas CRUDAS del
/// argumento (sin validar contra el catálogo: eso lo hace la UI, que sí lo conoce).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub dir: Option<PathBuf>,   // carpeta posicional, solo si es un directorio existente
    pub theme: Option<String>,  // valor de --theme
    pub layout: Option<String>, // valor de --layout
    pub help: bool,
    pub version: bool,
}

/// Parsea los argumentos (SIN el ejecutable). `is_dir` valida la existencia de la carpeta
/// posicional (inyectable para test puro; en producción `|p| p.is_dir()`). Reglas:
/// - `--help` / `--version` ponen su flag (no consumen valor).
/// - `--theme <v>` / `--layout <v>` consumen el SIGUIENTE token como valor; si no hay token
///   siguiente, el flag se ignora (queda None).
/// - El primer token que no sea flag ni valor-de-flag es la carpeta posicional (si `is_dir`).
/// Nunca hace panic. No toca disco salvo por `is_dir`.
pub fn parse_args(args: &[String], is_dir: impl Fn(&Path) -> bool) -> CliArgs;

/// Atajo de producción: usa `Path::is_dir`.
pub fn parse_args_real(args: &[String]) -> CliArgs;
```

Se conservan `first_positional`/`resolve_initial_dir`/`parse_initial_dir` (los reusa `parse_args`
para la carpeta; o se reimplementa la extracción de la carpeta saltando los flags y sus valores).

## Sección 2 — Cableado en `main.rs` (valida y aplica)

`main.rs` es la capa que conoce el catálogo de temas y el store de plantillas. Flujo al arrancar:

```
1. let cli = cli::parse_args_real(&args_sin_exe);
2. if cli.help    { mostrar_dialogo_ayuda();   return Ok(()); }   // NO abre la ventana
   if cli.version { mostrar_dialogo_version();  return Ok(()); }
3. Construir el WorkspaceCtrl (como hoy).
4. Determinar la carpeta de arranque y la sesión:
   - if let Some(dir) = cli.dir → abrir esa carpeta en el panel activo (override de sesión:
     se construye/restaura el workspace y luego se navega el panel activo a `dir`). 
   - else → flujo actual: load_session() ó apply_first_run_layout().
5. if let Some(name) = cli.layout → buscar la plantilla por nombre (built-in + usuario);
   si existe, aplicarla; si no, push a `avisos` ("plantilla '<name>' no encontrada").
   (Si hay dir Y layout: el layout arma la disposición; el dir va al panel activo resultante.)
6. if let Some(id) = cli.theme → si `ThemeCatalog` tiene ese id, aplicarlo a la sesión SIN
   persistir; si no, push a `avisos` ("tema '<id>' no encontrado").
   Si la carpeta posicional era inválida (cli.dir None pero había un primer token que no era
   flag), también push a `avisos` ("la ruta no es una carpeta: …"). 
7. if !avisos.is_empty() → mostrar UN diálogo breve que los resuma (rfd), y `log_line` cada uno.
   La app abre igual.
```

**Aplicar tema sin persistir:** usar el camino que cambia el tema activo en runtime (el que usa
el selector) PERO sin llamar al guardado de settings. Si el método actual persiste siempre,
agregar una variante "aplicar tema efímero" o setear el tema activo del `ConfigCtrl` en memoria
sin `save()`. Detalle a resolver al implementar; el contrato: la config en disco NO cambia.

**Aplicar plantilla:** reusar `apply_template`/equivalente. Igual: no debe alterar la plantilla
por defecto persistida; solo la disposición de esta sesión.

**Detección de "primer token inválido como carpeta":** para distinguir "no pasó carpeta" de
"pasó una ruta que no es dir" (y avisar en el 2º caso), `parse_args` puede exponer el primer
posicional crudo aunque `is_dir` falle — p. ej. un campo `dir_arg_raw: Option<String>` además
de `dir: Option<PathBuf>` (dir = Some solo si es dir válido; dir_arg_raw = el token tal cual).
Así main sabe que hubo un intento de carpeta inválida y avisa.

## Sección 3 — `--help` / `--version`

App GUI sin consola → diálogos nativos (rfd), y `return` sin abrir la ventana:
- `--version`: título "Naygo", cuerpo `Naygo v{CARGO_PKG_VERSION}\nNicolás Groth / ISGroth · MIT`.
- `--help`: título "Naygo — opciones", cuerpo con la sintaxis y cada opción explicada (carpeta,
  --theme con la lista de ids disponibles via `ThemeCatalog::available()`, --layout, --help,
  --version). Texto en español neutral.

---

## Archivos tocados

| Archivo | Acción |
|---|---|
| `crates/core/src/cli.rs` | + `CliArgs`, `parse_args`, `parse_args_real`, `dir_arg_raw`; tests. Conserva lo existente. |
| `crates/ui-slint/src/main.rs` | Consumir `parse_args_real(env::args)`; help/version (diálogo + return); aplicar dir (override) / layout / theme (efímero); juntar avisos en un diálogo + log. |
| `crates/ui-slint/src/workspace_ctrl.rs` o `config_ctrl.rs` | Si hace falta, un método "aplicar tema efímero (sin persistir)" y/o "abrir carpeta en panel activo" reusable. Solo si los existentes persisten. |
| `README.md`, `docs/GUIA-DE-USUARIO.md` | Documentar la sintaxis de CLI. |
| `CHANGELOG.md` | Entrada en "Sin publicar". |

## Testing

- **core (`cli.rs`), lo importante (puro):** `parse_args` con `is_dir` inyectable:
  - sin args → todo None/false;
  - solo carpeta válida → dir Some;
  - carpeta inválida → dir None pero `dir_arg_raw` Some;
  - `--theme winxp` → theme Some("winxp");
  - `--layout "Mi plantilla"` → layout Some;
  - `--theme` sin valor (último token) → theme None (ignorado, sin panic);
  - orden mezclado `["--theme","x","D:\\dir","--layout","y"]` → dir + theme + layout correctos;
  - `--help` / `--version` → flags;
  - flag desconocido (`--zzz`) → se ignora sin romper.
- **Validación contra catálogo + cableado (main.rs):** verificación manual / visto bueno de
  Nicolás (main monta la ventana, no es unit-testeable). Lanzar: `naygo.exe "D:\..."`;
  `--theme winxp`; `--theme inexistente` (aviso); `--layout <nombre>`; `--help`; `--version`.

## Fuera de alcance (YAGNI)

- Persistir el tema/plantilla del argumento (se decidió efímero).
- Múltiples carpetas → múltiples paneles (solo una carpeta posicional → panel activo).
- Una librería de parseo de args (clap, etc.): el parser a mano es suficiente y sin deps nuevas.
- Abrir archivos (no solo carpetas) por argumento — la carpeta es el caso pedido; un archivo no
  es dir → se ignora con aviso. (Abrir-y-seleccionar un archivo queda para otro momento.)

## Riesgos y mitigaciones

- **Tema/plantilla que persiste sin querer:** mitigado aplicando en memoria sin `save()`;
  verificar el camino real al implementar.
- **`--help` que igual abre la ventana:** asegurar el `return` antes de construir/mostrar la UI.
- **Arg con espacios/comillas (%V):** Windows lo entrega como un token; el parser lo respeta.
- **Doble propósito de `%1` (Abrir con) y `%V` (menú carpeta):** ambos entregan una ruta de
  carpeta como primer posicional; el mismo parser los cubre.
