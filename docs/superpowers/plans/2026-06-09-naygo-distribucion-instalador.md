# Distribución / Instalador de Naygo — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Empaquetar Naygo para probar en otros entornos: ícono propio en el `.exe`+ventana, soporte de ruta por CLI, splash al arrancar, instalador Inno Setup, ZIP portable, script de build y documentación completa.

**Architecture:** Fase de empaquetado. Un módulo `core::cli` puro decide la carpeta inicial desde los args; `main.rs` lo invoca y carga el ícono de ventana; la app egui gana un estado `Splash` breve (solo release). El empaquetado vive fuera de los crates: `installer/naygo.iss` (Inno), `scripts/build-release.ps1` (orquestador), `docs/BUILD.md`+`docs/DISTRIBUTION.md`. La versión sale del `Cargo.toml` (fuente única).

**Tech Stack:** Rust, eframe/egui 0.34, `image` 0.25 (decodifica PNG/ICO, ya está en `ui`), `embed-resource` 3 (ya compila `app.rc`), Inno Setup (externo), PowerShell 5.1.

**Estado de partida (rama `feat/distribucion`, desde `main` d0cb235):**
- `crates/ui/Cargo.toml`: `[[bin]] name = "naygo"` (el binario es `naygo.exe`). `image = "0.25"` con feature `png` ya en deps. `embed-resource = "3"` en build-deps.
- `crates/ui/app.rc`: recurso de versión COMPLETO (CompanyName "ISGroth", ProductName "Naygo", FileDescription, LegalCopyright MIT, FileVersion/ProductVersion 0.1.0.0). NO referencia ícono.
- `crates/ui/build.rs`: `embed_resource::compile("app.rc", embed_resource::NONE).manifest_optional().unwrap();` dentro de `#[cfg(target_os="windows")]`.
- `crates/ui/src/main.rs`: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`; `main()` arma `eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_title("Naygo").with_inner_size([1100.0,700.0]), ..Default::default() }` y `eframe::run_native("Naygo", native_options, Box::new(|cc| Ok(Box::new(NaygoApp::new(cc)))))`. NO procesa `std::env::args()`. Tiene `install_panic_handler()`.
- `crates/ui/src/app.rs`: `NaygoApp::new(cc)` usa `let home = default_start_dir();` (USERPROFILE o `C:\`) → `load_or_default_workspace(&config_dir, &home)`. Existe `pub(crate) fn navigate_active_to(&mut self, path: PathBuf)` (app.rs:1593) y `pub fn start_listing(&mut self, id: PaneId, dir: PathBuf)` (app.rs:543). `NaygoApp` implementa `eframe::App` con `fn update(&mut self, ctx, frame)`.
- `crates/core/src/lib.rs`: declara los módulos del core (config, listing, icon_kind, icon_set, theme, sizing, keymap, clipboard, format, disk, cli NO existe aún).
- eframe/egui 0.34: `egui::IconData { rgba: Vec<u8>, width: u32, height: u32 }`; `ViewportBuilder::with_icon(impl Into<Arc<IconData>>)`.
- `LICENSE` (MIT, raíz) y `README.md` (raíz) existen. `.gitignore` NO excluye `dist/`.
- `assets/icons/naygo_icon.ico` (multi-res 16..256) y `assets/icons/logo_naygo.png` (1254×1254 RGB) existen.

**Prerequisito de entorno:** Rust en PATH (`export PATH="$HOME/.cargo/bin:$PATH"` en bash). NUNCA `2>&1` con cargo. `cargo fmt --all` antes de cada commit (drift recurrente). Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt verdes antes de cada commit. Header de 2 líneas en archivos nuevos de código. Inno Setup (`ISCC.exe`) puede NO estar instalado en la máquina del agente — el script debe degradar con aviso, no romper; la generación real del setup quizá no se verifique más allá de validar el `.iss`.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. `core` NUNCA importa egui/Windows ni usa `Instant` de wall-clock que rompa pureza (el splash y su tiempo viven en `ui`). Tolerante (ícono/logo que no carga → se omite, nunca rompe arranque). Footer de commits:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/distribucion`. NO cambiar de rama.

**DOCUMENTACIÓN = entregable de primera clase** (Nicolás lo pidió explícito). Las Tasks 8 y 9 son de documentación y NO son opcionales; además `naygo.iss` y `build-release.ps1` se comentan por bloque al crearlos.

**Reparto de verificación:** el agente compila, corre tests, genera artefactos y revisa que el `.iss`/`.ps1` hagan lo esperado. La **prueba visual** de arranque/instalación/uso en una VM limpia la hace Nicolás (GUI en otro entorno).

---

## Estructura de archivos

```
crates/core/src/cli.rs            # NUEVO: parse_initial_dir (puro, testeado)
crates/core/src/lib.rs            # + pub mod cli;
crates/ui/app.rc                  # + línea de recurso ICON
crates/ui/src/main.rs             # + arg de ruta + ícono de ventana
crates/ui/src/app.rs              # NaygoApp::new acepta carpeta inicial opcional + estado splash
crates/ui/src/splash.rs           # NUEVO: estado/painter del splash (release)
installer/naygo.iss               # NUEVO: script Inno (comentado por bloque)
installer/LEEME.txt               # NUEVO: nota del ZIP portable
scripts/build-release.ps1         # NUEVO: orquestador (comentado por bloque)
docs/BUILD.md                     # NUEVO
docs/DISTRIBUTION.md              # NUEVO
README.md                         # + sección "Instalación / Build"
.gitignore                        # + /dist
dist/                             # (gitignored) artefactos generados
```

