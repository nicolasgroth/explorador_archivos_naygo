# Íconos: galería + toolbar en caliente + enlace Apariencia↔Íconos — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hacer que el toolbar use el set de íconos elegido (hoy usa vectores fijos y no cambia), reemplazar el combo de set por una galería de tarjetas con preview (estilo temas), y enlazar la selección en Apariencia con la pestaña Íconos para personalizar.

**Architecture:** `core` gana 6 claves de acción nuevas (Home/Search/ShowHidden/History/Favorites/Split) para cubrir todos los botones del toolbar; el generador las rasteriza en los 5 sets (33→39 claves). La UI Slint expone una `image` por botón del toolbar alimentada desde el `IconCache` (quitando los `Path` fijos), reemplaza el combo de set por una galería de tarjetas `IconSetCardVm`, y agrega un botón "Personalizar" que activa el set y salta a la pestaña Íconos.

**Tech Stack:** Rust, Slint, serde, image, resvg/usvg/tiny-skia (generador, build-time), zip.

---

## Contexto y convenciones (leer antes de empezar)

- Rama: `feat/iconos-personalizables` (continúa la feature de íconos ya implementada).
- Header en archivos nuevos: `// Naygo — <desc>.` + `// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.`
- Nombres en inglés; comentarios/commits en español NEUTRAL (tú/impersonal, **nunca** voseo).
- ⚠️ **REGLA GIT (un subagente corrompió el árbol antes):** SOLO `git add <rutas explícitas>` + `git commit`. PROHIBIDO `git reset/restore/checkout/stash/clean`, `git commit -a`/`-am`, `git add -A`/`add .`. Hay 2 archivos ajenos modificados (`CLAUDE.md`, `crates/core/src/favorites.rs`) que NO se tocan ni se stagean. Si el árbol parece mal, PARAR y reportar — no "arreglarlo" con git.
- Build limpio + clippy + tests verdes antes de cada commit. Correr clippy uno mismo.
- Tests core: `cargo test -p naygo-core`. Build UI bin-only: `cargo build -p naygo-ui-slint --bins`. Generador: `cargo run -p naygo-core --bin gen_icons --features gen-icons`.
- Hay un test de parity i18n `es_en_tienen_las_mismas_claves`: toda clave nueva va en es.json Y en.json.

## Mapa de archivos

**Modificar:**
- `crates/core/src/icon_kind.rs` — +6 variantes `ActionIcon` + `file_name` + `all()`.
- `crates/core/src/icons/mod.rs` — +6 entradas en el macro `set_table!` (39 claves).
- `crates/core/src/bin/gen_icons.rs` — +6 `Map` por set (mapeo SVG verificado) + `ALL_KEYS` del test.
- `assets/icons/{lucide,tabler,material,flat-color,mono}/` — +6 PNG c/u (regenerados).
- `crates/core/src/i18n/es.json`, `en.json` — +6 `icons.obj.action_*` + clave del botón Personalizar.
- `crates/ui-slint/ui/types.slint` — `IconSetCardVm` + `SettingsVm.icon-set-cards`.
- `crates/ui-slint/ui/app-window.slint` — props `image` por botón del toolbar; quitar `draw-*` y sus Path.
- `crates/ui-slint/ui/config-window.slint` — galería en Apariencia + Íconos; botón Personalizar.
- `crates/ui-slint/src/main.rs` — llenar `refresh_toolbar_icons`, `icon-set-cards`, handler `personalize-icon-set`.
- `crates/ui-slint/src/i18n_keys.rs` — cablear clave del botón si se usa en markup.

---

## Fase 1 — Las 6 claves de acción nuevas en `core`

### Task 1: Agregar 6 variantes a `ActionIcon`

**Files:**
- Modify: `crates/core/src/icon_kind.rs` (enum `ActionIcon`, `all()`, `file_name()`, tests)

- [ ] **Step 1: Write the failing test**

Agregar/actualizar en los tests de `crates/core/src/icon_kind.rs` (hay un test `action_icon_all_son_20_con_file_name_unico` — actualizarlo a 26):

```rust
#[test]
fn action_icon_all_son_26_con_file_name_unico() {
    let all = ActionIcon::all();
    assert_eq!(all.len(), 26);
    let mut names: Vec<&str> = all.iter().map(|a| a.file_name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 26, "cada acción tiene un nombre de archivo único");
    assert!(all.iter().all(|a| a.file_name().starts_with("action_")));
}

#[test]
fn las_6_acciones_nuevas_existen() {
    use ActionIcon::*;
    let nuevos = [Home, Search, ShowHidden, History, Favorites, Split];
    let names: Vec<&str> = nuevos.iter().map(|a| a.file_name()).collect();
    assert_eq!(names, [
        "action_home", "action_search", "action_show_hidden",
        "action_history", "action_favorites", "action_split",
    ]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core icon_kind`
