# watcher — Vigilar la carpeta visible + detectar dispositivos — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reflejar en vivo (sin refresco manual) las altas/bajas/cambios de las carpetas visibles, resaltando lo nuevo; y reaccionar al instante a pendrives enchufados/quitados.

**Architecture:** Dos subsistemas aislados con el patrón "worker→canal→pump": (W) `platform::dir_watch` envuelve el crate `notify` (debounce ~300ms) y emite `DirEvent`s que `core::listing::apply_dir_events` (puro) fusiona al `Vec<Entry>`; lo nuevo se marca en `FilePaneState.highlighted` y se tiñe con un token de tema. (D) `platform::device_watch` corre una ventana message-only Win32 en su propio hilo que escucha `WM_DEVICECHANGE` y dispara el `start_disk_scan()` de shell-A. El resaltado es estado de presentación; `Entry` no se toca.

**Tech Stack:** Rust, `naygo-core`/`naygo-platform`/`naygo-ui`, `eframe`/`egui` 0.34.3, crate `notify` 6/7, crate `windows` 0.62, `std::thread`/`mpsc`. Sin chrono.

**Estado de partida (rama `feat/watcher`, desde `main` con ops-A/ops-B/paste/shell-A):**
- `naygo_core::fs_model`: `Entry { name: String, path: PathBuf, kind: EntryKind, size: Option<u64>, modified: Option<SystemTime>, created: Option<SystemTime>, hidden: bool }`. `EntryKind { Directory, File, Other }`. NO se modifica.
- `naygo_core::listing`: `spawn_listing`/`spawn_listing_filtered` (worker pattern). `fn entry_from_dirent(dirent: &std::fs::DirEntry) -> Entry` (privado): construye un Entry tolerando metadata ausente (kind por is_dir/is_file, size solo para File, modified/created por metadata, hidden:false). Esta lógica se extrae a `entry_from_path(&Path, Option<&Metadata>)` compartido.
- `naygo_core::workspace::FilePaneState { current_dir, entries: Vec<Entry>, sort: SortSpec, view: ViewMode, focused: Option<usize>, selected: Vec<usize>, history, show_dirs: bool, table: TableState }`. `view_indices(&self) -> Vec<usize>` (file_pane.rs:70): si `table.filters.is_empty()` → `0..len`, si no → filtra con `filter_matches`. `focused_view_entry`. `navigate_to`/`enter`. (Verificar si FilePaneState deriva Serialize/Deserialize y cómo se persiste en el workspace.)
- `naygo_core::sort::sort_entries(entries: &mut [Entry], spec: &SortSpec)`. Se llama en `app.rs:pump_one` (~503) tras listar y en SetSort (~1938).
- `naygo_core::theme::Theme { name, base, accent, panel_bg, row_bg, row_alt_bg, text, text_dim, selection_bg, active_bar, error, border }` (todos `ThemeColor`). Serialize MANUAL (theme/mod.rs ~106: `st.serialize_field(...)` por campo). Deserialize TOLERANTE: un `RawTheme` con `Option<ThemeColor>` por campo + `Theme::from(raw)` que hace `raw.X.unwrap_or(def.X)` con `def = Theme::defaults_for(base, name)`. `defaults_for` tiene un bloque literal por tema embebido (Dark Blue ~180, Dark Teal ~194, Light, High Contrast). `ThemeColor::new(r,g,b)` y `from_hex`.
- `naygo_ui::theme_apply::ActiveTheme`: getters `accent()`, `active_bar()`, `selection_bg()`, `text_dim()`, `error()` → `egui::Color32`. `to_color32(ThemeColor)`.
- `naygo_ui::app::NaygoApp`: tiene `listings: HashMap<PaneId, ...>`, `trees: HashMap<PaneId, DirTree>` (patrón a imitar para `watchers`). `disk_usage`/`disk_rx`/`disk_scan_ticks` + `start_disk_scan()` (shell-A). pump loop (~1626): `pump_all/pump_tree/pump_ops/pump_paste_write/pump_disk_usage`. `start_listing(id, dir)`. `pump_one` re-sortea entries. `handle_input`. `config_dir`, `settings`, `i18n`, `status`.
- `naygo_platform`: patrón Win32 `#[cfg(windows)]`/`#[cfg(not(windows))]` (trash/clipboard/open/drive_space/drives). Cargo `[target.'cfg(windows)'.dependencies] windows = { workspace=true, features=[...] }` con `Win32_UI_WindowsAndMessaging` (añadido en shell-A), `Win32_Foundation`, `Win32_System_Com`, etc. `windows`=0.62. `[dependencies] naygo-core, tracing`.
- `naygo_core::config`: `Settings` con patrón aditivo `#[serde(default = "fn")]` + `fn default_x()`; manual `impl Default` (líneas ~127). `CONFIG_VERSION = 1` (NO cambia). `ImageFmt`/`ops_*`/`paste_*` ya viven aquí.
- i18n: `crates/core/src/i18n/{es,en}.json` planos; parity test.

**Prerequisito:** Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo. `cargo fmt --all -- --check`. Binario `--bin naygo`. Bash: NO `cd /d`.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. Header de 2 líneas en archivos NUEVOS. `core` NUNCA importa egui/windows (notify NO va en core — va en platform; core solo tiene tipos puros). UI nunca bloquea (watchers en workers). Tolerante (SO hostil, sin panics). Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt antes de cada commit. SIEMPRE `cargo fmt --all` antes de commitear. Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/watcher`. NO cambiar de rama.

**SECUENCIA:** Tasks 1-5 son core puro (self-contained). Tasks 6-7 platform (módulos nuevos; device_watch es el más arriesgado). Tasks 8-10 UI. Task 11 cierre. Core/platform no rompen el árbol entre tareas (son additivos). La UI integra al final.

**Alcance:** ENTRA: dir_watch (notify), device_watch (Win32), apply_dir_events, token highlight, highlighted en FilePaneState, Settings de watcher, UI (watchers por panel + pump + device + render + settings), i18n. NO ENTRA: vigilancia recursiva, pausar al minimizar, shell-B.

---

## Estructura de archivos

```
crates/core/src/
├── listing.rs       # + apply_dir_events + EntryMeta + entry_from_path (extraído); DirEvent vive aquí (tipo puro)
├── theme/mod.rs     # + token highlight (Serialize + deserialize tolerante + 4 temas)
├── workspace/file_pane.rs  # + highlighted: HashSet<PathBuf> (#[serde(skip)]) + clear_highlight/is_highlighted; "al final" en view_indices
├── config/mod.rs    # + HighlightDuration + new_items_at_end
└── i18n/{es,en}.json

crates/platform/src/
├── dir_watch.rs     # NUEVO: notify → DirEvent (coalesce ~300ms) + WatchHandle
├── device_watch.rs  # NUEVO: ventana message-only Win32 → DeviceEvent + stub
├── lib.rs           # + pub mod dir_watch; pub mod device_watch;
└── Cargo.toml       # + notify; + features windows (Devices_*, etc.)

