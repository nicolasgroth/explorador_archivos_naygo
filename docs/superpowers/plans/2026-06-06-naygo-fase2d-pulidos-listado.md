# Naygo — Fase 2D: Pulidos de listado (plan de implementación)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mostrar la fila `..` como una entrada de directorio normal, capturar la fecha de creación de cada archivo, agregar los criterios de orden Extensión y Fecha-de-creación al core, y hacer que los encabezados de las 3 columnas fijas (Nombre/Tamaño/Modificado) sean clicables para ordenar (toggle asc/desc con indicador).

**Architecture:** `naygo-core` gana `Entry.created` + `SortKey::{Extension, Created}` + sus brazos en `sort_entries` (puro, testeado), y `listing` captura `metadata.created()`. `naygo-ui` pinta la fila `..` con el ícono de carpeta normal y el mismo `icon_row` que las entries, y convierte los encabezados de columna en widgets clicables que mutan el `SortSpec` del panel + re-ordenan en memoria. La decisión "clic en columna X → nuevo SortSpec" se extrae a una función pura testeable.

**Tech Stack:** Rust, `naygo-core`, `eframe`/`egui` 0.34.3. Sin dependencias nuevas (`std::fs::Metadata::created()` ya está en std).

**Estado de partida (2C-i, en `main`/rama base):**
- `naygo-core::fs_model`: `Entry { name, path, kind, size: Option<u64>, modified: Option<SystemTime>, hidden: bool }`; `SortKey { Name, Size, Modified, Kind }`; `SortSpec { key, ascending, dirs_first }`.
- `sort::sort_entries(&mut [Entry], &SortSpec)`: match con `Name`/`Size`/`Modified`/`Kind`, respeta `dirs_first`, estable (`sort_by`).
- `listing::entry_from_dirent(&DirEntry) -> Entry`: ya captura `name`, `path`, `kind`, `size` (solo files), `modified = metadata.modified().ok()`, `hidden: false`.
- `ui::panes::file_panel::show(ui, workspace, id, pending, icons, show_parent_entry, i18n)`: pinta una fila `..` (con `IconKey::ParentDir`) cuando hay padre y la opción está on; los encabezados son `ui.strong(i18n.t("col.name"/"col.size"/"col.modified"))` (NO clicables); cada entry se pinta con `icon_row(ui, icons, icon_key_for(entry), &entry.name, selected)`.
- `IconKey::{Folder, ParentDir, File(..), Drive(..), Unknown}` en core; `icons.texture(key)` en ui.
- i18n: `i18n.t(key)`; claves en `crates/core/src/i18n/{es,en}.json`.
- **`Entry` se construye con literales en tests** de: `fs_model.rs`, `sort.rs`, `icon_kind.rs`, `listing.rs` (vía `entry_from_dirent`, no literal). Agregar `created` a `Entry` ROMPE esos literales → hay que actualizarlos.

**Prerequisito:** toolchain Rust en PATH. PowerShell: prepend `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. Nunca `2>&1` con cargo. Correr con `--bin naygo`. Verificar `$LASTEXITCODE`.

**Alcance:** ENTRA: `Entry.created` + captura en listing; `SortKey::Extension`/`Created` + sort; encabezados de las 3 columnas fijas clicables (toggle + indicador ▲/▼); función pura clic→SortSpec; fila `..` con ícono de carpeta normal; i18n de lo nuevo. NO ENTRA: columnas configurables / filtro / menú estilo Excel (sub-fase 2E); árbol real; watcher; temas (2C-ii); UI para Extension/Created (existen en core, las expone 2E).

---

## Estructura de archivos

```
crates/core/src/
├── fs_model.rs            # + Entry.created; + SortKey::{Extension, Created}
├── sort.rs                # + brazos Extension/Created
├── listing.rs             # + metadata.created() en entry_from_dirent
└── ... (tests con literales de Entry: actualizar a incluir created: None)

