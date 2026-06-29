# Comprimir / descomprimir .zip — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Crear y extraer archivos `.zip` desde el menú contextual, con worker async, panel de progreso, cancelación, conflictos, protección zip-slip y deshacer a papelera.

**Architecture:** Lógica pura y testeable en `core::archive_ops` (compress_zip/extract_zip con CancellationToken y callbacks de progreso/conflicto). Worker async en `ui-slint` que reusa el panel/canal de ops vía dos `OpKind` nuevos (`Compress`/`Extract`) despachados aparte en `start_op` (NO por el `plan/exec_step` de copiar). Menú contextual + modal de nombre. Deshacer a papelera con provenance.

**Tech Stack:** Rust, crate `zip` (v2, feature `deflate`, ya presente en core), `CancellationToken` propio, sistema de ops existente (`OpProgress`/`OpMsg`/`UndoEntry`), Slint para la UI.

**Spec:** `docs/superpowers/specs/2026-06-29-comprimir-descomprimir-design.md`

**Convenciones del proyecto (recordatorio para el implementador):**
- Header en cada archivo nuevo: `// Naygo — <desc>` + `// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.` + `// SPDX-License-Identifier: MIT`.
- Nombres en inglés en el código; comentarios/commits en español NEUTRAL (nunca voseo).
- Cada commit: build + tests verdes. Termina los mensajes de commit con `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- NO tocar `CLAUDE.md` ni `crates/core/src/favorites.rs` (cambios ajenos preexistentes). `git add` por rutas explícitas.

## Estructura de archivos

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `crates/core/src/archive_ops.rs` | compress_zip, extract_zip, default_zip_name, tipos (puro) | Crear |
| `crates/core/src/lib.rs` | declarar `pub mod archive_ops;` | Modificar |
| `crates/core/src/ops/mod.rs` | variantes `OpKind::Compress`/`Extract` | Modificar |
| `crates/ui-slint/src/ops_ctrl.rs` | branch en `start_op` + `spawn_zip_op` (worker async) | Modificar |
| `crates/ui-slint/src/workspace_ctrl/context.rs` | handlers `op_compress`/`op_extract_here`/`op_extract_to` | Modificar |
| `crates/ui-slint/src/workspace_ctrl/mod.rs` | estado del modal de nombre (reusa NewFolderState-like) | Modificar |
| `crates/ui-slint/ui/*.slint` | ítems del menú contextual + modal de nombre del zip | Modificar |
| `crates/ui-slint/src/i18n_keys.rs` + `crates/core/src/i18n/*.json` | claves nuevas en 10 idiomas | Modificar |

---

## FASE 1 — core::archive_ops (puro, TDD)

### Task 1: Tipos + default_zip_name

**Files:**
- Create: `crates/core/src/archive_ops.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear el módulo con tipos y `default_zip_name`, con su test**

Crear `crates/core/src/archive_ops.rs`:
```rust
// Naygo — comprimir/extraer .zip: lógica pura (sin UI ni Windows), cancelable y testeable.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Crear y extraer archivos `.zip`. Puro: recibe rutas + un `CancellationToken` + callbacks de
//! progreso/conflicto; no conoce egui/Slint/Win32. El crate `zip` ya es dependencia (lo usa el
//! preview). Protección zip-slip al extraer (mismo criterio que `icon_pack::import`).

use crate::cancel::CancellationToken;
use std::path::{Path, PathBuf};

/// Resultado de UNA entrada procesada (para el resumen y el deshacer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveOpItem {
    /// Ruta REAL escrita: el `.zip` (comprimir) o el archivo/carpeta extraído (extraer).
    pub path: PathBuf,
    pub outcome: ArchiveOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchiveOutcome {
    Done,
    Skipped,
    Failed(String),
}

/// Decisión ante un conflicto al extraer (un archivo destino ya existe).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtractConflict {
    Overwrite,
    Skip,
    KeepBoth,
    Cancel,
}

/// Error tipado de las operaciones de archivo comprimido. Ningún panic.
#[derive(Debug)]
pub enum ArchiveError {
    Io(std::io::Error),
    Zip(String),
    Cancelled,
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::Io(e) => write!(f, "io: {e}"),
            ArchiveError::Zip(s) => write!(f, "zip: {s}"),
            ArchiveError::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl From<std::io::Error> for ArchiveError {
    fn from(e: std::io::Error) -> Self {
        ArchiveError::Io(e)
    }
}

/// Nombre por defecto del `.zip` a crear desde `sources`:
/// - 1 ítem (archivo o carpeta) → su nombre base + ".zip".
/// - varios ítems → "archivos.zip".
/// El nombre base de un archivo CONSERVA su parte sin extensión: "informe.txt" → "informe.zip".
/// Una carpeta usa su nombre tal cual: "proyecto" → "proyecto.zip".
pub fn default_zip_name(sources: &[PathBuf]) -> String {
    if sources.len() == 1 {
        let p = &sources[0];
        // file_stem para archivos con extensión; para carpetas (sin "extensión") file_stem == nombre.
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            return format!("{stem}.zip");
        }
    }
    "archivos.zip".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_zip_name_un_archivo_usa_su_stem() {
        assert_eq!(default_zip_name(&[PathBuf::from("C:/x/informe.txt")]), "informe.zip");
    }

    #[test]
    fn default_zip_name_una_carpeta_usa_su_nombre() {
        assert_eq!(default_zip_name(&[PathBuf::from("C:/x/proyecto")]), "proyecto.zip");
    }

    #[test]
    fn default_zip_name_varios_es_generico() {
        let v = vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")];
        assert_eq!(default_zip_name(&v), "archivos.zip");
    }
}
```

Agregar en `crates/core/src/lib.rs` (junto a los otros `pub mod`, p.ej. tras `pub mod archive_tree;`):
```rust
pub mod archive_ops;
```

- [ ] **Step 2: Correr los tests**

Run: `cargo test -p naygo-core archive_ops`
Expected: 3 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/archive_ops.rs crates/core/src/lib.rs
git commit -m "feat(core): archive_ops — tipos + default_zip_name (comprimir/extraer .zip)"
```

---

### Task 2: compress_zip (con cancelación + borrado del parcial)

**Files:**
- Modify: `crates/core/src/archive_ops.rs`

- [ ] **Step 1: Escribir los tests de compresión PRIMERO**

Añadir al `mod tests`:
```rust
    use std::io::Write;

    fn noop_progress() -> impl FnMut(u64, u64) {
        |_done, _total| {}
    }

    #[test]
    fn compress_un_archivo_crea_zip_leible() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("hola.txt");
        std::fs::write(&src, b"contenido").unwrap();
        let zip_path = dir.path().join("out.zip");
        let token = CancellationToken::new();
        let items = compress_zip(&[src.clone()], &zip_path, &mut noop_progress(), &token).unwrap();
        assert!(zip_path.exists());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].outcome, ArchiveOutcome::Done);
        // El zip se puede abrir y tiene la entrada.
        let f = std::fs::File::open(&zip_path).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        assert!(z.by_name("hola.txt").is_ok());
    }

    #[test]
    fn compress_carpeta_anidada_incluye_todo_recursivo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proyecto");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("README.md"), b"x").unwrap();
        std::fs::write(root.join("src/main.rs"), b"fn main(){}").unwrap();
        let zip_path = dir.path().join("p.zip");
        let token = CancellationToken::new();
        compress_zip(&[root.clone()], &zip_path, &mut noop_progress(), &token).unwrap();
        let f = std::fs::File::open(&zip_path).unwrap();
        let mut z = zip::ZipArchive::new(f).unwrap();
        // Las entradas conservan la ruta relativa BAJO el nombre de la carpeta raíz.
        let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
        assert!(names.iter().any(|n| n.ends_with("proyecto/README.md")));
        assert!(names.iter().any(|n| n.ends_with("proyecto/src/main.rs")));
    }

    #[test]
    fn compress_cancelado_borra_el_parcial() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.txt");
        std::fs::write(&src, vec![0u8; 1024]).unwrap();
        let zip_path = dir.path().join("c.zip");
        let token = CancellationToken::new();
        token.cancel(); // cancelado de entrada
        let r = compress_zip(&[src], &zip_path, &mut noop_progress(), &token);
        assert!(matches!(r, Err(ArchiveError::Cancelled)));
        assert!(!zip_path.exists(), "el .zip parcial se borra al cancelar");
    }
```

- [ ] **Step 2: Correr para verificar que FALLAN**

Run: `cargo test -p naygo-core archive_ops`
Expected: FAIL — `compress_zip` no existe.

- [ ] **Step 3: Implementar `compress_zip`**

Añadir a `archive_ops.rs` (fuera del `mod tests`):
```rust
use std::io::{Read, Write};
use zip::write::SimpleFileOptions;

/// Recorre `sources` (archivos y/o carpetas, recursivo) y los empaqueta en `dest_zip`.
/// Progreso por BYTES. Cancelar → aborta y borra el `.zip` parcial. Un source ilegible se
/// registra Failed y la op continúa.
pub fn compress_zip(
    sources: &[PathBuf],
    dest_zip: &Path,
    on_progress: &mut dyn FnMut(u64, u64),
    token: &CancellationToken,
) -> Result<Vec<ArchiveOpItem>, ArchiveError> {
    // Lista plana de (ruta_en_disco, ruta_interna_en_zip), expandiendo carpetas.
    let mut entries: Vec<(PathBuf, String)> = Vec::new();
    for src in sources {
        let base = src.parent().unwrap_or(Path::new(""));
        collect_entries(src, base, &mut entries);
    }
    let total: u64 = entries
        .iter()
        .map(|(p, _)| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();

    // Crear el archivo destino; si se cancela o falla, lo borramos al salir.
    let file = std::fs::File::create(dest_zip)?;
    let mut zipw = zip::ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut done: u64 = 0;
    let mut items: Vec<ArchiveOpItem> = Vec::new();
    for (disk, internal) in &entries {
        if token.is_cancelled() {
            drop(zipw);
            let _ = std::fs::remove_file(dest_zip);
            return Err(ArchiveError::Cancelled);
        }
        token.wait_if_paused();
        // Carpeta: añadir entrada de directorio (con '/' al final).
        if disk.is_dir() {
            let dir_name = format!("{}/", internal.trim_end_matches('/'));
            if zipw.add_directory(dir_name, opts).is_err() {
                items.push(ArchiveOpItem { path: disk.clone(), outcome: ArchiveOutcome::Failed("add_directory".into()) });
            }
            continue;
        }
        match std::fs::File::open(disk) {
            Ok(mut f) => {
                if zipw.start_file(internal.clone(), opts).is_err() {
                    items.push(ArchiveOpItem { path: disk.clone(), outcome: ArchiveOutcome::Failed("start_file".into()) });
                    continue;
                }
                // Copia por bloques, emitiendo progreso y atendiendo cancelación a media copia.
                let mut buf = [0u8; 64 * 1024];
                let mut failed = false;
                loop {
                    if token.is_cancelled() {
                        drop(zipw);
                        let _ = std::fs::remove_file(dest_zip);
                        return Err(ArchiveError::Cancelled);
                    }
                    let n = match f.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => { failed = true; break; }
                    };
                    if zipw.write_all(&buf[..n]).is_err() { failed = true; break; }
                    done += n as u64;
                    on_progress(done, total);
                }
                items.push(ArchiveOpItem {
                    path: disk.clone(),
                    outcome: if failed { ArchiveOutcome::Failed("read/write".into()) } else { ArchiveOutcome::Done },
                });
            }
            Err(e) => items.push(ArchiveOpItem { path: disk.clone(), outcome: ArchiveOutcome::Failed(e.to_string()) }),
        }
    }
    zipw.finish().map_err(|e| ArchiveError::Zip(e.to_string()))?;
    Ok(items)
}

/// Expande `path` a entradas (disk, internal). `base` es la carpeta padre del source de nivel
/// superior; la ruta interna en el zip es `path` relativa a `base` (con `/` como separador).
fn collect_entries(path: &Path, base: &Path, out: &mut Vec<(PathBuf, String)>) {
    let internal = path
        .strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    if path.is_dir() {
        out.push((path.to_path_buf(), internal));
        if let Ok(rd) = std::fs::read_dir(path) {
            // Orden determinista (para tests estables).
            let mut children: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            children.sort();
            for c in children {
                collect_entries(&c, base, out);
            }
        }
    } else {
        out.push((path.to_path_buf(), internal));
    }
}
```

> NOTA: confirma la API exacta del crate `zip` v2: `ZipWriter::new`, `start_file(name, SimpleFileOptions)`, `add_directory`, `write_all`, `finish`. `SimpleFileOptions` está en `zip::write`. Si algún nombre difiere en la v2 concreta del lockfile, ajústalo (lee `zip`'s docs o el uso existente en el preview que LEE zips). El test de round-trip valida que funcione.

- [ ] **Step 4: Correr los tests**

Run: `cargo test -p naygo-core archive_ops`
Expected: los 3 nuevos + los 3 de Task 1 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/archive_ops.rs
git commit -m "feat(core): compress_zip — empaquetar recursivo con progreso y cancelación (borra parcial)"
```

---

### Task 3: extract_zip (con zip-slip + conflictos + cancelación)

**Files:**
- Modify: `crates/core/src/archive_ops.rs`

- [ ] **Step 1: Escribir los tests de extracción PRIMERO**

Añadir al `mod tests`:
```rust
    fn always_overwrite() -> impl FnMut(&Path) -> ExtractConflict {
        |_p| ExtractConflict::Overwrite
    }

    /// Crea un .zip de prueba con las entradas dadas (nombre interno → contenido). dir-entries
    /// terminan en '/'. Devuelve la ruta del zip.
    fn make_zip(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let zip_path = dir.join(name);
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        for (n, data) in entries {
            if n.ends_with('/') {
                z.add_directory(*n, opts).unwrap();
            } else {
                z.start_file(*n, opts).unwrap();
                z.write_all(data).unwrap();
            }
        }
        z.finish().unwrap();
        zip_path
    }

    #[test]
    fn extract_round_trip_restaura_los_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "in.zip", &[("a/b.txt", b"hola"), ("a/", b"")]);
        let dest = dir.path().join("salida");
        let token = CancellationToken::new();
        let items = extract_zip(&zip_path, &dest, &mut always_overwrite(), &mut noop_progress(), &token).unwrap();
        assert!(items.iter().any(|i| i.outcome == ArchiveOutcome::Done));
        assert_eq!(std::fs::read(dest.join("a/b.txt")).unwrap(), b"hola");
    }

    #[test]
    fn extract_rechaza_zip_slip() {
        let dir = tempfile::tempdir().unwrap();
        // Entrada maliciosa que intenta escapar del destino.
        let zip_path = make_zip(dir.path(), "evil.zip", &[("../escape.txt", b"pwn"), ("ok.txt", b"bien")]);
        let dest = dir.path().join("destino");
        let token = CancellationToken::new();
        let items = extract_zip(&zip_path, &dest, &mut always_overwrite(), &mut noop_progress(), &token).unwrap();
        // La entrada legítima se extrajo; la maliciosa NO escribió fuera del destino.
        assert_eq!(std::fs::read(dest.join("ok.txt")).unwrap(), b"bien");
        assert!(!dir.path().join("escape.txt").exists(), "zip-slip bloqueado: nada fuera del destino");
        assert!(items.iter().any(|i| i.outcome == ArchiveOutcome::Skipped));
    }

    #[test]
    fn extract_conflicto_skip_no_pisa() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "z.zip", &[("dato.txt", b"nuevo")]);
        let dest = dir.path().join("d");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join("dato.txt"), b"viejo").unwrap();
        let token = CancellationToken::new();
        let mut skip = |_p: &Path| ExtractConflict::Skip;
        extract_zip(&zip_path, &dest, &mut skip, &mut noop_progress(), &token).unwrap();
        assert_eq!(std::fs::read(dest.join("dato.txt")).unwrap(), b"viejo", "Skip no pisa el existente");
    }

    #[test]
    fn extract_cancelado_deja_lo_ya_extraido() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = make_zip(dir.path(), "z.zip", &[("uno.txt", b"a")]);
        let dest = dir.path().join("d");
        let token = CancellationToken::new();
        token.cancel();
        let r = extract_zip(&zip_path, &dest, &mut always_overwrite(), &mut noop_progress(), &token);
        assert!(matches!(r, Err(ArchiveError::Cancelled)));
        // Cancelado de entrada: no se extrajo nada, pero la regla es que lo ya extraído PERMANECE
        // (no se borra el destino). Aquí simplemente no debe panicar ni borrar el dir.
    }
```

- [ ] **Step 2: Correr para verificar que FALLAN**

Run: `cargo test -p naygo-core archive_ops`
Expected: FAIL — `extract_zip` no existe.

- [ ] **Step 3: Implementar `extract_zip`**

Añadir a `archive_ops.rs`:
```rust
/// Extrae `zip` dentro de `dest_dir`. Zip-slip: entradas con `..`/absolutas o que escapen de
/// `dest_dir` se RECHAZAN (Skipped). Progreso por bytes (total = suma de tamaños descomprimidos).
/// Conflicto (destino existe) → `on_conflict`. Cancelar → aborta; lo extraído PERMANECE.
pub fn extract_zip(
    zip: &Path,
    dest_dir: &Path,
    on_conflict: &mut dyn FnMut(&Path) -> ExtractConflict,
    on_progress: &mut dyn FnMut(u64, u64),
    token: &CancellationToken,
) -> Result<Vec<ArchiveOpItem>, ArchiveError> {
    let file = std::fs::File::open(zip)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| ArchiveError::Zip(e.to_string()))?;
    let total: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.size()))
        .sum();
    std::fs::create_dir_all(dest_dir)?;
    // Canonicalizar el destino para comparar de forma robusta (zip-slip).
    let dest_canon = std::fs::canonicalize(dest_dir).unwrap_or_else(|_| dest_dir.to_path_buf());

    let mut done: u64 = 0;
    let mut items: Vec<ArchiveOpItem> = Vec::new();
    for i in 0..archive.len() {
        if token.is_cancelled() {
            return Err(ArchiveError::Cancelled);
        }
        token.wait_if_paused();
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(e) => { items.push(ArchiveOpItem { path: PathBuf::new(), outcome: ArchiveOutcome::Failed(e.to_string()) }); continue; }
        };
        // `enclosed_name` ya neutraliza `..` y rutas absolutas; si es None, es una entrada hostil.
        let Some(rel) = entry.enclosed_name() else {
            items.push(ArchiveOpItem { path: PathBuf::from(entry.name()), outcome: ArchiveOutcome::Skipped });
            continue;
        };
        let target = dest_dir.join(&rel);
        // Defensa adicional: el target resuelto debe seguir dentro del destino.
        let within = target.starts_with(dest_dir)
            || std::fs::canonicalize(target.parent().unwrap_or(dest_dir))
                .map(|p| p.starts_with(&dest_canon))
                .unwrap_or(false);
        if !within {
            items.push(ArchiveOpItem { path: target, outcome: ArchiveOutcome::Skipped });
            continue;
        }
        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&target);
            items.push(ArchiveOpItem { path: target, outcome: ArchiveOutcome::Done });
            continue;
        }
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Conflicto: el archivo destino ya existe.
        let mut final_target = target.clone();
        if final_target.exists() {
            match on_conflict(&final_target) {
                ExtractConflict::Skip => {
                    items.push(ArchiveOpItem { path: final_target, outcome: ArchiveOutcome::Skipped });
                    continue;
                }
                ExtractConflict::Cancel => return Err(ArchiveError::Cancelled),
                ExtractConflict::Overwrite => {}
                ExtractConflict::KeepBoth => {
                    final_target = unique_path(&final_target);
                }
            }
        }
        match std::fs::File::create(&final_target) {
            Ok(mut out) => {
                let mut buf = [0u8; 64 * 1024];
                let mut failed = false;
                loop {
                    if token.is_cancelled() { return Err(ArchiveError::Cancelled); }
                    let n = match entry.read(&mut buf) { Ok(0) => break, Ok(n) => n, Err(_) => { failed = true; break; } };
                    if out.write_all(&buf[..n]).is_err() { failed = true; break; }
                    done += n as u64;
                    on_progress(done, total);
                }
                items.push(ArchiveOpItem {
                    path: final_target,
                    outcome: if failed { ArchiveOutcome::Failed("write".into()) } else { ArchiveOutcome::Done },
                });
            }
            Err(e) => items.push(ArchiveOpItem { path: final_target, outcome: ArchiveOutcome::Failed(e.to_string()) }),
        }
    }
    Ok(items)
}

/// Devuelve una variante de `path` que no existe: "a.txt" → "a (2).txt" → "a (3).txt"…
fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or(Path::new(""));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("archivo");
    let ext = path.extension().and_then(|s| s.to_str());
    for n in 2..10_000 {
        let name = match ext {
            Some(e) => format!("{stem} ({n}).{e}"),
            None => format!("{stem} ({n})"),
        };
        let cand = parent.join(name);
        if !cand.exists() {
            return cand;
        }
    }
    path.to_path_buf()
}
```

> NOTA: `enclosed_name()` del crate `zip` YA neutraliza zip-slip (devuelve None para rutas peligrosas). La defensa con `starts_with`/`canonicalize` es redundante pero barata (defensa en profundidad, mismo espíritu que `icon_pack::import`). Confirma que `enclosed_name` existe en la versión del lockfile; si no, usa el patrón manual de `icon_pack.rs:126-132` (`rel.contains("..") || is_absolute()` + `target.starts_with(dest)`).

- [ ] **Step 4: Correr los tests**

Run: `cargo test -p naygo-core archive_ops`
Expected: todos PASS (Task 1 + 2 + 3).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/archive_ops.rs
git commit -m "feat(core): extract_zip — extraer con zip-slip, conflictos y cancelación"
```

---

## FASE 2 — cableado en el motor de ops

### Task 4: OpKind::Compress / Extract

**Files:**
- Modify: `crates/core/src/ops/mod.rs`

- [ ] **Step 1: Agregar las variantes al enum `OpKind`**

En `crates/core/src/ops/mod.rs`, dentro de `pub enum OpKind` (tras `CreateFile`):
```rust
    /// Comprimir `sources` en un `.zip` llamado `dest_name`, dentro de `dest_dir` del OpRequest.
    Compress {
        dest_name: String,
    },
    /// Extraer el `.zip` (único source) dentro de `dest_dir` del OpRequest.
    Extract,
```

> Estas variantes NO pasan por `plan()`/`exec_step()` (el worker de zip las maneja aparte en
> ui-slint). Pero el enum es compartido, así que cualquier `match` EXHAUSTIVO sobre `OpKind` en
> core debe cubrirlas. Busca `match ` sobre kind: `grep -rn "OpKind::Copy" crates/core/src/ops/`.
> En `exec_step` (engine.rs) y `plan` (plan.rs), agrega un brazo que retorne un error/no-op claro
> para Compress/Extract (no deberían llegar ahí; si llegan, es un bug — devuelve un error tipado o
> un plan vacío con un `debug_assert!`). NO implementes la lógica de zip en el engine.

- [ ] **Step 2: Cubrir los match exhaustivos**

En `crates/core/src/ops/engine.rs`, en el `match kind` de `exec_step` (~línea 244), agrega:
```rust
        OpKind::Compress { .. } | OpKind::Extract => {
            // El worker de zip (ui-slint) maneja estas ops; nunca deberían llegar al engine de pasos.
            debug_assert!(false, "Compress/Extract no pasan por exec_step");
            (step.to.clone(), OpOutcome::Failed("compress/extract fuera del engine".into()), 0, false)
        }
```
En `crates/core/src/ops/plan.rs`, si `plan()` hace un `match` exhaustivo sobre kind, agrega un brazo que devuelva un plan vacío (`OpPlan { steps: vec![], total_bytes: 0, total_files: 0, .. }`) o el error correspondiente. LEE el match real antes de editar.

- [ ] **Step 3: Compilar core**

Run: `cargo build -p naygo-core` → compila (los match exhaustivos cubiertos).
Run: `cargo test -p naygo-core` → verde (no rompimos nada).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/ops/mod.rs crates/core/src/ops/engine.rs crates/core/src/ops/plan.rs
git commit -m "feat(core): OpKind::Compress/Extract (manejadas por el worker de zip, no el engine)"
```

---

### Task 5: spawn_zip_op (worker async) + branch en start_op

**Files:**
- Modify: `crates/ui-slint/src/ops_ctrl.rs`

- [ ] **Step 1: Branch en `start_op` para Compress/Extract**

En `start_op` (ops_ctrl.rs:274), tras el branch de Delete-a-papelera y ANTES del de Copy/Move, agrega:
```rust
        // Comprimir/extraer: worker propio de zip (no pasa por plan/exec_step). Reusa el panel,
        // el canal de progreso, el id de op y la cancelación, igual que el resto.
        if matches!(req.kind, OpKind::Compress { .. } | OpKind::Extract) {
            let id = self.alloc_op_id();
            self.spawn_zip_op(id, req, label, record_undo);
            return;
        }
```

- [ ] **Step 2: Implementar `spawn_zip_op`**

Estudia `spawn_op`/`start_planning` (ops_ctrl.rs) para copiar EXACTAMENTE su patrón de: crear `CancellationToken`, abrir el canal (`mpsc`), lanzar `std::thread::spawn`, registrar la op en la estructura que `pump_ops` drena (para que el panel la muestre), y al terminar emitir `OpMsg::Done`/`Cancelled` y registrar el undo. Implementa:
```rust
    /// Lanza el worker de comprimir/extraer. Reusa el canal/panel/cancelación de las ops normales.
    fn spawn_zip_op(&mut self, id: u64, req: OpRequest, label: String, record_undo: bool) {
        use naygo_core::archive_ops::{compress_zip, extract_zip, ExtractConflict};
        let token = CancellationToken::new();
        let (tx, rx) = std::sync::mpsc::channel::<OpMsg>();
        // Registrar la op en curso EXACTAMENTE como spawn_op (para el panel + pump_ops + cancelación
        // por id). Copia la estructura que usa spawn_op: id, token (clon), rx, label, record_undo.
        // … (replica el registro de spawn_op) …
        let token_worker = token.clone();
        let dest_dir = req.dest_dir.clone().unwrap_or_default();
        std::thread::spawn(move || {
            let mut on_progress = |done: u64, total: u64| {
                let _ = tx.send(OpMsg::Progress(naygo_core::ops::OpProgress {
                    bytes_done: done, bytes_total: total, files_done: 0, files_total: 0, current: std::path::PathBuf::new(),
                }));
            };
            let result = match &req.kind {
                OpKind::Compress { dest_name } => {
                    let dest_zip = dest_dir.join(dest_name);
                    compress_zip(&req.sources, &dest_zip, &mut on_progress, &token_worker)
                }
                OpKind::Extract => {
                    let zip = req.sources.first().cloned().unwrap_or_default();
                    // Conflicto: por ahora, política simple por defecto (Overwrite). El diálogo
                    // lado a lado se conecta vía OpMsg::Conflict + canal de respuesta en una
                    // sub-tarea (ver Task 7). Para el primer corte: Overwrite.
                    let mut on_conflict = |_p: &std::path::Path| ExtractConflict::Overwrite;
                    extract_zip(&zip, &dest_dir, &mut on_conflict, &mut on_progress, &token_worker)
                }
                _ => unreachable!(),
            };
            match result {
                Ok(items) => { let _ = tx.send(OpMsg::Done(zip_summary(items))); }
                Err(naygo_core::archive_ops::ArchiveError::Cancelled) => { let _ = tx.send(OpMsg::Cancelled(Default::default())); }
                Err(e) => { let _ = tx.send(OpMsg::Failed(e.to_string())); }
            }
        });
        // record_undo: registrar UndoEntry con las rutas creadas cuando llegue OpMsg::Done en pump_ops.
        let _ = (id, label, record_undo);
    }
```

> NOTA IMPORTANTE: este código es una GUÍA del patrón. El implementador DEBE leer `spawn_op` y
> replicar su forma exacta de registrar la op en curso (el struct/HashMap que `pump_ops` recorre),
> porque de eso depende que el panel muestre el progreso y que la cancelación por id funcione. La
> conversión `OpMsg::Done(OpSummary)` necesita un `OpSummary` — crea un helper `zip_summary(items:
> Vec<ArchiveOpItem>) -> OpSummary` que mapee a `OpItem { dest, outcome, src: None }` (reusa el
> OpItem de I-3) para que el resumen "N hechos / M con error" funcione igual. Para el UNDO, registra
> las rutas Done en un `UndoEntry` con `UndoAction::TrashCreated` (ya existe) por cada ruta creada.

- [ ] **Step 3: Compilar**

Run: `cargo build -p naygo-ui-slint --bins` → compila.
Run: `cargo test -p naygo-ui-slint --bins` → verde.

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/src/ops_ctrl.rs
git commit -m "feat(ui): spawn_zip_op — worker async de comprimir/extraer reusando el panel de ops"
```

---

## FASE 3 — UI: menú contextual + modal de nombre

### Task 6: handlers en el controlador + cableado del menú

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl/context.rs`, `crates/ui-slint/src/workspace_ctrl/ops.rs`

- [ ] **Step 1: Métodos del controlador (con test del default name + armado del OpRequest)**

En `workspace_ctrl/ops.rs` (o `context.rs`, donde encajen mejor con los `op_*` existentes), agrega:
```rust
    /// Comprimir la selección en un `.zip` con `name`, en la carpeta del panel activo.
    pub fn op_compress(&mut self, name: String) {
        let sources = self.selected_paths();
        if sources.is_empty() { return; }
        let Some(dir) = self.active_dir() else { return; };
        let req = naygo_core::ops::OpRequest {
            kind: naygo_core::ops::OpKind::Compress { dest_name: ensure_zip_ext(&name) },
            sources,
            dest_dir: Some(dir),
            conflict: naygo_core::ops::ConflictPolicy::Ask,
        };
        self.ensure_ops_pane();
        self.ops.start_op(req, "Comprimir".to_string(), true);
    }

    /// Extraer el `.zip` seleccionado a `dest` (carpeta).
    pub fn op_extract_to(&mut self, dest: std::path::PathBuf) {
        let Some(zip) = self.selected_paths().into_iter().next() else { return; };
        let req = naygo_core::ops::OpRequest {
            kind: naygo_core::ops::OpKind::Extract,
            sources: vec![zip],
            dest_dir: Some(dest),
            conflict: naygo_core::ops::ConflictPolicy::Ask,
        };
        self.ensure_ops_pane();
        self.ops.start_op(req, "Extraer".to_string(), true);
    }

    /// "Extraer aquí": subcarpeta con el nombre del zip (sin extensión) en la carpeta actual.
    pub fn op_extract_here(&mut self) {
        let Some(zip) = self.selected_paths().into_iter().next() else { return; };
        let Some(dir) = self.active_dir() else { return; };
        let sub = zip.file_stem().and_then(|s| s.to_str()).unwrap_or("extraido");
        self.op_extract_to(dir.join(sub));
    }
```
Helper libre (en el mismo módulo o en mod.rs):
```rust
/// Garantiza que el nombre termine en ".zip" (sin duplicar si ya lo trae).
fn ensure_zip_ext(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".zip") { name.to_string() } else { format!("{name}.zip") }
}
```
Test (en el tests.rs del módulo o donde correspondan los tests del ctrl):
```rust
    #[test]
    fn ensure_zip_ext_agrega_y_no_duplica() {
        assert_eq!(super::ensure_zip_ext("foo"), "foo.zip");
        assert_eq!(super::ensure_zip_ext("foo.zip"), "foo.zip");
        assert_eq!(super::ensure_zip_ext("FOO.ZIP"), "FOO.ZIP");
    }
```

> Verifica los nombres reales: `selected_paths`, `active_dir`, `ensure_ops_pane`, `ops.start_op`,
> `ConflictPolicy`. Todos existen (los usa el resto del controlador). Ajusta `active_dir` si el
> método se llama distinto (busca cómo otros `op_*` obtienen la carpeta del panel activo).

- [ ] **Step 2: Modal de nombre del zip + ítems del menú en .slint**

Reusa el patrón del modal de "nueva carpeta" (`NewFolderState` / `new_folder_*`) para pedir el
nombre del zip con el default (`naygo_core::archive_ops::default_zip_name(&sources)`). Agrega al
menú contextual de archivos (`*.slint` del menú contextual; busca `ctx.open`/`slint.ctx.*`):
- «Comprimir en .zip…» → abre el modal de nombre → al confirmar llama `op_compress(name)`.
- «Extraer aquí» y «Extraer en…» → VISIBLES solo si la selección es exactamente 1 archivo `.zip`
  (condición en Slint sobre la extensión, o un flag `sel_is_zip` que el controlador exponga).
  «Extraer en…» abre el FileDialog de carpeta (busca cómo `browse_home`/el selector de carpeta
  nativo se usa hoy) → `op_extract_to(dir)`. «Extraer aquí» → `op_extract_here()`.

Cablea los callbacks nuevos en main.rs como los demás `on_ctx_*` (busca `on_ctx_copy` para el patrón).

- [ ] **Step 3: Compilar + test**

Run: `cargo build -p naygo-ui-slint --bins` y `cargo test -p naygo-ui-slint --bins` → verde.

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/src/workspace_ctrl/ crates/ui-slint/src/main.rs crates/ui-slint/ui/
git commit -m "feat(ui): menú contextual comprimir/extraer + modal de nombre del zip"
```

---

### Task 7: conflicto al extraer vía el diálogo existente

**Files:**
- Modify: `crates/ui-slint/src/ops_ctrl.rs`

- [ ] **Step 1: Conectar `on_conflict` del extract al `OpMsg::Conflict` + canal de respuesta**

En `spawn_zip_op`, reemplaza el `on_conflict` que siempre Overwrite por uno que: emita
`OpMsg::Conflict(prompt)` por `tx` y BLOQUEE esperando la decisión del usuario por un canal de
respuesta (igual que el engine de copiar maneja el conflicto). LEE cómo `exec_copy_step` /
`exec_step` esperan la decisión de conflicto (el `conflict_rx` que aparece en engine.rs) y replica
ese mecanismo: el worker manda el prompt y espera en un `Receiver<ConflictDecision>`; `pump_ops`
ya enruta la decisión del usuario de vuelta. Mapea `ConflictDecision` → `ExtractConflict`.

> Este es el paso más delicado: reusar el ConflictPrompt. Si el mecanismo de conflicto del engine
> es difícil de reusar tal cual desde el worker de zip, una alternativa aceptable para la 1ª entrega
> (documentarla): extraer SIEMPRE a una subcarpeta nueva con nombre único (sin colisiones posibles),
> evitando el conflicto por construcción — «Extraer aquí» ya va a una subcarpeta. En ese caso, el
> conflicto real solo ocurre en «Extraer en…» sobre una carpeta con contenido; ahí, usar KeepBoth
> (renombrar a "(2)") por defecto es razonable y no destructivo. DECIDIR con el revisor cuál de las
> dos vías; ambas respetan "nunca pisar sin querer".

- [ ] **Step 2: Compilar + test + commit**

Run: build + test verde.
```bash
git add crates/ui-slint/src/ops_ctrl.rs
git commit -m "feat(ui): conflicto al extraer .zip (reusa el diálogo de conflicto / subcarpeta única)"
```

---

## FASE 4 — i18n + cierre

### Task 8: claves i18n en los 10 idiomas

**Files:**
- Modify: `crates/core/src/i18n/{es,en,pt,fr,de,it,zh,ja,ko,hi}.json`, `crates/ui-slint/ui/i18n.slint`, `crates/ui-slint/src/i18n_keys.rs`

- [ ] **Step 1: Agregar las claves nuevas**

Claves (valor ES → EN; el resto se traduce siguiendo el patrón de los 10 idiomas — usa el mismo
proceso que la auditoría i18n: cada idioma con EXACTAMENTE las mismas claves):
```
slint.ctx.compress      es:"Comprimir en .zip…"   en:"Compress to .zip…"
slint.ctx.extract_here  es:"Extraer aquí"          en:"Extract here"
slint.ctx.extract_to    es:"Extraer en…"           en:"Extract to…"
slint.zipname.title     es:"Nombre del archivo .zip"  en:".zip file name"
slint.zipname.create    es:"Crear"                  en:"Create"
slint.zipname.cancel    es:"Cancelar"               en:"Cancel"
ops.kind_compress       es:"Comprimir"              en:"Compress"
ops.kind_extract        es:"Extraer"                en:"Extract"
```
Cablea cada una en `i18n.slint` (propiedad `in property <string>`) y en `i18n_keys.rs` (setter
`tr.set_*`). Para los textos de Rust (`"Comprimir"`/`"Extraer"` que pasé como label a start_op),
resuélvelos con `c.t("ops.kind_compress")` en vez del literal — así el panel los muestra traducidos.

- [ ] **Step 2: Validar parity**

Run: `cargo test -p naygo-core i18n` → el test de parity de los 10 idiomas debe seguir verde
(todas las claves nuevas en TODOS los idiomas). Usa `python scripts/check_i18n_lang.py <code>` por
idioma si ayuda.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/i18n/ crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/ui-slint/src/main.rs crates/ui-slint/src/workspace_ctrl/
git commit -m "feat(i18n): textos de comprimir/extraer en los 10 idiomas"
```

---

### Task 9: deshacer (provenance a papelera) + suite final

**Files:**
- Modify: `crates/ui-slint/src/ops_ctrl.rs`

- [ ] **Step 1: Registrar el UndoEntry de la op de zip**

Cuando `spawn_zip_op` recibe `OpMsg::Done(summary)` y `record_undo` es true, construye un
`UndoEntry` con una `UndoAction::TrashCreated { path }` por cada item Done del resumen (las rutas
CREADAS: el `.zip` al comprimir; los archivos/carpetas extraídos al extraer). Mira cómo
`pump_ops` registra el undo de Copy hoy (`build_undo`/`undo_history.push`) y replica para el zip
(no necesitas `build_undo` del engine; arma el `UndoEntry` directo con TrashCreated de cada ruta).
Así Ctrl+Z manda lo creado a papelera, una entrada en el historial.

- [ ] **Step 2: Test de deshacer (core o ui según dónde viva la lógica)**

Si la construcción del UndoEntry queda en una función testeable, añade un test: dado un resumen
con 2 items Done, el UndoEntry tiene 2 `TrashCreated` con esas rutas. Si queda enredado en
ops_ctrl, un test de humo: tras `op_compress`, el `.zip` existe y hay una entrada de undo.

- [ ] **Step 3: Suite completa + clippy**

Run:
```bash
cargo test -p naygo-core
cargo test -p naygo-ui-slint --bins
cargo clippy -p naygo-core
cargo clippy -p naygo-ui-slint --bins
```
Expected: todo verde, sin warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/src/ops_ctrl.rs
git commit -m "feat(ui): deshacer comprimir/extraer (lo creado a papelera)"
```

---

## Verificación visual (Nicolás, en la VM)

- Comprimir 1 archivo, varios archivos y una carpeta → el `.zip` se crea, se abre con Explorer/7-Zip y tiene el contenido correcto.
- Extraer aquí y Extraer en… → los archivos salen bien, en la subcarpeta / carpeta elegida.
- Panel de progreso con barra/velocidad/cancelar durante una compresión grande.
- Cancelar a media compresión → el `.zip` parcial desaparece.
- Conflicto al extraer sobre archivos existentes → el diálogo (o la subcarpeta única) se comporta como se decidió.
- Ctrl+Z tras comprimir/extraer → lo creado va a la papelera.
- Los 3 ítems del menú en otro idioma (p.ej. inglés) salen traducidos.
