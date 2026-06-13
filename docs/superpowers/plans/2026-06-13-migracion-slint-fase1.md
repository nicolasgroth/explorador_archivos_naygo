# Migración a Slint — Fase 1: esqueleto navegable — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Crear el crate `naygo-ui-slint` (convive con `naygo-ui`/egui) con un panel de
archivos navegable: tabla virtualizada, listado async vía core, navegación, orden por
columna, selección clic/Ctrl/Shift y teclado completo de lista — render por software (sin
GPU).

**Architecture:** Crate nuevo, organizado por responsabilidad: `.slint` por componente +
módulos Rust `bridge` (Entry↔modelo, puro+tests), `keys` (KeyEvent→Chord, puro+tests),
`listing` (pump async con `slint::Timer` que drena por lotes), `controller` (estado
`FilePaneState` + handlers). CERO lógica de negocio nueva: reusa `naygo-core` y
`naygo-platform`.

**Tech Stack:** Rust, Slint 1.16 (`backend-winit` + `renderer-software`), naygo-core.

---

## Reglas operativas (NO te las saltes)

1. **Puertas antes de CADA commit**: `cargo test --workspace` (lee TODAS las líneas
   `test result:`), `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo fmt --all`. (El crate viejo `naygo-ui` debe seguir compilando: NO lo toques.)
2. **`cargo build -p naygo-ui-slint`** antes de cualquier prueba en vivo. Para forzar el
   render por software (caso VM sin GPU): `$env:SLINT_BACKEND="winit-software"`.
3. **Mata procesos** antes de compilar: `Stop-Process -Name naygo-slint,naygo -Force -ErrorAction SilentlyContinue`.
4. **Commits en español** con heredoc de Bash (`git commit -F - <<'EOF' … EOF`).
   **Stagea rutas explícitas**, NO `git add -A` (hay un cambio ajeno en `CLAUDE.md`).
5. Header de copyright en archivos nuevos; comentarios en español del PORQUÉ.
6. El binario nuevo se llama `naygo-slint` (no reemplaza a `naygo` hasta F6).

## Estructura de archivos (a crear)

```
crates/ui-slint/
  Cargo.toml
  build.rs
  ui/ app-window.slint   file-panel.slint   path-bar.slint   types.slint
  src/ main.rs   controller.rs   bridge.rs   listing.rs   keys.rs
```

---

### Tarea 1 — Andamiaje del crate (compila y abre ventana vacía)

**Files:**
- Create: `crates/ui-slint/Cargo.toml`, `crates/ui-slint/build.rs`,
  `crates/ui-slint/ui/app-window.slint`, `crates/ui-slint/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Cargo.toml del crate**

Crear `crates/ui-slint/Cargo.toml` (mismas features que el proto ya validado):

```toml
# Naygo — capa UI en Slint (render por software, sin GPU). Convive con naygo-ui (egui)
# durante la migracion. Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
[package]
name = "naygo-ui-slint"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[[bin]]
name = "naygo-slint"
path = "src/main.rs"

[dependencies]
naygo-core = { path = "../core" }
naygo-platform = { path = "../platform" }
slint = { version = "1.16", default-features = false, features = [
    "compat-1-2", "std", "backend-winit", "renderer-software",
] }

[build-dependencies]
slint-build = "1.16"
```

- [ ] **Step 2: build.rs**

Crear `crates/ui-slint/build.rs`:

```rust
// Naygo — compila los .slint de la capa UI Slint.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
fn main() {
    slint_build::compile("ui/app-window.slint").expect("compilar app-window.slint");
}
```

- [ ] **Step 3: app-window.slint mínima (ventana vacía con título)**

Crear `crates/ui-slint/ui/app-window.slint`:

```slint
// Naygo — ventana principal (Slint). Copyright (c) 2026 Nicolás Groth / ISGroth. MIT.
export component AppWindow inherits Window {
    title: "Naygo";
    preferred-width: 1000px;
    preferred-height: 640px;
    Text { text: "Naygo (Slint) — Fase 1"; }
}
```

- [ ] **Step 4: main.rs mínimo**

Crear `crates/ui-slint/src/main.rs`:

```rust
// Naygo — arranque de la capa UI en Slint (Fase 1).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.run()
}
```

- [ ] **Step 5: Agregar al workspace**

En el `Cargo.toml` raíz, agregar `"crates/ui-slint"` a `members`:

```toml
members = ["crates/core", "crates/platform", "crates/ui", "crates/proto-slint", "crates/ui-slint"]
```

- [ ] **Step 6: Build + commit**

Run (PowerShell): `cargo build -p naygo-ui-slint 2>&1 | Select-String "error|Finished" | Select-Object -Last 2`
Expected: `Finished`.

```bash
git add crates/ui-slint Cargo.toml Cargo.lock
git commit -F - <<'EOF'
feat(slint): andamiaje del crate naygo-ui-slint (ventana vacia)

Crate nuevo (binario naygo-slint) que convive con naygo-ui/egui. Slint con backend
winit + renderizador por software (sin GPU). Solo abre una ventana; las siguientes
tareas agregan el panel de archivos.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 2 — bridge: Entry/FilePaneState → modelo de filas (puro, con tests)

**Files:**
- Create: `crates/ui-slint/src/bridge.rs`
- Modify: `crates/ui-slint/ui/types.slint` (nuevo), `app-window.slint` (importar types),
  `crates/ui-slint/src/main.rs` (declarar `mod bridge;`)

- [ ] **Step 1: Definir el struct RowData en Slint**

Crear `crates/ui-slint/ui/types.slint`:

```slint
// Naygo — tipos compartidos UI<->Rust (Slint). Copyright (c) 2026 N. Groth / ISGroth. MIT.
export struct RowData {
    name: string,
    ext: string,
    size: string,
    modified: string,
    is-dir: bool,
    selected: bool,
    focused: bool,
}
```

En `app-window.slint`, al inicio, importar el struct para que el módulo Rust lo conozca:

```slint
import { RowData } from "types.slint";
```

(En la Tarea 4 `RowData` se usará en la ListView; por ahora solo se importa para que
`slint::include_modules!()` lo exponga a Rust.)

- [ ] **Step 2: Escribir el test del bridge**

Crear `crates/ui-slint/src/bridge.rs` con SOLO los tests primero (TDD):

```rust
// Naygo — puente entre el estado del panel (core) y el modelo de filas de Slint (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::fs_model::{Entry, EntryKind};
    use naygo_core::workspace::FilePaneState;
    use std::path::PathBuf;

    fn mk(name: &str, dir: bool, size: Option<u64>) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from(format!("C:/x/{name}")),
            kind: if dir { EntryKind::Directory } else { EntryKind::File },
            size,
            modified: None,
            created: None,
            hidden: false,
        }
    }

    #[test]
    fn rows_reflejan_vista_seleccion_y_foco() {
        let mut f = FilePaneState::new(PathBuf::from("C:/x"));
        f.entries = vec![mk("a.txt", false, Some(1024)), mk("dir", true, None)];
        // Ordenado por core: dirs_first → "dir" primero, "a.txt" después.
        f.select_single(0); // selecciona la 1ª de la vista
        let rows = rows_from_view(&f);
        assert_eq!(rows.len(), 2);
        // is_dir y formato de tamaño correctos.
        assert!(rows.iter().any(|r| r.name == "dir" && r.is_dir));
        assert!(rows.iter().any(|r| r.name == "a.txt" && !r.is_dir && !r.size.is_empty()));
        // exactamente una fila seleccionada y con foco (la posición 0 de la vista).
        assert_eq!(rows.iter().filter(|r| r.selected).count(), 1);
        assert_eq!(rows.iter().filter(|r| r.focused).count(), 1);
    }

    #[test]
    fn vista_vacia_da_modelo_vacio() {
        let f = FilePaneState::new(PathBuf::from("C:/x"));
        assert!(rows_from_view(&f).is_empty());
    }
}
```

- [ ] **Step 3: Run test (falla: `rows_from_view`/`PlainRow` no existen)**

Run (PowerShell): `cargo test -p naygo-ui-slint bridge 2>&1 | Select-String "error|test result"`
Expected: FALLA de compilación.

- [ ] **Step 4: Implementar el bridge**

El bridge produce un `Vec<PlainRow>` PURO (sin tipos de Slint, para testear sin Slint).
La conversión `PlainRow → RowData` (tipo generado por Slint) vive en `controller`/`main`.
Agregar ARRIBA del `mod tests` en `crates/ui-slint/src/bridge.rs`:

```rust
use naygo_core::workspace::FilePaneState;

/// Fila plana lista para pintar (espejo de `RowData` de Slint, pero sin depender de los
/// tipos generados → testeable en core puro). `controller` la convierte a `RowData`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlainRow {
    pub name: String,
    pub ext: String,
    pub size: String,
    pub modified: String,
    pub is_dir: bool,
    pub selected: bool,
    pub focused: bool,
}

/// Construye las filas a pintar desde el estado del panel: usa los índices de vista
/// CACHEADOS del core (filtrados+ordenados), y marca selección/foco por POSICIÓN DE
/// VISTA (consistente con `FilePaneState.selected`/`focused`). No clona las entries
/// completas: lee por índice.
pub fn rows_from_view(f: &FilePaneState) -> Vec<PlainRow> {
    let view = f.view_indices();
    view.iter()
        .enumerate()
        .filter_map(|(pos, &real)| {
            let e = f.entries.get(real)?;
            let ext = e
                .path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let size = match e.size {
                Some(b) => naygo_core::format::human_size(b),
                None => String::new(),
            };
            Some(PlainRow {
                name: e.name.clone(),
                ext,
                size,
                modified: fmt_time(e.modified),
                is_dir: e.kind == naygo_core::fs_model::EntryKind::Directory,
                selected: f.selected.contains(&pos),
                focused: f.focused == Some(pos),
            })
        })
        .collect()
}

/// Formato provisional de fecha (epoch en segundos), igual que la capa egui actual; el
/// formato bonito es ortogonal a la migración.
fn fmt_time(t: Option<std::time::SystemTime>) -> String {
    use std::time::UNIX_EPOCH;
    match t.and_then(|t| t.duration_since(UNIX_EPOCH).ok()) {
        Some(d) => format!("{}", d.as_secs()),
        None => String::new(),
    }
}
```

En `src/main.rs`, agregar `mod bridge;` (antes de `fn main`).

- [ ] **Step 5: Run test (pasa) + commit**

Run (PowerShell): `cargo test -p naygo-ui-slint bridge 2>&1 | Select-String "test result"`
Expected: `test result: ok`.

