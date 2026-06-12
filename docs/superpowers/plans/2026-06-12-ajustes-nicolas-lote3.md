# Lote 3 de ajustes de Nicolás — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Arreglar la regresión de íconos cuadrados del lote 2, hacer la vista previa
configurable por extensión (toggle + alias), corregir los íconos tapados de la
path-bar, dar glifos/íconos confiables a los botones de panel, y agregar un botón
para abrir otra ventana de Naygo.

**Architecture:** Rust workspace de 3 capas (`naygo-core` puro, `naygo-platform`
Win32, `naygo-ui` egui). El modelo de preview pasa de un CSV a `Vec<PreviewRule>`
en `Settings` (core, testeable); la UI lo edita con una tabla diferida. Los íconos
reales se restauran desde git; `gen_icons` se vuelve no-destructivo. La nueva
ventana es un `spawn` del propio ejecutable.

**Tech Stack:** Rust, egui/eframe 0.34.3, egui_extras, serde, crate `image`.

---

## Reglas operativas (del lote 2 — NO te las saltes)

1. **Puertas antes de CADA commit**: `cargo test --workspace` (lee TODAS las líneas
   `test result:`), `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo fmt --all`.
2. **`cargo build -p naygo-ui` explícito** antes de cualquier prueba en vivo.
3. **Mata `naygo.exe` antes de compilar**: `Stop-Process -Name naygo -Force`
   (tolerante).
4. **Convenciones**: header de copyright en archivos nuevos; comentarios en español
   del PORQUÉ; cero texto hardcodeado (i18n ES+EN en paridad SIEMPRE — hay un test
   que lo verifica); el hilo de UI no hace I/O; legibilidad sobre brevedad.
5. **Commits en español**, estilo del repo, uno por tarea, con
   `Co-Authored-By: <tu modelo>`. Para evitar el bug del `@` con here-strings de
   PowerShell, commitea con heredoc de Bash: `git commit -F - <<'EOF' … EOF`.
6. **NO uses `git add -A`**: stagea rutas explícitas (`crates/`, `assets/icons/`,
   `docs/`) — hay un cambio ajeno pendiente en `CLAUDE.md` (sección graphify) que
   NO debe entrar en estos commits.
7. **JAMÁS corras `cargo run --bin gen_icons` sin `--force` sobre todos los sets** —
   eso causó esta regresión. La Tarea 1 lo vuelve no-destructivo; úsalo así.

---

## Estructura de archivos

- `assets/icons/{flat,fluent,mono}/*.png` — restaurar reales (Tarea 1).
- `crates/ui/src/bin/gen_icons.rs` — no-destructivo + glifos reales nuevos (Tarea 1, 2, 5).
- `crates/core/src/icon_kind.rs` — `ActionIcon::NewWindow` (Tarea 5).
- `crates/ui/src/icons/assets.rs` — entrada de tabla `action_new_window` (Tarea 5).
- `crates/core/src/preview.rs` — `PreviewRule`, `classify` con reglas, migración (Tarea 4).
- `crates/core/src/config/mod.rs` — `preview_rules`, migración del CSV (Tarea 4).
- `crates/ui/src/app.rs` — usar `preview_rules` en el worker; spawn de ventana (Tarea 4, 5).
- `crates/ui/src/settings_window/preview.rs` — tabla editable (Tarea 4).
- `crates/ui/src/pathbar.rs` — reservar espacio de íconos (Tarea 3).
- `crates/ui/src/toolbar.rs` — glifos confiables + botón nueva ventana (Tarea 2, 5).
- `crates/core/src/i18n/{es,en}.json` — claves nuevas (Tarea 4, 5).

---

### Tarea 1 — Restaurar íconos reales + endurecer gen_icons

**Files:**
- Restore: `assets/icons/{flat,fluent,mono}/*.png` (desde commit `30a34d7`)
- Modify: `crates/ui/src/bin/gen_icons.rs`

- [ ] **Step 1: Restaurar los PNG reales desde git (antes del lote 2)**

Los 25 íconos de cada set fueron pisados con placeholders. Restaurar SOLO los que
existían en `30a34d7` (los 2 nuevos, `action_swap_panes`/`action_clone_path`, no
estaban ahí y se quedan como están por ahora — la Tarea 2 los redibuja):

```bash
git checkout 30a34d7 -- assets/icons/flat assets/icons/fluent assets/icons/mono
```

Esto restaura los reales y deja `action_swap_panes.png`/`action_clone_path.png`
intactos (git checkout de un pathspec no borra archivos que no existían en ese commit).

- [ ] **Step 2: Verificar que se restauraron (tamaños reales, no ~230 bytes)**

Run (PowerShell): `Get-Item assets\icons\flat\action_copy.png, assets\icons\flat\folder.png | Select Name,Length`
Expected: `action_copy.png` ~1324 bytes, `folder.png` ~615 bytes (NO 230).

- [ ] **Step 3: Hacer gen_icons no-destructivo (solo crea faltantes salvo --force)**

En `crates/ui/src/bin/gen_icons.rs`, reemplazar `main` por una versión que respeta
los PNG existentes. Cambiar la cabecera del comentario y el `main`:

```rust
fn main() {
    // No-destructivo por defecto: solo genera los PNG que FALTAN (placeholders para
    // claves nuevas). Con `--force` regenera todos (peligroso: pisa íconos curados).
    // Esto evita repetir la regresión del lote 2, donde un run completo aplastó los
    // íconos reales con cuadrados de color.
    let force = std::env::args().any(|a| a == "--force");
    let mut created = 0usize;
    for set in ["flat", "fluent", "mono"] {
        let dir = Path::new("assets/icons").join(set);
        std::fs::create_dir_all(&dir).expect("crear dir");
        let mono = set == "mono";
        for (name, color) in icon_specs() {
            let path = dir.join(format!("{name}.png"));
            if path.exists() && !force {
                continue;
            }
            make_icon(&path, color, mono);
            created += 1;
        }
    }
    if force {
        println!("--force: regenerados TODOS los íconos (placeholders).");
    } else {
        println!("generados {created} íconos faltantes (los existentes se respetaron).");
    }
}
```

