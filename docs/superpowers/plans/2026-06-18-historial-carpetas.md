# Ícono de historial de carpetas + límite configurable — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agregar a Naygo un ícono de historial en la toolbar que despliega un menú con las carpetas recientes (navega el panel activo al elegir), y hacer configurable (1–100, por defecto 50, en Avanzado) cuántas carpetas se recuerdan.

**Architecture:** Se reutiliza `core::recent_dirs::RecentDirs` (MRU global ya existente). El tope fijo `MAX_RECENTS=30` se vuelve un parámetro de `push` cuyo valor sale de un nuevo campo `Settings.recent_limit`. La UI añade un ícono+menú flotante en la toolbar (mismo patrón que el ▾ de USB) y una fila numérica en la sección Avanzado de la config.

**Tech Stack:** Rust workspace (naygo-core / naygo-platform / naygo-ui-slint), Slint 1.16 (render software). Build SIEMPRE con `CARGO_BUILD_JOBS=2`. Gate de cada tarea de código: `cargo fmt --all` + `cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform` + `cargo clippy --workspace --all-targets -- -D warnings`.

**Spec:** `docs/superpowers/specs/2026-06-18-historial-carpetas-design.md`

**Convenciones del repo (recordatorio):**
- Español neutral SIN voseo (código, comentarios, docs, UI). PowerShell 5.1: nunca `&&`/`||`.
- Header en archivos nuevos: `// Naygo — <descr>` + `// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.`
- i18n triple: `i18n.slint` (`in property <string> kebab-case`) + `es.json`/`en.json` (`"slint.xxx.yyy"`) + `i18n_keys.rs` (`tr.set_xxx(c.t("slint.xxx.yyy").into())`).
- Iconos con Slint `Path`, NUNCA glifos de fuente (render por software).
- NO commitear `CLAUDE.md`. `graphify-out/` y `assets/icons/otros/` ya están en .gitignore.
- NO `git push`. Commits locales por tarea. Mensajes terminan con `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Tras tocar código: `graphify update .`.

**Datos reales ya explorados:**
- `RecentDirs` (`crates/core/src/recent_dirs.rs`): `push(&mut self, dir: PathBuf)` con `const MAX_RECENTS: usize = 30`; `list() -> &[PathBuf]`, `remove_missing()`, `to_json`/`from_json`. Tiene tests (`push_es_mru_sin_duplicados`, `respeta_el_tope`, etc.).
- `WorkspaceCtrl` tiene `pub recents: RecentDirs` (campo, ~L96). Hay **11 call-sites** de `self.recents.push(...)` / `c.recents.push(...)` (líneas aprox. 366, 447, 1271, 1489, 1714, 2200, 3197, 3228, 3249, 3273, 3296).
- `Settings` (`crates/core/src/config/mod.rs:119`): campos con `#[serde(default = "default_x")]` + `fn default_x() -> T`. Ej.: `#[serde(default = "default_show_parent_entry")] pub show_parent_entry: bool`.
- `recent_rows(&self.recents, &folder)` (bridge) ya arma filas del panel Recientes; `navigate_active_to(path)` ya navega el panel activo.
- Patrón de menú flotante (veil + rectángulo anclado) ya existe en `app-window.slint` para el ▾ de USB (`drive-menu-*`) y el menú «agregar panel».

---

## File Structure

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `crates/core/src/recent_dirs.rs` | `push(dir, limit)`, `truncate_to(n)`, quitar `MAX_RECENTS` fijo; tests. | Modificar |
| `crates/core/src/config/mod.rs` | Campo `recent_limit: usize` + `default_recent_limit() -> 50`. | Modificar |
| `crates/ui-slint/src/workspace_ctrl.rs` | Helper `push_recent`; usarlo en los 11 call-sites; truncar al cambiar el límite; estado/lógica del menú de historial. | Modificar |
| `crates/ui-slint/ui/i18n.slint` | `history`, `history-empty`, `cfg-recent-limit`. | Modificar |
| `crates/core/src/i18n/es.json`, `en.json` | Traducciones. | Modificar |
| `crates/ui-slint/src/i18n_keys.rs` | Setters. | Modificar |
| `crates/ui-slint/ui/app-window.slint` | Ícono de historial + menú flotante en la toolbar. | Modificar |
| `crates/ui-slint/ui/config-window.slint` | Fila «Carpetas recientes a recordar» (1–100) en Avanzado. | Modificar |
| `crates/ui-slint/src/main.rs` | Cablear: abrir menú, modelo de filas del menú, navegar al elegir, control numérico de config. | Modificar |

