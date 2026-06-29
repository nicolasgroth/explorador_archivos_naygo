# Preview de comprimidos: árbol + totales + tar/gz — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reescribir el preview de comprimidos como árbol ASCII (├─ └─) con encabezado de totales, y sumar soporte .tar/.tar.gz además de .zip.

**Architecture:** La construcción del texto del árbol vive en `core::archive_tree` (pura, testeable sin I/O): recibe una lista de `ArchiveEntry` + un `ArchiveSummary` y devuelve el texto. `ui-slint/src/preview.rs` lee el índice de zip/tar/tar.gz (con `zip`/`tar`/`flate2`), arma esa lista, y llama a la función pura. El preview se sigue pintando como texto monoespaciado (cero rediseño del panel).

**Tech Stack:** Rust. Crates: `zip` (ya está), `tar`, `flate2` (puro-Rust vía miniz_oxide).

---

## Contexto y convenciones (leer antes de empezar)

- Rama: `feat/iconos-personalizables`.
- ⚠️ **REGLA GIT (un subagente corrompió el árbol antes):** SOLO `git add <rutas explícitas>` + `git commit`. PROHIBIDO `git reset/restore/checkout/stash/clean`, `git commit -a`/`-am`, `git add -A`/`add .`. Hay 2 archivos ajenos (`CLAUDE.md`, `crates/core/src/favorites.rs`) que NO se tocan ni se stagean. Si el árbol parece mal, PARAR y reportar.
- Header en archivos nuevos: `// Naygo — <desc>.` / `// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.` / `// SPDX-License-Identifier: MIT`.
- Comentarios/commits español NEUTRAL, NUNCA voseo.
- Build limpio + clippy + tests antes de commit. Tests core: `cargo test -p naygo-core`. Tests/build ui: `cargo test -p naygo-ui-slint`, `cargo build -p naygo-ui-slint --bins`.

## Hechos verificados del código

- El preview YA muestra .zip: `read_zip_listing(path) -> Payload` en `crates/ui-slint/src/preview.rs:257`. `build_payload` (línea 228) hace `if is_zip(path) { return read_zip_listing(path); }`. `is_zip` (línea 244) compara la extensión.
- `Payload::Text { text: String, truncated: bool, highlighted: Option<Vec<HlLine>> }` (preview.rs:42).
- `ZIP_MAX_ENTRIES: usize = 500` (preview.rs:252).
- `naygo_core::format::format_size(bytes: u64, fmt: SizeFormat) -> String`; `SizeFormat::Auto` da "1.5 KB" etc.
- `zip = { version = "2", default-features = false, features = ["deflate"] }` ya está en `crates/ui-slint/Cargo.toml` y `crates/core/Cargo.toml`.
- `core/src/lib.rs` declara módulos con `pub mod <nombre>;` en orden alfabético.

## Mapa de archivos

**Crear:**
- `crates/core/src/archive_tree.rs` — `ArchiveEntry`, `ArchiveSummary`, `render_archive_tree` (puro) + tests.

**Modificar:**
- `crates/core/src/lib.rs` — `pub mod archive_tree;`.
- `crates/ui-slint/src/preview.rs` — `read_archive_listing` (reemplaza read_zip_listing), `archive_format`, `is_archive`, enrutado en `build_payload`.
- `crates/ui-slint/Cargo.toml` — deps `tar`, `flate2`.

---

## Fase 1 — `core::archive_tree` (puro)

### Task 1: Tipos `ArchiveEntry` / `ArchiveSummary` + esqueleto del módulo

**Files:**
- Create: `crates/core/src/archive_tree.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

En `crates/core/src/archive_tree.rs`:

```rust
// Naygo — texto del preview de comprimidos: encabezado de totales + árbol ASCII.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! Construye el TEXTO de la vista previa de un archivo comprimido (zip/tar): un encabezado
//! con totales (N archivos, M carpetas, tamaño) y un árbol ASCII indentado (├─ └─ │) del
//! contenido. Puro y testeable: recibe las entradas ya leídas (sin tocar disco), las capas
//! de I/O (ui-slint) le pasan la lista. Determinista.

use crate::format::{format_size, SizeFormat};