- [ ] **Step 4: Build + verificar que compila (gen_icons es parte del crate ui)**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo build -p naygo-ui`
Expected: `Finished`. (No corras gen_icons todavía; lo hace la Tarea 2/5.)

- [ ] **Step 5: Commit**

```bash
git add assets/icons crates/ui/src/bin/gen_icons.rs
git commit -F - <<'EOF'
fix(iconos): restaurar set real pisado por gen_icons en el lote 2 + endurecer gen_icons

El lote 2 corrio gen_icons (destructivo) y aplasto los 25 iconos reales de cada set
con placeholders de 230 bytes -> toolbar cuadrada en modo Pack. Se restauran desde
30a34d7. gen_icons ahora solo crea los PNG faltantes salvo --force.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 2 — Glifos confiables + íconos propios de swap/clone/add_pane

**Files:**
- Modify: `crates/ui/src/bin/gen_icons.rs` (dibujo de glifos reales)
- Modify: `crates/ui/src/toolbar.rs` (glifos confiables en modo Glyphs)

**Diseño:** dos frentes. (a) En modo Glifos, los símbolos `➕ ⇄ ⎘` pueden no estar en
la fuente default de egui (se ven como tofu). Reemplazarlos por caracteres seguros.
(b) En modo Pack, `swap_panes`/`clone_path`/`add_pane` necesitan un PNG no-cuadrado.

- [ ] **Step 1: Dibujar glifos reales para los íconos de acción nuevos en gen_icons**

Hoy `make_icon` pinta un cuadrado. Agregar una variante que dibuje un glifo simple
(líneas) para las acciones que lo necesitan, manteniendo el cuadrado para el resto.
En `crates/ui/src/bin/gen_icons.rs`, añadir tras `make_icon`:

```rust
/// Dibuja un glifo simple (trazos) centrado sobre fondo transparente, para los íconos
/// de acción nuevos que no tienen arte propio (swap, clone, add_pane, new_window).
/// `draw` recibe un buffer mutable y pinta píxeles del color dado.
fn make_glyph(path: &Path, color: [u8; 4], mono: bool, glyph: Glyph) {
    let mut img = RgbaImage::new(SIZE, SIZE);
    let c = if mono { [200, 200, 200, 255] } else { color };
    let put = |img: &mut RgbaImage, x: i32, y: i32| {
        if x >= 0 && y >= 0 && (x as u32) < SIZE && (y as u32) < SIZE {
            img.put_pixel(x as u32, y as u32, Rgba(c));
        }
    };
    // Trazo grueso: pinta un cuadrado 2x2 por punto para que se vea a 32px.
    let dot = |img: &mut RgbaImage, x: i32, y: i32| {
        for dy in 0..2 {
            for dx in 0..2 {
                put(img, x + dx, y + dy);
            }
        }
    };
    let hline = |img: &mut RgbaImage, x0: i32, x1: i32, y: i32| {
        for x in x0..=x1 {
            dot(img, x, y);
        }
    };
    let vline = |img: &mut RgbaImage, y0: i32, y1: i32, x: i32| {
        for y in y0..=y1 {
            dot(img, x, y);
        }
    };
    match glyph {
        Glyph::Plus => {
            hline(&mut img, 8, 22, 15);
            vline(&mut img, 8, 22, 15);
        }
        Glyph::Swap => {
            // Dos flechas horizontales opuestas (arriba ->, abajo <-).
            hline(&mut img, 8, 22, 11);
            dot(&mut img, 20, 9);
            dot(&mut img, 22, 11);
            dot(&mut img, 20, 13);
            hline(&mut img, 8, 22, 20);
            dot(&mut img, 10, 18);
            dot(&mut img, 8, 20);
            dot(&mut img, 10, 22);
        }
        Glyph::Clone => {
            // Dos rectángulos solapados (copiar).
            hline(&mut img, 8, 18, 9);
            hline(&mut img, 8, 18, 19);
            vline(&mut img, 9, 19, 8);
            vline(&mut img, 9, 19, 18);
            hline(&mut img, 13, 23, 13);
            hline(&mut img, 13, 23, 23);
            vline(&mut img, 13, 23, 13);
            vline(&mut img, 13, 23, 23);
        }
        Glyph::NewWindow => {
            // Ventana con un "+" en la esquina.
            hline(&mut img, 7, 21, 9);
            hline(&mut img, 7, 21, 21);
            vline(&mut img, 9, 21, 7);
            vline(&mut img, 9, 21, 21);
            hline(&mut img, 16, 24, 7);
            vline(&mut img, 3, 11, 20);
        }
    }
    img.save(path).expect("guardar PNG");
}

/// Glifos dibujables para los íconos de acción nuevos.
#[derive(Clone, Copy)]
enum Glyph {
    Plus,
    Swap,
    Clone,
    NewWindow,
}
```

- [ ] **Step 2: Generar SOLO los íconos de acción nuevos con su glifo**

Al final de `main`, tras el bucle de `icon_specs`, agregar la generación de los
glifos de acción (siempre se regeneran porque hoy son cuadrados/placeholders; estos
SÍ los queremos pisar). Insertar antes del `println!` final de `main`:

```rust
    // Íconos de acción con glifo propio (reemplazan los placeholders cuadrados). Estos
    // SÍ se regeneran siempre: son los que se veían como cuadritos.
    for set in ["flat", "fluent", "mono"] {
        let dir = Path::new("assets/icons").join(set);
        let mono = set == "mono";
        let color = [90, 120, 170, 255];
        make_glyph(&dir.join("action_add_pane.png"), color, mono, Glyph::Plus);
        make_glyph(&dir.join("action_swap_panes.png"), color, mono, Glyph::Swap);
        make_glyph(&dir.join("action_clone_path.png"), color, mono, Glyph::Clone);
        make_glyph(
            &dir.join("action_new_window.png"),
            color,
            mono,
            Glyph::NewWindow,
        );
    }
```

