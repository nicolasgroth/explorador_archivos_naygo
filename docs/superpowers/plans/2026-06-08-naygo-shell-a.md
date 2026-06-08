# shell-A — Abrir con app default + espacio de discos — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Doble-clic/Enter abre un archivo con su app por defecto, "Abrir con…" lanza el diálogo nativo, y el árbol muestra el espacio libre de cada disco (barra + % usado + color) con un acceso rápido a unidades.

**Architecture:** La DECISIÓN/cálculo es puro en `core` (`DiskUsage`, `human_size` movido a `core::format`); el toque a Windows está aislado en `platform` (`open` vía ShellExecuteW, `drive_space` vía GetDiskFreeSpaceExW); la UI dispara abrir, lee el espacio en un worker async (drenado por frame) y lo pinta en el árbol. El menú contextual propio se reordena y deja un placeholder "Más opciones de Windows…" para shell-B.

**Tech Stack:** Rust, `naygo-core`/`naygo-platform`/`naygo-ui`, `eframe`/`egui` 0.34.3, crate `windows` 0.62, `std::thread`/`mpsc`. Sin chrono, sin dependencias nuevas de terceros.

**Estado de partida (rama `feat/shell-a`, desde `main` con ops-A/ops-B/paste):**
- `naygo_platform::drives() -> Vec<DriveInfo>` (drives.rs). `DriveInfo { path: PathBuf, label: String, kind: DriveKind }`. Patrón `#[cfg(windows)]` real + `#[cfg(not(windows))]` stub (devuelve `/`). SIN cambios en esta fase.
- `naygo_platform::trash` es el molde de Win32: `#[cfg(windows)]`/`#[cfg(not(windows))]`, error tipado, `OsStr`→UTF-16 (`encode_wide`/`encode_utf16().chain(once(0))`). Cargo `[target.'cfg(windows)'.dependencies] windows = { workspace = true, features = [...] }` con `Win32_UI_Shell`, `Win32_Storage_FileSystem`, `Win32_Foundation` YA presentes.
- `naygo_core` modules en `lib.rs`: cancel, clipboard, columns, config, filter, fs_model, i18n, icon_kind, listing, ops, sort, theme, tree, workspace.
- `human_size(bytes: u64) -> String` es `pub(crate)` en `crates/ui/src/panes/file_panel.rs:477` (KB/MB/GB, sin TB hoy). Callers: `app.rs` (`use crate::panes::file_panel::human_size;` ~1076, usos 1096/1110), `ops_panel.rs:12` (use) + `:161`/`:162`, `panes/file_panel.rs:472` (interno).
- `crates/ui/src/input.rs`: `pub enum Action { MoveUp, MoveDown, Activate, GoUp, GoBack, GoForward, SwitchPane, CancelListing, Copy, Cut, Paste, Delete, DeletePermanent, Rename, NewFile, NewDir, CopyToOther, MoveToOther }`.
- `apply_action(&mut self, action: Action)` (app.rs:939) — match exhaustivo de las variantes. `activate_focused` (app.rs:1443): carpeta→navega; archivo→`self.status = self.i18n.t("status.open_pending").replace("{name}", &entry.name)` (PLACEHOLDER a reemplazar). `entry.path`, `entry.name`, `entry.is_dir()`.
- File panel context menu (`crates/ui/src/panes/file_panel.rs:287` `row_resp.context_menu(|ui| {...})`): hoy botones `op.copy`/`op.cut`/`op.paste`/sep/`op.rename`/`op.delete`, cada uno `ops_actions.push(Action::X); ui.close();`. `ops_actions: &mut Vec<Action>` se pasa a `show` y `NaygoApp` los procesa con `apply_action`.
- Árbol: `crates/ui/src/panes/tree_panel.rs` — `show(ui, tree, actions, icons, i18n, theme) -> bool` itera `tree.roots`; `show_node(ui, node, depth, tree, actions, icons, i18n, theme)`. `TreeNode { path, state: NodeState, expanded, drive_kind: Option<DriveKind>, ... }`. Las RAÍCES (discos) tienen `drive_kind == Some(_)` y `depth == 0`. `TreeAction` en `tree_actions.rs` (Expand/Collapse/Navigate...). El árbol se pinta desde `app.rs` (buscar la llamada a `tree_panel::show`).
- `crates/ui/src/toolbar.rs` existe (toolbar superior).
- Patrón worker: `listings`/`ops` usan `mpsc::channel` + `CancellationToken` + `pump_*` drenado por frame en `update`/`ui` (app.rs ~1626 `pump_all/pump_tree/pump_ops`). `refresh_pane`, `start_listing`. `NaygoApp.config_dir`, `self.settings`, `self.i18n`, `self.status`.

