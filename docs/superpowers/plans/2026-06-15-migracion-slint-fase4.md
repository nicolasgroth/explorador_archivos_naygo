# Fase 4 (Slint): configuración + atajos + temas/i18n + persistencia — Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Llevar la configuración completa, el editor de atajos, los temas + i18n (con packs
de usuario e import/export `.zip`) y la persistencia del layout de paneles, de la capa egui a
la capa Slint (`crates/ui-slint`).

**Architecture:** Todo el modelo vive en `naygo-core` (`config`, `keymap`, `i18n`, `theme`,
`WorkspacePersist`) y se reusa. Se escribe la orquestación en un módulo nuevo `config_ctrl.rs`
y la UI en Slint (ventana de config, editor de atajos), con dos globals Slint nuevos: `Tr`
(i18n) y `Theme` (colores). La persistencia del layout reusa `config::save_workspace`/
`load_workspace`.

**Tech Stack:** Rust, Slint 1.16 (winit software), `naygo-core::{config,keymap,i18n,theme,
workspace}`, deps nuevas `zip` y `rfd` (selector de archivo).

**Convenciones del proyecto (OBLIGATORIAS):**
- Antes de leer/grepear código: `graphify query "<pregunta>"` para orientarse (graphify-out/
  existe). Solo leer crudo tras orientarse o para líneas puntuales.
- Gates antes de CADA commit: `cargo test --workspace` + `cargo clippy --workspace
  --all-targets -- -D warnings` + `cargo fmt --all -- --check`.
- Commits en español (heredoc), terminando con
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Stagear rutas EXPLÍCITAS (NUNCA `git add -A`): `CLAUDE.md` y `graphify-out/` NO se commitean.
- `graphify update .` tras cambios de código.
- Header en archivos nuevos: `// Naygo — <desc>.` / `// Copyright (c) 2026 Nicolás Groth /
  ISGroth. MIT License.`
- Probar la GUI en ESTA máquina con input REAL de computer-use (relanzar fresco la hace
  visible) o Win32 (PostMessage clic / PrintWindow). PostMessage WM_KEYDOWN sintético a winit
  NO sirve. Nicolás solo mide rendimiento en la VM.
- Trabajamos en `main` (sin rama aparte, como en F2/F3). NO se mergea/pushea sin visto bueno;
  el push lo autoriza Nicolás al cierre de cada bloque.

**Tipos de core que se reusan (firmas exactas, ya verificadas):**
- `config::{load_settings_flagged(dir)->(Settings,bool), save_settings(dir,&Settings),
  load_workspace_flagged(dir)->(Option<WorkspacePersist>,bool), save_workspace(dir,&WP),
  load_keymap(dir)->KeyMap, save_keymap(dir,&KeyMap), portable_dir()->PathBuf}`.
- `WorkspacePersist { version:u32, layout:SerializableDockLayout, active:Option<PaneId>,
  files:Vec<(PaneId,FilePanePersist)>, purposes:Vec<(PaneId,PanePurpose)> }`.
- `FilePaneState::to_persist()->FilePanePersist`, `FilePaneState::from_persist(FilePanePersist)
  ->FilePaneState`.
- `i18n::I18n::load(dir,&LangId)->I18n`, `.t(key)->&str`, `.set_language(&LangId)->bool`,
  `.active_lang()->LangId`, `.available()->&[LangId]`. `LangId::new(code)`.
- `theme::{ThemeCatalog::load(dir,&ThemeId)->ThemeCatalog, .get(&ThemeId)->&Theme,
  .available()->&[ThemeId], ThemeCatalog::default_id()->ThemeId}`. `Theme` tokens: `accent,
  panel_bg, row_bg, row_alt_bg, text, text_dim, selection_bg, active_bar, error, highlight,
  border` (cada uno `ThemeColor` con `.to_hex()`). `ThemeId::new`, `ThemeColor::from_hex`.
- `keymap::{Action (enum), Chord, KeyCode, KeyMap (action_for, defaults)}`.