(Nota: `action_new_window.png` lo consume la Tarea 5; generarlo aquí es inofensivo
aunque la clave del enum se agregue en la Tarea 5. Si ejecutas las tareas en orden,
el archivo queda listo.)

- [ ] **Step 3: Correr gen_icons (no-destructivo: respeta los reales, redibuja las acciones)**

Run (PowerShell): `cargo run -p naygo-ui --bin gen_icons`
Expected: imprime "generados N íconos faltantes…" y los `action_*` nuevos se
redibujan con glifo. Verifica que `action_copy.png` SIGUE siendo el real (~1324
bytes, no se tocó): `Get-Item assets\icons\flat\action_copy.png | Select Length`.

- [ ] **Step 4: Glifos confiables en modo Glyphs (toolbar)**

En `crates/ui/src/toolbar.rs`, los glifos `➕`/`⇄`/`⎘` pueden no renderizar. Cambiar
a caracteres seguros. Localizar las líneas de `btn!`:

```rust
    btn!("⇄", ActionIcon::SwapPanes, &lbl_swap, files_count >= 2);
    btn!("⎘", ActionIcon::ClonePath, &lbl_clone, true);
```
y
```rust
    btn!("➕", ActionIcon::AddPane, &lbl_add_pane, true);
```

Reemplazar los glifos por ASCII seguros (la fuente default SIEMPRE los tiene):

```rust
    btn!("<>", ActionIcon::SwapPanes, &lbl_swap, files_count >= 2);
    btn!("[]", ActionIcon::ClonePath, &lbl_clone, true);
```
```rust
    btn!("+", ActionIcon::AddPane, &lbl_add_pane, true);
```

(El `▾` del `menu_button` de plantillas SÍ está en la fuente default de egui —
dejarlo. El `⟳` de refresh también; no tocar.)

- [ ] **Step 5: Build + puertas + commit**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo build -p naygo-ui; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --all`
Expected: build `Finished`, tests `ok` (todas las líneas `test result:`), clippy sin warnings.

```bash
git add assets/icons crates/ui/src/bin/gen_icons.rs crates/ui/src/toolbar.rs
git commit -F - <<'EOF'
fix(iconos): glifos reales para add/swap/clone y glifos ASCII confiables en la toolbar

En modo Pack, add_pane/swap/clone tenian placeholder cuadrado -> ahora un glifo
dibujado (gen_icons). En modo Glifos, ➕/⇄/⎘ no estaban en la fuente default (tofu)
-> se reemplazan por +, <>, [] que la fuente siempre tiene.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 3 — Íconos de la path-bar que no se tapen con rutas largas

**Files:**
- Modify: `crates/ui/src/pathbar.rs` (función `breadcrumb_mode`)

**Diseño:** hoy los íconos (📋, ☆/★) y los breadcrumbs comparten el contenedor
`right_to_left`; con ruta larga los breadcrumbs invaden la zona de los íconos. Hay
que reservar el ancho de los íconos y limitar el sub-layout de breadcrumbs al ancho
restante.

- [ ] **Step 1: Limitar el ancho del sub-layout de breadcrumbs**

En `crates/ui/src/pathbar.rs`, dentro de `breadcrumb_mode`, los íconos ya se pintan
primero en el `right_to_left`. El sub-layout `left_to_right` de breadcrumbs hay que
acotarlo a `ui.available_width()` (lo que queda tras los íconos) y truncar segmentos
si no caben. Localizar el bloque:

```rust
            // El resto del ancho: breadcrumbs (izq→der) + zona vacía clicable.
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
```

Reemplazarlo por una versión que fija el ancho máximo del sub-layout al disponible y
recorta los breadcrumbs largos (clip), de modo que NUNCA empujen los íconos:

```rust
            // El resto del ancho: breadcrumbs (izq→der) + zona vacía clicable. Se acota
            // al ancho que QUEDA tras reservar los íconos de la derecha, y se recorta
            // (clip) si no cabe, para que los íconos copiar/favorito nunca se tapen.
            let remaining = ui.available_width();
            ui.allocate_ui_with_layout(
                egui::vec2(remaining, ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_clip_rect(ui.max_rect());
```

Y cerrar el `allocate_ui_with_layout` donde antes cerraba el `with_layout` (mismo
nivel de llaves). El cuerpo interno (el `for` de segmentos + la zona vacía clicable)
queda IGUAL, solo cambia el contenedor. Es decir, el bloque interno existente:

```rust
                for (i, (label, dir)) in split_segments(current_dir).into_iter().enumerate() {
                    ...
                }
                // Zona vacía a la derecha de los segmentos...
                let w = ui.available_width().max(24.0);
                let (_, resp) = ui.allocate_exact_size(...);
                if resp.on_hover_text(...).clicked() { ... }
```

permanece dentro del nuevo `allocate_ui_with_layout`. Cerrar con `},\n            );`
en vez del `});` del `with_layout` anterior.

- [ ] **Step 2: Build + verificar que compila**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo build -p naygo-ui`
Expected: `Finished`. (Verificación visual de rutas largas: queda para Nicolás.)

- [ ] **Step 3: Puertas + commit**

Run (PowerShell): `cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --all`
Expected: tests `ok`, clippy limpio.

```bash
git add crates/ui/src/pathbar.rs
git commit -F - <<'EOF'
fix(pathbar): reservar el ancho de los iconos copiar/favorito (no taparse con rutas largas)

