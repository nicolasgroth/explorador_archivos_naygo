# Multi-selección estilo Explorer — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Marcar varios archivos (clic / Ctrl+clic / Shift+clic, rectángulo desde celdas no-nombre o espacio vacío, y teclado), con las acciones operando sobre toda la selección.

**Architecture:** La lógica de selección es PURA en `core` (métodos sobre `FilePaneState`, espacio de vista, testeable sin UI). La UI (`file_panel`/`app`) detecta el input (clic con modificadores, rubber-band, teclado) y llama esos métodos; pinta `selected` y el foco/ancla. `selected_paths()` ya soporta la rama multi — solo hay que poblar `selected`.

**Tech Stack:** Rust, `naygo-core`, eframe/egui 0.34 + egui_extras (TableBuilder).

**Estado de partida (rama `feat/multiseleccion`, desde `main` 3f6af6a):**
- `crates/core/src/workspace/file_pane.rs`: `FilePaneState { ..., pub focused: Option<usize>, pub selected: Vec<usize>, ... }` — ambos en espacio de VISTA (posición en `view_indices()`). `FilePanePersist` (lo serializable) NO incluye `focused`/`selected`/`highlighted` → un campo nuevo `anchor` tampoco se persiste. `view_indices() -> Vec<usize>` (filtrada). `focused_view_entry()`. `fn enter(&mut self, dir)` (privado) ya hace `self.focused = None; self.selected.clear();` al navegar. `new()` inicializa `focused: None, selected: Vec::new()`.
- `crates/ui/src/app.rs`:
  - `fn selected_paths(&self) -> Vec<PathBuf>` (~1486): YA tiene `if !f.selected.is_empty() { ...mapea f.selected (pos vista)→view→entries... } else { f.focused_view_entry() }`. NO reescribir.
  - `fn move_focus(&mut self, delta: isize)` (~mover foco): pone `f.focused = Some((cur+delta).clamp(...))`; NO toca `selected`.
  - `fn apply_action(&mut self, action: Action)` (~1344): `Action::MoveUp => self.move_focus(-1)`, `MoveDown => self.move_focus(1)`, etc.
  - `fn handle_input(&mut self, ctx)`: arma Chord por tecla → `keymap.action_for` → `apply_action`. Typeahead aparte.
  - `self.status: String`; `self.i18n.t(key) -> &str` (usar `.to_string()` al asignar a status); `self.active_theme` (`ActiveTheme`, `.accent() -> Color32`).
  - `self.workspace.active_files()` / `active_files_mut() -> Option<&FilePaneState>`.
- `crates/core/src/keymap.rs`: `pub enum Action { MoveUp, MoveDown, Activate, Open, OpenWith, GoUp, GoBack, GoForward, SwitchPane, CancelListing, Copy, Cut, Paste, Delete, DeletePermanent, Rename, NewFile, NewDir, CopyToOther, MoveToOther, ComputeSize }`. `Action::all()`, `i18n_key()`. `KeyMap::defaults()`. `Chord` con ctrl/shift/alt.
- `crates/ui/src/panes/file_panel.rs`: el body (`body.rows(ROW_HEIGHT, rows.len(), |mut row| {...})`):
  - `DisplayRow::Entry(i)`: `let selected = focused == Some(i);` → `row.set_selected(selected);` (~263-275). Celda `ci == 0` = nombre (`icon_row(ui, icons, key, &entry.name, name_color)`); otras celdas = `ui.label(text)`. La fila completa: `let row_resp = row.response(); if row_resp.clicked() { clicked = Some(i); } if row_resp.double_clicked() { activated = Some(i); } if row_resp.secondary_clicked() { context_focus = Some(i); } row_resp.context_menu(|ui| {...})`.
  - Variables difieridas al inicio del render: `let mut clicked: Option<usize> = None;`, `activated`, `context_focus`, `parent_activated`. `let focused = f.focused;`. La fn recibe `f: &FilePaneState`, `ops_actions: &mut Vec<Action>`, `native_menu_request: &mut Option<(f32,f32)>`, `theme`, `icons`, `i18n`, etc. (READ la firma completa.)
  - El TableBuilder usa `.sense(egui::Sense::click())` (~175). NO hay `dnd_drag_source` en las filas (solo en headers de columna).
  - `ROW_HEIGHT` constante. `view` = `f.view_indices()` mapeado a `&Entry` (READ cómo arma `rows`/`view`).
  - Tras pintar, `app.rs` drena `clicked`/`activated`/`context_focus` (~2471+ donde llama file_panel via NaygoTabViewer en docking.rs).