**NOTA de alineación con el spec:** los temas en core son archivos PLANOS
`<config_dir>/themes/<id>.json` (no carpetas). El export/import de un tema empaqueta ese único
JSON. Los idiomas son `<config_dir>/lang/<code>.json`. (El spec mencionaba carpetas de tema con
íconos; los íconos por tema quedan fuera de F4 — sólo colores. Se documenta.)

---

## Fase A — Persistencia del layout (lo que pidió Nicolás)

### Task 1: ConfigCtrl mínimo (Settings + persistencia) + arrancar

**Files:**
- Create: `crates/ui-slint/src/config_ctrl.rs`
- Modify: `crates/ui-slint/src/main.rs` (`mod config_ctrl;`)
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (poseer `ConfigCtrl`; el keymap viene de él)

- [ ] **Step 1: Orientar**

Run: `graphify query "WorkspaceCtrl new keymap config_dir portable_dir Settings"`.

- [ ] **Step 2: Crear `config_ctrl.rs` con `ConfigCtrl`**

```rust
// Naygo — controlador de configuración de la UI Slint (Fase 4).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
use naygo_core::config::{self, Settings};
use naygo_core::i18n::{I18n, LangId};
use naygo_core::keymap::KeyMap;
use naygo_core::theme::{Theme, ThemeCatalog, ThemeId};
use std::path::PathBuf;

pub struct ConfigCtrl {
    pub settings: Settings,
    pub i18n: I18n,
    pub themes: ThemeCatalog,
    pub keymap: KeyMap,
    pub config_dir: PathBuf,
}

impl ConfigCtrl {
    pub fn new(config_dir: PathBuf) -> ConfigCtrl {
        let (settings, _recovered) = config::load_settings_flagged(&config_dir);
        let i18n = I18n::load(&config_dir, &settings.language);
        let themes = ThemeCatalog::load(&config_dir, &settings.theme);
        let keymap = config::load_keymap(&config_dir);
        ConfigCtrl { settings, i18n, themes, keymap, config_dir }
    }

    /// Texto traducido (atajo a i18n.t).
    pub fn t(&self, key: &str) -> String {
        self.i18n.t(key).to_string()
    }

    /// El tema activo (resuelto del catálogo por el id de settings).
    pub fn active_theme(&self) -> &Theme {
        self.themes.get(&self.settings.theme)
    }

    /// Persiste los settings actuales (escritura de JSON chica).
    pub fn save(&self) {
        config::save_settings(&self.config_dir, &self.settings);
    }

    /// Cambia el idioma activo y persiste. Devuelve true si cambió.
    pub fn set_language(&mut self, lang: LangId) -> bool {
        let changed = self.i18n.set_language(&lang);
        if changed {
            self.settings.language = lang;
            self.save();
        }
        changed
    }

    /// Cambia el tema activo y persiste.
    pub fn set_theme(&mut self, id: ThemeId) {
        self.settings.theme = id;
        self.save();
    }
}
```

- [ ] **Step 3: Registrar el módulo y poseerlo en WorkspaceCtrl**

`main.rs`: agregar `mod config_ctrl;`. `WorkspaceCtrl`: agregar campo `pub config:
config_ctrl::ConfigCtrl`, inicializarlo en `new` con `naygo_core::config::portable_dir()`, y
hacer que `keymap` se tome de `config.keymap` (reemplazar `keymap: KeyMap::defaults()` por usar
`self.config.keymap`; donde `on_key` use `self.keymap`, cambiar a `self.config.keymap`). Si
`WorkspaceCtrl` ya tiene `keymap`, quitarlo y redirigir.

