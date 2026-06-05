# Naygo — Fase 1: Esqueleto navegable (plan de implementación)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Levantar el workspace de 3 crates (`naygo-core` / `naygo-platform` / `naygo-ui`) y producir una ventana egui con docking (egui_dock) que lista una carpeta real por streaming incremental y se navega 100% por teclado, sin que el hilo de UI haga I/O.

**Architecture:** Workspace Cargo de 3 crates con aislamiento forzado por el compilador. `naygo-core` es lógica pura sin egui ni Windows (tipos `Entry`/`PaneState`, motor de `listing` por canal, `CancellationToken`, sort). `naygo-platform` aísla Win32 (solo lo mínimo de esta fase: nada todavía, se crea el crate vacío con su header). `naygo-ui` corre el loop de eframe, integra `egui_dock` y pinta lo que el worker de listing emite por un `std::sync::mpsc`. El hilo de UI nunca bloquea: cada listado corre en un `std::thread` worker cancelable.

**Tech Stack:** Rust (toolchain `stable-msvc`), `eframe`/`egui` 0.34, `egui_dock` 0.19, `serde`/`serde_json` 1, `tracing` + `tracing-appender` para logging a archivo. El crate `windows` 0.62 se declara en `naygo-platform` pero no se usa hasta fases posteriores.

**Distribución (objetivo del proyecto):** Naygo debe poder llevarse a otras máquinas (VMs, equipos con pocos recursos) **sin que tengan Rust ni runtime instalado**. Rust compila a un `.exe` nativo; en esta fase dejamos el **build de release listo para empaquetar**: CRT estático (`+crt-static` → no requiere VC++ Redistributable), perfil optimizado a tamaño, metadatos de autoría embebidos en el `.exe`, y assets (íconos/idiomas/temas) embebidos en el binario o copiables al lado. El **instalador completo (ZIP portable + .msi/.exe)** se construye en una fase posterior con su propio plan (herramientas previstas: `cargo-wix` para MSI, NSIS para setup .exe).

**Alcance de esta fase (qué entra / qué NO):**
- ENTRA: workspace, `Entry`, `PaneState`, `SortSpec`, `ViewMode` (enum, solo modo Detalle implementado), motor de listing streaming + cancelación, sort puro, ventana eframe, layout con egui_dock (árbol | panel de archivos | inspector como tabs dockables), navegación por teclado básica (↑↓ Enter Backspace Tab Esc), type-ahead, barra de estado, panic handler + logging a archivo, **perfil de release distribuible (CRT estático + metadatos de autoría en el .exe) verificado en máquina/VM limpia**.
- NO ENTRA (fases siguientes): `ops` (copiar/mover/eliminar/renombrar/crear), `sizing`, i18n real (esta fase usa un stub de claves), themes a fondo, vistas lista/íconos, íconos del Shell, inspector con metadatos reales (esta fase muestra metadatos básicos de `std::fs::Metadata`), drag&drop, espacio de disco, breadcrumbs clicables, atajos configurables.

**Prerequisito (bloqueante):** El toolchain de Rust debe estar instalado y en el PATH (`rustc`, `cargo`, linker MSVC). Verificar con `cargo --version` antes de la Tarea 1. Si falla, instalar Visual Studio Build Tools ("Desktop development with C++") + rustup (https://rustup.rs/) y reabrir la terminal.

---

## Estructura de archivos (decisiones de decomposición)

```
explorador_de_archivos/
├── Cargo.toml                          # workspace root (members + perfil release)
├── .cargo/
│   └── config.toml                     # CRT estático (distribución sin VC++ Redist)
├── crates/
│   ├── core/
│   │   ├── Cargo.toml                  # naygo-core
│   │   └── src/
│   │       ├── lib.rs                  # re-exports públicos del crate
│   │       ├── cancel.rs               # CancellationToken (AtomicBool compartido)
│   │       ├── fs_model.rs             # Entry, EntryKind, PaneState, SortSpec, SortKey, ViewMode
│   │       ├── sort.rs                 # ordenamiento puro de Vec<Entry>
│   │       └── listing.rs             # motor de streaming: spawn_listing -> mpsc<ListingMsg>
│   ├── platform/
│   │   ├── Cargo.toml                  # naygo-platform (declara windows 0.62, sin uso aún)
│   │   └── src/
│   │       └── lib.rs                  # header + placeholder; módulos reales en fases futuras
│   └── ui/
│       ├── Cargo.toml                  # naygo-ui (bin) — depende de core y platform
│       ├── build.rs                    # embebe metadatos de autoría en el .exe (Windows)
│       ├── app.rc                      # recurso de versión: CompanyName/Copyright/etc.
│       └── src/
│           ├── main.rs                 # entrypoint: panic handler, logging, lanza eframe
│           ├── app.rs                  # NaygoApp: estado raíz, impl eframe::App, despacho de mpsc
│           ├── logging.rs              # init de tracing a archivo
│           ├── docking.rs              # TabViewer de egui_dock, definición de los tabs/paneles
│           ├── panes/
│           │   ├── mod.rs
│           │   ├── tree_panel.rs       # árbol (esqueleto: raíz + carpeta actual)
│           │   ├── file_panel.rs       # vista Detalle de las Entry del pane activo
│           │   └── inspector_panel.rs  # metadatos básicos del Entry seleccionado
│           └── input.rs                # mapeo teclado -> Action (función pura testeable)
└── docs/superpowers/plans/...
```

**Por qué así:** un crate por capa fuerza el aislamiento (el compilador impide que `core` importe `egui`/`windows`). Dentro de `ui`, los paneles se separan por responsabilidad (cada uno se sostiene en contexto). `input.rs` aísla el mapeo teclado→acción como función pura para poder testearlo sin egui (el spec pide extraer la lógica de UI testeable a funciones puras).

---

## Task 1: Scaffolding del workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `.cargo/config.toml` (CRT estático para distribución)
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`
- Create: `crates/platform/Cargo.toml`
- Create: `crates/platform/src/lib.rs`
- Create: `crates/ui/Cargo.toml`
- Create: `crates/ui/src/main.rs`

- [ ] **Step 1: Crear el `Cargo.toml` raíz del workspace**

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/core", "crates/platform", "crates/ui"]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Nicolás Groth <ngroth@gmail.com>"]
license = "MIT"
repository = "https://github.com/nicolasgroth/explorador_archivos_naygo"

[workspace.dependencies]
eframe = "0.34"
egui = "0.34"
egui_dock = "0.19"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
windows = "0.62"

# Perfil de release optimizado para distribución: binario chico, sin símbolos,
# pensado para correr en máquinas con pocos recursos. NOTA: la velocidad es la
# prioridad del proyecto, así que usamos opt-level="s" (tamaño sin sacrificar
# tanto rendimiento como "z") y mantenemos unwind para poder recuperar paneles
# de un panic en un worker (la app "nunca cae").
[profile.release]
opt-level = "s"      # optimizar a tamaño, pero menos agresivo que "z" (velocidad importa)
lto = true           # link-time optimization: binario más chico y rápido
codegen-units = 1    # mejor optimización a costa de tiempo de compilación
strip = true         # quitar símbolos de debug del .exe
# panic = unwind (default): permite catch_unwind en workers; el panic handler loguea.
```

Create `.cargo/config.toml` (CRT estático: el `.exe` no dependerá del VC++ Redistributable, corre en cualquier Windows 10/11 limpio):

```toml
# Naygo — enlazar el runtime de C estáticamente para distribución sin dependencias.
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

- [ ] **Step 2: Crear `naygo-core`**

Create `crates/core/Cargo.toml`:

```toml
[package]
name = "naygo-core"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
```

Create `crates/core/src/lib.rs`:

```rust
// Naygo — núcleo: lógica pura de filesystem, sin UI ni Windows.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `naygo-core` contiene toda la lógica testeable del explorador: modelo de
//! filesystem, motor de listado por streaming, ordenamiento y cancelación.
//! No depende de egui ni de Windows.

pub fn hello() -> &'static str {
    "naygo-core"
}
```

- [ ] **Step 3: Crear `naygo-platform`**

Create `crates/platform/Cargo.toml`:

```toml
[package]
name = "naygo-platform"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[target.'cfg(windows)'.dependencies]
windows = { workspace = true }