- `crates/core/src/format.rs`: `pub fn human_size(bytes: u64) -> String`. `Entry { pub size: Option<u64>, pub path: PathBuf, pub name: String, ... }`.
- egui 0.34: `Sense::click_and_drag()`, `Response.drag_started()/dragged()/drag_released()/hover_pos()/interact_pointer_pos()`, `ui.input(|i| i.modifiers.ctrl/shift/command)`, `Response.rect: Rect`, `ui.painter().rect_stroke(rect, rounding, Stroke)`, `egui::Shape::dashed_line(...)`, `ui.interact(rect, id, sense)`.

**Prerequisito de entorno:** Rust en PATH (`export PATH="$HOME/.cargo/bin:$PATH"`). NUNCA `2>&1` con cargo. `cargo fmt --all` antes de cada commit. Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt verdes. Header de 2 líneas en archivos nuevos.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. La lógica de selección en `core` (pura, sin egui). Tolerante. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/multiseleccion`. NO cambiar de rama.

**Reparto de verificación:** el agente: tests PUROS de la lógica de selección + build/clippy/fmt + i18n parity. La prueba VISUAL (rectángulo, Ctrl/Shift, contador, foco, acciones sobre la selección, arrastre-sobre-nombre no dibuja) la hace Nicolás.

**SECUENCIA:** Task 1 (anchor + lógica pura, TDD) es la base testeable. Task 2 (clic + modificadores) y Task 3 (teclado) consumen la lógica. Task 4 (rubber-band) es lo más delicado (mecánica egui). Task 5 (pintar selección + foco punteado). Task 6 (feedback: status + menú). Task 7 (cierre). La selección se LIMPIA al re-listar (ya lo hace `enter()`; verificar filtro/orden).

---

## Estructura de archivos

```
crates/core/src/workspace/file_pane.rs   # + anchor + métodos de selección puros + tests
crates/core/src/keymap.rs                 # + Action::SelectAll
crates/ui/src/app.rs                       # clic con modificadores, teclado, rubber-band apply, status
crates/ui/src/panes/file_panel.rs          # captura clic+mods, rects de fila/nombre, pintar selección+foco, rubber-band, menú "N sel"
crates/core/src/i18n/{es,en}.json          # action.select_all, status.n_selected, menu.n_selected
```

---

## Task 1: Lógica de selección pura + anchor (core, TDD)

**Files:**
- Modify: `crates/core/src/workspace/file_pane.rs`

- [ ] **Step 1: Tests (TDD)**

Add to the `#[cfg(test)] mod tests` of file_pane.rs (helper to build a pane with N fake entries, no filter, so view == entries):
```rust
    fn pane_con_n(n: usize) -> FilePaneState {
        let mut p = FilePaneState::new(std::path::PathBuf::from("C:/"));
        for i in 0..n {
            p.entries.push(crate::fs_model::Entry::new_for_test(
                std::path::PathBuf::from(format!("C:/f{i}.txt")),
                format!("f{i}.txt"),
            ));
        }
        p
    }
```
(If `Entry` has no test constructor, build it via its real fields/`Entry { ... }` literal — READ `fs_model::Entry` to see how; a minimal entry with path+name+size=None suffices since selection works on view positions, not entry contents.)
```rust
    #[test]
    fn select_single_reemplaza_y_fija_ancla() {
        let mut p = pane_con_n(5);
        p.select_single(2);
        assert_eq!(p.selected, vec![2]);
        assert_eq!(p.focused, Some(2));
        assert_eq!(p.anchor, Some(2));
        p.select_single(4);
        assert_eq!(p.selected, vec![4]);
        assert_eq!(p.anchor, Some(4));
    }

    #[test]
    fn select_toggle_agrega_y_quita() {
        let mut p = pane_con_n(5);
        p.select_single(1);
        p.select_toggle(3);
        let mut s = p.selected.clone(); s.sort_unstable();
        assert_eq!(s, vec![1, 3]);
        assert_eq!(p.focused, Some(3));
        p.select_toggle(1); // quita
        assert_eq!(p.selected, vec![3]);
    }

    #[test]
    fn select_range_desde_ancla_normaliza() {
        let mut p = pane_con_n(6);
        p.select_single(4);        // ancla = 4
        p.select_range_to(1);      // shift-clic en 1 → rango 1..=4
        let mut s = p.selected.clone(); s.sort_unstable();
        assert_eq!(s, vec![1, 2, 3, 4]);
        assert_eq!(p.anchor, Some(4)); // el ancla NO se mueve
        assert_eq!(p.focused, Some(1));
    }

    #[test]
    fn select_rect_reemplaza_o_suma() {
        let mut p = pane_con_n(6);
        p.select_single(0);
        p.select_rect(&[2, 3], false); // reemplaza
        let mut s = p.selected.clone(); s.sort_unstable();
        assert_eq!(s, vec![2, 3]);
        p.select_rect(&[5], true);     // suma
        let mut s = p.selected.clone(); s.sort_unstable();
        assert_eq!(s, vec![2, 3, 5]);
    }

    #[test]
    fn select_all_toma_toda_la_vista() {
        let mut p = pane_con_n(4);
        p.select_all();
        let mut s = p.selected.clone(); s.sort_unstable();
        assert_eq!(s, vec![0, 1, 2, 3]);
    }

    #[test]
    fn move_focus_extend_con_shift_extiende_desde_ancla() {
        let mut p = pane_con_n(6);
        p.select_single(2);              // ancla=2, foco=2
        p.move_focus_extend(1, true);    // shift-abajo → foco 3, rango 2..=3
        let mut s = p.selected.clone(); s.sort_unstable();
        assert_eq!(s, vec![2, 3]);
        p.move_focus_extend(1, true);    // foco 4, rango 2..=4
        let mut s = p.selected.clone(); s.sort_unstable();
        assert_eq!(s, vec![2, 3, 4]);
        assert_eq!(p.anchor, Some(2));
    }

    #[test]
    fn move_focus_extend_sin_shift_es_seleccion_simple() {
        let mut p = pane_con_n(6);
        p.select_single(2);
        p.move_focus_extend(1, false);   // abajo sin shift → solo foco 3
        assert_eq!(p.selected, vec![3]);
        assert_eq!(p.anchor, Some(3));
    }

    #[test]
    fn ops_clampean_a_la_vista() {
        let mut p = pane_con_n(3);
        p.select_single(99);             // fuera de rango → clamp a 2
        assert_eq!(p.focused, Some(2));
        assert_eq!(p.selected, vec![2]);
    }
```

