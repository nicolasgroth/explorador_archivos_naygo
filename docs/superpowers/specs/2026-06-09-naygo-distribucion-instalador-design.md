# Naygo — Distribución / Instalador — Diseño

> Fase de **empaquetado y distribución** (no de feature de la app). Objetivo: poder
> instalar y probar Naygo en otros entornos (VMs / equipos limpios) mediante un
> instalador completo y un ZIP portable, con ícono y splash propios.

Autor: Nicolás Groth / ISGroth (Chile), 2026, MIT. Repo:
`github.com/nicolasgroth/explorador_archivos_naygo`.

## Contexto y punto de partida

Lo que YA existe (no rehacer):

- `crates/ui/Cargo.toml`: `[[bin]] name = "naygo"` → el binario ya es `naygo.exe`.
- `crates/ui/app.rc`: recurso de versión COMPLETO (CompanyName "ISGroth",
  ProductName "Naygo", FileDescription "Naygo — explorador de archivos rápido",
  LegalCopyright MIT, FileVersion/ProductVersion 0.1.0.0). **No** referencia un ícono.
- `crates/ui/build.rs`: compila `app.rc` vía `embed_resource::compile("app.rc", …)`
  (solo en Windows; no-op en otros SO). `embed-resource = "3"` ya es build-dependency.
- `crates/ui/src/main.rs`: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
  (sin consola en release). **No** procesa `std::env::args()`.
- `.cargo/config.toml`: `+crt-static` para MSVC → el `.exe` corre en equipos limpios
  sin instalar el VC++ Redistributable.
- `[profile.release]`: `opt-level="s"`, `lto=true`, `codegen-units=1`, `strip=true`.
- `core::config::portable_dir()`: Naygo ya guarda su config junto al `.exe` (portable).
- `assets/icons/naygo_icon.ico`: ícono propio multi-resolución (16/24/32/48/64/128/256).
- `assets/icons/logo_naygo.png`: logo 1254×1254 (para splash + wizard del instalador).

Lo que FALTA (esta fase): referenciar el ícono en el `.exe`; soporte de ruta por
línea de comandos; splash screen; instalador Inno Setup; ZIP portable; script de
build; documentación.

## Decisiones tomadas (brainstorm 2026-06-09)

- Tecnología de instalador: **Inno Setup** (script `.iss` legible, gratis).
- Artefactos: **instalador + ZIP portable** (ambos).
- Ícono/logo: los provee Nicolás (`naygo_icon.ico` + `logo_naygo.png`).
- Splash: **breve, auto-desaparece** (~1.2s o cuando la UI esté lista, lo que pase
  primero; clic/tecla también lo cierra). Solo en release.
- Modo de instalación: **elegible** (para mí / para todos) en el asistente.
- Opciones del wizard: acceso directo en Escritorio; lanzar Naygo al terminar;
  "Abrir con"; menú contextual "Abrir en Naygo".
- Soporte de ruta CLI: **se agrega ahora** (necesario para que "Abrir con" / menú
  contextual abran la carpeta clickeada).
- Firma de código: **sin firmar por ahora** (documentar la advertencia de SmartScreen).
- **Documentación: entregable de primera clase** de esta fase (ver sección propia).

## Componentes

### 1. Ícono en el `.exe` y en la ventana

- `app.rc`: agregar una línea de recurso de ícono, p. ej.
  `IDI_NAYGO ICON "../../assets/icons/naygo_icon.ico"` (ruta relativa al `.rc`,
  resuelta por el compilador de recursos en build; verificar la ruta real respecto
  a `crates/ui/`). El primer ícono del `.rc` es el que Windows usa para el `.exe`.
- Ventana de la app: eframe toma el ícono de ventana vía
  `egui::IconData` en `NativeOptions.viewport.icon`. Cargar desde el `.ico`/PNG
  embebido (`include_bytes!`) y decodificar a RGBA. Si falla la decodificación, se
  omite (la app arranca igual, sin ícono de ventana — tolerante).