[dependencies]
naygo-core = { path = "../core" }
tracing = { workspace = true }
```

Create `crates/platform/src/lib.rs`:

```rust
// Naygo — capa de plataforma: todo lo que toca Windows, aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `naygo-platform` aísla las llamadas a Win32 / Shell / COM tras interfaces
//! limpias. En la Fase 1 está vacío a propósito: la integración del Shell
//! (íconos, ShellExecute, papelera, discos) y el drag&drop COM llegan en fases
//! posteriores. Este crate existe ahora para fijar la frontera arquitectónica.

pub fn hello() -> &'static str {
    "naygo-platform"
}
```

- [ ] **Step 4: Crear `naygo-ui` (binario)**

Create `crates/ui/Cargo.toml`:

```toml
[package]
name = "naygo-ui"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[[bin]]
name = "naygo"
path = "src/main.rs"

[dependencies]
naygo-core = { path = "../core" }
naygo-platform = { path = "../platform" }
eframe = { workspace = true }
egui = { workspace = true }
egui_dock = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
tracing-appender = { workspace = true }
```

Create `crates/ui/src/main.rs` (placeholder mínimo, se reemplaza en Tarea 7):

```rust
// Naygo — explorador de archivos. Entrypoint de la aplicación.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

fn main() {
    println!("{} + {}", naygo_core::hello(), naygo_platform::hello());
}
```

- [ ] **Step 5: Verificar que el workspace compila**

Run: `cargo build`
Expected: compila los 3 crates sin errores (puede tardar la primera vez por descargar egui).

- [ ] **Step 6: Verificar que el binario corre**

Run: `cargo run -p naygo-ui`
Expected: imprime `naygo-core + naygo-platform`

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml .cargo/ crates/
git commit -m "feat: scaffolding del workspace de 3 crates (core/platform/ui) + perfil release distribuible"
```

---

## Task 2: CancellationToken

**Files:**
- Create: `crates/core/src/cancel.rs`
- Modify: `crates/core/src/lib.rs`
- Test: en el mismo `cancel.rs` (módulo `#[cfg(test)]`)

- [ ] **Step 1: Escribir el test que falla**

Create `crates/core/src/cancel.rs`:

```rust
// Naygo — token de cancelación compartido para operaciones largas.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `CancellationToken` es un flag booleano compartido entre el hilo de UI (que
//! puede pedir cancelar) y un worker (que lo chequea entre cada paso). Clonar el
//! token comparte el mismo estado interno.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Token de cancelación compartido. Barato de clonar (Arc).
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Crea un token nuevo, no cancelado.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marca el token como cancelado. Idempotente.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// `true` si alguien ya pidió cancelar.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_nuevo_no_esta_cancelado() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancelar_se_propaga_a_los_clones() {
        let token = CancellationToken::new();
        let clon = token.clone();
        token.cancel();
        assert!(clon.is_cancelled(), "el clon comparte el estado");
    }
}
```

Modify `crates/core/src/lib.rs` — añadir bajo el `//!` doc y reemplazar el `hello`:

```rust
pub mod cancel;

pub use cancel::CancellationToken;
```

(Borra la función `hello` de core y su uso en `main.rs` Step de Tarea 1 ya no aplica — `main.rs` se reescribe en Tarea 7; por ahora cambia `main.rs` a `fn main() { println!("naygo"); }` para que siga compilando.)

- [ ] **Step 2: Correr el test y verificar que pasa**

Run: `cargo test -p naygo-core cancel`
Expected: PASS (2 tests: `token_nuevo_no_esta_cancelado`, `cancelar_se_propaga_a_los_clones`)

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/cancel.rs crates/core/src/lib.rs crates/ui/src/main.rs
git commit -m "feat(core): CancellationToken compartido y testeado"
```

---

## Task 3: Modelo de filesystem (`fs_model`)

**Files:**
- Create: `crates/core/src/fs_model.rs`
- Modify: `crates/core/src/lib.rs`
- Test: módulo `#[cfg(test)]` en `fs_model.rs`

- [ ] **Step 1: Escribir el módulo con su test**

Create `crates/core/src/fs_model.rs`:

```rust
// Naygo — modelo de filesystem: tipos POCO sin lógica de I/O.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Tipos planos que describen lo que se ve en un panel. No tocan el disco: son
//! datos que el motor de `listing` produce y que la UI consume.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// Si una entrada es archivo, carpeta o un tipo que no pudimos clasificar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    Directory,
    File,
    /// Symlink, junction, device, etc. — se muestra pero no se asume navegable.
    Other,
}

/// Una entrada del filesystem tal como la pinta la UI.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    /// Tamaño en bytes. `None` para carpetas (se calcula bajo demanda en otra fase).
    pub size: Option<u64>,
    /// Fecha de última modificación, si el SO la entrega.
    pub modified: Option<SystemTime>,
    /// Atributo "oculto" (en Windows). En esta fase se rellena en fases futuras; default false.
    pub hidden: bool,
}

impl Entry {
    /// `true` si es una carpeta navegable.
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Directory
    }
}

/// Modos de vista del panel de archivos. En la Fase 1 solo `Details` se pinta.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    Details,
    List,
    Icons,
}

impl Default for ViewMode {
    fn default() -> Self {
        ViewMode::Details
    }
}

/// Clave por la que se ordena un panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortKey {
    Name,
    Size,
    Modified,
    Kind,
}

/// Especificación de ordenamiento: por qué clave y en qué dirección.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortSpec {
    pub key: SortKey,
    pub ascending: bool,
    /// Si las carpetas van siempre antes que los archivos (estilo Explorer).
    pub dirs_first: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        SortSpec { key: SortKey::Name, ascending: true, dirs_first: true }
    }
}

/// Estado de un panel de archivos: dónde está parado, qué ve y qué hay seleccionado.
#[derive(Clone, Debug)]
pub struct PaneState {
    pub current_dir: PathBuf,
    pub entries: Vec<Entry>,
    pub sort: SortSpec,
    pub view: ViewMode,
    /// Índice de la entrada con foco dentro de `entries`, si hay alguna.
    pub focused: Option<usize>,
    /// Índices marcados (selección múltiple).
    pub selected: Vec<usize>,
}

impl PaneState {
    /// Crea un panel vacío parado en `dir`.
    pub fn new(dir: PathBuf) -> Self {
        PaneState {
            current_dir: dir,
            entries: Vec::new(),
            sort: SortSpec::default(),
            view: ViewMode::default(),
            focused: None,
            selected: Vec::new(),
        }
    }

    /// Entrada actualmente con foco, si existe.
    pub fn focused_entry(&self) -> Option<&Entry> {
        self.focused.and_then(|i| self.entries.get(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_directory_es_dir() {
        let e = Entry {
            name: "docs".into(),
            path: PathBuf::from("C:/docs"),
            kind: EntryKind::Directory,
            size: None,
            modified: None,
            hidden: false,
        };
        assert!(e.is_dir());
    }

    #[test]
    fn pane_nuevo_no_tiene_foco_ni_seleccion() {
        let p = PaneState::new(PathBuf::from("C:/"));
        assert!(p.focused_entry().is_none());
        assert!(p.selected.is_empty());
        assert_eq!(p.view, ViewMode::Details);
        assert_eq!(p.sort.key, SortKey::Name);
    }
}
```

Modify `crates/core/src/lib.rs` — añadir:

```rust
pub mod fs_model;

pub use fs_model::{Entry, EntryKind, PaneState, SortKey, SortSpec, ViewMode};
```

- [ ] **Step 2: Correr los tests y verificar que pasan**

Run: `cargo test -p naygo-core fs_model`
Expected: PASS (2 tests)

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/fs_model.rs crates/core/src/lib.rs
git commit -m "feat(core): tipos de fs_model (Entry, PaneState, SortSpec, ViewMode)"
```

---

## Task 4: Ordenamiento puro (`sort`)

**Files:**
- Create: `crates/core/src/sort.rs`
- Modify: `crates/core/src/lib.rs`
- Test: módulo `#[cfg(test)]` en `sort.rs`

- [ ] **Step 1: Escribir el test que falla primero**

Create `crates/core/src/sort.rs` con SOLO la firma y los tests (la implementación viene en Step 3):

