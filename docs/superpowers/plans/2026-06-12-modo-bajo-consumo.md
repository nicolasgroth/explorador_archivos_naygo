# Modo de bajo consumo / render sin GPU — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reducir el consumo de CPU de Naygo durante el uso (hover/scroll) y hacerlo
viable en equipos sin GPU dedicada: hover a ~30fps para todos, detección de render por
software, modo de bajo consumo configurable (Auto/Siempre/Nunca) que apaga animaciones,
y pregunta de bienvenida en el primer arranque.

**Architecture:** Rust 3 capas. La clasificación de "¿renderer es software?" y el enum
`LowPowerMode` viven en `core` (puros, testeables). La lectura del `GL_RENDERER` y la
aplicación del modo (animation_time, framerate del hover) viven en `ui`. La pregunta de
bienvenida es un modal de egui en el primer arranque.

**Tech Stack:** Rust, egui/eframe 0.34.3 (reexporta `glow`), serde.

---

## Reglas operativas (NO te las saltes)

1. **Puertas antes de CADA commit**: `cargo test --workspace` (lee TODAS las líneas
   `test result:`), `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo fmt --all`.
2. **`cargo build -p naygo-ui`** antes de cualquier prueba en vivo; **mata `naygo.exe`**
   antes de compilar (`Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue`).
3. **i18n ES+EN en paridad SIEMPRE** (hay un test que lo verifica).
4. **Commits en español** con heredoc de Bash (`git commit -F - <<'EOF' … EOF`) para
   evitar el bug del `@` de PowerShell. **Stagea rutas explícitas** (`crates/`, `docs/`),
   NO `git add -A` (hay un cambio ajeno en `CLAUDE.md` que no debe entrar).
5. Header de copyright en archivos nuevos; comentarios en español del PORQUÉ.

## Estructura de archivos

- `crates/core/src/render_hint.rs` — NUEVO: `is_software_renderer` (puro + tests).
- `crates/core/src/lib.rs` — registrar `pub mod render_hint;`.
- `crates/core/src/config/mod.rs` — `LowPowerMode` + `Settings.low_power_mode`.
- `crates/ui/src/app.rs` — campo `software_render`, `show_welcome`; leer GL_RENDERER;
  `low_power_active()`; aplicar `animation_time`; hover a 30fps; modal de bienvenida.
- `crates/ui/src/settings_window/advanced.rs` — selector Auto/Siempre/Nunca.
- `crates/ui/src/welcome.rs` — NUEVO: el diálogo de bienvenida.
- `crates/ui/src/main.rs` — registrar `mod welcome;`.
- `crates/core/src/i18n/{es,en}.json` — claves del setting + bienvenida.

---

### Tarea 1 — core: detección de render por software (puro)

**Files:**
- Create: `crates/core/src/render_hint.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear el módulo con la función y sus tests**

Crear `crates/core/src/render_hint.rs`:

```rust
// Naygo — heurística pura: ¿el renderer OpenGL es por software? (sin egui/Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! La capa `ui` lee el nombre del renderer (`GL_RENDERER`) del contexto glow; esta
//! función PURA decide si es un rasterizador por software (sin GPU). Se usa para activar
//! el modo de bajo consumo en `Auto`. Testeable sin egui.