crates/ui/src/
├── sort_ui.rs             # NUEVO: next_sort_on_header_click(spec, key) -> SortSpec (puro)
├── panes/file_panel.rs    # fila ".." con IconKey::Folder; encabezados clicables
├── main.rs                # + mod sort_ui;
└── ...
```

---

## Task 1: `Entry.created` + actualizar constructores de test

**Files:**
- Modify: `crates/core/src/fs_model.rs`
- Modify: test constructors in `crates/core/src/sort.rs`, `crates/core/src/icon_kind.rs` (and any other `Entry { ... }` literal)
- Test: existing tests must still pass

- [ ] **Step 1: Añadir el campo `created` a `Entry`**

Modify `crates/core/src/fs_model.rs` — en el struct `Entry`, tras `modified`:

```rust
    /// Fecha de última modificación, si el SO la entrega.
    pub modified: Option<SystemTime>,
    /// Fecha de creación, si el SO la entrega.
    pub created: Option<SystemTime>,
    /// Atributo "oculto" (en Windows). En esta fase se rellena en fases futuras; default false.
    pub hidden: bool,
```

- [ ] **Step 2: Actualizar el constructor de test en fs_model.rs**

En el `#[cfg(test)]` de `fs_model.rs`, el test `entry_directory_es_dir` construye un `Entry { ... }` literal. Añade `created: None,` (tras `modified: None,`). Busca TODOS los `Entry {` en ese archivo y agrégales `created: None,`.

- [ ] **Step 3: Actualizar los constructores de test en sort.rs e icon_kind.rs**

En `crates/core/src/sort.rs`, la helper `fn entry(name, kind, size)` construye un `Entry { ... }` literal — añade `created: None,` (tras `modified: None,`).

En `crates/core/src/icon_kind.rs`, las helpers/tests que construyen `Entry { ... }` (p. ej. `fn file(...)` y los `Entry {` inline en `icon_key_de_carpeta_y_archivo` / `icon_key_de_other_es_unknown`) — añade `created: None,` a cada uno.

Busca con Grep `Entry {` en `crates/core/src` para no dejar ninguno: cada literal necesita el campo nuevo.

- [ ] **Step 4: Verificar que compila y los tests pasan**

Run: `cargo test -p naygo-core` → expect ALL pass (los literales actualizados compilan; el comportamiento no cambió).
Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

NOTA: `listing::entry_from_dirent` también construye un `Entry` (no en test) — se actualiza en la Tarea 3. Para que ESTA tarea compile, en `entry_from_dirent` añade `created: None,` provisionalmente (la Tarea 3 lo reemplaza por la captura real). Si no lo agregas aquí, `naygo-core` no compila. Hazlo: en `entry_from_dirent`, en el `Entry { ... }` final, añade `created: None,`.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/fs_model.rs crates/core/src/sort.rs crates/core/src/icon_kind.rs crates/core/src/listing.rs
git commit -m "feat(core): Entry.created (fecha de creación); actualizar constructores

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `SortKey::Extension` y `SortKey::Created` + sort

**Files:**
- Modify: `crates/core/src/fs_model.rs` (SortKey)
- Modify: `crates/core/src/sort.rs`
- Test: ampliar `#[cfg(test)]` de `sort.rs`

- [ ] **Step 1: Añadir las variantes a `SortKey`**

Modify `crates/core/src/fs_model.rs` — el enum `SortKey`:

```rust
/// Clave por la que se ordena un panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortKey {
    Name,
    Extension,
    Size,
    Modified,
    Created,
    Kind,
}
```

- [ ] **Step 2: Implementar los brazos en `sort_entries` (TDD: tests primero)**

Ampliar el `#[cfg(test)] mod tests` de `sort.rs` con (usa la helper `entry(name, kind, size)` existente — recuerda que ya tiene `created: None`; para el test de created necesitarás construir entries con `created` distinto, así que añade una helper o construye literales con `created`):