crates/ui/src/
├── app.rs           # watchers HashMap<PaneId,WatchHandle> + pump_watchers + pump_devices + device_watch al arrancar + limpieza highlight
├── panes/file_panel.rs  # render estilo A (fondo highlight + nombre)
├── theme_apply.rs   # getter highlight()
└── settings_window/  # opciones watcher
```

NOTE sobre `DirEvent`: es un tipo de datos PURO (sin notify ni windows), así que vive en `core` (`core::listing` o un `core::watch` chico) para que `apply_dir_events` (core) lo consuma y `platform::dir_watch` (platform) lo produzca. Ponerlo en core evita que core dependa de platform. Decisión: `DirEvent` + `EntryMeta` en `core::listing`.

---

## Task 1: `core` — DirEvent + EntryMeta + entry_from_path extraído

**Files:**
- Modify: `crates/core/src/listing.rs`

- [ ] **Step 1: Test (TDD) de entry_from_path**

En el `#[cfg(test)] mod tests` de listing.rs, añadir:
```rust
    #[test]
    fn entry_from_path_archivo() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.txt");
        std::fs::write(&f, b"hola").unwrap(); // 4 bytes
        let meta = std::fs::metadata(&f).ok();
        let e = entry_from_path(&f, meta.as_ref());
        assert_eq!(e.name, "a.txt");
        assert_eq!(e.kind, EntryKind::File);
        assert_eq!(e.size, Some(4));
    }

    #[test]
    fn entry_from_path_carpeta() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let meta = std::fs::metadata(&sub).ok();
        let e = entry_from_path(&sub, meta.as_ref());
        assert_eq!(e.kind, EntryKind::Directory);
        assert_eq!(e.size, None);
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core listing::tests::entry_from_path` → ERROR: `entry_from_path` no existe.

- [ ] **Step 3: Extraer entry_from_path + reusar en entry_from_dirent**

En `crates/core/src/listing.rs`, añadir `use std::fs::Metadata;` si falta. Añadir:
```rust
/// Construye un `Entry` desde una ruta + su metadata (ya leída). Tolerante a metadata
/// ausente. Compartido por el listado inicial y por el merge incremental del watcher.
pub fn entry_from_path(path: &std::path::Path, metadata: Option<&Metadata>) -> Entry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let kind = match metadata {
        Some(m) if m.is_dir() => EntryKind::Directory,
        Some(m) if m.is_file() => EntryKind::File,
        Some(_) => EntryKind::Other,
        None => EntryKind::Other,
    };
    let size = match (metadata, kind) {
        (Some(m), EntryKind::File) => Some(m.len()),
        _ => None,
    };
    let modified = metadata.and_then(|m| m.modified().ok());
    let created = metadata.and_then(|m| m.created().ok());
    Entry { name, path: path.to_path_buf(), kind, size, modified, created, hidden: false }
}
```
Y reescribir `entry_from_dirent` para delegar:
```rust
fn entry_from_dirent(dirent: &std::fs::DirEntry) -> Entry {
    let path = dirent.path();
    let metadata = dirent.metadata().ok();
    entry_from_path(&path, metadata.as_ref())
}
```

- [ ] **Step 4: DirEvent + EntryMeta (tipos puros)**

Añadir a listing.rs (cerca del tope, tras los `use`):
```rust
use std::path::PathBuf;

/// Cambio normalizado en una carpeta vigilada (producido por `platform::dir_watch`,
/// consumido por `apply_dir_events`). Tipo puro: sin notify ni Windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirEvent {
    Created(PathBuf),
    Removed(PathBuf),
    Modified(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}
```
(`EntryMeta` no es necesario como struct aparte: `apply_dir_events` usará un `read_meta: &dyn Fn(&Path) -> Option<Entry>` que devuelve un Entry ya construido — más simple que un EntryMeta intermedio. Ver Task 2. Si prefieres EntryMeta, defínelo; pero el plan usa `Fn(&Path) -> Option<Entry>` directamente, reusando `entry_from_path` en la UI.)

- [ ] **Step 5: Correr — pasan**