Expected: FAIL — las variantes no existen; el test de count espera 20.

- [ ] **Step 3: Write minimal implementation**

En `crates/core/src/icon_kind.rs`:

1. Agregar al `enum ActionIcon` (tras `Panel`):
```rust
    /// Ir a la carpeta de inicio.
    Home,
    /// Búsqueda recursiva (lupa).
    Search,
    /// Mostrar/ocultar archivos ocultos (ojo).
    ShowHidden,
    /// Historial de carpetas (reloj).
    History,
    /// Favoritos (estrella).
    Favorites,
    /// Dividir/partir la disposición de paneles.
    Split,
```

2. Agregar a `ActionIcon::all()` (al final del array):
```rust
            Home, Search, ShowHidden, History, Favorites, Split,
```

3. Agregar a `ActionIcon::file_name()` (antes del cierre del match):
```rust
            Home => "action_home",
            Search => "action_search",
            ShowHidden => "action_show_hidden",
            History => "action_history",
            Favorites => "action_favorites",
            Split => "action_split",
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core icon_kind`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/icon_kind.rs
git commit -m "feat(core): 6 acciones nuevas en ActionIcon (home/search/ocultos/historial/favoritos/dividir)"
```

---

### Task 2: Registrar las 6 claves en `icon_source` (key↔string)

**Files:**
- Modify: `crates/core/src/icon_source.rs` (verificar `key_from_string`, tests)

> `key_to_string` ya funciona (usa `file_name`). `key_from_string` resuelve las acciones
> vía `ActionIcon::all()`, así que las 6 nuevas ya deberían round-trip. Este task lo VERIFICA
> con un test explícito (no agrega lógica salvo que el round-trip falle).

- [ ] **Step 1: Write the failing test**

Agregar a los tests de `crates/core/src/icon_source.rs`:

```rust
#[test]
fn las_6_claves_nuevas_round_trip() {
    for name in [
        "action_home", "action_search", "action_show_hidden",
        "action_history", "action_favorites", "action_split",
    ] {
        let key = key_from_string(name).expect("clave válida");
        assert_eq!(key_to_string(key), name);
    }
}
```

- [ ] **Step 2: Run test to verify it fails (o pasa directo)**

Run: `cargo test -p naygo-core las_6_claves_nuevas_round_trip`
Expected: PASS directo (el round-trip ya está cubierto por `ActionIcon::all()`). Si FALLA,
es que `key_from_string` excluye acciones nuevas — en ese caso revisar el match directo
(no debe interceptar `action_*`) y dejar que caiga a la búsqueda en `ActionIcon::all()`.

- [ ] **Step 3: (solo si falló) ajustar `key_from_string`**

Si el test pasó, NO tocar código. Si falló, asegurar que el `match s { ... _ => None }` de
las claves directas NO incluya los `action_*`, dejándolos para el `ActionIcon::all().iter().find(...)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core icon_source`
Expected: PASS (todos).

- [ ] **Step 5: Commit (solo si hubo cambio o test nuevo)**

```bash
git add crates/core/src/icon_source.rs
git commit -m "test(core): round-trip de las 6 claves de acción nuevas"
```

---

## Fase 2 — Generar las 6 claves nuevas en los 5 sets

> ⚠️ ORDEN: este task debe completarse y los PNG commitearse ANTES de la Fase 3 (que
> los embebe con `include_bytes!`). Mismo gotcha que la entrega anterior.

### Task 3: Mapeo SVG de las 6 claves + regenerar los 5 sets

**Files:**
- Modify: `crates/core/src/bin/gen_icons.rs` (agregar 6 `Map` a cada set + `ALL_KEYS` del test)
- Modify (generados): `assets/icons/{lucide,tabler,material,flat-color,mono}/action_{home,search,show_hidden,history,favorites,split}.png`

- [ ] **Step 1: Actualizar el test de cobertura del generador**

En `crates/core/src/bin/gen_icons.rs`, el array `ALL_KEYS` del módulo de tests lista las
claves esperadas. Agregar las 6 nuevas al final:
```rust
    "action_home","action_search","action_show_hidden",
    "action_history","action_favorites","action_split",
```
(Ahora `ALL_KEYS` tiene 39 entradas; los tests `*_cubre_todas_las_claves` exigirán 39.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core --bin gen_icons --features gen-icons`
Expected: FAIL — cada set tiene 33 maps, el test exige 39.