Los breadcrumbs y los iconos (📋, ☆/★) compartian el contenedor right_to_left; una
ruta larga invadia la zona de los iconos. Ahora los breadcrumbs van en un sub-layout
acotado al ancho restante con clip; los iconos quedan siempre visibles a la derecha.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 4 — Vista previa configurable: toggle por extensión + alias

**Files:**
- Modify: `crates/core/src/preview.rs` (PreviewRule, classify con reglas, defaults)
- Modify: `crates/core/src/config/mod.rs` (preview_rules, migración del CSV)
- Modify: `crates/ui/src/app.rs:1165` y `:4273` (usar preview_rules en el worker)
- Modify: `crates/ui/src/settings_window/preview.rs` (tabla editable)
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: Escribir el test de `PreviewRule` + `classify` con reglas (core)**

En `crates/core/src/preview.rs`, agregar al `mod tests` (al final, dentro del módulo):

```rust
    #[test]
    fn classify_con_reglas_toggle_y_alias() {
        let rules = vec![
            PreviewRule { ext: "txt".into(), enabled: true, treat_as: None },
            PreviewRule { ext: "log".into(), enabled: false, treat_as: None },
            PreviewRule { ext: "sif".into(), enabled: true, treat_as: Some("xml".into()) },
            PreviewRule { ext: "xml".into(), enabled: true, treat_as: None },
            PreviewRule { ext: "png".into(), enabled: true, treat_as: None },
            PreviewRule { ext: "jpg".into(), enabled: false, treat_as: None },
        ];
        // txt habilitado -> texto.
        assert_eq!(classify_rules(Path::new("a.txt"), &rules), PreviewKind::Text);
        // log deshabilitado -> None aunque sea texto.
        assert_eq!(classify_rules(Path::new("a.log"), &rules), PreviewKind::None);
        // sif (alias a xml) -> texto.
        assert_eq!(classify_rules(Path::new("a.sif"), &rules), PreviewKind::Text);
        // png habilitado -> imagen.
        assert_eq!(classify_rules(Path::new("a.PNG"), &rules), PreviewKind::Image);
        // jpg deshabilitado -> None.
        assert_eq!(classify_rules(Path::new("a.jpg"), &rules), PreviewKind::None);
        // extension sin regla -> None.
        assert_eq!(classify_rules(Path::new("a.mp4"), &rules), PreviewKind::None);
    }

    #[test]
    fn classify_alias_a_imagen_y_alias_roto() {
        let rules = vec![
            PreviewRule { ext: "raw".into(), enabled: true, treat_as: Some("png".into()) },
            PreviewRule { ext: "png".into(), enabled: true, treat_as: None },
            PreviewRule { ext: "weird".into(), enabled: true, treat_as: Some("zzz".into()) },
        ];
        // raw -> png -> imagen.
        assert_eq!(classify_rules(Path::new("a.raw"), &rules), PreviewKind::Image);
        // alias a una extension sin regla ni tipo conocido -> None (1 salto, no cicla).
        assert_eq!(classify_rules(Path::new("a.weird"), &rules), PreviewKind::None);
    }

    #[test]
    fn default_rules_tiene_texto_e_imagen_habilitados() {
        let rules = default_preview_rules();
        assert!(rules.iter().any(|r| r.ext == "txt" && r.enabled));
        assert!(rules.iter().any(|r| r.ext == "png" && r.enabled));
        // Todas arrancan habilitadas y sin alias.
        assert!(rules.iter().all(|r| r.enabled && r.treat_as.is_none()));
    }

    #[test]
    fn migracion_csv_a_reglas() {
        let rules = rules_from_csv("txt, md, .RS,, json");
        // Cada extension del CSV es una regla de texto habilitada + las de imagen.
        assert!(rules.iter().any(|r| r.ext == "txt" && r.enabled && r.treat_as.is_none()));
        assert!(rules.iter().any(|r| r.ext == "rs"));
        assert!(rules.iter().any(|r| r.ext == "png"));
    }
```

- [ ] **Step 2: Run test para verificar que falla (símbolos no existen)**

Run: `cargo test -p naygo-core preview 2>&1 | Select-String "error|test result"`
Expected: FAIL de compilación (`PreviewRule`, `classify_rules`, `default_preview_rules`, `rules_from_csv` no existen).

- [ ] **Step 3: Implementar PreviewRule + classify_rules + defaults + migración (core)**

En `crates/core/src/preview.rs`, agregar el tipo y las funciones (tras `PreviewKind`
y las constantes existentes; conservar `classify`/`parse_text_extensions` por
compatibilidad de los tests viejos, o migrar esos tests — ver Step 5):