```bash
git add crates/ui-slint/src/bridge.rs crates/ui-slint/src/main.rs crates/ui-slint/ui/types.slint crates/ui-slint/ui/app-window.slint
git commit -F - <<'EOF'
feat(slint): bridge Entry/FilePaneState -> filas planas (puro, con tests)

rows_from_view usa los indices de vista cacheados del core (filtrados+ordenados) y
marca seleccion/foco por posicion de vista. PlainRow es independiente de los tipos de
Slint para testear sin UI. Struct RowData definido en types.slint.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 3 — keys: KeyEvent (texto+modificadores) → keymap::Chord (puro, con tests)

**Files:**
- Create: `crates/ui-slint/src/keys.rs`
- Modify: `crates/ui-slint/src/main.rs` (`mod keys;`)

**Diseño:** el `.slint` pasará a Rust `(text: SharedString, ctrl, shift, alt)`. `keys`
mapea ese `text` (las teclas especiales de Slint llegan como chars unicode de
`slint::platform::Key`) a un `keymap::KeyCode`, y arma el `Chord`. Así el teclado reusa
el keymap configurable del core.

- [ ] **Step 1: Escribir el test**

Crear `crates/ui-slint/src/keys.rs` con los tests primero:

```rust
// Naygo — mapeo de teclas de Slint a Chord del keymap del core (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

#[cfg(test)]
mod tests {
    use super::*;
    use naygo_core::keymap::{Chord, KeyCode};

    #[test]
    fn flechas_y_especiales() {
        // Slint entrega las teclas especiales como chars de slint::platform::Key.
        assert_eq!(chord_from(&key_char(slint::platform::Key::UpArrow), false, false, false),
                   Some(Chord::plain(KeyCode::ArrowUp)));
        assert_eq!(chord_from(&key_char(slint::platform::Key::Return), false, false, false),
                   Some(Chord::plain(KeyCode::Enter)));
        assert_eq!(chord_from(&key_char(slint::platform::Key::Backspace), false, false, false),
                   Some(Chord::plain(KeyCode::Backspace)));
        assert_eq!(chord_from(&key_char(slint::platform::Key::Home), false, false, false),
                   Some(Chord::plain(KeyCode::Home)));
    }

    #[test]
    fn letras_y_modificadores() {
        assert_eq!(chord_from("c", true, false, false),
                   Some(Chord::ctrl(KeyCode::Char('c'))));
        assert_eq!(chord_from("A", false, false, false),
                   Some(Chord::plain(KeyCode::Char('a'))));
        // Shift+Down extiende: el chord lleva shift.
        let c = chord_from(&key_char(slint::platform::Key::DownArrow), false, true, false).unwrap();
        assert!(c.shift && c.key == KeyCode::ArrowDown);
    }

    #[test]
    fn texto_vacio_o_desconocido_es_none() {
        assert_eq!(chord_from("", false, false, false), None);
    }

    /// Helper: el char unicode de una tecla especial de Slint, como String.
    fn key_char(k: slint::platform::Key) -> String {
        let s: slint::SharedString = k.into();
        s.to_string()
    }
}
```

- [ ] **Step 2: Run test (falla: `chord_from` no existe)**

Run (PowerShell): `cargo test -p naygo-ui-slint keys 2>&1 | Select-String "error|test result"`
Expected: FALLA de compilación.

- [ ] **Step 3: Implementar `chord_from`**

Arriba del `mod tests`:

```rust
use naygo_core::keymap::{Chord, KeyCode};

/// Convierte la tecla recibida de Slint (su texto unicode + modificadores) en un `Chord`
/// del keymap del core. Las teclas especiales de Slint (flechas, Enter, etc.) llegan como
/// chars de `slint::platform::Key`; las normales como su carácter. Devuelve `None` si la
/// tecla no la modela el keymap (se ignora).
pub fn chord_from(text: &str, ctrl: bool, shift: bool, alt: bool) -> Option<Chord> {
    let key = keycode_from(text)?;
    Some(Chord { key, ctrl, shift, alt })
}

/// Mapea el texto de la tecla a un `KeyCode`. Compara contra los chars de las teclas
/// especiales de Slint y, si no, toma el primer carácter como letra/dígito.
fn keycode_from(text: &str) -> Option<KeyCode> {
    use slint::platform::Key;
    let first = text.chars().next()?;
    // Teclas especiales: comparar el char contra el de cada Key de Slint.
    let special = |k: Key| -> char {
        let s: slint::SharedString = k.into();
        s.chars().next().unwrap_or('\0')
    };
    let kc = if first == special(Key::UpArrow) {
        KeyCode::ArrowUp
    } else if first == special(Key::DownArrow) {
        KeyCode::ArrowDown
    } else if first == special(Key::LeftArrow) {
        KeyCode::ArrowLeft
    } else if first == special(Key::RightArrow) {
        KeyCode::ArrowRight
    } else if first == special(Key::Return) {
        KeyCode::Enter
    } else if first == special(Key::Backspace) {
        KeyCode::Backspace
    } else if first == special(Key::Tab) {
        KeyCode::Tab
    } else if first == special(Key::Escape) {
        KeyCode::Escape
    } else if first == special(Key::Delete) {
        KeyCode::Delete
    } else if first == special(Key::PageUp) {
        KeyCode::PageUp
    } else if first == special(Key::PageDown) {
        KeyCode::PageDown
    } else if first == special(Key::Home) {
        KeyCode::Home
    } else if first == special(Key::End) {
        KeyCode::End
    } else if first == special(Key::Space) || first == ' ' {
        KeyCode::Space
    } else if first.is_alphanumeric() {
        // Letra o dígito: normalizar a minúscula (el keymap usa Char minúscula).
        KeyCode::Char(first.to_ascii_lowercase())
    } else {
        return None;
    };
    Some(kc)
}
```

En `src/main.rs`, agregar `mod keys;`.

- [ ] **Step 4: Run test (pasa) + commit**

Run (PowerShell): `cargo test -p naygo-ui-slint keys 2>&1 | Select-String "test result"`
Expected: `test result: ok`.

NOTA: si algún `Key` variante no existe con ese nombre exacto en slint 1.16 (p. ej.
`Return` vs `Enter`), corregir el nombre según el error del compilador (la API de
`slint::platform::Key` es estable; los nombres son los del enum). Las F2..F6 NO se mapean
en F1 (no las necesita el teclado de lista); se agregan en fases posteriores si hace falta.

```bash
git add crates/ui-slint/src/keys.rs crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(slint): mapear teclas de Slint a Chord del keymap del core (puro, con tests)

