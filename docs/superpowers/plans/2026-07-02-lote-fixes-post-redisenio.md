# Lote de fixes post-rediseño — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Arreglar 8 hallazgos post-rediseño — el crítico: copiar con Ctrl entre paneles del mismo disco movía los archivos (pérdida de datos) — más fecha+orden en los historiales, path-bar, metadata ampliada (audio/Office/PDF), y (con checkpoints de VM) el cierre-a-bandeja, el scroll bloqueado y el autostart directo a bandeja.

**Architecture:** El fix crítico transmite `MK_CONTROL` desde el drop OLE (platform) hasta la decisión ya-correcta `decide_drop_action` (core), que hoy recibe un Ctrl stale. La metadata reusa el sistema `core::metadata` existente con providers nuevos puro-Rust. Los bugs no reproducibles por lectura (cierre, scroll) se instrumentan con logs y se cierran con reproducción de Nicolás.

**Tech Stack:** Rust, Slint 1.16 (render por software), crate `windows` 0.62, `lofty` (audio), `zip`+`quick-xml` (Office), serde, i18n JSON 10 idiomas.

**Convenciones (leer antes de empezar):**
- PowerShell 5.1: NO `&&` ni `||`; usar `;` o líneas separadas. Compilar UI con `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint`.
- Header Naygo+SPDX en archivos NUEVOS (no tocar headers existentes):
  `// Naygo — <desc>` / `// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.` / `// SPDX-License-Identifier: MIT`
- Comentarios en español NEUTRAL (tú/impersonal, NUNCA voseo: nada de "podés/tenés/hacés/fijate/dale"). Código en inglés.
- i18n: texto visible SIEMPRE por clave, 10 idiomas. Test `todos_los_idiomas_tienen_las_claves_de_es` exige paridad. Tests que aseveran texto comparan contra la CLAVE (CI en inglés).
- GIT: solo `git add <rutas exactas>` + `git commit`. PROHIBIDO reset/restore/stash/checkout/clean, `add -A`, `add .`, `git rm`, push, `commit --amend`. NO tocar `CLAUDE.md` ni `favorites.rs`.
- Clippy con `-D warnings`. `cargo fmt` antes de cada commit.
- Cada commit termina con: `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`

---

## Fase 1 — Bug crítico: copiar con Ctrl movía los archivos

Contexto: `crates/core/src/dnd.rs` `decide_drop_action(ctrl, shift, same_drive)` YA es correcta
(Shift→Move, Ctrl→Copy, same_drive→Move, si no Copy). El bug es que el `ctrl` que le llega está
STALE: durante `DoDragDrop` la app no recibe teclado. El drop OLE hoy solo transmite Shift
(`move_` en `DropPayload`), nunca Ctrl. Fix: transmitir el Ctrl real del OLE (`MK_CONTROL`) y usarlo
en la decisión.

### Task 1: Transmitir `MK_CONTROL` desde el drop OLE

**Files:**
- Modify: `crates/platform/src/drop_target.rs` (imports ~120; `DropPayload` ~30-40; `effect_for` ~143; el `Drop` ~228)

- [ ] **Step 1: Añadir `copy_forced` al `DropPayload`**

En `crates/platform/src/drop_target.rs`, en `pub struct DropPayload` (tras `pub move_: bool,`):

```rust
    /// `true` si `MK_CONTROL` estaba activo al soltar (copiar SIEMPRE, aunque sea el mismo disco).
    /// El teclado de la app queda stale durante `DoDragDrop`, así que este flag —leído del
    /// `grfKeyState` que Windows entrega al SOLTAR— es la ÚNICA fuente fiable del Ctrl del usuario.
    pub copy_forced: bool,
```

- [ ] **Step 2: Importar `MK_CONTROL`**

En el `use` de modificadores (~línea 120, hoy `use windows::Win32::System::SystemServices::{MK_SHIFT, MODIFIERKEYS_FLAGS};`):

```rust
use windows::Win32::System::SystemServices::{MK_CONTROL, MK_SHIFT, MODIFIERKEYS_FLAGS};
```

- [ ] **Step 3: Leer Ctrl al soltar y poblar el payload**

En el método `Drop` del `IDropTarget` (~228, donde hoy hace `let move_ = (grfkeystate.0 & MK_SHIFT.0) != 0;`), añadir junto a `move_`:

```rust
        let copy_forced = (grfkeystate.0 & MK_CONTROL.0) != 0;
```

Y en la construcción del `DropPayload { ... }` que se envía por el canal, añadir el campo:

```rust
            copy_forced,
```

Busca en el archivo TODOS los sitios que construyen `DropPayload { ... }` (grep `DropPayload {`) y
añade `copy_forced` en cada uno (si hay más de uno, p. ej. un payload de prueba o vacío, poblarlo con
`false`).

- [ ] **Step 4: `effect_for` explícito para Ctrl (cursor coherente)**

En `effect_for(grfkeystate)` (~143), hoy: Shift→MOVE, si no→COPY. Dejar explícito que Ctrl fuerza
COPY (para que el cursor "+" sea coherente con lo que se ejecutará):

```rust
fn effect_for(grfkeystate: MODIFIERKEYS_FLAGS) -> DROPEFFECT {
    if (grfkeystate.0 & MK_CONTROL.0) != 0 {
        DROPEFFECT_COPY
    } else if (grfkeystate.0 & MK_SHIFT.0) != 0 {
        DROPEFFECT_MOVE
    } else {
        DROPEFFECT_COPY
    }
}
```

- [ ] **Step 5: Compilar platform**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-platform 2>&1 | tail -8`
Expected: compila. Si el build de `naygo-ui-slint` (que consume `DropPayload`) rompe por el campo
nuevo, se arregla en la Task 2 (ahí se pobla/lee en la UI).

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy -p naygo-platform --all-targets 2>&1 | tail -4`
Expected: sin warnings.