```rust
use serde::{Deserialize, Serialize};

/// Regla de previsualización para UNA extensión: si se previsualiza y, opcionalmente,
/// como qué otra extensión tratarla (alias). Editable en Configuración.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewRule {
    /// Extensión sin punto, en minúscula ("sif", "txt", "png").
    pub ext: String,
    /// Si se previsualiza esta extensión.
    pub enabled: bool,
    /// Tratar la extensión como otra (alias). `Some("xml")` => un .sif se clasifica
    /// como .xml. `None` => por sí misma.
    pub treat_as: Option<String>,
}

/// Clasifica una ruta según las reglas configuradas. Resuelve el alias `treat_as` (un
/// salto, sin ciclar) y luego decide Text/Image/None por la extensión efectiva. Una
/// extensión sin regla, o con la regla deshabilitada, es `None`.
pub fn classify_rules(path: &std::path::Path, rules: &[PreviewRule]) -> PreviewKind {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext.is_empty() {
        return PreviewKind::None;
    }
    let Some(rule) = rules.iter().find(|r| r.ext == ext) else {
        return PreviewKind::None;
    };
    if !rule.enabled {
        return PreviewKind::None;
    }
    // Extensión efectiva: el alias si lo hay (un salto), si no la propia.
    let effective = rule.treat_as.as_deref().unwrap_or(&ext);
    kind_of_extension(effective)
}

/// Tipo intrínseco de una extensión (sin reglas): imagen fija, o texto si está en la
/// lista semilla de texto, o None. Lo usa `classify_rules` tras resolver el alias.
fn kind_of_extension(ext: &str) -> PreviewKind {
    if IMAGE_EXTENSIONS.contains(&ext) {
        PreviewKind::Image
    } else if DEFAULT_TEXT_EXTENSIONS.contains(&ext) {
        PreviewKind::Text
    } else {
        PreviewKind::None
    }
}

/// Reglas por defecto: cada extensión de texto semilla + cada imagen, todas
/// habilitadas y sin alias.
pub fn default_preview_rules() -> Vec<PreviewRule> {
    let mk = |e: &&str| PreviewRule {
        ext: (*e).to_string(),
        enabled: true,
        treat_as: None,
    };
    DEFAULT_TEXT_EXTENSIONS
        .iter()
        .chain(IMAGE_EXTENSIONS.iter())
        .map(|e| mk(e))
        .collect()
}

/// Migra un CSV viejo de extensiones de texto a reglas (cada una habilitada, sin
/// alias) + las reglas de imagen por defecto. Para settings.json previos.
pub fn rules_from_csv(csv: &str) -> Vec<PreviewRule> {
    let mut rules: Vec<PreviewRule> = parse_text_extensions(csv)
        .into_iter()
        .map(|ext| PreviewRule {
            ext,
            enabled: true,
            treat_as: None,
        })
        .collect();
    // Agregar las imágenes que falten (habilitadas).
    for img in IMAGE_EXTENSIONS {
        if !rules.iter().any(|r| r.ext == *img) {
            rules.push(PreviewRule {
                ext: (*img).to_string(),
                enabled: true,
                treat_as: None,
            });
        }
    }
    rules
}
```

Nota: `kind_of_extension` reusa `DEFAULT_TEXT_EXTENSIONS` como el universo de
extensiones de texto "conocidas" para el destino de un alias. Esto cubre el caso de
Nicolás (.sif → xml, y xml ∈ semilla). Si el alias apunta a una extensión de texto
que no está en la semilla, no se reconoce como texto (limitación documentada; el
combo de la UI solo ofrece extensiones de la semilla, así que no ocurre por la UI).

- [ ] **Step 4: Run test para verificar que pasa**

Run: `cargo test -p naygo-core preview 2>&1 | Select-String "test result"`
Expected: `test result: ok` con los nuevos tests incluidos.

- [ ] **Step 5: Migrar los tests viejos de classify(...) a classify_rules**

Los tests `clasifica_imagen_texto_y_nada`, `parse_extensiones_normaliza` y
`default_csv_round_trip` usan la firma vieja. `classify`/`parse_text_extensions`/
`default_text_extensions_csv` se MANTIENEN (los usa la migración), así que esos tests
siguen pasando sin cambios. No hay que tocarlos. (Confirmar con el Step 4 que todo el
módulo pasa.)

- [ ] **Step 6: Settings: reemplazar preview_text_exts por preview_rules con migración**

En `crates/core/src/config/mod.rs`: cambiar el campo y su default. Reemplazar:

```rust
    #[serde(default = "default_preview_text_exts")]
    pub preview_text_exts: String,
```
por:
```rust
    /// Reglas de previsualización (una por extensión): toggle + alias. Editable en
    /// Configuración → Previsualización. `#[serde(default)]` retro-compat.
    #[serde(default = "default_preview_rules_cfg")]
    pub preview_rules: Vec<crate::preview::PreviewRule>,
    /// DEPRECADO: CSV de extensiones de texto (lote 2). Solo se LEE para migrar a
    /// `preview_rules`. Ya no se escribe (skip si está vacío).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preview_text_exts_legacy: String,
```

Reemplazar el default helper:
```rust
/// Default de `preview_text_exts`: la lista semilla de `core::preview`.
fn default_preview_text_exts() -> String {
    crate::preview::default_text_extensions_csv()
}
```
por:
```rust
/// Default de `preview_rules`: las reglas semilla (texto + imagen, habilitadas).
fn default_preview_rules_cfg() -> Vec<crate::preview::PreviewRule> {
    crate::preview::default_preview_rules()
}
```

En `Default for Settings`, reemplazar:
```rust
            preview_text_exts: default_preview_text_exts(),
```
por:
```rust
            preview_rules: default_preview_rules_cfg(),
            preview_text_exts_legacy: String::new(),
```

Migración al cargar: en `load_settings_flagged`, dentro del brazo
`Some(mut s) if s.version == CONFIG_VERSION =>`, tras la normalización del icon_set,
agregar:
```rust
            // Migrar el CSV de preview (lote 2) a reglas, si venía el campo viejo y no
            // hay reglas explícitas (settings anterior al lote 3).
            if !s.preview_text_exts_legacy.is_empty() && s.preview_rules.is_empty() {
                s.preview_rules = crate::preview::rules_from_csv(&s.preview_text_exts_legacy);
            }
            if s.preview_rules.is_empty() {
                s.preview_rules = crate::preview::default_preview_rules();
            }
            s.preview_text_exts_legacy.clear();
```

Actualizar el `serde(rename)` del campo legacy para que lea el JSON viejo: cambiar
la línea del campo legacy a:
```rust
    #[serde(default, rename = "preview_text_exts", skip_serializing_if = "String::is_empty")]
    pub preview_text_exts_legacy: String,
```

- [ ] **Step 7: Actualizar el test settings_round_trip**

En `crates/core/src/config/mod.rs`, en el test `settings_round_trip`, reemplazar:
```rust
            preview_text_exts: "txt, md, rs".to_string(),
```
por:
```rust
            preview_rules: vec![
                crate::preview::PreviewRule {
                    ext: "sif".into(),
                    enabled: true,
                    treat_as: Some("xml".into()),
                },
                crate::preview::PreviewRule {
                    ext: "png".into(),
                    enabled: false,
                    treat_as: None,
                },
            ],
            preview_text_exts_legacy: String::new(),