- [ ] **Step 3: Agregar los 6 Map a cada set (mapeo VERIFICADO)**

Para CADA tabla (`LUCIDE`, `TABLER`, `MATERIAL`, `FLAT_COLOR`; `MONO` aliasa `LUCIDE`),
agregar 6 entradas. VERIFICA cada nombre SVG con `unzip -l <zip> | grep` antes de fijarlo
(material y flat-color nombran distinto; donde falte, elige el más cercano y deja un
comentario `//`). Candidatos ya verificados:

**LUCIDE** (zip `lucide-main.zip`, prefijo `lucide-main/icons/`):
```rust
    Map { key: "action_home",        svg: "house.svg" },   // lucide usa house, no home
    Map { key: "action_search",      svg: "search.svg" },
    Map { key: "action_show_hidden", svg: "eye.svg" },
    Map { key: "action_history",     svg: "history.svg" },
    Map { key: "action_favorites",   svg: "star.svg" },
    Map { key: "action_split",       svg: "split.svg" },
```

**TABLER** (zip `tabler-icons-main.zip`, prefijo `tabler-icons-main/icons/outline/`):
```rust
    Map { key: "action_home",        svg: "home.svg" },
    Map { key: "action_search",      svg: "search.svg" },
    Map { key: "action_show_hidden", svg: "eye.svg" },
    Map { key: "action_history",     svg: "history.svg" },
    Map { key: "action_favorites",   svg: "star.svg" },
    Map { key: "action_split",       svg: "layout-columns.svg" },
```

**MATERIAL** (zip `material-design-icons-master.zip`, prefijo `material-design-icons-master/src/`;
el `svg` lleva la ruta `<categoría>/<nombre>/materialicons/24px.svg`). VERIFICA con
`unzip -p material-design-icons-master.zip <ruta>` que existan. Candidatos a verificar:
```rust
    Map { key: "action_home",        svg: "action/home/materialicons/24px.svg" },
    Map { key: "action_search",      svg: "action/search/materialicons/24px.svg" },
    Map { key: "action_show_hidden", svg: "action/visibility/materialicons/24px.svg" },
    Map { key: "action_history",     svg: "action/history/materialicons/24px.svg" },
    Map { key: "action_favorites",   svg: "toggle/star/materialicons/24px.svg" },
    Map { key: "action_split",       svg: "action/view_column/materialicons/24px.svg" },
```
(Si alguna ruta no existe, busca la categoría correcta: `unzip -l material-design-icons-master.zip | grep "/<nombre>/materialicons/24px.svg"`.)

**FLAT_COLOR** (zip `flat-color-icons-master.zip`, prefijo `flat-color-icons-master/svg/`).
Set limitado; VERIFICA y usa sustitutos con comentario `// FIXME` si falta. Candidatos:
```rust
    Map { key: "action_home",        svg: "home.svg" },
    Map { key: "action_search",      svg: "search.svg" },
    Map { key: "action_show_hidden", svg: "view_details.svg" },   // verificar; si no, el más cercano
    Map { key: "action_history",     svg: "clock.svg" },          // verificar (icons8_clock?)
    Map { key: "action_favorites",   svg: "rating.svg" },         // estrella; verificar
    Map { key: "action_split",       svg: "grid.svg" },           // dividir; verificar
```
Para flat-color, lista los nombres reales con `unzip -l flat-color-icons-master.zip | grep -iE "home|search|eye|view|clock|time|star|rating|favorite|grid|split|column"` y elige; documenta cada sustituto con `// FIXME` como en la entrega anterior.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core --bin gen_icons --features gen-icons`
Expected: PASS (5 tests de cobertura, 39 claves c/u).

- [ ] **Step 5: Correr el generador y verificar los PNG**

Run: `cargo run -p naygo-core --bin gen_icons --features gen-icons`
Expected: "set 'lucide' generado (39 íconos)" … para los 5.
Verificar: `ls assets/icons/lucide/ | wc -l` → 40 (39 png + manifest.json) por set.
Inspeccionar 2-3 de los PNG nuevos (Read sobre el .png): tintables blancos, mono gris,
flat-color con color.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/bin/gen_icons.rs assets/icons/lucide assets/icons/tabler assets/icons/material assets/icons/flat-color assets/icons/mono
git commit -m "feat(gen): 6 claves de acción nuevas en los 5 sets (39 íconos c/u)"
```

---

## Fase 3 — Embeber las 39 claves + i18n de los 6 objetos