- [ ] **Step 2: Run → fail**

Run: `cargo test -p naygo-core file_pane` → ERROR (`anchor` field + methods missing).

- [ ] **Step 3: Implementar**

In `crates/core/src/workspace/file_pane.rs`:
a) Add field to `FilePaneState`: `pub anchor: Option<usize>,` (doc: `/// Ancla de la selección por rango (Shift). Efímero, NO se persiste (no está en FilePanePersist).`). Init `anchor: None,` in `new()`.
b) In `enter()` add `self.anchor = None;` (so navigation clears it too).
c) Add the pure methods:
```rust
    /// Largo de la vista (entries visibles). Clamp helper.
    fn view_len(&self) -> usize {
        self.view_indices().len()
    }

    /// Posición válida dentro de la vista (clamp a [0, len-1]); None si la vista está vacía.
    fn clamp_pos(&self, pos: usize) -> Option<usize> {
        let len = self.view_len();
        if len == 0 {
            None
        } else {
            Some(pos.min(len - 1))
        }
    }

    /// Selección simple (clic): solo `pos`, fija foco y ancla ahí.
    pub fn select_single(&mut self, pos: usize) {
        if let Some(p) = self.clamp_pos(pos) {
            self.selected = vec![p];
            self.focused = Some(p);
            self.anchor = Some(p);
        }
    }

    /// Toggle (Ctrl+clic): agrega o quita `pos`; mueve foco y ancla a `pos`.
    pub fn select_toggle(&mut self, pos: usize) {
        if let Some(p) = self.clamp_pos(pos) {
            if let Some(idx) = self.selected.iter().position(|&x| x == p) {
                self.selected.remove(idx);
            } else {
                self.selected.push(p);
            }
            self.focused = Some(p);
            self.anchor = Some(p);
        }
    }

    /// Rango (Shift+clic): selecciona desde el ancla hasta `pos` (ancla NO cambia).
    /// Si no hay ancla, equivale a `select_single`.
    pub fn select_range_to(&mut self, pos: usize) {
        let Some(p) = self.clamp_pos(pos) else { return };
        let anchor = match self.anchor {
            Some(a) => a.min(self.view_len().saturating_sub(1)),
            None => {
                self.select_single(p);
                return;
            }
        };
        let (lo, hi) = if anchor <= p { (anchor, p) } else { (p, anchor) };
        self.selected = (lo..=hi).collect();
        self.focused = Some(p);
        // anchor se mantiene
    }

    /// Rectángulo (rubber-band): reemplaza con `positions`, o suma si `additive`.
    pub fn select_rect(&mut self, positions: &[usize], additive: bool) {
        let len = self.view_len();
        let valid: Vec<usize> = positions.iter().copied().filter(|&p| p < len).collect();
        if additive {
            for p in valid {
                if !self.selected.contains(&p) {
                    self.selected.push(p);
                }
            }
        } else {
            self.selected = valid;
        }
        // El foco va al último tocado (si hay).
        if let Some(&last) = positions.iter().filter(|&&p| p < len).last() {
            self.focused = Some(last);
            self.anchor = Some(last);
        }
    }

    /// Selecciona toda la vista.
    pub fn select_all(&mut self) {
        let len = self.view_len();
        self.selected = (0..len).collect();
        if len > 0 {
            self.focused = Some(len - 1);
        }
    }

    /// Mueve el foco `delta` (teclado). Con `extend` (Shift) extiende el rango desde el
    /// ancla; sin extend es selección simple del nuevo foco.
    pub fn move_focus_extend(&mut self, delta: isize, extend: bool) {
        let len = self.view_len();
        if len == 0 {
            return;
        }
        let cur = self.focused.unwrap_or(0) as isize;
        let new = (cur + delta).clamp(0, len as isize - 1) as usize;
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(cur as usize);
            }
            self.select_range_to(new);
        } else {
            self.select_single(new);
        }
    }

    /// ¿La posición de vista `pos` está seleccionada?
    pub fn is_selected(&self, pos: usize) -> bool {
        self.selected.contains(&pos)
    }

    /// Cuántos ítems seleccionados.
    pub fn selection_count(&self) -> usize {
        self.selected.len()
    }
```
(NOTE: `select_range_to` is named so to avoid clashing with a 2-arg `select_range(anchor,pos)` — the tests above use `select_range_to(pos)` with the stored anchor. Keep these EXACT names; Tasks 2-3 call them.)