Run: `cargo test -p naygo-core listing` → todos PASS (existentes + 2 nuevos).
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 6: Commit**
```
git add crates/core/src/listing.rs
git commit -m "feat(core): entry_from_path compartido + tipo DirEvent

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `entry_from_path(&Path, Option<&Metadata>) -> Entry` y `DirEvent` EXACTOS (Tasks 2, 6, 8 dependen).

---

## Task 2: `core::listing::apply_dir_events` — merge incremental puro

**Files:**
- Modify: `crates/core/src/listing.rs`

- [ ] **Step 1: Tests (TDD)**

Añadir al `mod tests`:
```rust
    fn mk_entry(name: &str, dir: &std::path::Path) -> Entry {
        Entry {
            name: name.into(),
            path: dir.join(name),
            kind: EntryKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
        }
    }

    #[test]
    fn apply_created_inserta_y_reporta_nuevo() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("a.txt", dir)];
        let newp = dir.join("b.txt");
        let np = newp.clone();
        let read = move |p: &std::path::Path| {
            if p == np { Some(mk_entry("b.txt", dir)) } else { None }
        };
        let nuevas = apply_dir_events(&mut entries, &[DirEvent::Created(newp.clone())], &read);
        assert_eq!(entries.len(), 2);
        assert_eq!(nuevas, vec![newp]);
    }

    #[test]
    fn apply_created_idempotente() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("a.txt", dir)];
        let p = dir.join("a.txt");
        let read = |_: &std::path::Path| Some(mk_entry("a.txt", dir));
        let nuevas = apply_dir_events(&mut entries, &[DirEvent::Created(p)], &read);
        assert_eq!(entries.len(), 1, "no duplica algo ya presente");
        assert!(nuevas.is_empty());
    }

    #[test]
    fn apply_removed_quita() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("a.txt", dir), mk_entry("b.txt", dir)];
        let read = |_: &std::path::Path| None;
        apply_dir_events(&mut entries, &[DirEvent::Removed(dir.join("a.txt"))], &read);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "b.txt");
    }

    #[test]
    fn apply_modified_actualiza_size() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("a.txt", dir)];
        let mut updated = mk_entry("a.txt", dir);
        updated.size = Some(999);
        let read = move |_: &std::path::Path| Some(updated.clone());
        apply_dir_events(&mut entries, &[DirEvent::Modified(dir.join("a.txt"))], &read);
        assert_eq!(entries[0].size, Some(999));
    }

    #[test]
    fn apply_renamed_renombra() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![mk_entry("old.txt", dir)];
        let read = |_: &std::path::Path| None;
        let nuevas = apply_dir_events(
            &mut entries,
            &[DirEvent::Renamed { from: dir.join("old.txt"), to: dir.join("new.txt") }],
            &read,
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "new.txt");
        assert_eq!(entries[0].path, dir.join("new.txt"));
        assert_eq!(nuevas, vec![dir.join("new.txt")]);
    }

    #[test]
    fn apply_renamed_from_ausente_es_created() {
        let dir = std::path::Path::new("D:/x");
        let mut entries = vec![];
        let read = |_: &std::path::Path| Some(mk_entry("new.txt", dir));
        let nuevas = apply_dir_events(
            &mut entries,
            &[DirEvent::Renamed { from: dir.join("ghost.txt"), to: dir.join("new.txt") }],
            &read,
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(nuevas, vec![dir.join("new.txt")]);
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core listing::tests::apply` → ERROR: `apply_dir_events` no existe.

- [ ] **Step 3: Implementar**

Añadir a listing.rs:
```rust
/// Aplica `events` a `entries` SIN re-listar la carpeta. `read_entry` produce el `Entry`
/// de una ruta (en producción usa el FS vía `entry_from_path`; en tests, un closure).
/// Devuelve las rutas NUEVAS (Created, o Renamed cuyo `from` no estaba) para resaltarlas.
/// El llamador re-ordena/re-filtra la vista después.
pub fn apply_dir_events(
    entries: &mut Vec<Entry>,
    events: &[DirEvent],
    read_entry: &dyn Fn(&std::path::Path) -> Option<Entry>,
) -> Vec<PathBuf> {
    let mut nuevas = Vec::new();
    for ev in events {
        match ev {
            DirEvent::Created(p) => {
                if entries.iter().any(|e| &e.path == p) {
                    continue; // idempotente: ya está
                }
                if let Some(e) = read_entry(p) {
                    entries.push(e);
                    nuevas.push(p.clone());
                }
            }
            DirEvent::Removed(p) => {
                entries.retain(|e| &e.path != p);
            }
            DirEvent::Modified(p) => {
                if let Some(updated) = read_entry(p) {
                    if let Some(slot) = entries.iter_mut().find(|e| &e.path == p) {
                        *slot = updated;
                    }
                }
            }
            DirEvent::Renamed { from, to } => {
                if let Some(slot) = entries.iter_mut().find(|e| &e.path == from) {
                    slot.path = to.clone();
                    slot.name = to
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    nuevas.push(to.clone());
                } else if !entries.iter().any(|e| &e.path == to) {
                    if let Some(e) = read_entry(to) {
                        entries.push(e);
                        nuevas.push(to.clone());
                    }
                }
            }
        }
    }
    nuevas
}
```

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core listing` → todos PASS.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/listing.rs
git commit -m "feat(core): apply_dir_events (merge incremental puro)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `apply_dir_events(&mut Vec<Entry>, &[DirEvent], &dyn Fn(&Path)->Option<Entry>) -> Vec<PathBuf>` EXACTO (Task 8 depende).

---

## Task 3: `core::theme` — token highlight

**Files:**
- Modify: `crates/core/src/theme/mod.rs`

- [ ] **Step 1: Test (TDD) de tolerancia + default por tema**

Añadir al `mod tests` de theme/mod.rs:
```rust
    #[test]
    fn highlight_default_si_falta_en_json() {
        // Un JSON de tema sin "highlight" cae al default del tema (tolerante).
        let json = r#"{"name":"X","base":"Dark","accent":"#2f81f7"}"#;
        let t: Theme = serde_json::from_str(json).unwrap();
        // No paniquea y highlight tiene algún valor (el default dark).
        let _ = t.highlight;
    }

    #[test]
    fn highlight_round_trip() {
        let t = Theme::defaults_for(ThemeBase::Dark, "X".into());
        let json = serde_json::to_string(&t).unwrap();
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(back.highlight, t.highlight);
    }
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core theme::tests::highlight` → ERROR: campo `highlight` no existe.

- [ ] **Step 3: Añadir el campo + Serialize + deserialize tolerante + defaults**

Modify `crates/core/src/theme/mod.rs`:
a) En `struct Theme`, añadir tras `error`: `pub highlight: ThemeColor,` (y antes de `border` si existe — ubícalo de forma consistente).
b) En la Serialize manual (~106), añadir `st.serialize_field("highlight", &self.highlight)?;` (ajustar el conteo de campos si `serialize_struct` recibe un número — incrementarlo en 1).
c) En `RawTheme` (~125), añadir `highlight: Option<ThemeColor>,`.
d) En `Theme::from(raw)` (~143), añadir `highlight: raw.highlight.unwrap_or(def.highlight),`.
e) En CADA bloque de `defaults_for` (los 4 temas), añadir un `highlight` sensato. Para Dark Blue / Dark Teal (oscuros): un verde tenue, p. ej. `highlight: c(0x2e, 0x7d, 0x32)` (verde) o un verde-acento; para Light: `highlight: c(0xc8, 0xe6, 0xc9)` (verde claro); para High Contrast: un verde vivo `c(0x00, 0xc8, 0x00)`. Usar `let c = ThemeColor::new;` como ya hace el archivo. (El render lo usará teñido al ~18% alpha, así que el color base puede ser saturado.)

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core theme` → todos PASS (incluidos los existentes — si algún test del tema compara estructura completa, actualizarlo para incluir highlight).
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/theme/mod.rs
git commit -m "feat(core): token de tema highlight (resaltado de archivos nuevos)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep el nombre `highlight` EXACTO (Tasks 9, 10 dependen).

---

## Task 4: `core::workspace::FilePaneState` — highlighted + "al final" en view_indices

**Files:**
- Modify: `crates/core/src/workspace/file_pane.rs`

- [ ] **Step 1: Tests (TDD)**

Añadir al `mod tests` de file_pane.rs:
```rust
    #[test]
    fn highlighted_set_y_clear() {
        let mut s = FilePaneState::new(std::path::PathBuf::from("D:/x"));
        let p = std::path::PathBuf::from("D:/x/a.txt");
        s.highlighted.insert(p.clone());
        assert!(s.is_highlighted(&p));
        s.clear_highlight();
        assert!(!s.is_highlighted(&p));
    }

    #[test]
    fn view_al_final_mueve_resaltadas_al_fondo() {
        // entries a, b, c (sin filtro); b está resaltada; new_items_at_end → b al final.
        let mut s = FilePaneState::new(std::path::PathBuf::from("D:/x"));
        s.entries = vec![
            entry_named("a"), entry_named("b"), entry_named("c"),
        ];
        s.highlighted.insert(std::path::PathBuf::from("D:/x/b"));
        let normal = s.view_indices();
        assert_eq!(normal, vec![0, 1, 2]); // sin el flag, orden natural
        let al_final = s.view_indices_ordered(true);
        assert_eq!(al_final, vec![0, 2, 1]); // b (idx 1) movida al final, estable
    }
```
(Helper `entry_named` en el test: construye un Entry con path `D:/x/<name>`. `FilePaneState::new` — verificar el constructor real; si no existe, construir el struct directamente con `..` o el `Default` si lo deriva. Ajustar a la realidad.)

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core file_pane::tests::highlighted` → ERROR: `highlighted`/`is_highlighted`/`view_indices_ordered` no existen.

- [ ] **Step 3: Implementar**

