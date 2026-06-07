# Pulido de UX de la Fase 2E — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cuatro pulidos de UX sobre la tabla de 2E: fila completa clicable (file panel + árbol), línea de inserción al reordenar columnas, cambiar el glifo de filtro `⏷`→`≡`, y abrir el menú de columna también con clic derecho.

**Architecture:** Todo es capa `ui` (`file_panel.rs`, `tree_panel.rs`); `core` NO cambia. Se reutiliza `column_menu::show_menu` y el `TableState` existentes. APIs egui 0.34.3 verificadas contra la fuente del registry.

**Tech Stack:** Rust, `naygo-ui`, `eframe`/`egui` 0.34.3, `egui_extras` 0.34.3. Sin dependencias nuevas.

**Estado de partida (rama base `feat/fase2e-pulido-ux`, desde `main` con 2E ya mergeada):**
- `crates/ui/src/panes/file_panel.rs`: `show(ui, workspace, id, pending, icons, show_parent_entry, i18n, table_actions)`. Cuerpo usa `egui_extras::TableBuilder`: `.header(HEADER_HEIGHT, |mut header| { for col ... { header.col(|ui| { ui.dnd_drag_source(dnd_id, to_real, |ui| column_header(...)) ; if let Some(from_real)=resp.dnd_release_payload::<usize>(){...MoveColumn...} }) } })` y `.body(|body| body.rows(ROW_HEIGHT, rows.len(), |mut row| { match rows[row.index()] { DisplayRow::Parent => {...}, DisplayRow::Entry(i) => { row.set_selected(selected); for col { row.col(|ui| { if ci==0 { icon_row(...); if resp.clicked() clicked=Some(i); if resp.double_clicked() activated=Some(i); } else ui.label(cell_text(...)) }) } }, DisplayRow::NoMatches => {...} } }))`. Después del body: detección de resize (measured_widths) y procesamiento de `parent_activated`/`clicked`/`activated`.
  - `column_header(ui, id, kind, table, sort, ext_counts, i18n, actions)`: arma `title` (string) + `▲/▼` si es la columna de orden + `⏷` si `table.filters.contains_key(&kind)`, pinta `ui.label(RichText::new(title).strong())`, luego `let menu_button = ui.add(egui::Button::new("▾").frame(false)); let popup_id = ui.make_persistent_id(("col_menu", id.0, kind)); egui::Popup::menu(&menu_button).id(popup_id).show(|ui| column_menu::show_menu(...));`.
  - `icon_row(ui, icons, key, name, selected) -> egui::Response`: une ícono (con `Sense::click()`) + `selectable_label(selected, name)` con `Response::union`, devuelve `.inner`.
  - Constantes: `ICON_SIZE`, `HEADER_HEIGHT`, `ROW_HEIGHT`, `WIDTH_CHANGE_EPS`. `DisplayRow { Parent, Entry(usize), NoMatches }`. `view: Vec<Entry>` = entries filtradas+ordenadas; `focused: Option<usize>` = posición en la vista.
- `crates/ui/src/panes/tree_panel.rs`: `show_node(ui, node, depth, tree, actions, icons, i18n) -> bool`. Pinta una fila con un closure `row_content`: `ui.horizontal(|ui| { ui.add_space(depth*INDENT); if !Empty/Error { let tri = ui.add(Label::new(glyph).sense(click)); if tri.clicked() {Expand/Collapse} } else add_space(INDENT); let tex=icons.texture(key); ui.add(Image::new(tex).fit_to_exact_size(...)); let label = ui.selectable_label(is_active, &node.name); if label.clicked() {Navigate}; if Loading {spinner} }).response`. Luego envuelve `row_content` en un `Frame` si `is_active`. `INDENT`, `ICON_SIZE` constantes. `TreeAction { Expand(PathBuf), Collapse(PathBuf), Navigate(PathBuf) }`.
- `crates/ui/src/column_menu.rs`: `show_menu(ui, kind, table, sort, ext_counts, i18n, actions)` — el desplegable modo B. NO cambia en esta sub-fase.