### Task 4: Embeber las 6 claves nuevas en `icons/mod.rs`

**Files:**
- Modify: `crates/core/src/icons/mod.rs` (macro `set_table!` + test de cobertura)

- [ ] **Step 1: Write the failing test**

En `crates/core/src/icons/mod.rs`, hay un test `cada_set_de_fabrica_cubre_las_33_claves`
(o similar) que itera `all_keys()`. Como `all_keys()` deriva de `ActionIcon::all()` (ya con
26), ahora produce 39 claves. El test ya las cubrirá; pero el `set_table!` macro lista las
claves EXPLÍCITAMENTE, así que sin agregar las 6 entradas, `bytes_for_id` devolverá vacío
para ellas y el test fallará. Renombrar el test a `..._39_claves` si menciona 33, y
verificar que itera `all_keys()`:

```rust
#[test]
fn cada_set_de_fabrica_cubre_las_39_claves() {
    for set in ["lucide", "tabler", "material", "flat-color", "mono"] {
        for key in all_keys() {
            assert!(!bytes_for_id(set, key).is_empty(), "asset vacío {set}/{:?}", key);
        }
    }
    assert_eq!(all_keys().len(), 39);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core cada_set_de_fabrica_cubre`
Expected: FAIL — las 6 claves nuevas no están en `set_table!`, `bytes_for_id` da vacío.
(Si no compila por el `include_bytes!` de un PNG faltante, es que la Fase 2 no se corrió:
hacerla primero.)

- [ ] **Step 3: Agregar las 6 entradas al macro `set_table!`**

En `crates/core/src/icons/mod.rs`, dentro de `macro_rules! set_table!`, en la lista de
`($konst, $set)`, agregar las 6 entradas (junto a las demás `action_*`):
```rust
            ("action_home", png!($set, "action_home")),
            ("action_search", png!($set, "action_search")),
            ("action_show_hidden", png!($set, "action_show_hidden")),
            ("action_history", png!($set, "action_history")),
            ("action_favorites", png!($set, "action_favorites")),
            ("action_split", png!($set, "action_split")),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core icons`
Expected: PASS (cobertura 39 claves × 5 sets).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/icons/mod.rs
git commit -m "feat(core): embeber las 6 claves de acción nuevas (39 por set)"
```

---

### Task 5: Labels i18n de los 6 objetos nuevos

**Files:**
- Modify: `crates/core/src/i18n/es.json`, `crates/core/src/i18n/en.json`

- [ ] **Step 1: Agregar las 6 claves a AMBOS JSON**

es.json (junto a las demás `icons.obj.action_*`):
```json
"icons.obj.action_home": "Inicio",
"icons.obj.action_search": "Buscar",
"icons.obj.action_show_hidden": "Mostrar ocultos",
"icons.obj.action_history": "Historial",
"icons.obj.action_favorites": "Favoritos",
"icons.obj.action_split": "Dividir paneles",
```
en.json:
```json
"icons.obj.action_home": "Home",
"icons.obj.action_search": "Search",
"icons.obj.action_show_hidden": "Show hidden",
"icons.obj.action_history": "History",
"icons.obj.action_favorites": "Favorites",
"icons.obj.action_split": "Split panes",
```

- [ ] **Step 2: Validar JSON + parity**

Run: `/c/Python313/python -c "import json; json.load(open('crates/core/src/i18n/es.json', encoding='utf-8')); json.load(open('crates/core/src/i18n/en.json', encoding='utf-8')); print('ok')"`
Run: `cargo test -p naygo-core i18n`
Expected: JSON válido + `es_en_tienen_las_mismas_claves` verde.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(i18n): labels de los 6 objetos de acción nuevos (ES/EN)"
```

---

## Fase 4 — Toolbar usa el set de íconos (en caliente)

> UI Slint: se verifica compilando `--bins` + prueba visual. El componente `ToolBtn`
> (app-window.slint:~79) YA tiene `in property <image> icon` y ya pinta
> `if root.icon.width > 0 && !root.draw-terminal: Image { source: root.icon }`. Los
> botones se instancian en markup con flags `draw-back: true`, etc. (no hay modelo).
> ENFOQUE: exponer una `image` por botón en `AppWindow`, alimentar cada `ToolBtn` con
> ella (quitando su `draw-*`), y llenar esas props desde Rust con el IconCache.

### Task 6: Props de imagen del toolbar en AppWindow + quitar draw-*

**Files:**
- Modify: `crates/ui-slint/ui/app-window.slint` (AppWindow props + ToolBtn instances + ToolBtn component)