```

Agregar un test de migración tras `settings_viejo_sin_columnas_cae_a_fijo_y_sin_plantilla`:
```rust
    #[test]
    fn settings_migra_preview_csv_a_reglas() {
        let dir = tempfile::tempdir().unwrap();
        // settings.json del lote 2: trae preview_text_exts (CSV), sin preview_rules.
        std::fs::write(
            dir.path().join("settings.json"),
            br#"{"version":1,"bar_position":"Top","icon_only":true,"icon_set":"flat","preview_text_exts":"txt, md"}"#,
        )
        .unwrap();
        let s = load_settings(dir.path());
        assert!(s.preview_rules.iter().any(|r| r.ext == "txt" && r.enabled));
        assert!(s.preview_rules.iter().any(|r| r.ext == "md"));
        // Las imágenes se agregan en la migración.
        assert!(s.preview_rules.iter().any(|r| r.ext == "png"));
        assert!(s.preview_text_exts_legacy.is_empty(), "el CSV viejo se limpia");
    }
```

- [ ] **Step 8: Run tests de core**

Run: `cargo test -p naygo-core 2>&1 | Select-String "test result|FAILED"`
Expected: todas `ok`.

- [ ] **Step 9: Usar preview_rules en el worker (app.rs)**

En `crates/ui/src/app.rs`, donde hoy arma `text_exts` (línea ~1165):
```rust
                let text_exts = naygo_core::preview::parse_text_extensions(
                    &self.settings.preview_text_exts,
                );
```
reemplazar por clonar las reglas:
```rust
                let rules = self.settings.preview_rules.clone();
```
Y propagar `rules` (en vez de `text_exts`) al `std::thread::spawn` y a
`build_preview_payload`. En la firma de `build_preview_payload` (línea ~4267):
```rust
fn build_preview_payload(
    path: &std::path::Path,
    text_exts: &[String],
    token: &CancellationToken,
) -> PreviewPayload {
    use naygo_core::preview::{self, PreviewKind};
    match preview::classify(path, text_exts) {
```
cambiar a:
```rust
fn build_preview_payload(
    path: &std::path::Path,
    rules: &[naygo_core::preview::PreviewRule],
    token: &CancellationToken,
) -> PreviewPayload {
    use naygo_core::preview::{self, PreviewKind};
    match preview::classify_rules(path, rules) {
```
Y en el `spawn` (donde se mueve `text_exts` al hilo), renombrar la captura a `rules`:
```rust
                let worker_token = token.clone();
                std::thread::spawn(move || {
                    let payload = build_preview_payload(&path, &rules, &worker_token);
                    let _ = tx.send((path, payload));
                });
```

- [ ] **Step 10: Tabla editable en Configuración → Previsualización**

Reemplazar `crates/ui/src/settings_window/preview.rs` por la tabla editable. La UI
acumula los cambios directo sobre `app.settings.preview_rules` (persiste por el
watcher de settings). Contenido completo del archivo:

```rust
// Naygo — sección Previsualización de la Configuración.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Tabla editable de reglas de previsualización: por extensión, un check on/off y un
//! alias opcional ("tratar como" otra extensión). Las imágenes y los textos aparecen
//! como filas. El cambio se persiste por el watcher de settings de `NaygoApp`.

use crate::app::NaygoApp;
use naygo_core::preview::PreviewRule;

/// Extensiones que ofrece el combo "tratar como" (concretas; el motor solo distingue
/// texto/imagen, pero mostrarlas es más claro y deja la puerta a resaltado futuro).
const TREAT_AS_OPTIONS: &[&str] = &[
    "txt", "log", "md", "json", "xml", "csv", "toml", "yaml", "ini", "html", "rs",
];

pub fn show(ui: &mut egui::Ui, app: &mut NaygoApp) {
    let (title, sub) = (app.tr("settings.preview"), app.tr("settings.preview.sub"));
    super::section_header(ui, &title, &sub);

    let l_ext = app.tr("settings.preview.col_ext");
    let l_on = app.tr("settings.preview.col_enabled");
    let l_as = app.tr("settings.preview.col_treat_as");
    let l_own = app.tr("settings.preview.treat_as_own");
    let l_add = app.tr("settings.preview.add");
    let l_hint = app.tr("settings.preview.hint");

    ui.label(egui::RichText::new(l_hint).weak().small());
    ui.add_space(6.0);

    // Encabezados de la tabla.
    ui.horizontal(|ui| {
        ui.add_sized([90.0, 18.0], egui::Label::new(egui::RichText::new(l_ext).strong()));
        ui.add_sized([50.0, 18.0], egui::Label::new(egui::RichText::new(l_on).strong()));
        ui.label(egui::RichText::new(l_as).strong());
    });

    // Índice a quitar tras pintar (no se puede mutar el Vec mientras se itera).
    let mut remove: Option<usize> = None;
    let rules = &mut app.settings.preview_rules;
    for (i, rule) in rules.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            // Extensión editable (corta).
            ui.add_sized(
                [90.0, 22.0],
                egui::TextEdit::singleline(&mut rule.ext).hint_text("ext"),
            );
            // Check on/off.
            ui.add_sized([50.0, 22.0], egui::Checkbox::without_text(&mut rule.enabled));
            // Combo "tratar como": (propia) o una extensión concreta.
            let current = rule.treat_as.clone().unwrap_or_else(|| l_own.clone());
            egui::ComboBox::from_id_salt(("treat_as", i))
                .selected_text(current)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(rule.treat_as.is_none(), &l_own).clicked() {
                        rule.treat_as = None;
                    }
                    for opt in TREAT_AS_OPTIONS {
                        let sel = rule.treat_as.as_deref() == Some(*opt);
                        if ui.selectable_label(sel, *opt).clicked() {
                            rule.treat_as = Some((*opt).to_string());
                        }
                    }
                });
            if ui.button("🗑").on_hover_text(app_tr_remove()).clicked() {
                remove = Some(i);
            }
        });
    }

    if let Some(i) = remove {
        app.settings.preview_rules.remove(i);
    }

    ui.add_space(6.0);
    if ui.button(format!("+ {l_add}")).clicked() {
        app.settings.preview_rules.push(PreviewRule {
            ext: String::new(),
            enabled: true,
            treat_as: None,
        });
    }
}