**Prerequisito:** Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo. `cargo fmt --all -- --check`. Binario `--bin naygo`. Bash: NO `cd /d`. NUNCA `2>&1` con exes nativos en PowerShell.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. Header de 2 líneas en archivos NUEVOS. `core` NUNCA importa egui/windows (es puro). UI nunca hace I/O de disco en el hilo de UI (el espacio va en worker; ShellExecute retorna al lanzar, es inmediato). Tolerante (SO hostil, Result tipado, sin panics). Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt antes de cada commit. SIEMPRE `cargo fmt --all` antes de commitear (este repo arrastra fmt drift). Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/shell-a`. NO cambiar de rama.

**SECUENCIA:** Tasks 1-2 (core disk + format) son self-contained. Task 3 (mover human_size: crear en core ya hecho en Task 2, repuntar los 3 callers de ui, quitar de file_panel) toca cross-crate — se hace en UN commit que deja todo compilando. Tasks 4-5 (platform open + drive_space) self-contained (módulos nuevos). Tasks 6-8 (UI: abrir + menú; worker+render espacio; strip de discos) sobre lo anterior. Task 9 cierre.

**Alcance:** ENTRA: abrir (default + "abrir con…"), menú reordenado + placeholder, espacio de discos (barra+%+color, worker async, re-escaneo), strip de discos, `human_size`→core, `core::disk`, i18n. NO ENTRA: menú COM nativo (shell-B), detección en vivo de dispositivos (watcher), abrir carpeta en Explorer.

---

## Estructura de archivos

```
crates/core/src/
├── disk.rs          # NUEVO: DiskUsage + tests
├── format.rs        # NUEVO: human_size (movido) + tests
└── lib.rs           # + pub mod disk; pub mod format;

crates/platform/src/
├── open.rs          # NUEVO: open_default/open_with_dialog (ShellExecuteW) + stub
├── drive_space.rs   # NUEVO: read_space (GetDiskFreeSpaceExW) + stub
├── lib.rs           # + pub mod open; pub mod drive_space;
└── Cargo.toml       # + Win32_UI_WindowsAndMessaging (si falta SW_SHOWNORMAL)

crates/ui/src/
├── app.rs           # activate_focused→open_default; Action::Open/OpenWith; worker espacio + mapa + re-escaneo; pump
├── input.rs         # + Action::Open, Action::OpenWith
├── panes/file_panel.rs   # menú reordenado + Abrir/Abrir con…/placeholder; human_size→core
├── panes/tree_panel.rs   # render barra+%+color de espacio en raíces (recibe el mapa)
├── ops_panel.rs     # human_size → naygo_core::format
├── toolbar.rs        # strip de discos clicables (acceso rápido)
crates/core/src/i18n/{es,en}.json  # claves abrir/abrir-con/sin-asociación/más-opciones/espacio
```

---

## Task 1: `core::disk` — DiskUsage

**Files:**
- Create: `crates/core/src/disk.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear `disk.rs` con tests (TDD)**

Create `crates/core/src/disk.rs`:
```rust
// Naygo — uso de disco: cálculo puro de espacio usado/libre y umbrales.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `DiskUsage` resume el espacio de una unidad (total y libre, en bytes) y deriva
//! el porcentaje USADO y los umbrales de alerta. Puro: sin Windows ni I/O. La
//! lectura real del espacio vive en `platform::drive_space`.

/// Espacio de una unidad, en bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskUsage {
    pub total: u64,
    pub free: u64,
}

impl DiskUsage {
    /// Bytes usados. Satura a 0 si `free > total` (datos inconsistentes del SO).
    pub fn used(self) -> u64 {
        self.total.saturating_sub(self.free)
    }

    /// Porcentaje USADO, 0..=100. `0` si `total == 0` (unidad sin tamaño conocido).
    pub fn percent_used(self) -> u8 {
        if self.total == 0 {
            return 0;
        }
        let pct = (self.used() as u128 * 100 / self.total as u128) as u64;
        pct.min(100) as u8
    }

    /// Uso alto: > 75% usado.
    pub fn is_high(self) -> bool {
        self.percent_used() > 75
    }

    /// Uso crítico: > 90% usado.
    pub fn is_critical(self) -> bool {
        self.percent_used() > 90
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_normal() {
        let d = DiskUsage { total: 1000, free: 400 };
        assert_eq!(d.used(), 600);
    }

    #[test]
    fn used_satura_si_free_mayor_que_total() {
        let d = DiskUsage { total: 100, free: 500 };
        assert_eq!(d.used(), 0);
        assert_eq!(d.percent_used(), 0);
    }

    #[test]
    fn percent_total_cero_es_cero() {
        let d = DiskUsage { total: 0, free: 0 };
        assert_eq!(d.percent_used(), 0);
    }

    #[test]
    fn percent_y_umbrales() {
        let half = DiskUsage { total: 1000, free: 500 };
        assert_eq!(half.percent_used(), 50);
        assert!(!half.is_high() && !half.is_critical());

        let p76 = DiskUsage { total: 1000, free: 240 }; // 76% usado
        assert_eq!(p76.percent_used(), 76);
        assert!(p76.is_high() && !p76.is_critical());

        let p91 = DiskUsage { total: 1000, free: 90 }; // 91% usado
        assert_eq!(p91.percent_used(), 91);
        assert!(p91.is_high() && p91.is_critical());

        let p75 = DiskUsage { total: 1000, free: 250 }; // 75% exacto: NO high (>75)
        assert_eq!(p75.percent_used(), 75);
        assert!(!p75.is_high());
    }

    #[test]
    fn percent_satura_a_100() {
        let full = DiskUsage { total: 1000, free: 0 };
        assert_eq!(full.percent_used(), 100);
        assert!(full.is_critical());
    }
}
```

- [ ] **Step 2: Declarar el módulo**

