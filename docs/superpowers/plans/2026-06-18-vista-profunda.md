# Vista profunda (modo recursivo en el panel) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agregar a Naygo un modo de vista "profunda" en el panel de archivos que lista, de forma plana y con sangría por profundidad, todo el árbol bajo la carpeta actual (recursivo), por streaming y cancelable.

**Architecture:** Un motor nuevo en `core` (`deep_listing.rs`) que es el gemelo del walk de búsqueda — mismo recorrido con pila propia, sin descender a symlinks, tolerante a carpetas ilegibles — pero SIN filtro de nombre y SIN tope, adjuntando `depth` y `rel_path` a cada entrada. La UI consume ese stream en un modo nuevo del panel (junto a normal/búsqueda), vuelca las entradas en las filas `RowData` normales (que ganan `depth` para la sangría), y reusa orden/filtros/selección/operaciones existentes. Un toggle en la barra del panel lo activa.

**Tech Stack:** Rust workspace (naygo-core / naygo-platform / naygo-ui-slint), Slint 1.16 (render software). Build SIEMPRE con `CARGO_BUILD_JOBS=2`. Gate de cada tarea de código: `cargo fmt --all` + `cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform` + `cargo clippy --workspace --all-targets -- -D warnings`.

**Spec:** `docs/superpowers/specs/2026-06-18-vista-profunda-design.md`

