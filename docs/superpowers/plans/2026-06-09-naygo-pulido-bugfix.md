# Pulido / bugfix post-prueba — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Corregir los problemas hallados en la primera prueba real: hover/clics lentos al cargar (repaint), íconos Flat incompletos, falta de límites de ventana y de vista previa del pack.

**Architecture:** Cuatro frentes independientes en `ui`: (1) afinar la cadencia de repaint en `NaygoApp::logic`/`ui` para fluidez sin romper bajo consumo; (2) `with_min_inner_size` en la ventana principal y la de Configuración + ScrollArea responsive + ancho mínimo de tabs del dock; (3) reemplazar 7 PNG placeholder de `assets/icons/flat/` por íconos reales; (4) fila de preview del pack en Configuración. `core` no se toca.

**Tech Stack:** Rust, eframe/egui 0.34, egui_dock 0.19, `image` 0.25; `resvg` solo en un binario throwaway fuera del workspace para rasterizar.

**Estado de partida (rama `feat/pulido-bugfix`, desde `main` c2d3ab5):**
- `crates/ui/src/main.rs`: `let native_options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_title("Naygo").with_inner_size([1100.0, 700.0]) /* + .with_icon(...) */, ..Default::default() };`. (Task 3-distribución añadió `with_icon`; el builder se arma en una variable `viewport` y luego se mete en `native_options` — READ el bloque actual.)
- `crates/ui/src/app.rs` `impl eframe::App for NaygoApp`:
  - `fn logic(&mut self, ctx, _frame)` (~2296): `if self.splash.is_some() { ctx.request_repaint(); return; }` luego los `pump_*`, `process_shortcut_capture`, `handle_input`, y el bloque de repaint condicional (líneas ~2323-2335):
    ```rust
        if self.any_listing_active()
            || self.any_tree_listing_active()
            || self.any_op_active()
            || self.pending_paste_write.is_some()
            || self.disk_rx.is_some()
            || !self.size_jobs.is_empty()
        {
            ctx.request_repaint();
        }
        if !self.watchers.is_empty() || self.device_watch.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    ```
  - `fn ui(&mut self, ui, _frame)` (~2342): `if self.splash.is_some() { ... } if let Some(splash) = &self.splash { let keep = splash.show(ui); if !keep { self.splash = None; } return; }` (splash se limpia en ~2347). Luego el dock: `egui_dock::DockArea::new(&mut self.dock_state).style(egui_dock::Style::from_egui(ui.style().as_ref()))...` (~2487).
- `crates/ui/src/settings_window/mod.rs`: `show_settings_viewport(app, ctx)` (~30) arma `ViewportBuilder::default().with_title(app.tr("settings.title")).with_inner_size([560.0, 420.0]).with_close_button(true)` y `ctx.show_viewport_immediate(...)`. Dentro: `Panel::left("settings_sections").exact_size(160.0)` + `egui::CentralPanel::default().show_inside(ui, |ui| match app.settings_section { Appearance => appearance::show(ui, app), ... })` (~54). Las secciones NO están envueltas en ScrollArea.
- `crates/ui/src/settings_window/appearance.rs`: `pub fn show(ui, app)` — tiene el selector de Set (loop sobre el catálogo) y el toggle Glifos/Pack + color de glifos. `app` da acceso a `app.icons` (IconProvider) y `app.settings.toolbar_icon_style`/`icon_set`.
- `crates/ui/src/icons/mod.rs`: `IconProvider::texture(key: IconKey) -> &egui::TextureHandle`. `IconKey` (core::icon_kind): `Folder`, `File(FileCategory)`, `Drive(DriveKind)`, `Unknown`, `Action(ActionIcon)`. `FileCategory::{Image,Code,...}`, `ActionIcon::{Copy,Settings,...}`.
- `assets/icons/flat/`: 7 placeholders de ~230 bytes: `action_copy.png`, `action_cut.png`, `action_paste.png`, `action_new_file.png`, `action_new_folder.png`, `file_font.png`, `file_model3d.png`. El resto son reales. `assets.rs` espera EXACTAMENTE esos nombres.
- `assets/icons/fluentui-emoji-main.zip` presente (SVG color en `assets/<Nombre>/Color/<...>_color.svg`). `lucide-main.zip` presente (SVG mono). `.gitignore` excluye `assets/icons/*.zip`.
- egui 0.34: `ctx.is_pointer_over_area() -> bool` (CONFIRMADO en context.rs:2919) — "el puntero está sobre un área de egui" (es decir, sobre la ventana con UI). `egui::ScrollArea::vertical().show(ui, |ui| {...})`. `egui::Image::new(&TextureHandle).fit_to_exact_size(egui::vec2(w,h))`.
- egui_dock 0.19: `Style` tiene `minimum_width: Option<f32>` (CONFIRMADO, style.rs:227) — ancho mínimo de tab.