```rust
// Naygo — ordenamiento puro de entradas de un panel.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Ordena un `Vec<Entry>` según un `SortSpec`. Función pura: misma entrada,
//! misma salida; sin I/O. Comparación de nombres case-insensitive (estilo
//! Windows). El orden es estable para que reordenar no "salte".

use crate::fs_model::{Entry, SortKey, SortSpec};

/// Ordena `entries` in-place según `spec`.
pub fn sort_entries(entries: &mut [Entry], spec: &SortSpec) {
    let _ = (entries, spec);
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_model::EntryKind;
    use std::path::PathBuf;

    fn entry(name: &str, kind: EntryKind, size: u64) -> Entry {
        Entry {
            name: name.into(),
            path: PathBuf::from(name),
            kind,
            size: Some(size),
            modified: None,
            hidden: false,
        }
    }

    #[test]
    fn por_nombre_ascendente_case_insensitive() {
        let mut v = vec![
            entry("banana.txt", EntryKind::File, 1),
            entry("Apple.txt", EntryKind::File, 1),
            entry("cherry.txt", EntryKind::File, 1),
        ];
        let spec = SortSpec { key: SortKey::Name, ascending: true, dirs_first: false };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["Apple.txt", "banana.txt", "cherry.txt"]);
    }

    #[test]
    fn dirs_first_pone_carpetas_arriba() {
        let mut v = vec![
            entry("zeta.txt", EntryKind::File, 1),
            entry("alpha_dir", EntryKind::Directory, 0),
        ];
        let spec = SortSpec { key: SortKey::Name, ascending: true, dirs_first: true };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["alpha_dir", "zeta.txt"], "la carpeta va primero");
    }

    #[test]
    fn por_tamano_descendente() {
        let mut v = vec![
            entry("small", EntryKind::File, 10),
            entry("big", EntryKind::File, 100),
            entry("mid", EntryKind::File, 50),
        ];
        let spec = SortSpec { key: SortKey::Size, ascending: false, dirs_first: false };
        sort_entries(&mut v, &spec);
        let names: Vec<&str> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["big", "mid", "small"]);
    }
}
```

Modify `crates/core/src/lib.rs` — añadir:

```rust
pub mod sort;

pub use sort::sort_entries;
```

- [ ] **Step 2: Correr el test y verificar que falla**

Run: `cargo test -p naygo-core sort`
Expected: los 3 tests fallan/panic con `not implemented` (la función es `unimplemented!()`).

- [ ] **Step 3: Implementar `sort_entries`**

Reemplaza el cuerpo de `sort_entries` en `crates/core/src/sort.rs`:

```rust
pub fn sort_entries(entries: &mut [Entry], spec: &SortSpec) {
    entries.sort_by(|a, b| {
        // Carpetas primero si así se pidió, sin importar la clave.
        if spec.dirs_first {
            let a_dir = a.is_dir();
            let b_dir = b.is_dir();
            if a_dir != b_dir {
                // dir (true) debe ir antes que archivo (false).
                return b_dir.cmp(&a_dir);
            }
        }

        let ordering = match spec.key {
            SortKey::Name => cmp_name(a, b),
            SortKey::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            SortKey::Modified => a.modified.cmp(&b.modified),
            SortKey::Kind => format!("{:?}", a.kind).cmp(&format!("{:?}", b.kind)),
        };

        if spec.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    });
}

/// Comparación de nombres case-insensitive estilo Windows.
fn cmp_name(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    a.name.to_lowercase().cmp(&b.name.to_lowercase())
}
```

- [ ] **Step 4: Correr los tests y verificar que pasan**

Run: `cargo test -p naygo-core sort`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/sort.rs crates/core/src/lib.rs
git commit -m "feat(core): ordenamiento puro de entradas (nombre/tamaño/fecha, dirs-first)"
```

---

## Task 5: Motor de listing por streaming + cancelación

**Files:**
- Create: `crates/core/src/listing.rs`
- Modify: `crates/core/src/lib.rs`
- Test: módulo `#[cfg(test)]` en `listing.rs` (usa `tempfile` como dev-dependency)

- [ ] **Step 1: Añadir `tempfile` como dev-dependency de core**

Modify `crates/core/Cargo.toml` — añadir al final:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Escribir el motor con sus tests**

Create `crates/core/src/listing.rs`:

```rust
// Naygo — motor de listado por streaming incremental, cancelable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lee un directorio en un hilo worker y emite cada entrada por un canal a
//! medida que la descubre, sin acumular todo antes de responder. Chequea el
//! `CancellationToken` entre cada entrada: cancelar corta el listado al instante.
//! El hilo de UI nunca llama a estas funciones directamente sobre el disco; usa
//! `spawn_listing`, que devuelve el receptor del canal.

use crate::cancel::CancellationToken;
use crate::fs_model::{Entry, EntryKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

/// Mensajes que el worker de listado emite hacia la UI.
#[derive(Debug)]
pub enum ListingMsg {
    /// Una entrada recién descubierta.
    Entry(Entry),
    /// El directorio no se pudo abrir (permiso, ruta inexistente, disco caído).
    Error(String),
    /// El listado terminó de forma natural (recorrió todo).
    Done,
    /// El listado se abortó porque el token fue cancelado.
    Cancelled,
}

/// Lanza el listado de `dir` en un hilo worker. Devuelve el receptor del canal
/// (la UI lo drena frame a frame) y el `JoinHandle` (por si se quiere unir).
pub fn spawn_listing(
    dir: PathBuf,
    token: CancellationToken,
) -> (Receiver<ListingMsg>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        list_into(&dir, &token, &tx);
    });
    (rx, handle)
}

/// Cuerpo del worker: recorre el directorio emitiendo por `tx`. Extraído para
/// poder testearlo de forma síncrona sin spawnear un hilo.
fn list_into(dir: &Path, token: &CancellationToken, tx: &mpsc::Sender<ListingMsg>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            let _ = tx.send(ListingMsg::Error(e.to_string()));
            return;
        }
    };

    for dirent in read_dir {
        if token.is_cancelled() {
            let _ = tx.send(ListingMsg::Cancelled);
            return;
        }

        let dirent = match dirent {
            Ok(d) => d,
            // Una entrada ilegible no aborta todo el listado: se salta.
            Err(_) => continue,
        };

        let entry = entry_from_dirent(&dirent);
        // Si el receptor se cayó (la UI cambió de carpeta), dejar de trabajar.
        if tx.send(ListingMsg::Entry(entry)).is_err() {
            return;
        }
    }

    if token.is_cancelled() {
        let _ = tx.send(ListingMsg::Cancelled);
    } else {
        let _ = tx.send(ListingMsg::Done);
    }
}

/// Construye un `Entry` a partir de un `DirEntry`, tolerando metadata ausente.
fn entry_from_dirent(dirent: &std::fs::DirEntry) -> Entry {
    let path = dirent.path();
    let name = dirent.file_name().to_string_lossy().into_owned();
    let metadata = dirent.metadata().ok();

    let kind = match &metadata {
        Some(m) if m.is_dir() => EntryKind::Directory,
        Some(m) if m.is_file() => EntryKind::File,
        Some(_) => EntryKind::Other,
        None => EntryKind::Other,
    };

    let size = match (&metadata, kind) {
        (Some(m), EntryKind::File) => Some(m.len()),
        _ => None,
    };

    let modified = metadata.as_ref().and_then(|m| m.modified().ok());

    Entry { name, path, kind, size, modified, hidden: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn lista_archivos_de_un_directorio() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"hola").unwrap();
        fs::create_dir(dir.path().join("subcarpeta")).unwrap();

        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        list_into(dir.path(), &token, &tx);
        drop(tx);

        let mut nombres = Vec::new();
        let mut done = false;
        for msg in rx {
            match msg {
                ListingMsg::Entry(e) => nombres.push(e.name),
                ListingMsg::Done => done = true,
                other => panic!("mensaje inesperado: {:?}", other),
            }
        }
        nombres.sort();
        assert_eq!(nombres, vec!["a.txt", "subcarpeta"]);
        assert!(done, "debe terminar con Done");
    }

    #[test]
    fn token_cancelado_antes_de_empezar_no_emite_entradas() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"x").unwrap();

        let token = CancellationToken::new();
        token.cancel();
        let (tx, rx) = mpsc::channel();
        list_into(dir.path(), &token, &tx);
        drop(tx);

        let msgs: Vec<_> = rx.into_iter().collect();
        // Con el token ya cancelado, el primer chequeo del loop aborta:
        // o no entró al loop (dir vacío del iterador) o cortó en la 1ª vuelta.
        assert!(
            msgs.iter().any(|m| matches!(m, ListingMsg::Cancelled)),
            "debe emitir Cancelled, got {:?}",
            msgs
        );
        assert!(
            !msgs.iter().any(|m| matches!(m, ListingMsg::Entry(_))),
            "no debe emitir entradas tras cancelar"
        );
    }

    #[test]
    fn directorio_inexistente_emite_error() {
        let token = CancellationToken::new();
        let (tx, rx) = mpsc::channel();
        list_into(Path::new("Z:/ruta/que/no/existe/naygo"), &token, &tx);
        drop(tx);

        let msgs: Vec<_> = rx.into_iter().collect();
        assert!(
            matches!(msgs.first(), Some(ListingMsg::Error(_))),
            "debe emitir Error, got {:?}",
            msgs
        );
    }
}
```