---

## Task 1: `core::cli` — parse_initial_dir (puro, testeado)

**Files:**
- Create: `crates/core/src/cli.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear cli.rs con tests (TDD)**

Create `crates/core/src/cli.rs`:
```rust
// Naygo — parseo de argumentos de línea de comandos (carpeta inicial).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Decide la carpeta inicial de Naygo a partir de los argumentos. Soporta
//! `naygo.exe <ruta>`: si el primer argumento posicional es un directorio existente,
//! esa es la carpeta inicial; en cualquier otro caso (ausente, archivo, inexistente,
//! vacío) no hay override y la app arranca en su carpeta por defecto.

use std::path::{Path, PathBuf};

/// El primer argumento posicional, si lo hay (sin tocar disco). `args` son los
/// argumentos SIN el ejecutable (`&args[1..]`). Cadena vacía → `None`.
pub fn first_positional(args: &[String]) -> Option<&str> {
    args.iter().map(|s| s.as_str()).find(|s| !s.is_empty())
}

/// Resuelve la carpeta inicial: `Some(dir)` solo si el primer argumento es un
/// directorio existente. Validación de existencia mediante el predicado `is_dir`
/// (inyectable para test puro; en producción se pasa `|p| p.is_dir()`).
pub fn resolve_initial_dir(
    args: &[String],
    is_dir: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let candidate = first_positional(args)?;
    let path = PathBuf::from(candidate);
    if is_dir(&path) {
        Some(path)
    } else {
        None
    }
}

/// Atajo de producción: usa `Path::is_dir` real.
pub fn parse_initial_dir(args: &[String]) -> Option<PathBuf> {
    resolve_initial_dir(args, |p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_args_es_none() {
        assert_eq!(first_positional(&[]), None);
        assert_eq!(resolve_initial_dir(&[], |_| true), None);
    }

    #[test]
    fn arg_vacio_se_ignora() {
        let args = vec![String::new()];
        assert_eq!(first_positional(&args), None);
    }

    #[test]
    fn primer_arg_no_vacio() {
        let args = vec!["C:\\Users".to_string(), "ignorado".to_string()];
        assert_eq!(first_positional(&args), Some("C:\\Users"));
    }

    #[test]
    fn dir_existente_da_some() {
        let args = vec!["cualquier".to_string()];
        let got = resolve_initial_dir(&args, |_| true);
        assert_eq!(got, Some(PathBuf::from("cualquier")));
    }

    #[test]
    fn no_dir_da_none() {
        let args = vec!["cualquier".to_string()];
        assert_eq!(resolve_initial_dir(&args, |_| false), None);
    }

    #[test]
    fn dir_real_via_tempfile() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().to_string_lossy().to_string();
        let args = vec![p.clone()];
        assert_eq!(parse_initial_dir(&args), Some(PathBuf::from(p)));
    }

    #[test]
    fn archivo_no_es_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.txt");
        std::fs::write(&file, b"x").unwrap();
        let args = vec![file.to_string_lossy().to_string()];
        assert_eq!(parse_initial_dir(&args), None);
    }

    #[test]
    fn ruta_inexistente_es_none() {
        let args = vec!["Z:\\no\\existe\\naygo-xyz".to_string()];
        assert_eq!(parse_initial_dir(&args), None);
    }
}
```

- [ ] **Step 2: Declarar el módulo**

In `crates/core/src/lib.rs`, add `pub mod cli;` (junto a los otros `pub mod`).

- [ ] **Step 3: Verificar (rojo→verde)**

Run: `cargo test -p naygo-core cli` → 8 PASS. (Si `tempfile` no fuera dev-dep de core, lo es — ya lo usan los tests de config/icon_set.)
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 4: Commit**
```
git add crates/core/src/cli.rs crates/core/src/lib.rs
git commit -m "feat(core): cli::parse_initial_dir (carpeta inicial desde args)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `parse_initial_dir(&[String]) -> Option<PathBuf>`, `resolve_initial_dir`, `first_positional` EXACTOS (Task 3 los usa).

---

## Task 2: Ícono en el `.exe` (app.rc)

**Files:**
- Modify: `crates/ui/app.rc`

- [ ] **Step 1: Agregar el recurso de ícono**