- No se modifican los metadatos existentes del `.rc` salvo, opcionalmente, la versión
  (se deja 0.1.0).

### 2. Soporte de ruta por línea de comandos

- `core::cli` (NUEVO módulo, PURO, testeable): `parse_initial_dir(args: &[String])
  -> Option<PathBuf>`. Toma el primer argumento posicional; devuelve `Some(path)` solo
  si existe y es un directorio; en cualquier otro caso (ausente, archivo, ruta
  inexistente, vacío) devuelve `None`. No hace I/O más allá de un `is_dir()` chequeo.
  Para testear sin tocar disco real, la función acepta los args y un predicado de
  validación inyectable, o bien se testea con rutas de `tempfile`. (El implementador
  elige; lo esencial es que el parseo de "cuál es el primer arg" sea puro y testeado,
  y la validación de existencia esté aislada.)
- `main.rs`: invoca `parse_initial_dir(&std::env::args().collect::<Vec<_>>()[1..])`
  (o equivalente) y pasa el resultado como carpeta inicial del primer panel. Si es
  `None`, arranque normal (carpeta por defecto de hoy). Una ruta inválida se loguea
  y se ignora — nunca rompe el arranque.
- Esto habilita `naygo.exe <ruta>` desde cualquier lado y es lo que usan las
  asociaciones del instalador.

### 3. Splash screen (solo release)

- Al iniciar (en release), mostrar `logo_naygo.png` (embebido vía `include_bytes!`)
  en una presentación liviana. Implementación a criterio del implementador dentro de
  estas restricciones:
  - No frena el arranque: el arranque de Naygo es rápido, así que el splash es un
    destello corto. Se cierra cuando la UI está lista **o** tras ~1.2s, lo que pase
    primero; clic o cualquier tecla también lo cierra.
  - Centrado, sin bordes; no roba el foco de teclado de la app; es solo al inicio del
    proceso (no reaparece en navegaciones).
  - Tolerante: si el logo no carga/decodifica, se omite el splash (la app arranca
    igual).
  - En debug NO aparece (para no estorbar el desarrollo).
- Enfoque recomendado (no obligatorio): un estado inicial `Splash` dentro de la propia
  app egui que pinta el logo sobre el área central y transiciona al layout normal por
  tiempo/primer-frame/input, en vez de una segunda ventana de SO (más simple, sin
  parpadeo de ventanas, y respeta `windows_subsystem="windows"`). Si el implementador
  encuentra que un viewport separado es claramente mejor, puede proponerlo, pero el
  default es el estado interno.

### 4. Instalador Inno Setup

`installer/naygo.iss` → genera `dist/Naygo-<versión>-setup.exe`.

- **Modo elegible**: `PrivilegesRequiredOverridesAllowed=dialog` (o equivalente) →
  el asistente ofrece "para mí" (sin UAC, `{localappdata}\Programs\Naygo`) o
  "para todos" (`{autopf}\Naygo`, con elevación).
- **Archivos**: `naygo.exe` (único ejecutable, todo embebido por CRT estático +
  assets en el `.exe`), `LICENSE`, `README.md` (o un README de distribución).
- **Accesos directos**: menú Inicio (siempre); Escritorio (checkbox, marcado por
  defecto).
- **Páginas del wizard**: página de licencia (MIT); imágenes del asistente derivadas
  de `logo_naygo.png` (Inno consume BMP: `WizardImageFile` lateral y
  `WizardSmallImageFile`; se generan los BMP a los tamaños que pide Inno —
  típicamente 164×314 y 55×58 — como paso del build o como archivos versionados);
  página final con checkbox **"Ejecutar Naygo"**.
- **Asociaciones (página de Tareas, checkboxes)**:
  - **"Abrir con"**: registrar Naygo en `OpenWithProgids` para `Directory` (NO como
    handler predeterminado; no se toca Win+E ni el default del shell).
  - **"Abrir en Naygo"** (menú contextual de carpetas): claves
    `Directory\shell\Naygo` y `Directory\Background\shell\Naygo`, con
    `command = "\"{app}\naygo.exe\" \"%V\""`. Texto e ícono de la entrada usan el
    `.exe`. Ambas claves se eliminan al desinstalar.