```bash
cargo fmt -p naygo-platform
git add crates/platform/src/drop_target.rs
git commit -m "$(cat <<'EOF'
fix(platform): transmitir MK_CONTROL en el drop OLE (copy_forced en DropPayload)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

### Task 2: Usar `copy_forced` en la decisión del drop (core + UI)

**Files:**
- Modify: `crates/core/src/dnd.rs` (nueva función que combina los flags; + tests)
- Modify: `crates/ui-slint/src/main.rs` (~1403: leer `payload.copy_forced`, pasarlo a `drop_at`)
- Modify: `crates/ui-slint/src/workspace_ctrl/ops.rs` (`drop_at`: firma + decisión ~286-288)

- [ ] **Step 1: Escribir el test de la capa de decisión del drop (falla)**

En `crates/core/src/dnd.rs`, en el módulo `#[cfg(test)]`, añadir tests para una función nueva
`decide_drop(move_hint, copy_forced, same_drive)` que combina las señales del OLE con la heurística
de disco. La REGLA: Shift-del-OLE (`move_hint`) → Move; Ctrl-del-OLE (`copy_forced`) → Copy; si
ninguno, cae en `same_drive` (Move) / distinto (Copy). Si ambos (Shift+Ctrl), Move gana (como
Windows y como `decide_drop_action`):

```rust
    #[test]
    fn decide_drop_ctrl_mismo_disco_copia() {
        // EL BUG: Ctrl + mismo disco DEBE copiar, no mover.
        assert_eq!(decide_drop(false, true, true), DropAction::Copy);
    }
    #[test]
    fn decide_drop_shift_copia_forzada_gana_move() {
        // Shift (move_hint) tiene prioridad sobre Ctrl (como Windows).
        assert_eq!(decide_drop(true, true, false), DropAction::Move);
    }
    #[test]
    fn decide_drop_sin_teclas_mismo_disco_mueve() {
        assert_eq!(decide_drop(false, false, true), DropAction::Move);
    }
    #[test]
    fn decide_drop_sin_teclas_distinto_disco_copia() {
        assert_eq!(decide_drop(false, false, false), DropAction::Copy);
    }
    #[test]
    fn decide_drop_shift_mueve_entre_discos() {
        assert_eq!(decide_drop(true, false, false), DropAction::Move);
    }
```

- [ ] **Step 2: Ejecutar para ver que falla**

Run: `cargo test -p naygo-core decide_drop 2>&1 | head -12`
Expected: FALLA a compilar (`decide_drop` no existe).

- [ ] **Step 3: Implementar `decide_drop`**

En `crates/core/src/dnd.rs`, junto a `decide_drop_action`:

```rust
/// Decide la acción de un drop combinando las señales FIABLES del OLE (`move_hint` = Shift,
/// `copy_forced` = Ctrl, ambas leídas del `grfKeyState` al soltar) con la heurística de disco.
/// Es la que debe usar el drop entre paneles: los flags de teclado de la app llegan stale durante
/// `DoDragDrop`, así que aquí NO se usan. Prioridad: Shift→Move, Ctrl→Copy, si no same_drive→Move.
pub fn decide_drop(move_hint: bool, copy_forced: bool, same_drive: bool) -> DropAction {
    if move_hint {
        DropAction::Move
    } else if copy_forced {
        DropAction::Copy
    } else if same_drive {
        DropAction::Move
    } else {
        DropAction::Copy
    }
}
```

- [ ] **Step 4: Ejecutar los tests**

Run: `cargo test -p naygo-core decide_drop 2>&1 | tail -8`
Expected: PASS (los 5).

- [ ] **Step 5: Cablear `copy_forced` en la UI (main.rs)**

En `crates/ui-slint/src/main.rs`, donde se recibe el drop OLE (~1403, hoy lee `payload.move_` y pasa
`ctrl_down`/`shift_down` stale a `drop_at`). Leer también `payload.copy_forced` y pasarlo a `drop_at`
(la firma de `drop_at` cambia en el Step 6). Ejemplo (adaptar al código real):

```rust
                routed = ctrl.borrow_mut().drop_at(
                    cx,
                    cy,
                    payload.move_,          // move_hint (Shift del OLE)
                    payload.copy_forced,    // copy_forced (Ctrl del OLE)
                    payload.paths.clone(),
                );
```

Es decir, `drop_at` deja de recibir los `ctrl_down`/`shift_down` stale y recibe las dos señales del
OLE. (Si `drop_at` se usa en OTRO sitio con datos no-OLE, ver Step 6.)

- [ ] **Step 6: Cambiar `drop_at` para usar `decide_drop` (ops.rs)**

En `crates/ui-slint/src/workspace_ctrl/ops.rs`, `drop_at`: cambiar la firma para recibir
`(move_hint: bool, copy_forced: bool, ...)` en vez de `(ctrl, shift, ..., move_hint)`, y la decisión
(~286-288) a:

```rust
        let same = naygo_core::dnd::same_drive(&paths[0], &dest_dir);
        let is_move = matches!(
            naygo_core::dnd::decide_drop(move_hint, copy_forced, same),
            naygo_core::dnd::DropAction::Move
        );
```

El `is_move` resultante ya alimenta la ejecución Y el `label`/registro del historial, así que el
texto del historial queda correcto sin más cambios. Verifica que no queden usos de las variables
`ctrl`/`shift` removidas (si `drop_at` las usaba para otra cosa, revisarlo; según el spec, para el
drop mandan las señales del OLE).