/// Texto del tooltip de quitar (constante i18n resuelta aparte para no pelear con el
/// préstamo de `app` dentro del bucle).
fn app_tr_remove() -> String {
    "Quitar".to_string()
}
```

Nota sobre `app_tr_remove`: para evitar el conflicto de préstamo (`app` está prestado
mutable por `rules`), el tooltip "Quitar" se resuelve como literal. Si se quiere i18n
estricto, mover la lectura de `app.tr("settings.preview.remove")` ANTES del `let rules
= &mut ...` a una variable `l_remove` y usarla en el closure. Hacerlo así:
reemplazar `app_tr_remove()` por una variable `l_remove` capturada antes del bucle, y
borrar la función `app_tr_remove`. (Patrón idéntico a las otras `l_*` de arriba.)

- [ ] **Step 11: Claves i18n (ES + EN en paridad)**

En `crates/core/src/i18n/es.json`, reemplazar las claves de preview existentes de
settings (las `settings.preview.text_exts*`) por las nuevas de la tabla:
```json
  "settings.preview.col_ext": "Extensión",
  "settings.preview.col_enabled": "Activa",
  "settings.preview.col_treat_as": "Tratar como",
  "settings.preview.treat_as_own": "(propia)",
  "settings.preview.add": "agregar extensión",
  "settings.preview.remove": "Quitar",
  "settings.preview.hint": "Activa o desactiva la vista previa por extensión. «Tratar como» previsualiza una extensión propia usando otro tipo (p. ej. .sif como .xml).",
```
Borrar las claves `settings.preview.text_exts` y `settings.preview.text_exts_hint`.
Hacer lo MISMO en `en.json` con las traducciones:
```json
  "settings.preview.col_ext": "Extension",
  "settings.preview.col_enabled": "On",
  "settings.preview.col_treat_as": "Treat as",
  "settings.preview.treat_as_own": "(itself)",
  "settings.preview.add": "add extension",
  "settings.preview.remove": "Remove",
  "settings.preview.hint": "Toggle preview per extension. \"Treat as\" previews a custom extension using another type (e.g. .sif as .xml).",
```

Si optaste por la variable `l_remove`, usa `app.tr("settings.preview.remove")`.

- [ ] **Step 12: Build + puertas + commit**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo build -p naygo-ui; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --all`
Expected: build `Finished`, tests `ok` (incluido el de i18n en paridad), clippy limpio.

```bash
git add crates/core/src/preview.rs crates/core/src/config/mod.rs crates/ui/src/app.rs crates/ui/src/settings_window/preview.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -F - <<'EOF'
feat(preview): reglas por extension (activar/desactivar + alias) en vez de CSV

Settings.preview_rules: Vec<PreviewRule { ext, enabled, treat_as }> reemplaza el CSV
preview_text_exts. classify_rules resuelve el alias (un salto, p. ej. .sif como .xml)
y respeta el toggle. Migracion del CSV viejo + tests. Configuracion -> Previsualizacion
pasa a tabla editable (ext, on/off, tratar como, quitar, agregar). i18n ES+EN.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 5 — Botón "abrir otra ventana de Naygo"

**Files:**
- Modify: `crates/core/src/icon_kind.rs` (ActionIcon::NewWindow + file_name + all + test)
- Modify: `crates/ui/src/icons/assets.rs` (entrada de tabla action_new_window)
- Modify: `crates/ui/src/toolbar.rs` (botón + handler)
- Modify: `crates/ui/src/app.rs` (método spawn_new_window)
- Modify: `crates/core/src/i18n/{es,en}.json` (tooltip)

- [ ] **Step 1: Agregar ActionIcon::NewWindow (core) + actualizar conteo del test**

En `crates/core/src/icon_kind.rs`, en el enum `ActionIcon` agregar la variante (antes
de `Settings`):
```rust
    /// Abrir otra ventana de Naygo (nuevo proceso).
    NewWindow,
    Settings,
```
En `all()`, agregar `NewWindow` a la lista (antes de `Settings`):
```rust
            SwapPanes, ClonePath, NewWindow, Settings,
```
En `file_name()`:
```rust
            NewWindow => "action_new_window",
            Settings => "action_settings",