- **Desinstalador**: el de Inno (aparece en "Agregar o quitar programas"); borra
  archivos, accesos directos y las claves de registro creadas. **No** borra la config
  del usuario (se documenta dónde queda y que se respeta).
- **Versión**: el `.iss` toma la versión desde una variable que el script de build
  inyecta leyéndola del `Cargo.toml` (fuente única de verdad), p. ej. vía
  `/DMyAppVersion=...` en la línea de `ISCC`.
- **Sin firma**: el setup funciona igual; SmartScreen mostrará "editor desconocido"
  la primera vez (documentado).

### 5. ZIP portable

`dist/Naygo-<versión>-portable.zip` contiene: `naygo.exe`, `LICENSE`,
`installer/LEEME.txt` (nota corta: qué es, cómo correr, que la config se guarda junto
al `.exe`). Copiar y ejecutar, sin instalación. Convive con la versión instalada sin
pisarse (el portable guarda config junto al exe; el instalado en su carpeta/`%AppData%`).

### 6. Script de build

`scripts/build-release.ps1` (PowerShell), orquesta todo en un comando:

1. `cargo build --release` (produce `target/release/naygo.exe` con ícono + metadatos
   + CRT estático).
2. Lee la versión del `Cargo.toml` (workspace.package.version) — fuente única.
3. Arma `dist/Naygo-<versión>-portable.zip` (`naygo.exe` + `LICENSE` + `LEEME.txt`).
4. Genera (si hace falta) los BMP del wizard desde `logo_naygo.png`.
5. Llama a `ISCC.exe` sobre `installer/naygo.iss` con la versión inyectada →
   `dist/Naygo-<versión>-setup.exe`.
6. Si Inno (`ISCC.exe`) no está instalado/en PATH, emite un mensaje claro con el link
   de descarga y **no** falla la corrida: el ZIP portable igual queda hecho.
- El script lleva comentarios ricos por bloque (qué hace y por qué), no solo el header.

### 7. Documentación (entregable de primera clase)

- `docs/BUILD.md` (NUEVO): cómo compilar release; prerequisitos (Rust toolchain MSVC,
  Inno Setup + link de descarga, cómo dejar `ISCC.exe` accesible); cómo correr
  `build-release.ps1`; qué genera cada artefacto y dónde; troubleshooting común
  (Inno ausente, ruta del ícono, etc.).
- `docs/DISTRIBUTION.md` (NUEVO): el modelo de distribución (instalador vs portable);
  modos de instalación (por-usuario / para-todos) y dónde instala cada uno; qué
  escribe en disco y en el registro (accesos directos, "Abrir con", menú contextual);
  cómo desinstalar limpio y qué pasa con la config del usuario; la nota de SmartScreen
  y los pasos exactos para pasar la advertencia ("Más info → Ejecutar de todos modos").
- `naygo.iss` y `build-release.ps1`: comentados por sección (el qué y el por qué).
- `README.md`: sección "Instalación / Build" que enlaza a `docs/BUILD.md` y
  `docs/DISTRIBUTION.md`, y resume la nota de SmartScreen.
- `core::cli`: doc-comments del módulo y de `parse_initial_dir` (qué hace, cómo se usa,
  de qué depende), coherente con el resto del core.

## Estructura de archivos (resumen)

```
assets/icons/naygo_icon.ico        (ya está)
assets/icons/logo_naygo.png        (ya está)
crates/ui/app.rc                   (+ línea ICON)
crates/ui/src/main.rs              (+ arg de ruta + splash en release + ícono ventana)
crates/core/src/cli.rs             (NUEVO: parse_initial_dir, puro, testeado)
crates/core/src/lib.rs             (+ pub mod cli)
installer/naygo.iss                (NUEVO: script Inno, comentado)
installer/LEEME.txt                (NUEVO: nota del portable)
scripts/build-release.ps1          (NUEVO: orquestador, comentado)
docs/BUILD.md                      (NUEVO)
docs/DISTRIBUTION.md               (NUEVO)
dist/                              (gitignored: artefactos generados)
README.md                          (+ sección Instalación / Build)
.gitignore                         (+ /dist)
```