Modify `crates/core/src/lib.rs` — añadir:

```rust
pub mod listing;

pub use listing::{spawn_listing, ListingMsg};
```

- [ ] **Step 3: Correr los tests y verificar que pasan**

Run: `cargo test -p naygo-core listing`
Expected: PASS (3 tests). Nota: `token_cancelado_antes_de_empezar` depende de que el primer dirent del iterador exista; el archivo creado lo garantiza, así que el chequeo `is_cancelled` en la 1ª vuelta del loop emite `Cancelled`.

- [ ] **Step 4: Correr toda la suite de core**

Run: `cargo test -p naygo-core`
Expected: PASS (todos: cancel + fs_model + sort + listing).

- [ ] **Step 5: Commit**

```bash
git add crates/core/Cargo.toml crates/core/src/listing.rs crates/core/src/lib.rs
git commit -m "feat(core): motor de listing por streaming, cancelable y tolerante a errores"
```

---

## Task 6: Mapeo de teclado a acciones (`input`, función pura)

**Files:**
- Create: `crates/ui/src/input.rs`
- Test: módulo `#[cfg(test)]` en `input.rs` (no necesita egui para el mapeo, usamos tipos propios)

- [ ] **Step 1: Escribir el módulo con sus tests**

Create `crates/ui/src/input.rs`:

```rust
// Naygo — mapeo de teclado a acciones de navegación (lógica pura testeable).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Traduce una pulsación de tecla (representada con tipos propios, no egui) a una
//! `Action` de alto nivel. Se aísla aquí para testear el mapeo sin levantar la UI.
//! En la Fase 1 el mapa es fijo (default estilo Windows); los atajos
//! configurables llegan en una fase posterior.

/// Teclas que nos interesan en la Fase 1. Espejo reducido de `egui::Key`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    Enter,
    Backspace,
    Tab,
    Escape,
}

/// Acción de alto nivel resultante de una tecla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    MoveUp,
    MoveDown,
    /// Entrar a la carpeta enfocada / abrir el archivo enfocado.
    Activate,
    /// Subir un nivel (carpeta padre).
    GoUp,
    /// Cambiar el panel de archivos activo.
    SwitchPane,
    /// Cancelar el listado en curso.
    CancelListing,
}

/// Mapea una tecla a su acción, si tiene una asignada en la Fase 1.
pub fn map_key(key: Key) -> Option<Action> {
    Some(match key {
        Key::ArrowUp => Action::MoveUp,
        Key::ArrowDown => Action::MoveDown,
        Key::Enter => Action::Activate,
        Key::Backspace | Key::ArrowLeft => Action::GoUp,
        Key::Tab => Action::SwitchPane,
        Key::Escape => Action::CancelListing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flechas_mueven_seleccion() {
        assert_eq!(map_key(Key::ArrowUp), Some(Action::MoveUp));
        assert_eq!(map_key(Key::ArrowDown), Some(Action::MoveDown));
    }

    #[test]
    fn backspace_y_flecha_izquierda_suben_nivel() {
        assert_eq!(map_key(Key::Backspace), Some(Action::GoUp));
        assert_eq!(map_key(Key::ArrowLeft), Some(Action::GoUp));
    }

    #[test]
    fn enter_activa_y_escape_cancela() {
        assert_eq!(map_key(Key::Enter), Some(Action::Activate));
        assert_eq!(map_key(Key::Escape), Some(Action::CancelListing));
    }
}
```

- [ ] **Step 2: Correr los tests y verificar que pasan**

Run: `cargo test -p naygo-ui input`
Expected: PASS (3 tests). (Como `input.rs` aún no está declarado en un crate compilable con `main`, este step puede requerir que `main.rs` declare `mod input;` — se hace en Tarea 7. Si `cargo test -p naygo-ui` no encuentra el módulo, continúa: el test corre tras la Tarea 7. Para testear ya mismo, añade temporalmente `mod input;` al `main.rs` placeholder.)

Para poder correr el test ahora, Modify `crates/ui/src/main.rs`:

```rust
// Naygo — explorador de archivos. Entrypoint de la aplicación.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

mod input;

fn main() {
    println!("naygo");
}
```

Run de nuevo: `cargo test -p naygo-ui input`
Expected: PASS (3 tests).

- [ ] **Step 3: Commit**

```bash
git add crates/ui/src/input.rs crates/ui/src/main.rs
git commit -m "feat(ui): mapeo puro de teclado a acciones de navegación"
```

---

## Task 7: Logging a archivo + panic handler + entrypoint eframe

**Files:**
- Create: `crates/ui/src/logging.rs`
- Modify: `crates/ui/src/main.rs`
- Create (stub que se rellena en Tarea 8): `crates/ui/src/app.rs`

- [ ] **Step 1: Escribir el init de logging**

Create `crates/ui/src/logging.rs`:

```rust
// Naygo — inicialización de logging a archivo (sin telemetría).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Logging básico a archivo desde el día uno. Escribe a `logs/naygo.log` junto
//! al ejecutable usando un appender con rotación diaria. Sin telemetría, sin red.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Inicializa el subscriber global. Devuelve un guard que debe vivir tanto como
/// el programa (si se dropea, se pierden logs pendientes en el buffer).
pub fn init() -> WorkerGuard {
    let file_appender = tracing_appender::rolling::daily("logs", "naygo.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,naygo_core=debug,naygo_ui=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    tracing::info!("Naygo iniciado");
    guard
}
```

- [ ] **Step 2: Escribir el `main.rs` definitivo con panic handler**

Replace `crates/ui/src/main.rs`:

```rust
// Naygo — explorador de archivos rápido para Windows. Entrypoint.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
//
// Naygo — un explorador de archivos estilo Commander.
// Autor: Nicolás Groth / ISGroth (Chile). Licencia MIT.

// En release, no abrir consola en Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod docking;
mod input;
mod logging;
mod panes;

use app::NaygoApp;

fn main() -> eframe::Result<()> {
    let _log_guard = logging::init();
    install_panic_handler();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Naygo")
            .with_inner_size([1100.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Naygo",
        native_options,
        Box::new(|cc| Ok(Box::new(NaygoApp::new(cc)))),
    )
}

/// Captura panics: los loguea en vez de morir en silencio. Sigue el principio del
/// spec: la app comunica el fallo, no desaparece sin rastro.
fn install_panic_handler() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("PANIC: {info}");
        default_hook(info);
    }));
}
```

- [ ] **Step 3: Crear el stub de `app.rs` para que compile**

Create `crates/ui/src/app.rs` (stub; se completa en Tarea 8):

```rust
// Naygo — estado raíz de la aplicación y loop de egui.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

use eframe::CreationContext;

/// Estado raíz de Naygo. En la Tarea 8 se le agregan los paneles y el docking.
pub struct NaygoApp {}

impl NaygoApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        NaygoApp {}
    }
}

impl eframe::App for NaygoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Naygo");
            ui.label("Esqueleto en construcción…");
        });
    }
}
```

Create stubs vacíos para que `mod docking;` y `mod panes;` compilen:

Create `crates/ui/src/docking.rs`:

```rust
// Naygo — integración de egui_dock para paneles dinámicos dockables.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

// Contenido real en la Tarea 8.
```

Create `crates/ui/src/panes/mod.rs`:

```rust
// Naygo — paneles de la UI (árbol, archivos, inspector).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

pub mod file_panel;
pub mod inspector_panel;
pub mod tree_panel;
```

Create `crates/ui/src/panes/tree_panel.rs`, `file_panel.rs`, `inspector_panel.rs` — cada uno con su header y vacío por ahora:

```rust
// Naygo — panel de árbol de carpetas. (Contenido real en Tarea 8.)
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```