Modify `crates/core/src/workspace/file_pane.rs`:
a) Añadir el campo (con `#[serde(default, skip)]` — es estado efímero, NO se persiste):
```rust
    /// Rutas resaltadas como "recién aparecidas" (estado de presentación efímero; no se
    /// persiste). El render las tiñe; la interacción/refresh las limpia.
    #[serde(default, skip)]
    pub highlighted: std::collections::HashSet<std::path::PathBuf>,
```
(VERIFICAR si FilePaneState deriva Serialize/Deserialize. Si NO los deriva, el `#[serde(...)]` sobra — quitarlo. Si los deriva, `skip` es obligatorio para no romper la persistencia del workspace. Si hay un `impl Default` manual o un `new`, inicializar `highlighted: HashSet::new()`.)
b) Métodos:
```rust
    /// ¿Está esta ruta resaltada como nueva?
    pub fn is_highlighted(&self, path: &std::path::Path) -> bool {
        self.highlighted.contains(path)
    }
    /// Limpia todo el resaltado (al interactuar o re-listar).
    pub fn clear_highlight(&mut self) {
        self.highlighted.clear();
    }
```
c) Refactor `view_indices` para soportar "al final" sin romper el llamador actual: renombrar la lógica a un `view_indices_ordered(&self, new_items_at_end: bool)` y dejar `view_indices(&self)` llamando con `false`:
```rust
    pub fn view_indices(&self) -> Vec<usize> {
        self.view_indices_ordered(false)
    }

    /// Índices de la vista (filtrada). Si `new_items_at_end`, las filas resaltadas se
    /// mueven al final de forma ESTABLE (conservando su orden relativo).
    pub fn view_indices_ordered(&self, new_items_at_end: bool) -> Vec<usize> {
        let mut idx: Vec<usize> = if self.table.filters.is_empty() {
            (0..self.entries.len()).collect()
        } else {
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, e)| filter_matches(e, &self.table.filters))
                .map(|(i, _)| i)
                .collect()
        };
        if new_items_at_end && !self.highlighted.is_empty() {
            // Partición estable: no-resaltadas primero, resaltadas al final.
            let hl = &self.highlighted;
            idx.sort_by_key(|&i| hl.contains(&self.entries[i].path));
            // sort_by_key es estable en Rust, así que dentro de cada grupo se preserva
            // el orden original (que ya viene de `entries` ordenado por la columna).
        }
        idx
    }
```
NOTE: `sort_by_key` con clave bool (false<true) es estable → no-resaltadas (false) primero, resaltadas (true) al final, preservando orden interno. Correcto.

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core file_pane` → todos PASS.
Run: `cargo test -p naygo-core` → verde (workspace serde si aplica).
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/workspace/file_pane.rs
git commit -m "feat(core): highlighted en FilePaneState + view 'al final' estable

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `highlighted`, `is_highlighted`, `clear_highlight`, `view_indices_ordered(bool)` EXACTOS (Tasks 8-10 dependen).

---

## Task 5: `core::config` — Settings del watcher

**Files:**
- Modify: `crates/core/src/config/mod.rs`

- [ ] **Step 1: Test (TDD) de round-trip**

En el `mod tests` de config/mod.rs, extender el test de round-trip existente (o añadir uno) para incluir los campos nuevos con valores no-default y verificar que sobreviven serde.

- [ ] **Step 2: Implementar**

Modify `crates/core/src/config/mod.rs`:
a) Definir el enum (cerca de `OpsMode`/`ImageFmt`):
```rust
/// Cuánto dura el resaltado de un archivo recién aparecido.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighlightDuration {
    /// Hasta que el usuario interactúa con el panel (default).
    UntilInteract,
    /// Se desvanece tras N segundos.
    FadeSeconds(u32),
    /// Persiste hasta re-listar la carpeta.
    UntilRefresh,
}
```
b) En `struct Settings`, añadir:
```rust
    /// Duración del resaltado de archivos recién aparecidos (watcher).
    #[serde(default = "default_highlight_duration")]
    pub highlight_duration: HighlightDuration,
    /// Si los archivos nuevos se agrupan al final del listado (resaltados) en vez de
    /// insertarse ya ordenados.
    #[serde(default = "default_new_items_at_end")]
    pub new_items_at_end: bool,
```
c) Defaults:
```rust
fn default_highlight_duration() -> HighlightDuration {
    HighlightDuration::UntilInteract
}
fn default_new_items_at_end() -> bool {
    false
}
```
d) Si `Settings` tiene `impl Default` MANUAL, añadir `highlight_duration: HighlightDuration::UntilInteract, new_items_at_end: false,`. Si el test de round-trip construye Settings con todos los campos, añadir los dos. CONFIG_VERSION sigue 1.

- [ ] **Step 3: Correr — pasan**

Run: `cargo test -p naygo-core config` → PASS.
Run: `cargo test -p naygo-core` → verde.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 4: Commit**
```
git add crates/core/src/config/mod.rs
git commit -m "feat(core): Settings del watcher (duración del resaltado, nuevos al final)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `HighlightDuration { UntilInteract, FadeSeconds(u32), UntilRefresh }`, `highlight_duration`, `new_items_at_end` EXACTOS (Tasks 8-10 dependen).

---

## Task 6: `platform::dir_watch` — notify → DirEvent (debounce)

**Files:**
- Modify: `crates/platform/Cargo.toml`
- Create: `crates/platform/src/dir_watch.rs`
- Modify: `crates/platform/src/lib.rs`

- [ ] **Step 1: Añadir notify a Cargo**

Modify `crates/platform/Cargo.toml`: en `[dependencies]` (NO target-cfg; notify es multiplataforma), añadir:
```toml
notify = "6"
notify-debouncer-full = "0.3"
```
(Verificar las versiones disponibles; notify 6.x con notify-debouncer-full 0.3.x es una combinación estable. Si la resolución de Cargo prefiere notify 7 / debouncer 0.4, usar esas — ajustar los `use` a la API real. Ambos son MIT/Apache.)

- [ ] **Step 2: Crear dir_watch.rs**