- [ ] **Step 7: Compilar todo + tests + clippy**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -10`
Expected: compila (el campo `copy_forced` de `DropPayload` ya existe por Task 1).
Run: `cargo test -p naygo-core dnd 2>&1 | grep "test result:"`
Expected: ok (incluye los antiguos de `decide_drop_action` + los nuevos de `decide_drop`).
Run: `cargo test -p naygo-ui-slint 2>&1 | grep "test result:"`
Expected: ok, 0 failed.
Run: `cargo clippy -p naygo-core -p naygo-ui-slint --all-targets 2>&1 | tail -6`
Expected: sin warnings.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all
git add crates/core/src/dnd.rs crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl/ops.rs
git commit -m "$(cat <<'EOF'
fix(dnd): copiar con Ctrl entre paneles del mismo disco ya no mueve (usa el Ctrl real del OLE)

El drop entre paneles decidía Copy/Move con el estado de teclado de la app, que queda
stale durante DoDragDrop; con Ctrl+arrastre en el mismo disco caía en la regla "mismo
disco = mover" y MOVÍA los archivos (pérdida de datos), además de rotular mal el historial.
Ahora la decisión usa move_hint (Shift) y copy_forced (Ctrl) leídos del grfKeyState del OLE
al soltar, vía la nueva decide_drop. El label del historial se corrige solo (sale del mismo
is_move).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

> **CHECKPOINT VM (Fase 1)**: por ser pérdida de datos, Nicolás verifica en la VM que Ctrl+arrastre
> en el MISMO disco COPIA (no mueve) y que el historial dice "Copiar", antes de continuar. El
> controlador anota esto y sigue con las demás fases (no bloquea la implementación del resto, pero sí
> el merge).

---

## Fase 2 — Historial: fecha del registro + orden (nuevos arriba)

Hay dos historiales. El de ACCIONES (undo) YA guarda el timestamp (`UndoEntry.when_epoch_secs`) y
`HistRow.when` lo lleva, pero lo formatea CRUDO (`format!("{}", e.when_epoch_secs)` en
`bridge.rs:517`). El de OPERACIONES (`OpRowVm kind==2`) no tiene fecha.

### Task 3: Formatear la fecha del historial de acciones + nuevos arriba

**Files:**
- Modify: `crates/ui-slint/src/bridge.rs` (`history_rows` ~499-524: formatear `when`; invertir orden)
- Modify: `crates/core/src/format.rs` si hace falta un helper (ya existe `format_time`)

Contexto: `format_time(local_epoch_secs: Option<i64>, fmt: DateFormat) -> String` (`format.rs:121`)
formatea epoch LOCAL según el `DateFormat` de la config. `UndoEntry.when_epoch_secs` es epoch (revisar
si ya es local o UTC — si es UTC, aplicar el offset local como hace el resto; si `when_epoch_secs` ya
viene local, pasarlo directo). `history_rows` no recibe hoy el `DateFormat`; habrá que pasárselo.

- [ ] **Step 1: Test de que `when` se formatea como fecha, no como número crudo**

En `crates/ui-slint/src/bridge.rs`, módulo `#[cfg(test)]`, añadir un test que arme un `UndoEntry` con
un `when_epoch_secs` conocido y verifique que `history_rows(...)` produce un `when` con formato de
fecha (contiene `-` y `:`), no el número crudo. Ajustar a la firma nueva (con `DateFormat`):

```rust
    #[test]
    fn history_when_se_formatea_como_fecha() {
        let e = UndoEntry {
            id: 1,
            label: "Copiar".into(),
            when_epoch_secs: 1_700_000_000, // un instante conocido
            actions: vec![],
            undone: false,
        };
        // Firma nueva: history_rows recibe el DateFormat.
        let rows = history_rows(&[e], naygo_core::format::DateFormat::IsoMinute);
        assert!(
            rows[0].when.contains('-') && rows[0].when.contains(':'),
            "when debe ser una fecha legible, no el epoch crudo: {}",
            rows[0].when
        );
    }
```

(Ajustar los campos de `UndoEntry` a su definición real — leerla antes.)

- [ ] **Step 2: Ejecutar (falla a compilar por la firma)**

Run: `cargo test -p naygo-ui-slint history_when 2>&1 | head -12`
Expected: FALLA (la firma de `history_rows` no acepta `DateFormat` aún).

- [ ] **Step 3: Cambiar `history_rows` para formatear y recibir el `DateFormat`**

En `bridge.rs`, cambiar `pub fn history_rows(entries: &[UndoEntry]) -> Vec<HistRow>` a
`pub fn history_rows(entries: &[UndoEntry], date_fmt: naygo_core::format::DateFormat) -> Vec<HistRow>`,
y la línea `when: format!("{}", e.when_epoch_secs),` por:

```rust
                when: naygo_core::format::format_time(Some(e.when_epoch_secs), date_fmt),
```

(Si `when_epoch_secs` es UTC y `format_time` espera local, aplicar el offset local que use el resto
del proyecto — buscar cómo la columna "Modificado" obtiene el epoch local; reutilizar ese camino.)

Actualizar el/los llamador(es) de `history_rows` (en `bridge.rs` mismo hay un método `history_rows`
del ctrl, y en `main.rs:487` `c.history_rows()`): pasar el `DateFormat` de la config
(`config.settings.date_format` o como se llame — buscar cómo lo lee la columna Modificado).

- [ ] **Step 4: Nuevos arriba (invertir presentación)**

En `history_rows`, invertir el orden de las filas producidas para que la más RECIENTE quede primero.
`undo_history` se llena con `push` (la última acción al final), así que:

```rust
    entries
        .iter()
        .rev()  // más reciente primero (sin alterar el Vec del undo)
        .map(|e| { ... })
        .collect()
```

IMPORTANTE: invertir SOLO aquí (capa de presentación). NO tocar `undo_history` ni el orden que usa
Ctrl+Z. Si hay un test que asuma el orden viejo de `history_rows`, actualizarlo.

- [ ] **Step 5: Tests + clippy**

Run: `cargo test -p naygo-ui-slint history 2>&1 | grep "test result:"`
Expected: ok, 0 failed.
Run: `cargo clippy -p naygo-ui-slint --all-targets 2>&1 | tail -4`
Expected: sin warnings.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/src/bridge.rs crates/ui-slint/src/main.rs
git commit -m "$(cat <<'EOF'
feat(ui): historial de acciones con fecha legible y los registros más nuevos arriba

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

### Task 4: Fecha + nuevos-arriba en el panel de operaciones

**Files:**
- Modify: `crates/ui-slint/ui/types.slint` (`OpRowVm`: campo `when`)
- Modify: `crates/ui-slint/src/ops_ctrl.rs` (`ActiveOp`: timestamp de fin; `OpRowData`: campo; construcción ~1516; orden del historial)
- Modify: `crates/ui-slint/ui/ops-panel.slint` (`HistoryRow`: mostrar la fecha)
- Modify: `crates/ui-slint/src/main.rs` (mapeo `OpRowData`→`OpRowVm`)