/// `true` si el nombre del renderer corresponde a un rasterizador por SOFTWARE (sin GPU
/// real): llvmpipe (Mesa), SwiftShader, el "Microsoft Basic Render Driver"/"GDI Generic"
/// de Windows, o softpipe. La comparación es insensible a mayúsculas.
pub fn is_software_renderer(renderer_name: &str) -> bool {
    let n = renderer_name.to_lowercase();
    [
        "llvmpipe",
        "softpipe",
        "software",
        "swiftshader",
        "microsoft basic render",
        "gdi generic",
    ]
    .iter()
    .any(|m| n.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_software() {
        assert!(is_software_renderer("llvmpipe (LLVM 15.0.7, 256 bits)"));
        assert!(is_software_renderer("SwiftShader Device"));
        assert!(is_software_renderer("Microsoft Basic Render Driver"));
        assert!(is_software_renderer("GDI Generic"));
        assert!(is_software_renderer("softpipe"));
    }

    #[test]
    fn no_marca_gpu_real() {
        assert!(!is_software_renderer("NVIDIA GeForce RTX 3060/PCIe/SSE2"));
        assert!(!is_software_renderer("Intel(R) UHD Graphics 620"));
        assert!(!is_software_renderer("AMD Radeon RX 580"));
        assert!(!is_software_renderer("ANGLE (Intel, Direct3D11)"));
    }

    #[test]
    fn case_insensitive() {
        assert!(is_software_renderer("LLVMPIPE"));
    }
}
```

- [ ] **Step 2: Registrar el módulo en lib.rs**

En `crates/core/src/lib.rs`, agregar junto a los otros `pub mod` (orden alfabético, tras
`pub mod recent_dirs;` o donde calce):

```rust
pub mod render_hint;
```

- [ ] **Step 3: Run tests**

Run (PowerShell): `cargo test -p naygo-core render_hint 2>&1 | Select-String "test result|FAILED"`
Expected: `test result: ok` (3 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/render_hint.rs crates/core/src/lib.rs
git commit -F - <<'EOF'
feat(core): heuristica pura is_software_renderer para detectar render sin GPU

Decide si un nombre de GL_RENDERER es un rasterizador por software (llvmpipe,
swiftshader, microsoft basic render, gdi generic, softpipe). La capa ui lo usa para
activar el modo de bajo consumo en Auto. Tests incluidos.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 2 — core: enum LowPowerMode + setting

**Files:**
- Modify: `crates/core/src/config/mod.rs`

- [ ] **Step 1: Agregar el enum LowPowerMode**

En `crates/core/src/config/mod.rs`, cerca de los otros enums de settings (p. ej. tras
`ColumnWidthMode`), agregar:

```rust
/// Cuándo activar el modo de bajo consumo (menos repaints, sin animaciones): para
/// equipos sin GPU dedicada. `Auto` lo decide la app según el renderer detectado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LowPowerMode {
    /// Automático: ON si el render es por software (sin GPU), OFF si hay GPU.
    Auto,
    /// Siempre ON (fuerza bajo consumo aunque haya GPU).
    Always,
    /// Siempre OFF (todo activo: animaciones + framerate pleno).
    Never,
}
```

- [ ] **Step 2: Agregar el campo a Settings con default Auto**

En el struct `Settings`, junto a los otros campos `#[serde(default ...)]` (al final del
struct, tras `preview_rules`/`preview_text_exts_legacy`):

```rust
    /// Cuándo usar el modo de bajo consumo. `#[serde(default)]` retro-compat (settings
    /// previos → Auto).
    #[serde(default = "default_low_power_mode")]
    pub low_power_mode: LowPowerMode,
```

Y el helper de default (junto a los otros `fn default_*`):

```rust
/// Default de `low_power_mode`: Auto (lo decide la app según el renderer).
fn default_low_power_mode() -> LowPowerMode {
    LowPowerMode::Auto
}
```

- [ ] **Step 3: Inicializar en `Default for Settings`**

En `impl Default for Settings`, junto a los otros campos:

```rust
            low_power_mode: LowPowerMode::Auto,
```

- [ ] **Step 4: Extender el test settings_round_trip**

En el test `settings_round_trip`, en el literal de `Settings { ... }`, agregar el campo
(elige un valor != default para que el round-trip sea significativo):

```rust
            low_power_mode: LowPowerMode::Always,
```

Y agregar un test de retro-compat tras `settings_viejo_sin_columnas_cae_a_fijo_y_sin_plantilla`:

```rust
    #[test]
    fn settings_viejo_sin_low_power_cae_a_auto() {
        let json = r#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"flat"}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.low_power_mode, LowPowerMode::Auto);
    }
```

- [ ] **Step 5: Run tests**

Run (PowerShell): `cargo test -p naygo-core config 2>&1 | Select-String "test result|FAILED"`
Expected: `test result: ok`.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/config/mod.rs
git commit -F - <<'EOF'
feat(core): Settings.low_power_mode {Auto, Always, Never}

Controla el modo de bajo consumo. Auto lo decide la app segun el renderer; Always
fuerza ON; Never lo apaga. serde default Auto (retro-compat) + round-trip y test de
settings viejo.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 3 — ui: detectar renderer, modo efectivo, aplicar animaciones + hover 30fps

**Files:**
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: Agregar el campo `software_render` al struct NaygoApp**

En `crates/ui/src/app.rs`, en el struct `NaygoApp` (junto a `egui_ctx`):

```rust
    /// `true` si eframe está renderizando por software (sin GPU real), detectado al
    /// arrancar leyendo `GL_RENDERER`. Decide el modo de bajo consumo en `Auto`.
    software_render: bool,
    /// Mostrar el diálogo de bienvenida (solo el primer arranque). Se baja al elegir.
    show_welcome: bool,