- [ ] **Step 4: Test de ConfigCtrl**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carga_defaults_en_dir_vacio() {
        let tmp = tempfile::tempdir().unwrap();
        let c = ConfigCtrl::new(tmp.path().to_path_buf());
        assert!(!c.t("app.title").is_empty()); // t nunca vacío (cae a la clave)
        assert!(c.themes.available().len() >= 4); // 4 temas embebidos
    }
}
```

- [ ] **Step 5: Gate + commit**

```bash
cargo test -p naygo-ui-slint && cargo clippy -p naygo-ui-slint --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/ui-slint/src/config_ctrl.rs crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(slint): ConfigCtrl — settings/i18n/temas/keymap desde core (Fase 4)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Task 2: Guardar y restaurar la sesión (layout de paneles)

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (`session_persist`, `load_session`)
- Modify: `crates/ui-slint/src/main.rs` (load_session al arrancar; save_session al cerrar)
- (Posible) Modify: `crates/core/src/workspace/mod.rs` (helper `from_persist` si conviene
  centralizar la reconstrucción; ver Step 3)

- [ ] **Step 1: Orientar**

Run: `graphify query "rebuild_workspace remap_layout WorkspacePersist reconstruct from persist"`.
Leer en `crates/ui/src/app.rs` las fns `rebuild_workspace` (~L4482) y `remap_layout` para
replicar la lógica.

- [ ] **Step 2: `session_persist()` en WorkspaceCtrl**

```rust
/// Arma el estado persistible del workspace (para guardar al cerrar).
pub fn session_persist(&self) -> naygo_core::config::WorkspacePersist {
    use naygo_core::config::WorkspacePersist;
    let files = self
        .ws
        .panes()
        .iter()
        .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.to_persist())))
        .collect();
    let purposes = self.ws.panes().iter().map(|p| (p.id, p.purpose)).collect();
    WorkspacePersist {
        version: 1,
        layout: self.ws.layout.clone(),
        active: self.ws.active_id(),
        files,
        purposes,
    }
}

/// Guarda la sesión en disco (al cerrar).
pub fn save_session(&self) {
    naygo_core::config::save_workspace(&self.config.config_dir, &self.session_persist());
}
```

- [ ] **Step 3: Reconstrucción `Workspace::from_persist` en core (centralizada)**

En `crates/core/src/workspace/mod.rs`, agregar (porta la lógica de egui `rebuild_workspace`):

```rust
/// Reconstruye un Workspace desde un WorkspacePersist. Crea cada panel con su purpose y, si
/// es Files, su FilePaneState restaurado; rearma el layout (los ids del persist se conservan)
/// y fija el activo. Si el layout queda vacío o sin paneles válidos, devuelve None (el
/// llamador cae al arranque default).
pub fn from_persist(p: &crate::config::WorkspacePersist) -> Option<Workspace> {
    if p.layout.pane_ids().is_empty() {
        return None;
    }
    let mut w = Workspace::new();
    let files: std::collections::HashMap<_, _> = p.files.iter().cloned().collect();
    let mut max_id = 0u64;
    for (id, purpose) in &p.purposes {
        let node = PaneNode {
            id: *id,
            purpose: *purpose,
            files: files.get(id).cloned().map(FilePaneState::from_persist),
        };
        w.push_node(node); // ver Step 4
        max_id = max_id.max(id.0);
    }
    w.set_next_id(max_id + 1); // ver Step 4
    w.layout = p.layout.clone();
    if let Some(a) = p.active {
        w.set_active(a);
    }
    Some(w)
}
```

- [ ] **Step 4: Helpers `push_node`/`set_next_id` en Workspace**

`Workspace` tiene campos privados `panes`, `next_id`. Agregar:

```rust
/// Inserta un PaneNode ya construido (para reconstruir desde persist). Si es el primero,
/// queda activo.
pub fn push_node(&mut self, node: PaneNode) {
    let id = node.id;
    self.panes.push(node);
    if self.active.is_none() {
        self.active = Some(id);
    }
}
/// Fija el contador de ids (tras reconstruir, para que add_pane no colisione).
pub fn set_next_id(&mut self, next: u64) {
    self.next_id = next;
}
```

- [ ] **Step 5: Test de round-trip en core**