- [ ] **Step 1: Declarar props image en AppWindow**

En el componente `AppWindow` (el `export component AppWindow inherits Window`), agregar una
`in property <image>` por botón del toolbar que hoy usa `draw-*`. Nombres sugeridos (kebab):
```slint
    in property <image> tb-back;
    in property <image> tb-forward;
    in property <image> tb-up;
    in property <image> tb-refresh;
    in property <image> tb-home;
    in property <image> tb-search;
    in property <image> tb-show-hidden;
    in property <image> tb-history;
    in property <image> tb-favorites;
    in property <image> tb-split;
    in property <image> tb-panel;
    in property <image> tb-tabs;
    in property <image> tb-swap;
    in property <image> tb-clone;
    in property <image> tb-new-folder;
    in property <image> tb-add-pane;
    in property <image> tb-layouts;
    in property <image> tb-settings;
    in property <image> tb-terminal;
```
(Una por cada `draw-*` real que tenga clave de set. Revisa la lista real de ToolBtn en el
markup ~líneas 1368-1610 y mapea cada `draw-X` a su `tb-X`. Hay ToolBtn con nombre como
`view-btn` (draw-eye→tb-show-hidden), `history-btn`, `fav-btn`, `split-btn`, `panel-btn`,
`term-btn`, `layouts-btn` — úsalos para ubicarlos.)

- [ ] **Step 2: Alimentar cada ToolBtn con su image y quitar el draw-***

Para cada `ToolBtn { draw-X: true; ... }` en el markup del toolbar, reemplazar
`draw-X: true;` por `icon: root.tb-X;`. Ejemplo (back):
```slint
// antes:
ToolBtn { draw-back: true; ... }
// después:
ToolBtn { icon: root.tb-back; ... }
```
Hacerlo para los 18/19 botones. El botón de terminal tiene un caso especial
(`!root.draw-terminal` en el `if` del Image): al quitar `draw-terminal`, el Image se pinta
normal — quitar también la condición `&& !root.draw-terminal` del `if` del Image en el
componente ToolBtn (línea ~149) para que el ícono de terminal del set se muestre.

- [ ] **Step 3: Limpiar el componente ToolBtn**

En el componente `ToolBtn` (~79-330), eliminar TODAS las `in property <bool> draw-*` y sus
bloques `if root.draw-X: Rectangle { Path { ... } }`. El botón queda con solo el `Image`
(de `root.icon`) + el texto. Mantener `icon-tint` solo si algún otro uso lo necesita; los
íconos del set ya vienen teñidos del IconCache, así que el Image NO debe re-teñir (no aplica
colorize sobre un PNG ya teñido). Verificar que no quede ningún `draw-` huérfano:
`grep -n "draw-" crates/ui-slint/ui/app-window.slint` debe quedar vacío (o solo comentarios).

- [ ] **Step 4: Compilar**