chord_from(text, ctrl, shift, alt) traduce el KeyEvent de Slint (texto unicode +
modificadores) al Chord del keymap configurable del core. Asi el teclado de la UI Slint
reusa el mismo keymap de 47 acciones. Tests de flechas/especiales/letras/modificadores.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 4 — file-panel.slint: tabla virtualizada + encabezados + path-bar

**Files:**
- Create: `crates/ui-slint/ui/file-panel.slint`, `crates/ui-slint/ui/path-bar.slint`
- Modify: `crates/ui-slint/ui/app-window.slint`

- [ ] **Step 1: path-bar.slint (breadcrumbs simples + subir)**

Crear `crates/ui-slint/ui/path-bar.slint`:

```slint
// Naygo — path-bar minima (Fase 1): ruta actual + boton subir.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
export component PathBar inherits Rectangle {
    in property <string> path;
    callback go-up();
    height: 30px;
    background: #1b2838;
    border-radius: 4px;
    HorizontalLayout {
        padding: 4px;
        spacing: 6px;
        Rectangle {
            width: 30px;
            background: up-touch.has-hover ? #2a4a7a : transparent;
            border-radius: 4px;
            Text { text: "↑"; color: white; horizontal-alignment: center; vertical-alignment: center; }
            up-touch := TouchArea { clicked => { root.go-up(); } }
        }
        Text { text: root.path; color: #cdd; vertical-alignment: center; }
    }
}
```

- [ ] **Step 2: file-panel.slint (tabla con ListView virtualizada + encabezados + foco de teclado)**

Crear `crates/ui-slint/ui/file-panel.slint`:

```slint
// Naygo — panel de archivos (Fase 1): tabla virtualizada, encabezados, foco de teclado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
import { ListView } from "std-widgets.slint";
import { RowData } from "types.slint";

export component FilePanel inherits FocusScope {
    in property <[RowData]> rows;
    in property <string> sort-indicator-name;   // "▲"/"▼"/"" en la columna activa
    in property <length> scroll-y;              // el controller lo fija para seguir el foco
    callback row-clicked(int, bool, bool);      // (pos, ctrl, shift)
    callback row-double-clicked(int);
    callback sort-by(string);                   // "name"|"ext"|"size"|"modified"
    callback key(string, bool, bool, bool);     // (texto, ctrl, shift, alt)

    // El FocusScope captura el teclado; reenvia a Rust (que resuelve via keymap).
    key-pressed(event) => {
        root.key(event.text, event.modifiers.control, event.modifiers.shift, event.modifiers.alt);
        accept
    }

    VerticalLayout {
        // Encabezados de columna (clic ordena).
        HorizontalLayout {
            height: 22px;
            HeaderCell { text: "Nombre"; width: 50%; clicked => { root.sort-by("name"); } }
            HeaderCell { text: "Extensión"; width: 15%; clicked => { root.sort-by("ext"); } }
            HeaderCell { text: "Tamaño"; width: 15%; clicked => { root.sort-by("size"); } }
            HeaderCell { text: "Modificado"; width: 20%; clicked => { root.sort-by("modified"); } }
        }
        // Lista VIRTUALIZADA (solo materializa filas visibles).
        list := ListView {
            viewport-y <=> root.scroll-y;
            for row[i] in root.rows: Rectangle {
                height: 22px;
                background: row.selected ? #2a4a7a : (row-touch.has-hover ? #1f3350 : transparent);
                // Borde de foco (teclado): un rectangulo de 1px de acento.
                border-width: row.focused ? 1px : 0px;
                border-color: #4f8ae0;
                HorizontalLayout {
                    padding-left: 6px;
                    Text { text: (row.is-dir ? "📁 " : "") + row.name; color: row.is-dir ? #ffd479 : #dde; vertical-alignment: center; width: 50%; overflow: elide; }
                    Text { text: row.ext; color: #9ab; vertical-alignment: center; width: 15%; overflow: elide; }
                    Text { text: row.size; color: #9ab; vertical-alignment: center; width: 15%; horizontal-alignment: right; }
                    Text { text: row.modified; color: #9ab; vertical-alignment: center; width: 20%; overflow: elide; }
                }
                row-touch := TouchArea {
                    clicked => {
                        root.focus();
                        root.row-clicked(i, row-touch.pressed-event.modifiers.control, row-touch.pressed-event.modifiers.shift);
                    }
                    double-clicked => { root.row-double-clicked(i); }
                }
            }
        }
    }
}

// Celda de encabezado clicable.
component HeaderCell inherits Rectangle {
    in property <string> text;
    callback clicked();
    background: hcell-touch.has-hover ? #234 : #1b2838;
    Text { text: root.text; color: #9ab; font-weight: 700; vertical-alignment: center; x: 6px; }
    hcell-touch := TouchArea { clicked => { root.clicked(); } }
}
```

