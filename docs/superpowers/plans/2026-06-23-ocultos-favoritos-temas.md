# Ocultos + favoritos con grupos + editor de temas — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agregar a Naygo tres features independientes: mostrar/ocultar archivos ocultos/sistema/dotfiles, favoritos con grupos anidados, y un editor de colores de temas en vivo (reduciendo los temas de fábrica a 5).

**Architecture:** Las tres son independientes. F1 amplía `Entry` con atributos reales de Windows + un filtro puro. F3 recorta el catálogo de temas y agrega un editor que reusa el sistema de temas de usuario (JSON) y `theme_apply::apply` para el preview en vivo. F2 cambia el modelo de favoritos de lista plana a árbol (con migración) y hace editable el panel de Favoritos.

**Tech Stack:** Rust workspace (naygo-core / naygo-platform / naygo-ui-slint), Slint 1.16 render software, serde. Build con `CARGO_BUILD_JOBS=2`.

**Gate (correr SIEMPRE uno mismo tras cada subagente):**
```
$env:CARGO_BUILD_JOBS = "2"
cargo fmt
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**i18n:** triple (es + en en `crates/core/src/i18n/{es,en}.json` + props `crates/ui-slint/ui/i18n.slint` + setters `crates/ui-slint/src/i18n_keys.rs`), español neutral SIN voseo. NO reutilizar claves.

**REGLA para subagentes:** graphify antes de grep (`graphify query "<x>"` en bash); incluirla en cada prompt.

---

## Estructura de archivos

**F1 (ocultos):** `crates/core/src/listing.rs` (atributos en Entry), `crates/core/src/fs_model.rs` (campo `system`), capa de filtro/vista (is_visible — ubicar dónde se filtra hoy), `crates/core/src/config/mod.rs` (Settings), `crates/ui-slint/src/workspace_ctrl.rs` + `main.rs` + `ui/app-window.slint` (menú toolbar), i18n.

**F3 (temas):** `crates/core/src/theme/mod.rs` (recortar catálogo, helper slug, marca fábrica/usuario), `crates/core/src/theme/builtin/*` (borrar 10 .json), `crates/ui-slint/src/config_ctrl.rs` + `ui/config-window.slint` (editor + selector de color) + `main.rs` (preview vivo), i18n.

**F2 (favoritos):** `crates/core/src/favorites.rs` (modelo árbol + migración), `crates/ui-slint/src/workspace_ctrl.rs`/`main.rs` (menú ▾ + panel editable), `ui/app-window.slint` (▾ toolbar) + `ui/favorites-panel.slint` (árbol editable), i18n.

---

# FASE 1 — Archivos ocultos/sistema/dotfiles

### Task 1: Entry lee atributos hidden/system reales

**Files:**
- Modify: `crates/core/src/fs_model.rs` (campo `system`)
- Modify: `crates/core/src/listing.rs` (poblar hidden+system)

- [ ] **Step 1: Orientarte**

`graphify query "Entry struct fs_model hidden campos kind"`. Lee `Entry` en fs_model.rs (tiene
`hidden: bool`). `graphify query "entry_from_path metadata listing atributos"`; lee
`entry_from_path` (listing.rs:116, hoy `hidden: false` hardcodeado).

- [ ] **Step 2: Agregar campo `system` a Entry**

En `fs_model.rs`, en `struct Entry`, junto a `hidden: bool`, agregar:
```rust
    pub system: bool,
```
Actualizar TODOS los sitios que construyen Entry con literal (el compilador los marca) agregando
`system: false` por defecto donde no se conozca.

- [ ] **Step 3: Poblar hidden+system desde atributos reales (Windows) en entry_from_path**

En `listing.rs`, reemplazar el `hidden: false` por la lectura real. Al tope del archivo o en la fn:
```rust
#[cfg(windows)]
fn attrs_of(metadata: Option<&std::fs::Metadata>) -> (bool, bool) {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    match metadata {
        Some(m) => {
            let a = m.file_attributes();
            (a & FILE_ATTRIBUTE_HIDDEN != 0, a & FILE_ATTRIBUTE_SYSTEM != 0)
        }
        None => (false, false),
    }
}
#[cfg(not(windows))]
fn attrs_of(_metadata: Option<&std::fs::Metadata>) -> (bool, bool) {
    (false, false)
}
```
Y en `entry_from_path`, antes de construir el Entry:
```rust
    let (hidden, system) = attrs_of(metadata);
```
Cambiar `hidden: false` por `hidden,` y agregar `system,` en el literal del Entry.

- [ ] **Step 4: Compilar**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-core`
Expected: compila (arreglar todos los literales de Entry que falten `system`).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/fs_model.rs crates/core/src/listing.rs
git commit -m "feat(core): Entry lee atributos hidden y system reales de Windows"
```
Termina el mensaje con esta línea literal:
Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>

---

### Task 2: filtro de visibilidad puro `is_visible`

**Files:**
- Modify: `crates/core/src/fs_model.rs` (o el módulo de filtro/vista — ubicar con graphify)

- [ ] **Step 1: Orientarte**

`graphify query "view_indices filtro visible Entry vista filtered list_into_filtered ListingFilter"`.
Encuentra dónde se decide qué entries entran en la vista (puede ser `core::filter`, o
`list_into_filtered` en listing.rs). El `is_visible` se llamará desde ahí. Decide el archivo más
coherente (donde ya vive el filtrado de la vista).

- [ ] **Step 2: Test (falla)**

En el módulo elegido, `#[cfg(test)]`:
```rust
    #[test]
    fn visibilidad_segun_flags() {
        use crate::fs_model::{Entry, EntryKind};
        let mk = |name: &str, hidden: bool, system: bool| Entry {
            name: name.to_string(),
            path: std::path::PathBuf::from(name),
            kind: EntryKind::File,
            size: None, modified: None, created: None,
            hidden, system,
        };
        let normal = mk("a.txt", false, false);
        let oculto = mk("b.txt", true, false);
        let sis = mk("c.sys", false, true);
        let dot = mk(".gitignore", false, false);
        // show_hidden, show_system, hide_dotfiles
        assert!(is_visible(&normal, false, false, false));
        assert!(!is_visible(&oculto, false, false, false));  // oculto y no se muestran
        assert!(is_visible(&oculto, true, false, false));    // oculto pero se muestran
        assert!(!is_visible(&sis, true, false, false));      // sistema y no se muestran
        assert!(is_visible(&sis, true, true, false));        // sistema y se muestran
        assert!(is_visible(&dot, true, true, false));        // dotfile visible por defecto
        assert!(!is_visible(&dot, true, true, true));        // dotfile oculto por toggle
    }
```
Ajusta los campos del literal `Entry` a los REALES (puede tener más campos; agrégalos).

- [ ] **Step 3: Correr y ver que falla**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core visibilidad_segun_flags`
Expected: FAIL — `is_visible` no existe.

- [ ] **Step 4: Implementar**

```rust
/// ¿El entry debe mostrarse según los toggles de visibilidad? Puro.
pub fn is_visible(entry: &crate::fs_model::Entry, show_hidden: bool, show_system: bool, hide_dotfiles: bool) -> bool {
    if entry.hidden && !show_hidden {
        return false;
    }
    if entry.system && !show_system {
        return false;
    }
    if hide_dotfiles && entry.name.starts_with('.') {
        return false;
    }
    true
}
```

- [ ] **Step 5: Correr y ver que pasa**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core visibilidad_segun_flags`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/<archivo>.rs
git commit -m "feat(core): is_visible (filtro de ocultos/sistema/dotfiles)"
```
(con la línea Co-Authored-By literal).

---

### Task 3: Settings (show_hidden/show_system/hide_dotfiles)

**Files:**
- Modify: `crates/core/src/config/mod.rs`

- [ ] **Step 1: Test (falla)**

En `#[cfg(test)]` de config/mod.rs:
```rust
    #[test]
    fn settings_visibilidad_defaults() {
        let s = Settings::default();
        assert!(s.show_hidden);     // mostrar todo por defecto (pedido de Nicolás)
        assert!(s.show_system);
        assert!(!s.hide_dotfiles);
    }
```

- [ ] **Step 2: Correr y ver que falla**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core settings_visibilidad`
Expected: FAIL.

- [ ] **Step 3: Agregar los campos + fn default + Default for Settings**

En `struct Settings` (al final, antes del `}`):
```rust
    /// Mostrar archivos/carpetas con atributo oculto. Default true (Naygo muestra todo).
    #[serde(default = "default_show_hidden")]
    pub show_hidden: bool,
    /// Mostrar archivos/carpetas con atributo de sistema. Default true.
    #[serde(default = "default_show_system")]
    pub show_system: bool,
    /// Ocultar los que empiezan con punto (dotfiles estilo Linux). Default false.
    #[serde(default)]
    pub hide_dotfiles: bool,
```
Funciones default (junto a las otras `fn default_*`):
```rust
fn default_show_hidden() -> bool { true }
fn default_show_system() -> bool { true }
```
En `impl Default for Settings` (o `Settings::default()`), agregar:
```rust
            show_hidden: true,
            show_system: true,
            hide_dotfiles: false,
```

- [ ] **Step 4: Correr y ver que pasa**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core settings_visibilidad`
Expected: PASS. Corre `cargo test -p naygo-core --lib` (no romper round-trip de settings; si hay
un test que construye Settings literal, agregar los 3 campos ahí).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/config/mod.rs
git commit -m "feat(core): Settings show_hidden/show_system/hide_dotfiles (mostrar todo por defecto)"
```
(con la línea Co-Authored-By literal).

---

### Task 4: Menú "ojo" en el toolbar + aplicar el filtro

**Files:**
- Modify: `crates/ui-slint/ui/app-window.slint` (botón ojo + menú), `crates/ui-slint/src/workspace_ctrl.rs` (aplicar is_visible en la vista + árbol), `crates/ui-slint/src/main.rs` (cablear toggles), `crates/ui-slint/src/config_ctrl.rs` (getters/setters), i18n.

- [ ] **Step 1: Orientarte**

`graphify query "view_indices construir vista panel filtro workspace_ctrl arbol tree_listing
ListingFilter DirsOnly"`. Encuentra dónde se construye la vista del panel (los índices visibles) y
dónde se listan los nodos del árbol. Ahí aplicarás `is_visible` con los 3 flags de settings.

- [ ] **Step 2: Aplicar is_visible en la vista de los paneles y el árbol**

Donde se arma la vista (view_indices) de un panel Files, filtrar los entries con
`naygo_core::<modulo>::is_visible(e, settings.show_hidden, settings.show_system, settings.hide_dotfiles)`
antes de incluirlos. Igual en la construcción de nodos del árbol (solo-carpetas). AJUSTA al punto
real donde se filtra/ordena hoy (puede que ya exista un filtro de columna; el de visibilidad se
aplica ANTES o en conjunto). Mantener el patrón existente.

- [ ] **Step 3: Botón "ojo" + menú en el toolbar**

En `app-window.slint`, agregar un ToolBtn con un ícono de "ojo" dibujado con Path (un óvalo + un
círculo central) + ▾, y un menú flotante (patrón de los menús ▾ existentes — busca cómo se hace el
menú de historial/USB) con 3 casillas: "Mostrar archivos ocultos", "Mostrar archivos de sistema",
"Ocultar archivos que empiezan con punto", enlazadas a props in-out `show-hidden`/`show-system`/
`hide-dotfiles`. Al alternar una, callback a Rust.

- [ ] **Step 4: Cablear en main.rs + config_ctrl**

En config_ctrl.rs: getters/setters `show_hidden()/set_show_hidden(bool)` (+ system, dotfiles) que
persisten (patrón de otros setters). En main.rs: cablear los callbacks del menú → setter + re-armar
las vistas de todos los paneles y el árbol (refresco) para que el filtro se aplique al instante.
Alimentar las props del menú con los valores de settings al iniciar.

- [ ] **Step 5: i18n**

es/en + i18n.slint + i18n_keys.rs: `view.hidden_tip` ("Mostrar/ocultar archivos"),
`view.show_hidden` ("Mostrar archivos ocultos"), `view.show_system` ("Mostrar archivos de sistema"),
`view.hide_dotfiles` ("Ocultar los que empiezan con punto"). en: "Show/hide files", "Show hidden
files", "Show system files", "Hide dotfiles". NO reutilizar.

- [ ] **Step 6: Gate + commit**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio.
```bash
git add crates/ui-slint/ui/app-window.slint crates/ui-slint/src/workspace_ctrl.rs crates/ui-slint/src/main.rs crates/ui-slint/src/config_ctrl.rs crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): menú de visibilidad (ocultos/sistema/dotfiles) en el toolbar"
```
(con la línea Co-Authored-By literal).

---

# FASE 2 — Temas: reducir a 5 + editor de colores

### Task 5: Reducir el catálogo de temas a 5

**Files:**
- Modify: `crates/core/src/theme/mod.rs`
- Delete: 10 `.json` en `crates/core/src/theme/builtin/`

- [ ] **Step 1: Orientarte**

`graphify query "ThemeCatalog load builtin include_str catalog temas"`. Lee `ThemeCatalog::load`
(theme/mod.rs:244) y el bloque de `const *_JSON = include_str!`.

- [ ] **Step 2: Recortar**

Conservar SOLO estos 5 en el array de `load` y sus `include_str!`: `dark-blue`, `winxp`,
`green-on-blue`, `high-contrast`, `neon-retro`. Quitar del array Y borrar los `const *_JSON` de:
`dark-teal`, `light`, `citrus-glow`, `ocean-midnight`, `ember-forge`, `polar-graphite`, `macos`,
`solarized-dark`, `amber-terminal`, `plum-dusk`. Borrar también esos 10 archivos en `builtin/`.
Ajustar el test `catalog_tiene_los_cuatro_embebidos` (o como se llame el de conteo) al nuevo número.

- [ ] **Step 3: Verificar el fallback de tema inexistente**

Asegurar que si el tema activo guardado era uno borrado, `get()` cae al default (ya lo hace). Si hay
un test, confírmalo; si no, agrega uno:
```rust
    #[test]
    fn tema_borrado_cae_al_default() {
        let dir = tempfile::tempdir().unwrap();
        let cat = ThemeCatalog::load(dir.path(), &ThemeId::new("macos")); // ya no existe
        // get del id viejo debe devolver el default, no panic
        let _ = cat.get(&ThemeId::new("macos"));
        assert!(cat.available().iter().any(|t| t.as_str() == "dark-blue"));
        assert!(!cat.available().iter().any(|t| t.as_str() == "macos"));
    }
```

- [ ] **Step 4: Gate + commit**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core theme; cargo clippy -p naygo-core --all-targets -- -D warnings`
Expected: PASS.
```bash
git add crates/core/src/theme/mod.rs crates/core/src/theme/builtin
git commit -m "feat(core): reducir temas de fábrica a 5 (Dark Blue, Win XP, Verde sobre azul, High Contrast, Neón Retro)"
```
(con la línea Co-Authored-By literal).

---

### Task 6: Helpers de tema de usuario (slug, guardar, distinguir fábrica)

**Files:**
- Modify: `crates/core/src/theme/mod.rs`

- [ ] **Step 1: Test (falla)**

```rust
    #[test]
    fn slug_de_nombre() {
        assert_eq!(theme_slug("Mi Tema"), "mi-tema");
        assert_eq!(theme_slug("Azul / Noche 2"), "azul-noche-2");
        assert!(!theme_slug("").is_empty()); // nombre vacío → algo usable
    }

    #[test]
    fn ids_de_fabrica() {
        assert!(is_builtin_id("dark-blue"));
        assert!(!is_builtin_id("mi-tema"));
    }
```

- [ ] **Step 2: Correr y ver que falla**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core slug_de_nombre ids_de_fabrica`
Expected: FAIL.

- [ ] **Step 3: Implementar**

```rust
/// Los ids de los 5 temas de fábrica (embebidos), que NO se editan/borran.
pub const BUILTIN_THEME_IDS: &[&str] = &["dark-blue", "winxp", "green-on-blue", "high-contrast", "neon-retro"];

/// ¿Es un tema de fábrica? (no editable/borrable; solo duplicable).
pub fn is_builtin_id(id: &str) -> bool {
    BUILTIN_THEME_IDS.contains(&id)
}

/// Convierte un nombre legible a un id-slug (minúsculas, espacios/símbolos → guiones).
pub fn theme_slug(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in name.trim().chars().flat_map(|c| c.to_lowercase()) {
        if c.is_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() { "tema".to_string() } else { s }
}
```

- [ ] **Step 4: Correr y ver que pasa + commit**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core slug_de_nombre ids_de_fabrica`
Expected: PASS.
```bash
git add crates/core/src/theme/mod.rs
git commit -m "feat(core): helpers de tema de usuario (slug, is_builtin_id)"
```
(con la línea Co-Authored-By literal).

> Para guardar un tema de usuario, la UI usará `Theme::to_json()` (ya existe) y escribirá en
> `<config>/themes/<slug>.json` (el `load` ya levanta esos sueltos). No hace falta función nueva
> para escribir si la UI ya tiene el helper de escribir archivos de config; si no, agregar
> `save_user_theme(dir, id, &theme)` aquí.

---

### Task 7: Selector de color estilo Office (componente Slint)

**Files:**
- Create: `crates/ui-slint/ui/color-picker.slint`

- [ ] **Step 1: Componente del selector**

`graphify query "config-window selector color theme combo ThemeCombo slint componente"` para ver
patrones. Crear `color-picker.slint` con el header del proyecto. Componente `ColorPicker`:
- `in-out property <color> value;` (o `in-out property <string> hex;` — elige; lo importante es que
  emita el color elegido).
- callback `changed(string)` (hex elegido).
- **Grilla de presets**: filas de swatches (Rectangle de color, clic → emite ese hex). Una fila
  base (blanco/negro/grises + colores del tema) + 4 filas de variaciones (claro→oscuro) + fila
  "Colores estándar" (rojo/naranja/amarillo/verde/cian/azul/morado puros). Hardcodea una paleta
  razonable de hex (estilo Office).
- **"Más colores…"**: un botón que muestra/oculta una sección con 3 sliders (R/G/B 0-255) + un
  `LineEdit` hex + una muestra grande. Slider = un Rectangle de fondo con un "thumb" arrastrable
  (TouchArea con moved → calcular valor por posición x). Bidireccional: mover slider actualiza hex
  y muestra; escribir hex actualiza sliders.

> Slint 1.16 NO tiene Slider que sirva con color picker; el slider se dibuja a mano (Rectangle +
> TouchArea). Si construir el slider arrastrable resulta complejo, alternativa aceptable: 3 LineEdit
> numéricos (R/G/B) + el hex, sin arrastre — reporta si tomas ese camino. Lo esencial: elegir
> cualquier color por presets o por valor exacto.

- [ ] **Step 2: Build**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint`
Expected: compila (el componente sin usar aún es ok, o instánciarlo en una prueba mínima).

- [ ] **Step 3: Commit**

```bash
git add crates/ui-slint/ui/color-picker.slint
git commit -m "feat(ui): componente selector de color estilo Office (presets + sliders + hex)"
```
(con la línea Co-Authored-By literal).

---

### Task 8: Editor de temas en config + preview en vivo

**Files:**
- Modify: `crates/ui-slint/ui/config-window.slint`, `crates/ui-slint/src/config_ctrl.rs`, `crates/ui-slint/src/main.rs`, i18n.

- [ ] **Step 1: Orientarte**

`graphify query "config-window apariencia galería temas ThemeCombo theme_apply apply config_ctrl
set_theme available"`. Lee cómo la galería de temas lista los temas y cómo se aplica uno
(`theme_apply::apply`). Ese `apply` es el del preview en vivo.

- [ ] **Step 2: Botones por tema (Personalizar / Editar / Eliminar)**

En la galería de Apariencia: para cada tema, si `is_builtin_id` → botón "Personalizar" (duplica);
si es de usuario → "Editar" + "Eliminar". config_ctrl: métodos `duplicate_theme(src_id) ->
nuevo_id_editable` (copia el Theme, nombre "<nombre> (copia)", slug nuevo, NO guarda aún),
`delete_user_theme(id)` (borra el .json y quita del catálogo; refuse si is_builtin).

- [ ] **Step 3: Estado del editor + los 11 tokens**

En config_ctrl: un estado de "tema en edición" (`Option<Theme>` + nombre + base). Exponer a la UI
los 11 colores como hex (para las muestras) y un método `set_token_color(token_idx, hex)` que
actualiza el Theme en edición y **aplica el preview en vivo** (llamar el mismo camino que aplica un
tema — `theme_apply::apply` con el Theme en edición). La lista de 11 tokens en config-window: cada
fila (muestra + nombre + hex) abre el `ColorPicker` (Task 7) para ese token; su `changed(hex)` →
`set_token_color`.

- [ ] **Step 4: Preview vivo + Guardar/Cancelar/Restaurar**

- Al entrar al editor, recordar el tema activo previo (`prev_theme_id`).
- Cada cambio de color → aplica el Theme en edición a toda la app (preview vivo).
- **Guardar**: `Theme::to_json()` → `<config>/themes/<slug>.json`; recargar catálogo; dejar ese
  tema como activo; persistir settings.theme.
- **Cancelar**: re-aplicar `prev_theme_id` (revertir el preview) y descartar el tema en edición.
- **Restaurar de fábrica**: re-cargar los valores del tema de fábrica del que se duplicó (resetea
  los tokens en edición a los originales) y re-aplicar el preview.

- [ ] **Step 5: i18n**

es/en + i18n.slint + i18n_keys.rs: `theme.customize` ("Personalizar"), `theme.edit` ("Editar"),
`theme.delete` ("Eliminar tema"), `theme.save` ("Guardar tema"), `theme.restore` ("Restaurar de
fábrica"), `theme.more_colors` ("Más colores…"), `theme.standard_colors` ("Colores estándar"),
`theme.name` ("Nombre del tema"), `theme.base` ("Base"), + nombres de los 11 tokens
(`theme.tok.accent` "Acento", `theme.tok.panel_bg` "Fondo del panel", etc.). en: traducir. NO
reutilizar claves.

- [ ] **Step 6: Gate + commit**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio.
```bash
git add crates/ui-slint/ui/config-window.slint crates/ui-slint/src/config_ctrl.rs crates/ui-slint/src/main.rs crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): editor de temas con selector de color y preview en vivo"
```
(con la línea Co-Authored-By literal).

---

# FASE 3 — Favoritos con grupos anidados

### Task 9: Modelo de árbol de favoritos + migración (core)

**Files:**
- Modify: `crates/core/src/favorites.rs`

> VERIFICADO: hoy `Favorites { items: Vec<Favorite> }`, `Favorite { path, label }`. Métodos
> `contains/toggle/remove/list/to_json/from_json`. Los consumidores: path-bar (★), panel Favoritos,
> árbol anclado, atajos Ctrl+1..9.

- [ ] **Step 1: Tests del árbol + migración (fallan)**

```rust
    #[test]
    fn arbol_grupos_y_favoritos() {
        let mut f = Favorites::new();
        f.add_favorite(&p("D:/a"));
        let g = f.new_group(None, "Trabajo");        // grupo en la raíz, devuelve su id/ruta
        f.move_into_group(&p("D:/a"), &g);            // mover el favorito al grupo
        let flat = f.list_flat();                     // recorrido para Ctrl+1..9
        assert!(flat.iter().any(|fav| fav.path == p("D:/a")));
        assert!(f.contains(&p("D:/a")));
    }

    #[test]
    fn migra_formato_plano_viejo() {
        // JSON viejo: {"items":[{"path":"D:/x","label":"x"}]}
        let viejo = r#"{"items":[{"path":"D:/x","label":"x"}]}"#;
        let f = Favorites::from_json(viejo);
        assert!(f.contains(&p("D:/x")), "el favorito viejo debe migrar al árbol");
    }
```
> AJUSTA las firmas (`new_group`/`move_into_group`/`add_favorite`/`list_flat`) a como las definas en
> el Step 3. Lo esencial: árbol con grupos + favoritos, y que el JSON plano viejo se migre.

- [ ] **Step 2: Correr y ver que fallan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core arbol_grupos migra_formato`
Expected: FAIL.

- [ ] **Step 3: Implementar el modelo de árbol + migración**

```rust
/// Un nodo del árbol de favoritos: una carpeta favorita o un grupo con hijos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FavNode {
    Favorite { path: PathBuf, label: String },
    Group { name: String, children: Vec<FavNode> },
}

/// Árbol de favoritos (formato nuevo).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Favorites {
    roots: Vec<FavNode>,
}
```
Métodos (puros): `add_favorite(path)` (agrega un FavNode::Favorite al raíz si no existe),
`contains(path)` (recorre el árbol), `remove(path)` (recorre y quita), `new_group(parent, name)`
(crea un grupo; `parent: Option<&GroupRef>` o por ruta lógica — decide un identificador de grupo
simple, p.ej. índice-path `Vec<usize>` o nombre-path; documenta), `rename_group`, `move_into_group`
/`move_node`, `list_flat()` (recorrido en orden devolviendo `Vec<Favorite>` para Ctrl+1..9),
`roots()` (para la UI). `to_json` serializa el árbol.

**Migración en `from_json`:** intentar parsear el formato nuevo (`{roots:[...]}`); si falla, intentar
el viejo (`{items:[{path,label}]}`) y convertir cada item a `FavNode::Favorite` en `roots`. Si ambos
fallan → `Favorites::default()`.
```rust
    pub fn from_json(s: &str) -> Self {
        if let Ok(nuevo) = serde_json::from_str::<Favorites>(s) {
            return nuevo;
        }
        #[derive(Deserialize)]
        struct Viejo { items: Vec<Favorite> }
        if let Ok(v) = serde_json::from_str::<Viejo>(s) {
            return Favorites {
                roots: v.items.into_iter().map(|f| FavNode::Favorite { path: f.path, label: f.label }).collect(),
            };
        }
        Favorites::default()
    }
```
> Mantén `Favorite { path, label }` como tipo (lo usa list_flat y la migración). Ajusta `contains`/
> `toggle` que usan los consumidores actuales para que sigan compilando (toggle puede agregar/quitar
> al raíz). Si cambias firmas públicas, ARREGLA los llamadores en la UI (la siguiente task) — o
> mantén métodos compatibles.

- [ ] **Step 4: Correr y ver que pasan + arreglar llamadores de core**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core favorites`
Expected: PASS. Luego `cargo build -p naygo-ui-slint` para ver qué llamadores de la UI rompieron
con el cambio de modelo; déjalos compilando (mínimo: `list_flat()` donde antes usaban `list()`).
Si es mucho, esta task puede dejar la UI con un shim temporal y la Task 10 la cablea bien — reporta.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/favorites.rs
git commit -m "feat(core): favoritos como árbol de grupos anidados + migración del formato plano"
```
(con la línea Co-Authored-By literal).

---

### Task 10: Panel de Favoritos editable + ícono ▾ en el toolbar

**Files:**
- Modify: `crates/ui-slint/ui/favorites-panel.slint`, `crates/ui-slint/ui/app-window.slint`, `crates/ui-slint/src/workspace_ctrl.rs`, `crates/ui-slint/src/main.rs`, i18n.

- [ ] **Step 1: Orientarte**

`graphify query "favorites-panel favorite_rows panel Favoritos PanePurpose menú historial flotante
add_pane_of clic derecho contexto"`. Lee cómo el panel Favoritos lista hoy (`favorite_rows`), cómo
se hace un menú flotante ▾ (historial/USB), y cómo se hace un menú contextual (clic derecho) en
otros paneles.

- [ ] **Step 2: Panel de Favoritos como árbol editable**

En `favorites-panel.slint`: mostrar el árbol (grupos expandibles con indentación + favoritos). Clic
en favorito → navega. Clic derecho → menú: "Nuevo grupo", "Renombrar", "Eliminar", "Mover a…".
Construir las filas desde `favorites.roots()` (en workspace_ctrl/bridge, una función que aplane el
árbol a filas con nivel de indentación + tipo grupo/favorito + expandido). Los métodos del
controlador (new_group/rename/remove/move) llaman a core y persisten.

> Drag para mover: si el drag dentro del árbol Slint resulta viable, úsalo; si no, el menú
> "Mover a… <grupo>" (submenú con los grupos existentes) cumple el requisito sin drag. Implementa
> el menú "Mover a…" como camino seguro; el drag queda como mejora.

- [ ] **Step 3: Ícono ▾ de favoritos en el toolbar**

En `app-window.slint`, un ToolBtn con ícono de estrella + ▾ → menú flotante que muestra el árbol de
favoritos jerárquico (grupos como sub-secciones/submenús, favoritos navegables). Clic en favorito →
`go_to_favorite(path)` (navega el panel activo). Solo navegación. Reusa el patrón del menú de
historial.

- [ ] **Step 4: Cablear en main.rs**

Callbacks del panel (new_group/rename/remove/move/navigate) y del menú ▾ (navigate) → controlador +
refrescar. Persistir el árbol tras cada cambio (to_json → favorites.json).

- [ ] **Step 5: i18n**

es/en + i18n.slint + i18n_keys.rs: `fav.menu_tip` ("Favoritos"), `fav.new_group` ("Nuevo grupo"),
`fav.rename` ("Renombrar"), `fav.delete` ("Eliminar"), `fav.move_to` ("Mover a…"). en: "Favorites",
"New group", "Rename", "Delete", "Move to…". NO reutilizar (ojo: ya existen claves `fav.*` del panel
viejo — reúsalas si encajan, no dupliques).

- [ ] **Step 6: Gate + commit**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio.
```bash
git add crates/ui-slint/ui/favorites-panel.slint crates/ui-slint/ui/app-window.slint crates/ui-slint/src/workspace_ctrl.rs crates/ui-slint/src/main.rs crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): panel de Favoritos editable (grupos) + menú ▾ de favoritos en el toolbar"
```
(con la línea Co-Authored-By literal).

---

# FASE 4 — Cierre

### Task 11: CHANGELOG + guía + gate final + dist

**Files:**
- Modify: `CHANGELOG.md`, `docs/GUIA-DE-USUARIO.md`

- [ ] **Step 1: CHANGELOG**

En `CHANGELOG.md`, "### Añadido":
```
- Menú de visibilidad en la barra de herramientas: mostrar u ocultar archivos ocultos, archivos de
  sistema, y los que empiezan con punto (estilo Linux). Por defecto Naygo los muestra todos.
- Favoritos organizables en grupos (carpetas) anidados, con un ícono en la barra para desplegarlos
  rápido y gestión (nuevo grupo, renombrar, mover, eliminar) en el panel de Favoritos.
- Editor de temas: crea tu propio tema duplicando uno existente y ajustando cada color con una
  paleta de colores y valores R/G/B, viendo el cambio en vivo. Los temas de fábrica quedan
  intactos.
```
Y en "### Cambiado":
```
- Los temas de fábrica se redujeron a cinco (Dark Blue, Windows XP, Verde sobre azul, High Contrast
  y Neón Retro); el resto se puede recrear con el editor de temas.
```

- [ ] **Step 2: Guía de usuario**

Agregar a `docs/GUIA-DE-USUARIO.md` las 3 funciones (visibilidad, favoritos con grupos, editor de
temas). Español neutral sin voseo.

- [ ] **Step 3: Gate final**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings
```
Expected: PASS, clippy limpio.

- [ ] **Step 4: Grafo + commit**

```
graphify update .
git add CHANGELOG.md docs/GUIA-DE-USUARIO.md
git commit -m "docs: visibilidad de archivos, favoritos con grupos y editor de temas"
```
(con la línea Co-Authored-By literal).

- [ ] **Step 5: Dist**

Run: `$env:CARGO_BUILD_JOBS = "2"; powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1`
Expected: `dist/Naygo-0.1.0-portable.zip` + `dist/Naygo-0.1.0-setup.exe`.

(Push de Nicolás. Verificación visual en VM: menú ojo muestra/oculta; favoritos con grupos se crean
y navegan; editor de temas cambia colores en vivo y guarda; los 5 temas de fábrica están.)

---

## Notas para el implementador

- **SIEMPRE correr el gate uno mismo tras cada subagente** (no confiar en su reporte).
- **graphify antes de grep** (hook obligatorio); inclúyelo en prompts de subagentes.
- **Slint 1.16**: no hay color picker ni slider nativos útiles → dibujar con Rectangle/Path/TouchArea;
  menús flotantes reusan el patrón existente (historial/USB); ToolBtn tooltip = `tip`; íconos por Path.
- **file_attributes() es std** en Windows (`std::os::windows::fs::MetadataExt`); NO requiere la crate `windows`.
- **theme_apply::apply** es el camino del preview en vivo de temas.
- **i18n triple, sin voseo, sin reutilizar claves.**
- Las 3 features son independientes — si una se complica, no bloquea a las otras.
- Un solo dist al final (Task 11). Nicolás hace el push.