Contexto: `ActiveOp` (ops_ctrl.rs:109) usa `started_at: Instant` (monótono, NO sirve para fecha).
Para la fecha de fin se necesita wall-clock. `OpSummary` (`summary: Option<OpSummary>`) marca la op
terminada. Al pasar a terminada, capturar `SystemTime::now()` → epoch i64.

- [ ] **Step 1: Añadir el timestamp de fin a `ActiveOp` y poblarlo al terminar**

En `ActiveOp`, añadir `pub finished_epoch_secs: Option<i64>` (default `None` en los inits). Donde la
op pasa a terminada (se setea `summary = Some(...)` — buscar los sitios), setear:

```rust
                op.finished_epoch_secs = Some(naygo_core::now_epoch_secs());
```

Si no existe un helper `now_epoch_secs`, crearlo en `crates/core/src/lib.rs` o en `format.rs`:

```rust
/// Segundos desde epoch UTC del instante actual (wall-clock). Para timestamps de historial.
pub fn now_epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
```

(Si el proyecto ya tiene un helper equivalente —buscar `UNIX_EPOCH` en core— reutilizarlo.)

- [ ] **Step 2: Campo `when` en `OpRowData` y `OpRowVm`**

En `ops_ctrl.rs` `pub struct OpRowData`, añadir `pub when: String` (fecha ya formateada, vacía si la
op no es de historial). En `types.slint` `OpRowVm`, añadir `when: string,`.

- [ ] **Step 3: Poblar `when` en la construcción de filas**

En el literal `OpRowData { ... }` (~1516), poblar `when` solo para las filas de historial (`kind==2`):
formatear `o.finished_epoch_secs` con `format_time` y el `DateFormat` de la config; para las no-historial,
`String::new()`:

```rust
                    when: if kind == 2 {
                        naygo_core::format::format_time(o.finished_epoch_secs, self.config.settings.date_format)
                    } else {
                        String::new()
                    },
```

(Ajustar `self.config.settings.date_format` al nombre real; aplicar offset local si el epoch es UTC,
igual que la Task 3.)

- [ ] **Step 4: Nuevos arriba en el historial del panel de operaciones**

Las filas de historial (`kind==2`) deben salir con la más reciente primero. En la función que arma
las filas (`ops_ctrl.rs`, la que itera `active_ops`), invertir SOLO el tramo de historial. Si todas
las filas se generan en una pasada, separar: recolectar las `kind==2` y `.rev()`-erlas (o ordenarlas
por `finished_epoch_secs` desc) antes de emitirlas, dejando en-curso/cola en su orden. Documentar en
un comentario que solo se invierte la presentación del historial.

- [ ] **Step 5: Mapear `when` en main.rs**

En `main.rs`, en `to_op_row_vm` (~6322, donde se mapea `OpRowData`→`OpRowVm`), añadir
`when: d.when.into(),` (o el binding real).

- [ ] **Step 6: Mostrar la fecha en `HistoryRow` (ops-panel.slint)**

En `crates/ui-slint/ui/ops-panel.slint`, `HistoryRow`, añadir un `Text` con `root.op.when` (discreto:
`Theme.text-dim`, `font-size: 11px`), alineado a la derecha de la primera línea (junto al `status`) o
bajo la etiqueta. No romper el alto adaptable (28/44px) — si hace falta, la fecha va inline en la
primera línea, no en una nueva.

- [ ] **Step 7: Compilar + tests + clippy**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -10`
Expected: compila.
Run: `cargo test -p naygo-core -p naygo-ui-slint 2>&1 | grep "test result:"`
Expected: ok, 0 failed.
Run: `cargo clippy -p naygo-core -p naygo-ui-slint --all-targets 2>&1 | tail -6`
Expected: sin warnings.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all
git add crates/core/src/lib.rs crates/core/src/format.rs crates/ui-slint/ui/types.slint crates/ui-slint/src/ops_ctrl.rs crates/ui-slint/ui/ops-panel.slint crates/ui-slint/src/main.rs
git commit -m "$(cat <<'EOF'
feat(ui): fecha del registro y orden nuevos-arriba en el historial del panel de operaciones

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

(ajustar `git add` a los archivos realmente tocados; quitar `lib.rs`/`format.rs` si el helper ya existía.)

---

## Fase 3 — Path-bar: seleccionar todo + alinear a la izquierda

### Task 5: Editar el path-bar con texto seleccionado y a la izquierda

**Files:**
- Modify: `crates/ui-slint/ui/file-panel.slint` (`path-edit` LineEdit ~507-528)

- [ ] **Step 1: Seleccionar todo al enfocar**

En el `LineEdit` de edición del path (`file-panel.slint:512`, hoy `init => { self.focus(); }`),
cambiar a (patrón idéntico al rename inline de la línea 672):

```slint
        init => { self.focus(); self.select-all(); }
```

- [ ] **Step 2: Alinear a la izquierda**

Envolver el `LineEdit` de edición en un `HorizontalLayout` (como los breadcrumbs, línea 385) para que
no lo centre el estilo por defecto, y darle `horizontal-stretch: 1`:

```slint
            if root.editing: HorizontalLayout {
                padding-left: 4px;
                path-edit := LineEdit {
                    horizontal-stretch: 1;
                    text: root.edit-text;
                    placeholder-text: "C:\\…";
                    init => { self.focus(); self.select-all(); }
                    edited(t) => { root.path-edit-changed(root.pane-id, t); }
                    accepted(t) => { root.path-edit-commit(root.pane-id, t); }
                    changed has-focus => { if (!self.has-focus) { root.path-edit-cancel(root.pane-id); } }
                    key-pressed(ev) => {
                        if (ev.text == Key.Escape) { root.path-edit-cancel(root.pane-id); return accept; }
                        return reject;
                    }
                }
            }
```

(Conservar EXACTAMENTE los callbacks actuales del LineEdit —edited/accepted/changed has-focus/
key-pressed— para no romper commit/cancel. Leer el bloque real antes de reemplazar; si difiere,
adaptar. Si `LineEdit` soporta `horizontal-alignment` directo en esta versión y basta con eso, usarlo
en vez del wrapper — pero el wrapper es lo verificado como equivalente a los breadcrumbs.)

- [ ] **Step 3: Compilar**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -8`
Expected: compila.