```rust
#[test]
fn workspace_round_trip_persist() {
    use crate::workspace::{PanePurpose, Workspace};
    let mut w = Workspace::new();
    let a = w.add_pane(PanePurpose::Files, std::path::PathBuf::from("C:/a"));
    let _t = w.add_pane(PanePurpose::Tree, std::path::PathBuf::new());
    w.set_active(a);
    // Persistir vía config::WorkspacePersist construido a mano desde w:
    let persist = crate::config::WorkspacePersist {
        version: 1,
        layout: w.layout.clone(),
        active: w.active_id(),
        files: w.panes().iter().filter_map(|p| p.files.as_ref().map(|f| (p.id, f.to_persist()))).collect(),
        purposes: w.panes().iter().map(|p| (p.id, p.purpose)).collect(),
    };
    let w2 = Workspace::from_persist(&persist).unwrap();
    assert_eq!(w2.panes().len(), 2);
    assert_eq!(w2.active_id(), Some(a));
    assert_eq!(w2.active_files().map(|f| f.current_dir.clone()), Some(std::path::PathBuf::from("C:/a")));
}
```

Run: `cargo test -p naygo-core workspace_round_trip_persist`. Esperado: PASS.

- [ ] **Step 6: `load_session` en WorkspaceCtrl + arrancar con ella**

En `WorkspaceCtrl`, factorizar el arranque: si `config::load_workspace_flagged(dir)` da un
persist, `Workspace::from_persist` → usarlo (y arrancar listados de cada Files + DirTree de
cada Tree); si no, el arranque default actual (un Files en HOME). Hacerlo en `new` o en un
`load_session(&mut self)` llamado tras construir. Arrancar el listado de cada panel Files
restaurado (`start_listing(id, dir)`) y `build_tree` para los Tree.

- [ ] **Step 7: save_session al cerrar la ventana**

En `main.rs`, conectar el cierre: `ui.window().on_close_requested(move || { ctrl.borrow().save_session(); slint::CloseRequestResponse::HideWindow })`
(o `KeepWindowShown`+salir según el patrón; usar el que cierre de verdad). Verificar que
`save_session` corre antes de salir.

- [ ] **Step 8: Gate + commit**