**Orden:** core primero (Task 1-2, testeable), i18n (Task 3), config-limit wiring (Task 4), menú de historial en UI (Task 5).

---

## Task 1: `RecentDirs` con límite por parámetro + `truncate_to`

**Files:**
- Modify: `crates/core/src/recent_dirs.rs`

- [ ] **Step 1: Cambiar `push` para recibir el límite y agregar `truncate_to`**

En `crates/core/src/recent_dirs.rs`, reemplaza la constante y el método `push`, y agrega `truncate_to`. **Quita `const MAX_RECENTS` por completo** (ya no se usa; dejarla daría un warning de dead-code que rompe `clippy -D warnings`). El nuevo código:

```rust
// ... dentro de impl RecentDirs ...

/// Registra una visita: `dir` pasa al frente (sin duplicados). Recorta a `limit`
/// (clampeado a >= 1: un push siempre deja al menos esa carpeta).
pub fn push(&mut self, dir: PathBuf, limit: usize) {
    let limit = limit.max(1);
    self.dirs.retain(|d| d != &dir);
    self.dirs.insert(0, dir);
    self.dirs.truncate(limit);
}

/// Recorta la lista a los `n` más recientes (n=0 deja la lista vacía). Se usa cuando el
/// usuario BAJA el límite en la configuración.
pub fn truncate_to(&mut self, n: usize) {
    self.dirs.truncate(n);
}
```
(Deja `list`, `remove_missing`, `to_json`, `from_json`, `recents_path` como están. No introduzcas ninguna constante nueva sin uso.)

- [ ] **Step 2: Actualizar los tests existentes y agregar los nuevos**

En el `mod tests` de `recent_dirs.rs`:
- Los tests que llaman `r.push(p("..."))` ahora deben pasar un límite. Cambia `push_es_mru_sin_duplicados` a usar un límite holgado:
```rust
    #[test]
    fn push_es_mru_sin_duplicados() {
        let mut r = RecentDirs::new();
        r.push(p("D:/a"), 50);
        r.push(p("D:/b"), 50);
        r.push(p("D:/a"), 50); // re-visita: sube al frente, sin duplicar
        assert_eq!(r.list(), &[p("D:/a"), p("D:/b")]);
    }
```
- Reemplaza `respeta_el_tope` (que dependía de MAX_RECENTS) por uno con límite explícito:
```rust
    #[test]
    fn push_respeta_el_limite() {
        let mut r = RecentDirs::new();
        for i in 0..40 {
            r.push(p(&format!("D:/d{i}")), 10);
        }
        assert_eq!(r.list().len(), 10);
        assert_eq!(r.list()[0], p("D:/d39"));
    }

    #[test]
    fn push_limite_cero_se_clampa_a_uno() {
        let mut r = RecentDirs::new();
        r.push(p("D:/a"), 0);
        assert_eq!(r.list(), &[p("D:/a")]); // nunca queda vacío tras un push
    }

    #[test]
    fn truncate_to_recorta_a_los_mas_recientes() {
        let mut r = RecentDirs::new();
        for i in 0..5 {
            r.push(p(&format!("D:/d{i}")), 50);
        }
        r.truncate_to(2);
        assert_eq!(r.list(), &[p("D:/d4"), p("D:/d3")]);
    }
```
- El test `json_round_trip_y_carga_tolerante` y `remove_missing_filtra_inexistentes` también llaman `push`; actualízalos para pasar un límite (p. ej. `50`).

- [ ] **Step 3: Tests pasan**

```
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core recent_dirs
```
Esperado: todos PASS.

- [ ] **Step 4: Gate + commit** (NO compiles aún ui-slint: los call-sites se arreglan en Task 4; este commit es solo core y romperá temporalmente el build de ui-slint, lo cual es esperado dentro del flujo de tareas. Para no dejar el árbol roto entre tareas, NO corras `cargo test` del workspace aquí; valida solo core.)

```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt -p naygo-core
$env:CARGO_BUILD_JOBS = "2"; cargo clippy -p naygo-core --all-targets -- -D warnings
```