**Prerequisito de entorno:** Rust en PATH (`export PATH="$HOME/.cargo/bin:$PATH"`). NUNCA `2>&1` con cargo. `cargo fmt --all` antes de cada commit. Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt verdes antes de cada commit. Para rasterizar SVG (Task 4): bin Rust throwaway con `resvg` en una carpeta TEMP FUERA del repo (patrón de la fase toolbar-icons), borrado al terminar — NO añadir resvg al workspace.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. `core` NO se toca. Bajo consumo: el repaint NO debe volverse continuo incondicional. Tolerante. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/pulido-bugfix`. NO cambiar de rama.

**Reparto de verificación:** el agente compila/clippy/fmt/tests, confirma que los íconos Flat ya no son de 230 bytes y que no se commitea ningún zip. La prueba VISUAL (clics toman al instante, hover fluido, ventana respeta el mínimo, Flat completo, preview) la hace Nicolás.

---

## Estructura de archivos

```
crates/ui/src/app.rs                          # repaint afinado (logic/ui) + dock minimum_width
crates/ui/src/main.rs                         # with_min_inner_size ventana principal
crates/ui/src/settings_window/mod.rs          # with_min_inner_size Config + ScrollArea por sección
crates/ui/src/settings_window/appearance.rs   # fila de preview del pack
assets/icons/flat/*.png                       # 7 placeholders → reales
assets/icons/NOTICE.md                        # atribución de la fuente nueva en Flat
```

---

## Task 1: Repaint reactivo afinado (el bug grave)

**Files:**
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: Hover fluido + primer frame post-splash**

In `crates/ui/src/app.rs`:
a) In `logic`, AFTER the conditional repaint block (~2335, after the `if !self.watchers.is_empty() ...` block), add the hover-repaint:
```rust
        // Fluidez del hover/selección: mientras el puntero está sobre la UI, repintamos
        // continuo para que la fila bajo el mouse y el clic respondan al instante (egui,
        // por defecto, solo repinta una vez por evento de movimiento → el hover se siente
        // lento). En IDLE con el mouse FUERA de la ventana NO se repinta → se respeta el
        // bajo consumo. NO convertir esto en un repaint incondicional.
        if ctx.is_pointer_over_area() {
            ctx.request_repaint();
        }
```
b) In `ui`, where the splash is cleared (`self.splash = None;` ~2347), request a repaint so the first real frame renders and processes input immediately instead of waiting for an event:
```rust
            if !keep {
                self.splash = None;
                // El primer frame real tras el splash debe renderizar y atender input ya
                // (sin esperar un evento), si no los primeros clics "no toman".
                ui.ctx().request_repaint();
            }
```
(VERIFY `ctx.is_pointer_over_area()` compiles on egui 0.34 — confirmed present in context.rs. If for some reason it behaves as "over an interactable widget" only and misses empty panel space, fall back to `ctx.input(|i| i.pointer.latest_pos().is_some())` which is "pointer has a known position inside the window". Report which you used.)

- [ ] **Step 2: Confirmar el primer listado post-arranque repinta**

Read `any_listing_active()` (app.rs:933). Confirm it returns `true` while the startup listing(s) are streaming (i.e. there's an in-flight listing right after `start_all_listings()` in `new`). If it does (the conditional block already requests repaint for active listings), NO extra code is needed — the hover-repaint from Step 1 plus the active-listing repaint cover the load phase. If `any_listing_active` does NOT cover the very first listing (e.g. it checks a map that's populated only after the first pump), note it and add a repaint request for the initial load. Report what you found — do NOT add redundant code if it's already covered.

- [ ] **Step 3: Verify**

Run: `cargo build -p naygo-ui` → compiles. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.
NOTE: the actual feel (clicks take instantly, hover smooth) is Nicolás's manual check on a real run.

- [ ] **Step 4: Commit**
```
git add crates/ui/src/app.rs
git commit -m "fix(ui): repaint fluido con el puntero sobre la UI + primer frame post-splash

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Límite mínimo de la ventana principal

**Files:**
- Modify: `crates/ui/src/main.rs`

- [ ] **Step 1: with_min_inner_size en el viewport principal**

In `crates/ui/src/main.rs`, find where the `ViewportBuilder` is built (a `let mut viewport = egui::ViewportBuilder::default().with_title("Naygo").with_inner_size([1100.0, 700.0]);` plus the `.with_icon` from the distribución phase). Add `.with_min_inner_size([640.0, 400.0])`:
```rust
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Naygo")
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size([640.0, 400.0]);
```
(Keep the existing `.with_icon(...)` wiring intact — just add the `.with_min_inner_size` to the chain. READ the current block and insert it without disturbing the icon code.)

- [ ] **Step 2: Verify**

Run: `cargo build -p naygo-ui` → compiles. `cargo clippy -p naygo-ui --all-targets -- -D warnings` → clean. `cargo fmt --all`.

- [ ] **Step 3: Commit**
```
git add crates/ui/src/main.rs
git commit -m "fix(ui): tope mínimo de la ventana principal (640x400)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Configuración — mínimo + ScrollArea responsive + ancho mínimo de tabs

**Files:**
- Modify: `crates/ui/src/settings_window/mod.rs`
- Modify: `crates/ui/src/app.rs` (dock minimum_width)

- [ ] **Step 1: Mínimo del viewport de Configuración**

In `crates/ui/src/settings_window/mod.rs` `show_settings_viewport`, add `.with_min_inner_size([460.0, 360.0])` to the builder:
```rust
    let builder = egui::ViewportBuilder::default()
        .with_title(app.tr("settings.title"))
        .with_inner_size([560.0, 420.0])
        .with_min_inner_size([460.0, 360.0])
        .with_close_button(true);
```

- [ ] **Step 2: ScrollArea en el contenido de las secciones**

Wrap the section content in a vertical ScrollArea so it doesn't get cut off on a small window. Change the CentralPanel block (~54):
```rust
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match app.settings_section {
                SettingsSection::Appearance => appearance::show(ui, app),
                SettingsSection::Panes => panes::show(ui, app),
                SettingsSection::Shortcuts => shortcuts::show(ui, app),
                SettingsSection::Language => language::show(ui, app),
                SettingsSection::Advanced => advanced::show(ui, app),
            });
        });
```
(This makes any section scroll vertically instead of clipping. The left sections panel (`exact_size(160.0)`) stays as-is; with the 460px min width there's room for both. If the left panel feels cramped at min width, that's acceptable — the min stops it from going smaller.)

- [ ] **Step 3: Ancho mínimo de las tabs del dock (paneles del explorador)**

In `crates/ui/src/app.rs` where the `DockArea` is built (~2487-2488), set the dock Style's `minimum_width` so a panel can't be dragged below a usable width:
```rust
            let mut dock_style = egui_dock::Style::from_egui(ui.style().as_ref());
            dock_style.tab_bar.minimum_width = Some(150.0); // adaptar a la ruta real del campo
            egui_dock::DockArea::new(&mut self.dock_state)
                .style(dock_style)
                // ... resto de la cadena igual ...
```
VERIFY the exact path of `minimum_width` in egui_dock 0.19 `Style` (style.rs:227 has `pub minimum_width: Option<f32>` — confirm whether it's `Style.minimum_width` directly or nested under `Style.tab_bar` / `Style.tab`). Adapt the field path until it compiles. This is the TAB minimum width; egui_dock 0.19 may not expose a per-split panel minimum — if `minimum_width` only affects the tab bar and not the draggable split, that's the best available and the window minimum (640px) is the safety net. Report what `minimum_width` actually controls in this version.

- [ ] **Step 4: Verify**

Run: `cargo build -p naygo-ui` → compiles. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.

- [ ] **Step 5: Commit**
```
git add crates/ui/src/settings_window/mod.rs crates/ui/src/app.rs
git commit -m "fix(ui): Configuración con mínimo + scroll responsive; ancho mínimo de tabs del dock

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Íconos Flat completos (reemplazar los 7 placeholders)

**Files:**
- Replace: `assets/icons/flat/{action_copy,action_cut,action_paste,action_new_file,action_new_folder,file_font,file_model3d}.png`
- Modify: `assets/icons/NOTICE.md`

TAREA MANUAL Y DELICADA (rasterización). Tolerante: si un ícono no se logra, dejar un genérico decente del pack (NO el cuadro de 230 bytes). NUNCA commitear un `.zip`.

- [ ] **Step 1: Construir el rasterizador throwaway (fuera del workspace)**

In a TEMP dir OUTSIDE the repo (e.g. `$env:TEMP\svgconv2`), create a tiny Cargo project with `resvg` that converts an SVG to a 32×32 PNG (the toolbar-icons phase used exactly this). Minimal `main.rs`:
```rust
use resvg::{tiny_skia, usvg::{self, Transform}};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (inp, outp, size) = (&a[1], &a[2], a[3].parse::<u32>().unwrap());
    let svg = std::fs::read_to_string(inp).unwrap();
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
    let ts = tree.size();
    let scale = size as f32 / ts.width().max(ts.height());
    let mut pm = tiny_skia::Pixmap::new(size, size).unwrap();
    let tx = (size as f32 - ts.width() * scale) / 2.0;
    let ty = (size as f32 - ts.height() * scale) / 2.0;
    resvg::render(&tree, Transform::from_scale(scale, scale).post_translate(tx, ty), &mut pm.as_mut());
    pm.save_png(outp).unwrap();
}
```
`Cargo.toml`: `[dependencies] resvg = "0.44"`. Build with `cargo build --release` in that temp dir. (If resvg 0.44 API differs, adapt — the toolbar-icons phase built this successfully.)

- [ ] **Step 2: Extraer las fuentes y mapear los 7 nombres**

Extract fluentui (color) and lucide (mono fallback) to TEMP (NOT the repo):
```
Expand-Archive -Path assets\icons\fluentui-emoji-main.zip -DestinationPath $env:TEMP\fluent2 -Force
Expand-Archive -Path assets\icons\lucide-main.zip -DestinationPath $env:TEMP\lucide2 -Force
```
fluentui SVGs live at `fluentui-emoji-main\assets\<Nombre>\Color\<...>_color.svg`. Map each of the 7 to the best colored emoji (explore the asset dirs to find them):
- `action_copy` → "Bookmark tabs" / "Clipboard" (or lucide `copy` if no good color match)
- `action_cut` → "Scissors"
- `action_paste` → "Clipboard"
- `action_new_file` → "Page facing up" / "Memo" (a doc-with-plus look; emoji may lack "+", pick the closest)
- `action_new_folder` → "Open file folder" / "File folder"
- `file_font` → "Input latin letters" / "Input symbols"
- `file_model3d` → "Gem stone" / "Package" (a 3D-ish object)
Prefer fluentui (color, matches Flat's multicolor style). If a given name has no reasonable fluentui emoji, use the lucide SVG (monochrome) as fallback, OR copy an existing real Flat icon as the generic (e.g. `file_generic.png` for `file_font`/`file_model3d`) — never leave the 230-byte placeholder.

- [ ] **Step 3: Rasterizar a assets/icons/flat/<name>.png (overwrite los 7)**

Run the throwaway converter for each mapping → write 32×32 PNG OVERWRITING the placeholder in `assets/icons/flat/`. Verify each output is a valid PNG with real content (byte size clearly > 230, e.g. several hundred bytes to a few KB). Example:
```
& $conv "$env:TEMP\fluent2\fluentui-emoji-main\assets\Scissors\Color\scissors_color.svg" "assets\icons\flat\action_cut.png" 32
```
After all 7, confirm: `Get-ChildItem assets\icons\flat\action_copy.png,...,file_model3d.png | Select Name,Length` — none should be ~230 bytes.

- [ ] **Step 4: NOTICE.md**

Update `assets/icons/NOTICE.md`: note that the Flat set's 7 previously-missing icons (copy/cut/paste/new_file/new_folder/font/model3d) now come from fluentui-emoji (MIT) and/or lucide (ISC) as the case may be. Keep the existing attribution lines.

- [ ] **Step 5: Verify (build + tests + no zip staged)**

Run: `cargo build -p naygo-ui` → compiles (include_bytes! resolves the same names). `cargo test -p naygo-ui icons::assets` → the existing `cada_clave_tiene_su_propio_asset…` etc. PASS. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all -- --check`.
CRITICAL: `git status` — confirm NO `.zip` is staged/new. `git check-ignore assets/icons/fluentui-emoji-main.zip` → prints the path (ignored). `git diff --cached --name-only | grep -i .zip` → empty.

- [ ] **Step 6: Commit (solo los PNG + NOTICE; NUNCA zips)**

Clean up the TEMP rasterizer + extracted dirs. Then:
```
git add assets/icons/flat/action_copy.png assets/icons/flat/action_cut.png assets/icons/flat/action_paste.png assets/icons/flat/action_new_file.png assets/icons/flat/action_new_folder.png assets/icons/flat/file_font.png assets/icons/flat/file_model3d.png assets/icons/NOTICE.md
git commit -m "assets: completar el set Flat (7 íconos reales reemplazan los placeholders)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
Report exactly which of the 7 came from fluentui, which from lucide, and which (if any) fell back to a generic Flat icon.

---

## Task 5: Vista previa del pack en Configuración (visual)

**Files:**
- Modify: `crates/ui/src/settings_window/appearance.rs`
- Modify: `crates/core/src/i18n/{es,en}.json` (etiqueta del preview)

- [ ] **Step 1: i18n — etiqueta del preview (ambos json)**

Add to both `es.json` and `en.json` (identical keys):
- ES `settings.icons.preview`: `"Vista previa del pack:"`
- EN `settings.icons.preview`: `"Pack preview:"`
(Keys identical in both — parity test.)

- [ ] **Step 2: Fila de preview en appearance.rs**

In `crates/ui/src/settings_window/appearance.rs` `show(ui, app)`, after the Set selector (and near the Glifos/Pack toggle), add a preview row of representative icons rendered with the active IconProvider:
```rust
    // Vista previa del pack activo: así el usuario ve cómo se ven los íconos del set
    // elegido (Flat/Fluent/Mono) antes de usarlos en la toolbar.
    ui.add_space(4.0);
    ui.label(app.tr("settings.icons.preview"));
    ui.horizontal(|ui| {
        use naygo_core::icon_kind::{ActionIcon, FileCategory, IconKey};
        let keys = [
            IconKey::Folder,
            IconKey::File(FileCategory::Image),
            IconKey::File(FileCategory::Code),
            IconKey::Action(ActionIcon::Copy),
            IconKey::Action(ActionIcon::Settings),
        ];
        for key in keys {
            let tex = app.icons.texture(key);
            ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(24.0, 24.0)));
        }
    });
```
(VERIFY: `app.icons` is accessible from appearance.rs (it's `pub(crate)` on NaygoApp — Task 7 of toolbar-icons made `icons`/`active_theme` pub(crate)). `app.icons.texture(IconKey) -> &TextureHandle`. `FileCategory::Image`/`Code` and `ActionIcon::Copy`/`Settings` exist. Borrow note: `app.tr(...)` borrows app immutably; `app.icons.texture(...)` also immutable — both fine in sequence, but if a borrow conflict arises with `&mut app` elsewhere in `show`, read the label into a local before the `ui.horizontal` closure. Adapt.)
NOTE: the preview reflects the CURRENTLY LOADED set (IconProvider reloads when `icon_set` changes — verified in app.rs). So when the user picks a different Set, the next frame the preview updates. Good.

- [ ] **Step 3: Verify**

Run: `cargo build -p naygo-ui` → compiles. `cargo test --workspace` → green (i18n parity). `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.
NOTE: the visual look of the preview row is Nicolás's check.

- [ ] **Step 4: Commit**
```
git add crates/ui/src/settings_window/appearance.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): vista previa de los íconos del pack en Configuración

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Cierre — verificación final + push

**Files:** (ninguno nuevo; verificación)

- [ ] **Step 1: Verificación final del workspace**

Run: `cargo build --workspace` → compiles. `cargo build --release -p naygo-ui` → release compila. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all -- --check` → clean.
CRÍTICO: `git status` limpio; `git ls-files "assets/icons/*.zip"` vacío; los 7 PNG de Flat ya NO son de 230 bytes (`ls -la assets/icons/flat/action_copy.png ...`).

- [ ] **Step 2: Push**
```
git push -u origin feat/pulido-bugfix
```

Report: resultados de verificación, confirmación de íconos Flat con contenido real, push result.

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| Repaint: hover fluido (puntero dentro) | 1 |
| Repaint: primer frame post-splash | 1 |
| Repaint: confirmar primer listado | 1 (Step 2) |
| Repaint NO continuo incondicional + comentario | 1 |
| Íconos Flat 26/26 (7 placeholders reales) | 4 |
| NOTICE actualizado | 4 |
| Vista previa del pack | 5 |
| Mínimo ventana principal (640×400) | 2 |
| Mínimo Configuración (460×360) + ScrollArea | 3 |
| Mínimos paneles del dock | 3 |
| Zips no commiteados | 4, 6 (git check-ignore) |
| FUERA: multi-selección, inline rename, drag&drop | (no se tocan) |

**Notas de riesgo:**
- **`ctx.is_pointer_over_area()`** (Task 1): confirmado en egui 0.34. Si en la práctica solo cubre widgets interactivos y no el panel vacío, usar `ctx.input(|i| i.pointer.latest_pos().is_some())`. Medir que el idle con mouse FUERA no repinte (no romper bajo consumo).
- **`any_listing_active` primer listado** (Task 1 Step 2): solo agregar código si hay un hueco real; si ya cubre, no duplicar.
- **egui_dock 0.19 `minimum_width`** (Task 3): confirmar la ruta del campo (`Style.minimum_width` vs anidado) y QUÉ controla (tab bar vs split). Si no hay mínimo por split, el mínimo de ventana es la red de seguridad — documentarlo.
- **Rasterización Flat** (Task 4): bin throwaway resvg en TEMP, fluentui (color) preferido, lucide o genérico como fallback; VERIFICAR `git status`/`git check-ignore` que ningún `.zip` se commitee; confirmar bytes reales (>230).
- **Borrow en el preview** (Task 5): `app.tr` + `app.icons.texture` ambos inmutables; si choca, leer la etiqueta a un local antes del closure.
- **ScrollArea Configuración** (Task 3): envolver sin alterar la lógica del `match` de secciones.
```