```bash
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
git add crates/core/src/workspace/mod.rs crates/ui-slint/src/workspace_ctrl.rs crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(slint): persistir el layout de paneles — recordar la sesión al cerrar/abrir

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

- [ ] **Step 9: Verificación viva**

Build release, lanzar, abrir 2-3 paneles en carpetas distintas, cerrar, reabrir → deben
restaurarse. Capturar con PrintWindow/computer-use.

---

## Fase B — i18n: externalizar textos (global `Tr`)

### Task 3: Infra de i18n en Slint (global `Tr` + módulo de claves)

**Files:**
- Create: `crates/ui-slint/ui/i18n.slint` (global `Tr` con una propiedad por clave)
- Create: `crates/ui-slint/src/i18n_keys.rs` (lista central de (clave Slint, clave i18n))
- Modify: `crates/ui-slint/src/main.rs` (setear las propiedades de `Tr` desde `ConfigCtrl`)
- Modify: `crates/core/src/i18n/es.json` y `en.json` (agregar claves que falten)

- [ ] **Step 1: Definir el set inicial de claves**

Empezar por la toolbar y los modales (los textos más visibles). Crear `i18n.slint`:

```slint
// Naygo — textos de la UI (i18n). Las propiedades las setea Rust según el idioma activo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
export global Tr {
    in property <string> toolbar-up: "Subir";
    in property <string> toolbar-add: "+";
    in property <string> toolbar-panel: "Panel";
    in property <string> toolbar-swap: "Swap";
    in property <string> toolbar-clone: "Clonar";
    in property <string> toolbar-tabs: "Tabs";
    in property <string> toolbar-config: "Config";
    in property <string> dlg-delete-trash: "¿Enviar a la papelera?";
    in property <string> dlg-cancel: "Cancelar";
    in property <string> dlg-delete: "Eliminar";
    // … (se irá ampliando por archivo)
}
```

(El default de cada propiedad es el texto ES, así que si Rust no setea nada, la UI sigue en ES.)

- [ ] **Step 2: Módulo `i18n_keys.rs` con el mapeo**

```rust
// Naygo — claves i18n: (setter del global Tr, clave del catálogo). Una sola fuente para no
// desincronizar Tr.slint con los catálogos. Copyright (c) 2026 Nicolás Groth / ISGroth. MIT.
/// Aplica todos los textos del idioma activo al global Tr.
pub fn apply(tr: &crate::Tr, c: &crate::config_ctrl::ConfigCtrl) {
    tr.set_toolbar_up(c.t("toolbar.up").into());
    tr.set_toolbar_add(c.t("toolbar.add").into());
    tr.set_toolbar_panel(c.t("toolbar.panel").into());
    tr.set_toolbar_swap(c.t("toolbar.swap").into());
    tr.set_toolbar_clone(c.t("toolbar.clone").into());
    tr.set_toolbar_tabs(c.t("toolbar.tabs").into());
    tr.set_toolbar_config(c.t("toolbar.config").into());
    tr.set_dlg_delete_trash(c.t("dialog.delete.trash").into());
    tr.set_dlg_cancel(c.t("dialog.cancel").into());
    tr.set_dlg_delete(c.t("dialog.delete").into());
    // … (crece junto con i18n.slint)
}
```

- [ ] **Step 3: Agregar las claves a es.json/en.json**

En `crates/core/src/i18n/es.json` y `en.json`, agregar las claves usadas (toolbar.*,
dialog.*). ES con el texto español, EN con el inglés. (Reusar claves existentes si ya están.)

- [ ] **Step 4: Llamar `i18n_keys::apply` al arrancar y al cambiar idioma**

En `main.rs`, tras crear la UI: `i18n_keys::apply(&ui.global::<Tr>(), &ctrl.borrow().config);`.
Y en el setter de idioma (Task 6) volver a llamarlo.

- [ ] **Step 5: Importar `Tr` y usarlo en la toolbar**

En `app-window.slint`, `import { Tr } from "i18n.slint";` y reemplazar los textos de la
toolbar (`"Subir"`, etc.) por `Tr.toolbar-up`, etc.

- [ ] **Step 6: Gate + verificación visual + commit**

Build, lanzar, confirmar que la toolbar se ve igual (textos desde Tr). Gate verde.

```bash
git add crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/ui-slint/src/main.rs crates/ui-slint/ui/app-window.slint crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -F - <<'EOF'
feat(slint): infraestructura i18n (global Tr) + toolbar externalizada a claves

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 4: Externalizar el resto de los textos (.slint) a `Tr`

**Files:** todos los `.slint` con texto literal: `op-dialogs.slint`, `context-menu.slint`,
`ops-panel.slint`, `tree-panel.slint`, `inspector-panel.slint`, `history-panel.slint`,
`favorites-panel.slint`, `preview-panel.slint`, `file-panel.slint`, `app-window.slint`.

- [ ] **Step 1..N (uno por archivo):** Para cada `.slint`, por cada texto literal: agregar una
  propiedad a `Tr` (i18n.slint), su línea en `i18n_keys::apply`, las claves en es.json/en.json,
  e importar `Tr` + reemplazar el literal por `Tr.<clave>`. Build + gate verde tras cada
  archivo. Commit por archivo o por grupo coherente:

```bash
git commit -F - <<'EOF'
feat(slint): externalizar textos de <archivo> a claves i18n

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

- [ ] **Step final: Verificación** de que toda la UI sigue viéndose en ES (defaults) y el gate
  pasa. (El cambio de idioma en caliente se prueba en Task 6.)

---

## Fase C — Temas (global `Theme`)

### Task 5: Infra de temas en Slint (global `Theme` + aplicar colores)

**Files:**
- Create: `crates/ui-slint/ui/theme.slint` (global `Theme` con propiedades de color)
- Create: `crates/ui-slint/src/theme_apply.rs` (setear `Theme` desde un `naygo_core::theme::Theme`)
- Modify: `crates/ui-slint/src/main.rs` (aplicar el tema activo al arrancar)
- Modify: los `.slint` que usan hex hardcodeado → `Theme.<color>` (por archivo)

- [ ] **Step 1: `theme.slint` global**

```slint
// Naygo — colores del tema activo. Los setea Rust desde el tema activo.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
export global Theme {
    in property <color> accent: #4f8ae0;
    in property <color> panel-bg: #0e1622;
    in property <color> row-bg: #0e1622;
    in property <color> row-alt-bg: #111c2b;
    in property <color> text: #dddee6;
    in property <color> text-dim: #99aabb;
    in property <color> selection-bg: #2a4a7a;
    in property <color> active-bar: #4f8ae0;
    in property <color> error: #d4574e;
    in property <color> highlight: #ffd479;
    in property <color> border: #2a3a52;
}
```

- [ ] **Step 2: `theme_apply.rs`**

```rust
// Naygo — aplica un tema de core al global Theme de Slint. Copyright (c) 2026 N. Groth. MIT.
use slint::Color;
fn col(c: naygo_core::theme::ThemeColor) -> Color { Color::from_rgb_u8(c.r, c.g, c.b) }
/// Vuelca los colores del tema activo al global Theme.
pub fn apply(theme: &crate::Theme, t: &naygo_core::theme::Theme) {
    theme.set_accent(col(t.accent));
    theme.set_panel_bg(col(t.panel_bg));
    theme.set_row_bg(col(t.row_bg));
    theme.set_row_alt_bg(col(t.row_alt_bg));
    theme.set_text(col(t.text));
    theme.set_text_dim(col(t.text_dim));
    theme.set_selection_bg(col(t.selection_bg));
    theme.set_active_bar(col(t.active_bar));
    theme.set_error(col(t.error));
    theme.set_highlight(col(t.highlight));
    theme.set_border(col(t.border));
}
```

- [ ] **Step 3: Aplicar al arrancar**

En `main.rs`: `theme_apply::apply(&ui.global::<Theme>(), ctrl.borrow().config.active_theme());`.

- [ ] **Step 4..N (por archivo):** Reemplazar los hex hardcodeados de cada `.slint` por
  `Theme.<color>` (mapear cada hex al token más cercano: fondos→panel-bg/row-bg,
  selección→selection-bg, acento/bordes activos→accent/active-bar, texto→text/text-dim,
  peligro→error, etc.). Build + verificación visual (debe verse igual con el tema default) +
  gate. Commit por archivo/grupo.

- [ ] **Step final: commit**

```bash
git commit -F - <<'EOF'
feat(slint): temas — global Theme + colores del tema activo aplicados a la UI

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Fase D — Ventana de configuración + selectores tema/idioma + editor de atajos

### Task 6: Ventana de configuración (general/ops/pegado/apariencia)

**Files:**
- Create: `crates/ui-slint/ui/config-window.slint`
- Modify: `crates/ui-slint/ui/types.slint` (`SettingsVm`)
- Modify: `crates/ui-slint/ui/app-window.slint` (botón "Config" + overlay + callbacks)
- Modify: `crates/ui-slint/src/config_ctrl.rs` (`settings_vm()`, setters por campo)
- Modify: `crates/ui-slint/src/main.rs` (poblar VM + wirear callbacks)

- [ ] **Step 1: `SettingsVm` en types.slint** con los campos editables (bar-position:int,
  icon-only:bool, show-parent:bool, ops-mode:int, confirm-trash:bool, show-op-summary:bool,
  size-no-subdirs:bool, paste-text-name:string, paste-text-ext:string, language:string,
  theme:string, + listas de idiomas/temas disponibles como `[string]`).

- [ ] **Step 2: `config-window.slint`** — overlay con velo + barra lateral de categorías
  (General / Operaciones / Pegado / Apariencia / Atajos / Import-Export) + panel de contenido.
  Cada control (toggle/combo/text) emite un callback con el nuevo valor. Categoría Apariencia:
  combos de tema e idioma poblados desde las listas del VM.