```rust
    #[test]
    fn por_extension_ascendente() {
        let mut v = vec![
            entry("z.txt", EntryKind::File, 1),
            entry("a.zip", EntryKind::File, 1),
            entry("m.jpg", EntryKind::File, 1),
        ];
        let spec = SortSpec { key: SortKey::Extension, ascending: true, dirs_first: false };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        // jpg < txt < zip
        assert_eq!(names, vec!["m.jpg", "z.txt", "a.zip"]);
    }

    #[test]
    fn por_extension_case_insensitive() {
        let mut v = vec![
            entry("b.TXT", EntryKind::File, 1),
            entry("a.zip", EntryKind::File, 1),
        ];
        let spec = SortSpec { key: SortKey::Extension, ascending: true, dirs_first: false };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        // txt < zip, sin importar mayúsculas
        assert_eq!(names, vec!["b.TXT", "a.zip"]);
    }

    #[test]
    fn por_creacion_descendente() {
        use std::time::{Duration, SystemTime};
        let base = SystemTime::UNIX_EPOCH;
        let mut older = entry("viejo", EntryKind::File, 1);
        older.created = Some(base);
        let mut newer = entry("nuevo", EntryKind::File, 1);
        newer.created = Some(base + Duration::from_secs(100));
        let mut v = vec![older, newer];
        let spec = SortSpec { key: SortKey::Created, ascending: false, dirs_first: false };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        // descendente: el más nuevo primero
        assert_eq!(names, vec!["nuevo", "viejo"]);
    }
```

- [ ] **Step 3: Correr los tests y verlos fallar**

Run: `cargo test -p naygo-core sort`
Expected: los 3 nuevos fallan a compilar o por aserción (porque `sort_entries` aún no maneja Extension/Created — el match no es exhaustivo → ERROR de compilación: "non-exhaustive patterns"). Eso confirma que falta implementarlos.

- [ ] **Step 4: Implementar los brazos**

Modify `crates/core/src/sort.rs` — en el `match spec.key`, añadir los brazos (y una helper `cmp_extension`):

```rust
        let ordering = match spec.key {
            SortKey::Name => cmp_name(a, b),
            SortKey::Extension => cmp_extension(a, b),
            SortKey::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Created => a.created.cmp(&b.created),
            SortKey::Kind => format!("{:?}", a.kind).cmp(&format!("{:?}", b.kind)),
        };
```

Y añadir la helper (junto a `cmp_name`):

```rust
/// Comparación por extensión del path, case-insensitive. Sin extensión = vacío.
fn cmp_extension(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    let ext = |e: &Entry| {
        e.path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
    };
    ext(a).cmp(&ext(b))
}
```

- [ ] **Step 5: Correr los tests — pasan**

Run: `cargo test -p naygo-core sort` → expect ALL pass (los previos + 3 nuevos).
Run: `cargo test -p naygo-core` → todo verde.
Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/fs_model.rs crates/core/src/sort.rs
git commit -m "feat(core): ordenar por extensión y fecha de creación

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `listing` captura `created`

**Files:**
- Modify: `crates/core/src/listing.rs`

- [ ] **Step 1: Capturar `metadata.created()` en `entry_from_dirent`**

Modify `crates/core/src/listing.rs` — en `entry_from_dirent`, donde se calcula `modified`, añadir `created`. Buscar la línea `let modified = metadata.as_ref().and_then(|m| m.modified().ok());` (o similar) y agregar análogo:

```rust
    let modified = metadata.as_ref().and_then(|m| m.modified().ok());
    let created = metadata.as_ref().and_then(|m| m.created().ok());
```

Y en el `Entry { ... }` final, reemplazar el `created: None,` provisional (de la Tarea 1) por `created,`:

```rust
    Entry {
        name,
        path,
        kind,
        size,
        modified,
        created,
        hidden: false,
    }
```

- [ ] **Step 2: Verificar**

Run: `cargo test -p naygo-core listing` → los tests de listing siguen pasando (no aserciones sobre `created`, pero compila y corre).
Run: `cargo test -p naygo-core` → todo verde.
Run: `cargo clippy -p naygo-core -- -D warnings` → limpio.