```

- [ ] **Step 2: Detectar el renderer en `NaygoApp::new` y capturar primer arranque**

En `NaygoApp::new`, donde está `let settings_exists = ...` (línea ~538), NO cambiar esa
línea. Más abajo, antes de construir el struct `NaygoApp { ... }`, agregar la detección
del renderer leyendo el contexto glow de `cc`:

```rust
        // Detectar render por software (VM/equipo sin GPU): leer GL_RENDERER del contexto
        // glow que eframe expone. Si no hay contexto glow (caso raro), asumir GPU (false).
        let software_render = cc
            .gl
            .as_ref()
            .map(|gl| {
                use eframe::glow::HasContext as _;
                // SAFETY: `get_parameter_string` con GL_RENDERER es una lectura simple del
                // driver; el contexto es válido (lo creó eframe). No muta estado GL.
                let name = unsafe { gl.get_parameter_string(eframe::glow::RENDERER) };
                tracing::info!(renderer = %name, "GL_RENDERER detectado");
                naygo_core::render_hint::is_software_renderer(&name)
            })
            .unwrap_or(false);
```

- [ ] **Step 3: Inicializar los dos campos en el struct literal**

En el `NaygoApp { ... }`, junto a `egui_ctx: cc.egui_ctx.clone(),`:

```rust
            software_render,
            show_welcome: !settings_exists,
```

- [ ] **Step 4: Agregar el helper `low_power_active`**

En `impl NaygoApp`, junto a otros helpers de consulta (p. ej. cerca de `active_page_rows`):

```rust
    /// ¿El modo de bajo consumo está activo? `Always`/`Never` mandan; `Auto` sigue al
    /// renderer detectado (ON si es por software).
    fn low_power_active(&self) -> bool {
        match self.settings.low_power_mode {
            naygo_core::config::LowPowerMode::Always => true,
            naygo_core::config::LowPowerMode::Never => false,
            naygo_core::config::LowPowerMode::Auto => self.software_render,
        }
    }
```

- [ ] **Step 5: Aplicar el modo (animation_time) cada frame, idempotente**

En el método `update` (mostrado como `fn logic`), al inicio del cuerpo tras el guard del
splash y ANTES de los `pump_*`, agregar:

```rust
        // Modo de bajo consumo: apagar las animaciones de egui (los fades de hover/
        // selección disparan ráfagas de repaints por interacción). Idempotente y barato:
        // solo setea un campo del estilo. El hover a 30fps (abajo) aplica a todos.
        let anim = if self.low_power_active() { 0.0 } else { 0.083_333_336 };
        ctx.style_mut(|s| {
            if s.animation_time != anim {
                s.animation_time = anim;
            }
        });
```

- [ ] **Step 6: Limitar el hover a ~30fps (para todos)**

Localizar el bloque `pointer_moving` (busca `i.pointer.is_moving()`):

```rust
        let pointer_moving =
            ctx.input(|i| i.pointer.is_moving() && i.pointer.interact_pos().is_some());
        if pointer_moving {
            ctx.request_repaint();
        }