- [ ] **Step 3: setters en ConfigCtrl** — `set_ops_mode(i32)`, `set_confirm_trash(bool)`,
  `set_show_op_summary(bool)`, `set_show_parent(bool)`, `set_icon_only(bool)`,
  `set_bar_position(i32)`, `set_paste_text_name(String)`, etc. Cada uno actualiza `settings`,
  llama `save()`. `settings_vm()` arma el `SettingsVm` (incluye `i18n.available()` y
  `themes.available()` como listas de strings).

- [ ] **Step 4: cambiar idioma/tema desde la ventana** — el combo de idioma llama
  `config.set_language(LangId::new(code))` + `i18n_keys::apply(&Tr,...)`; el de tema llama
  `config.set_theme(ThemeId::new(id))` + `theme_apply::apply(&Theme, config.active_theme())`.
  Verificar EN VIVO que la UI se traduce y recolorea en caliente.

- [ ] **Step 5: botón "Config" en la toolbar** que abre la ventana; aplicar las opciones que
  tienen efecto inmediato (ops_mode → `ops.ops_mode`, etc.).

- [ ] **Step 6: tests** (config_ctrl): cada setter persiste; `set_language`/`set_theme`
  cambian el activo. Gate + verificación visual + commit.

```bash
git commit -F - <<'EOF'
feat(slint): ventana de configuración + cambio de tema e idioma en caliente

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

### Task 7: Editor de atajos

**Files:**
- Modify: `crates/ui-slint/ui/config-window.slint` (categoría Atajos: lista + capturar/reset)
- Modify: `crates/ui-slint/ui/types.slint` (`ShortcutRowVm { action_key, label, chord_text,
  conflict }`)
- Modify: `crates/ui-slint/src/config_ctrl.rs` (listar acciones+chords, rebind, conflicto, reset)
- Modify: `crates/ui-slint/src/main.rs` (wirear captura de tecla + callbacks)

- [ ] **Step 1: `shortcut_rows()` en ConfigCtrl** — por cada `Action::all()` (o la lista de
  acciones), su etiqueta i18n y el chord actual del keymap como texto (`chord_text`). Helper
  `chord_to_text(&Chord) -> String` (ej "Ctrl+C", "Supr").

- [ ] **Step 2: rebind + conflicto + reset** — `rebind(action_key, chord)`: si el chord ya
  está en otra acción → marcar conflicto (devolver la acción en conflicto); si se confirma,
  quitarlo de la otra y asignarlo; `save_keymap`. `reset_shortcut(action_key)`: vuelve al
  default. Tests: rebind sin conflicto; rebind con conflicto detectado; reset.

- [ ] **Step 3: UI de captura** — fila con "Cambiar" → un `FocusScope` captura la próxima
  combinación (`keys::chord_from`) y la propone; muestra conflicto si lo hay; "Reset" por fila.

- [ ] **Step 4: Gate + verificación viva** (reasignar un atajo y usarlo) + commit.

```bash
git commit -F - <<'EOF'
feat(slint): editor de atajos — reasignar, detectar conflictos, reset a default

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Fase E — Import/Export de packs (.zip)

### Task 8: Export/Import de idiomas, temas y config

**Files:**
- Modify: `crates/ui-slint/Cargo.toml` (deps `zip` y `rfd`)
- Create: `crates/ui-slint/src/packs.rs` (empaquetar/desempaquetar `.zip`, validar)
- Modify: `crates/ui-slint/ui/config-window.slint` (categoría Import/Export: botones)
- Modify: `crates/ui-slint/src/main.rs` (wirear botones → rfd file dialog → packs)

- [ ] **Step 1: deps** — agregar a `crates/ui-slint/Cargo.toml`: `zip = "2"` (o la última),
  `rfd = "0.15"` (default features). Confirmar licencias MIT/Apache. Build.