- [ ] **Step 4: Run → pass**

Run: `cargo test -p naygo-core file_pane` → all PASS. `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean. `cargo fmt --all`.

- [ ] **Step 5: Commit**
```
git add crates/core/src/workspace/file_pane.rs
git commit -m "feat(core): lógica de selección múltiple (single/toggle/range/rect/all + ancla)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `anchor`, `select_single`/`select_toggle`/`select_range_to`/`select_rect`/`select_all`/`move_focus_extend`/`is_selected`/`selection_count` EXACTOS.

---

## Task 2: Clic con modificadores (UI)

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: Capturar el clic CON modificadores en file_panel**

In `crates/ui/src/panes/file_panel.rs`, change the deferred-click variable from `Option<usize>` to carry modifiers. At the top where `let mut clicked: Option<usize> = None;` is declared, change to:
```rust
    let mut clicked: Option<(usize, bool, bool)> = None; // (pos, ctrl, shift)
```
In the row handler, replace `if row_resp.clicked() { clicked = Some(i); }` with:
```rust
                        if row_resp.clicked() {
                            let (ctrl, shift) = ui.input(|inp| {
                                (inp.modifiers.command || inp.modifiers.ctrl, inp.modifiers.shift)
                            });
                            clicked = Some((i, ctrl, shift));
                        }
```
(`command` covers macOS ⌘; on Windows it's `ctrl`. Including both is harmless.)

- [ ] **Step 2: Procesar el clic en app.rs con la lógica pura**

Find where `app.rs` drains `clicked` after the file_panel render (search for the old `clicked` handling — it likely called something like focusing/selecting). Replace the handling so it dispatches to the pure methods on the active pane:
```rust
        if let Some((pos, ctrl, shift)) = clicked {
            if let Some(f) = self.workspace.active_files_mut() {
                if shift {
                    f.select_range_to(pos);
                } else if ctrl {
                    f.select_toggle(pos);
                } else {
                    f.select_single(pos);
                }
            }
            // (mantener cualquier efecto colateral que el clic tenía antes: activar panel,
            //  limpiar highlight de interacción, etc. — verificar el código previo)
        }
```
(VERIFY the exact spot/signature where `clicked` flows from the file_panel call (through `NaygoTabViewer` in docking.rs — the out-param type changes from `Option<usize>` to `Option<(usize,bool,bool)>`; update the field in `NaygoTabViewer` and the local in app.rs to match). Preserve whatever the old click did beyond focus, e.g. `clear_highlight_on_interact`.)

- [ ] **Step 3: Verify**

Run: `cargo build -p naygo-ui` → compiles. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.
(Visual: Nicolás verifies clic/Ctrl/Shift behave.)

- [ ] **Step 4: Commit**
```
git add crates/ui/src/panes/file_panel.rs crates/ui/src/app.rs crates/ui/src/docking.rs
git commit -m "feat(ui): clic con Ctrl/Shift puebla la selección múltiple

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Teclado (flechas/Shift/Ctrl+A/Espacio)

**Files:**
- Modify: `crates/core/src/keymap.rs`
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: Action::SelectAll + i18n**

In `crates/core/src/keymap.rs`: add `SelectAll` to the `Action` enum; add it to `Action::all()`; in `i18n_key()` return `"action.select_all"`; in `KeyMap::defaults()` bind it to `Ctrl+A` (`Chord::ctrl(KeyCode::Char('a'))` — match the existing chord constructor style). 
In both `crates/core/src/i18n/es.json` and `en.json` add `"action.select_all"`: ES `"Seleccionar todo"`, EN `"Select all"` (identical key — there's an action-parity test).

- [ ] **Step 2: Teclado en apply_action / move_focus**

In `crates/ui/src/app.rs`:
a) Change `move_focus(delta)` (used by MoveUp/MoveDown) to route through the pure method so arrows populate `selected` (single-select on the new focus, reading Shift from input). Replace its body:
```rust
    fn move_focus(&mut self, delta: isize) {
        let shift = /* read shift from the last input — see note */ false;
        if let Some(f) = self.workspace.active_files_mut() {
            f.move_focus_extend(delta, shift);
        }
    }
```
NOTE: `move_focus` may not have `ctx` to read modifiers. Two options — pick what fits the codebase: (i) read shift in `handle_input` where the Chord is built (the chord already knows shift) and pass it down, e.g. add `MoveUpExtend`/use the chord's shift; OR (ii) simplest: in `handle_input`, when the action is `MoveUp`/`MoveDown`, read `ctx.input(|i| i.modifiers.shift)` and call `f.move_focus_extend(delta, shift)` directly instead of the no-arg `move_focus`. Prefer (ii): in the `apply_action` or `handle_input` path for MoveUp/MoveDown, thread the shift flag. VERIFY how handle_input maps keys → actions and whether shift is available there (the Chord has a `shift` field — a `Shift+Down` chord may map to no action today, so reading `ctx.input` modifiers at the MoveUp/Down handler is the robust route).
b) Add `Action::SelectAll => { if let Some(f) = self.workspace.active_files_mut() { f.select_all(); } }` to `apply_action`.
c) Space toggles the focused item: add a handler — if there's an `Action` for Space or it's handled in `handle_input`, call `f.select_toggle(focus)` for the current focus. If Space isn't an action, add minimal handling in `handle_input`: on `KeyCode::Char(' ')` (or Space key) with no modifiers and not in typeahead, toggle the focused pos. VERIFY whether Space is consumed by typeahead — if so, gate it (Space toggles only when there IS a focus and the typeahead buffer is empty). Keep it simple; if Space conflicts with typeahead, document and make Ctrl+Space or skip Space (report the decision).

- [ ] **Step 3: Verify**

Run: `cargo build --workspace` → compiles. `cargo test --workspace` → green (action i18n parity passes). `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.

- [ ] **Step 4: Commit**
```
git add crates/core/src/keymap.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat: selección por teclado (Shift+flechas, Ctrl+A, Espacio)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Rubber-band (rectángulo de selección)

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/ui/src/app.rs`

ESTA ES LA TAREA MÁS DELICADA (mecánica egui). El rectángulo arranca al arrastrar desde una celda NO-nombre o el espacio vacío; NO desde la celda del nombre (reservada a drag&drop futuro).

- [ ] **Step 1: Capturar rects durante el render**

In `file_panel.rs`, during the row render, collect two things into vectors declared before the table:
```rust
    let mut row_rects: Vec<(usize, egui::Rect)> = Vec::new();   // (pos vista, rect fila completa)
    let mut name_rects: Vec<egui::Rect> = Vec::new();           // rects de la celda NOMBRE (ci==0)
```
In the `DisplayRow::Entry(i)` arm: after rendering the cells, capture the name cell's rect (the `ci == 0` column's `ui.max_rect()` or the cell response rect — the existing code already enters `row.col(|ui| {...})` for `ci==0`; capture `ui.max_rect()` there into a local and push to `name_rects`), and push `(i, row_resp.rect)` to `row_rects` using the row response.
(VERIFY how to get the name-cell rect: inside the `ci == 0` `row.col(|ui| ...)` closure, `ui.max_rect()` is the cell area — capture it. The `row.response().rect` is the full row. Adapt to the egui_extras API.)