```

Reemplazar el `request_repaint()` por uno limitado a ~30fps:

```rust
        let pointer_moving =
            ctx.input(|i| i.pointer.is_moving() && i.pointer.interact_pos().is_some());
        if pointer_moving {
            // ~30fps en vez de 60: la fila bajo el cursor se resalta con fluidez a la mitad
            // del costo (imperceptible, beneficia con y sin GPU).
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
```

- [ ] **Step 7: Build + puertas**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo build -p naygo-ui; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --all`
Expected: build `Finished`, tests `ok`, clippy limpio.

- [ ] **Step 8: Commit**

```bash
git add crates/ui/src/app.rs
git commit -F - <<'EOF'
feat(ui): detectar render por software y aplicar modo de bajo consumo

Lee GL_RENDERER del contexto glow al arrancar -> software_render. low_power_active()
resuelve el modo efectivo (Auto sigue al renderer). En modo bajo consumo se apagan las
animaciones de egui (animation_time=0). El hover pasa a ~30fps PARA TODOS (mitad del
costo del repaint continuo, imperceptible). Captura el primer arranque (show_welcome).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 4 — ui: selector en Configuración → Avanzado

**Files:**
- Modify: `crates/ui/src/settings_window/advanced.rs`
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: Agregar el selector segmented**

En `crates/ui/src/settings_window/advanced.rs`, dentro de `show`, en la sección de
sistema (tras el bloque `settings.system.*`, antes del separador del factory reset),
agregar. NOTA: `accent` ya está disponible en esta función (se definió para los otros
segmented del lote 2; si no, agregar `let accent = app.active_theme.accent();` al inicio):

```rust
    super::group_sep(ui);
    super::group_label(ui, &app.tr("settings.power.section"));
    let (l_power, l_auto, l_always, l_never) = (
        app.tr("settings.power.mode"),
        app.tr("settings.power.auto"),
        app.tr("settings.power.always"),
        app.tr("settings.power.never"),
    );
    ui.label(l_power);
    crate::widgets::segmented(
        ui,
        &mut app.settings.low_power_mode,
        &[
            (naygo_core::config::LowPowerMode::Auto, l_auto.as_str()),
            (naygo_core::config::LowPowerMode::Always, l_always.as_str()),
            (naygo_core::config::LowPowerMode::Never, l_never.as_str()),
        ],
        accent,
    );
    ui.label(egui::RichText::new(app.tr("settings.power.hint")).weak().small());
```

- [ ] **Step 2: Verificar que `accent` está en alcance**

Run (PowerShell): `Select-String -Path crates\ui\src\settings_window\advanced.rs -Pattern "let accent"`
Expected: aparece `let accent = app.active_theme.accent();`. Si NO aparece, agregar esa
línea al inicio de `show` (tras `section_header`).

- [ ] **Step 3: Claves i18n ES**

En `crates/core/src/i18n/es.json`, agregar (junto a otras `settings.*`):

```json
  "settings.power.section": "Rendimiento",
  "settings.power.mode": "Modo de consumo",
  "settings.power.auto": "Automático",
  "settings.power.always": "Bajo consumo",
  "settings.power.never": "Todo activo",
  "settings.power.hint": "Reduce el uso de CPU en equipos sin tarjeta gráfica dedicada (menos repintados, sin animaciones). Automático lo decide según tu equipo.",
```

- [ ] **Step 4: Claves i18n EN (paridad)**

En `crates/core/src/i18n/en.json`, agregar las MISMAS claves traducidas:

```json
  "settings.power.section": "Performance",
  "settings.power.mode": "Power mode",
  "settings.power.auto": "Automatic",
  "settings.power.always": "Low power",
  "settings.power.never": "Full",
  "settings.power.hint": "Reduces CPU usage on machines without a dedicated GPU (fewer repaints, no animations). Automatic decides based on your machine.",
```

- [ ] **Step 5: Build + puertas + commit**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo build -p naygo-ui; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --all`
Expected: build `Finished`, tests `ok` (incluido el de i18n en paridad), clippy limpio.

```bash
git add crates/ui/src/settings_window/advanced.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -F - <<'EOF'
feat(config): selector de modo de consumo (Auto/Bajo consumo/Todo activo) en Avanzado

Seccion "Rendimiento" con un segmented control para low_power_mode + hint. i18n ES+EN.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 5 — ui: diálogo de bienvenida en el primer arranque

**Files:**
- Create: `crates/ui/src/welcome.rs`
- Modify: `crates/ui/src/main.rs`
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: Crear el módulo del diálogo**

Crear `crates/ui/src/welcome.rs`:

```rust
// Naygo — diálogo de bienvenida del primer arranque (elección del modo de consumo).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Se muestra UNA vez, en el primer arranque (no existía settings.json). Pregunta cómo
//! quiere el usuario que Naygo use los recursos y setea `low_power_mode`. Modal ligero
//! (no bloquea workers). Devuelve la elección, o `None` si el usuario aún no decidió.

use naygo_core::config::LowPowerMode;

/// Pinta el diálogo de bienvenida centrado. Devuelve `Some(modo)` cuando el usuario
/// elige (el caller baja `show_welcome` y persiste), o `None` si sigue abierto.
pub fn show(
    ctx: &egui::Context,
    i18n: &naygo_core::i18n::I18n,
    theme: &crate::theme_apply::ActiveTheme,
) -> Option<LowPowerMode> {
    let mut chosen: Option<LowPowerMode> = None;
    egui::Modal::new(egui::Id::new("naygo_welcome")).show(ctx, |ui| {
        ui.set_width(380.0);
        ui.heading(i18n.t("welcome.title"));
        ui.add_space(6.0);
        ui.label(i18n.t("welcome.body"));
        ui.add_space(12.0);
        // Botón recomendado (Automático) en acento; los otros, normales.
        let auto = egui::Button::new(
            egui::RichText::new(i18n.t("welcome.auto")).color(theme.accent()).strong(),
        );
        if ui.add(auto).clicked() {
            chosen = Some(LowPowerMode::Auto);
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(i18n.t("welcome.low")).clicked() {
                chosen = Some(LowPowerMode::Always);
            }
            if ui.button(i18n.t("welcome.full")).clicked() {
                chosen = Some(LowPowerMode::Never);
            }
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new(i18n.t("welcome.hint")).weak().small());
    });
    chosen
}
```

- [ ] **Step 2: Registrar el módulo en main.rs**

En `crates/ui/src/main.rs`, junto a los otros `mod` (orden alfabético, p. ej. tras
`mod typeahead;` o donde calce):

```rust
mod welcome;
```

- [ ] **Step 3: Pintar el diálogo en `update` mientras `show_welcome`**

En `app.rs`, en el método `update` (mostrado como `fn logic`), cerca de donde se pintan
los otros modales al final (tras el bloque de `pending_dialog`/`pending_resume`), agregar:

```rust
        // Bienvenida del primer arranque: elegir el modo de consumo. Una vez elegido, se
        // baja el flag y se persiste (el watcher de settings ya guarda al cambiar, pero lo
        // forzamos para que quede aunque la app se cierre antes del próximo guardado).
        if self.show_welcome {
            let ctx2 = ui.ctx().clone();
            if let Some(mode) = crate::welcome::show(&ctx2, &self.i18n, &self.active_theme) {
                self.settings.low_power_mode = mode;
                self.show_welcome = false;
                config::save_settings(&self.config_dir, &self.settings);
                self.last_saved_settings = self.settings.clone();
            } else {
                ui.ctx().request_repaint();
            }
        }
```

NOTA sobre tipos: `self.active_theme` es `ActiveTheme`, `self.i18n` es `I18n`,
`self.config_dir` es `PathBuf`, `self.last_saved_settings` es `Settings` — todos campos
existentes usados igual en el resto de `app.rs`.

- [ ] **Step 4: Claves i18n ES**

En `crates/core/src/i18n/es.json`:

```json
  "welcome.title": "Bienvenido a Naygo",
  "welcome.body": "¿Cómo prefieres que Naygo use los recursos de tu equipo?",
  "welcome.auto": "Automático (recomendado)",
  "welcome.low": "Bajo consumo",
  "welcome.full": "Todo activo",
  "welcome.hint": "Puedes cambiarlo después en Configuración → Avanzado → Rendimiento.",
```

- [ ] **Step 5: Claves i18n EN (paridad)**

En `crates/core/src/i18n/en.json`:

```json
  "welcome.title": "Welcome to Naygo",
  "welcome.body": "How would you like Naygo to use your machine's resources?",
  "welcome.auto": "Automatic (recommended)",
  "welcome.low": "Low power",
  "welcome.full": "Full",
  "welcome.hint": "You can change this later in Settings → Advanced → Performance.",
```

- [ ] **Step 6: Build + puertas**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo build -p naygo-ui; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --all`
Expected: build `Finished`, tests `ok`, clippy limpio. (Si `egui::Modal` no existe en
0.34, usar `egui::Window::new(...).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0,0.0])` con un fondo oscurecido; ver Step 7.)

- [ ] **Step 7: (Si `egui::Modal` no compila) usar Window con backdrop**

Si el Step 6 falla porque `egui::Modal` no existe en egui 0.34.3, reemplazar el cuerpo
de `welcome::show` por:

```rust
    let mut chosen: Option<LowPowerMode> = None;
    // Backdrop semitransparente sobre toda la pantalla (modal manual).
    egui::Area::new(egui::Id::new("naygo_welcome_backdrop"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            let r = ctx.content_rect();
            ui.painter()
                .rect_filled(r, 0.0, egui::Color32::from_black_alpha(160));
        });
    egui::Window::new(i18n.t("welcome.title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(i18n.t("welcome.body"));
            ui.add_space(12.0);
            let auto = egui::Button::new(
                egui::RichText::new(i18n.t("welcome.auto")).color(theme.accent()).strong(),
            );
            if ui.add(auto).clicked() {
                chosen = Some(LowPowerMode::Auto);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button(i18n.t("welcome.low")).clicked() {
                    chosen = Some(LowPowerMode::Always);
                }
                if ui.button(i18n.t("welcome.full")).clicked() {
                    chosen = Some(LowPowerMode::Never);
                }
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new(i18n.t("welcome.hint")).weak().small());
        });
    chosen
```

Re-correr el Step 6 tras el cambio.

- [ ] **Step 8: Commit**

```bash
git add crates/ui/src/welcome.rs crates/ui/src/main.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -F - <<'EOF'
feat(ui): dialogo de bienvenida del primer arranque (elige modo de consumo)

La 1a vez (sin settings.json) se pregunta como usar los recursos: Automatico
(recomendado) / Bajo consumo / Todo activo, y se setea low_power_mode + persiste.
Cubre instalador y portable (vive en la app). i18n ES+EN.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 6 — Cierre del lote

- [ ] **Step 1: Pasada final de puertas**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo fmt --all --check; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo build -p naygo-ui`
Expected: fmt sin diff, todas las líneas `test result: ok`, clippy limpio, build `Finished`.

- [ ] **Step 2: Medir el efecto (verificación local)**

Para confirmar que el modo bajo consumo reduce el repaint: insertar TEMPORALMENTE
`ctx.request_repaint();` al final de `update` (simula uso continuo), build, correr con
`low_power_mode=Never` vs `Always`, medir `%CPU` con
`(Get-Counter '\Process(naygo)\% Processor Time').CounterSamples.CookedValue`. Quitar la
línea temporal tras medir. (En modo Always las animaciones off no cambian el repaint
forzado, pero el hover a 30fps sí se nota en uso real; la medición dura es para Nicolás
en la VM.)

- [ ] **Step 3: Regenerar distribución**

Run (PowerShell): `powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1`
y luego: `& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" /DMyAppVersion=0.1.0 installer\naygo.iss`
Verifica timestamps frescos en `dist\`.

- [ ] **Step 4: Avisar a Nicolás + actualizar memoria + pedir autorización de push**

Resumen para Nicolás: en la VM, el consumo durante scroll/hover debe bajar; el selector
Auto/Bajo/Todo en Configuración → Avanzado → Rendimiento; el diálogo de bienvenida
aparece solo la 1ª vez. Actualizar la memoria del proyecto y pedir merge/push.

---

## Autoevaluación del plan (hecha)

- Cubre todas las secciones del spec: detección [T1], enum+setting [T2], hover 30fps +
  animaciones + modo efectivo + primer-arranque-flag [T3], selector [T4], bienvenida [T5].
- Sin placeholders: cada paso trae el código real.
- Consistencia de tipos: `LowPowerMode {Auto,Always,Never}` igual en core/ui/config/UI;
  `is_software_renderer(&str)->bool` igual en definición y uso; `low_power_active(&self)`
  y `software_render`/`show_welcome` definidos en T3 y usados en T3/T5.
- Riesgo conocido señalado: `egui::Modal` puede no existir en 0.34.3 → Step 7 da el
  fallback con `Window`+backdrop. La lectura de `GL_RENDERER` es `unsafe` (glow) con
  SAFETY documentada.
- `animation_time` default de egui = 0.083333 (1/12 s); se restaura ese valor en modo
  normal para no dejar las animaciones apagadas si el usuario cambia de Always a Never.