NOTA sobre los modificadores del clic: si `TouchArea.pressed-event` no expone modifiers
en slint 1.16, simplificar `row-clicked` a `row-clicked(int)` y leer Ctrl/Shift desde el
último estado de teclado en Rust (el controller guarda los modificadores del último
`key`), o usar `clicked` sin modificadores en F1 y resolver Ctrl/Shift+clic en la Tarea 6
si la API lo permite. El compilador dirá si `pressed-event.modifiers` existe.

- [ ] **Step 3: app-window.slint compone path-bar + file-panel y expone propiedades/callbacks**

Reemplazar `crates/ui-slint/ui/app-window.slint`:

```slint
// Naygo — ventana principal (Slint, Fase 1). Copyright (c) 2026 N. Groth / ISGroth. MIT.
import { RowData } from "types.slint";
import { PathBar } from "path-bar.slint";
import { FilePanel } from "file-panel.slint";

export component AppWindow inherits Window {
    title: "Naygo";
    preferred-width: 1000px;
    preferred-height: 640px;

    in property <string> current-path;
    in property <[RowData]> rows;
    in property <length> panel-scroll-y;
    callback go-up();
    callback row-clicked(int, bool, bool);
    callback row-double-clicked(int);
    callback sort-by(string);
    callback key(string, bool, bool, bool);

    VerticalLayout {
        padding: 6px;
        spacing: 4px;
        PathBar { path: root.current-path; go-up => { root.go-up(); } }
        panel := FilePanel {
            rows: root.rows;
            scroll-y <=> root.panel-scroll-y;
            row-clicked(i, c, s) => { root.row-clicked(i, c, s); }
            row-double-clicked(i) => { root.row-double-clicked(i); }
            sort-by(col) => { root.sort-by(col); }
            key(t, c, s, a) => { root.key(t, c, s, a); }
        }
    }

    // Pedir el foco del panel al iniciar (el teclado va al FocusScope).
    init => { panel.focus(); }
}
```

- [ ] **Step 4: build (solo compila el .slint; main aún no usa las props)**

Run (PowerShell): `cargo build -p naygo-ui-slint 2>&1 | Select-String "error|Finished" | Select-Object -Last 5`
Expected: `Finished` (o errores de sintaxis del .slint a corregir; ej. nombres de Key,
`pressed-event`). Resolver según el mensaje; la lógica Rust se conecta en la Tarea 5.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/ui
git commit -F - <<'EOF'
feat(slint): file-panel (tabla virtualizada + encabezados ordenables) y path-bar

ListView virtualiza filas; FocusScope captura el teclado y lo reenvia a Rust; clic y
doble clic con callbacks; encabezados clicables para ordenar; path-bar minima con boton
subir. app-window compone todo y expone props/callbacks. Aun sin logica Rust (Tarea 5).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 5 — listing: pump async (Timer drena por lotes) + controller (estado y handlers)

**Files:**
- Create: `crates/ui-slint/src/listing.rs`, `crates/ui-slint/src/controller.rs`
- Modify: `crates/ui-slint/src/main.rs`

- [ ] **Step 1: listing.rs — worker + Timer que drena por lotes**

Crear `crates/ui-slint/src/listing.rs`:

```rust
// Naygo — listado async para la UI Slint: worker del core + Timer que drena por lotes.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// El worker del core emite un Entry por archivo. Un slint::Timer de ~30ms drena TODO lo
// acumulado en el canal en un lote y se APAGA al terminar (Done): 0 trabajo en reposo,
// repaints acotados (~30fps) durante el listado. Es el patron clave para el bajo consumo
// sin GPU (no inundar el event loop con miles de eventos por archivo).

use naygo_core::cancel::CancellationToken;
use naygo_core::fs_model::Entry;
use naygo_core::listing::{spawn_listing, ListingMsg};
use std::sync::mpsc::Receiver;

/// Estado de un listado en curso (worker + canal + token de cancelacion).
pub struct Listing {
    rx: Receiver<ListingMsg>,
    token: CancellationToken,
}

impl Listing {
    /// Lanza el listado de `dir`. El worker corre en su hilo; el Timer (en el controller)
    /// drenara `poll`.
    pub fn start(dir: std::path::PathBuf) -> Listing {
        let token = CancellationToken::new();
        let (rx, _handle) = spawn_listing(dir, token.clone());
        Listing { rx, token }
    }

    /// Cancela el listado (al navegar a otra carpeta antes de terminar).
    pub fn cancel(&self) {
        self.token.cancel();
    }

    /// Drena TODO lo acumulado en el canal AHORA (sin bloquear). Devuelve las entries
    /// nuevas del lote y si el listado TERMINO (Done/Error/Cancelled).
    pub fn poll(&self) -> (Vec<Entry>, bool) {
        let mut batch = Vec::new();
        let mut done = false;
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ListingMsg::Entry(e) => batch.push(e),
                ListingMsg::Done | ListingMsg::Cancelled | ListingMsg::Error(_) => {
                    done = true;
                    break;
                }
            }
        }
        (batch, done)
    }
}
```

- [ ] **Step 2: controller.rs — estado del panel + handlers de gestos**

Crear `crates/ui-slint/src/controller.rs`:

```rust
// Naygo — controlador de la UI Slint (Fase 1): estado del panel + handlers de gestos.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Es el equivalente ACOTADO de NaygoApp para la Fase 1: posee el FilePaneState (un solo
// panel), traduce los gestos de Slint (clic, doble clic, orden, teclado) a llamadas del
// core, y reconstruye el modelo de filas. CERO logica de negocio nueva.

use crate::bridge::{rows_from_view, PlainRow};
use crate::listing::Listing;
use naygo_core::columns::ColumnKind;
use naygo_core::fs_model::{EntryKind, SortKey};
use naygo_core::keymap::{Action, KeyMap};
use naygo_core::workspace::FilePaneState;

/// Alto de fila (px) — fijo, igual que la tabla egui; se usa para el scroll a foco.
pub const ROW_HEIGHT: f32 = 22.0;

/// Estado de la Fase 1: un panel + su keymap + el listado en curso.
pub struct Controller {
    pub pane: FilePaneState,
    pub keymap: KeyMap,
    pub listing: Option<Listing>,
    pub typeahead: String,
}

impl Controller {
    pub fn new(start: std::path::PathBuf) -> Controller {
        let mut pane = FilePaneState::new(start.clone());
        pane.navigate_to(start.clone());
        let mut c = Controller {
            pane,
            keymap: KeyMap::defaults(),
            listing: None,
            typeahead: String::new(),
        };
        c.start_listing(start);
        c
    }

    /// Arranca (o reinicia) el listado de `dir`. Cancela el anterior.
    pub fn start_listing(&mut self, dir: std::path::PathBuf) {
        if let Some(l) = &self.listing {
            l.cancel();
        }
        self.listing = Some(Listing::start(dir));
    }

    /// Drena el lote actual del listado. Devuelve `true` si el listado TERMINO (para que
    /// el caller apague el timer). Aplica las entries y reordena al terminar.
    pub fn pump_listing(&mut self) -> bool {
        let Some(l) = &self.listing else { return true };
        let (batch, done) = l.poll();
        if !batch.is_empty() {
            self.pane.entries.extend(batch);
        }
        if done {
            let spec = self.pane.sort;
            naygo_core::sort::sort_entries(&mut self.pane.entries, &spec);
            if self.pane.focused.is_none() && !self.pane.entries.is_empty() {
                self.pane.focused = Some(0);
            }
            self.listing = None;
        }
        done
    }

    /// ¿Hay un listado activo? (el timer corre mientras sí).
    pub fn listing_active(&self) -> bool {
        self.listing.is_some()
    }

    /// Filas actuales para el modelo de Slint.
    pub fn rows(&self) -> Vec<PlainRow> {
        rows_from_view(&self.pane)
    }

    pub fn current_path(&self) -> String {
        self.pane.current_dir.display().to_string()
    }

    /// Clic en una fila (pos de vista) con modificadores.
    pub fn on_row_clicked(&mut self, pos: usize, ctrl: bool, shift: bool) {
        if shift {
            self.pane.select_range_to(pos);
        } else if ctrl {
            self.pane.select_toggle(pos);
        } else {
            self.pane.select_single(pos);
        }
    }

    /// Doble clic: carpeta navega; archivo abre con la app por defecto. Devuelve la
    /// carpeta a la que navegar (para que el caller arranque el listado), si aplica.
    pub fn on_row_double_clicked(&mut self, pos: usize) -> Option<std::path::PathBuf> {
        let view = self.pane.view_indices();
        let real = *view.get(pos)?;
        let e = self.pane.entries.get(real)?.clone();
        if e.kind == EntryKind::Directory {
            self.pane.navigate_to(e.path.clone());
            Some(e.path)
        } else {
            let _ = naygo_platform::open::open_default(&e.path);
            None
        }
    }

    /// Subir al padre. Devuelve la carpeta a listar, si hay padre.
    pub fn on_go_up(&mut self) -> Option<std::path::PathBuf> {
        self.pane.go_up()
    }

    /// Clic en encabezado: ordenar por esa columna (alterna asc/desc si ya era esa).
    pub fn on_sort_by(&mut self, column: &str) {
        let key = match column {
            "name" => SortKey::Name,
            "ext" => SortKey::Extension,
            "size" => SortKey::Size,
            "modified" => SortKey::Modified,
            _ => return,
        };
        if self.pane.sort.key == key {
            self.pane.sort.ascending = !self.pane.sort.ascending;
        } else {
            self.pane.sort.key = key;
            self.pane.sort.ascending = true;
        }
        let spec = self.pane.sort;
        naygo_core::sort::sort_entries(&mut self.pane.entries, &spec);
        let _ = ColumnKind::Name; // (ColumnKind se usará al mapear encabezados->orden en fases con menú)
    }

    /// Tecla: resuelve via keymap y aplica la accion navegable de Fase 1. Devuelve la
    /// carpeta a listar si la accion navegó (GoUp/Activate sobre carpeta).
    pub fn on_key(&mut self, text: &str, ctrl: bool, shift: bool, alt: bool) -> Option<std::path::PathBuf> {
        let Some(chord) = crate::keys::chord_from(text, ctrl, shift, alt) else {
            // Sin chord conocido: typeahead (salto por tipeo) con texto imprimible.
            return self.typeahead(text);
        };
        let Some(action) = self.keymap.action_for(&chord) else {
            return self.typeahead(text);
        };
        self.typeahead.clear();
        let rows = self.pane.view_indices().len();
        match action {
            Action::MoveUp => { self.pane.move_focus_extend(-1, false); None }
            Action::MoveDown => { self.pane.move_focus_extend(1, false); None }
            Action::ExtendUp => { self.pane.move_focus_extend(-1, true); None }
            Action::ExtendDown => { self.pane.move_focus_extend(1, true); None }
            Action::FocusPageUp => { self.pane.focus_page(-1, 20, false); None }
            Action::FocusPageDown => { self.pane.focus_page(1, 20, false); None }
            Action::ExtendPageUp => { self.pane.focus_page(-1, 20, true); None }
            Action::ExtendPageDown => { self.pane.focus_page(1, 20, true); None }
            Action::FocusHome => { self.pane.focus_home(false); None }
            Action::FocusEnd => { self.pane.focus_end(false); None }
            Action::ExtendHome => { self.pane.focus_home(true); None }
            Action::ExtendEnd => { self.pane.focus_end(true); None }
            Action::FocusUpKeep => { self.pane.move_focus_keep(-1); None }
            Action::FocusDownKeep => { self.pane.move_focus_keep(1); None }
            Action::ToggleSelect | Action::ToggleFocused => {
                if let Some(p) = self.pane.focused { self.pane.select_toggle(p); }
                None
            }
            Action::SelectAll => { self.pane.select_all(); None }
            Action::GoUp => self.on_go_up(),
            Action::Activate => {
                let pos = self.pane.focused?;
                self.on_row_double_clicked(pos)
            }
            _ => { let _ = rows; None } // resto de acciones: no-op en F1 (se conectan en F3+)
        }
    }

    /// Salto por tipeo: primer item de la vista cuyo nombre empieza con lo tecleado
    /// (case-insensitive). Reset implicito al limpiar `typeahead` ante una accion.
    fn typeahead(&mut self, text: &str) -> Option<std::path::PathBuf> {
        let ch = text.chars().next().filter(|c| !c.is_control())?;
        self.typeahead.push(ch.to_ascii_lowercase());
        let view = self.pane.view_indices();
        let needle = &self.typeahead;
        for (pos, &real) in view.iter().enumerate() {
            if let Some(e) = self.pane.entries.get(real) {
                if e.name.to_lowercase().starts_with(needle.as_str()) {
                    self.pane.select_single(pos);
                    break;
                }
            }
        }
        None
    }

    /// Offset de scroll (px) para que la fila enfocada quede visible. El caller lo aplica
    /// a `panel-scroll-y` (negativo = el viewport sube). Simplificacion F1: alinea la fila
    /// enfocada al tope si esta fuera; el pulido fino se hace en verificacion viva.
    pub fn focus_scroll_y(&self) -> f32 {
        match self.pane.focused {
            Some(p) => -(p as f32) * ROW_HEIGHT,
            None => 0.0,
        }
    }
}
```