NOTA: `Metadata::created()` devuelve `io::Result<SystemTime>`; en algún FS puede no estar soportado → `.ok()` → `None`. Tolerante por diseño.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/listing.rs
git commit -m "feat(core): listing captura la fecha de creación

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Función pura `next_sort_on_header_click` (ui::sort_ui)

**Files:**
- Create: `crates/ui/src/sort_ui.rs`
- Modify: `crates/ui/src/main.rs` (declarar `mod sort_ui;`)
- Test: módulo `#[cfg(test)]` en `sort_ui.rs`

- [ ] **Step 1: Escribir la función pura con tests**

Create `crates/ui/src/sort_ui.rs`:

```rust
// Naygo — lógica pura del ordenamiento por clic en encabezado de columna.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Dado el `SortSpec` actual y la columna (SortKey) que el usuario clicó, devuelve
//! el nuevo `SortSpec`: si clicó la columna ya activa, invierte la dirección; si
//! clicó otra, la activa en ascendente. `dirs_first` se preserva. Puro, testeable.

use naygo_core::fs_model::{SortKey, SortSpec};

/// Calcula el nuevo `SortSpec` al clicar el encabezado de `clicked`.
pub fn next_sort_on_header_click(current: SortSpec, clicked: SortKey) -> SortSpec {
    if current.key == clicked {
        SortSpec {
            ascending: !current.ascending,
            ..current
        }
    } else {
        SortSpec {
            key: clicked,
            ascending: true,
            dirs_first: current.dirs_first,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clic_en_columna_activa_invierte_direccion() {
        let cur = SortSpec { key: SortKey::Name, ascending: true, dirs_first: true };
        let next = next_sort_on_header_click(cur, SortKey::Name);
        assert_eq!(next.key, SortKey::Name);
        assert!(!next.ascending, "invierte a descendente");
        assert!(next.dirs_first, "preserva dirs_first");
    }

    #[test]
    fn clic_en_columna_activa_descendente_vuelve_a_ascendente() {
        let cur = SortSpec { key: SortKey::Size, ascending: false, dirs_first: false };
        let next = next_sort_on_header_click(cur, SortKey::Size);
        assert!(next.ascending);
    }

    #[test]
    fn clic_en_otra_columna_la_activa_ascendente() {
        let cur = SortSpec { key: SortKey::Name, ascending: false, dirs_first: true };
        let next = next_sort_on_header_click(cur, SortKey::Modified);
        assert_eq!(next.key, SortKey::Modified);
        assert!(next.ascending, "nueva columna arranca ascendente");
        assert!(next.dirs_first, "preserva dirs_first");
    }
}
```

- [ ] **Step 2: Declarar el módulo y correr los tests**

Modify `crates/ui/src/main.rs` — añadir `mod sort_ui;` junto a los otros `mod`.

Run: `cargo test -p naygo-ui sort_ui` → expect 3 tests PASS.