Add at the TOP of `crates/ui/app.rc`, BEFORE the `#include <winver.h>` line (the first `ICON` statement in a `.rc` is the one Windows uses for the `.exe`):
```
// Ícono de la aplicación. El primer recurso ICON del .rc es el que Windows usa
// para el .exe (Explorer, taskbar, Alt-Tab, accesos directos). Ruta relativa a
// este archivo (crates/ui/), sube a la raíz del repo a assets/icons/.
IDI_NAYGO ICON "../../assets/icons/naygo_icon.ico"

#include <winver.h>
```
(VERIFY the relative path resolves: the resource compiler resolves paths relative to the `.rc` file's directory = `crates/ui/`. `../../assets/icons/naygo_icon.ico` → repo-root `assets/icons/`. If `embed-resource` resolves relative to the crate manifest or OUT_DIR instead and the build fails to find the icon, adjust the path until `cargo build -p naygo-ui` succeeds — the build error names the missing path.)

- [ ] **Step 2: Verificar build con el recurso**

Run: `cargo build -p naygo-ui` → compiles (resource compiler embeds the icon). If it errors with "cannot open file ...naygo_icon.ico", fix the relative path and rebuild.
NOTE: verifying the icon visually on the `.exe` (Explorer/Properties) is for the release build (Task 7 / Nicolás). Here we only confirm the build embeds it without error.

- [ ] **Step 3: Commit**
```
git add crates/ui/app.rc
git commit -m "build(ui): ícono de la app en el .exe (naygo_icon.ico)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: main.rs — ruta por CLI + ícono de ventana

**Files:**
- Modify: `crates/ui/src/main.rs`
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: NaygoApp acepta carpeta inicial opcional**

Modify `crates/ui/src/app.rs`. Change `NaygoApp::new(cc)` to `NaygoApp::new(cc, initial_dir: Option<std::path::PathBuf>)`. Inside, AFTER the app is fully built (después de construir `app` y antes de devolverlo), si hay `initial_dir`, navega el panel activo:
```rust
        // Carpeta inicial por línea de comandos (naygo.exe <ruta>): si se pasó una
        // carpeta válida, el panel activo se navega ahí; si no, queda el arranque normal.
        if let Some(dir) = initial_dir {
            app.navigate_active_to(dir);
        }
        app
```
(Find the existing `... app` return at the end of `new` and insert the override just before it. `navigate_active_to(PathBuf)` already exists at app.rs:1593.)

- [ ] **Step 2: main.rs invoca parse_initial_dir y pasa el resultado**

Modify `crates/ui/src/main.rs` `main()`:
```rust
    let args: Vec<String> = std::env::args().collect();
    let initial_dir = naygo_core::cli::parse_initial_dir(&args.get(1..).unwrap_or(&[]).to_vec());
    if let Some(d) = &initial_dir {
        tracing::info!("carpeta inicial por CLI: {}", d.display());
    }
```
And change the closure:
```rust
        Box::new(move |cc| Ok(Box::new(NaygoApp::new(cc, initial_dir.clone())))),
```
(The closure must be `move` to capture `initial_dir`; `initial_dir.clone()` so the closure can be `FnOnce`/`FnMut` as eframe needs. If eframe's signature wants `FnOnce`, a plain move of `initial_dir` is fine without clone — adapt so it compiles.)
NOTE: an invalid/missing path → `parse_initial_dir` returns `None` → normal startup. Never panics.

- [ ] **Step 3: Ícono de ventana (eframe)**

In `main.rs`, add a helper that loads the window icon from the embedded PNG logo or the `.ico`, tolerant:
```rust
/// Carga el ícono de la ventana desde el PNG del logo embebido. Si falla, devuelve
/// None (la app arranca igual, sin ícono de ventana — tolerante).
fn load_window_icon() -> Option<egui::IconData> {
    // Reusa el logo; decodifica a RGBA con el crate `image` (ya es dep de ui).
    let bytes = include_bytes!("../../../assets/icons/logo_naygo.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = (img.width(), img.height());
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}
```
(VERIFY the relative path `../../../assets/icons/logo_naygo.png` from `crates/ui/src/main.rs` → repo-root `assets/icons/`. `crates/ui/src/` → up 3 = repo root. Confirm with the build; the existing `assets.rs` uses `../../../../assets/...` from `crates/ui/src/icons/` so from `crates/ui/src/` it's one fewer `../`.)
Wire it into the viewport:
```rust
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Naygo")
        .with_inner_size([1100.0, 700.0]);
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
```
(`with_icon` takes `impl Into<Arc<IconData>>`; `Arc::new(icon)` satisfies it. Verify it compiles.)

- [ ] **Step 4: Verificar**

Run: `cargo build -p naygo-ui` → compiles. Run: `cargo test --workspace` → green. Run: `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.

- [ ] **Step 5: Commit**
```
git add crates/ui/src/main.rs crates/ui/src/app.rs
git commit -m "feat(ui): naygo.exe <ruta> abre esa carpeta + ícono de ventana

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `NaygoApp::new(cc, Option<PathBuf>)` EXACTO (Task 4 lo toca de nuevo).

---

## Task 4: Splash screen (solo release)

**Files:**
- Create: `crates/ui/src/splash.rs`
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/main.rs` (declarar `mod splash;`)

- [ ] **Step 1: Crear splash.rs**

Create `crates/ui/src/splash.rs`:
```rust
// Naygo — splash de arranque: muestra el logo brevemente al iniciar (solo release).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Estado liviano que pinta el logo de Naygo centrado durante el arranque. Se cierra
//! solo tras `MAX_VISIBLE` o cuando la UI ya está lista (lo que pase primero), y
//! también ante cualquier clic o tecla. No frena el arranque: como Naygo arranca
//! rápido, en la práctica es un destello corto. Tolerante: si el logo no carga, no
//! hay splash.

use std::time::{Duration, Instant};

/// Tiempo máximo visible del splash.
const MAX_VISIBLE: Duration = Duration::from_millis(1200);

/// Estado del splash mientras está activo.
pub struct Splash {
    texture: egui::TextureHandle,
    started: Instant,
}

impl Splash {
    /// Crea el splash si el logo decodifica; si no, `None` (no hay splash).
    pub fn new(ctx: &egui::Context) -> Option<Self> {
        let bytes = include_bytes!("../../../assets/icons/logo_naygo.png");
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        let texture = ctx.load_texture("naygo_splash", color, egui::TextureOptions::LINEAR);
        Some(Splash {
            texture,
            started: Instant::now(),
        })
    }

    /// Pinta el splash y devuelve `true` si debe seguir visible. Devuelve `false`
    /// cuando expira el tiempo o hubo input (clic/tecla).
    pub fn show(&self, ctx: &egui::Context) -> bool {
        let dismissed_by_input = ctx.input(|i| {
            i.pointer.any_click() || !i.events.is_empty() && i.keys_down.iter().next().is_some()
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                let max = ui.available_size() * 0.6;
                let img = egui::Image::new(&self.texture).max_size(max);
                ui.add(img);
            });
        });
        // Repaint para que el tiempo avance aunque no haya input.
        ctx.request_repaint();
        let expired = self.started.elapsed() >= MAX_VISIBLE;
        !(expired || dismissed_by_input)
    }
}
```
(VERIFY egui 0.34: `egui::Image::new(&TextureHandle).max_size(vec2)`, `ui.centered_and_justified`, `ctx.input(|i| i.pointer.any_click())`, `i.keys_down`. Adapt the input-dismiss check to whatever compiles cleanly — the intent: any click or key dismisses. If `keys_down` iteration is awkward, use `i.events.iter().any(|e| matches!(e, egui::Event::Key{..}))`.)

- [ ] **Step 2: Integrar el splash en NaygoApp (solo release)**

Modify `crates/ui/src/app.rs`:
a) Add a field to `NaygoApp`: `splash: Option<crate::splash::Splash>,`.
b) In `NaygoApp::new`, initialize it ONLY in release:
```rust
        // Splash de arranque solo en release (en debug estorba el desarrollo).
        #[cfg(debug_assertions)]
        let splash = None;
        #[cfg(not(debug_assertions))]
        let splash = crate::splash::Splash::new(&cc.egui_ctx);
```
Add `splash,` to the struct literal (after the other fields). Apply the `initial_dir` navigation (Task 3) BEFORE returning `app` as before.
c) In `impl eframe::App for NaygoApp`'s `update`, at the very TOP, render the splash and short-circuit while active:
```rust
        // Mientras el splash esté activo, lo pintamos y NO dibujamos la UI todavía.
        if let Some(splash) = &self.splash {
            let keep = splash.show(ctx);
            if !keep {
                self.splash = None;
            }
            return;
        }
```
(This makes the splash own the first frame(s); once it returns false, `self.splash` is cleared and subsequent frames render the real UI. Place this BEFORE any other `update` body work. The splash naturally disappears "when the UI is ready or after 1.2s" — since the real UI renders the frame after dismissal, and 1.2s caps it.)