Create `crates/platform/src/dir_watch.rs`:
```rust
// Naygo — vigilar una carpeta (crate notify) y emitir DirEvent coalescidos, aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Envuelve `notify` (vía debouncer) para observar UNA carpeta (no recursivo) y emitir
//! lotes de `naygo_core::listing::DirEvent` coalescidos (~300 ms). Tolerante: si no se
//! puede observar (red, permiso), el handle queda inerte (no crashea; el panel no se
//! auto-actualiza, pero sigue usable). El Drop del handle detiene el watcher.

use naygo_core::listing::DirEvent;
use std::path::Path;
use std::sync::mpsc::Sender;

/// Handle de un watcher activo. Al dropearse, detiene el watcher (libera el handle del SO).
pub struct WatchHandle {
    // El debouncer posee el hilo y el watcher; dropearlo los detiene.
    _debouncer: Option<notify_debouncer_full::Debouncer<notify::RecommendedWatcher, notify_debouncer_full::FileIdMap>>,
}

/// Empieza a vigilar `dir` (no recursivo). Emite lotes de `DirEvent` por `tx` con
/// coalescing ~300 ms. Si falla, devuelve un handle inerte (sin crashear).
pub fn watch(dir: &Path, tx: Sender<Vec<DirEvent>>) -> WatchHandle {
    use notify::{RecursiveMode, Watcher};
    use notify_debouncer_full::new_debouncer;
    use std::time::Duration;

    let dir_owned = dir.to_path_buf();
    let result = new_debouncer(Duration::from_millis(300), None, move |res: notify_debouncer_full::DebounceEventResult| {
        if let Ok(events) = res {
            let mut out = Vec::new();
            for ev in events {
                // ev.event: notify::Event. Normalizar a DirEvent.
                normalize_into(&ev.event, &mut out);
            }
            if !out.is_empty() {
                let _ = tx.send(out);
            }
        }
    });
    match result {
        Ok(mut deb) => {
            if deb.watcher().watch(&dir_owned, RecursiveMode::NonRecursive).is_ok() {
                WatchHandle { _debouncer: Some(deb) }
            } else {
                WatchHandle { _debouncer: None } // inerte
            }
        }
        Err(_) => WatchHandle { _debouncer: None },
    }
}

/// Normaliza un `notify::Event` a uno o más `DirEvent`.
fn normalize_into(ev: &notify::Event, out: &mut Vec<DirEvent>) {
    use notify::EventKind;
    match &ev.kind {
        EventKind::Create(_) => {
            for p in &ev.paths {
                out.push(DirEvent::Created(p.clone()));
            }
        }
        EventKind::Remove(_) => {
            for p in &ev.paths {
                out.push(DirEvent::Removed(p.clone()));
            }
        }
        EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
            // Rename: notify suele entregar 2 paths (from, to) o 1 por evento.
            if ev.paths.len() == 2 {
                out.push(DirEvent::Renamed { from: ev.paths[0].clone(), to: ev.paths[1].clone() });
            } else {
                // Un solo path: lo tratamos como modificación de nombre → Created/Removed
                // según contexto; lo más seguro es Created (el merge dedup/maneja).
                for p in &ev.paths {
                    out.push(DirEvent::Created(p.clone()));
                }
            }
        }
        EventKind::Modify(_) => {
            for p in &ev.paths {
                out.push(DirEvent::Modified(p.clone()));
            }
        }
        _ => {}
    }
}
```
VERIFY la API real de notify 6/7 + notify-debouncer-full 0.3/0.4: `new_debouncer(timeout, tick_rate, handler)` firma, el tipo `DebounceEventResult`, `deb.watcher().watch(path, RecursiveMode)`, el tipo del Debouncer genérico (`Debouncer<RecommendedWatcher, FileIdMap>`), y `ev.event` vs `ev` (en debouncer-full cada item es un `DebouncedEvent` que deref-ea a `notify::Event`). Ajustar los nombres EXACTOS a la versión que Cargo resuelva (usar `cargo doc -p notify-debouncer-full --open` mentalmente o los errores del compilador). La LÓGICA (Create→Created, Remove→Removed, Modify::Name→Renamed si 2 paths, otro Modify→Modified) es lo fijo.

- [ ] **Step 3: Declarar el módulo**

Modify `crates/platform/src/lib.rs`: add `pub mod dir_watch;`.

- [ ] **Step 4: Build + lint + smoke**

Run: `cargo build -p naygo-platform` → compiles.
Run: `cargo clippy -p naygo-platform --all-targets -- -D warnings` → clean.
MANUAL smoke (report): un `#[test] #[ignore]` `dir_watch_smoke` que crea un tempdir, llama `watch(dir, tx)`, crea un archivo dentro, y verifica que `rx.recv_timeout(2s)` trae un lote con un `DirEvent::Created` de ese archivo. Run con `--ignored --test-threads=1`. Report el resultado (puede ser flaky por timing; si pasa, bien; si el debounce tarda, subir el timeout del recv).

- [ ] **Step 5: fmt + commit**

Run `cargo fmt --all`.
```
git add crates/platform/src/dir_watch.rs crates/platform/src/lib.rs crates/platform/Cargo.toml Cargo.lock
git commit -m "feat(platform): dir_watch (notify → DirEvent, debounce 300ms)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `watch(&Path, Sender<Vec<DirEvent>>) -> WatchHandle` EXACTO (Task 8 depende). Si notify resulta inviable de integrar limpio, DONE_WITH_CONCERNS con el error exacto (NO dejar un stub que diga que vigila pero no lo haga).

---

## Task 7: `platform::device_watch` — ventana message-only Win32

**Files:**
- Create: `crates/platform/src/device_watch.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/platform/Cargo.toml`

ESTO ES LO MÁS DELICADO. Aislado en su propio hilo + ventana oculta; NO toca eframe.

- [ ] **Step 1: Cargo features**

Modify `crates/platform/Cargo.toml`: a la lista de features de `windows`, añadir lo necesario para una ventana message-only + WM_DEVICECHANGE: `Win32_UI_WindowsAndMessaging` (ya está, de shell-A), `Win32_System_LibraryLoader` (GetModuleHandleW), `Win32_Foundation` (ya está). `WM_DEVICECHANGE`/`DBT_*` viven en `Win32_UI_WindowsAndMessaging`. Añadir solo las que falten (el compilador dirá).

- [ ] **Step 2: Crear device_watch.rs**

Create `crates/platform/src/device_watch.rs`:
```rust
// Naygo — detección de dispositivos (ventana message-only Win32, WM_DEVICECHANGE), aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Crea, EN SU PROPIO HILO, una ventana oculta `HWND_MESSAGE` que escucha
//! `WM_DEVICECHANGE` (llegada/quita de volúmenes) y emite `DeviceEvent::DrivesChanged`.
//! NO toca el HWND de eframe (aislamiento: un fallo aquí no afecta la ventana principal).
//! Al dropear el handle, postea `WM_QUIT` y une el hilo. Tolerante: si la ventana no se
//! crea, el handle queda inerte (los discos siguen con el re-escaneo periódico de shell-A).

use std::sync::mpsc::Sender;

/// Evento de cambio de dispositivos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceEvent {
    /// Cambió el conjunto de unidades (llegó/se fue un volumen).
    DrivesChanged,
}

/// Handle del watcher de dispositivos. Drop detiene el hilo y la ventana.
pub struct DeviceWatchHandle {
    #[cfg(windows)]
    inner: Option<windows_impl::Inner>,
}

#[cfg(not(windows))]
pub fn watch(_tx: Sender<DeviceEvent>) -> DeviceWatchHandle {
    DeviceWatchHandle {}
}