Modify `crates/core/src/lib.rs`: add `pub mod disk;` (alfabético, entre `config` y `filter`... ubicarlo donde encaje el orden existente; el orden exacto no importa funcionalmente).

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core disk` → 5 tests PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → clean.

- [ ] **Step 4: Commit**
```
git add crates/core/src/disk.rs crates/core/src/lib.rs
git commit -m "feat(core): DiskUsage (uso de disco, puro)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `DiskUsage { total, free }` + `used`/`percent_used`/`is_high`/`is_critical` EXACTLY (Tasks 6-8 depend).

---

## Task 2: `core::format` — human_size (creación + tests)

**Files:**
- Create: `crates/core/src/format.rs`
- Modify: `crates/core/src/lib.rs`

NOTE: esta tarea CREA `human_size` en core (con tests). La REMOCIÓN del de ui y el repunte de callers es la Task 3 (para no romper ui entre tareas: aquí ui sigue con su copia `pub(crate)`, que se elimina en Task 3).

- [ ] **Step 1: Crear `format.rs` con tests (TDD)**

Create `crates/core/src/format.rs`:
```rust
// Naygo — formateo de tamaños legibles (B/KB/MB/GB/TB). Puro y reutilizable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `human_size` formatea un número de bytes a una cadena legible (1 decimal). Base
//! 1024. Reutilizado por tamaños de archivo, de transferencia y de disco.

const KB: f64 = 1024.0;
const MB: f64 = KB * 1024.0;
const GB: f64 = MB * 1024.0;
const TB: f64 = GB * 1024.0;

/// Formatea `bytes` como "B/KB/MB/GB/TB" con un decimal (salvo bytes crudos).
pub fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= TB {
        format!("{:.1} TB", b / TB)
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_crudos() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1023), "1023 B");
    }

    #[test]
    fn kilobytes() {
        assert_eq!(human_size(1024), "1.0 KB");
        assert_eq!(human_size(1536), "1.5 KB");
    }

    #[test]
    fn mega_giga_tera() {
        assert_eq!(human_size(1024 * 1024), "1.0 MB");
        assert_eq!(human_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(human_size(1024u64 * 1024 * 1024 * 1024), "1.0 TB");
    }
}
```
NOTE: el `human_size` actual de ui NO tiene tier TB (corta en GB). Añadir TB aquí es una mejora compatible (un archivo de ≥1 TB ahora se muestra en TB en vez de "1024.0 GB"). Los tests de bytes/KB/MB/GB siguen valiendo.

- [ ] **Step 2: Declarar el módulo**

Modify `crates/core/src/lib.rs`: add `pub mod format;`.

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core format` → 3 tests PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → clean.

- [ ] **Step 4: Commit**
```
git add crates/core/src/format.rs crates/core/src/lib.rs
git commit -m "feat(core): human_size en core::format (reutilizable, +tier TB)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `naygo_core::format::human_size(bytes: u64) -> String` EXACTLY (Task 3 + UI depend).

---

## Task 3: Migrar callers de `human_size` a core y eliminar la copia de ui

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/ops_panel.rs`

Cross-crate en UN commit: ui pasa a usar `naygo_core::format::human_size` y se borra la copia local. Antes y después compila.

- [ ] **Step 1: Eliminar la copia de ui y repuntar su uso interno**

Modify `crates/ui/src/panes/file_panel.rs`:
- Delete the `pub(crate) fn human_size(bytes: u64) -> String { ... }` (líneas ~477-491).
- En `format_size` (~470-475), cambiar `human_size(bytes)` → `naygo_core::format::human_size(bytes)`. (O añadir `use naygo_core::format::human_size;` al tope del archivo y dejar las llamadas como están — elegir UNA forma y aplicarla consistentemente en este archivo.)

- [ ] **Step 2: Repuntar `app.rs`**

Modify `crates/ui/src/app.rs`: el `use crate::panes::file_panel::human_size;` (dentro de `pump_paste_write`, ~1076) → `use naygo_core::format::human_size;`. (Las llamadas `human_size(...)` quedan igual.)

- [ ] **Step 3: Repuntar `ops_panel.rs`**

Modify `crates/ui/src/ops_panel.rs`: `use crate::panes::file_panel::human_size;` (línea 12) → `use naygo_core::format::human_size;`. Usos en 161/162 quedan igual.

- [ ] **Step 4: Verificar (grep que no quede referencia a la vieja)**

Run: `cargo build -p naygo-ui` → compiles.
Grep: no debe quedar `panes::file_panel::human_size` ni una def local `fn human_size` en ui. (Usar Grep tool sobre crates/ui/src.)
Run: `cargo test --workspace` → green.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → clean.
Run: `cargo fmt --all`.

- [ ] **Step 5: Commit**
```
git add crates/ui/src/panes/file_panel.rs crates/ui/src/app.rs crates/ui/src/ops_panel.rs
git commit -m "refactor(ui): usar naygo_core::format::human_size (quitar copia local)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `platform::open` — ShellExecute (abrir / abrir con…)

**Files:**
- Create: `crates/platform/src/open.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/platform/Cargo.toml`

- [ ] **Step 1: Asegurar features de `windows`**

Modify `crates/platform/Cargo.toml`: ensure the `windows` features list includes `Win32_UI_Shell` (already present) and add `Win32_UI_WindowsAndMessaging` (for `SW_SHOWNORMAL` / `SHOW_WINDOW_CMD`) and `Win32_System_Com` is already present. If `ShellExecuteW`/`SW_SHOWNORMAL` resolve without `Win32_UI_WindowsAndMessaging`, that's fine — add it only if the compiler asks.

- [ ] **Step 2: Crear `open.rs`**