- [ ] **Step 2: `packs.rs`** — funciones puras-ish (testeables con rutas temporales):
  - `export_lang(config_dir, code, out_zip) -> Result<(), String>` (mete `lang/<code>.json`).
  - `export_theme(config_dir, id, out_zip)` (mete `themes/<id>.json`).
  - `export_config(config_dir, out_zip)` (mete `settings.json` + `keymap.json`).
  - `import_zip(config_dir, in_zip) -> Result<ImportKind, String>` (detecta el tipo por el
    contenido: un `<code>.json` con claves → idioma a `lang/`; un `<id>.json` con colores →
    tema a `themes/`; `settings.json`/`keymap.json` → config; descomprime a la carpeta
    correcta; valida estructura). `ImportKind { Lang(code) | Theme(id) | Config }`.

- [ ] **Step 3: tests de packs** — export a un zip temporal y re-import a otro config_dir
  reproduce el archivo; un zip con estructura inválida da `Err`.

- [ ] **Step 4: UI + wiring** — botones "Exportar idioma/tema/config" (rfd save dialog) e
  "Importar pack" (rfd open dialog); tras importar idioma/tema, recargar el catálogo
  (`I18n::load`/`ThemeCatalog::load`) y refrescar selectores; tras importar config, recargar
  settings/keymap. Verificación viva: exportar un idioma, importarlo en un dir limpio.

- [ ] **Step 5: Gate + commit**

```bash
git commit -F - <<'EOF'
feat(slint): import/export de packs (.zip) — idiomas, temas y configuración

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Fase F — Cierre

### Task 9: Verificación integral + release + push

- [ ] **Step 1: Gate completo** — `cargo test --workspace` + clippy `-D warnings` + fmt.
- [ ] **Step 2: Verificación viva (Win32/computer-use)** — capturando: abrir Config; cambiar
  una opción y confirmar persistencia al reabrir; reasignar un atajo; cambiar idioma (UI
  traducida) y tema (recoloreada) en caliente; cerrar con varios paneles y reabrir
  restaurándolos; exportar e importar un pack de idioma y uno de tema.
- [ ] **Step 3: dist + memoria + graphify** — `cp target/release/naygo-slint.exe
  dist/slint-fase1/`; actualizar `memory/project-migracion-slint.md` con el estado de F4;
  `graphify update .`.
- [ ] **Step 4: push a main** (autorizado al cierre) — stage explícito; `git push origin main`;
  verificar sync (`git rev-list --left-right --count origin/main...main` = `0 0`).
- [ ] **Step 5: avisar a Nicolás** — F4 funcional+verificada; pedir rendimiento en la VM; y
  entregar el RESUMEN de todas las fases de la migración (lo pidió explícitamente).

---

## Self-review (cobertura del spec)

- §1 Arquitectura (ConfigCtrl + globals Tr/Theme) → Tasks 1, 3, 5. ✓
- §2 Persistencia del layout → Task 2 (+ core from_persist). ✓
- §3 Ventana de configuración → Task 6. ✓
- §4 Editor de atajos → Task 7. ✓
- §5 Temas + i18n → Tasks 3-4 (i18n), 5 (temas), 6 (selectores en caliente). ✓
- §6 Import/Export .zip → Task 8. ✓
- §7 Testing → tests en cada task + Task 9 verificación viva. ✓
- §8 Puertas → en cada commit. ✓
- §9 Riesgos (externalizar todo por archivo; aplicar colores por archivo; reconstrucción de
  layout reusada de egui→core; deps zip/rfd; packs) → Tasks 2,4,5,8 + nota de alineación
  (temas = JSON plano, no carpeta; íconos por tema fuera de F4). ✓

Sin placeholders: cada step tiene código o comando concreto. Tipos consistentes: `ConfigCtrl`,
`session_persist`/`save_session`/`load_session`, `Workspace::from_persist`/`push_node`/
`set_next_id`, globals `Tr`/`Theme`, `i18n_keys::apply`, `theme_apply::apply`, `packs::{export_*,
import_zip}`, `SettingsVm`/`ShortcutRowVm` se usan con los mismos nombres en todas las tasks.
```