/// Una entrada de un archivo comprimido: ruta interna + tamaño descomprimido.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveEntry {
    /// Ruta interna con `/` como separador, p.ej. "proyecto/src/main.rs".
    pub path: String,
    pub is_dir: bool,
    /// Tamaño descomprimido en bytes (0 para carpetas).
    pub size: u64,
}

/// Resumen de un archivo comprimido (para el encabezado).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ArchiveSummary {
    pub files: usize,
    pub dirs: usize,
    pub total_uncompressed: u64,
    /// Si se listaron menos entradas que las reales (se aplicó un tope).
    pub truncated: bool,
    pub total_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_default_es_cero() {
        let s = ArchiveSummary::default();
        assert_eq!(s.files, 0);
        assert_eq!(s.dirs, 0);
        assert_eq!(s.total_uncompressed, 0);
        assert!(!s.truncated);
    }
}
```

Agregar a `crates/core/src/lib.rs` (orden alfabético, antes de `batch_rename`):
```rust
pub mod archive_tree;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core archive_tree`
Expected: FAIL si el módulo no compila (p.ej. `format_size`/`SizeFormat` mal importados) — ajustar el `use`. Si compila, el test pasa directo (es trivial). El objetivo de este task es el esqueleto + tipos.

- [ ] **Step 3: (si falló por import) ajustar**

Verifica que `crate::format::{format_size, SizeFormat}` existan con esos nombres (sí existen). Si el módulo compila, no hay nada que arreglar.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core archive_tree`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/archive_tree.rs crates/core/src/lib.rs
git commit -m "feat(core): tipos ArchiveEntry/ArchiveSummary del preview de comprimidos"
```

---

### Task 2: Construcción del árbol (nodos a partir de rutas planas)

**Files:**
- Modify: `crates/core/src/archive_tree.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn build_tree_crea_carpetas_implicitas_y_ordena() {
    // Entradas planas SIN carpetas explícitas: el árbol debe crear a/ y a/b/.
    let entries = vec![
        ArchiveEntry { path: "a/b/c.txt".into(), is_dir: false, size: 10 },
        ArchiveEntry { path: "a/z.txt".into(), is_dir: false, size: 20 },
        ArchiveEntry { path: "a/b/".into(), is_dir: true, size: 0 },
    ];
    let root = build_tree(&entries);
    // root tiene un hijo "a" (carpeta)
    assert_eq!(root.children.len(), 1);
    let a = &root.children[0];
    assert_eq!(a.name, "a");
    assert!(a.is_dir);
    // dentro de "a": carpeta "b" antes que archivo "z.txt" (carpetas primero, luego alfabético)
    assert_eq!(a.children.len(), 2);
    assert_eq!(a.children[0].name, "b");
    assert!(a.children[0].is_dir);
    assert_eq!(a.children[1].name, "z.txt");
    assert!(!a.children[1].is_dir);
    // dentro de "a/b": "c.txt"
    assert_eq!(a.children[0].children.len(), 1);
    assert_eq!(a.children[0].children[0].name, "c.txt");
    assert_eq!(a.children[0].children[0].size, 10);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core build_tree_crea_carpetas`
Expected: FAIL — `build_tree` / el tipo nodo no existen.

- [ ] **Step 3: Write minimal implementation**

Agregar a `archive_tree.rs` (antes del `mod tests`):

```rust
/// Un nodo del árbol del archivo: una carpeta (con hijos) o un archivo (hoja).
#[derive(Debug)]
pub struct TreeNode {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,            // tamaño del archivo (0 en carpetas)
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    fn new_dir(name: &str) -> TreeNode {
        TreeNode { name: name.to_string(), is_dir: true, size: 0, children: Vec::new() }
    }
}

/// Construye el árbol a partir de las entradas planas. Crea carpetas intermedias implícitas
/// (rutas presentes en un archivo pero sin entrada propia). Ordena cada nivel: carpetas
/// primero, luego archivos, alfabético dentro de cada grupo. Devuelve el nodo raíz (sin nombre).
pub fn build_tree(entries: &[ArchiveEntry]) -> TreeNode {
    let mut root = TreeNode::new_dir("");
    for e in entries {
        // partir la ruta por '/', ignorando segmentos vacíos (rutas con '//' o '/' final).
        let comps: Vec<&str> = e.path.split('/').filter(|s| !s.is_empty()).collect();
        if comps.is_empty() {
            continue;
        }
        let mut cur = &mut root;
        for (i, comp) in comps.iter().enumerate() {
            let last = i + 1 == comps.len();
            // ¿es el último componente y la entrada es un archivo?
            let want_file = last && !e.is_dir;
            // buscar el hijo con ese nombre
            let pos = cur.children.iter().position(|c| c.name == *comp);
            let idx = match pos {
                Some(p) => p,
                None => {
                    let node = if want_file {
                        TreeNode { name: comp.to_string(), is_dir: false, size: e.size, children: Vec::new() }
                    } else {
                        TreeNode::new_dir(comp)
                    };
                    cur.children.push(node);
                    cur.children.len() - 1
                }
            };
            // si ya existía pero ahora sabemos que es un archivo (caso raro), actualizar tamaño
            if want_file {
                cur.children[idx].is_dir = false;
                cur.children[idx].size = e.size;
            }
            cur = &mut cur.children[idx];
        }
    }
    sort_tree(&mut root);
    root
}

/// Ordena recursivamente: carpetas antes que archivos, alfabético (case-insensitive) dentro.
fn sort_tree(node: &mut TreeNode) {
    node.children.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir) // true (dir) antes que false (file)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for c in &mut node.children {
        sort_tree(c);
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core build_tree_crea_carpetas`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/archive_tree.rs
git commit -m "feat(core): build_tree — árbol del comprimido con carpetas implícitas + orden"
```

---

### Task 3: `render_archive_tree` (encabezado + árbol ASCII)

**Files:**
- Modify: `crates/core/src/archive_tree.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn render_incluye_encabezado_y_arbol_ascii() {
    let entries = vec![
        ArchiveEntry { path: "p/src/main.rs".into(), is_dir: false, size: 4300 },
        ArchiveEntry { path: "p/README.md".into(), is_dir: false, size: 2100 },
    ];
    let summary = ArchiveSummary {
        files: 2, dirs: 2, total_uncompressed: 6400, truncated: false, total_entries: 2,
    };
    let out = render_archive_tree(&entries, &summary, "demo.zip", SizeFormat::Auto);
    // encabezado con totales
    assert!(out.contains("2 archivo"), "menciona archivos");
    assert!(out.contains("carpeta"), "menciona carpetas");
    // árbol ASCII: hay conectores
    assert!(out.contains("├─ ") || out.contains("└─ "), "tiene conectores de árbol");
    // muestra los nombres
    assert!(out.contains("main.rs"));
    assert!(out.contains("README.md"));
    // el último hijo de la raíz usa └─
    assert!(out.contains("└─"));
}

#[test]
fn render_truncado_agrega_y_n_mas() {
    let summary = ArchiveSummary {
        files: 1, dirs: 0, total_uncompressed: 5, truncated: true, total_entries: 600,
    };
    let entries = vec![ArchiveEntry { path: "a.txt".into(), is_dir: false, size: 5 }];
    let out = render_archive_tree(&entries, &summary, "big.zip", SizeFormat::Auto);
    assert!(out.contains("más"), "indica que hay más entradas");
}

#[test]
fn render_lista_vacia_no_panica() {
    let out = render_archive_tree(&[], &ArchiveSummary::default(), "vacio.zip", SizeFormat::Auto);
    assert!(out.contains("0 archivo"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-core render_`
Expected: FAIL — `render_archive_tree` no existe.

- [ ] **Step 3: Write minimal implementation**

```rust
/// Texto del preview: encabezado de totales + árbol ASCII. Puro y determinista.
pub fn render_archive_tree(
    entries: &[ArchiveEntry],
    summary: &ArchiveSummary,
    name: &str,
    size_fmt: SizeFormat,
) -> String {
    let mut out = String::new();
    // Encabezado: nombre + totales.
    out.push_str(name);
    out.push('\n');
    out.push_str(&format!(
        "{} archivo(s), {} carpeta(s) · {} sin comprimir\n",
        summary.files,
        summary.dirs,
        format_size(summary.total_uncompressed, size_fmt),
    ));
    out.push_str("──────────────────────────────\n");
    // Árbol.
    let root = build_tree(entries);
    render_children(&root.children, "", &mut out, size_fmt);
    // Truncado.
    if summary.truncated {
        let extra = summary.total_entries.saturating_sub(entries.len());
        out.push_str(&format!("\n… y {extra} más\n"));
    }
    out
}

/// Pinta los hijos de un nivel con prefijo de continuación `prefix`. Cada hijo lleva
/// `├─ ` (no último) o `└─ ` (último); la continuación de sus subniveles usa `│  ` o `   `.
fn render_children(children: &[TreeNode], prefix: &str, out: &mut String, size_fmt: SizeFormat) {
    let n = children.len();
    for (i, node) in children.iter().enumerate() {
        let last = i + 1 == n;
        let connector = if last { "└─ " } else { "├─ " };
        out.push_str(prefix);
        out.push_str(connector);
        out.push_str(&node.name);
        if node.is_dir {
            out.push('/');
        } else {
            // tamaño alineado tras dos espacios
            out.push_str(&format!("  {}", format_size(node.size, size_fmt)));
        }
        out.push('\n');
        if node.is_dir && !node.children.is_empty() {
            let child_prefix = format!("{}{}", prefix, if last { "   " } else { "│  " });
            render_children(&node.children, &child_prefix, out, size_fmt);
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-core archive_tree`
Expected: PASS (todos: summary, build_tree, render).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/archive_tree.rs
git commit -m "feat(core): render_archive_tree — encabezado de totales + árbol ASCII"
```

---

## Fase 2 — Lectura de zip/tar/gz en ui-slint

### Task 4: Deps tar + flate2

**Files:**
- Modify: `crates/ui-slint/Cargo.toml`

- [ ] **Step 1: Agregar deps**

En `crates/ui-slint/Cargo.toml`, sección `[dependencies]`, agregar:
```toml
tar = "0.4"
flate2 = "1"
```
(flate2 usa por defecto el backend `miniz_oxide` puro-Rust — sin libs C, fiel a la regla del proyecto. NO añadir features que activen zlib-ng/zlib nativos.)

- [ ] **Step 2: Verificar que compila + licencias**

Run: `cargo build -p naygo-ui-slint --bins`
Expected: compila (descarga tar + flate2).
Verifica que ambas son permisivas: `cargo tree -p naygo-ui-slint -i tar` y `-i flate2` no son necesarios, pero confirma en crates.io que tar=MIT/Apache-2.0 y flate2=MIT/Apache-2.0 (lo son). Si el proyecto regenera THIRD-PARTY-NOTICES, hazlo en el task de cierre.

- [ ] **Step 3: Commit**

```bash
git add crates/ui-slint/Cargo.toml Cargo.lock
git commit -m "build(ui): deps tar + flate2 (puro-Rust) para preview de tar/tar.gz"
```

---

### Task 5: `read_archive_listing` (zip/tar/tar.gz → render)

**Files:**
- Modify: `crates/ui-slint/src/preview.rs`

- [ ] **Step 1: Write the failing test**

Agregar a los tests de `crates/ui-slint/src/preview.rs` (hay `#[cfg(test)] mod tests`? si no, créalo). Test de integración liviano: crear un .zip en un tempdir y leerlo.

```rust
#[test]
fn read_archive_listing_zip_muestra_entradas() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("demo.zip");
    {
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zw.start_file("carpeta/hola.txt", opts).unwrap();
        zw.write_all(b"hola mundo").unwrap();
        zw.finish().unwrap();
    }
    let payload = read_archive_listing(&zip_path);
    match payload {
        Payload::Text { text, .. } => {
            assert!(text.contains("hola.txt"), "lista el archivo");
            assert!(text.contains("carpeta"), "lista la carpeta");
            assert!(text.contains("archivo"), "tiene encabezado de totales");
        }
        _ => panic!("se esperaba Payload::Text"),
    }
}

#[test]
fn read_archive_listing_corrupto_es_message() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("malo.zip");
    std::fs::write(&bad, b"esto no es un zip").unwrap();
    let payload = read_archive_listing(&bad);
    assert!(matches!(payload, Payload::Message(_)), "zip dañado => Message, no panic");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-ui-slint read_archive_listing`
Expected: FAIL — `read_archive_listing` no existe.

- [ ] **Step 3: Write minimal implementation**

En `crates/ui-slint/src/preview.rs`, reemplazar `read_zip_listing` (y `is_zip`) por:

```rust
/// Formato de archivo comprimido detectado por extensión.
enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
}

/// Detecta el formato por extensión (case-insensitive). `None` si no es comprimido legible.
fn archive_format(path: &Path) -> Option<ArchiveFormat> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Some(ArchiveFormat::TarGz)
    } else if name.ends_with(".tar") {
        Some(ArchiveFormat::Tar)
    } else if name.ends_with(".zip") {
        Some(ArchiveFormat::Zip)
    } else {
        None
    }
}

/// `true` si el archivo es un comprimido con preview (zip/tar/tar.gz/tgz).
fn is_archive(path: &Path) -> bool {
    archive_format(path).is_some()
}

/// Cuántas entradas listar como máximo (evita textos enormes en archivos con miles).
const ARCHIVE_MAX_ENTRIES: usize = 500;

/// Lee el índice de un comprimido (zip/tar/tar.gz) y devuelve el texto del preview
/// (encabezado + árbol ASCII) vía `naygo_core::archive_tree`. Tolerante: archivo
/// ilegible/corrupto → `Payload::Message`. No extrae contenido (solo el índice).
fn read_archive_listing(path: &Path) -> Payload {
    use naygo_core::archive_tree::{ArchiveEntry, ArchiveSummary, render_archive_tree};
    let Some(fmt) = archive_format(path) else {
        return Payload::Message("No previsualizable".to_string());
    };
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    // Acumular entradas (hasta el tope) + summary.
    let mut entries: Vec<ArchiveEntry> = Vec::new();
    let mut summary = ArchiveSummary::default();
    let read_ok = match fmt {
        ArchiveFormat::Zip => read_zip_entries(path, &mut entries, &mut summary),
        ArchiveFormat::Tar => read_tar_entries(path, &mut entries, &mut summary, false),
        ArchiveFormat::TarGz => read_tar_entries(path, &mut entries, &mut summary, true),
    };
    if !read_ok {
        return Payload::Message("Archivo comprimido inválido o dañado".to_string());
    }
    let text = render_archive_tree(
        &entries, &summary, &name, naygo_core::format::SizeFormat::Auto,
    );
    Payload::Text { text, truncated: summary.truncated, highlighted: None }
}

/// Lee las entradas de un .zip. Devuelve false si no se pudo abrir/leer.
fn read_zip_entries(
    path: &Path,
    entries: &mut Vec<naygo_core::archive_tree::ArchiveEntry>,
    summary: &mut naygo_core::archive_tree::ArchiveSummary,
) -> bool {
    use naygo_core::archive_tree::ArchiveEntry;
    let Ok(file) = std::fs::File::open(path) else { return false; };
    let Ok(mut archive) = zip::ZipArchive::new(file) else { return false; };
    let total = archive.len();
    summary.total_entries = total;
    let shown = total.min(ARCHIVE_MAX_ENTRIES);
    summary.truncated = total > shown;
    for i in 0..shown {
        let Ok(entry) = archive.by_index(i) else { continue; };
        let is_dir = entry.is_dir();
        let size = entry.size();
        if is_dir { summary.dirs += 1; } else { summary.files += 1; summary.total_uncompressed += size; }
        entries.push(ArchiveEntry { path: entry.name().to_string(), is_dir, size });
    }
    true
}

/// Lee las entradas de un .tar o .tar.gz (si `gz`, descomprime con flate2). false si falla.
fn read_tar_entries(
    path: &Path,
    entries: &mut Vec<naygo_core::archive_tree::ArchiveEntry>,
    summary: &mut naygo_core::archive_tree::ArchiveSummary,
    gz: bool,
) -> bool {
    use naygo_core::archive_tree::ArchiveEntry;
    let Ok(file) = std::fs::File::open(path) else { return false; };
    // Caja para el reader (gz o directo).
    let reader: Box<dyn std::io::Read> = if gz {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    };
    let mut archive = tar::Archive::new(reader);
    let Ok(iter) = archive.entries() else { return false; };
    for entry in iter {
        if entries.len() >= ARCHIVE_MAX_ENTRIES {
            summary.truncated = true;
            // seguir contando el total es caro en tar (hay que iterar igual); paramos y marcamos.
            break;
        }
        let Ok(e) = entry else { continue; };
        let header = e.header();
        let is_dir = header.entry_type().is_dir();
        let size = header.size().unwrap_or(0);
        let path_str = e.path().ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if path_str.is_empty() { continue; }
        if is_dir { summary.dirs += 1; } else { summary.files += 1; summary.total_uncompressed += size; }
        entries.push(ArchiveEntry { path: path_str, is_dir, size });
    }
    summary.total_entries = entries.len() + if summary.truncated { 1 } else { 0 };
    true
}
```
> NOTA tar: a diferencia de zip (índice central), tar es secuencial — no se sabe el total
> sin iterar todo. Por eso, al cortar en el tope marcamos `truncated` y `total_entries` se
> aproxima (no se sigue contando para no leer todo). El encabezado dirá los totales de lo
> mostrado; "… y N más" usa `total_entries - entries.len()` que será ≥0 (al menos 1). Es
> aceptable para un preview liviano. Documenta esto con un comentario.

- [ ] **Step 4: Enrutar en build_payload**

En `build_payload` (~línea 228), cambiar:
```rust
    if is_zip(path) {
        return read_zip_listing(path);
    }
```
por:
```rust
    if is_archive(path) {
        return read_archive_listing(path);
    }
```
(Eliminar la antigua `is_zip` y `read_zip_listing` si quedaron sin uso; `read_zip_entries` las reemplaza.)

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p naygo-ui-slint read_archive_listing`
Expected: PASS (zip + corrupto).

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/src/preview.rs
git commit -m "feat(ui): preview de comprimidos zip/tar/tar.gz como árbol (vía archive_tree)"
```

---

### Task 6: Test de integración tar.gz + cierre

**Files:**
- Modify: `crates/ui-slint/src/preview.rs` (test) 

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn read_archive_listing_targz_muestra_entradas() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let tgz_path = dir.path().join("demo.tar.gz");
    {
        let file = std::fs::File::create(&tgz_path).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        let data = b"contenido";
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_cksum();
        tar.append_data(&mut header, "dir/archivo.txt", &data[..]).unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }
    let payload = read_archive_listing(&tgz_path);
    match payload {
        Payload::Text { text, .. } => {
            assert!(text.contains("archivo.txt"), "lista el archivo del tar.gz");
        }
        _ => panic!("se esperaba Payload::Text"),
    }
}
```
> Ajustar la API de `tar::Builder`/`Header` a la versión real (0.4) si difiere; el patrón
> es: GzEncoder envuelve el File, tar::Builder sobre el encoder, append_data con un Header
> que tenga el tamaño, finish del gz al final. Si la API exacta varía, adáptala — el objetivo
> es producir un .tar.gz válido de prueba.

- [ ] **Step 2: Run test to verify it fails (o pasa)**

Run: `cargo test -p naygo-ui-slint read_archive_listing_targz`
Expected: PASS si Task 5 quedó correcta (este test ejercita el camino TarGz). Si falla por la API de tar en el test, arregla el test (no la implementación, salvo que revele un bug real).

- [ ] **Step 3: Suite + clippy + build**

Run: `cargo test -p naygo-core` y `cargo test -p naygo-ui-slint` → verdes.
Run: `cargo clippy -p naygo-core` y `cargo clippy -p naygo-ui-slint --bins` → sin warnings.
Run: `cargo build -p naygo-ui-slint --bins` → compila.

- [ ] **Step 4: THIRD-PARTY-NOTICES (si el proyecto lo mantiene a mano)**

Verifica si `THIRD-PARTY-NOTICES.md` lista las deps. Si se genera con `cargo license`, regenéralo; si es a mano, agrega tar (MIT/Apache-2.0) y flate2 (MIT/Apache-2.0) + miniz_oxide (MIT/Zlib/Apache-2.0) a la lista. Commit aparte:
`git add THIRD-PARTY-NOTICES.md; git commit -m "docs(licenses): tar + flate2 + miniz_oxide en THIRD-PARTY-NOTICES"`.

- [ ] **Step 5: Commit del test**

```bash
git add crates/ui-slint/src/preview.rs
git commit -m "test(ui): integración del preview de tar.gz"
```

- [ ] **Step 6: Verificación en VM (Nicolás)**

Seleccionar un .zip, un .tar y un .tar.gz reales en el explorador y confirmar que el preview
muestra el árbol con encabezado de totales.

---

## Resumen de fases
1. **core::archive_tree** (puro, TDD): tipos → build_tree → render_archive_tree.
2. **ui-slint**: deps tar/flate2 → read_archive_listing (zip/tar/tar.gz) → enrutar → tests + cierre.