```bash
git add crates/core/src/recent_dirs.rs
git commit -F - <<'EOF'
feat(core): RecentDirs con limite por parametro + truncate_to

push(dir, limit) reemplaza el tope fijo MAX_RECENTS; el limite (clampeado a >=1)
lo decide quien llama (la preferencia del usuario). truncate_to(n) recorta al
bajar el limite. Tests actualizados.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 2: `Settings.recent_limit` (default 50)

**Files:**
- Modify: `crates/core/src/config/mod.rs`

- [ ] **Step 1: Agregar el campo a `Settings` con default serde**

En `crates/core/src/config/mod.rs`, dentro del struct `Settings`, agrega (junto a otros campos con `#[serde(default = ...)]`):
```rust
    /// Cuántas carpetas recientes recordar (1–100). `#[serde(default)]` por retro-compat
    /// (settings viejo sin el campo → 50). El uso real lo clampa a 1..=100.
    #[serde(default = "default_recent_limit")]
    pub recent_limit: usize,
```
Y la función de default (junto a las otras `fn default_*`):
```rust
fn default_recent_limit() -> usize {
    50
}
```
Si `Settings` tiene un `impl Default` o un constructor que inicializa todos los campos a mano, agrégale `recent_limit: 50,` (búscalo: si deriva `Default` vía serde no hace falta; si hay un `Default` manual o un `Settings { ... }` literal en algún sitio, ajústalo).

- [ ] **Step 2: Test del default**

Agrega a los tests de `config/mod.rs` (o donde estén los tests de Settings):
```rust
    #[test]
    fn recent_limit_default_es_50() {
        let s = Settings::default();
        assert_eq!(s.recent_limit, 50);
        // Y un JSON viejo sin el campo deserializa a 50.
        let viejo = r#"{"version":1}"#;
        let s2: Settings = serde_json::from_str(viejo).unwrap_or_default();
        assert_eq!(s2.recent_limit, 50);
    }
```
> Si `Settings::default()` no existe o el JSON mínimo `{"version":1}` no deserializa por campos
> requeridos, ajusta el test al patrón real de los demás tests de Settings (mira cómo testean
> otros defaults serde en ese archivo). Lo esencial: verificar que el default es 50 y que la
> ausencia del campo no rompe la carga.

- [ ] **Step 3: Compila core + test**

```
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core config
```
Esperado: PASS.

- [ ] **Step 4: Gate + commit**

```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt -p naygo-core
$env:CARGO_BUILD_JOBS = "2"; cargo clippy -p naygo-core --all-targets -- -D warnings
```
```bash
git add crates/core/src/config/mod.rs
git commit -F - <<'EOF'
feat(core): Settings.recent_limit (default 50) para el historial

Cuantas carpetas recientes recordar; serde-default 50 por retro-compat. El uso
lo clampa a 1..=100. Con test del default y de la carga tolerante.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 3: i18n (history, history-empty, cfg-recent-limit)

**Files:**
- Modify: `crates/ui-slint/ui/i18n.slint`, `crates/core/src/i18n/es.json`, `crates/core/src/i18n/en.json`, `crates/ui-slint/src/i18n_keys.rs`

- [ ] **Step 1: Defaults en `i18n.slint`**

Agrega (junto a otras claves de toolbar/config):
```slint
    in property <string> history: "Historial de carpetas";
    in property <string> history-empty: "Sin carpetas recientes";
    in property <string> cfg-recent-limit: "Carpetas recientes a recordar";
```

- [ ] **Step 2: `es.json`** (cuida comas JSON):
```json
  "slint.history.label": "Historial de carpetas",
  "slint.history.empty": "Sin carpetas recientes",
  "slint.cfg.recent_limit": "Carpetas recientes a recordar",
```

- [ ] **Step 3: `en.json`**:
```json
  "slint.history.label": "Folder history",
  "slint.history.empty": "No recent folders",
  "slint.cfg.recent_limit": "Recent folders to remember",
```

- [ ] **Step 4: Setters en `i18n_keys.rs`**:
```rust
    tr.set_history(c.t("slint.history.label").into());
    tr.set_history_empty(c.t("slint.history.empty").into());
    tr.set_cfg_recent_limit(c.t("slint.cfg.recent_limit").into());
```

- [ ] **Step 5: Compila** (Nota: ui-slint puede seguir roto por los call-sites de push hasta Task 4; este step solo valida que la i18n no introduce errores propios. Si el build falla SOLO por `recents.push` argumentos, es esperado; continúa. Si falla por una clave i18n mal escrita, corrígela.)
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
```

- [ ] **Step 6: Commit**
```bash
git add crates/ui-slint/ui/i18n.slint crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/src/i18n_keys.rs
git commit -F - <<'EOF'
feat(i18n): claves de historial de carpetas y limite de recientes