- [ ] **Step 4: Tests + clippy + commit**

Run: `cargo test -p naygo-ui-slint 2>&1 | grep "test result:"`
Expected: ok, 0 failed.
Run: `cargo clippy -p naygo-ui-slint --all-targets 2>&1 | tail -4`
Expected: sin warnings.

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/ui/file-panel.slint
git commit -m "$(cat <<'EOF'
feat(ui): al editar la ruta a mano, seleccionar todo el texto y alinearlo a la izquierda

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 4 — Metadata ampliada (audio, Office moderno, PDF)

Sobre `core::metadata` (trait `MetadataProvider`, registro `ensure_core_providers`, `metadata_for`).
Todos los providers van en core y se registran en `ensure_core_providers`.

### Task 6: Provider de audio (lofty)

**Files:**
- Create: `crates/core/src/metadata/audio_meta.rs`
- Modify: `crates/core/Cargo.toml` (dep `lofty`)
- Modify: `crates/core/src/metadata/mod.rs` (`pub mod audio_meta;`, registrar en `ensure_core_providers`)
- Modify: `crates/core/src/i18n/*.json` (10) + `THIRD-PARTY-NOTICES.md`

- [ ] **Step 1: Añadir `lofty` a core**

En `crates/core/Cargo.toml`, sección `[dependencies]`, añadir (verificar la última versión estable y
que sea pura-Rust MIT/Apache; a la fecha `lofty = "0.21"` o similar):

```toml
# Metadata de audio (mp3/flac/ogg/m4a/wav): duración, bitrate, tags. Pura-Rust, MIT/Apache.
lofty = "0.21"
```

Run: `cargo build -p naygo-core 2>&1 | tail -5` para confirmar que compila y NO arrastra deps nativas
(si `cargo build` intenta compilar C o pide un compilador de C, DETENERSE: violaría la política; en
ese caso probar una versión distinta o reportar BLOCKED).

- [ ] **Step 2: Test del provider (falla)**

En `audio_meta.rs`, módulo `#[cfg(test)]` (o generar un WAV mínimo con `lofty`/`hound` si está;
alternativamente incluir un fixture binario pequeño). Test tolerante:

```rust
    #[test]
    fn audio_inexistente_da_vacio() {
        let fields = AudioMeta.read(std::path::Path::new(r"C:\no\existe.mp3"));
        assert!(fields.is_empty());
    }
    // Si se puede generar un archivo de audio mínimo en el test, aseverar que reporta
    // al menos meta.duration con formato válido; si no, dejar solo el test de "no panic + vacío".
```

- [ ] **Step 3: Implementar `AudioMeta`**

`crates/core/src/metadata/audio_meta.rs` (header Naygo). Usar la API de `lofty` para leer solo la
cabecera (`lofty::read_from_path` o `Probe`). Extraer, cuando existan, y OMITIR los vacíos:
- técnicos: `meta.duration` (HH:MM:SS), `meta.bitrate` ("N kbps"), `meta.sample_rate` ("N Hz"),
  `meta.channels` ("N");
- tags (si hay): `meta.artist`, `meta.title`, `meta.album`, `meta.year`.
`extensions()` → `&["mp3", "flac", "ogg", "m4a", "wav"]`. `read` tolerante: cualquier error → `Vec::new()`.
Formatear la duración con un helper simple (segundos → "H:MM:SS" o "M:SS").

- [ ] **Step 4: Registrar + claves i18n**

En `metadata/mod.rs`: `pub mod audio_meta;` y en `ensure_core_providers`:
`register_provider(Box::new(audio_meta::AudioMeta));`.
Añadir a los 10 JSON las claves: `meta.duration, meta.bitrate, meta.sample_rate, meta.channels,
meta.artist, meta.title, meta.album, meta.year` (ES/EN + traducir a los 8). Estas se resuelven con
`config.t` en la UI (no necesitan property de `Tr`, igual que `meta.dimensions`).

- [ ] **Step 5: THIRD-PARTY-NOTICES + tests + clippy + commit**

Actualizar `THIRD-PARTY-NOTICES.md` con `lofty` (regenerar con `cargo license` si el proyecto lo usa,
o añadir la entrada a mano).
Run: `cargo test -p naygo-core audio 2>&1 | grep "test result:"` → ok.
Run: `cargo test -p naygo-core i18n 2>&1 | grep "test result:"` → ok (paridad).
Run: `cargo clippy -p naygo-core --all-targets 2>&1 | tail -4` → sin warnings.