Create `crates/platform/src/open.rs`:
```rust
// Naygo — abrir archivos con su programa por defecto (Win32 ShellExecuteW), aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `open_default` abre un archivo con la app asociada (verbo "open"); `open_with_dialog`
//! lanza el diálogo "Abrir con…" de Windows (verbo "openas"). Tolerante: devuelve un
//! `Result` tipado, nunca tumba el proceso. La política de Naygo es abrir con el SO,
//! no reproducir/editar.

use std::path::Path;

/// Error al pedirle al Shell que abra un archivo.
#[derive(Debug)]
pub enum ShellError {
    /// No soportado en esta plataforma.
    NotSupported,
    /// No hay programa asociado a ese tipo de archivo.
    NoAssociation,
    /// Otra falla del Shell (el mensaje describe el código).
    Failed(String),
}

#[cfg(not(windows))]
pub fn open_default(_path: &Path) -> Result<(), ShellError> {
    Err(ShellError::NotSupported)
}

#[cfg(not(windows))]
pub fn open_with_dialog(_path: &Path) -> Result<(), ShellError> {
    Err(ShellError::NotSupported)
}

#[cfg(windows)]
pub fn open_default(path: &Path) -> Result<(), ShellError> {
    windows_impl::shell_execute(path, "open")
}

#[cfg(windows)]
pub fn open_with_dialog(path: &Path) -> Result<(), ShellError> {
    windows_impl::shell_execute(path, "openas")
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    /// Ejecuta `verb` sobre `path`. ShellExecuteW devuelve un HINSTANCE cuyo valor
    /// numérico > 32 indica éxito; <= 32 es un código de error (SE_ERR_*).
    pub fn shell_execute(path: &Path, verb: &str) -> Result<(), ShellError> {
        let verb_w: Vec<u16> = verb.encode_utf16().chain(std::iter::once(0)).collect();
        let path_w: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: punteros válidos a cadenas NUL-terminadas; HWND(0)=sin ventana padre.
        let hinst = unsafe {
            ShellExecuteW(
                Some(HWND(std::ptr::null_mut())),
                PCWSTR(verb_w.as_ptr()),
                PCWSTR(path_w.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        let code = hinst.0 as usize;
        if code > 32 {
            Ok(())
        } else {
            // SE_ERR_NOASSOC = 31, SE_ERR_ASSOCINCOMPLETE = 27.
            const SE_ERR_NOASSOC: usize = 31;
            const SE_ERR_ASSOCINCOMPLETE: usize = 27;
            if code == SE_ERR_NOASSOC || code == SE_ERR_ASSOCINCOMPLETE {
                Err(ShellError::NoAssociation)
            } else {
                Err(ShellError::Failed(format!("ShellExecuteW devolvió {code}")))
            }
        }
    }
}
```
VERIFY against `windows` 0.62: `ShellExecuteW` signature (it takes `Option<HWND>` for the hwnd in recent versions, and returns `HINSTANCE`). `HWND(std::ptr::null_mut())` constructs a null HWND (HWND wraps `*mut c_void` in 0.62). `SW_SHOWNORMAL` is a `SHOW_WINDOW_CMD` in `Win32::UI::WindowsAndMessaging`. The `.0` of the returned `HINSTANCE` is a raw pointer — cast `as usize` (or `as isize as usize`) to compare with 32. Adapt the exact casts/Option-wrapping to what the 0.62 API and the compiler require; the LOGIC (>32 ok, 31/27 = no assoc) is what matters. Follow `trash.rs` for how this repo calls `windows` 0.62 fns and wraps HWND/pointers.

- [ ] **Step 3: Declarar el módulo**

Modify `crates/platform/src/lib.rs`: add `pub mod open;`.

- [ ] **Step 4: Build + lint + smoke**

Run: `cargo build -p naygo-platform` → compiles.
Run: `cargo clippy -p naygo-platform --all-targets -- -D warnings` → clean.
MANUAL smoke (report result): a scratch or `#[ignore]`d test or a tiny example that calls `open_default(Path::new("C:/Windows/win.ini"))` (or any text file) and confirms Notepad/associated app launches; `open_with_dialog` on the same path shows the "Abrir con…" dialog. Mark any test `#[ignore]` (it spawns a GUI app). Report whether the app launched.

- [ ] **Step 5: fmt + commit**

Run `cargo fmt --all`.
```
git add crates/platform/src/open.rs crates/platform/src/lib.rs crates/platform/Cargo.toml Cargo.lock
git commit -m "feat(platform): abrir con app default + abrir con… (ShellExecuteW)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `open_default(&Path) -> Result<(), ShellError>`, `open_with_dialog(&Path) -> Result<(), ShellError>`, `ShellError { NotSupported, NoAssociation, Failed(String) }` EXACTLY (Task 6 depends).

---

## Task 5: `platform::drive_space` — GetDiskFreeSpaceExW

**Files:**
- Create: `crates/platform/src/drive_space.rs`
- Modify: `crates/platform/src/lib.rs`

- [ ] **Step 1: Crear `drive_space.rs`**

Create `crates/platform/src/drive_space.rs`:
```rust
// Naygo — espacio libre/total de una unidad (Win32 GetDiskFreeSpaceExW), aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `read_space` consulta el espacio TOTAL y LIBRE de una unidad. Puede tardar en
//! discos de red/ópticos, así que se llama DESDE un worker (nunca en el hilo de UI).
//! Tolerante: `None` si la unidad no responde. El cálculo de % usado vive en
//! `naygo_core::disk::DiskUsage`.