#[cfg(windows)]
pub fn watch(tx: Sender<DeviceEvent>) -> DeviceWatchHandle {
    DeviceWatchHandle { inner: windows_impl::start(tx) }
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    // ... imports de windows::Win32::{Foundation, UI::WindowsAndMessaging, System::LibraryLoader}

    pub struct Inner {
        thread: Option<std::thread::JoinHandle<()>>,
        hwnd_bits: isize, // el HWND como entero, para postear WM_QUIT desde Drop
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            // Postear WM_CLOSE/WM_QUIT a la ventana para que su loop termine, luego join.
            // PostMessageW(HWND(self.hwnd_bits as *mut _), WM_CLOSE, 0, 0)  (ver impl real)
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    /// Arranca el hilo con la ventana message-only. Devuelve None si no se pudo crear.
    pub fn start(tx: Sender<DeviceEvent>) -> Option<Inner> {
        // 1. Spawn de un hilo que:
        //    a. RegisterClassW con un wndproc estático.
        //    b. CreateWindowExW(0, class, "", 0, 0,0,0,0, HWND_MESSAGE, ...) → HWND oculto.
        //    c. Guardar `tx` accesible por el wndproc (SetWindowLongPtrW(GWLP_USERDATA) con
        //       un Box<Sender> raw, o un thread_local / static con Mutex). El patrón estándar
        //       es pasar el puntero al Sender vía lpParam de CreateWindowExW y recuperarlo en
        //       WM_NCCREATE para guardarlo en GWLP_USERDATA.
        //    d. Loop GetMessageW/TranslateMessage/DispatchMessageW hasta WM_QUIT.
        //    e. Al salir, liberar el Box<Sender>.
        // 2. El wndproc: si msg == WM_DEVICECHANGE y wparam es DBT_DEVICEARRIVAL o
        //    DBT_DEVICEREMOVECOMPLETE, recuperar el Sender de GWLP_USERDATA y enviar
        //    DeviceEvent::DrivesChanged. (Opcional: filtrar lparam DBT_DEVTYP_VOLUME.)
        // 3. Comunicar el HWND creado de vuelta al hilo padre (por un canal sync de 1) para
        //    poder postearle WM_CLOSE desde Drop. Si CreateWindowExW falla, el hilo termina
        //    y start() devuelve None.
        // Implementar con cuidado: el wndproc es `extern "system" fn`. GWLP_USERDATA guarda
        // el `*mut Sender<DeviceEvent>`. Liberar en WM_DESTROY.
        None // <- reemplazar por la implementación real
    }
}
```
THIS IS A SKELETON. The implementer MUST write the real message-only window. Key correctness points (follow `windows` 0.62 idioms from trash/clipboard/open):
- The wndproc is `unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT`. Default-handle everything via `DefWindowProcW` except WM_DEVICECHANGE.
- Pass the `Box::into_raw(Box::new(tx))` pointer through `CreateWindowExW`'s `lpparam`; in `WM_NCCREATE` read it from `CREATESTRUCTW.lpCreateParams` and store via `SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr)`. Retrieve in WM_DEVICECHANGE via `GetWindowLongPtrW`. Free the Box in `WM_DESTROY` (`Box::from_raw`).
- `RegisterClassW` once (guard against double-register: use a unique class name; if RegisterClassW fails because already registered, proceed — or register with a per-instance unique name).
- Window parent = `HWND_MESSAGE` (a special HWND for message-only windows) → no taskbar/visible window.
- The message loop (`GetMessageW`) runs on the spawned thread; it exits when the window is destroyed / WM_QUIT posted.
- Drop: `PostMessageW(hwnd, WM_CLOSE, ...)` (triggers WM_DESTROY → PostQuitMessage in wndproc → GetMessageW returns 0 → loop ends), then `join`. Send the created HWND back to the parent via a `std::sync::mpsc::sync_channel(1)` so Drop has it.
- `WM_DEVICECHANGE` = 0x0219; `DBT_DEVICEARRIVAL` = 0x8000; `DBT_DEVICEREMOVECOMPLETE` = 0x8004. These constants are in `Win32_UI_WindowsAndMessaging` (or hardcode with a comment if not exported).
- Tolerant: any failure → `start` returns `None` → inert handle.

- [ ] **Step 3: Declarar el módulo**

Modify `crates/platform/src/lib.rs`: add `pub mod device_watch;`.

- [ ] **Step 4: Build + lint + smoke**

Run: `cargo build -p naygo-platform` → compiles (Windows impl).
Run: `cargo clippy -p naygo-platform --all-targets -- -D warnings` → clean.
MANUAL smoke (report): `watch(tx)` returns a handle without panicking; if you can plug/unplug a USB drive, confirm a `DrivesChanged` arrives on `rx`. If no USB available, at least confirm the window/thread starts and Drop doesn't hang (create handle, drop it, process exits cleanly). Report what you could verify.

If the message-only window proves too risky/fiddly to complete confidently, report **DONE_WITH_CONCERNS** with the exact blocker — the disk re-scan of shell-A is the fallback, so the app still detects drives (just with ~3s delay). Do NOT leave a stub that pretends to work.

- [ ] **Step 5: fmt + commit**

Run `cargo fmt --all`.
```
git add crates/platform/src/device_watch.rs crates/platform/src/lib.rs crates/platform/Cargo.toml Cargo.lock
git commit -m "feat(platform): device_watch (WM_DEVICECHANGE, ventana message-only aislada)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `watch(Sender<DeviceEvent>) -> DeviceWatchHandle` and `DeviceEvent::DrivesChanged` EXACTOS (Task 9 depende).

---

## Task 8: UI — watchers por panel + pump_watchers + resaltado en datos

**Files:**
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: Estado + creación/destrucción de watchers**

Modify `crates/ui/src/app.rs`:
a) Añadir a `NaygoApp`:
```rust
    /// Watcher de carpeta por panel (vigila la carpeta visible de cada FilePane).
    watchers: std::collections::HashMap<PaneId, naygo_platform::dir_watch::WatchHandle>,
    /// Receptores de eventos de carpeta por panel.
    watch_rx: std::collections::HashMap<PaneId, std::sync::mpsc::Receiver<Vec<naygo_core::listing::DirEvent>>>,
```
Init en `new`: ambos `HashMap::new()`.
b) Un helper para (re)armar el watcher de un panel cuando lista una carpeta:
```rust
    /// (Re)crea el watcher de carpeta del panel `id` apuntando a `dir`. Soltar el viejo
    /// libera su handle del SO.
    fn rewatch_pane(&mut self, id: PaneId, dir: std::path::PathBuf) {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = naygo_platform::dir_watch::watch(&dir, tx);
        self.watchers.insert(id, handle);
        self.watch_rx.insert(id, rx);
    }
```
c) Llamar `rewatch_pane(id, dir)` donde se inicia un listing de una carpeta — en `start_listing` (o justo después de navegar/listar). Buscar `start_listing` y, al final, añadir `self.rewatch_pane(id, dir.clone());` (asegurando que `dir` esté disponible; clonar si hace falta).
d) Al cerrar un panel, quitar su watcher: buscar dónde se eliminan panes (donde se limpian `listings`/`trees` de un id cerrado) y añadir `self.watchers.remove(&id); self.watch_rx.remove(&id);`. (Si no hay limpieza al cerrar — había deuda anotada de eso — al menos no acumular: el insert por id reemplaza el viejo.)

- [ ] **Step 2: pump_watchers**