```
Actualizar el test `action_icon_all_son_14_...`: ahora son 15. Renombrar y cambiar
ambos `14` por `15`:
```rust
    fn action_icon_all_son_15_con_file_name_unico() {
        let all = ActionIcon::all();
        assert_eq!(all.len(), 15);
        ...
        assert_eq!(names.len(), 15, "cada acción tiene un nombre de archivo único");
```

- [ ] **Step 2: Agregar la entrada a la tabla de assets (UI)**

En `crates/ui/src/icons/assets.rs`, dentro del macro `set_table!`, agregar la línea
(junto a las otras `action_*`, antes de `action_settings`):
```rust
            ("action_new_window", png!($set, "action_new_window")),
```
(El PNG ya lo generó la Tarea 2 Step 2. Si ejecutas fuera de orden, corre
`cargo run -p naygo-ui --bin gen_icons` antes de compilar.)

- [ ] **Step 3: Run test de core para verificar el conteo**

Run: `cargo test -p naygo-core icon 2>&1 | Select-String "test result|FAILED"`
Expected: `ok` (15 acciones).

- [ ] **Step 4: Método spawn_new_window en NaygoApp**

En `crates/ui/src/app.rs`, agregar el método (junto a `add_files_pane` u otro método
público de acción de toolbar):
```rust
    /// Abre OTRA ventana de Naygo lanzando un nuevo proceso del propio ejecutable, sin
    /// argumentos: arranca como un inicio normal y restaura el workspace persistido
    /// (carpetas iniciales, layout). Cada ventana es un proceso independiente. Un fallo
    /// se reporta discreto en la barra de estado (no crashea).
    pub fn spawn_new_window(&mut self) {
        match std::env::current_exe() {
            Ok(exe) => match std::process::Command::new(exe).spawn() {
                Ok(_) => {
                    self.status = self.i18n.t("status.new_window").to_string();
                }
                Err(e) => {
                    tracing::warn!("no se pudo abrir otra ventana: {e}");
                    self.status = self.i18n.t("status.new_window_failed").to_string();
                }
            },
            Err(e) => {
                tracing::warn!("current_exe falló: {e}");
                self.status = self.i18n.t("status.new_window_failed").to_string();
            }
        }
    }
```

- [ ] **Step 5: Botón en la toolbar + handler**

En `crates/ui/src/toolbar.rs`: agregar el label cerca de los otros (`lbl_*`):
```rust
    let lbl_new_window = app.tr("toolbar.new_window");
```
Agregar el botón tras el `▾` de plantillas (o junto a `➕`/`+` add_pane). Usar un
glifo confiable `&`-libre; ASCII seguro p. ej. `"++"` no — mejor un símbolo simple.
Insertar tras la línea del `btn!("+", ActionIcon::AddPane, ...)`:
```rust
    btn!("⧉", ActionIcon::NewWindow, &lbl_new_window, true);
```
NOTA: `⧉` puede no estar en la fuente (igual que clone). Para no arriesgar tofu en
modo Glifos, usar el ícono del pack como referencia y un glifo ASCII de respaldo:
usar `"[+]"`:
```rust
    btn!("[+]", ActionIcon::NewWindow, &lbl_new_window, true);
```
En el `match` de `clicked`, agregar el brazo (junto a `AddPane`):
```rust
            ActionIcon::NewWindow => app.spawn_new_window(),
```

- [ ] **Step 6: Claves i18n (ES + EN)**

En `crates/core/src/i18n/es.json`:
```json
  "toolbar.new_window": "Abrir otra ventana de Naygo",
  "status.new_window": "Abriendo otra ventana…",
  "status.new_window_failed": "No se pudo abrir otra ventana",
```
En `crates/core/src/i18n/en.json`:
```json
  "toolbar.new_window": "Open another Naygo window",
  "status.new_window": "Opening another window…",
  "status.new_window_failed": "Could not open another window",
```

- [ ] **Step 7: Build + puertas + commit**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo run -p naygo-ui --bin gen_icons; cargo build -p naygo-ui; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --all`
Expected: build `Finished`, tests `ok`, clippy limpio.

```bash
git add crates/core/src/icon_kind.rs crates/ui/src/icons/assets.rs crates/ui/src/toolbar.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json assets/icons
git commit -F - <<'EOF'
feat(toolbar): boton para abrir otra ventana de Naygo (nuevo proceso)

ActionIcon::NewWindow en la toolbar lanza un nuevo proceso del propio ejecutable sin
argumentos: arranca normal y restaura el workspace persistido. Glifo confiable + icono
de pack dibujado (gen_icons). i18n ES+EN. Un fallo se reporta en la barra de estado.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 6 — Cierre del lote

- [ ] **Step 1: Pasada final de puertas completas**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo fmt --all --check; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo build -p naygo-ui`
Expected: fmt sin diff, todas las líneas `test result: ok`, clippy limpio, build `Finished`.

- [ ] **Step 2: Regenerar distribución**

Run (PowerShell): `powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1`
Luego, si ISCC no está en PATH:
`& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" /DMyAppVersion=0.1.0 installer\naygo.iss`
Verifica timestamps frescos en `dist\`.

- [ ] **Step 3: Avisar a Nicolás qué probar manualmente**

Resumen para Nicolás: (a) íconos de toolbar reales en modo Pack Y modo Glifos (ya no
cuadrados); (b) íconos copiar/favorito del path no se tapan con rutas largas; (c)
Configuración → Previsualización: desactivar una extensión, agregar .sif → tratar
como xml y ver el archivo; (d) botón nueva ventana abre otra instancia con las
carpetas iniciales.

- [ ] **Step 4: Actualizar la memoria del proyecto + pedir autorización de push**

Actualizar el backlog (lote 3 implementado, branch sin pushear) y pedir a Nicolás
autorización para merge/push a main (como el lote 2).

---

## Autoevaluación del plan (hecha)

- Cubre los 5 puntos del spec: (1) restaurar íconos + endurecer gen_icons [T1], (2)
  glifos/íconos de botones de panel [T2], (3) path-bar íconos [T3], (4) preview
  toggle+alias [T4], (5) nueva ventana [T5].
- Sin placeholders: cada paso de código trae el código real.
- Consistencia de tipos: `PreviewRule { ext, enabled, treat_as }` usado igual en
  core, settings, worker y UI; `classify_rules` con la misma firma en su definición y
  su uso en `build_preview_payload`; `ActionIcon::NewWindow` con `file_name`
  `action_new_window` consistente entre icon_kind, assets y gen_icons.
- Riesgo conocido señalado: el alias solo reconoce destinos de texto que estén en la
  semilla `DEFAULT_TEXT_EXTENSIONS` (el combo de la UI solo ofrece esos, así que no se
  rompe por la UI).
- Orden de dependencias: T2 genera `action_new_window.png`; T5 lo consume — si se
  ejecuta fuera de orden, T5 Step 7 corre gen_icons antes de compilar.