use std::path::Path;

/// Devuelve `(total_bytes, free_bytes)` de la unidad que contiene `root`, o `None`
/// si la consulta falla (unidad caída, óptico vacío, ruta inválida).
#[cfg(windows)]
pub fn read_space(root: &Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let root_w: Vec<u16> = root
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free_to_caller: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;
    // SAFETY: PCWSTR a cadena NUL-terminada; los out-params son u64 válidos.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(root_w.as_ptr()),
            Some(&mut free_to_caller as *mut u64 as *mut _),
            Some(&mut total as *mut u64 as *mut _),
            Some(&mut total_free as *mut u64 as *mut _),
        )
    };
    match ok {
        Ok(()) => Some((total, total_free)),
        Err(_) => None,
    }
}

/// Stub no-Windows: sin información de espacio.
#[cfg(not(windows))]
pub fn read_space(_root: &Path) -> Option<(u64, u64)> {
    None
}
```
VERIFY `windows` 0.62 `GetDiskFreeSpaceExW` signature: it takes `PCWSTR` + three `Option<*mut u64>` (the params are `lpFreeBytesAvailableToCaller`, `lpTotalNumberOfBytes`, `lpTotalNumberOfFreeBytes`) and returns `windows::core::Result<()>`. The exact pointer type may be `*mut u64` directly (no cast needed) — adapt the casts to what compiles. We return `(total, total_free)` = (TotalNumberOfBytes, TotalNumberOfFreeBytes); `free_to_caller` is read but unused (quota-aware free; we use total_free for the disk). If clippy flags `free_to_caller` as unused-assigned, keep it (the API needs the out-param) or read it into `_`.

- [ ] **Step 2: Declarar el módulo**

Modify `crates/platform/src/lib.rs`: add `pub mod drive_space;`.

- [ ] **Step 3: Build + lint + smoke**

Run: `cargo build -p naygo-platform` → compiles.
Run: `cargo clippy -p naygo-platform --all-targets -- -D warnings` → clean.
MANUAL smoke (report): a scratch/`#[ignore]` test calling `read_space(Path::new("C:\\"))` returns `Some((total, free))` with `total > 0` and `free <= total`, plausible for the C: drive. Report the values seen.

- [ ] **Step 4: fmt + commit**

Run `cargo fmt --all`.
```
git add crates/platform/src/drive_space.rs crates/platform/src/lib.rs
git commit -m "feat(platform): read_space (espacio de disco, GetDiskFreeSpaceExW)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `read_space(&Path) -> Option<(u64, u64)>` returning `(total, free)` EXACTLY (Task 7 depends).

---

## Task 6: UI — abrir archivo + menú contextual reordenado

**Files:**
- Modify: `crates/ui/src/input.rs`
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: i18n keys (ambos archivos, mismas claves)**

ES: `"op.open": "Abrir"`, `"op.open_with": "Abrir con…"`, `"op.more_windows": "Más opciones de Windows…"`, `"op.more_windows_soon": "Disponible próximamente"`, `"status.opening": "Abriendo {name}"`, `"status.no_association": "No hay programa asociado a {name}"`, `"status.open_failed": "No se pudo abrir {name}"`.
EN: `"op.open": "Open"`, `"op.open_with": "Open with…"`, `"op.more_windows": "More Windows options…"`, `"op.more_windows_soon": "Coming soon"`, `"status.opening": "Opening {name}"`, `"status.no_association": "No program associated with {name}"`, `"status.open_failed": "Couldn't open {name}"`.
(READ both files; keep identical key sets — parity test.)

- [ ] **Step 2: Action variants**

Modify `crates/ui/src/input.rs`: add to `enum Action` (after `Activate` or near the file ops):
```rust
    /// Abrir el elemento enfocado con su app por defecto (menú contextual).
    Open,
    /// Abrir el elemento enfocado con… (diálogo nativo de elección de app).
    OpenWith,
```

- [ ] **Step 3: activate_focused abre archivos; apply_action maneja Open/OpenWith**

Modify `crates/ui/src/app.rs`:
a) `activate_focused` (~1443): replace the file `else` branch (the `status.open_pending` placeholder) with a call to a new helper `self.open_path(&entry.path, &entry.name)`:
```rust
        if entry.is_dir() {
            // ... navega (sin cambios) ...
        } else {
            let (path, name) = (entry.path.clone(), entry.name.clone());
            self.open_path(&path, &name);
        }
```
b) Add the helper to `impl NaygoApp`:
```rust
    /// Abre un archivo con su app por defecto; deja status de éxito/error.
    fn open_path(&mut self, path: &std::path::Path, name: &str) {
        match naygo_platform::open::open_default(path) {
            Ok(()) => {
                self.status = self.i18n.t("status.opening").replace("{name}", name);
            }
            Err(naygo_platform::open::ShellError::NoAssociation) => {
                self.status = self.i18n.t("status.no_association").replace("{name}", name);
            }
            Err(_) => {
                self.status = self.i18n.t("status.open_failed").replace("{name}", name);
            }
        }
    }

    /// Abre el diálogo "Abrir con…" del SO para un archivo; status en error.
    fn open_with_path(&mut self, path: &std::path::Path, name: &str) {
        if naygo_platform::open::open_with_dialog(path).is_err() {
            self.status = self.i18n.t("status.open_failed").replace("{name}", name);
        }
    }

    /// Resuelve la entry enfocada del panel activo (ruta + nombre), si hay archivo.
    fn focused_file(&self) -> Option<(std::path::PathBuf, String)> {
        let entry = self
            .workspace
            .active_files()
            .and_then(|f| f.focused_view_entry().cloned())?;
        if entry.is_dir() {
            None
        } else {
            Some((entry.path, entry.name))
        }
    }