NOTA: `focus_scroll_y` es una primera aproximación (alinea al tope). En la verificación
viva se ajusta a "scroll mínimo para que la fila entre en vista" si hace falta; no es
crítico para la paridad funcional de F1.

- [ ] **Step 3: main.rs — conecta callbacks de Slint al controller + Timer**

Reemplazar `crates/ui-slint/src/main.rs`:

```rust
// Naygo — arranque de la capa UI en Slint (Fase 1).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Para forzar el renderizador por software (caso VM sin GPU):
//   $env:SLINT_BACKEND="winit-software"; cargo run -p naygo-ui-slint
mod bridge;
mod controller;
mod keys;
mod listing;

use controller::Controller;
use slint::{Model, ModelRc, SharedString, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let start = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/"));

    let ctrl = Rc::new(RefCell::new(Controller::new(start)));
    let model: Rc<VecModel<RowData>> = Rc::new(VecModel::default());
    ui.set_rows(ModelRc::from(model.clone()));

    // Refresca el modelo + path + scroll desde el estado del controller.
    let refresh = {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let model = model.clone();
        move || {
            let c = ctrl.borrow();
            let rows: Vec<RowData> = c.rows().into_iter().map(to_row_data).collect();
            model.set_vec(rows);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_current_path(SharedString::from(c.current_path().as_str()));
                ui.set_panel_scroll_y(slint::LogicalLength::new(c.focus_scroll_y()));
            }
        }
    };

    // Timer del listado: drena por lotes ~30ms mientras hay listado activo; se apaga al
    // terminar (0 trabajo en reposo). Se (re)arranca tras cada navegacion.
    let listing_timer = Rc::new(slint::Timer::default());
    let start_timer = {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let timer = listing_timer.clone();
        move || {
            let ctrl = ctrl.clone();
            let refresh = refresh.clone();
            let timer2 = timer.clone();
            timer.start(
                TimerMode::Repeated,
                std::time::Duration::from_millis(30),
                move || {
                    let done = ctrl.borrow_mut().pump_listing();
                    refresh();
                    if done {
                        timer2.stop();
                    }
                },
            );
        }
    };
    start_timer(); // listado inicial

    // Cablear callbacks.
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_row_double_clicked(move |i| {
            if ctrl.borrow_mut().on_row_double_clicked(i as usize).is_some() {
                start_timer();
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        ui.on_row_clicked(move |i, ctrl_mod, shift_mod| {
            ctrl.borrow_mut().on_row_clicked(i as usize, ctrl_mod, shift_mod);
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_go_up(move || {
            if ctrl.borrow_mut().on_go_up().is_some() {
                start_timer();
            }
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        ui.on_sort_by(move |col| {
            ctrl.borrow_mut().on_sort_by(col.as_str());
            refresh();
        });
    }
    {
        let ctrl = ctrl.clone();
        let refresh = refresh.clone();
        let start_timer = start_timer.clone();
        ui.on_key(move |text, c, s, a| {
            if ctrl.borrow_mut().on_key(text.as_str(), c, s, a).is_some() {
                start_timer();
            }
            refresh();
        });
    }

    refresh();
    ui.run()
}

/// Convierte la fila plana del bridge al `RowData` generado por Slint.
fn to_row_data(r: bridge::PlainRow) -> RowData {
    RowData {
        name: SharedString::from(r.name.as_str()),
        ext: SharedString::from(r.ext.as_str()),
        size: SharedString::from(r.size.as_str()),
        modified: SharedString::from(r.modified.as_str()),
        is_dir: r.is_dir,
        selected: r.selected,
        focused: r.focused,
    }
}
```