- [ ] **Step 2: Detectar el arrastre de fondo (rubber-band) y pintarlo**

After the table is built (after `body.rows(...)`), add the rubber-band logic. The TableBuilder area's response: get a response for the whole panel area to detect drag from empty space. Use the table's outer rect (the `ui` the file_panel renders into). Concretely:
```rust
    // Rubber-band: arrastrar desde una celda NO-nombre o el espacio vacío dibuja un
    // rectángulo de selección. Arrastrar sobre la celda del NOMBRE NO lo dispara (se
    // reserva para drag&drop futuro). El clic simple de fila lo maneja `Sense::click`
    // de cada fila (independiente de este arrastre de fondo).
    let panel_rect = ui.min_rect(); // o el rect que ocupa la tabla — verificar
    let band_id = egui::Id::new(("rubberband", id.0));
    let band_resp = ui.interact(panel_rect, band_id, egui::Sense::click_and_drag());
    // ... usar un estado persistente (egui memory) para el punto de inicio del arrastre ...
```
The mechanics (resolve precisely against egui 0.34 — this is the crux):
- On `band_resp.drag_started()`: read the start pointer pos (`band_resp.interact_pointer_pos()`). If that pos falls INSIDE any `name_rects` rect → DO NOT start a rubber-band (it's a name-cell drag, reserved). Otherwise, store the start pos in egui temp memory (`ui.memory_mut(|m| m.data.insert_temp(band_id, start))`).
- On `band_resp.dragged()` with a stored start: compute `Rect::from_two_pos(start, current)`; paint it (Step 3); compute which `row_rects` it intersects → collect their pos → store as the pending selection (or compute on release).
- On `band_resp.drag_released()` (or when the drag ends): if there was a stored start, read ctrl (`ui.input(|i| i.modifiers.ctrl || i.modifiers.command)`), gather intersected positions, and signal them out via a new out-param `rubber_select: &mut Option<(Vec<usize>, bool)>` (positions, additive=ctrl). Clear the stored start from memory.
NOTE: distinguishing "drag started on a name cell" requires the start point; since rows have `Sense::click()` (not drag), a drag gesture starting over a row is NOT consumed by the row (rows only sense clicks), so the background `click_and_drag` interact on `panel_rect` SHOULD receive the drag. VERIFY this coexistence: row `Sense::click()` + panel `Sense::click_and_drag()` — a click still goes to the row (click), a drag goes to the panel. If egui gives the drag to the row's response area instead, adjust (e.g. make the panel interact use a higher layer, or check `band_resp` actually fires). This is the key risk — test it builds and the drag fires; Nicolás verifies the feel.

- [ ] **Step 3: Pintar el rectángulo punteado (estilo B)**

While dragging, paint the rubber-band rect with a dashed azure border + faint fill:
```rust
        let painter = ui.painter();
        let fill = egui::Color32::from_rgba_unmultiplied(
            theme.accent().r(), theme.accent().g(), theme.accent().b(), 16,
        );
        painter.rect_filled(band_rect, 0.0, fill);
        // Borde punteado: egui::Shape::dashed_line por cada lado, o rect_stroke sólido fino
        // si dashed resulta complejo. Intentar dashed primero.
        let stroke = egui::Stroke::new(1.0, theme.accent());
        let dash = 4.0; let gap = 3.0;
        let c = band_rect;
        let corners = [c.left_top(), c.right_top(), c.right_bottom(), c.left_bottom(), c.left_top()];
        for w in corners.windows(2) {
            painter.add(egui::Shape::dashed_line(&[w[0], w[1]], stroke, dash, gap));
        }
```
(VERIFY `egui::Shape::dashed_line(points: &[Pos2], stroke, dash_length, gap_length) -> Vec<Shape>` signature in egui 0.34 — it may return `Vec<Shape>` (then `painter.extend(...)`) or a single Shape. Adapt. If dashed_line is awkward, fall back to a thin solid `rect_stroke` — report which you used.)

- [ ] **Step 4: Aplicar la selección del rect en app.rs**

Thread the new out-param `rubber_select: &mut Option<(Vec<usize>, bool)>` from the file_panel call (through `NaygoTabViewer`/docking.rs) to app.rs. After draining `clicked`, handle it:
```rust
        if let Some((positions, additive)) = rubber_select {
            if let Some(f) = self.workspace.active_files_mut() {
                f.select_rect(&positions, additive);
            }
        }
```

- [ ] **Step 5: Verify**

Run: `cargo build -p naygo-ui` → compiles. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.
(Visual: Nicolás verifies the rectangle draws from empty/non-name cells, not from the name; Ctrl adds.)

- [ ] **Step 6: Commit**
```
git add crates/ui/src/panes/file_panel.rs crates/ui/src/app.rs crates/ui/src/docking.rs
git commit -m "feat(ui): rectángulo de selección (rubber-band) desde celdas no-nombre/vacío

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Pintar la selección múltiple + foco/ancla punteado

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`

- [ ] **Step 1: Pintar todas las filas seleccionadas + el foco distinguible**

In `file_panel.rs` `DisplayRow::Entry(i)`: change `let selected = focused == Some(i);` to use the pane's full selection:
```rust
                        let selected = f.is_selected(i);
                        let is_focus = focused == Some(i);
```
Keep `row.set_selected(selected);` (now reflects multi-selection). The `is_new` highlight check should consider `selected` (already does `!selected`).
After the cells are rendered, if `is_focus`, paint a dashed border around the row to distinguish the focus/anchor from the rest of the selection:
```rust
                        if is_focus {
                            let r = row.response().rect;
                            let stroke = egui::Stroke::new(1.0, theme.accent());
                            let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                            for w in corners.windows(2) {
                                ui.painter().add(egui::Shape::dashed_line(&[w[0], w[1]], stroke, 3.0, 2.0));
                            }
                        }
```
(Reuse whatever dashed approach worked in Task 4. `f` is the `&FilePaneState` the fn already receives. VERIFY `f.is_selected` is reachable — the fn takes `f: &FilePaneState`. If the fn currently only receives `focused` not `f`, it DOES receive `f` per the render — confirm and use `f.is_selected(i)`.)

- [ ] **Step 2: Verify**

Run: `cargo build -p naygo-ui` → compiles. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.

- [ ] **Step 3: Commit**
```
git add crates/ui/src/panes/file_panel.rs
git commit -m "feat(ui): pintar la selección múltiple y distinguir el foco con borde punteado

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Feedback — contador en status + encabezado del menú

**Files:**
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: i18n**

Add to both json (identical keys):
- ES `status.n_selected`: `"{n} seleccionados"` ; `menu.n_selected`: `"{n} seleccionados"`
- EN `status.n_selected`: `"{n} selected"` ; `menu.n_selected`: `"{n} selected"`
(VERIFY how the i18n layer does interpolation — if it has no `{n}` substitution, build the string with `format!` in code using a key that's just the suffix, e.g. ES `"seleccionados"` / EN `"selected"` and prepend the count + size in code. Adapt to the real i18n API; the simplest is a plain label key and `format!("{n} {}", t("...selected"))`.)

- [ ] **Step 2: Status con contador + tamaño sumado**

In `app.rs`, after processing a selection change (clic/rubber/teclado), if `selection_count() >= 2`, set the status. Add a helper:
```rust
    /// Actualiza la barra de estado con el resumen de la selección múltiple (N + tamaño
    /// conocido sumado). Las carpetas sin tamaño calculado no suman (no se dispara cálculo).
    fn update_selection_status(&mut self) {
        let Some(f) = self.workspace.active_files() else { return };
        let count = f.selection_count();
        if count < 2 {
            return; // 0/1: el status normal lo maneja el flujo habitual
        }
        let view = f.view_indices();
        let total: u64 = f
            .selected
            .iter()
            .filter_map(|&pos| view.get(pos))
            .filter_map(|&real| f.entries.get(real))
            .filter_map(|e| e.size)
            .sum();
        let label = self.i18n.t("status.selected_suffix"); // "seleccionados"/"selected"
        self.status = format!("{count} {label} · {}", naygo_core::format::human_size(total));
    }
```
Call `self.update_selection_status()` after each selection-mutating handler (clic, rubber, teclado). (Adjust the i18n key to whatever you added in Step 1 — keep it consistent. If you prefer the `{n}` template, use the real interpolation.)

- [ ] **Step 3: Encabezado "N seleccionados" en el menú contextual**

In `file_panel.rs`, in the `row_resp.context_menu(|ui| {...})`, at the TOP (before the first button), if there's a multi-selection show a disabled header:
```rust
                        row_resp.context_menu(|ui| {
                            context_focus = Some(i);
                            let n = f.selection_count();
                            if n >= 2 {
                                ui.add_enabled(false, egui::Button::new(
                                    format!("{n} {}", i18n.t("menu.selected_suffix"))
                                ));
                                ui.separator();
                            }
                            // ... resto de los botones igual ...
```
(Use the same suffix-key approach as Step 2. `f.selection_count()` reachable via `f`.)

- [ ] **Step 4: Verify**

Run: `cargo build --workspace` → compiles. `cargo test --workspace` → green (i18n parity). `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all`.

- [ ] **Step 5: Commit**
```
git add crates/ui/src/app.rs crates/ui/src/panes/file_panel.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): contador de selección en la barra de estado + encabezado del menú

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Cierre — limpieza al filtrar/ordenar + verificación final + push

**Files:**
- Modify: `crates/ui/src/app.rs` (o donde se apliquen filtros/orden)
- Modify: `README.md`

- [ ] **Step 1: Limpiar la selección al cambiar filtro/orden**

`enter()` ya limpia `selected`/`focused`/`anchor` al navegar/re-listar. Falta: cuando el usuario cambia un FILTRO o el ORDEN (las posiciones de vista cambian), la selección por posición queda desfasada. Buscar dónde se aplican `TableAction` de filtro/orden en `app.rs` (donde se mutan `f.table.filters` / `f.sort`) y tras aplicarlas hacer `f.selected.clear(); f.anchor = None;` (el foco se puede conservar o limpiar — preferir limpiar selección, conservar foco si sigue válido tras clamp). VERIFICAR los call sites de cambio de filtro/orden; si son varios, un helper `clear_selection_on_view_change()`. Si re-ordenar mantuviera las mismas entries (solo cambia el orden), técnicamente se podría re-mapear, pero el spec decidió LIMPIAR (simple, como Windows). Documentar.

- [ ] **Step 2: README estado**

Update the README status block to mention multi-selección entre lo completado:
```markdown
> **Estado:** Sprint de funcionalidad completo + pulido. Multi-selección estilo Explorer
> (clic/Ctrl/Shift, rectángulo, teclado) recién agregada. Diseño en
> [`docs/superpowers/specs/2026-06-09-naygo-multiseleccion-design.md`](docs/superpowers/specs/2026-06-09-naygo-multiseleccion-design.md).
```
(READ the current status block and replace it.)

- [ ] **Step 3: Verificación final**

Run: `cargo build --workspace` → compiles. `cargo build --release -p naygo-ui` → release compila. `cargo test --workspace` → green. `cargo clippy --workspace --all-targets -- -D warnings` → clean. `cargo fmt --all -- --check` → clean.

- [ ] **Step 4: Commit + push**
```
git add crates/ui/src/app.rs README.md
git commit -m "feat: limpiar selección al cambiar filtro/orden + estado del README (multi-selección)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/multiseleccion
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| anchor en FilePaneState (efímero) | 1 |
| Lógica pura single/toggle/range/rect/all/teclado | 1 |
| Tests de la lógica | 1 |
| Clic simple / Ctrl / Shift | 2 |
| Teclado Shift+flechas / Ctrl+A / Espacio | 3 |
| Rubber-band híbrido (nombre=drag, resto/vacío=rect) | 4 |
| Rectángulo punteado estilo B | 4 |
| Ctrl+arrastrar = suma | 4 |
| Pintar selección múltiple | 5 |
| Foco/ancla punteado distinguible | 5 |
| Contador + tamaño en status | 6 |
| Encabezado "N seleccionados" en menú | 6 |
| Acciones sobre toda la selección (selected_paths) | (ya existe; se activa al poblar selected en 2-4) |
| Limpiar selección al re-listar/filtrar/ordenar | 1 (enter, ya), 7 (filtro/orden) |
| i18n parity | 3, 6 |
| FUERA: drag&drop, inline rename, cortado atenuado | (no se tocan) |

**Notas de riesgo:**
- **Rubber-band fondo vs nombre vs fila** (Task 4): lo más delicado. Rows tienen `Sense::click()` (no drag) → un drag debería ir al `click_and_drag` de fondo; verificar la coexistencia en egui 0.34 y que el drag dispare. Distinguir inicio-sobre-nombre guardando `name_rects` y chequeando el punto inicial. Si la mecánica no funciona limpia con TableBuilder, reportar y considerar acotar el rect al espacio bajo la última fila (degradar) — pero intentar el híbrido primero.
- **Borde punteado** (Tasks 4,5): `egui::Shape::dashed_line` en egui 0.34 (verificar firma/retorno); fallback sólido fino. Reusar el mismo enfoque en ambas tareas.
- **anchor no persiste** (Task 1): no está en `FilePanePersist` — confirmado; solo se agrega a `FilePaneState`.
- **selected en espacio de vista** (Task 7): limpiar al cambiar filtro/orden (no re-mapear). `enter()` ya cubre navegación.
- **Teclado shift** (Task 3): leer `ctx.input(|i| i.modifiers.shift)` en el handler de MoveUp/MoveDown (el chord Shift+flecha quizá no mapea a acción); Espacio puede chocar con typeahead → gatear o reportar alternativa.
- **Ctrl+A** (Task 3): Action::SelectAll configurable + i18n parity.
- **Tamaño sumado** (Task 6): solo `entry.size` conocido; NO disparar F3.
- **selected_paths()** ya soporta multi — NO reescribir; las acciones se activan solas al poblar `selected`.
- **out-params nuevos** (Tasks 2,4): `clicked` pasa a `(usize,bool,bool)` y se agrega `rubber_select` — actualizar `NaygoTabViewer` (docking.rs) y los call sites.
```