```
c) In `apply_action` (~939), add arms:
```rust
            Action::Open => {
                if let Some((p, n)) = self.focused_file() {
                    self.open_path(&p, &n);
                }
            }
            Action::OpenWith => {
                if let Some((p, n)) = self.focused_file() {
                    self.open_with_path(&p, &n);
                }
            }
```
NOTE: the context-menu sets `context_focus` to the right-clicked row BEFORE the action is processed, so `focused_file()` resolves the correct entry. Verify the menu focuses the row first (it does: `context_focus = Some(i)` in the menu closure). If `focused_view_entry` doesn't reflect `context_focus` in time, route the path through the menu action differently — but the existing Copy/Cut/Rename already rely on this same focus-then-action ordering, so it works.

- [ ] **Step 4: Reordenar el menú contextual + Abrir/Abrir con…/placeholder**

Modify `crates/ui/src/panes/file_panel.rs` (the `row_resp.context_menu(|ui| {...})` at ~287). Reorder to:
```rust
                        row_resp.context_menu(|ui| {
                            context_focus = Some(i);
                            if ui.button(i18n.t("op.open")).clicked() {
                                ops_actions.push(Action::Open);
                                ui.close();
                            }
                            if ui.button(i18n.t("op.open_with")).clicked() {
                                ops_actions.push(Action::OpenWith);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(i18n.t("op.copy")).clicked() {
                                ops_actions.push(Action::Copy);
                                ui.close();
                            }
                            if ui.button(i18n.t("op.cut")).clicked() {
                                ops_actions.push(Action::Cut);
                                ui.close();
                            }
                            if ui.button(i18n.t("op.paste")).clicked() {
                                ops_actions.push(Action::Paste);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(i18n.t("op.rename")).clicked() {
                                ops_actions.push(Action::Rename);
                                ui.close();
                            }
                            if ui.button(i18n.t("op.delete")).clicked() {
                                ops_actions.push(Action::Delete);
                                ui.close();
                            }
                            ui.separator();
                            // Placeholder de shell-B: el menú COM nativo se construye
                            // SOLO bajo demanda (es lento de enumerar). Deshabilitado aquí.
                            ui.add_enabled(false, egui::Button::new(i18n.t("op.more_windows")))
                                .on_disabled_hover_text(i18n.t("op.more_windows_soon"));
                        });
```
(Preserve whatever the existing menu had beyond these — if it has more entries like DeletePermanent, keep them in a sensible spot. Match the real current content; the above is the target order.)

- [ ] **Step 5: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings` → clean; `cargo fmt --all` + `--check`.
MANUAL: double-click a .txt → opens in default app; right-click → menu shows Abrir/Abrir con…/…/"Más opciones de Windows…" (grayed, tooltip); "Abrir con…" → native dialog.

- [ ] **Step 6: Commit**
```
git add crates/ui/src/input.rs crates/ui/src/app.rs crates/ui/src/panes/file_panel.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): abrir con app default + Abrir con… + menú reordenado (placeholder shell-B)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: UI — worker de espacio de discos + render en el árbol

**Files:**
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/ui/src/panes/tree_panel.rs`
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: i18n keys (ambos archivos)**

ES: `"disk.usage": "{free} libres de {total} · {pct}% usado"`.
EN: `"disk.usage": "{free} free of {total} · {pct}% used"`.

- [ ] **Step 2: Estado + worker en NaygoApp (app.rs)**

Add a field to `NaygoApp`:
```rust
    /// Espacio por unidad (root → uso), rellenado async por un worker; lo pinta el árbol.
    disk_usage: std::collections::HashMap<std::path::PathBuf, naygo_core::disk::DiskUsage>,
    /// Canal del worker de espacio en curso (None si no hay escaneo activo).
    disk_rx: Option<std::sync::mpsc::Receiver<(std::path::PathBuf, naygo_core::disk::DiskUsage)>>,
    /// Frames desde el último escaneo (para re-escanear cada ~N frames sin reloj).
    disk_scan_ticks: u32,
```
Initialize in `new`: `disk_usage: std::collections::HashMap::new(), disk_rx: None, disk_scan_ticks: 0,`.

Add methods:
```rust
    /// Lanza un worker que lee el espacio de cada unidad y lo emite por canal. No
    /// solapa escaneos: si ya hay uno (`disk_rx` Some), no hace nada.
    fn start_disk_scan(&mut self) {
        if self.disk_rx.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for d in naygo_platform::drives() {
                if let Some((total, free)) = naygo_platform::drive_space::read_space(&d.path) {
                    let _ = tx.send((d.path.clone(), naygo_core::disk::DiskUsage { total, free }));
                }
            }
            // tx se cae al terminar → el receptor detecta Disconnected.
        });
        self.disk_rx = Some(rx);
    }

    /// Drena el worker de espacio (por frame) y re-escanea cada ~180 frames (~3s a 60fps).
    fn pump_disk_usage(&mut self) {
        // Drenar lo que haya llegado.
        let mut done = false;
        if let Some(rx) = &self.disk_rx {
            loop {
                match rx.try_recv() {
                    Ok((root, usage)) => {
                        self.disk_usage.insert(root, usage);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                }
            }
        }
        if done {
            self.disk_rx = None;
        }
        // Re-escaneo periódico (solo cuando no hay uno en curso).
        self.disk_scan_ticks = self.disk_scan_ticks.wrapping_add(1);
        if self.disk_rx.is_none() && self.disk_scan_ticks >= 180 {
            self.disk_scan_ticks = 0;
            self.start_disk_scan();
        }
    }
```
NOTE: usar un contador de frames (no reloj) evita `Date::now` (prohibido) y es suficiente. A 60fps son ~3s; si la app está idle y no repinta, el escaneo se pausa (aceptable: sin actividad, el espacio no cambia visiblemente). Para forzar el primer escaneo, llamar `self.start_disk_scan()` una vez en `new` (tras construir el struct) o en el primer `pump_disk_usage` (poner `disk_scan_ticks = 180` inicial para disparar en el primer frame). Elegir: inicializar `disk_scan_ticks: 180` para escanear en el primer frame.

- [ ] **Step 3: Llamar pump_disk_usage en el loop**

Modify `crates/ui/src/app.rs`: junto a `self.pump_ops();` (~1628), add `self.pump_disk_usage();`. Y en la condición de `request_repaint` (~1630), añadir `|| self.disk_rx.is_some()` para que el drenaje progrese mientras hay un escaneo.

- [ ] **Step 4: Pasar el mapa al árbol y pintar la barra**

Modify `crates/ui/src/panes/tree_panel.rs`: add a param `disk_usage: &std::collections::HashMap<std::path::PathBuf, naygo_core::disk::DiskUsage>` to `show` and `show_node` (thread it through). In `show_node`, AFTER the row content, if the node is a root drive (`node.drive_kind.is_some()` and `depth == 0`), look up `disk_usage.get(&node.path)` and, if present, render a usage bar + label:
```rust
            if depth == 0 && node.drive_kind.is_some() {
                if let Some(usage) = disk_usage.get(&node.path) {
                    let pct = usage.percent_used();
                    let frac = pct as f32 / 100.0;
                    let color = if usage.is_critical() {
                        egui::Color32::from_rgb(0xE0, 0x55, 0x55)
                    } else if usage.is_high() {
                        egui::Color32::from_rgb(0xE0, 0xA0, 0x30)
                    } else {
                        theme.accent() // azul del tema
                    };
                    // Barra fina indentada bajo el disco.
                    ui.horizontal(|ui| {
                        ui.add_space((depth as f32 * INDENT) + INDENT);
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(120.0, 5.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 2.0, egui::Color32::from_gray(60));
                        let mut fill = rect;
                        fill.set_width(rect.width() * frac);
                        ui.painter().rect_filled(fill, 2.0, color);
                    });
                    let label = i18n
                        .t("disk.usage")
                        .replace("{free}", &naygo_core::format::human_size(usage.free))
                        .replace("{total}", &naygo_core::format::human_size(usage.total))
                        .replace("{pct}", &pct.to_string());
                    ui.horizontal(|ui| {
                        ui.add_space((depth as f32 * INDENT) + INDENT);
                        ui.label(egui::RichText::new(label).weak().small());
                    });
                }
            }
```
VERIFY egui 0.34.3 painter API: `ui.allocate_exact_size`, `ui.painter().rect_filled(rect, rounding, color)` (rounding may be `egui::CornerRadius`/`f32`/`Rounding` depending on version — use what the codebase uses elsewhere, e.g. grep `rect_filled` in crates/ui/src). `theme.accent()` — confirm the ActiveTheme getter name (grep `fn accent` in theme_apply.rs; ops-A/2C used theme color getters). `RichText::weak().small()`. Adapt to real APIs.

- [ ] **Step 5: Update the `tree_panel::show` call site in app.rs**

Find where `tree_panel::show(...)` is called (grep) and pass `&self.disk_usage` as the new arg.

- [ ] **Step 6: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings` → clean; `cargo fmt --all` + `--check`.
MANUAL: open the app; the folders tree shows, under each drive root, a usage bar + "X libres de Y · N% usado"; C: bar colored by usage; the value appears shortly after start (async).

- [ ] **Step 7: Commit**
```
git add crates/ui/src/app.rs crates/ui/src/panes/tree_panel.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): espacio de discos en el árbol (barra+%+color, worker async)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: UI — strip de discos de acceso rápido en el toolbar

**Files:**
- Modify: `crates/ui/src/toolbar.rs`
- Modify: `crates/ui/src/app.rs` (si el toolbar necesita datos/acciones)

- [ ] **Step 1: Leer el toolbar actual**

READ `crates/ui/src/toolbar.rs` para ver su firma (`pub fn show(...)`/qué recibe) y cómo dispara navegación (probablemente emite acciones o llama a `app`). Replicar ese patrón.

- [ ] **Step 2: Añadir el strip de discos**

En el toolbar, añadir una fila/sección con un botón por unidad (de `naygo_platform::drives()` cacheado, o de las raíces del árbol/`self.disk_usage` keys). Cada botón muestra la letra/ícono de la unidad; al clicar, navega el panel activo a esa raíz. Reusar el mecanismo de navegación existente (p. ej. `self.navigate_active_to(root)` o la acción que ya use el árbol — grep cómo el árbol navega: `TreeAction::Navigate` o similar, y `app.rs` lo procesa). Para no re-escanear en el render, usar la lista que el worker ya mantiene: las keys de `self.disk_usage` o un `Vec<DriveInfo>` cacheado actualizado en `pump_disk_usage`. SIMPLE: mantener en `NaygoApp` un `drives_cache: Vec<naygo_platform::drives::DriveInfo>` actualizado al inicio de cada escaneo (`start_disk_scan` puede setearlo, o el worker enviarlo). Mínimo viable: en `start_disk_scan`, antes de spawnear, `self.drives_cache = naygo_platform::drives();` y el toolbar itera ese cache.

NOTE: `drives()` es barato (no toca espacio), así que cachearlo al inicio del escaneo es suficiente; el toolbar no llama `drives()` por frame.

```rust
// En NaygoApp: drives_cache: Vec<naygo_platform::DriveInfo>  (init Vec::new())
// En start_disk_scan, primera línea: self.drives_cache = naygo_platform::drives();
// En toolbar show(...), recibir &drives + un &mut Option<PathBuf> "navigate_to" (o un Vec<Action>/closure):
for d in drives {
    let label = d.path.to_string_lossy();
    if ui.button(label.as_ref()).on_hover_text(label.as_ref()).clicked() {
        *navigate_to = Some(d.path.clone());
    }
}
// app.rs, tras el toolbar: if let Some(root) = navigate_to { navegar el panel activo a root }.
```
Navegar a una raíz = lo mismo que activar un disco en el árbol. Reusar ese camino exacto (grep cómo el árbol navega a `node.path`: probablemente `f.navigate_to(path)` + `start_listing`). Llamar a ese helper.

- [ ] **Step 3: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all` + `--check`.
MANUAL: the toolbar shows drive buttons; clicking C:/D: navigates the active pane to that root; plugging a USB makes a new button appear within ~3s (re-scan).

- [ ] **Step 4: Commit**
```
git add crates/ui/src/toolbar.rs crates/ui/src/app.rs
git commit -m "feat(ui): strip de discos de acceso rápido en el toolbar

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README**

READ the current status block in `README.md` and replace it with:
```markdown
> **Estado:** Fase shell-A (abrir con app default + espacio de discos) en desarrollo.
> Diseño en
> [`docs/superpowers/specs/2026-06-08-naygo-shell-a-design.md`](docs/superpowers/specs/2026-06-08-naygo-shell-a-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-08-naygo-shell-a.md`](docs/superpowers/plans/2026-06-08-naygo-shell-a.md).
> Operaciones (ops-A), journal/retomar (ops-B), paste inteligente y bloque visual completos.
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compiles.
Run: `cargo test --workspace` → green.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → clean.
Run: `cargo fmt --all -- --check` → clean.
Run: `cargo build --release -p naygo-ui` → release compiles.
MANUAL end-to-end: abrir archivo (doble-clic/Enter), "Abrir con…", barra de espacio por disco con color, strip de discos clicable, re-escaneo detecta pendrive.