- [ ] **Step 3: Declarar el módulo**

In `crates/ui/src/main.rs`, add `mod splash;` with the other `mod` declarations.

- [ ] **Step 4: Verificar**

Run: `cargo build -p naygo-ui` (debug — splash None, compila). Run: `cargo build --release -p naygo-ui` (release — splash activo, compila). Run: `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.
NOTE: la verificación VISUAL del splash (que aparezca y se vaya) es del release corrido por Nicolás; aquí confirmamos que compila en ambos perfiles y no rompe tests.

- [ ] **Step 5: Commit**
```
git add crates/ui/src/splash.rs crates/ui/src/app.rs crates/ui/src/main.rs
git commit -m "feat(ui): splash de arranque con el logo (breve, solo release)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: .gitignore — excluir dist/

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: Agregar /dist**

Append to `.gitignore`:
```
# Artefactos de empaquetado generados (ZIP portable, setup.exe). No se versionan.
/dist
```

- [ ] **Step 2: Commit**
```
git add .gitignore
git commit -m "chore: ignorar dist/ (artefactos de empaquetado)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Instalador Inno Setup (naygo.iss)

**Files:**
- Create: `installer/naygo.iss`

- [ ] **Step 1: Crear naygo.iss (comentado por bloque)**

Create `installer/naygo.iss`. La versión llega por `/DMyAppVersion=...` desde el script de build (Task 7); aquí se define un default por si se compila el `.iss` a mano.
```
; Naygo — script de Inno Setup. Genera el instalador (setup.exe).
; Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
;
; La versión se inyecta desde scripts/build-release.ps1 con /DMyAppVersion=...
; (fuente única: el Cargo.toml del workspace). El default de abajo es solo para
; compilar el .iss a mano sin el script.

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#define MyAppName "Naygo"
#define MyAppPublisher "ISGroth"
#define MyAppURL "https://github.com/nicolasgroth/explorador_archivos_naygo"
#define MyAppExe "naygo.exe"