**APIs egui 0.34.3 verificadas (contra `C:\Users\ngrot\.cargo\registry\src\index.crates.io-*\egui-0.34.3\` y egui_extras-0.34.3):**
- `egui::Response::secondary_clicked() -> bool` (response.rs:204).
- `egui::Response::dnd_hover_payload::<P>() -> Option<Arc<P>>` (487); `dnd_release_payload` (503); `contains_pointer() -> bool` (314).
- `egui::Painter::vline(x: f32, y: impl Into<Rangef>, stroke: impl Into<Stroke>)` (painter.rs:378).
- `egui_extras::TableRow::response() -> egui::Response` (egui_extras table.rs:1355) — cubre la fila completa. `TableRow::col(...) -> (Rect, Response)`.

**Prerequisito:** toolchain Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo en PowerShell. Binario `--bin naygo`. Verificar `$LASTEXITCODE`.

**Convenciones (CLAUDE.md):** código en inglés; comentarios/commits en español OK. `core` no se toca. Build limpio + tests + clippy antes de cada commit. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/fase2e-pulido-ux`. NO cambiar de rama.

**Validación:** estas mejoras son interacción/pintado de egui → validación MANUAL. No forzar tests sobre el render. Solo se extrae+testea una función pura si surge naturalmente (Tarea 3 incluye una).

---

## Estructura de archivos

```
crates/ui/src/panes/file_panel.rs   # fila completa (TableRow::response); línea de drop (vline);
                                     # glifo filtro ⏷→≡; clic derecho abre el menú
crates/ui/src/panes/tree_panel.rs   # fila de nodo completa clicable (triángulo conserva su zona)
crates/ui/src/column_menu.rs        # SIN cambios (se reutiliza)
```

---

## Task 1: File panel — fila completa clicable (TableRow::response)

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`

Objetivo: clic en CUALQUIER celda/zona de una fila la selecciona; doble clic en cualquier parte navega/activa. Usar `row.response()` (cubre la fila completa) en vez de detectar el clic solo en la celda del nombre.

- [ ] **Step 1: Cambiar la rama `DisplayRow::Entry` para usar `row.response()`**

En `.body(|body| body.rows(...))`, la rama `DisplayRow::Entry(i)` actualmente pinta cada celda y captura el clic solo en `ci==0` vía `icon_row`. Reemplazar la captura de clic por el response de la fila completa, DESPUÉS de pintar todas las celdas.

Reemplazar el bloque `DisplayRow::Entry(i) => { ... }` por:
```rust
                    DisplayRow::Entry(i) => {
                        let entry = &view[i];
                        let selected = focused == Some(i);
                        row.set_selected(selected);
                        for (ci, col) in visible_cols.iter().enumerate() {
                            row.col(|ui| {
                                if ci == 0 {
                                    let key = icon_key_for(entry);
                                    // El ícono+nombre se pintan; el sensado de clic de la
                                    // FILA completa (abajo) cubre toda la fila, así que
                                    // aquí no hace falta capturar el clic por celda.
                                    let _ = icon_row(ui, icons, key, &entry.name, false);
                                } else {
                                    ui.label(cell_text(entry, col.kind));
                                }
                            });
                        }
                        // Fila completa clicable: clic en cualquier celda/zona selecciona;
                        // doble clic navega/activa. `TableRow::response()` cubre la fila.
                        let row_resp = row.response();
                        if row_resp.clicked() {
                            clicked = Some(i);
                        }
                        if row_resp.double_clicked() {
                            activated = Some(i);
                        }
                    }
```

- [ ] **Step 2: Hacer lo mismo en la fila `..` (DisplayRow::Parent)**

Reemplazar el bloque `DisplayRow::Parent => { ... }` por:
```rust
                    DisplayRow::Parent => {
                        // ".." se ve como una carpeta normal (estilo Total Commander).
                        for (ci, _col) in visible_cols.iter().enumerate() {
                            row.col(|ui| {
                                if ci == 0 {
                                    let _ = icon_row(ui, icons, IconKey::Folder, "..", false);
                                }
                            });
                        }
                        // Fila completa: ".." sube con un clic (o doble), en cualquier celda.
                        let row_resp = row.response();
                        if row_resp.clicked() || row_resp.double_clicked() {
                            parent_activated = true;
                        }
                    }
```

NOTA: `row.response()` debe llamarse DESPUÉS de los `row.col(...)` (cuando la fila ya pintó todas sus celdas). Si el borrow checker se queja porque `row` se movió, revisar: `body.rows(..., |mut row| {...})` da `row` por valor mutable; `row.col` toma `&mut self`, `row.response()` toma `&self` — se puede llamar tras los `col`. Verificar que compila; si `row.response()` requiere que la fila esté "terminada", igual funciona al final del closure.

- [ ] **Step 3: Compilar y verificar**

Run: `cargo build -p naygo-ui` → compila.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio. (Si `icon_row`'s `selected` param queda siempre `false` y el `Sense::click()` del ícono ahora es redundante, NO lo quites: `icon_row` se usa igual; dejarlo es inofensivo. Si clippy marca algo sin usar, resolverlo mínimamente.)
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start (`--bin naygo`): clic a la derecha del nombre de un archivo (sobre la columna Tamaño o el espacio vacío) selecciona esa fila; doble clic en cualquier celda entra a la carpeta; ".." sube con un clic en cualquier parte.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/panes/file_panel.rs
git commit -m "feat(ui): fila completa clicable en el file panel (TableRow::response)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Árbol — fila de nodo completa clicable

**Files:**
- Modify: `crates/ui/src/panes/tree_panel.rs`

Objetivo: clic en cualquier parte de la fila de un nodo (ícono o nombre, no el triángulo) navega; el triángulo ▶/▼ conserva su zona para expandir/colapsar.

- [ ] **Step 1: Unir ícono + nombre en una zona clicable única**

En `show_node`, dentro de `row_content`, el ícono se pinta con `ui.add(Image::new(tex)...)` (sin sense) y el nombre con `selectable_label`. Cambiar para que AMBOS formen un único elemento clicable (patrón `icon_row` del file panel), sin incluir el triángulo.

Reemplazar el bloque del ícono + nombre (las líneas que pintan `Image` y `selectable_label`) por:
```rust
            // Ícono: unidad si el nodo es una raíz, carpeta en otro caso.
            let key = match node.drive_kind {
                Some(kind) => IconKey::Drive(kind),
                None => IconKey::Folder,
            };
            let tex = icons.texture(key);
            // Ícono + nombre como UNA zona clicable: clic en cualquiera de los dos
            // (no en el triángulo) navega el panel activo a esta carpeta.
            let img = ui.add(
                egui::Image::new(tex)
                    .fit_to_exact_size(egui::vec2(ICON_SIZE, ICON_SIZE))
                    .sense(egui::Sense::click()),
            );
            let label = ui.selectable_label(is_active, &node.name);
            if img.union(label).clicked() {
                actions.push(TreeAction::Navigate(node.path.clone()));
            }

            // Spinner mientras lista sus hijos.
            if node.state == NodeState::Loading {
                ui.spinner();
            }
```

NOTA: el triángulo (el bloque `if !matches!(node.state, Empty|Error) { let tri = ui.add(Label::new(glyph).sense(click)); if tri.clicked() {Expand/Collapse} }`) se mantiene IGUAL, antes de este bloque. Solo el ícono+nombre se unen. Así clic en el triángulo expande/colapsa (no navega) y clic en ícono/nombre navega.

- [ ] **Step 2: Compilar y verificar**

Run: `cargo build -p naygo-ui` → compila.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start: en el árbol, clic sobre el ícono de una carpeta (no solo el texto) navega; clic sobre el triángulo solo expande/colapsa; el nodo activo sigue resaltado.

NOTA: la fila del árbol no ocupa todo el ancho del panel (es ícono+nombre con indentación), así que "fila completa" en el árbol = la zona ícono+nombre, que es lo natural ahí (no hay celdas a la derecha como en la tabla). Esto cumple el pedido para el árbol.

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src/panes/tree_panel.rs
git commit -m "feat(ui): clic en ícono o nombre del árbol navega (no solo el texto)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Línea de inserción al reordenar columnas

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`

Objetivo: mientras se arrastra un encabezado, pintar una línea vertical azul en el borde izquierdo del encabezado bajo el cursor (dónde caería al soltar).

- [ ] **Step 1: (Opcional pero recomendado) función pura para el color/stroke — omitir; es trivial**

No se necesita función pura nueva; el cálculo del borde es `cell_resp.rect.left()`. Saltar a Step 2.

- [ ] **Step 2: Pintar la línea de drop en el header**

En `.header(HEADER_HEIGHT, |mut header| { for (ci, col) ... })`, ya se captura `cell_resp` (el `(Rect, Response)` de cada `header.col`). Añadir: si durante el arrastre el cursor está sobre este encabezado, pintar una `vline` en su borde izquierdo.

Dentro del `for`, tras obtener `(_, cell_resp)` del `header.col(...)` y antes de guardar `measured_widths[ci]`, añadir:
```rust
                // Indicador de drop: si se está arrastrando una columna y el cursor está
                // sobre este encabezado, pintar una línea de inserción azul en su borde
                // izquierdo ("caería antes de esta columna").
                if cell_resp.dnd_hover_payload::<usize>().is_some() && cell_resp.contains_pointer() {
                    let rect = cell_resp.rect;
                    let painter = ui.painter(); // `ui` del closure del header
                    painter.vline(
                        rect.left(),
                        rect.y_range(),
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(0x2f, 0x81, 0xf7)),
                    );
                }
```
PROBLEMA DE SCOPE: `cell_resp` se obtiene del `header.col(|ui| {...})` que devuelve `(Rect, Response)`, pero el `ui.painter()` que necesitas es el del CONTEXTO del header, no el `ui` interno de la celda. En egui_extras, tras `header.col(...)`, no tienes un `ui` del header directamente. SOLUCIÓN: usar `cell_resp.ctx` para pintar — `cell_resp` es un `egui::Response` que tiene `.ctx` (el `Context`). Pintar con una capa sobre el rect:
```rust
                if cell_resp.dnd_hover_payload::<usize>().is_some() && cell_resp.contains_pointer() {
                    let rect = cell_resp.rect;
                    let painter = cell_resp.ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::new(("drop_line", id.0)),
                    ));
                    painter.vline(
                        rect.left(),
                        rect.y_range(),
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(0x2f, 0x81, 0xf7)),
                    );
                }
```
VERIFICAR egui 0.34.3: `Response` tiene campo público `ctx: Context` (sí, en 0.34); `Context::layer_painter(LayerId) -> Painter` (sí); `egui::Order::Foreground`, `egui::LayerId::new(order, id)`, `Painter::vline`, `Rect::y_range() -> Rangef`. Adaptar si alguna firma difiere. Usar una capa Foreground asegura que la línea se vea sobre el header.

- [ ] **Step 3: Compilar y verificar**

Run: `cargo build -p naygo-ui` → compila. (Resolver el acceso al painter como arriba; si `cell_resp.ctx` no es público en 0.34, usar `ui.ctx()` capturándolo ANTES del `for` en una variable `let ctx = ui.ctx().clone();` — pero `ui` no está disponible dentro del closure del header; en ese caso, capturar `let ctx = header.... ` no aplica. La vía robusta: capturar el `Context` ANTES de `builder.header(...)`: el `show` tiene `ui` al inicio → `let ctx = ui.ctx().clone();` y usar `ctx.layer_painter(...)` dentro del header closure, que captura `ctx` por referencia/clone. Hacer eso: declarar `let ctx = ui.ctx().clone();` antes de `builder.header(...)` y usarlo en el bloque de la línea.)
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start: arrastrar un encabezado y pasar sobre otro muestra una línea azul vertical en el borde izquierdo del encabezado bajo el cursor; al soltar, la columna se mueve ahí.

NOTA: si por el orden de pintado la línea no aparece o parpadea, alternativa: pintar en `ui.painter()` del header con `Order::Foreground` no disponible — usar `cell_resp.ctx`/`ctx` layer painter (incluido). La detección `dnd_hover_payload::<usize>()` solo es `Some` durante un arrastre con payload usize (nuestro caso), así que la línea no aparece sin arrastre.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/panes/file_panel.rs
git commit -m "feat(ui): línea de inserción al arrastrar columnas (dónde caería al soltar)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Glifo de filtro ≡ + clic derecho abre el menú de columna

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs` (función `column_header`)

Dos cambios pequeños en `column_header`.

- [ ] **Step 1: Cambiar el glifo de filtro `⏷` por `≡`**

En `column_header`, el bloque del indicador de filtro:
```rust
        if table.filters.contains_key(&kind) {
            title.push(' ');
            title.push('⏷');
        }
```
cambiar el carácter a `≡`:
```rust
        if table.filters.contains_key(&kind) {
            title.push(' ');
            title.push('≡');
        }
```

- [ ] **Step 2: Abrir el menú también con clic derecho**

En `column_header`, hoy el `▾` (menu_button) abre el popup con clic izquierdo vía `egui::Popup::menu(&menu_button).id(popup_id).show(...)`. Para abrir el MISMO popup con clic derecho sobre el ENCABEZADO, detectar `secondary_clicked()` y abrir el popup por su id.

El encabezado se pinta dentro de `ui.horizontal(|ui| {...})`. Capturar el `Response` de ese `horizontal` y, si `secondary_clicked()`, abrir el popup. Reescribir `column_header` así (manteniendo lo demás):
```rust
fn column_header(
    ui: &mut egui::Ui,
    id: PaneId,
    kind: ColumnKind,
    table: &TableState,
    sort: SortSpec,
    ext_counts: &std::collections::BTreeMap<String, usize>,
    i18n: &naygo_core::i18n::I18n,
    actions: &mut Vec<TableAction>,
) {
    let popup_id = ui.make_persistent_id(("col_menu", id.0, kind));
    let header_resp = ui
        .horizontal(|ui| {
            let mut title = column_title(kind, i18n);
            if sort.key == naygo_core::columns::sort_key_of(kind) {
                title.push(' ');
                title.push(if sort.ascending { '▲' } else { '▼' });
            }
            if table.filters.contains_key(&kind) {
                title.push(' ');
                title.push('≡');
            }
            ui.label(egui::RichText::new(title).strong());

            let menu_button = ui.add(egui::Button::new("▾").frame(false));
            egui::Popup::menu(&menu_button).id(popup_id).show(|ui| {
                crate::column_menu::show_menu(ui, kind, table, sort, ext_counts, i18n, actions);
            });
        })
        .response;

    // Clic derecho sobre el encabezado abre el MISMO menú (el del ▾).
    if header_resp.secondary_clicked() {
        egui::Popup::open_id(ui.ctx(), popup_id);
    }
}
```
VERIFICAR egui 0.34.3: cómo abrir un Popup por id programáticamente. Opciones a comprobar en la fuente (`egui-0.34.3/src/containers/popup.rs` y `memory.rs`):
- `egui::Popup::open_id(ctx, id)` — si existe.
- o `ui.memory_mut(|m| m.open_popup(popup_id))` / `m.toggle_popup(popup_id)`.
- o el builder `Popup::menu(&resp)` ya soporta abrir con secondary click vía alguna opción.
USAR la que provea 0.34. Si `Popup::menu(&menu_button)` se ancla SOLO al botón y no se puede reabrir por id desde fuera, alternativa robusta: anclar el popup al `header_resp` completo en vez de al `menu_button`, y que `Popup::menu` se dispare tanto por el botón como por el secondary-click del header — investigar la API `Popup` de 0.34 (tiene métodos como `.open_memory(...)`/`PopupCloseBehavior`/gating por respuesta). El objetivo final: clic izquierdo en ▾ O clic derecho en el header → mismo menú. Implementar la vía que 0.34 permita limpiamente; documentar cuál se usó.

NOTA borrow: `actions` es `&mut` y se usa dentro del closure de `Popup::menu`. El `secondary_clicked` se evalúa DESPUÉS del `horizontal` (el closure ya terminó), así que no hay doble préstamo. Si abrir el popup por id requiere `actions` de nuevo, no es el caso (solo abre; el contenido se pinta en el frame siguiente por el `Popup::menu(...).show`). Verificar que compila.

- [ ] **Step 3: Compilar y verificar**

Run: `cargo build -p naygo-ui` → compila.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start: una columna con filtro activo muestra `≡` (no `⏷`); clic derecho sobre el encabezado abre el menú de columna (el mismo que el ▾); clic izquierdo en ▾ sigue funcionando.

- [ ] **Step 4: Commit**

```bash
git add crates/ui/src/panes/file_panel.rs
git commit -m "feat(ui): glifo de filtro ≡ y clic derecho abre el menú de columna

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: Actualizar el README**

Modify `README.md` — bloque de estado:
```markdown
> **Estado:** Pulido de UX de la Fase 2E en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-07-naygo-fase2e-pulido-ux-design.md`](docs/superpowers/specs/2026-06-07-naygo-fase2e-pulido-ux-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-07-naygo-fase2e-pulido-ux.md`](docs/superpowers/plans/2026-06-07-naygo-fase2e-pulido-ux.md).
> Fases 1, 2A, 2B, 2C-i, 2D, árbol de directorios y 2E (columnas Excel) completas.
```
(READ el bloque actual del README primero y reemplazarlo.)

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → verde.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → limpio.
Run: `cargo fmt --check` → limpio.
Run: `cargo build --release -p naygo-ui` → release compila.
App-start manual: repasar los 4 checklists (fila completa file panel + árbol; línea de drop; glifo ≡; clic derecho).

- [ ] **Step 3: Commit y push**

```bash
git add README.md
git commit -m "chore: actualizar estado del README (pulido de UX de 2E)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/fase2e-pulido-ux
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| #1 Fila completa clicable — file panel | 1 |
| #1 Fila completa clicable — árbol | 2 |
| #2 Línea de inserción al arrastrar columnas | 3 |
| #3 Glifo de filtro ⏷→≡ | 4 (Step 1) |
| #4 Clic derecho abre el menú de columna | 4 (Step 2) |
| core sin cambios | — (ninguna tarea toca core) |
| Validación manual (interacción/pintado) | todas |

**Notas de riesgo:**
- `TableRow::response()` (Tarea 1): confirmar que llamarlo tras los `row.col(...)` da el response de la fila completa y que `clicked()`/`double_clicked()` funcionan. Si egui_extras requiere `row.set_selected` u otra cosa para que el response cubra toda la fila, ajustar. (La fuente confirma que `response()` existe y cubre la fila.)
- Pintado de la línea de drop (Tarea 3): el acceso al painter es el punto delicado. Capturar `let ctx = ui.ctx().clone();` antes de `builder.header(...)` y usar `ctx.layer_painter(LayerId::new(Order::Foreground, ...))` dentro del header closure es la vía robusta. Verificar `Order`/`LayerId`/`vline`/`y_range` en 0.34.
- Abrir popup por clic derecho (Tarea 4): verificar la API de `Popup` 0.34 para abrir por id (`Popup::open_id` o `memory.open_popup/toggle_popup`). Si no hay forma limpia de abrir el mismo popup anclado al botón desde el secondary-click del header, anclar el popup al `header_resp` y dispararlo por ambos. Documentar la vía usada.
- Las 4 son UI; ningún test unitario nuevo salvo que surja una fn pura. No forzar tests sobre egui.