Run: `cargo build -p naygo-ui-slint --bins`
Expected: compila. Los botones aún se verán SIN ícono hasta el Task 7 (Rust llena las props),
pero debe compilar. (Si una prop tb-* no se usa o falta, el compilador de Slint avisa.)

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/ui/app-window.slint
git commit -m "feat(ui): toolbar pinta el ícono del set (props image, sin Path fijo)"
```

---

### Task 7: Llenar `refresh_toolbar_icons` desde el IconCache

**Files:**
- Modify: `crates/ui-slint/src/main.rs` (cuerpo de `refresh_toolbar_icons`, hoy no-op)

- [ ] **Step 1: Implementar el cuerpo (sin test unitario; verificación en vivo)**

`refresh_toolbar_icons` está en main.rs:~2393 como `Rc::new(move || {})` (no-op) y YA está
conectado a los handlers de cambio de set/tema (7 call-sites). Llenar su cuerpo para que,
por cada botón del toolbar, tome el `Image` del IconCache y lo setee en la prop `tb-*` de
AppWindow. Necesita `ctrl` (para `c.icons`) y `ui_weak`. Reescribir:

```rust
let refresh_toolbar_icons: Rc<dyn Fn()> = {
    let ctrl = ctrl.clone();
    let ui_weak = ui.as_weak();
    Rc::new(move || {
        let Some(ui) = ui_weak.upgrade() else { return; };
        use naygo_core::icon_kind::{ActionIcon, IconKey};
        let mut c = ctrl.borrow_mut();
        // (key del toolbar, setter de la prop tb-*)
        let pairs: [(IconKey, fn(&AppWindow, slint::Image)); 19] = [
            (IconKey::Action(ActionIcon::Back), AppWindow::set_tb_back),
            (IconKey::Action(ActionIcon::Forward), AppWindow::set_tb_forward),
            (IconKey::Action(ActionIcon::Up), AppWindow::set_tb_up),
            (IconKey::Action(ActionIcon::Refresh), AppWindow::set_tb_refresh),
            (IconKey::Action(ActionIcon::Home), AppWindow::set_tb_home),
            (IconKey::Action(ActionIcon::Search), AppWindow::set_tb_search),
            (IconKey::Action(ActionIcon::ShowHidden), AppWindow::set_tb_show_hidden),
            (IconKey::Action(ActionIcon::History), AppWindow::set_tb_history),
            (IconKey::Action(ActionIcon::Favorites), AppWindow::set_tb_favorites),
            (IconKey::Action(ActionIcon::Split), AppWindow::set_tb_split),
            (IconKey::Action(ActionIcon::Panel), AppWindow::set_tb_panel),
            (IconKey::Action(ActionIcon::Tabs), AppWindow::set_tb_tabs),
            (IconKey::Action(ActionIcon::SwapPanes), AppWindow::set_tb_swap),
            (IconKey::Action(ActionIcon::ClonePath), AppWindow::set_tb_clone),
            (IconKey::Action(ActionIcon::NewFolder), AppWindow::set_tb_new_folder),
            (IconKey::Action(ActionIcon::AddPane), AppWindow::set_tb_add_pane),
            (IconKey::Action(ActionIcon::Layouts), AppWindow::set_tb_layouts),
            (IconKey::Action(ActionIcon::Settings), AppWindow::set_tb_settings),
            (IconKey::Action(ActionIcon::Terminal), AppWindow::set_tb_terminal),
        ];
        for (key, setter) in pairs {
            let img = c.icons.get(key);
            setter(&ui, img);
        }
    })
};
refresh_toolbar_icons();
```
> Ajustar: el tipo del setter (`AppWindow::set_tb_back`) es el setter generado por Slint para
> la prop `tb-back`. Si el array tipado da problemas de tipo con los fn pointers, usar un
> bloque por botón (`ui.set_tb_back(c.icons.get(IconKey::Action(ActionIcon::Back)));`) — más
> verboso pero infalible. Mapea CADA prop tb-* declarada en Task 6 (mismo conjunto). Si algún
> nombre de ActionIcon difiere (p.ej. el botón "new" del toolbar es NewFolder o NewFile),
> revisar el markup para saber cuál clave corresponde a cada botón.

- [ ] **Step 2: Compilar + verificación en vivo**

Run: `cargo build -p naygo-ui-slint --bins`, luego `cargo run -p naygo-ui-slint --bin naygo`
Verificar: los botones del toolbar muestran los íconos del set activo; al cambiar de set en
Configuración, el toolbar cambia en caliente; al cambiar de tema, los íconos del toolbar se
retiñen.

- [ ] **Step 3: Commit**

```bash
git add crates/ui-slint/src/main.rs
git commit -m "feat(ui): cablear refresh_toolbar_icons al IconCache (toolbar en caliente)"
```

---

## Fase 5 — Galería de tarjetas de sets + enlace Apariencia↔Íconos

### Task 8: VM `IconSetCardVm` + llenado en Rust

**Files:**
- Modify: `crates/ui-slint/ui/types.slint` (struct + campo en SettingsVm)
- Modify: `crates/ui-slint/src/main.rs` (llenar `icon-set-cards` en build_settings_vm)

- [ ] **Step 1: VM en types.slint**

Agregar:
```slint
// Una tarjeta de set en la galería (preview estilo temas).
export struct IconSetCardVm {
    id: string,
    label: string,
    samples: [image],   // ~5 íconos de muestra del set, ya teñidos
    selected: bool,
}
```
Y a `SettingsVm`:
```slint
    icon-set-cards: [IconSetCardVm],