history, history-empty y cfg-recent-limit en las tres capas.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 4: Controlador — helper `push_recent`, los 11 call-sites, y el límite de la config

Esto restaura el build de ui-slint (arregla la firma de push) y conecta el límite de Settings.

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs`

- [ ] **Step 1: Helper privado `push_recent`**

En el `impl WorkspaceCtrl`, agrega:
```rust
    /// Registra una visita en el historial, respetando el límite configurado por el usuario
    /// (`Settings.recent_limit`, clampeado a 1..=100). Centraliza el límite para no repetirlo
    /// en cada call-site.
    fn push_recent(&mut self, dir: std::path::PathBuf) {
        let limit = self.config.settings.recent_limit.clamp(1, 100);
        self.recents.push(dir, limit);
    }
```
> Verifica la ruta real al settings: en el código es `self.config.settings.recent_limit` si
> `config` es el `ConfigCtrl` con un campo `settings`. Mira cómo otros métodos leen settings
> (p. ej. `self.config.settings.<algo>`); usa el acceso real.

- [ ] **Step 2: Reemplazar los 11 call-sites**

Cambia cada `self.recents.push(<dir>)` por `self.push_recent(<dir>)`, y el de `new_in`
(`c.recents.push(start.clone())`, ~L366) por `c.push_recent(start.clone())`. Los call-sites
(aprox.): L366, 447, 1271, 1489, 1714, 2200, 3197, 3228, 3249, 3273, 3296. Busca TODOS con
`rg -n "recents\.push\(" crates/ui-slint/src/workspace_ctrl.rs` para no dejar ninguno (la firma
vieja ya no compila, así que el compilador te marcará los que falten).
> OJO con `new_in`: `c` es la `WorkspaceCtrl` recién construida; `c.push_recent(...)` requiere
> que `push_recent` sea método de `&mut self` y que `c` sea mutable — ya lo es en ese contexto.
> Si `push_recent` no es accesible ahí por orden de inicialización (p. ej. settings aún no
> cargado), usa el acceso directo con el límite ya disponible; pero normalmente `config` ya
> está construido antes de ese push.

- [ ] **Step 3: Aplicar el límite al cambiarlo en config (truncar)**

Localiza dónde el controlador recibe/guarda los cambios de settings desde la ventana de config
(un método tipo `set_setting`, `apply_settings`, `on_setting_changed`, o donde se asigna
`self.config.settings.<campo> = ...` y se persiste). Cuando cambie `recent_limit`, tras
asignarlo, trunca la lista:
```rust
        // Si bajó el límite de recientes, recorta la lista al nuevo tope.
        let limit = self.config.settings.recent_limit.clamp(1, 100);
        self.recents.truncate_to(limit);
```
> Si los settings numéricos de la config se cablean por un método genérico, añade el caso de
> `recent_limit` ahí. Si cada setting tiene su setter, crea/usa el de `recent_limit`. Mira cómo
> se maneja, por ejemplo, el alto de fila u otro setting numérico existente, y replica.

- [ ] **Step 4: Compila el workspace (ya debe compilar)**

```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
```
Esperado: compila (la firma de push ya está alineada en todos los call-sites).

- [ ] **Step 5: Gate**

```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform
$env:CARGO_BUILD_JOBS = "2"; cargo clippy --workspace --all-targets -- -D warnings
```
Esperado: tests PASS, clippy sin warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(panel): historial respeta el limite configurado (push_recent)

Helper push_recent centraliza el limite (Settings.recent_limit, clamp 1..=100)
en los call-sites del historial; al bajar el limite en config se trunca la
lista. Restaura el build tras el cambio de firma de RecentDirs::push.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 5: Fila de config (Avanzado) + ícono y menú de historial en la toolbar

La parte de UI. Se apoya en patrones existentes (fila de config numérica; menú flotante del ▾ USB).

**Files:**
- Modify: `crates/ui-slint/ui/config-window.slint` (fila Avanzado)
- Modify: `crates/ui-slint/ui/app-window.slint` (ícono + menú flotante)
- Modify: `crates/ui-slint/src/main.rs` (wiring)

- [ ] **Step 1: Fila «Carpetas recientes a recordar» en Avanzado (config-window.slint)**

En la sección Avanzado de `config-window.slint`, agrega una fila con la etiqueta
`Tr.cfg-recent-limit` y un control numérico acotado a 1–100. Sigue el patrón de OTRA fila
numérica ya existente en la config (busca cómo se edita, p. ej., un valor entero de settings:
un SpinBox, un campo de texto validado, o +/−). Propiedad de entrada y callback:
```slint
    in property <int> recent-limit: 50;
    callback recent-limit-changed(int);