- [ ] **Step 3: Commit y push**
```
git add README.md
git commit -m "chore: actualizar estado del README (fase shell-A)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/shell-a
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| core::disk DiskUsage (used/percent_used/is_high/is_critical, saturación) | 1 |
| human_size → core::format (+tier TB) | 2 |
| Repuntar callers de ui + quitar copia | 3 |
| platform::open open_default/open_with_dialog (ShellExecuteW, ShellError) | 4 |
| platform::drive_space read_space (GetDiskFreeSpaceExW) | 5 |
| Abrir archivo (doble-clic/Enter) | 6 |
| "Abrir con…" en el menú | 6 |
| Menú reordenado + placeholder "Más opciones de Windows…" | 6 |
| Espacio en el árbol (barra+%+color) | 7 |
| Worker async + re-escaneo periódico (sin solape) | 7 |
| Strip de discos de acceso rápido | 8 |
| i18n ES/EN | 6, 7 |
| Menú COM / detección en vivo FUERA | (no se tocan) |

**Notas de riesgo:**
- **windows 0.62 API** (Tasks 4-5): `ShellExecuteW` (Option<HWND>, HINSTANCE return, `.0 as usize` vs 32), `GetDiskFreeSpaceExW` (Option<*mut u64> out-params, Result). Verificar contra `trash.rs` y los errores del compilador; la LÓGICA es lo fijo.
- **Mover human_size** (Task 3): cross-crate; grep que no quede `panes::file_panel::human_size`. Antes/después compila.
- **egui painter** (Task 7): `rect_filled` rounding type + `theme.accent()` getter — verificar contra uso existente (grep). 
- **Frame-tick re-scan** (Task 7): si la app está idle sin repintar, el escaneo se pausa; aceptable (sin actividad el espacio no cambia). El `request_repaint` mientras `disk_rx.is_some()` asegura que un escaneo en curso progrese.
- **focused_file vs context_focus** (Task 6): el menú enfoca la fila antes de procesar la acción (igual que Copy/Cut hoy); `focused_file()` resuelve la entry correcta. Si fallara, el patrón existente de Copy/Cut es la referencia.
- **Toolbar navegación** (Task 8): reusar el MISMO camino que el árbol para navegar a una raíz (no inventar); cachear `drives()` al inicio del escaneo (no por frame).
```