## Verificación

- Tests de `core::cli`: ruta válida → `Some`; inexistente / archivo / ausente / vacío
  → `None`.
- `cargo build --release` compila; `naygo.exe` muestra el ícono propio y los metadatos
  (clic derecho → Propiedades → Detalles: ISGroth, Naygo, copyright, versión).
- Correr el `.exe`: arranca; el splash aparece y se va (release); abre normal;
  `naygo.exe C:\Windows` abre el primer panel en esa carpeta; una ruta inválida no
  rompe el arranque.
- `build-release.ps1` genera el ZIP portable y (con Inno presente) el setup; sin Inno,
  avisa y deja el ZIP.
- Instalación manual (la hace Nicolás en una VM): modo por-usuario; accesos directos
  en menú Inicio/Escritorio; "Abrir en Naygo" aparece en el clic derecho de una
  carpeta y abre ahí; "Ejecutar Naygo" al final funciona; desinstalación limpia (sin
  dejar claves de registro ni accesos); el ZIP portable corre en limpio.
- `cargo clippy --workspace --all-targets -- -D warnings` y `cargo fmt --all --check`
  verdes; tests del workspace verdes.
- `.gitignore` excluye `dist/`.

## Reparto de verificación

El agente compila, corre tests, genera los artefactos y verifica que el setup se
construya y que el `.iss` registre las claves/accesos esperados (revisión del script).
**La prueba visual de arranque, instalación y uso en una VM/equipo limpio la hace
Nicolás** (es interfaz gráfica en otro entorno, fuera del alcance de verificación
automatizada del agente).

## Fuera de alcance (explícito)

- Firma de código (code signing) — sin certificado por ahora; documentar SmartScreen.
- Splash configurable/avanzado (duración ajustable, animaciones) — el splash es fijo y
  breve.
- Menú contextual NATIVO completo del shell (IContextMenu/IShellFolder) — eso es la
  fase **shell-B**. Esta fase solo agrega la entrada simple "Abrir en Naygo" vía
  registro + el handler de ruta CLI.
- Auto-update / actualizaciones in-app.
- Build multiplataforma — Naygo es Windows 10/11.

## Notas de riesgo / cuidado para el plan

- **Ruta del ícono en `app.rc`**: el compilador de recursos resuelve la ruta relativa
  al `.rc` (en `crates/ui/`). Verificar que `../../assets/icons/naygo_icon.ico` (o la
  que corresponda) resuelva en el build de `embed-resource`. Confirmar contra el build
  real al implementar.
- **`IconData` de eframe 0.34**: confirmar la API exacta (`egui::IconData { rgba,
  width, height }` y `ViewportBuilder::with_icon`) contra la versión en uso.
- **Inno entorno headless**: el agente puede no tener `ISCC.exe`; el script debe
  degradar con aviso, no romper. La generación del setup quizá no se pueda verificar
  en la máquina del agente más allá de validar el `.iss` — documentarlo.
- **BMP del wizard**: Inno requiere BMP (no PNG) para `WizardImageFile`. Decidir si se
  versionan los BMP generados o se generan en el build desde `logo_naygo.png`
  (preferible generarlos en el build para no versionar binarios derivados; si la
  generación en build es frágil, versionarlos es aceptable).
- **`%V` vs `%1`** en el verbo del menú contextual: para `Directory\Background\shell`
  se usa `%V`; para `Directory\shell` también `%V` funciona como la carpeta. Confirmar
  el comportamiento al probar.
- **Documentación**: es requisito de la fase (Nicolás lo pidió explícitamente). El plan
  debe incluir tareas dedicadas a `docs/BUILD.md`, `docs/DISTRIBUTION.md`, comentarios
  de los scripts y la sección del README — no tratarlas como opcionales.