```
La fila debe llamar `recent-limit-changed(nuevo)` con el valor clampeado a 1–100. Si la config
usa un componente numérico reutilizable, úsalo; si no, un editor de texto que parsea int y
clampa. Mantén el estilo visual de las otras filas de Avanzado.

- [ ] **Step 2: Ícono de historial + menú flotante (app-window.slint)**

(a) En la toolbar global, junto a los otros grupos/íconos, agrega un botón con ícono de
historial dibujado con `Path` (un reloj: círculo + dos manecillas). Patrón:
```slint
    Rectangle {
        width: 28px; height: 24px; border-radius: 4px;
        background: hist-touch.has-hover ? Theme.selection-bg : Theme.row-bg;
        Path {
            width: 16px; height: 16px;
            x: (parent.width - self.width) / 2; y: (parent.height - self.height) / 2;
            stroke: Theme.text; stroke-width: 1.3px; fill: transparent;
            viewbox-width: 16; viewbox-height: 16;
            // Esfera del reloj (aprox. con líneas; un círculo se sugiere con un octágono).
            MoveTo { x: 8; y: 1.5; } LineTo { x: 12.5; y: 3.5; } LineTo { x: 14.5; y: 8; }
            LineTo { x: 12.5; y: 12.5; } LineTo { x: 8; y: 14.5; } LineTo { x: 3.5; y: 12.5; }
            LineTo { x: 1.5; y: 8; } LineTo { x: 3.5; y: 3.5; } Close { }
            // Manecillas.
            MoveTo { x: 8; y: 8; } LineTo { x: 8; y: 4.5; }
            MoveTo { x: 8; y: 8; } LineTo { x: 11; y: 9.5; }
        }
        hist-touch := TouchArea {
            mouse-cursor: pointer;
            clicked => { root.history-open(self.absolute-position.x + self.width / 2); }
            changed has-hover => { root.hovered-tip = self.has-hover ? Tr.history : ""; }
        }
    }
```
(b) Agrega a `AppWindow` las propiedades y callbacks (junto a los del `drive-menu`):
```slint
    callback history-open(length);          // abre el menú anclado en la x dada
    callback history-pick(string);          // (ruta) navegar a la carpeta elegida
    in property <[NavRow]> history-rows;     // filas del menú (reusa NavRow: nombre + ruta + icono)
    property <length> history-menu-x: 0;
    in-out property <bool> history-menu-open: false;
```
(c) Réplica del menú flotante (igual que `drive-menu`): un `if root.history-menu-open: TouchArea`
veil que al hacer clic cierra (`root.history-menu-open = false;`), y un
`if root.history-menu-open: Rectangle { ... }` anclado en `history-menu-x`, con un
`VerticalLayout` que itera `for r in root.history-rows`: cada uno un MenuItem que al click hace
`root.history-pick(r.path); root.history-menu-open = false;`. Si `history-rows` está vacío,
muestra un `Text { text: Tr.history-empty; ... }`. Reusa el componente `MenuItem` existente y
el tipo `NavRow` (mira cómo el panel Recientes arma sus filas con `NavRow`).
> Para `history-open`: el handler del callback en main.rs setea `history-menu-x`, llena
> `history-rows` y pone `history-menu-open = true`. (La propiedad `history-menu-x` es privada de
> AppWindow; el callback la setea desde Rust vía un setter generado, o se setea en el .slint
> dentro del propio handler `history-open(x) => { self.history-menu-x = x; ... }` y Rust solo
> llena las filas + abre. Elige UNA vía coherente con cómo está hecho `drive-menu` y replícala.)

- [ ] **Step 3: Wiring en main.rs**

(a) Cablea `on_history_open`: arma las filas recientes (tras `remove_missing`) y abre el menú.
Reusa el helper que ya produce filas de recientes (`recent_rows`/`to_nav_row`). Patrón:
```rust
        {
            let ctrl = ctrl.clone();
            let ui_weak = ui.as_weak();
            ui.on_history_open(move |x| {
                let rows: Vec<NavRow> = {
                    let mut c = ctrl.borrow_mut();
                    c.recents.remove_missing();
                    c.recent_history_rows() // helper: NavRow por cada reciente (nombre+ruta+icono)
                };
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_history_rows(ModelRc::new(VecModel::from(rows)));
                    ui.set_history_menu_x(x);
                    ui.set_history_menu_open(true);
                }
            });
        }