```bash
cargo fmt -p naygo-core
git add crates/core/Cargo.toml crates/core/src/metadata/audio_meta.rs crates/core/src/metadata/mod.rs crates/core/src/i18n THIRD-PARTY-NOTICES.md
git commit -m "$(cat <<'EOF'
feat(core): metadata de audio (mp3/flac/ogg/m4a/wav) vía lofty — duración, bitrate, tags

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

### Task 7: Provider de Office moderno (docx/xlsx/pptx)

**Files:**
- Create: `crates/core/src/metadata/office_meta.rs`
- Modify: `crates/core/Cargo.toml` (asegurar `quick-xml` como dep explícita si no lo está)
- Modify: `crates/core/src/metadata/mod.rs` (registrar)
- Modify: `crates/core/src/i18n/*.json` (10)

- [ ] **Step 1: Asegurar `quick-xml` como dep explícita**

`quick-xml` ya es transitiva. Añadirla explícita a `crates/core/Cargo.toml` (`quick-xml = "0.39"` o la
versión ya presente en Cargo.lock) para poder usarla. `zip` ya está.

- [ ] **Step 2: Test (falla) con un docx mínimo armado en el test**

En `office_meta.rs`, `#[cfg(test)]`: crear en un tempdir un .docx mínimo = un ZIP con una entrada
`docProps/core.xml` con `<dc:creator>` y `<dc:title>` conocidos (usar la crate `zip` para escribirlo).
Aseverar que `OfficeMeta.read(path)` devuelve `meta.author` y `meta.title` con esos valores. Más un
test de "archivo no-zip → vacío, sin panic".

- [ ] **Step 3: Implementar `OfficeMeta`**

`office_meta.rs` (header Naygo). `extensions()` → `&["docx", "xlsx", "pptx"]`. `read`:
1. Abrir el path con `zip::ZipArchive`. Si falla → `Vec::new()`.
2. Leer `docProps/core.xml` (si existe): parsear con `quick-xml` los nodos `dc:creator` (→
   `meta.author`), `dc:title` (→ `meta.title`), `dcterms:modified` (→ `meta.modified`, formatear la
   fecha ISO del XML a algo legible o dejarla tal cual si ya es legible).
3. Leer `docProps/app.xml` (si existe) para el contador propio del tipo: por extensión, `Words` →
   `meta.word_count` (docx), `Sheets`/contar hojas → `meta.sheet_count` (xlsx), `Slides` →
   `meta.slide_count` (pptx).
4. Omitir los campos vacíos. Tolerante: cualquier fallo de parseo → lo que se haya podido leer (o vacío).

Mantener el parseo XML simple y defensivo (buscar los tags puntuales; no construir un modelo completo).

- [ ] **Step 4: Registrar + i18n**

`pub mod office_meta;` + `register_provider(Box::new(office_meta::OfficeMeta));` en
`ensure_core_providers`. Claves nuevas en los 10 JSON: `meta.author, meta.modified, meta.word_count,
meta.sheet_count, meta.slide_count` (ES/EN + traducir; `meta.title` ya existe de la Task 6).

- [ ] **Step 5: Tests + clippy + commit**

Run: `cargo test -p naygo-core office 2>&1 | grep "test result:"` → ok.
Run: `cargo test -p naygo-core i18n 2>&1 | grep "test result:"` → ok.
Run: `cargo clippy -p naygo-core --all-targets 2>&1 | tail -4` → sin warnings.

```bash
cargo fmt -p naygo-core
git add crates/core/Cargo.toml crates/core/src/metadata/office_meta.rs crates/core/src/metadata/mod.rs crates/core/src/i18n
git commit -m "$(cat <<'EOF'
feat(core): metadata de Office moderno (docx/xlsx/pptx) leyendo docProps del zip

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

### Task 8: Provider de PDF (con contingencia)

**Files:**
- Create: `crates/core/src/metadata/pdf_meta.rs` (si la crate cumple la política)
- Modify: `crates/core/Cargo.toml`, `metadata/mod.rs`, `i18n/*.json`, `THIRD-PARTY-NOTICES.md`

- [ ] **Step 1: Evaluar la crate PDF**

Probar `lopdf` (MIT): añadir `lopdf = "0.34"` (o la estable) a `crates/core/Cargo.toml` y
`cargo build -p naygo-core`. Verificar que es puro-Rust y NO arrastra deps nativas/C. Si `lopdf` no
cumple (deps nativas, peso excesivo, o no compila limpio), probar `pdf` (crate alternativa) una vez.
Si NINGUNA crate puro-Rust aceptable existe → **DIFERIR PDF**: revertir el cambio de Cargo.toml, NO
crear el provider, y saltar al Step 5 dejando constancia en el reporte (esta task se cierra como
"PDF diferido" sin bloquear el lote).

- [ ] **Step 2: Test (falla)**

En `pdf_meta.rs`, `#[cfg(test)]`: test tolerante de "pdf inexistente → vacío, sin panic". Si se puede
generar/incluir un PDF mínimo con `/Info` (autor/título) y páginas, aseverar `meta.pages` y `meta.author`.

- [ ] **Step 3: Implementar `PdfMeta`**

`pdf_meta.rs` (header Naygo). `extensions()` → `&["pdf"]`. `read`: cargar con la crate elegida; extraer
número de páginas (`meta.pages`), y del diccionario `/Info`: `Author` (→ `meta.author`), `Title` (→
`meta.title`). Omitir vacíos. Tolerante: cualquier error → `Vec::new()`.

- [ ] **Step 4: Registrar + i18n + notices**

`pub mod pdf_meta;` + registrar. Clave nueva `meta.pages` en los 10 JSON (ES "Páginas" / EN "Pages" +
traducir; `meta.author`/`meta.title` ya existen). Actualizar `THIRD-PARTY-NOTICES.md` con la crate PDF.

- [ ] **Step 5: Tests + clippy + commit (o commit de "PDF diferido")**

Si se implementó:
Run: `cargo test -p naygo-core pdf 2>&1 | grep "test result:"` → ok. i18n paridad ok. clippy limpio.
```bash
cargo fmt -p naygo-core
git add crates/core/Cargo.toml crates/core/src/metadata/pdf_meta.rs crates/core/src/metadata/mod.rs crates/core/src/i18n THIRD-PARTY-NOTICES.md
git commit -m "$(cat <<'EOF'
feat(core): metadata de PDF (páginas, autor, título) vía <crate>

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```
Si se difirió: no hay commit de código; el controlador anota "PDF diferido, sin crate puro-Rust
aceptable" y sigue.

---

## Fase 5 — Diagnóstico dirigido (logging) de cerrar-mata-proceso y scroll

> Estas tasks NO arreglan a ciegas: instrumentan, Nicolás reproduce en la VM, y con los logs se
> cierra la causa. Cada una tiene un CHECKPOINT de reproducción.

### Task 9: Instrumentar el flujo de cierre / creación del tray

**Files:**
- Modify: `crates/ui-slint/src/tray.rs` (loguear éxito/error de la creación del tray)
- Modify: `crates/ui-slint/src/main.rs` (loguear `tray_active`, la rama de `on_close_requested`)

Contexto: el logging del proyecto va a `naygo.log` (buscar `logging::` / `breadcrumb` / `tracing`).
Usar el mismo mecanismo.

- [ ] **Step 1: Loguear la creación del tray**

En `crates/ui-slint/src/tray.rs`, donde se crea el `TrayIcon` (~39-113), envolver el `Result` de
creación: loguear "tray creado OK" en el camino feliz y el error concreto (`{e}`) si falla (icono
ilegible, `Shell_NotifyIcon`, etc.). Que NINGÚN error quede silenciado.

- [ ] **Step 2: Loguear `tray_active` y la decisión de cierre**

En `main.rs`, tras calcular `tray_active` (~1217-1230), loguear su valor y `tray_enabled`/
`close_to_tray`. En `on_close_requested` (~5754), loguear qué rama se toma
(`should_quit_on_close` → quit vs HideWindow).

- [ ] **Step 3: Compilar + commit del build instrumentado**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -5` → compila.
```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/src/tray.rs crates/ui-slint/src/main.rs
git commit -m "$(cat <<'EOF'
chore(ui): instrumentar la creación del tray y la decisión de cierre (diagnóstico)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 4: CHECKPOINT — regenerar dist y pedir reproducción**

El controlador regenera dist (`build-release.ps1`) y pide a Nicolás: cerrar la ventana en la VM y
compartir `naygo.log`. Con el log se determina si el tray falla al crearse (y por qué) o si la rama
de cierre es la incorrecta. El FIX real se hace en la Task 11 (según evidencia).

### Task 10: Instrumentar el cálculo del scroll del file-panel

**Files:**
- Modify: `crates/ui-slint/ui/file-panel.slint` (exponer/loguear el estado del scroll)
- Modify: `crates/ui-slint/src/main.rs` o `workspace_ctrl` (recibir el log desde Slint)

Contexto: el scroll usa `wheel-scroll` acotado a `[0, rows.length*row-h - self.height]`
(`file-panel.slint:855`), con `self` = `body-touch`, que se DESMONTA si
`rename-pos != -1 || modal-open || drag-in-progress` (línea 721).

- [ ] **Step 1: Exponer un callback de diagnóstico del scroll**

Como Slint no puede escribir el log directamente, añadir un `callback debug-scroll(int, length, length, length)`
(pane-id, rows-height-total, visible-height, max-scroll) que el `scroll-event` invoque UNA vez (o con
throttle) con los valores. En Rust, conectarlo a un log. Alternativamente, más simple: añadir un
`Text` temporal (solo en este build) en una esquina del panel mostrando
`rows.length + " / vis=" + self.height + " / max=" + (rows.length*row-h - self.height)` y los flags
`rename-pos/modal-open/drag-in-progress`, para que Nicolás lo vea en pantalla y mande captura. Elegir
la vía más rápida de leer (el `Text` en pantalla es lo más directo).

- [ ] **Step 2: Compilar + commit**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -5` → compila.
```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/ui/file-panel.slint crates/ui-slint/src/main.rs
git commit -m "$(cat <<'EOF'
chore(ui): instrumentar el estado del scroll del file-panel (diagnóstico)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 3: CHECKPOINT — reproducción**

Regenerar dist; Nicolás abre el panel de 469 archivos y comparte lo que muestra el diagnóstico
(rows/visible/max/flags). Con eso se identifica la causa (body-touch desmontado, alto mal calculado,
modo deep, etc.). El FIX se hace en la Task 11.

### Task 11: Fix de cierre-a-bandeja y scroll según la evidencia

**Files:** a determinar según los logs (probablemente `tray.rs`/`main.rs` para el cierre, y
`file-panel.slint` para el scroll).

- [ ] **Step 1: Aplicar el fix del cierre**

Según el log de la Task 9. Casos típicos: (a) el tray falla al crearse → arreglar la causa (icono,
API) o hacer que `close_to_tray=true` NO cierre aunque el tray falle (mantener la ventana viva
oculta, o avisar); (b) la rama de decisión es incorrecta → corregir `should_quit_on_close`. Añadir un
test de la lógica de decisión si aplica.

- [ ] **Step 2: Aplicar el fix del scroll**

Según el diagnóstico de la Task 10. Casos típicos: (a) un flag pegado desmonta el `body-touch` →
limpiarlo; (b) usar `list.visible-height` en vez de `self.height` en el clamp; (c) alto del contenedor
mal → dar `vertical-stretch: 1` o corregir el layout; (d) modo deep con `rows` vacías → usar la fuente
correcta de filas.

- [ ] **Step 3: Quitar la instrumentación temporal**

Remover el `Text`/callback de diagnóstico del scroll (Task 10) y reducir los logs de la Task 9 a lo
razonable (dejar un log útil de la creación del tray, quitar el ruido).

- [ ] **Step 4: Compilar + tests + clippy + commit**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -8` → compila.
Run: `cargo test -p naygo-ui-slint 2>&1 | grep "test result:"` → ok.
Run: `cargo clippy -p naygo-ui-slint --all-targets 2>&1 | tail -4` → sin warnings.
```bash
cargo fmt -p naygo-ui-slint
git add <archivos del fix>
git commit -m "$(cat <<'EOF'
fix(ui): <cerrar a bandeja> y <scroll del panel> según el diagnóstico de la VM

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 6 — Autostart directo a la bandeja (tras Fase 5a)

### Task 12: Persistir "ventana abierta al cerrar" + arrancar en bandeja sin flash/botón

**Files:**
- Modify: `crates/core/src/config/mod.rs` o el estado de sesión (flag `window_was_open_on_exit`)
- Modify: `crates/platform/src/window.rs` (Win32: quitar botón de barra de tareas / ocultar)
- Modify: `crates/ui-slint/src/main.rs` (arranque `--tray`: lógica mostrar-vs-ocultar; aplicar Win32)

Contexto: hoy `--tray` hace `set_minimized(true)` en el primer `on_wake` (`main.rs:1760`), lo que deja
botón en la barra de tareas y muestra un flash. Se quiere: arrancar oculto en la bandeja, salvo que se
haya cerrado con la ventana abierta.

- [ ] **Step 1: Test de la lógica pura mostrar-vs-ocultar**

Función pura testeable, p. ej. en `main.rs` o un módulo: `fn should_show_on_start(tray_flag: bool,
window_was_open: bool) -> bool` → mostrar si NO es arranque-tray, o si es tray pero la ventana estaba
abierta al cerrar. Test:

```rust
    #[test]
    fn arranque_tray_oculta_salvo_ventana_abierta() {
        assert!(!should_show_on_start(true, false)); // tray + estaba en bandeja → oculto
        assert!(should_show_on_start(true, true));   // tray + estaba abierta → mostrar
        assert!(should_show_on_start(false, false)); // arranque normal → mostrar
    }
```

- [ ] **Step 2: Implementar la función + persistir el flag**

Implementar `should_show_on_start`. Persistir `window_was_open_on_exit` al cerrar (en la sesión/config
que ya se guarda en `on_close_requested`, `main.rs:5754`): `true` si la ventana estaba visible (no en
bandeja) al cerrar; `false` si estaba oculta en bandeja. Leerlo al arrancar.

- [ ] **Step 3: Win32 para ocultar de la barra de tareas**

En `crates/platform/src/window.rs`, añadir una función que, dado el HWND, quite el botón de la barra
de tareas — vía `ITaskbarList::DeleteTab` (requiere COM) o alternando el estilo extendido
`WS_EX_TOOLWINDOW` (más simple: `SetWindowLongPtrW(GWL_EXSTYLE, ... | WS_EX_TOOLWINDOW)` y ocultar).
Elegir la vía que ya encaje con lo que el crate `windows` expone en el proyecto (buscar usos de
`SetWindowLongPtrW`/`GWL_EXSTYLE`/`ShowWindow` existentes). La función debe ser reversible (al abrir
desde la bandeja, restaurar el botón). Verificar la feature del crate `windows` necesaria y añadirla si
falta.

- [ ] **Step 4: Cablear en el arranque**

En `main.rs`, en el arranque `--tray`: si `should_show_on_start(cli.tray, window_was_open)` es `false`,
aplicar la función Win32 para ocultar de la barra de tareas y NO mostrar la ventana (mantenerla oculta,
sin `set_minimized`), dejando la app viva por el tray. Si es `true`, restaurar/mostrar normal. Al abrir
desde el ícono de bandeja, restaurar el botón de la barra de tareas y mostrar.

- [ ] **Step 5: Compilar + tests + clippy + commit**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-platform -p naygo-ui-slint 2>&1 | tail -10` → compila.
Run: `cargo test -p naygo-ui-slint should_show_on_start 2>&1 | grep "test result:"` → ok.
Run: `cargo clippy -p naygo-platform -p naygo-ui-slint --all-targets 2>&1 | tail -4` → sin warnings.
```bash
cargo fmt --all
git add crates/core/src/config/mod.rs crates/platform/src/window.rs crates/platform/Cargo.toml crates/ui-slint/src/main.rs
git commit -m "$(cat <<'EOF'
feat: autostart arranca directo a la bandeja (sin ventana ni botón de barra), salvo si se cerró con la ventana abierta

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 6: CHECKPOINT VM**

Regenerar dist; Nicolás verifica en la VM: (a) con autostart, Naygo arranca sin ventana ni botón de
barra de tareas (solo el ícono de bandeja); (b) al abrir desde la bandeja, la ventana aparece con su
botón; (c) si cierra con la ventana abierta y reinicia, la ventana se restaura.

---

## Fase 7 — Verificación integral y dist

### Task 13: Suite completa + clippy estricto + dist

- [ ] **Step 1: Suite completa**

Run: `cargo test --workspace 2>&1 | grep -E "test result:|FAILED"`
Expected: todos ok, 0 failed.

- [ ] **Step 2: Clippy estricto (CI)**

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -E "warning:|error" | head`
Expected: vacío.

- [ ] **Step 3: fmt check**

Run: `cargo fmt --all -- --check`
Expected: sin diffs.

- [ ] **Step 4: Regenerar dist**

Run: `powershell -File scripts\build-release.ps1`
Expected: portable + instalador en `dist/`, sin warnings de idioma.

- [ ] **Step 5: Verificación visual pendiente (VM)**

Anotar para Nicolás el set de verificación: bug de copiar (Ctrl copia, no mueve), fechas y orden en
ambos historiales, path-bar (texto seleccionado e izquierda), metadata de mp3/docx/xlsx/pptx/pdf en
Propiedades y Preview, cierre a bandeja, scroll del panel grande, autostart a bandeja.

---

## Notas de orden de ejecución

- **Fase 1 primero** (pérdida de datos), con su checkpoint VM.
- Fases 2, 3, 4 son independientes entre sí (distintos archivos en su mayoría; Fase 2 Task 3 y 4
  tocan bridge/ops_ctrl, Fase 4 toca core/metadata) — se pueden intercalar, pero secuencial es seguro.
- **Fase 5** (Tasks 9, 10) instrumenta y PARA en el checkpoint de reproducción; la **Task 11** (fix)
  solo se ejecuta CON los logs de Nicolás. El controlador debe esperar la evidencia.
- **Fase 6** (Task 12) va DESPUÉS de la 5a (el tray debe crearse bien).
- **Fase 7** (Task 13) al final.

## Self-review (cobertura del spec)

- Fase 1 bug crítico (MK_CONTROL + decide_drop + label) → Task 1, 2 ✓
- Fase 2 fecha + orden (los DOS historiales) → Task 3 (acciones/undo), Task 4 (operaciones) ✓
- Fase 3 path-bar (select-all + izquierda) → Task 5 ✓
- Fase 4 metadata (audio lofty, Office moderno docx/xlsx/pptx sin deps nuevas, PDF con contingencia) →
  Task 6, 7, 8 ✓
- Fase 5 diagnóstico dirigido (cierre + scroll, con logs y checkpoints) → Task 9, 10, 11 ✓
- Fase 6 autostart a bandeja (persistir estado + Win32) → Task 12 ✓
- i18n 10 idiomas (todas las claves nuevas) → en Tasks 6, 7, 8 ✓
- Verificación + dist → Task 13 ✓
- Office viejo y extender-diseño: fuera de alcance (no hay tasks, correcto) ✓

Ambigüedades resueltas: dos historiales distintos (undo ya tiene epoch, operaciones no); wall-clock vs
monótono para la fecha; invertir presentación no estructura del undo; PDF con contingencia de diferir;
Fases 5/6 con checkpoints de VM explícitos.