```rust
// Naygo — panel de archivos (vista Detalle). (Contenido real en Tarea 8.)
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```

```rust
// Naygo — panel inspector de metadatos. (Contenido real en Tarea 8.)
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```

- [ ] **Step 4: Verificar que compila y abre ventana**

Run: `cargo run -p naygo-ui`
Expected: abre una ventana titulada "Naygo" con el texto "Esqueleto en construcción…". Cerrar la ventana. Verificar que se creó `logs/naygo.log` con la línea "Naygo iniciado".

- [ ] **Step 5: Commit**

```bash
git add crates/ui/src/
git commit -m "feat(ui): entrypoint eframe con logging a archivo y panic handler"
```

---

## Task 8: Docking + paneles + navegación por teclado en vivo

> **⚠️ API ACTUALIZADA (egui/eframe 0.34.3, verificado contra la fuente):** El
> código de esta tarea que sigue más abajo fue escrito contra una API de egui
> ANTERIOR y NO compila tal cual en 0.34.3. El patrón correcto en 0.34.3 es:
> - `eframe::App` requiere `fn ui(&mut self, ui: &mut egui::Ui, frame)` (no
>   `update(ctx)`); la lógica que NO pinta va en el opcional
>   `fn logic(&mut self, ctx: &egui::Context, frame)` (corre antes de `ui`).
> - Los paneles (`CentralPanel`/`TopBottomPanel`/`SidePanel`) y `DockArea` se
>   pintan con `.show_inside(ui, ...)`, NO con `.show(ctx, ...)` (deprecado).
> - Dentro de `ui()` se obtiene el contexto con `ui.ctx()`; repintar:
>   `ui.ctx().request_repaint()`. `Style::from_egui(ui.style().as_ref())`.
> - El `app_creator` de `run_native` devuelve `Result<Box<dyn App>, _>` → `Ok(Box::new(..))`.
>
> El implementador de esta tarea recibe el código YA CORREGIDO en su prompt y debe
> seguir ESE, no el markdown de abajo. El markdown se conserva como referencia de
> intención (qué hace cada panel), no como fuente literal.

Esta es la tarea grande: conecta el motor de listing con la UI vía mpsc, mete egui_dock, pinta el panel de archivos en vista Detalle, y cablea el teclado. Se subdivide en pasos chicos.

**Files:**
- Modify: `crates/ui/src/app.rs` (estado real + despacho de mpsc + acciones)
- Modify: `crates/ui/src/docking.rs` (TabViewer de egui_dock)
- Modify: `crates/ui/src/panes/file_panel.rs`
- Modify: `crates/ui/src/panes/tree_panel.rs`
- Modify: `crates/ui/src/panes/inspector_panel.rs`

- [ ] **Step 1: Definir el estado de la app y la conexión con el worker de listing**

Replace `crates/ui/src/app.rs`:

```rust
// Naygo — estado raíz de la aplicación y loop de egui.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! `NaygoApp` mantiene el estado de un panel de archivos, drena el canal del
//! worker de listing cada frame (sin bloquear), y traduce el teclado a acciones.
//! El hilo de UI nunca lee el disco: solo dispara `spawn_listing` y consume mpsc.

use crate::input::{map_key, Action, Key as NaygoKey};
use eframe::CreationContext;
use egui_dock::{DockArea, DockState, NodeIndex, Style};
use naygo_core::cancel::CancellationToken;
use naygo_core::fs_model::PaneState;
use naygo_core::listing::{spawn_listing, ListingMsg};
use naygo_core::sort::sort_entries;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// Qué panel ocupa cada tab del dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneTab {
    Tree,
    Files,
    Inspector,
}

/// Estado compartido que los paneles del dock leen y modifican.
pub struct UiState {
    pub pane: PaneState,
    /// Recibe entradas del worker activo; `None` si no hay listado en curso.
    pub listing_rx: Option<Receiver<ListingMsg>>,
    /// Token del listado en curso, para cancelarlo.
    pub listing_token: CancellationToken,
    /// Texto de estado en la barra inferior.
    pub status: String,
}

impl UiState {
    /// Empieza a listar `dir`: cancela el listado anterior y lanza uno nuevo.
    pub fn navigate_to(&mut self, dir: PathBuf) {
        self.listing_token.cancel(); // corta el worker anterior si lo hay
        let token = CancellationToken::new();
        let (rx, _handle) = spawn_listing(dir.clone(), token.clone());
        self.pane = PaneState::new(dir);
        self.listing_rx = Some(rx);
        self.listing_token = token;
        self.status = "Listando…".to_string();
    }

    /// Drena lo que el worker haya emitido hasta ahora, sin bloquear.
    pub fn pump_listing(&mut self) {
        let Some(rx) = &self.listing_rx else { return };
        let mut finished = false;
        // try_recv en loop: consume lo disponible y vuelve a la UI.
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ListingMsg::Entry(e) => self.pane.entries.push(e),
                ListingMsg::Done => {
                    finished = true;
                    self.status = format!("{} elementos", self.pane.entries.len());
                }
                ListingMsg::Cancelled => {
                    finished = true;
                    self.status = "Cancelado".to_string();
                }
                ListingMsg::Error(err) => {
                    finished = true;
                    self.status = format!("Error: {err}");
                }
            }
        }
        if finished {
            let spec = self.pane.sort;
            sort_entries(&mut self.pane.entries, &spec);
            if self.pane.focused.is_none() && !self.pane.entries.is_empty() {
                self.pane.focused = Some(0);
            }
            self.listing_rx = None;
        }
    }

    /// Aplica una acción de navegación al estado del panel.
    pub fn apply_action(&mut self, action: Action) {
        match action {
            Action::MoveUp => self.move_focus(-1),
            Action::MoveDown => self.move_focus(1),
            Action::Activate => self.activate_focused(),
            Action::GoUp => self.go_up(),
            Action::CancelListing => {
                self.listing_token.cancel();
            }
            Action::SwitchPane => { /* multi-panel llega en fase posterior */ }
        }
    }

    fn move_focus(&mut self, delta: isize) {
        if self.pane.entries.is_empty() {
            return;
        }
        let len = self.pane.entries.len() as isize;
        let cur = self.pane.focused.unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, len - 1);
        self.pane.focused = Some(next as usize);
    }

    fn activate_focused(&mut self) {
        let Some(entry) = self.pane.focused_entry().cloned() else { return };
        if entry.is_dir() {
            self.navigate_to(entry.path);
        } else {
            // Abrir con el programa por defecto llega con platform::shell (fase posterior).
            self.status = format!("Abrir: {} (pendiente platform::shell)", entry.name);
        }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.pane.current_dir.parent() {
            self.navigate_to(parent.to_path_buf());
        }
    }
}

/// Estado raíz: el dock y el estado compartido de los paneles.
pub struct NaygoApp {
    dock_state: DockState<PaneTab>,
    ui_state: UiState,
}

impl NaygoApp {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        // Layout inicial: árbol a la izquierda | archivos al centro | inspector a la derecha.
        let mut dock_state = DockState::new(vec![PaneTab::Files]);
        let surface = dock_state.main_surface_mut();
        let [files, _tree] =
            surface.split_left(NodeIndex::root(), 0.22, vec![PaneTab::Tree]);
        surface.split_right(files, 0.78, vec![PaneTab::Inspector]);

        let start_dir = default_start_dir();

        let mut ui_state = UiState {
            pane: PaneState::new(start_dir.clone()),
            listing_rx: None,
            listing_token: CancellationToken::new(),
            status: String::new(),
        };
        ui_state.navigate_to(start_dir);

        NaygoApp { dock_state, ui_state }
    }

    /// Lee el teclado y aplica acciones. Se llama una vez por frame.
    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let keys = [
            (egui::Key::ArrowUp, NaygoKey::ArrowUp),
            (egui::Key::ArrowDown, NaygoKey::ArrowDown),
            (egui::Key::ArrowLeft, NaygoKey::ArrowLeft),
            (egui::Key::Enter, NaygoKey::Enter),
            (egui::Key::Backspace, NaygoKey::Backspace),
            (egui::Key::Tab, NaygoKey::Tab),
            (egui::Key::Escape, NaygoKey::Escape),
        ];
        let mut actions = Vec::new();
        ctx.input(|i| {
            for (egui_key, naygo_key) in keys {
                if i.key_pressed(egui_key) {
                    if let Some(action) = map_key(naygo_key) {
                        actions.push(action);
                    }
                }
            }
        });
        for action in actions {
            self.ui_state.apply_action(action);
        }
    }
}

impl eframe::App for NaygoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1) Consumir lo que el worker haya producido (no bloquea).
        self.ui_state.pump_listing();
        // 2) Teclado.
        self.handle_keyboard(ctx);

        // 3) Barra de estado abajo.
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.ui_state.pane.current_dir.display().to_string());
                ui.separator();
                ui.label(&self.ui_state.status);
            });
        });

        // 4) El dock con los paneles.
        let mut viewer = crate::docking::NaygoTabViewer { state: &mut self.ui_state };
        DockArea::new(&mut self.dock_state)
            .style(Style::from_egui(ctx.style().as_ref()))
            .show(ctx, &mut viewer);

        // 5) Si hay un listado en curso, repintar para que el streaming se vea fluido.
        if self.ui_state.listing_rx.is_some() {
            ctx.request_repaint();
        }
    }
}

/// Carpeta inicial razonable: el home del usuario, o `C:\` como fallback.
fn default_start_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from("C:\\"))
}
```