NOTA de tipos: `panel-scroll-y` es `length` en Slint → en Rust es `slint::LogicalLength`.
Si el setter generado espera `f32`, ajustar `set_panel_scroll_y` al tipo que pida el
compilador. Si `Model` import queda sin usar, quitarlo.

- [ ] **Step 4: Build + arreglar lo que el compilador marque**

Run (PowerShell): `Stop-Process -Name naygo-slint -Force -ErrorAction SilentlyContinue; cargo build -p naygo-ui-slint 2>&1 | Select-String "error\[|error:|warning:|Finished" | Select-Object -First 30`
Expected: `Finished`. Resolver errores de API de Slint según el mensaje (nombres de
setters generados, tipo de `scroll-y`, `pressed-event.modifiers`). Si `row-clicked` con
modificadores no compila (la API de `TouchArea` no expone modifiers del clic), cambiar el
callback a `row-clicked(int)` y en el controller usar `select_single` (Ctrl/Shift+clic se
implementan en una tarea de pulido posterior cuando se resuelva la API); documentarlo.

- [ ] **Step 5: Puertas + commit**

Run (PowerShell): `cargo test --workspace 2>&1 | Select-String "test result:|FAILED" | Select-Object -First 8; cargo clippy --workspace --all-targets -- -D warnings 2>&1 | Select-String "error|warning:|Finished" | Select-Object -Last 3; cargo fmt --all`
Expected: tests `ok` (incluidos los de `naygo-ui` viejo, intacto), clippy limpio.

```bash
git add crates/ui-slint/src
git commit -F - <<'EOF'
feat(slint): listado async (Timer drena por lotes) + controller navegable

listing: worker del core + poll que drena el canal sin bloquear. controller: posee el
FilePaneState, traduce gestos (clic/doble clic/orden/teclado via keymap) a llamadas del
core y reconstruye el modelo. main: cablea callbacks de Slint + Timer de ~30ms que se
apaga al terminar el listado (0 trabajo en reposo). Panel de archivos navegable con
teclado completo de lista y orden por columna.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 6 — Cierre de la Fase 1

- [ ] **Step 1: Pasada final de puertas**

Run (PowerShell): `Stop-Process -Name naygo-slint,naygo -Force -ErrorAction SilentlyContinue; cargo fmt --all --check; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo build -p naygo-ui-slint`
Expected: fmt sin diff, todas las líneas `test result: ok`, clippy limpio, build `Finished`.

- [ ] **Step 2: Verificación viva local (no-regresión funcional)**

Run (PowerShell): `$env:SLINT_BACKEND="winit-software"; cargo run -p naygo-ui-slint`
Criterio: abre en el home; doble clic en carpeta navega; ↑ sube; flechas mueven el foco
(borde de acento), Enter entra; clic selecciona; clic en encabezado ordena (▲/▼);
typeahead salta. Cerrar.

- [ ] **Step 3: Empaquetar build para que Nicolás mida en la VM**

Run (PowerShell): `cargo build -p naygo-ui-slint --release; New-Item -ItemType Directory -Force dist\slint-fase1 | Out-Null; Copy-Item target\release\naygo-slint.exe dist\slint-fase1\`
Crear `dist\slint-fase1\correr-software.cmd` con:
```
@echo off
set SLINT_BACKEND=winit-software
start "" "%~dp0naygo-slint.exe"
```

- [ ] **Step 4: Avisar a Nicolás + memoria**

Resumen: Fase 1 lista (panel navegable en Slint, teclado+orden+selección). Criterio de
rendimiento: mover el mouse en la VM debe mantener la CPU baja como el prototipo.
Actualizar la memoria del proyecto (estado de la migración) y pedir merge/push del branch
`slint-fase1`.

---

## Autoevaluación del plan (hecha)

- Cubre el spec de F1: crate paralelo [T1], tabla virtualizada+columnas [T4], listado
  async timer-drena-lote [T5], navegación (doble clic/subir/Enter) [T5], orden por
  columna [T4/T5], selección clic/Ctrl/Shift [T4/T5], teclado completo de lista vía
  keymap [T3/T5], typeahead [T5], path-bar mínima [T4], render software [T1]. Tests puros
  en bridge [T2] y keys [T3].
- Sin placeholders: código real en cada paso. Las NOTAs marcan puntos donde la API exacta
  de Slint 1.16 se confirma contra el compilador (nombres de Key, modifiers del clic,
  tipo de scroll-y) — son ajustes acotados, no huecos de diseño.
- Consistencia de tipos: `PlainRow`/`rows_from_view` (T2) usados por `controller` (T5) y
  `to_row_data` (T5); `chord_from(text,ctrl,shift,alt)` (T3) usado por `on_key` (T5);
  `RowData` (types.slint, T2) usado por file-panel (T4) y main (T5); `ROW_HEIGHT` y
  `focus_scroll_y` consistentes con `panel-scroll-y` (T4/T5).
- Riesgo conocido y mitigado: Ctrl/Shift+clic depende de que `TouchArea` exponga los
  modifiers del clic en 1.16; si no, F1 degrada a clic simple y se resuelve en pulido
  (la selección por TECLADO —Shift/Ctrl+flechas— sí queda completa vía keymap).
- `naygo-ui` (egui) NO se toca: sigue compilando; las puertas del workspace lo verifican.