Añadir a `impl NaygoApp`:
```rust
    /// Drena los eventos de carpeta de cada panel y los fusiona al listado en vivo.
    fn pump_watchers(&mut self) {
        // Recoger (id, eventos) sin mantener prestado self.watch_rx mientras mutamos panes.
        let mut batches: Vec<(PaneId, Vec<naygo_core::listing::DirEvent>)> = Vec::new();
        for (id, rx) in &self.watch_rx {
            let mut events = Vec::new();
            while let Ok(mut batch) = rx.try_recv() {
                events.append(&mut batch);
            }
            if !events.is_empty() {
                batches.push((*id, events));
            }
        }
        if batches.is_empty() {
            return;
        }
        let sort_specs: Vec<_> = batches.iter().map(|(id, _)| *id).collect();
        for (id, events) in batches {
            if let Some(f) = self.workspace.pane_mut(id).and_then(|p| p.files.as_mut()) {
                let read = |p: &std::path::Path| {
                    let meta = std::fs::metadata(p).ok();
                    Some(naygo_core::listing::entry_from_path(p, meta.as_ref()))
                };
                let nuevas = naygo_core::listing::apply_dir_events(&mut f.entries, &events, &read);
                // Re-ordenar tras el merge (entries se mantiene ordenado por la columna).
                naygo_core::sort::sort_entries(&mut f.entries, &f.sort);
                for p in nuevas {
                    f.highlighted.insert(p);
                }
            }
        }
        let _ = sort_specs;
        // Repintar para mostrar los cambios.
        // (el request_repaint del loop se encarga; ver Step 4.)
    }
```
NOTE: el closure `read` usa `std::fs::metadata` — eso es I/O en el hilo de UI. Para un merge de pocos archivos (debounced) es barato (un stat por archivo nuevo/modificado). Si un lote trae miles (fallback de ráfaga), considerar re-listar; pero por ahora el stat por evento es aceptable dado el debounce. (Si el reviewer objeta, mover a worker — pero YAGNI.)

- [ ] **Step 3: Limpieza del resaltado según el modo**

Añadir lógica: cuando el usuario interactúa con un panel (clic en una fila, navegar, mover foco), si `settings.highlight_duration == UntilInteract`, llamar `f.clear_highlight()` para ese panel. El punto natural: en `handle_input` cuando hay navegación/foco en el panel activo, y en el clic de fila del file_panel. Implementar al menos: en `apply_action` para MoveUp/MoveDown/Activate/GoUp/GoBack/GoForward y en el clic de selección, si modo UntilInteract, limpiar el highlight del panel activo. Para FadeSeconds y UntilRefresh: FadeSeconds requiere timestamps (un `HashMap<PathBuf, Instant>` por panel o un `Instant` de lote) — SIMPLIFICACIÓN aceptable para esta fase: implementar UntilInteract y UntilRefresh bien; para FadeSeconds, limpiar todo el highlight del panel tras N segundos del último evento (un `last_highlight: Option<Instant>` por panel, chequeado en pump con repaint mientras viva). Si FadeSeconds añade demasiada complejidad, implementarlo como "igual que UntilRefresh por ahora" y anotarlo — pero PREFERIR el timestamp por panel. Documentar la decisión tomada.

- [ ] **Step 4: Llamar pump_watchers en el loop + repaint**

Junto a `self.pump_disk_usage();` añadir `self.pump_watchers();`. En la guarda de `request_repaint`, añadir `|| !self.watch_rx.is_empty()` NO (eso sería siempre true); mejor: pump_watchers ya aplicó cambios este frame; para que los eventos lleguen sin input, el watcher necesita despertar la UI. egui no sabe del canal. SOLUCIÓN: usar `ctx.request_repaint_after(Duration::from_millis(500))` mientras haya watchers activos, para que la UI revise los canales ~2 veces por segundo aunque no haya input. Añadir en `update`/`ui`: `if !self.watchers.is_empty() { ctx.request_repaint_after(std::time::Duration::from_millis(500)); }`. Esto es barato (2 wakeups/s) y respeta bajo consumo razonablemente. (Alternativa más reactiva: que el worker de notify llame a un `egui::Context` clonado `request_repaint()` al enviar — pero pasar el ctx al worker complica; el repaint_after 500ms es suficiente y simple.)

- [ ] **Step 5: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings` → clean; `cargo fmt --all` + `--check`.
MANUAL: abrir Naygo en una carpeta, crear/copiar un archivo ahí desde Explorer → aparece en el listado en vivo (sin F5), resaltado. Borrar uno → desaparece.

- [ ] **Step 6: Commit**
```
git add crates/ui/src/app.rs
git commit -m "feat(ui): watcher de carpeta por panel + merge en vivo + resaltado

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: UI — device_watch al arrancar + pump_devices

**Files:**
- Modify: `crates/ui/src/app.rs`

- [ ] **Step 1: Estado + arranque**

Modify `crates/ui/src/app.rs`:
a) Añadir a `NaygoApp`:
```rust
    /// Watcher de dispositivos (pendrives) — ventana message-only.
    device_watch: Option<naygo_platform::device_watch::DeviceWatchHandle>,
    /// Receptor de eventos de dispositivos.
    device_rx: Option<std::sync::mpsc::Receiver<naygo_platform::device_watch::DeviceEvent>>,
```
Init en `new`: tras construir el struct (o en él), arrancar:
```rust
        let (dev_tx, dev_rx) = std::sync::mpsc::channel();
        let device_watch = Some(naygo_platform::device_watch::watch(dev_tx));
        // ... en el struct: device_watch, device_rx: Some(dev_rx),
```
(Construir el canal antes del struct literal y poner los campos.)

- [ ] **Step 2: pump_devices**

```rust
    /// Drena eventos de dispositivos: ante un cambio de unidades, re-escanea discos ya.
    fn pump_devices(&mut self) {
        let mut changed = false;
        if let Some(rx) = &self.device_rx {
            while let Ok(_ev) = rx.try_recv() {
                changed = true;
            }
        }
        if changed {
            self.start_disk_scan();
        }
    }
```

- [ ] **Step 3: Llamar en el loop + repaint**

Junto a `self.pump_watchers();` añadir `self.pump_devices();`. El `request_repaint_after(500ms)` del Task 8 (si hay watchers) ya cubre el drenaje; además, mientras `device_rx` exista, el mismo repaint_after periódico revisa el canal. Asegurar que el repaint_after se active también si hay device_watch (ampliar la guarda: `if !self.watchers.is_empty() || self.device_watch.is_some()`).

- [ ] **Step 4: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all` + `--check`.
MANUAL: enchufar un pendrive → el árbol/strip de discos lo muestran casi al instante (no a los 3s). Quitarlo → desaparece.

- [ ] **Step 5: Commit**
```
git add crates/ui/src/app.rs
git commit -m "feat(ui): detección de dispositivos en vivo → re-escaneo de discos inmediato

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: UI — render del resaltado (estilo A) + getter de tema + Settings

**Files:**
- Modify: `crates/ui/src/theme_apply.rs`
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/ui/src/settings_window/...`
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: getter highlight() del tema**

Modify `crates/ui/src/theme_apply.rs`: junto a `accent()`/`active_bar()`, add:
```rust
    /// Color base del resaltado de archivos nuevos (se tiñe el fondo de la fila).
    pub fn highlight(&self) -> egui::Color32 {
        to_color32(self.theme.highlight)
    }