- [ ] **Step 2: Implementar el TabViewer de egui_dock**

Replace `crates/ui/src/docking.rs`:

```rust
// Naygo — integración de egui_dock para paneles dinámicos dockables.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Define cómo egui_dock pinta cada tab. Cada `PaneTab` delega en su panel.
//! El docking permite reacomodar/arrastrar los paneles (árbol, archivos,
//! inspector) en caliente, como pide el spec.

use crate::app::{PaneTab, UiState};
use crate::panes;

/// Implementa `TabViewer`: egui_dock le pide, por cada tab, su título y su UI.
pub struct NaygoTabViewer<'a> {
    pub state: &'a mut UiState,
}

impl egui_dock::TabViewer for NaygoTabViewer<'_> {
    type Tab = PaneTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            PaneTab::Tree => "Carpetas".into(),
            PaneTab::Files => "Archivos".into(),
            PaneTab::Inspector => "Propiedades".into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            PaneTab::Tree => panes::tree_panel::show(ui, self.state),
            PaneTab::Files => panes::file_panel::show(ui, self.state),
            PaneTab::Inspector => panes::inspector_panel::show(ui, self.state),
        }
    }
}
```

- [ ] **Step 3: Implementar el panel de archivos (vista Detalle)**

Replace `crates/ui/src/panes/file_panel.rs`:

```rust
// Naygo — panel de archivos: vista Detalle (columnas).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Pinta las entradas del panel activo en columnas (Nombre, Tamaño, Modificado).
//! Clic selecciona; doble clic activa (entra a carpeta). El foco de teclado se
//! refleja resaltando la fila. No hace I/O: solo dibuja `state.pane.entries`.

use crate::app::UiState;
use naygo_core::fs_model::{Entry, EntryKind};

pub fn show(ui: &mut egui::Ui, state: &mut UiState) {
    let focused = state.pane.focused;
    let mut clicked: Option<usize> = None;
    let mut activated: Option<usize> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("file_grid")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Nombre");
                ui.strong("Tamaño");
                ui.strong("Modificado");
                ui.end_row();

                for (i, entry) in state.pane.entries.iter().enumerate() {
                    let selected = focused == Some(i);
                    let label = format!("{} {}", kind_glyph(entry.kind), entry.name);
                    let resp = ui.selectable_label(selected, label);
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                    if resp.double_clicked() {
                        activated = Some(i);
                    }
                    ui.label(format_size(entry));
                    ui.label(format_modified(entry));
                    ui.end_row();
                }
            });
    });

    if let Some(i) = clicked {
        state.pane.focused = Some(i);
    }
    if let Some(i) = activated {
        state.pane.focused = Some(i);
        state.apply_action(crate::input::Action::Activate);
    }
}

/// Glifo de texto provisional según el tipo. Los íconos reales del Shell llegan
/// con `platform::shell` en una fase posterior.
fn kind_glyph(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Directory => "[D]",
        EntryKind::File => "   ",
        EntryKind::Other => "[?]",
    }
}

fn format_size(entry: &Entry) -> String {
    match entry.size {
        Some(bytes) => human_size(bytes),
        None => String::new(),
    }
}

/// Formatea bytes en KB/MB/GB con un decimal.
fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn format_modified(entry: &Entry) -> String {
    use std::time::UNIX_EPOCH;
    match entry.modified.and_then(|t| t.duration_since(UNIX_EPOCH).ok()) {
        Some(d) => {
            // Provisional: segundos epoch. Formato de fecha legible llega con i18n.
            format!("{}", d.as_secs())
        }
        None => String::new(),
    }
}
```

- [ ] **Step 4: Implementar el árbol (esqueleto) y el inspector (metadatos básicos)**

Replace `crates/ui/src/panes/tree_panel.rs`:

```rust
// Naygo — panel de árbol de carpetas (esqueleto de Fase 1).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! En la Fase 1 el árbol es un esqueleto: muestra la carpeta actual y permite
//! subir al padre con un botón. El árbol expandible real (con lazy-load por
//! streaming) se construye en una fase posterior; este panel reserva el espacio
//! arquitectónico y el lugar en el dock.

use crate::app::UiState;
use crate::input::Action;

pub fn show(ui: &mut egui::Ui, state: &mut UiState) {
    ui.label("Ubicación actual:");
    ui.monospace(state.pane.current_dir.display().to_string());
    ui.separator();
    if ui.button("⬆ Subir un nivel").clicked() {
        state.apply_action(Action::GoUp);
    }
}
```

Replace `crates/ui/src/panes/inspector_panel.rs`:

```rust
// Naygo — panel inspector: metadatos básicos del elemento enfocado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Muestra los metadatos que ya tenemos en el `Entry` (nombre, tipo, tamaño,
//! fecha). Las propiedades extendidas del Shell (atributos, propietario, etc.)
//! llegan con `platform::shell` en una fase posterior.

use crate::app::UiState;
use naygo_core::fs_model::EntryKind;

pub fn show(ui: &mut egui::Ui, state: &mut UiState) {
    let Some(entry) = state.pane.focused_entry() else {
        ui.label("Nada seleccionado.");
        return;
    };

    egui::Grid::new("inspector_grid").num_columns(2).show(ui, |ui| {
        ui.strong("Nombre");
        ui.label(&entry.name);
        ui.end_row();

        ui.strong("Tipo");
        ui.label(match entry.kind {
            EntryKind::Directory => "Carpeta",
            EntryKind::File => "Archivo",
            EntryKind::Other => "Otro",
        });
        ui.end_row();

        ui.strong("Ruta");
        ui.label(entry.path.display().to_string());
        ui.end_row();

        if let Some(size) = entry.size {
            ui.strong("Tamaño");
            ui.label(format!("{size} bytes"));
            ui.end_row();
        }
    });
}
```

- [ ] **Step 5: Compilar y verificar el binario**

Run: `cargo build -p naygo-ui`
Expected: compila sin errores ni warnings de imports sin usar.

- [ ] **Step 6: Verificación manual de la navegación**

Run: `cargo run -p naygo-ui`
Verificar a mano (es UI; no hay test automatizado de egui en esta fase):
1. Abre con la carpeta del usuario listada en el panel central.
2. ↑/↓ mueven el resaltado de fila.
3. Enter (o doble clic) sobre una carpeta entra; la barra de estado y el panel se actualizan.
4. Backspace (o el botón "Subir un nivel" del árbol) sube al padre.
5. El inspector de la derecha muestra los datos de la fila enfocada.
6. Los tres paneles se pueden arrastrar/reacomodar (egui_dock).
7. En una carpeta muy grande, las filas aparecen progresivamente (streaming) y el estado dice "Listando…" y luego "N elementos".
8. Cerrar; `logs/naygo.log` tiene entradas.

- [ ] **Step 7: Correr toda la suite + clippy**

Run: `cargo test`
Expected: PASS (todos los tests de core + ui input).

Run: `cargo clippy --workspace -- -D warnings`
Expected: sin warnings (si hay, corregirlos antes del commit).

- [ ] **Step 8: Commit**

```bash
git add crates/ui/src/
git commit -m "feat(ui): docking + vista Detalle + navegación por teclado sobre listing streaming"
```

---

## Task 9: Type-ahead (saltar al archivo que empieza con lo tipeado)