[Setup]
; AppId fijo: identifica el producto para upgrades/desinstalación (NO cambiar entre versiones).
AppId={{B7E6A4C2-1F3D-4E9B-9C2A-NAYGO0000001}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
; Modo elegible: el asistente pregunta "para mí" (sin admin) o "para todos" (admin).
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\dist
OutputBaseFilename=Naygo-{#MyAppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Imágenes del asistente (BMP generados desde logo_naygo.png por el script de build).
WizardImageFile=wizard-large.bmp
WizardSmallImageFile=wizard-small.bmp
LicenseFile=..\LICENSE
SetupIconFile=..\assets\icons\naygo_icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "openwith"; Description: "Registrar Naygo en 'Abrir con' para carpetas"; Flags: unchecked
Name: "ctxmenu"; Description: "Agregar 'Abrir en Naygo' al menú contextual de carpetas"; Flags: unchecked

[Files]
; Único ejecutable (CRT estático + assets embebidos), licencia y readme.
Source: "..\target\release\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{group}\Desinstalar {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Registry]
; "Abrir con" (NO predeterminado): registra el ProgId y lo lista para carpetas.
Root: HKA; Subkey: "Software\Classes\Naygo.Folder"; ValueType: string; ValueData: "Carpeta en Naygo"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Naygo.Folder\shell\open\command"; ValueType: string; ValueData: """{app}\{#MyAppExe}"" ""%1"""; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Directory\OpenWithProgids"; ValueType: string; ValueName: "Naygo.Folder"; ValueData: ""; Flags: uninsdeletevalue; Tasks: openwith
; Menú contextual "Abrir en Naygo" en carpetas y en el fondo de carpeta.
Root: HKA; Subkey: "Software\Classes\Directory\shell\Naygo"; ValueType: string; ValueData: "Abrir en Naygo"; Flags: uninsdeletekey; Tasks: ctxmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\Naygo"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\{#MyAppExe}"; Tasks: ctxmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\Naygo\command"; ValueType: string; ValueData: """{app}\{#MyAppExe}"" ""%V"""; Flags: uninsdeletekey; Tasks: ctxmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\Naygo"; ValueType: string; ValueData: "Abrir en Naygo"; Flags: uninsdeletekey; Tasks: ctxmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\Naygo\command"; ValueType: string; ValueData: """{app}\{#MyAppExe}"" ""%V"""; Flags: uninsdeletekey; Tasks: ctxmenu

[Run]
; Página final: ofrecer ejecutar Naygo.
Filename: "{app}\{#MyAppExe}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent
```
NOTE on `HKA`: Inno's `HKA` (HKEY_AUTO) maps to HKCU for per-user installs and HKLM for all-users — correct for the elegible mode. The registry keys use `Software\Classes` so they work under both.
NOTE: this `.iss` references `wizard-large.bmp` / `wizard-small.bmp` in the `installer/` dir — the build script (Task 7) generates them from `logo_naygo.png` before calling ISCC. If absent, ISCC errors; the build script must generate them first.

- [ ] **Step 2: Validar sintaxis si ISCC está disponible (tolerante)**

Run (PowerShell): `if (Get-Command ISCC.exe -ErrorAction SilentlyContinue) { ISCC.exe /? } else { "ISCC no instalado — se valida en build-release.ps1" }`.
NOTE: si `ISCC.exe` no está en la máquina del agente, NO se puede compilar el `.iss` aquí; se deja documentado y se valida en Task 7 / por Nicolás. No bloquea el commit.

- [ ] **Step 3: Commit**
```
git add installer/naygo.iss
git commit -m "build: script de instalador Inno Setup (naygo.iss)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: ZIP portable (LEEME) + script de build

**Files:**
- Create: `installer/LEEME.txt`
- Create: `scripts/build-release.ps1`

- [ ] **Step 1: LEEME.txt del portable**

Create `installer/LEEME.txt`:
```
Naygo — versión portable
=========================

Naygo es un explorador de archivos rápido para Windows 10/11.
Copyright (c) 2026 Nicolás Groth / ISGroth. Licencia MIT.

Cómo usar
---------
1. Descomprime este ZIP donde quieras (un pendrive, una carpeta, el escritorio).
2. Ejecuta naygo.exe haciendo doble clic. No requiere instalación.

Notas
-----
- La configuración se guarda JUNTO a naygo.exe (modo portable). Si mueves el .exe,
  llévate también los archivos de configuración que crea a su lado.
- También puedes abrir una carpeta directamente:  naygo.exe "C:\ruta\a\carpeta"
- La primera vez, Windows SmartScreen puede advertir "editor desconocido" (el .exe
  no está firmado). Haz clic en "Más información" y luego en "Ejecutar de todos modos".

Más información: https://github.com/nicolasgroth/explorador_archivos_naygo
```

- [ ] **Step 2: build-release.ps1 (comentado por bloque)**

Create `scripts/build-release.ps1`:
```powershell
# Naygo - orquestador de empaquetado: compila release, arma el ZIP portable y
# (si Inno Setup esta instalado) genera el instalador.
# Copyright (c) 2026 Nicolas Groth / ISGroth. MIT License.
#
# Uso:  powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
# Prerequisitos: Rust (toolchain MSVC). Inno Setup (ISCC.exe) opcional: si falta,
# se genera solo el ZIP portable y se avisa.

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot           # raiz del repo (scripts/ esta un nivel abajo)
$dist = Join-Path $repo "dist"

# --- 1. Version: fuente unica = workspace.package.version del Cargo.toml raiz ---
$cargoToml = Get-Content (Join-Path $repo "Cargo.toml") -Raw
if ($cargoToml -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
    throw "No pude leer la version del Cargo.toml raiz."
}
$version = $Matches[1]
Write-Host "Naygo version $version"

# --- 2. Compilar release ---
Write-Host "Compilando release..."
& cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build --release fallo." }
$exe = Join-Path $repo "target\release\naygo.exe"
if (-not (Test-Path $exe)) { throw "No se encontro $exe tras compilar." }

# --- 3. Preparar dist/ ---
if (-not (Test-Path $dist)) { New-Item -ItemType Directory -Path $dist | Out-Null }

# --- 4. ZIP portable: naygo.exe + LICENSE + LEEME.txt ---
Write-Host "Armando ZIP portable..."
$stage = Join-Path $dist "portable-stage"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item $exe (Join-Path $stage "naygo.exe")
Copy-Item (Join-Path $repo "LICENSE") (Join-Path $stage "LICENSE")
Copy-Item (Join-Path $repo "installer\LEEME.txt") (Join-Path $stage "LEEME.txt")
$zip = Join-Path $dist "Naygo-$version-portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip
Remove-Item -Recurse -Force $stage
Write-Host "Portable: $zip"

# --- 5. Imagenes del asistente: BMP desde logo_naygo.png (Inno consume BMP) ---
# Usa System.Drawing para redimensionar. Tamanos tipicos de Inno: 164x314 y 55x58.
Add-Type -AssemblyName System.Drawing
function Convert-LogoToBmp([string]$dst, [int]$w, [int]$h) {
    $src = Join-Path $repo "assets\icons\logo_naygo.png"
    $img = [System.Drawing.Image]::FromFile($src)
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.Clear([System.Drawing.Color]::White)
    # Encaja el logo cuadrado centrado dentro del area.
    $side = [Math]::Min($w, $h)
    $x = [int](($w - $side) / 2); $y = [int](($h - $side) / 2)
    $g.DrawImage($img, $x, $y, $side, $side)
    $g.Dispose(); $img.Dispose()
    $bmp.Save($dst, [System.Drawing.Imaging.ImageFormat]::Bmp)
    $bmp.Dispose()
}
$wizLarge = Join-Path $repo "installer\wizard-large.bmp"
$wizSmall = Join-Path $repo "installer\wizard-small.bmp"
Convert-LogoToBmp $wizLarge 164 314
Convert-LogoToBmp $wizSmall 55 58
Write-Host "Imagenes del asistente generadas."

# --- 6. Instalador Inno (opcional): solo si ISCC.exe esta disponible ---
$iscc = Get-Command ISCC.exe -ErrorAction SilentlyContinue
if ($null -eq $iscc) {
    Write-Warning "Inno Setup (ISCC.exe) no encontrado en PATH."
    Write-Warning "Se genero solo el ZIP portable. Para el instalador, instala Inno Setup:"
    Write-Warning "  https://jrsoftware.org/isdl.php"
    Write-Warning "y volve a correr este script."
} else {
    Write-Host "Generando instalador con Inno Setup..."
    & $iscc.Source "/DMyAppVersion=$version" (Join-Path $repo "installer\naygo.iss")
    if ($LASTEXITCODE -ne 0) { throw "ISCC fallo al compilar el instalador." }
    Write-Host "Instalador: $dist\Naygo-$version-setup.exe"
}

Write-Host "Listo. Artefactos en: $dist"
```
NOTE: las BMP del wizard (`installer/wizard-*.bmp`) son artefactos GENERADOS — NO se versionan. Agregar `installer/wizard-large.bmp` y `installer/wizard-small.bmp` al `.gitignore` (o `installer/*.bmp`).

- [ ] **Step 3: Añadir las BMP generadas al .gitignore**

Append to `.gitignore`:
```
# Imágenes del asistente del instalador, generadas desde logo_naygo.png. No versionar.
installer/*.bmp
```

- [ ] **Step 4: Verificar el script (parcial, tolerante)**

Run (PowerShell, desde la raíz):
```
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```
Expected: compila release, crea `dist/Naygo-0.1.0-portable.zip`, genera las BMP, y o bien crea el setup (si hay Inno) o avisa que falta Inno (sin fallar). VERIFY: `dist/Naygo-0.1.0-portable.zip` existe y contiene `naygo.exe`+`LICENSE`+`LEEME.txt` (`Expand-Archive` a un temp para inspeccionar, o `[System.IO.Compression.ZipFile]`). Si el release ya estaba compilado, el paso 2 es rápido.
NOTE: si System.Drawing no estuviera disponible (Server Core), el paso 5 fallaría — en ese caso documentar y considerar versionar las BMP a mano. En Windows 10/11 normal está disponible.

- [ ] **Step 5: Commit (NO incluir dist/ ni las BMP)**

VERIFY `git status`: `dist/` y `installer/*.bmp` NO aparecen (gitignored). Solo el script + LEEME + .gitignore.
```
git add scripts/build-release.ps1 installer/LEEME.txt .gitignore
git commit -m "build: orquestador build-release.ps1 + LEEME del portable

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Documentación — BUILD.md y DISTRIBUTION.md

**Files:**
- Create: `docs/BUILD.md`
- Create: `docs/DISTRIBUTION.md`

- [ ] **Step 1: docs/BUILD.md**

Create `docs/BUILD.md`:
```markdown
# Compilar y empaquetar Naygo

## Prerequisitos

- **Rust** (toolchain MSVC, `x86_64-pc-windows-msvc`). Instalar desde
  <https://rustup.rs>. El proyecto usa CRT estático (`.cargo/config.toml`), por lo que
  el `.exe` resultante corre en equipos limpios sin el "Visual C++ Redistributable".
- **Inno Setup** (opcional, solo para generar el instalador): descargar de
  <https://jrsoftware.org/isdl.php>. Tras instalar, asegurate de que `ISCC.exe` esté
  en el `PATH` (o ejecutá el script desde la consola "Inno Setup" / agregá su carpeta,
  típicamente `C:\Program Files (x86)\Inno Setup 6`, al `PATH`).

## Compilar (desarrollo)

```
cargo build            # debug
cargo run -p naygo-ui  # corre Naygo
cargo test --workspace # tests
```

## Compilar release + empaquetar

Un solo comando arma todo:

```
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

Qué hace, en orden:

1. Lee la versión del `Cargo.toml` raíz (fuente única de verdad).
2. `cargo build --release` → `target\release\naygo.exe` (con ícono, metadatos de
   autoría y CRT estático).
3. Genera `dist\Naygo-<versión>-portable.zip` (`naygo.exe` + `LICENSE` + `LEEME.txt`).
4. Genera las imágenes del asistente del instalador (`installer\wizard-*.bmp`) desde
   `assets\icons\logo_naygo.png`.
5. Si `ISCC.exe` está disponible, genera `dist\Naygo-<versión>-setup.exe`. Si no,
   avisa (con el link de descarga) y deja igual el ZIP portable.

## Artefactos (en `dist\`, no versionado)

| Archivo | Qué es |
|---|---|
| `Naygo-<versión>-portable.zip` | Versión portable: descomprimir y ejecutar, sin instalar. |
| `Naygo-<versión>-setup.exe` | Instalador (asistente, accesos directos, desinstalador). |

## Troubleshooting

- **"ISCC.exe no encontrado"**: Inno Setup no está instalado o no está en el `PATH`.
  Instalalo (link arriba) y reintentá; el ZIP portable se genera igual sin Inno.
- **El `.exe` no muestra el ícono**: rehacé `cargo build --release` (el ícono se
  embebe vía `crates/ui/app.rc`). Explorer cachea íconos; probá en otra carpeta.
- **Error al generar las BMP del asistente**: el script usa `System.Drawing` de .NET;
  en Windows 10/11 normal está disponible. En ediciones recortadas (Server Core),
  generá las BMP a mano y volvé a correr.
- **Falla `cargo build`**: confirmá el toolchain MSVC (`rustup default
  stable-x86_64-pc-windows-msvc`).
```

- [ ] **Step 2: docs/DISTRIBUTION.md**

Create `docs/DISTRIBUTION.md`:
```markdown
# Distribución de Naygo

Naygo se distribuye de dos formas, generadas por `scripts\build-release.ps1`
(ver [BUILD.md](BUILD.md)).

## Portable (ZIP)

`Naygo-<versión>-portable.zip` contiene `naygo.exe`, `LICENSE` y `LEEME.txt`.
Descomprimir y ejecutar — no instala nada. **La configuración se guarda junto al
`.exe`** (modo portable): si movés el ejecutable, llevate los archivos de config que
crea a su lado. Ideal para probar rápido en una VM o llevar en un pendrive.

## Instalador (setup.exe)

`Naygo-<versión>-setup.exe` es un asistente (Inno Setup). Ofrece:

- **Modo de instalación**: "para mí" (sin permisos de administrador, instala en
  `%LocalAppData%\Programs\Naygo`) o "para todos" (requiere administrador, instala en
  `C:\Program Files\Naygo`). El asistente lo pregunta.
- **Accesos directos**: en el menú Inicio siempre; en el Escritorio si marcás la opción.
- **"Abrir con" (opcional)**: registra a Naygo en el menú "Abrir con" de carpetas, sin
  hacerlo predeterminado (no toca Win+E ni reemplaza el Explorador).
- **"Abrir en Naygo" (opcional)**: agrega una entrada al menú contextual (clic derecho)
  de carpetas y del fondo de carpeta, que abre esa carpeta en Naygo.
- **Ejecutar al terminar**: opción en la última página.

### Qué escribe en el sistema

- Archivos: el `.exe`, `LICENSE` y `README.md` en la carpeta de instalación.
- Accesos directos: menú Inicio (y Escritorio si se eligió).
- Registro (solo si marcaste las opciones): claves bajo `Software\Classes` (en HKCU
  para "para mí", HKLM para "para todos") para "Abrir con" y el menú contextual.

### Desinstalar

Desde "Agregar o quitar programas" (o el acceso directo "Desinstalar Naygo"). Elimina
el ejecutable, los accesos directos y las claves de registro creadas. **No** borra la
configuración del usuario (queda en su ubicación; podés borrarla a mano si querés un
reinicio total).

## Advertencia de SmartScreen (importante)

El `.exe` y el instalador **no están firmados** (no hay certificado de firma de código
por ahora). La primera vez que los ejecutés, Windows SmartScreen puede mostrar:

> "Windows protegió tu PC — Editor desconocido"

Esto es normal en software open-source sin firma. Para continuar:

1. Hacé clic en **"Más información"**.
2. Hacé clic en **"Ejecutar de todos modos"**.

Naygo es open source (MIT); podés revisar el código en
<https://github.com/nicolasgroth/explorador_archivos_naygo>.
```

- [ ] **Step 3: Commit**
```
git add docs/BUILD.md docs/DISTRIBUTION.md
git commit -m "docs: BUILD.md y DISTRIBUTION.md (compilar, empaquetar, instalar, SmartScreen)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: README — sección Instalación / Build + cierre

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Actualizar estado + agregar sección**

In `README.md`, replace the status block:
```markdown
> **Estado:** Fase distribución/instalador (instalador + portable + ícono + splash) en
> desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-09-naygo-distribucion-instalador-design.md`](docs/superpowers/specs/2026-06-09-naygo-distribucion-instalador-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-09-naygo-distribucion-instalador.md`](docs/superpowers/plans/2026-06-09-naygo-distribucion-instalador.md).
> Operaciones (ops-A/B), paste, shell-A, watcher, atajos, sizing (F3), toolbar-icons y
> bloque visual completos.
```
And add a new section BEFORE `## Licencia`:
```markdown
## Instalación / Build

Para usar Naygo:

- **Portable**: descargá `Naygo-<versión>-portable.zip`, descomprimí y ejecutá
  `naygo.exe`. No instala nada.
- **Instalador**: ejecutá `Naygo-<versión>-setup.exe` y seguí el asistente.

La primera vez, Windows SmartScreen puede advertir "editor desconocido" (el `.exe` no
está firmado): hacé clic en **"Más información" → "Ejecutar de todos modos"**.

Para compilar y empaquetar desde el código, ver
[`docs/BUILD.md`](docs/BUILD.md) y [`docs/DISTRIBUTION.md`](docs/DISTRIBUTION.md).
```

- [ ] **Step 2: Verificación final del workspace**

Run: `cargo build --workspace` → compiles. `cargo build --release -p naygo-ui` → release compila. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all -- --check` → clean.
VERIFY `git status`: `dist/` e `installer/*.bmp` NO trackeados.

- [ ] **Step 3: Commit y push**
```
git add README.md
git commit -m "docs: README sección Instalación / Build + estado de la fase

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/distribucion
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| Ícono en el `.exe` (app.rc) | 2 |
| Ícono de ventana (eframe IconData) | 3 |
| `core::cli::parse_initial_dir` (puro, testeado) | 1 |
| main.rs usa el arg + navega el panel | 3 |
| Splash breve, solo release, tolerante | 4 |
| Instalador Inno (modo elegible, accesos, wizard, tareas, desinstalador) | 6 |
| "Abrir con" + menú contextual "Abrir en Naygo" | 6 |
| Versión inyectada desde Cargo.toml | 7 (script) + 6 (`/DMyAppVersion`) |
| ZIP portable + LEEME | 7 |
| Script build-release.ps1 (degrada sin Inno) | 7 |
| BMP del wizard desde el logo | 7 |
| docs/BUILD.md + docs/DISTRIBUTION.md | 8 |
| README sección Instalación / Build | 9 |
| Scripts comentados por bloque | 6, 7 (en el contenido) |
| .gitignore excluye dist/ + BMP | 5, 7 |
| Nota SmartScreen | 7 (LEEME), 8 (DISTRIBUTION), 9 (README) |

**Notas de riesgo (recordatorio para el implementador):**
- **Ruta del ícono en `app.rc`** (Task 2): el compilador de recursos resuelve relativo al `.rc` (`crates/ui/`). Si `embed-resource` falla al hallar el `.ico`, ajustar la ruta hasta que `cargo build -p naygo-ui` compile (el error nombra el path).
- **Rutas `include_bytes!`** (Tasks 3,4): desde `crates/ui/src/main.rs`/`splash.rs` → `../../../assets/icons/...` (tres niveles a la raíz). Verificar contra el build; el `assets.rs` de íconos usa cuatro `../` porque está un nivel más adentro (`crates/ui/src/icons/`).
- **API eframe 0.34** (Task 3): `egui::IconData{rgba,width,height}` + `ViewportBuilder::with_icon(impl Into<Arc<IconData>>)` confirmados en la fuente. `image::load_from_memory(...).to_rgba8().into_raw()` da el `Vec<u8>` RGBA.
- **Splash input-dismiss** (Task 4): adaptar el chequeo de clic/tecla a lo que compile limpio en egui 0.34 (`i.pointer.any_click()`, `i.events`/`Event::Key`). El cap de 1.2s es la garantía dura.
- **Inno headless** (Tasks 6,7): `ISCC.exe` puede faltar en la máquina del agente → el script degrada con aviso, NO rompe; el setup quizá no se verifique más allá de validar el `.iss`. Documentado.
- **BMP del wizard** (Task 7): generadas por el script con `System.Drawing`; NO se versionan (gitignored). Si la generación fallara en un entorno recortado, versionarlas a mano es aceptable.
- **`dist/` y `installer/*.bmp` NUNCA se commitean** — verificar `git status` en Tasks 7 y 9.
- **PowerShell 5.1** (Task 7): el script usa `$ErrorActionPreference="Stop"` + `throw`; sin `&&`; `Compress-Archive`/`System.Drawing` nativos. No redirige stderr de cargo.
```