```
> `recent_history_rows` es el nombre placeholder de un método del controlador que devuelve las
> recientes como filas listas para `NavRow`. Si ya existe `recent_rows(&self.recents, &folder)`
> que devuelve algo convertible con `to_nav_row`, reúsalo (ajusta args). NO dupliques el armado
> de NavRow: usa el camino existente del panel Recientes.

(b) Cablea `on_history_pick`: navega el panel activo a la ruta elegida.
```rust
        {
            let ctrl = ctrl.clone();
            let sync_rows = sync_rows.clone();
            ui.on_history_pick(move |path| {
                ctrl.borrow_mut().navigate_active_to(std::path::PathBuf::from(path.as_str()));
                sync_rows();
            });
        }
```
> Usa el nombre real del método de navegación del panel activo (`navigate_active_to` o el que
> sea). Si requiere otra firma (p. ej. PaneId), adáptalo: el objetivo es que el panel activo vaya
> a esa carpeta. Tras navegar, `sync_rows()` para refrescar.

(c) Cablea el control numérico de la config: `on_recent_limit_changed` asigna el setting
(clamp 1–100), persiste y trunca (el truncado ya lo hace el controlador en Task 4 Step 3 si
enrutas el cambio por ahí; si el control de config llama un callback propio, conéctalo al método
del controlador que asigna `recent_limit` y trunca). Y al abrir la config, setear
`config.set_recent_limit(ctrl.borrow().config.settings.recent_limit as i32)` para reflejar el
valor actual.

- [ ] **Step 4: Compila + gate completo**

```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform
$env:CARGO_BUILD_JOBS = "2"; cargo clippy --workspace --all-targets -- -D warnings
```
Esperado: compila, tests PASS, clippy sin warnings. Diagnostica y corrige hasta verde.

> RECORDATORIO CRÍTICO (gotcha Slint, ya nos pasó): un callback/propiedad nuevo en un componente
> hijo NO existe en `AppWindow` hasta propagarlo. Aquí el ícono y el menú van DIRECTO en
> `app-window.slint` (no en un hijo), así que los callbacks `history-open`/`history-pick` y las
> props se declaran en `AppWindow` y main.rs los usa como `ui.on_history_open` / `ui.set_history_rows`.
> Si pusieras algo en `FilePanel` o `config-window`, recuerda propagarlo. La fila de config va en
> `config-window.slint`: su `recent-limit`/`recent-limit-changed` deben exponerse en el componente
> de la ventana de config tal como las otras filas de Avanzado (mira una existente y replica el
> binding hacia Rust).

- [ ] **Step 5: Verificación visual (Nicolás)**

Compilar release y verificar: el ícono de historial aparece en la toolbar; al pulsarlo se abre el
menú con las carpetas recientes (o «Sin carpetas recientes» si está vacío); elegir una navega el
panel activo; en Configuración → Avanzado, la fila «Carpetas recientes a recordar» edita 1–100 y
al bajarla se recorta el historial. Visto bueno visual de Nicolás.
```
$env:CARGO_BUILD_JOBS = "2"; cargo build --release -p naygo-ui-slint
```

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/ui/config-window.slint crates/ui-slint/ui/app-window.slint crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(toolbar): icono de historial de carpetas + limite en config

Icono (reloj, Path) en la toolbar que abre un menu flotante con las carpetas
recientes; al elegir una navega el panel activo. Fila "Carpetas recientes a
recordar" (1-100) en Avanzado. Reusa RecentDirs, NavRow y el patron de menu
flotante del menu de discos.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Verificación final del bloque

- [ ] Gate completo verde:
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform; cargo clippy --workspace --all-targets -- -D warnings
```
- [ ] Sin voseo en lo nuevo: `rg -ni "\b(arrastrá|escribí|elegí|podés|tenés|querés|hacé|volvé)\b"` en los archivos tocados → vacío.
- [ ] `graphify update .`.
- [ ] Documentar en `docs/GUIA-DE-USUARIO.md` (el ícono de historial y el ajuste de Avanzado) y una línea en «Sin publicar» del `CHANGELOG.md`.
- [ ] Visto bueno visual de Nicolás (Task 5 Step 5).

## Notas de cierre

- Actualizar memoria de proyecto al cerrar.
- Siguiente pendiente del backlog: argumentos de CLI para `naygo.exe` (abrir directorio / plantilla visual) — su propio ciclo brainstorm→spec→plan.
```