**Files:**
- Create: `crates/ui/src/typeahead.rs`
- Modify: `crates/ui/src/main.rs` (declarar `mod typeahead;`)
- Modify: `crates/ui/src/app.rs` (usar el buscador en `handle_keyboard`)
- Test: módulo `#[cfg(test)]` en `typeahead.rs`

- [ ] **Step 1: Escribir la función pura de búsqueda con sus tests**

Create `crates/ui/src/typeahead.rs`:

```rust
// Naygo — type-ahead: saltar a la entrada cuyo nombre empieza con lo tipeado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica pura del type-ahead, separada para testearla sin egui. Dada la lista de
//! nombres, el prefijo tipeado y desde qué índice buscar, devuelve el índice de la
//! primera coincidencia (case-insensitive), envolviendo al inicio si hace falta.

/// Busca la primera entrada cuyo nombre empieza con `prefix` (case-insensitive),
/// empezando en `start` y dando la vuelta. Devuelve `None` si nada coincide.
pub fn find_match(names: &[String], prefix: &str, start: usize) -> Option<usize> {
    if names.is_empty() || prefix.is_empty() {
        return None;
    }
    let prefix_lower = prefix.to_lowercase();
    let n = names.len();
    for offset in 0..n {
        let i = (start + offset) % n;
        if names[i].to_lowercase().starts_with(&prefix_lower) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        ["Apple", "banana", "Blueberry", "cherry"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn salta_a_la_primera_coincidencia() {
        assert_eq!(find_match(&names(), "b", 0), Some(1)); // banana
    }

    #[test]
    fn es_case_insensitive() {
        assert_eq!(find_match(&names(), "BL", 0), Some(2)); // Blueberry
    }

    #[test]
    fn da_la_vuelta_buscando_desde_el_medio() {
        // Desde índice 3 (cherry), buscar "a" debe envolver hasta Apple (0).
        assert_eq!(find_match(&names(), "a", 3), Some(0));
    }

    #[test]
    fn sin_coincidencia_devuelve_none() {
        assert_eq!(find_match(&names(), "z", 0), None);
    }
}
```

- [ ] **Step 2: Correr los tests y verificar que pasan**

Run: `cargo test -p naygo-ui typeahead`
Expected: los tests no compilan/corren todavía porque `mod typeahead;` no está declarado. Modify `crates/ui/src/main.rs` para añadir `mod typeahead;` junto a los otros `mod`.

Run de nuevo: `cargo test -p naygo-ui typeahead`
Expected: PASS (4 tests).

- [ ] **Step 3: Cablear el type-ahead en la app**

Modify `crates/ui/src/app.rs`:

Añadir campos al struct `UiState` (junto a `status`):

```rust
    /// Buffer del type-ahead acumulado entre teclas seguidas.
    pub typeahead_buf: String,
```

Inicializar en `NaygoApp::new` donde se crea `UiState` (añadir el campo):

```rust
            typeahead_buf: String::new(),
```

Añadir al final del `impl UiState` un método:

```rust
    /// Procesa caracteres tipeados para el type-ahead: acumula el prefijo y
    /// mueve el foco a la primera entrada que empieza así.
    pub fn typeahead(&mut self, typed: &str) {
        if typed.is_empty() {
            return;
        }
        self.typeahead_buf.push_str(typed);
        let names: Vec<String> =
            self.pane.entries.iter().map(|e| e.name.clone()).collect();
        let start = self.pane.focused.unwrap_or(0);
        if let Some(i) = crate::typeahead::find_match(&names, &self.typeahead_buf, start) {
            self.pane.focused = Some(i);
        }
    }
```

En `handle_keyboard`, después del loop de teclas especiales, recoger el texto tipeado y limpiarlo cuando corresponda. Añadir dentro del `ctx.input(|i| { ... })`, antes de cerrar el closure, la recolección de `i.events` de texto; y tras el closure, despachar:

Reemplazar el cuerpo de `handle_keyboard` por:

```rust
    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let keys = [
            (egui::Key::ArrowUp, NaygoKey::ArrowUp),
            (egui::Key::ArrowDown, NaygoKey::ArrowDown),
            (egui::Key::ArrowLeft, NaygoKey::ArrowLeft),
            (egui::Key::Enter, NaygoKey::Enter),
            (egui::Key::Backspace, NaygoKey::Backspace),
            (egui::Key::Tab, NaygoKey::Tab),
            (egui::Key::Escape, NaygoKey::Escape),
        ];
        let mut actions = Vec::new();
        let mut typed = String::new();
        ctx.input(|i| {
            for (egui_key, naygo_key) in keys {
                if i.key_pressed(egui_key) {
                    if let Some(action) = map_key(naygo_key) {
                        actions.push(action);
                    }
                }
            }
            for event in &i.events {
                if let egui::Event::Text(t) = event {
                    typed.push_str(t);
                }
            }
        });

        // Las acciones de navegación reinician el buffer de type-ahead.
        if !actions.is_empty() {
            self.ui_state.typeahead_buf.clear();
        }
        for action in actions {
            self.ui_state.apply_action(action);
        }
        if !typed.is_empty() {
            self.ui_state.typeahead(&typed);
        }
    }
```

- [ ] **Step 4: Compilar, testear, clippy**

Run: `cargo build -p naygo-ui`
Expected: compila.

Run: `cargo test`
Expected: PASS (todo).

Run: `cargo clippy --workspace -- -D warnings`
Expected: sin warnings.

- [ ] **Step 5: Verificación manual del type-ahead**

Run: `cargo run -p naygo-ui`
Con el panel de archivos enfocado, tipear las primeras letras de un nombre: el foco debe saltar a esa entrada. Tipear seguido refina; pausar y mover con flechas reinicia el buffer.

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/typeahead.rs crates/ui/src/main.rs crates/ui/src/app.rs
git commit -m "feat(ui): type-ahead para saltar al archivo por prefijo"
```

---

## Task 10: Metadatos de autoría en el .exe + verificación del release distribuible

Marca la autoría de Nicolás Groth / ISGroth en los metadatos del ejecutable (lo
pide el CLAUDE.md) y verifica que el binario de release corre en una máquina/VM
limpia sin Rust ni VC++ Redistributable.

**Files:**
- Create: `crates/ui/build.rs` (script de build que embebe los metadatos del .exe)
- Create: `crates/ui/app.rc` (recurso de versión de Windows)
- Modify: `crates/ui/Cargo.toml` (añadir `embed-resource` como build-dependency)

- [ ] **Step 1: Añadir `embed-resource` como build-dependency**

Modify `crates/ui/Cargo.toml` — añadir al final:

```toml
[build-dependencies]
embed-resource = "3"
```

- [ ] **Step 2: Escribir el recurso de versión de Windows**

Create `crates/ui/app.rc` (define la metadata que aparece en Propiedades → Detalles del .exe):

```rc
#include <winver.h>

VS_VERSION_INFO VERSIONINFO
 FILEVERSION 0,1,0,0
 PRODUCTVERSION 0,1,0,0
 FILEFLAGSMASK 0x3fL
 FILEFLAGS 0x0L
 FILEOS 0x40004L
 FILETYPE 0x1L
 FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "0c0a04b0"
        BEGIN
            VALUE "CompanyName", "ISGroth"
            VALUE "FileDescription", "Naygo — explorador de archivos rápido"
            VALUE "FileVersion", "0.1.0.0"
            VALUE "InternalName", "naygo"
            VALUE "LegalCopyright", "Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License."
            VALUE "OriginalFilename", "naygo.exe"
            VALUE "ProductName", "Naygo"
            VALUE "ProductVersion", "0.1.0.0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0c0a, 1200
    END
END
```

- [ ] **Step 3: Escribir el build script que compila el recurso**

Create `crates/ui/build.rs`:

```rust
// Naygo — build script: embebe los metadatos de autoría en el .exe (solo Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