```
(VERIFICAR cómo `ActiveTheme` guarda el `Theme` — `self.theme.highlight`. Si guarda colores pre-convertidos, adaptar.)

- [ ] **Step 2: Render estilo A en el file panel**

Modify `crates/ui/src/panes/file_panel.rs`: la `show` necesita saber qué filas están resaltadas + el modo `new_items_at_end` + el color. Esto requiere pasar info nueva a `file_panel::show`. Verificar su firma actual y cómo recibe el theme/pane. Para cada fila, si la entry está en `highlighted` (consultar `pane.is_highlighted(&entry.path)` — o pasar el set), pintar el fondo de la fila con `theme.highlight()` a alpha ~0.18 (un `Color32::from_rgba_unmultiplied(r,g,b,46)` derivado del highlight) y el nombre con un color más saturado del mismo. Usar el patrón de fondo de fila que el file panel ya use para la selección/zebra (grep cómo pinta `row_bg`/selección y replicar para highlight). Si el panel construye su vista con `view_indices()`, cambiarlo a `view_indices_ordered(settings.new_items_at_end)` para soportar el modo "al final".
NOTE: la firma de `file_panel::show` y de dónde saca el set `highlighted` hay que resolverla leyendo el archivo + cómo `docking.rs`/`app.rs` lo invocan (similar a cómo se pasó `disk_usage` al tree en shell-A). Pasar `highlighted: &HashSet<PathBuf>` (o `&FilePaneState` ya lo tiene) + `new_items_at_end: bool` + el color del tema. Threading análogo al de `disk_usage` en shell-A (vía `NaygoTabViewer`).

- [ ] **Step 3: i18n + opciones en Settings**

i18n (ambos, idénticas):
ES: `"settings.watch.section": "Vigilancia"`, `"settings.watch.highlight_duration": "Duración del resaltado de archivos nuevos"`, `"settings.watch.until_interact": "Hasta interactuar"`, `"settings.watch.fade": "Desvanecer (segundos)"`, `"settings.watch.until_refresh": "Hasta refrescar"`, `"settings.watch.new_at_end": "Agrupar archivos nuevos al final"`.
EN: `"settings.watch.section": "Watching"`, `"settings.watch.highlight_duration": "New-file highlight duration"`, `"settings.watch.until_interact": "Until you interact"`, `"settings.watch.fade": "Fade (seconds)"`, `"settings.watch.until_refresh": "Until refresh"`, `"settings.watch.new_at_end": "Group new files at the end"`.
Settings UI (en advanced.rs o una sección nueva): un selector de `HighlightDuration` (3 opciones vía `selectable_value`; para FadeSeconds, un valor fijo p. ej. 6s — `HighlightDuration::FadeSeconds(6)` — o un slider si querés exponer N) y un checkbox `new_items_at_end`. Mutar `app.settings.*`. Seguir el patrón de la sección "Pegar" de shell-A.

- [ ] **Step 4: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all` + `--check`.
MANUAL: archivo nuevo aparece con fondo teñido + nombre en color; al hacer clic/navegar (modo UntilInteract) el resaltado se limpia; modo "al final" agrupa los nuevos abajo; las opciones en Configuración funcionan.

- [ ] **Step 5: Commit**
```
git add crates/ui/src/theme_apply.rs crates/ui/src/panes/file_panel.rs crates/ui/src/settings_window crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): resaltado estilo A de archivos nuevos + opciones de vigilancia

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README**

READ the status block y reemplazar con:
```markdown
> **Estado:** Fase watcher (carpeta en vivo + detección de dispositivos) en desarrollo.
> Diseño en
> [`docs/superpowers/specs/2026-06-08-naygo-watcher-design.md`](docs/superpowers/specs/2026-06-08-naygo-watcher-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-08-naygo-watcher.md`](docs/superpowers/plans/2026-06-08-naygo-watcher.md).
> Operaciones (ops-A/B), paste inteligente, shell-A (abrir + discos) y bloque visual completos.
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compiles.
Run: `cargo test --workspace` → green.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → clean.
Run: `cargo fmt --all -- --check` → clean.
Run: `cargo build --release -p naygo-ui` → release compiles.
MANUAL end-to-end: cambios en vivo en un panel con resaltado; los 3 modos de duración; modo "al final"; pendrive en vivo.

- [ ] **Step 3: Commit y push**
```
git add README.md
git commit -m "chore: actualizar estado del README (fase watcher)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/watcher
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| DirEvent (tipo puro) + entry_from_path compartido | 1 |
| apply_dir_events (Created/Removed/Modified/Renamed, idempotente, nuevas) | 2 |
| token highlight (Serialize+tolerante+4 temas) | 3 |
| highlighted en FilePaneState (#[serde(skip)]) + is_highlighted/clear | 4 |
| view "al final" estable | 4 |
| Settings (HighlightDuration + new_items_at_end) | 5 |
| platform::dir_watch (notify, debounce 300ms, DirEvent) | 6 |
| platform::device_watch (WM_DEVICECHANGE, message-only aislada) | 7 |
| watchers por panel + pump_watchers + merge + highlighted | 8 |
| limpieza del resaltado por modo | 8 |
| device_watch al arrancar + pump_devices → start_disk_scan | 9 |
| render estilo A + getter highlight + view_indices_ordered usado | 10 |
| opciones en Configuración + i18n | 10 |
| vigilar TODOS los paneles visibles | 8 (un watcher por pane) |
| recursivo/minimizar/shell-B FUERA | (no se tocan) |

**Notas de riesgo:**
- **device_watch (Task 7)** es lo más delicado: ventana message-only + wndproc + GWLP_USERDATA + Drop limpio. Aislado en su hilo; si inviable limpio → DONE_WITH_CONCERNS (el re-escaneo de shell-A es el respaldo, la app sigue detectando discos con retardo).
- **notify API (Task 6)**: verificar versiones (notify 6/7, debouncer 0.3/0.4) y la firma de `new_debouncer`/`DebounceEventResult`/`Debouncer<...>` contra lo que Cargo resuelva; la lógica de normalización es lo fijo.
- **Repaint sin input (Task 8/9)**: `ctx.request_repaint_after(500ms)` mientras haya watchers/device — barato, revisa los canales 2x/s sin input. Evita el problema de que egui no repinte en idle.
- **`read` con std::fs::metadata en pump_watchers** corre en el hilo de UI: barato por el debounce (pocos stats); si un lote es gigante, considerar re-listar (anotar; no implementar el fallback salvo que el reviewer lo pida).
- **highlighted serde**: si FilePaneState se persiste, `#[serde(skip)]` es obligatorio (no guardar estado efímero). Verificar si deriva Serialize.
- **file_panel::show threading (Task 10)**: pasar `highlighted`+`new_items_at_end`+color análogo a cómo shell-A pasó `disk_usage` al tree vía `NaygoTabViewer`. Verificar la firma real.
- **Theme Serialize field count (Task 3)**: si `serialize_struct("Theme", N)` usa un N literal, incrementarlo al añadir highlight.
```