**Convenciones del repo (recordatorio):**
- Español neutral SIN voseo en todo (código, comentarios, docs, UI). PowerShell 5.1: nunca `&&`/`||`.
- Header en archivos nuevos: `// Naygo — <descr>` + `// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.`
- i18n triple: `i18n.slint` (`in property <string> kebab-case`) + `es.json`/`en.json` (`"slint.xxx.yyy"`) + `i18n_keys.rs` (`tr.set_xxx(c.t("slint.xxx.yyy").into())`).
- Iconos con Slint `Path`, NUNCA glifos de fuente (render por software).
- NO commitear: `CLAUDE.md`, `assets/icons/otros/`, `graphify-out/` (estos dos últimos ya en .gitignore).
- NO `git push` (lo hace Nicolás). Commits locales por tarea.
- Mensajes de commit terminan con `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Tras tocar código: `graphify update .`.

**Patrones existentes a imitar (ya leídos):**
- `core/src/search.rs`: `search_walk(root, query, lister: &dyn Fn(&Path)->ListResult, token, tx)` con pila `Vec<PathBuf>`, `SearchMsg::{Hit(Entry),Progress{dirs_scanned},Done{partial,hit_cap},Cancelled}`, `MAX_HITS=5000`, `make_entry(path,is_dir)`, `fs_lister`, `DirEntryRaw{path,is_dir,is_symlink}`, `ListResult = Option<Vec<DirEntryRaw>>`. El `lister` es inyectable → los tests usan un closure con un `HashMap`.
- `ui-slint/src/workspace_ctrl.rs`: `SearchJob{ root, query, rx, token, hits, dirs_scanned, done, cancelled, partial, hit_cap }` (worker+canal+token+acumulado). La búsqueda usa un OVERLAY aparte (`SearchRow`/`SearchHitVm`). **La vista profunda NO usa ese overlay**: alimenta las filas normales del panel (`RowData`).

---

## File Structure

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `crates/core/src/deep_listing.rs` | Motor del walk profundo: `DeepEntry`, `DeepMsg`, `spawn_deep_listing`, `deep_walk`. + tests. | Crear |
| `crates/core/src/search.rs` | `DirEntryRaw`, `ListResult`, `fs_lister` → `pub(crate)` (sin cambiar comportamiento). | Modificar |
| `crates/core/src/lib.rs` | `pub mod deep_listing;`. | Modificar |
| `crates/ui-slint/ui/types.slint` | `RowData` gana `depth: int`. | Modificar |
| `crates/ui-slint/ui/file-panel.slint` | Sangría por `depth` en la fila; botón toggle de vista profunda en la barra del panel. | Modificar |
| `crates/ui-slint/ui/i18n.slint` | `deep-view`, `deep-view-tip`. | Modificar |
| `crates/core/src/i18n/es.json`, `en.json` | Traducciones. | Modificar |
| `crates/ui-slint/src/i18n_keys.rs` | Setters. | Modificar |
| `crates/ui-slint/src/workspace_ctrl.rs` | `DeepJob` + activar/desactivar/poll, volcar filas con `depth`, cancelar al navegar/apagar, ruteo doble clic. | Modificar |
| `crates/ui-slint/src/main.rs` | Cablear callback del toggle + poll del DeepJob. | Modificar |

**Orden:** core primero (Task 1-2, testeable), luego i18n (Task 3), luego RowData+sangría (Task 4), luego el controlador (Task 5, la integración), luego toggle UI + wiring (Task 6).

---

## Task 1: `pub(crate)` en search.rs para compartir el lister

Permite que `deep_listing` reuse `DirEntryRaw`/`ListResult`/`fs_lister` sin duplicarlos. Cambio de visibilidad puro.

**Files:**
- Modify: `crates/core/src/search.rs`

- [ ] **Step 1: Cambiar visibilidad de los tres símbolos**

En `crates/core/src/search.rs`:
- `struct DirEntryRaw {` → `pub(crate) struct DirEntryRaw {` y sus campos `path`/`is_dir`/`is_symlink` a `pub(crate)`:
```rust
pub(crate) struct DirEntryRaw {
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
    pub(crate) is_symlink: bool,
}
```
- `type ListResult = ...` → `pub(crate) type ListResult = Option<Vec<DirEntryRaw>>;`
- `fn fs_lister(dir: &Path) -> ListResult {` → `pub(crate) fn fs_lister(dir: &Path) -> ListResult {`

No cambies nada más de search.rs (ni comportamiento ni la API pública).

- [ ] **Step 2: Verificar que compila y los tests de search siguen verdes**

```
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core search
```
Esperado: PASS (los tests de search no cambian).

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/search.rs
git commit -F - <<'EOF'
refactor(core): exponer lister de search como pub(crate)

DirEntryRaw, ListResult y fs_lister pasan a pub(crate) para que deep_listing
reuse el recorrido de directorios sin duplicarlo. Sin cambios de comportamiento.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 2: Motor `deep_listing.rs` en core (gemelo del walk, sin filtro ni tope)

**Files:**
- Create: `crates/core/src/deep_listing.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear `crates/core/src/deep_listing.rs` con el motor**

```rust
// Naygo — listado profundo: recorrido recursivo cancelable de TODO el árbol, con streaming.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lista todo el contenido bajo una carpeta raíz, recorriendo subcarpetas a cualquier
//! profundidad. Gemelo del recorrido de `search`, pero SIN filtro de nombre (emite cada
//! entrada) y SIN tope (`MAX_HITS`): el control de tamaño es el streaming + la cancelación.
//! Usa una pila propia (no recursión de stack), NO desciende a symlinks/junctions (evita
//! loops), y marca el resultado como "parcial" si alguna carpeta fue ilegible. Cada entrada
//! se emite con su PROFUNDIDAD (0 = hijo directo de la raíz) y su RUTA RELATIVA a la raíz,
//! para que la UI pueda sangrar y mostrar el origen. La consume el modo "vista profunda".

use crate::cancel::CancellationToken;
use crate::fs_model::Entry;
use crate::listing::entry_from_path;
use crate::search::{fs_lister, ListResult};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Throttle del mensaje `Progress` (cuántas carpetas se llevan recorridas).
const PROGRESS_THROTTLE: Duration = Duration::from_millis(150);

/// Una entrada del recorrido profundo: la entrada normal + su lugar en el árbol.
#[derive(Debug, Clone, PartialEq)]
pub struct DeepEntry {
    /// La entrada en sí (path absoluto, nombre, tipo, tamaño, fecha…).
    pub entry: Entry,
    /// Profundidad relativa a la raíz: 0 = hijo directo de la raíz, 1 = nieto, etc.
    pub depth: u32,
    /// Ruta relativa a la raíz, con separadores del SO (p. ej. "2025\\enero\\informe.pdf").
    /// Vacía nunca: siempre incluye al menos el nombre del propio ítem.
    pub rel_path: String,
}

/// Mensajes del worker de listado profundo hacia la UI. Espeja a `SearchMsg` pero sin
/// `hit_cap` (no hay tope): emite TODO hasta agotar el árbol o ser cancelado.
#[derive(Debug, Clone, PartialEq)]
pub enum DeepMsg {
    Entry(DeepEntry),
    Progress { dirs_scanned: usize },
    Done { partial: bool },
    Cancelled,
}

/// Lanza el listado profundo bajo `root` en un worker. La UI drena el receptor frame a
/// frame. Cancelable vía `token`.
pub fn spawn_deep_listing(
    root: PathBuf,
    token: CancellationToken,
) -> (Receiver<DeepMsg>, JoinHandle<()>) {
    let (tx, rx) = channel();
    let handle = thread::spawn(move || {
        deep_walk(&root, &fs_lister, &token, &tx);
    });
    (rx, handle)
}

/// Núcleo PURO: recorre el árbol bajo `root` con una pila de `(carpeta, profundidad)`,
/// emitiendo por `tx` CADA entrada (no filtra) con su `depth` y `rel_path`. `lister` produce
/// las entradas de un directorio (en producción lee el FS; en tests, un closure). Chequea
/// `token` entre directorios. NO desciende a symlinks/junctions. Sin tope.
fn deep_walk(
    root: &Path,
    lister: &dyn Fn(&Path) -> ListResult,
    token: &CancellationToken,
    tx: &Sender<DeepMsg>,
) {
    let mut partial = false;
    let mut dirs_scanned = 0usize;
    let mut last_progress = Instant::now();
    // La pila lleva (carpeta a listar, profundidad de SUS HIJOS).
    let mut stack: Vec<(PathBuf, u32)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if token.is_cancelled() {
            let _ = tx.send(DeepMsg::Cancelled);
            return;
        }
        match lister(&dir) {
            None => partial = true,
            Some(entries) => {
                dirs_scanned += 1;
                for e in entries {
                    let rel_path = e
                        .path
                        .strip_prefix(root)
                        .unwrap_or(&e.path)
                        .to_string_lossy()
                        .into_owned();
                    let entry = make_entry(&e.path, e.is_dir);
                    if tx
                        .send(DeepMsg::Entry(DeepEntry {
                            entry,
                            depth,
                            rel_path,
                        }))
                        .is_err()
                    {
                        // El receptor se cayó (la UI cerró el modo): dejar de trabajar.
                        return;
                    }
                    // Descender a subcarpetas (jamás a symlinks/junctions: evita loops).
                    if e.is_dir && !e.is_symlink {
                        stack.push((e.path, depth + 1));
                    }
                }
                if last_progress.elapsed() >= PROGRESS_THROTTLE {
                    let _ = tx.send(DeepMsg::Progress { dirs_scanned });
                    last_progress = Instant::now();
                }
            }
        }
    }

    if token.is_cancelled() {
        let _ = tx.send(DeepMsg::Cancelled);
    } else {
        let _ = tx.send(DeepMsg::Done { partial });
    }
}

/// Construye el `Entry` de una entrada leyendo su metadata real (tamaño/fechas). Si la
/// metadata no se puede leer, cae a un `Entry` mínimo con el tipo ya conocido del walk.
/// (Misma idea que `search::make_entry`; se replica aquí para no acoplar módulos por una
/// función trivial.)
fn make_entry(path: &Path, is_dir: bool) -> Entry {
    match std::fs::metadata(path) {
        Ok(m) => entry_from_path(path, Some(&m)),
        Err(_) => Entry {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: path.to_path_buf(),
            kind: if is_dir {
                crate::fs_model::EntryKind::Directory
            } else {
                crate::fs_model::EntryKind::File
            },
            size: None,
            modified: None,
            created: None,
            hidden: false,
        },
    }
}
```

> Nota de diseño: `make_entry` se replica (no se reexporta el de search) porque es trivial y
> reexportar una privada de search solo para esto acoplaría los módulos sin beneficio. Si en
> review se prefiere compartir, se puede hacer `pub(crate)` en search en otra iteración.

- [ ] **Step 2: Exponer el módulo en `lib.rs`**

En `crates/core/src/lib.rs`, en orden alfabético. El orden actual incluye `... config; disk; dnd; ...` — `deep_listing` va **entre `config` y `disk`**:
```rust
pub mod config;
pub mod deep_listing;
pub mod disk;
```

- [ ] **Step 3: Tests al final de `deep_listing.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::DirEntryRaw;
    use std::collections::HashMap;
    use std::sync::mpsc::channel;

    // Construye un lister falso desde un mapa carpeta -> entradas. Las rutas son sintéticas
    // (no tocan el FS). `make_entry` sí leerá metadata real y caerá al Entry mínimo (las
    // rutas no existen), lo cual es suficiente para verificar depth/rel_path/recorrido.
    fn fake_lister(
        map: HashMap<PathBuf, Vec<DirEntryRaw>>,
    ) -> impl Fn(&Path) -> ListResult {
        move |dir: &Path| map.get(dir).cloned().map(|v| v.to_vec())
    }

    fn raw(path: &str, is_dir: bool) -> DirEntryRaw {
        DirEntryRaw {
            path: PathBuf::from(path),
            is_dir,
            is_symlink: false,
        }
    }

    fn collect(root: &str, lister: &dyn Fn(&Path) -> ListResult) -> Vec<DeepEntry> {
        let (tx, rx) = channel();
        let token = CancellationToken::new();
        deep_walk(Path::new(root), lister, &token, &tx);
        let mut out = Vec::new();
        for m in rx.iter() {
            match m {
                DeepMsg::Entry(e) => out.push(e),
                DeepMsg::Done { .. } | DeepMsg::Cancelled => break,
                DeepMsg::Progress { .. } => {}
            }
        }
        out
    }

    #[test]
    fn emite_todo_el_arbol_con_depth_y_rel_path() {
        // raíz: a.txt, sub/   ;   sub/: b.txt, sub/deep/   ;   sub/deep/: c.txt
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        map.insert(
            PathBuf::from("/root"),
            vec![raw("/root/a.txt", false), raw("/root/sub", true)],
        );
        map.insert(
            PathBuf::from("/root/sub"),
            vec![raw("/root/sub/b.txt", false), raw("/root/sub/deep", true)],
        );
        map.insert(
            PathBuf::from("/root/sub/deep"),
            vec![raw("/root/sub/deep/c.txt", false)],
        );
        let lister = fake_lister(map);
        let items = collect("/root", &lister);

        // Debe emitir 5 entradas (a.txt, sub, b.txt, deep, c.txt) en algún orden.
        assert_eq!(items.len(), 5);
        let by_rel: HashMap<&str, &DeepEntry> =
            items.iter().map(|e| (e.rel_path.as_str(), e)).collect();
        assert_eq!(by_rel["a.txt"].depth, 0);
        assert_eq!(by_rel["sub"].depth, 0);
        // rel_path usa el separador del SO; en los tests las rutas se construyen con "/",
        // así que comparamos por componentes para no depender del separador.
        let deep = items
            .iter()
            .find(|e| e.rel_path.replace('\\', "/") == "sub/deep")
            .expect("sub/deep presente");
        assert_eq!(deep.depth, 1);
        let c = items
            .iter()
            .find(|e| e.rel_path.replace('\\', "/") == "sub/deep/c.txt")
            .expect("c.txt presente");
        assert_eq!(c.depth, 2);
    }

    #[test]
    fn no_desciende_a_symlinks() {
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        // 'link' es un dir symlink: aparece como entrada pero NO se recorre su interior.
        let mut link = raw("/root/link", true);
        link.is_symlink = true;
        map.insert(PathBuf::from("/root"), vec![link]);
        map.insert(
            PathBuf::from("/root/link"),
            vec![raw("/root/link/inside.txt", false)],
        );
        let lister = fake_lister(map);
        let items = collect("/root", &lister);
        // Solo 'link' se emite; 'inside.txt' NO (no se descendió).
        assert_eq!(items.len(), 1);
        assert!(items.iter().all(|e| !e.rel_path.contains("inside")));
    }

    #[test]
    fn carpeta_ilegible_no_aborta_y_marca_partial() {
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        // raíz tiene 'ok.txt' y 'bad' (dir); 'bad' NO está en el mapa => lister da None => partial.
        map.insert(
            PathBuf::from("/root"),
            vec![raw("/root/ok.txt", false), raw("/root/bad", true)],
        );
        let lister = fake_lister(map);
        let (tx, rx) = channel();
        let token = CancellationToken::new();
        deep_walk(Path::new("/root"), &lister, &token, &tx);
        let mut partial = None;
        let mut count = 0;
        for m in rx.iter() {
            match m {
                DeepMsg::Entry(_) => count += 1,
                DeepMsg::Done { partial: p } => {
                    partial = Some(p);
                    break;
                }
                DeepMsg::Cancelled => break,
                DeepMsg::Progress { .. } => {}
            }
        }
        assert_eq!(count, 2); // ok.txt + bad (el dir se emite aunque no se pueda listar)
        assert_eq!(partial, Some(true));
    }

    #[test]
    fn token_cancelado_emite_cancelled_y_no_recorre() {
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        map.insert(PathBuf::from("/root"), vec![raw("/root/a.txt", false)]);
        let lister = fake_lister(map);
        let (tx, rx) = channel();
        let token = CancellationToken::new();
        token.cancel();
        deep_walk(Path::new("/root"), &lister, &token, &tx);
        let first = rx.recv().expect("debe haber un mensaje");
        assert_eq!(first, DeepMsg::Cancelled);
    }

    #[test]
    fn arbol_vacio_emite_solo_done() {
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        map.insert(PathBuf::from("/root"), vec![]);
        let lister = fake_lister(map);
        let items = collect("/root", &lister);
        assert!(items.is_empty());
    }

    #[test]
    fn sin_tope_emite_mas_de_5000() {
        // Search corta en MAX_HITS=5000; deep NO. Verificamos > 5000 en una sola carpeta.
        let big: Vec<DirEntryRaw> = (0..6000)
            .map(|i| raw(Box::leak(format!("/root/f{i}.txt").into_boxed_str()), false))
            .collect();
        let mut map: HashMap<PathBuf, Vec<DirEntryRaw>> = HashMap::new();
        map.insert(PathBuf::from("/root"), big);
        let lister = fake_lister(map);
        let items = collect("/root", &lister);
        assert_eq!(items.len(), 6000);
    }
}
```

> Si `DirEntryRaw` no es construible desde el test por visibilidad de campos, confirma que la
> Task 1 los dejó `pub(crate)` (los tests están en el mismo crate, así que `pub(crate)` basta).
> Si `entry_from_path` requiere una firma distinta a `(path, Option<&Metadata>)`, ajústalo a la
> real (mira `search::make_entry`, que ya la llama así).

- [ ] **Step 4: Tests pasan**

```
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core deep_listing
```
Esperado: 6 tests PASS.

- [ ] **Step 5: Gate + commit**

```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all
$env:CARGO_BUILD_JOBS = "2"; cargo clippy -p naygo-core --all-targets -- -D warnings
```
Esperado: sin warnings. Luego:

```bash
git add crates/core/src/deep_listing.rs crates/core/src/lib.rs
git commit -F - <<'EOF'
feat(core): motor de listado profundo (deep_listing)

spawn_deep_listing + deep_walk: recorre TODO el arbol bajo una raiz (recursivo,
pila propia, sin symlinks, tolerante a carpetas ilegibles), emitiendo cada
entrada con su depth y rel_path por streaming. Sin filtro y sin tope (a
diferencia de search). Con tests (depth/rel_path, symlinks, parcial, cancelado,
vacio, sin tope).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 3: i18n del toggle de vista profunda

Claves: `deep-view` ("Vista profunda" / "Deep view") y `deep-view-tip` (tooltip explicativo).

**Files:**
- Modify: `crates/ui-slint/ui/i18n.slint`, `crates/core/src/i18n/es.json`, `crates/core/src/i18n/en.json`, `crates/ui-slint/src/i18n_keys.rs`

- [ ] **Step 1: Defaults en `i18n.slint`**

Junto a otras claves de la barra del panel o de acciones, agregar:
```slint
    in property <string> deep-view: "Vista profunda";
    in property <string> deep-view-tip: "Muestra todo el contenido de la carpeta y sus subcarpetas (recursivo)";
```

- [ ] **Step 2: `es.json`** — agregar (cuidando comas JSON):
```json
  "slint.deep.view": "Vista profunda",
  "slint.deep.view_tip": "Muestra todo el contenido de la carpeta y sus subcarpetas (recursivo)",
```

- [ ] **Step 3: `en.json`** — agregar:
```json
  "slint.deep.view": "Deep view",
  "slint.deep.view_tip": "Shows all contents of the folder and its subfolders (recursive)",
```

- [ ] **Step 4: Setters en `i18n_keys.rs`** — agregar junto a los otros `tr.set_...`:
```rust
    tr.set_deep_view(c.t("slint.deep.view").into());
    tr.set_deep_view_tip(c.t("slint.deep.view_tip").into());
```

- [ ] **Step 5: Compila**

```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
```
Esperado: compila (los setters `set_deep_view`/`set_deep_view_tip` los genera Slint a partir de las `in property`).

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/ui/i18n.slint crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/src/i18n_keys.rs
git commit -F - <<'EOF'
feat(i18n): claves del toggle de vista profunda

deep-view y deep-view-tip en las tres capas.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 4: `RowData` gana `depth` + sangría en la fila

**Files:**
- Modify: `crates/ui-slint/ui/types.slint` (struct `RowData`)
- Modify: `crates/ui-slint/ui/file-panel.slint` (sangría)

- [ ] **Step 1: Agregar `depth` a `RowData`**

En `crates/ui-slint/ui/types.slint`, dentro de `export struct RowData { ... }`, agregar el campo:
```slint
    depth: int,
```
(Default de los int en Slint es 0, así que en vista normal/búsqueda no hay que setearlo.)

- [ ] **Step 2: Aplicar la sangría en la fila del panel**

En `crates/ui-slint/ui/file-panel.slint`, localiza dónde se renderiza el NOMBRE de cada fila (la celda de la columna nombre dentro del `for row in root.rows`). Agrega un padding-left proporcional a `row.depth`. El patrón concreto: si el nombre vive en un `HorizontalLayout`/`Rectangle`, antes del texto del nombre inserta un espaciador de ancho `row.depth * 14px`, o suma `row.depth * 14px` al `x`/`padding-left` del contenedor del nombre. Ejemplo de espaciador:
```slint
    // Sangría por profundidad (vista profunda). En vista normal depth=0 => sin efecto.
    if row.depth > 0: Rectangle { width: row.depth * 14px; }
```
Colócalo como primer hijo del layout horizontal de la celda de nombre, antes del ícono/texto. Respeta la estructura real del archivo; si la celda de nombre no es un HorizontalLayout, adapta sumando `row.depth * 14px` al desplazamiento horizontal del texto del nombre.

- [ ] **Step 3: Compila**

```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
```
Esperado: compila. (Si algún sitio construye `RowData { ... }` sin `depth`, Slint no obliga a listar todos los campos en literales de struct desde Rust — se setean por campo; verifica que el build pase. En Rust el struct generado tendrá el campo `depth: i32`; los sitios que crean filas vía `RowData { ..Default::default() }` o asignando campos seguirán compilando. Si hay un literal exhaustivo en Rust que ahora falta `depth`, agrégale `depth: 0`.)

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/ui/types.slint crates/ui-slint/ui/file-panel.slint
git commit -F - <<'EOF'
feat(panel): RowData gana depth + sangria por profundidad en la fila

Campo depth en RowData (0 por defecto: sin efecto en vista normal/busqueda) y
sangria proporcional en la celda de nombre, para la vista profunda.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 5: Modo profundo en el controlador (`DeepJob` + activar/poll/cancelar)

El corazón de la integración. Sigue el patrón de `SearchJob` pero alimentando las filas normales del panel.

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs`

- [ ] **Step 1: Definir `DeepJob` y el estado del modo**

Cerca de `SearchJob` (en `workspace_ctrl.rs`), agrega:
```rust
/// Listado profundo (vista recursiva) en curso/terminado para un panel. Un solo job a la vez.
/// Worker + canal + token + entradas acumuladas, igual que SearchJob, pero las entradas se
/// vuelcan en las filas NORMALES del panel (no en un overlay): cada una lleva su profundidad.
pub struct DeepJob {
    /// Panel sobre el que está activa la vista profunda.
    pub pane: PaneId,
    /// Carpeta raíz del recorrido (la del panel al activar).
    pub root: PathBuf,
    pub rx: std::sync::mpsc::Receiver<naygo_core::deep_listing::DeepMsg>,
    pub token: naygo_core::CancellationToken,
    /// Entradas acumuladas con su profundidad (orden de descubrimiento).
    pub items: Vec<(naygo_core::fs_model::Entry, u32)>,
    pub dirs_scanned: usize,
    pub done: bool,
    pub cancelled: bool,
    pub partial: bool,
}
```
Agrega el campo en `WorkspaceCtrl`:
```rust
    /// Vista profunda activa (None = ningún panel en modo profundo). Un solo job a la vez.
    pub deep: Option<DeepJob>,
```
e inicialízalo en `new_in` (junto a `pending_pick: None,` u otros `None`): `deep: None,`.

- [ ] **Step 2: Activar / desactivar el modo**

Agrega los métodos:
```rust
    /// ¿El panel `id` está en vista profunda ahora mismo?
    pub fn is_deep_active(&self, id: PaneId) -> bool {
        self.deep.as_ref().is_some_and(|d| d.pane == id)
    }

    /// Activa la vista profunda en el panel `id` sobre su carpeta actual. Cancela cualquier
    /// job profundo anterior. Si el panel no es Files o no tiene carpeta válida, no hace nada.
    pub fn deep_start(&mut self, id: PaneId) {
        let dir = self
            .ws
            .pane(id)
            .filter(|p| p.purpose == PanePurpose::Files)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone())
            .filter(|d| d.is_dir());
        let Some(dir) = dir else { return };
        self.deep_cancel(); // cancela el anterior si lo hubiera
        let token = naygo_core::CancellationToken::new();
        let (rx, _handle) =
            naygo_core::deep_listing::spawn_deep_listing(dir.clone(), token.clone());
        self.deep = Some(DeepJob {
            pane: id,
            root: dir,
            rx,
            token,
            items: Vec::new(),
            dirs_scanned: 0,
            done: false,
            cancelled: false,
            partial: false,
        });
    }

    /// Apaga la vista profunda: cancela el worker y vuelve el panel a su listado normal.
    pub fn deep_cancel(&mut self) {
        if let Some(d) = self.deep.take() {
            d.token.cancel();
            // Repoblar el panel con su listado normal (relanza el listing de su carpeta).
            self.relist_pane(d.pane);
        }
    }

    /// Alterna la vista profunda en el panel activo (para el toggle de la barra).
    pub fn deep_toggle(&mut self, id: PaneId) {
        if self.is_deep_active(id) {
            self.deep_cancel();
        } else {
            self.deep_start(id);
        }
    }
```
> `relist_pane(id)` debe ser el método que ya relanza el listado normal de un panel (búscalo:
> probablemente exista como `relist`, `start_listing`, `refresh_pane` o similar). Si el nombre
> difiere, usa el real. Si no existe uno reutilizable, repoblar = relanzar el `Listing` de la
> carpeta actual del panel como ya se hace al navegar/refrescar.

- [ ] **Step 3: Poll del job (drenar el canal a las filas del panel)**

Agrega el método que main.rs llamará cada frame mientras `deep` esté activo:
```rust
    /// Drena los mensajes del worker profundo hacia las entradas acumuladas. Devuelve `true`
    /// si hubo cambios (la UI debe re-sincronizar las filas del panel). No bloquea.
    pub fn deep_poll(&mut self) -> bool {
        let Some(d) = self.deep.as_mut() else {
            return false;
        };
        let mut changed = false;
        loop {
            match d.rx.try_recv() {
                Ok(naygo_core::deep_listing::DeepMsg::Entry(de)) => {
                    d.items.push((de.entry, de.depth));
                    changed = true;
                }
                Ok(naygo_core::deep_listing::DeepMsg::Progress { dirs_scanned }) => {
                    d.dirs_scanned = dirs_scanned;
                }
                Ok(naygo_core::deep_listing::DeepMsg::Done { partial }) => {
                    d.done = true;
                    d.partial = partial;
                    changed = true;
                    break;
                }
                Ok(naygo_core::deep_listing::DeepMsg::Cancelled) => {
                    d.cancelled = true;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    d.done = true;
                    break;
                }
            }
        }
        changed
    }

    /// Las entradas profundas acumuladas (entrada + profundidad), para que main.rs arme las
    /// filas con sangría. Vacío si no hay vista profunda activa.
    pub fn deep_items(&self) -> &[(naygo_core::fs_model::Entry, u32)] {
        self.deep.as_ref().map(|d| d.items.as_slice()).unwrap_or(&[])
    }
```

- [ ] **Step 4: Cancelar la vista profunda al navegar (no pegajoso)**

Localiza el método que el panel usa para navegar a una carpeta (p. ej. `navigate`, `open_dir`, `set_dir` del panel activo). Al inicio de esa navegación, si el panel está en modo profundo, apágalo:
```rust
        // La vista profunda no es pegajosa: navegar vuelve al listado normal.
        if self.is_deep_active(id) {
            self.deep = None; // el listado normal se relanza igual por la navegación
        }
```
> Ajusta `id` a la variable real del panel que navega. Importante: aquí basta con soltar el
> job (`self.deep = None`) porque la navegación ya va a relanzar el listado normal; cancela el
> token primero si quieres ser explícito: `if let Some(d)=self.deep.take(){ d.token.cancel(); }`.

- [ ] **Step 5: Tests del controlador (sin FS real, sobre un árbol temporal)**

Agrega a los tests de `workspace_ctrl.rs` un test que use `tempfile` (ya es dev-dependency) para crear un árbol y verificar el ciclo activar→poll→items. Patrón:
```rust
    #[test]
    fn vista_profunda_activa_acumula_y_cancela() {
        use std::fs;
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("a.txt"), b"x").unwrap();
        fs::write(root.join("sub/b.txt"), b"y").unwrap();

        let cfg = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(root.to_path_buf(), cfg.path().to_path_buf());
        let id = c.ws.active().expect("hay panel activo");

        c.deep_start(id);
        assert!(c.is_deep_active(id));
        // Drenar hasta que el worker termine (con un límite de iteraciones para no colgar).
        let mut tries = 0;
        while !c.deep.as_ref().map(|d| d.done || d.cancelled).unwrap_or(true) && tries < 1000 {
            c.deep_poll();
            std::thread::sleep(std::time::Duration::from_millis(2));
            tries += 1;
        }
        c.deep_poll();
        // Debe haber acumulado a.txt, sub y sub/b.txt = 3 entradas.
        assert_eq!(c.deep_items().len(), 3);

        c.deep_cancel();
        assert!(!c.is_deep_active(id));
        assert!(c.deep_items().is_empty());
    }
```
> Ajusta `c.ws.active()` al método real para obtener el `PaneId` activo (mira otros tests del
> archivo, p. ej. los que usan `open_context_menu`). Si `active()` devuelve `Option<PaneId>` u
> otra forma, adáptalo. Si `WorkspaceCtrl::new_in` arranca un listado asíncrono que interfiere,
> el test igual debe poder activar deep y drenar; si hay flakiness por timing, sube el límite.

- [ ] **Step 6: Tests + gate**

```
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-ui-slint
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all
$env:CARGO_BUILD_JOBS = "2"; cargo clippy -p naygo-ui-slint --all-targets -- -D warnings
```
Esperado: tests PASS, clippy limpio. Si el test de timing es inestable, conviértelo en uno que llame `deep_poll` en bucle corto como arriba (ya lo hace) o márcalo `#[ignore]` con nota — pero intenta primero que pase de forma estable.

- [ ] **Step 7: Commit**

```bash
git add crates/ui-slint/src/workspace_ctrl.rs
git commit -F - <<'EOF'
feat(panel): modo vista profunda en el controlador (DeepJob)

deep_start/deep_cancel/deep_toggle/deep_poll/deep_items + is_deep_active:
activan el listado recursivo sobre la carpeta del panel, acumulan entradas con
profundidad por streaming, y cancelan al apagar o navegar (no pegajoso). Con
test de ciclo activar->poll->cancelar sobre un arbol temporal.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Task 6: Toggle en la barra del panel + wiring en main.rs

**Files:**
- Modify: `crates/ui-slint/ui/file-panel.slint` (botón toggle)
- Modify: `crates/ui-slint/src/main.rs` (callback + poll + armar filas con depth)

- [ ] **Step 1: Callback y propiedad en `file-panel.slint`**

En el componente del panel de archivos, agrega un callback y una propiedad de estado:
```slint
    // Vista profunda: estado on/off (lo setea Rust) y solicitud de alternar (pane-id).
    in property <bool> deep-active: false;
    callback toggle-deep(int);   // (pane-id)
```
En la barra del panel (junto a los otros botones de la barra: la lupa de búsqueda, etc.), agrega un botón toggle con ícono dibujado con `Path` (NUNCA glifo de fuente). El ícono: representa "árbol recursivo / capas" — p. ej. tres líneas horizontales escalonadas (sangría). Estructura:
```slint
    // Botón de vista profunda (recursiva). Fondo acento cuando está activa.
    Rectangle {
        width: 26px;
        height: 22px;
        border-radius: 4px;
        background: deep-touch.has-hover || root.deep-active ? Theme.accent : transparent;
        Path {
            width: 14px;
            height: 14px;
            x: (parent.width - self.width) / 2;
            y: (parent.height - self.height) / 2;
            stroke: (deep-touch.has-hover || root.deep-active) ? white : Theme.text-dim;
            stroke-width: 1.3px;
            fill: transparent;
            viewbox-width: 14;
            viewbox-height: 14;
            // Tres niveles escalonados (sugiere jerarquía con sangría).
            MoveTo { x: 1; y: 3; }   LineTo { x: 9; y: 3; }
            MoveTo { x: 4; y: 7; }   LineTo { x: 12; y: 7; }
            MoveTo { x: 7; y: 11; }  LineTo { x: 13; y: 11; }
        }
        deep-touch := TouchArea {
            mouse-cursor: pointer;
            clicked => { root.toggle-deep(root.pane-id); }
            // Tooltip via el sistema de hover-tip del panel si existe; si no, omitir.
        }
    }
```
> Ubícalo junto a los demás controles de la barra del panel. Usa `root.pane-id` (el panel ya
> conoce su id; mira cómo otros callbacks del panel pasan el pane-id, p. ej. `copy-path(int)`).
> Si el panel expone un mecanismo de tooltip (hover-tip), conéctale `Tr.deep-view-tip`; si no,
> deja el botón sin tooltip (no es bloqueante).

- [ ] **Step 2: Wiring del callback en main.rs**

Donde se cablean los callbacks del panel de archivos (busca `on_copy_path`, `on_toggle_...` u otros `on_*` del FilePanel), agrega:
```rust
        // Toggle de vista profunda: alterna el modo en el panel y resincroniza.
        {
            let ctrl = ctrl.clone();
            let ui_weak = ui.as_weak();
            ui.on_toggle_deep(move |pane_id| {
                ctrl.borrow_mut().deep_toggle(PaneId(pane_id as usize));
                if let Some(ui) = ui_weak.upgrade() {
                    // Resincroniza filas + estado del botón (helper que ya actualiza el panel).
                    sync_rows(&ctrl, &ui);
                }
            });
        }
```
> Ajusta: el constructor de `PaneId` (mira cómo otros callbacks convierten el `int` del panel a
> `PaneId` — puede ser `PaneId(x as usize)` o un helper). `sync_rows` es el nombre placeholder
> del helper que ya re-vuelca las filas del panel desde el controlador; usa el real (en main.rs
> ya existe la rutina que arma las filas `RowData` de cada panel — esa misma debe, cuando el
> panel está en deep, tomar `deep_items()` en vez del listado normal; ver Step 3).

- [ ] **Step 3: Armar las filas profundas con `depth` + poll cada frame**

(a) En la rutina de main.rs que arma las `RowData` de un panel (la que hoy toma el listado normal o los resultados), cuando el panel esté en vista profunda usa las entradas profundas. Pseudo-patrón a integrar en el sitio real:
```rust
        // Si este panel está en vista profunda, sus filas vienen del DeepJob (entrada+depth),
        // no del listado normal.
        let deep_rows: Option<Vec<RowData>> = if ctrl.is_deep_active(pane_id) {
            Some(
                ctrl.deep_items()
                    .iter()
                    .map(|(entry, depth)| {
                        // Reusa el mismo armado de RowData que el listado normal (nombre,
                        // detalle, ícono…) y además setea depth = *depth as i32.
                        let mut row = row_from_entry(entry); // helper existente para una fila
                        row.depth = *depth as i32;
                        row
                    })
                    .collect(),
            )
        } else {
            None
        };
```
Aplica las `deep_rows` al `VecModel` de filas del panel cuando existan; si no, el flujo normal. `row_from_entry` es el nombre placeholder del helper que ya convierte un `Entry` en `RowData` (úsalo real; si el armado está inline, factorízalo o replica los campos y añade `depth`). El orden/filtro de columnas se aplica igual que al listado normal (las filas profundas pasan por el mismo `sort`/filtro de la tabla).

(b) En el tick/timer de main.rs que ya hace polling de jobs (búsqueda/tamaño/listado — busca dónde se llama `poll` o se drenan canales cada frame), agrega el poll del deep:
```rust
            // Vista profunda: drenar entradas nuevas y re-sincronizar si las hubo.
            if ctrl.borrow_mut().deep_poll() {
                sync_rows(&ctrl, &ui);
            }
```
(c) Setea `deep-active` del panel cuando se sincroniza (para el estado on/off del botón): donde se actualizan las propiedades del FilePanel, `panel.set_deep_active(ctrl.is_deep_active(pane_id));`.

> Estos tres puntos se integran en sitios que YA existen en main.rs (armado de filas, tick de
> polling, set de propiedades del panel). El implementador debe localizarlos (grep por cómo se
> llenan las filas y por el timer de poll) y enganchar ahí, NO crear un sistema nuevo.

- [ ] **Step 4: Ruteo de doble clic (carpeta sale del modo y navega; archivo abre)**

El doble clic ya está implementado (abre archivo / navega carpeta). Verifica que, estando en vista profunda, el doble clic sobre una fila use `entry.path` (absoluto) — que ya es lo que hace. Como `deep_cancel`/navegar ya apaga el modo al navegar a una carpeta (Task 5 Step 4), el comportamiento "carpeta sale del modo y navega" sale solo. Solo CONFIRMA en el código que la acción de doble clic toma el path de la fila seleccionada (no un índice del listado normal que en deep no aplicaría). Si el doble clic resuelve la fila por índice contra el listado normal del panel, ajústalo para que en modo deep resuelva contra `deep_items()` (mismo índice de vista). Documenta lo que encuentres en el reporte.

- [ ] **Step 5: Compila + gate completo**

```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all
$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform
$env:CARGO_BUILD_JOBS = "2"; cargo clippy --workspace --all-targets -- -D warnings
```
Esperado: compila, tests PASS, clippy sin warnings.

- [ ] **Step 6: Verificación visual (Nicolás)**

Compilar release y verificar a ojo: el botón toggle aparece en la barra del panel; al activarlo, el panel muestra el árbol completo con sangría; las filas profundas se ordenan/filtran; doble clic en carpeta navega (y apaga el modo), en archivo abre; Esc o re-clic del toggle cancela. Este paso es visto bueno visual de Nicolás.
```
$env:CARGO_BUILD_JOBS = "2"; cargo build --release -p naygo-ui-slint
```

- [ ] **Step 7: Commit**

```bash
git add crates/ui-slint/ui/file-panel.slint crates/ui-slint/src/main.rs
git commit -F - <<'EOF'
feat(panel): toggle de vista profunda en la barra + wiring

Boton toggle (icono Path) que alterna la vista profunda; main.rs poll-ea el
DeepJob cada frame, arma las filas con sangria por profundidad y refleja el
estado on/off. Doble clic usa el path absoluto de la fila (carpeta navega y
apaga el modo; archivo abre).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

## Verificación final del bloque

- [ ] Gate completo verde:
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt --all; cargo test -p naygo-core -p naygo-ui-slint -p naygo-platform; cargo clippy --workspace --all-targets -- -D warnings
```
- [ ] Sin voseo en lo nuevo: `rg -ni "\b(arrastrá|escribí|elegí|podés|tenés|querés|hacé|volvé)\b"` en los archivos tocados → vacío.
- [ ] `graphify update .`.
- [ ] Documentar la vista profunda en `docs/GUIA-DE-USUARIO.md` (qué es, cómo se activa, que es cancelable) y agregar una línea a la sección "Sin publicar" del `CHANGELOG.md`.
- [ ] Visto bueno visual de Nicolás (Task 6 Step 6).

## Notas de cierre

- Al terminar, actualizar memoria de proyecto con el estado de la vista profunda.
- Sugerir a Nicolás: posibles mejoras futuras (profundidad configurable, tope de seguridad) que quedaron fuera de alcance, por si las quiere más adelante.
```