fn main() {
    // Solo en Windows se compila el recurso de versión; en otros SO es no-op.
    #[cfg(target_os = "windows")]
    {
        embed_resource::compile("app.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
```

- [ ] **Step 4: Compilar en release y verificar que embebe la metadata**

Run: `cargo build --release -p naygo-ui`
Expected: compila. El binario queda en `target/release/naygo.exe`.

Run (PowerShell): `(Get-Item target/release/naygo.exe).VersionInfo | Format-List CompanyName,ProductName,LegalCopyright,FileVersion`
Expected: muestra `CompanyName: ISGroth`, `ProductName: Naygo`, el copyright de Nicolás Groth / ISGroth y `FileVersion: 0.1.0.0`.

- [ ] **Step 5: Verificar CRT estático (no depende del VC++ Redistributable)**

Run (PowerShell, requiere las Build Tools en el PATH; si `dumpbin` no está, usar el "Developer PowerShell for VS"):
`dumpbin /dependents target/release/naygo.exe`
Expected: entre las DLLs **NO** debe aparecer `VCRUNTIME140.dll` ni `MSVCP140.dll` (gracias a `+crt-static`). Sí pueden aparecer DLLs del sistema (`KERNEL32.dll`, `USER32.dll`, `GDI32.dll`, `OPENGL32.dll`, etc.), que existen en todo Windows 10/11.

Si `dumpbin` no está disponible, alternativa: copiar `naygo.exe` solo (sin nada más) a una VM limpia de Windows 10/11 sin Rust ni VC++ Redist y confirmar que abre (Step 7 lo cubre).

- [ ] **Step 6: Anotar el tamaño del binario (referencia de "liviano")**

Run (PowerShell): `"{0:N2} MB" -f ((Get-Item target/release/naygo.exe).Length / 1MB)`
Expected: registrar el tamaño en el commit (con `opt-level="z"` + `lto` + `strip`, un egui mínimo suele quedar en el orden de ~6–12 MB). No es un fallo si difiere; es una línea base para vigilar el consumo en fases futuras.

- [ ] **Step 7: Verificación en máquina/VM limpia (manual, clave para el objetivo del proyecto)**

Copiar **solo** `target/release/naygo.exe` a una VM o equipo Windows 10/11 sin Rust
ni VC++ Redistributable instalados. Ejecutarlo. Verificar:
1. Abre la ventana sin errores de "falta VCRUNTIME140.dll" ni similares.
2. Lista la carpeta inicial y se navega con teclado.
3. Crea su `logs/naygo.log` junto al `.exe` (config portable, según el spec).

Si falla por una DLL faltante, revisar que `.cargo/config.toml` tenga `+crt-static`
y recompilar en release.

- [ ] **Step 8: Commit**

```bash
git add crates/ui/build.rs crates/ui/app.rc crates/ui/Cargo.toml
git commit -m "build(ui): metadatos de autoría en el .exe + release distribuible con CRT estático"
```

---

## Task 11: Cierre de fase — README de estado, push y verificación final

**Files:**
- Modify: `README.md` (actualizar el estado de "en diseño" a "Fase 1 en desarrollo")
- Modify: `.gitignore` (decidir Cargo.lock)

- [ ] **Step 1: Versionar Cargo.lock (es un binario de aplicación)**

El `.gitignore` actual ignora `Cargo.lock` con una nota de "revisar en el plan". Para una **aplicación** (no librería) se versiona, para builds reproducibles.

Modify `.gitignore` — borrar la línea:

```
Cargo.lock        # binario de app: lo versionaremos cuando haya Cargo.toml; revisar en el plan
```

- [ ] **Step 2: Actualizar el estado en el README**

Modify `README.md:7-8` — reemplazar el bloque de estado:

```markdown
> **Estado:** Fase 1 (esqueleto navegable) en desarrollo. Diseño del núcleo en
> [`docs/superpowers/specs/2026-06-05-explorador-nucleo-design.md`](docs/superpowers/specs/2026-06-05-explorador-nucleo-design.md);
> plan de la Fase 1 en
> [`docs/superpowers/plans/2026-06-05-naygo-fase1-esqueleto-navegable.md`](docs/superpowers/plans/2026-06-05-naygo-fase1-esqueleto-navegable.md).
```

- [ ] **Step 3: Verificación final completa**

Run: `cargo build --workspace`
Expected: compila los 3 crates.

Run: `cargo test --workspace`
Expected: PASS todos.

Run: `cargo clippy --workspace -- -D warnings`
Expected: limpio.

Run: `cargo run -p naygo-ui`
Expected: la app abre, navega, lista por streaming, type-ahead funciona.

- [ ] **Step 4: Commit y push**

```bash
git add Cargo.lock README.md .gitignore
git commit -m "chore: versionar Cargo.lock, actualizar estado del README a Fase 1"
git push
```

Expected: push exitoso a `origin/main`.

---

## Self-review (cobertura del spec por esta fase)

| Requisito del spec (Build 1) | Cubierto en esta fase |
|---|---|
| Workspace / estructura de proyecto | Tarea 1 (decidido: workspace 3 crates) |
| `fs_model` (Entry, PaneState, SortSpec, ViewMode) | Tarea 3 |
| `listing` streaming incremental + cancelable | Tarea 5 |
| Sort puro | Tarea 4 |
| CancellationToken (mecanismo uniforme) | Tarea 2; aplicado a listing en Tarea 5/8 |
| Cancelación universal (Esc cancela listado) | Tarea 6 (mapeo) + Tarea 8 (apply_action) |
| Paneles dockables (egui_dock) | Tarea 8 |
| Vista Detalle | Tarea 8 (file_panel) |
| Inspector de metadatos (básicos) | Tarea 8 (inspector_panel) |
| Navegación por teclado (↑↓ Enter Backspace Tab Esc) | Tareas 6 + 8 |
| Type-ahead | Tarea 9 |
| Hilo de UI nunca hace I/O | Tarea 8 (worker + mpsc + try_recv) |
| Logging a archivo + panic handler | Tarea 7 |
| `platform` aislado (crate creado, frontera fijada) | Tarea 1 |
| Autoría en metadatos del .exe (CLAUDE.md) | Tarea 10 (app.rc + build.rs) |
| Distribuible a VM/equipos sin runtime (CRT estático) | Tareas 1 + 10 (verificado en VM limpia) |
| Bajo consumo / binario liviano | Tarea 1 (perfil release) + Tarea 10 Step 6 (medición) |

**Diferido explícitamente a fases siguientes (NO en esta fase, con su propio plan):**
`ops` (copiar/mover/eliminar/renombrar/crear), `sizing` (tamaño de carpeta + F3),
i18n real (catálogo + ES/EN), `theme`/color sets, `config` (persistencia de layout/
columnas/teclas), vistas Lista e Íconos, íconos del Shell + toolbar rica,
breadcrumbs clicables, panel de discos + espacio libre, drag interno y drag&drop
COM/OLE, atajos configurables, segundo panel de archivos (multi-pane real),
formato de fecha legible. El stub de fecha (epoch secs) y los glifos `[D]` se
reemplazan cuando lleguen i18n e íconos.

**Empaquetado/instalador completo — fase aparte (plan propio):** generación del
**ZIP portable** (naygo.exe + assets) y del **instalador** (.msi vía `cargo-wix`
y/o setup .exe vía NSIS), ícono de la app, accesos directos, desinstalador, y
"About" con autoría. Esta Fase 1 deja el binario de release ya distribuible (corre
en máquina limpia); el empaquetado formal se construye encima.

**Notas de riesgo conocidas:**
- API de `egui_dock` 0.19 (`split_left`/`split_right`, `TabViewer`): si la firma
  difiere en la versión exacta resuelta, ajustar en Tarea 8 Step 1-2 contra la doc
  de la versión bloqueada en `Cargo.lock`.
- `windows_subsystem = "windows"` oculta la consola en release: revisar que el
  logging a archivo siga funcionando ahí (Tarea 7).
- Perfil release usa `panic = unwind` (default) a propósito: permite `catch_unwind`
  en workers para que un panic listando/operando no mate toda la app (el spec exige
  que "nunca caiga"). El panic handler (Tarea 7) loguea igual. `opt-level = "s"`
  (no "z") por la misma razón: velocidad es prioridad, tamaño es secundario.
- `embed-resource` 3.x: la firma `compile(path, NONE)` puede variar entre menores;
  ajustar en Tarea 10 Step 3 contra la doc de la versión resuelta. El `app.rc` con
  `#include <winver.h>` requiere el Windows SDK (viene con las Build Tools).
- CRT estático (`+crt-static`) elimina la dependencia del VC++ Redist, pero el `.exe`
  igual necesita las DLLs del sistema y un driver de OpenGL/Direct3D presente (egui
  usa GPU). En VMs muy mínimas sin aceleración, eframe cae a software; verificar en
  Tarea 10 Step 7 que arranca en la VM objetivo.