```

- [ ] **Step 2: Llenar `icon-set-cards` en `build_settings_vm`**

En `crates/ui-slint/src/main.rs`, donde se arman las listas del VM (ya se llenan
`icon_rows`/`icon_set_labels` con acceso a `c.icons`), agregar el llenado de
`icon_set_cards`. Por cada set del catálogo, renderizar ~5 muestras con la función
`render_for_set` (ya existe, de la entrega anterior — `render_for_set(key, set_id, tintable,
tint, config_dir) -> Image`). Muestras sugeridas: folder, file_image, action_copy,
action_settings, action_back. Construir:

```rust
use naygo_core::icon_kind::{ActionIcon, FileCategory, IconKey};
let sample_keys = [
    IconKey::Folder,
    IconKey::File(FileCategory::Image),
    IconKey::Action(ActionIcon::Copy),
    IconKey::Action(ActionIcon::Settings),
    IconKey::Action(ActionIcon::Back),
];
let cat = naygo_core::icon_set::IconSetCatalog::load(&c.config_dir);
let active = c.settings.icon_set.clone();
let tint = theme_text_rgb(&c.settings, &c.themes);  // (u8,u8,u8)
let cards: Vec<IconSetCardVm> = cat.available().iter().map(|info| {
    let tintable = info.tintable;
    let samples: Vec<slint::Image> = sample_keys.iter()
        .map(|k| crate::icons::render_for_set(*k, &info.id, tintable, tint, &c.config_dir))
        .collect();
    IconSetCardVm {
        id: info.id.as_str().into(),
        label: info.label.as_str().into(),
        samples: ModelRc::from(Rc::new(VecModel::from(samples))),
        selected: info.id == active,
    }
}).collect();
// asignar al settings_vm: settings_vm.icon_set_cards = ModelRc::from(Rc::new(VecModel::from(cards)));
```
> Ajustar a la firma real de `render_for_set` (verifícala en `crates/ui-slint/src/icons.rs`)
> y a cómo `build_settings_vm` obtiene `c` (ConfigCtrl) vs el IconCache. Si `build_settings_vm`
> solo tiene `&ConfigCtrl`, usar el mismo patrón con que se llenó `icon_rows` (que ya resolvió
> el acceso al cache/config). `render_for_set` NO necesita el IconCache (resuelve bytes +
> tiñe + decodifica directo), así que basta con `&ConfigCtrl` + el config_dir + el theme rgb.

- [ ] **Step 3: Compilar**

Run: `cargo build -p naygo-ui-slint --bins`
Expected: compila (el VM se llena; aún no se pinta hasta el Task 9).

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/ui/types.slint crates/ui-slint/src/main.rs
git commit -m "feat(ui): VM IconSetCardVm + render de las muestras de cada set"
```

---

### Task 9: Galería en Apariencia + pestaña Íconos (reemplaza el combo) + botón Personalizar

**Files:**
- Modify: `crates/ui-slint/ui/config-window.slint` (Apariencia cat 3 + Íconos cat 9 + callback)
- Modify: `crates/ui-slint/src/main.rs` (handler personalize-icon-set)
- Modify: `crates/core/src/i18n/es.json`, `en.json` (clave del botón Personalizar)

- [ ] **Step 1: Clave i18n del botón**

Agregar a ambos JSON (parity):
- es: `"settings.icons.personalize": "Personalizar"`
- en: `"settings.icons.personalize": "Customize"`
Validar JSON + `cargo test -p naygo-core i18n`.
Cablear en `i18n_keys.rs` si se usa como `Tr.*` en markup (patrón `tr.set_icons_personalize(...)`)
y agregar el campo Tr en `ui/i18n.slint` (`in property <string> icons-personalize;`).

- [ ] **Step 2: Declarar el callback**

En `config-window.slint`, junto a los otros callbacks de íconos:
```slint
callback personalize-icon-set(string);   // (set_id) — activa el set y salta a la pestaña Íconos
```

- [ ] **Step 3: Galería en Apariencia (cat 3)**

En la sección `if root.cat == 3` (Apariencia), localizar el `Field { label: Tr.cfg-icon-set;
ThemeCombo { ... } }` (el combo de set) y REEMPLAZARLO por una galería de tarjetas. Usar como
plantilla el markup de la galería de TEMAS que está en esa misma sección (las tarjetas con
muestras + nombre + botón). Estructura:
```slint
Text { text: Tr.cfg-icon-set; color: Theme.text-dim; font-weight: 700; }
// galería responsive de tarjetas de set
for card in root.vm.icon-set-cards: Rectangle {
    // tarjeta: borde de acento si card.selected
    border-width: card.selected ? 2px : 1px;
    border-color: card.selected ? Theme.accent : Theme.border;
    border-radius: 8px;
    background: Theme.row-bg;
    VerticalLayout {
        padding: 8px; spacing: 6px;
        // fila de muestras
        HorizontalLayout {
            spacing: 4px;
            for s in card.samples: Image { source: s; width: 18px; height: 18px; }
        }
        Text { text: card.label; }
        // botón Personalizar
        CfgBtn { label: Tr.icons-personalize; clicked => { root.personalize-icon-set(card.id); } }
    }
    TouchArea { clicked => { root.set-icon-set(card.id); } }  // click en cuerpo activa el set
}
```
> Copiar el layout EXACTO de la galería de temas (grid/flow, anchos, gaps) para que se vea
> igual. Cuida que el TouchArea del cuerpo no se trague el clic del botón Personalizar
> (el CfgBtn debe quedar encima / el TouchArea no cubrirlo, igual que en las tarjetas de tema).