NOTA: `next_sort_on_header_click` quedará sin usar hasta la Tarea 5 → posible dead-code. Es `pub fn` en un crate binario, lo que SÍ dispara dead_code bajo `-D warnings`. Si `cargo clippy -p naygo-ui -- -D warnings` se queja, añade `#[allow(dead_code)]` a la fn con comentario "consumido en Tarea 5", y reporta; se quita en la Tarea 5. (O implementa Tareas 4 y 5 juntas.)

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src/sort_ui.rs crates/ui/src/main.rs
git commit -m "feat(ui): función pura clic-en-encabezado → nuevo SortSpec

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: file_panel — fila `..` normal + encabezados clicables

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/ui/src/sort_ui.rs` (quitar el allow si se puso)
- Modify: `crates/core/src/i18n/{es,en}.json` (indicadores de orden, si se usan textos)

- [ ] **Step 1: Fila `..` con ícono de carpeta normal**

Modify `crates/ui/src/panes/file_panel.rs` — en el bloque que pinta la fila `..`,
cambiar `IconKey::ParentDir` por `IconKey::Folder` para que use el ícono de carpeta
normal. El resto de esa fila (es la primera, usa `icon_row`, emite NavigateTo al
padre con un clic) se mantiene. Antes:
```rust
                if parent.is_some() {
                    let resp = icon_row(ui, icons, IconKey::ParentDir, "..", false);
```
Después:
```rust
                if parent.is_some() {
                    // ".." se ve como una carpeta normal (ícono de carpeta, mismo
                    // estilo de fila). Estilo Total Commander.
                    let resp = icon_row(ui, icons, IconKey::Folder, "..", false);
```
(El comentario sobre el clic-simple que ya existe se mantiene.)

- [ ] **Step 2: Encabezados clicables que ordenan**

Modify `crates/ui/src/panes/file_panel.rs` — los encabezados actuales son
`ui.strong(i18n.t("col.name"))` etc. (no clicables). Reemplázalos por encabezados
clicables que muestren el indicador de dirección y, al clicar, calculen el nuevo
SortSpec y re-ordenen.

Primero, ANTES del `ScrollArea`/`Grid`, lee el sort actual del panel:
```rust
    let sort = f.sort; // SortSpec es Copy
```
(`f` es el `&FilePaneState` ya obtenido al inicio de `show`.)

Dentro del `Grid::show`, reemplaza la fila de encabezados. Necesitas detectar el
clic en cada encabezado y, fuera del closure de la Grid, aplicar el cambio. Patrón:
acumula `header_clicked: Option<SortKey>` (como ya se hace con `clicked`/`activated`).

```rust
    let mut header_clicked: Option<naygo_core::fs_model::SortKey> = None;
```
(declarar junto a `clicked`/`activated`/`parent_activated`).

En la fila de encabezados de la Grid:
```rust
                use naygo_core::fs_model::SortKey;
                if header_label(ui, i18n.t("col.name"), sort, SortKey::Name).clicked() {
                    header_clicked = Some(SortKey::Name);
                }
                if header_label(ui, i18n.t("col.size"), sort, SortKey::Size).clicked() {
                    header_clicked = Some(SortKey::Size);
                }
                if header_label(ui, i18n.t("col.modified"), sort, SortKey::Modified).clicked() {
                    header_clicked = Some(SortKey::Modified);
                }
                ui.end_row();
```

Añade la helper `header_label` (al final del archivo, junto a las otras fns):
```rust
/// Pinta un encabezado de columna clicable con indicador de dirección si es el
/// criterio activo. Devuelve el `Response` (clic ordena por esa columna).
fn header_label(
    ui: &mut egui::Ui,
    title: &str,
    sort: naygo_core::fs_model::SortSpec,
    key: naygo_core::fs_model::SortKey,
) -> egui::Response {
    let text = if sort.key == key {
        let arrow = if sort.ascending { " ▲" } else { " ▼" };
        format!("{title}{arrow}")
    } else {
        title.to_string()
    };
    // selectable_label clicable; resaltado leve si es la columna activa.
    ui.selectable_label(sort.key == key, egui::RichText::new(text).strong())
}
```

Después del `ScrollArea`/`Grid` (junto a donde se procesan `clicked`/`activated`),
aplica el cambio de orden:
```rust
    if let Some(key) = header_clicked {
        let new_spec = crate::sort_ui::next_sort_on_header_click(sort, key);
        if let Some(f) = workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
            f.sort = new_spec;
            let spec = f.sort;
            naygo_core::sort::sort_entries(&mut f.entries, &spec);
        }
    }
```
NOTA: `sort_entries` se importa o se llama con path completo. Asegúrate de tener
`use naygo_core::sort::sort_entries;` o usar el path completo como arriba.

- [ ] **Step 3: Quitar el allow de sort_ui (si se puso en Tarea 4)**

Si en la Tarea 4 quedó `#[allow(dead_code)]` en `next_sort_on_header_click`, quítalo
ahora que file_panel lo consume.

- [ ] **Step 4: Compilar, verificar, formatear**

Run: `cargo build -p naygo-ui` → compila. Reporta warnings. Cuida los borrows: `sort`
es `Copy` (se lee a un local antes de los closures); `header_clicked` se procesa tras
la Grid con `workspace.pane_mut` (igual patrón que `clicked`).
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo test --workspace` → verde.
Run: `cargo fmt`.

App-start (`--bin naygo`): los encabezados Nombre/Tamaño/Modificado son clicables;
clic ordena (▲), segundo clic invierte (▼); la columna activa muestra la flecha; la
fila `..` se ve con ícono de carpeta normal (igual que las demás carpetas).

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/panes/file_panel.rs crates/ui/src/sort_ui.rs
git commit -m "feat(ui): fila '..' como carpeta normal + encabezados clicables para ordenar

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Cierre de fase — README, verificación final, push

**Files:**
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: Actualizar README**

Modify `README.md` — bloque de estado:
```markdown
> **Estado:** Fase 2D (pulidos de listado) en desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-06-naygo-fase2d-pulidos-listado-design.md`](docs/superpowers/specs/2026-06-06-naygo-fase2d-pulidos-listado-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-06-naygo-fase2d-pulidos-listado.md`](docs/superpowers/plans/2026-06-06-naygo-fase2d-pulidos-listado.md).
> Fases 1, 2A, 2B, 2C-i completas.
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compila.
Run: `cargo test --workspace` → todo verde (core: ... + sort con extensión/creación; ui: ... + sort_ui 3).
Run: `cargo clippy --workspace -- -D warnings` → limpio.
Run: `cargo fmt --check` → limpio (si no, fmt + incluir).
Run: `cargo build --release -p naygo-ui` → release compila.
App-start manual (`--bin naygo`): fila `..` como carpeta normal; clic en encabezados ordena con flecha; el criterio persiste al reabrir.