- [ ] **Step 4: Misma galería en la pestaña Íconos (cat 9)**

En `if root.cat == 9` (Íconos), reemplazar el selector de "Set base" actual (el combo que se
puso en la entrega anterior) por la MISMA galería (`for card in root.vm.icon-set-cards`),
arriba de la grilla de objetos. Sin el botón Personalizar acá (ya estás en Íconos) — o
déjalo, es inofensivo; lo importante es que el click activa el set y la grilla de abajo se
actualiza. Mantener la grilla de objetos + import/export/reset debajo, sin cambios.

- [ ] **Step 5: Handler `personalize-icon-set` en main.rs**

Conectar el callback (junto a los otros `cfg_win.on_set_icon_*`):
```rust
{
    let ctrl = ctrl.clone();
    let refresh = refresh_config_vm.clone();
    let refresh_icons = refresh_toolbar_icons.clone();
    let refresh_drives = refresh_drives.clone();
    let cfg_weak = cfg_win.as_weak();
    cfg_win.on_personalize_icon_set(move |id| {
        {
            let mut c = ctrl.borrow_mut();
            c.config.set_icon_set(id.to_string());
            let active = c.config.settings.icon_set.clone();
            c.icons.set_active(active.clone());
            let tintable = naygo_core::icon_set::IconSetCatalog::load(&c.config.config_dir).is_tintable(&active);
            c.icons.set_overrides(c.config.settings.icon_overrides.clone());
            c.icons.set_tint(tintable, theme_text_rgb(&c.config.settings, &c.config.themes));
        }
        // saltar a la pestaña Íconos (cat 9)
        if let Some(cfg) = cfg_weak.upgrade() { cfg.set_cat(9); }
        refresh_icons();
        refresh_drives();
        refresh();
    });
}
```
> `cfg.set_cat(9)` usa el setter generado de la `property <int> cat`. Verificar que `cat` sea
> accesible (es `property <int> cat: 3;` en config-window.slint — si no es `in property`/`in-out`,
> cambiarla a `in-out property <int> cat: 3;` para poder setearla desde Rust).

- [ ] **Step 6: Compilar + verificación en vivo**

Run: `cargo build -p naygo-ui-slint --bins`, luego `cargo run -p naygo-ui-slint --bin naygo`
Verificar: Apariencia muestra la galería de tarjetas con muestras; click en una tarjeta cambia
el set (toolbar + paneles en caliente); botón Personalizar activa el set y salta a la pestaña
Íconos con ese set como base y su grilla.

- [ ] **Step 7: Commit**

```bash
git add crates/ui-slint/ui/config-window.slint crates/ui-slint/src/main.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs
git commit -m "feat(ui): galería de tarjetas de sets en Apariencia/Íconos + botón Personalizar (enlace + caliente)"
```

---

## Fase 6 — Cierre

### Task 10: Repaso final + verificación

- [ ] **Step 1: Suite + clippy + build**

Run: `cargo test -p naygo-core` (incluido el bin gen_icons con `--features gen-icons` para sus tests)
Run: `cargo test -p naygo-ui-slint`
Run: `cargo clippy -p naygo-core` y `cargo clippy -p naygo-ui-slint --bins`
Run: `cargo build -p naygo-ui-slint --bins`
Expected: todo verde, sin warnings.

- [ ] **Step 2: Verificación visual en VM (la hace Nicolás)**

Cambiar set en Apariencia (galería) → toolbar y paneles cambian en caliente; cambiar tema →
íconos del toolbar se retiñen; Personalizar → salta a Íconos preseleccionado; los 6 botones
nuevos (home/search/ojo/historial/estrella/dividir) muestran el ícono del set.

- [ ] **Step 3: Sin commit (si todo pasa)** — la rama queda lista para visto bueno + merge.

---

## Resumen de fases
1. **6 claves** nuevas en ActionIcon (core, TDD).
2. **Generar** las 6 en los 5 sets (build-time; correr ANTES de Fase 3).
3. **Embeber** 39 claves + i18n de los 6 objetos.
4. **Toolbar** usa el set (props image + refresh_toolbar_icons cableado).
5. **Galería** de tarjetas + enlace Apariencia↔Íconos (Personalizar).
6. **Cierre** (suite + clippy + VM).