- [ ] **Step 3: Commit y push**

```bash
git add README.md
git commit -m "chore: actualizar estado del README a Fase 2D

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/fase2d-pulidos-listado
```

---

## Self-review (cobertura del spec)

| Requisito del spec 2D | Tarea(s) |
|---|---|
| `Entry.created` + captura en listing | 1 (campo) + 3 (captura) |
| `SortKey::Extension` + sort | 2 |
| `SortKey::Created` + sort | 2 |
| Encabezados de las 3 columnas fijas clicables | 5 |
| Toggle asc/desc + indicador ▲/▼ | 5 (header_label) + 4 (next_sort) |
| Función pura clic→SortSpec | 4 |
| Re-ordenar en memoria tras clic (sin re-listar) | 5 |
| Fila `..` con ícono de carpeta normal | 5 |
| SortSpec persistido por panel | ya en FilePanePersist (sin cambio) |
| Extension/Created en core, sin UI en 2D | 2 (core) — UI en 2E |
| Tolerancia (created None, extensión vacía) | 2 (cmp_extension vacío) + 3 (.ok()) |

**Diferido (NO en 2D):** columnas configurables / filtro / estilo Excel (2E); árbol
real; watcher; temas (2C-ii). 

**Notas de riesgo:**
- Agregar `Entry.created` rompe TODOS los literales `Entry { ... }` en tests (Tarea 1
  los actualiza; grep `Entry {` en crates/core/src para no dejar ninguno). Y el
  `entry_from_dirent` no-test necesita `created: None` provisional en Tarea 1, real
  en Tarea 3.
- Agregar variantes a `SortKey` hace NO-exhaustivo el match de `sort_entries` →
  error de compilación hasta implementar los brazos (Tarea 2). Esperado (TDD).
- Borrows en file_panel: `sort` es `Copy` (leer a local); `header_clicked` se procesa
  tras la Grid vía `workspace.pane_mut` (mismo patrón ya usado para `clicked`).
- `header_label` usa `selectable_label` con `RichText::strong` — confirmar que
  compila en egui 0.34.3 (ambos existen).
```
