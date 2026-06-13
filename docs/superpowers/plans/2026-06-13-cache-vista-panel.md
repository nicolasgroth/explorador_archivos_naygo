# Caché de la vista del panel — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminar el recálculo por frame de la vista del panel (clonar+filtrar+ordenar
TODAS las entries) cacheando los índices filtrados+ordenados en `core::FilePaneState` y
reusándolos entre frames; el panel deja de clonar la lista completa cada frame.

**Architecture:** El caché vive en `FilePaneState` (core, testeable). `view_indices()`
sigue siendo `&self`: usa un `RefCell<Option<ViewCache>>` para recompute perezoso. La
invalidación es por una FIRMA O(1) de los inputs (len de entries, sort, filtros,
group_new_at_end, len de highlighted) comparada en cada llamada — robusta contra sitios
de mutación omitidos, sin tocar los ~10 sitios de la UI que mutan campos públicos. La UI
(`file_panel::show`) deja de construir `view: Vec<Entry>` y lee `&entries[idx]` por índice.

**Tech Stack:** Rust, egui/egui_extras 0.34.3, std `RefCell`.

---

## Reglas operativas (NO te las saltes)

1. **Puertas antes de CADA commit**: `cargo test --workspace` (lee TODAS las líneas
   `test result:`), `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo fmt --all`.
2. **`cargo build -p naygo-ui`** antes de cualquier prueba en vivo; **mata `naygo.exe`**
   antes de compilar (`Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue`).
3. **Commits en español** con heredoc de Bash (`git commit -F - <<'EOF' … EOF`).
   **Stagea rutas explícitas**, NO `git add -A` (hay un cambio ajeno en `CLAUDE.md`).
4. Header de copyright en archivos nuevos; comentarios en español del PORQUÉ.

## Estructura de archivos

- `crates/core/src/workspace/file_pane.rs` — `ViewCache`, firma, caché en
  `view_indices`, `Clone` manual, tests. (ÚNICO archivo de core que cambia.)
- `crates/ui/src/panes/file_panel.rs` — `show` usa índices cacheados en vez de clonar
  `entries` y reordenar cada frame.

---

### Tarea 1 — core: caché de la vista en FilePaneState

**Files:**
- Modify: `crates/core/src/workspace/file_pane.rs`

- [ ] **Step 1: Escribir el test del caché (recompute solo al cambiar un input)**

En el `mod tests` de `crates/core/src/workspace/file_pane.rs`, agregar (usa el helper
`pane_n` ya existente, que crea N entries `f0.txt..fN.txt`):

```rust
    #[test]
    fn view_cache_reusa_y_se_invalida() {
        let mut p = pane_n(50);
        // 1ª llamada: calcula. 2ª sin mutar: mismo resultado y NO recalcula.
        let a = p.view_indices();
        assert_eq!(a.len(), 50);
        assert_eq!(p.view_recomputes_for_test(), 1, "una sola vez");
        let _b = p.view_indices();
        assert_eq!(p.view_recomputes_for_test(), 1, "2ª llamada sirve del caché");

        // Cambiar el sort invalida (la firma incluye el sort).
        p.sort = crate::fs_model::SortSpec {
            key: crate::fs_model::SortKey::Name,
            ascending: false,
        };
        let _c = p.view_indices();
        assert_eq!(p.view_recomputes_for_test(), 2, "el cambio de sort recalcula");

        // Agregar una entry (cambia el len) invalida.
        p.entries.push(crate::fs_model::Entry {
            name: "zzz.txt".into(),
            path: std::path::PathBuf::from("C:/zzz.txt"),
            kind: crate::fs_model::EntryKind::File,
            size: Some(1),
            modified: None,
            created: None,
            hidden: false,
        });
        let d = p.view_indices();
        assert_eq!(d.len(), 51);
        assert_eq!(p.view_recomputes_for_test(), 3, "agregar entry recalcula");
    }

    #[test]
    fn view_cache_invalida_por_filtro_y_group_flag() {
        use crate::columns::ColumnKind;
        use crate::filter::ColumnFilter;
        let mut p = pane_n(10);
        let _ = p.view_indices();
        let base = p.view_recomputes_for_test();
        // Cambiar un filtro invalida.
        p.table.set_filter(
            ColumnKind::Name,
            ColumnFilter::Text { contains: "f1".into(), case_sensitive: false },
        );
        let _ = p.view_indices();
        assert_eq!(p.view_recomputes_for_test(), base + 1);
        // Toggle group_new_at_end invalida.
        p.group_new_at_end = !p.group_new_at_end;
        let _ = p.view_indices();
        assert_eq!(p.view_recomputes_for_test(), base + 2);
    }

    #[test]
    fn clone_no_arrastra_cache_viejo() {
        let mut p = pane_n(5);
        let _ = p.view_indices(); // llena el caché del original
        let mut c = p.clone();
        // Mutar el clon y pedir su vista: debe reflejar SU estado, no el caché del original.
        c.entries.clear();
        assert_eq!(c.view_indices().len(), 0);
        // El original sigue intacto.
        assert_eq!(p.view_indices().len(), 5);
    }
```

- [ ] **Step 2: Run test para verificar que falla (símbolos no existen)**

Run (PowerShell): `cargo test -p naygo-core view_cache 2>&1 | Select-String "error|test result"`
Expected: FALLA de compilación (`view_recomputes_for_test` no existe; `FilePaneState`
no es `Clone` manual aún).

- [ ] **Step 3: Agregar los campos del caché al struct + quitar el derive(Clone)**

En `crates/core/src/workspace/file_pane.rs`, el struct hoy es `#[derive(Clone, Debug)]`.
El caché (`RefCell`) NO debe clonarse (cada panel reconstruye el suyo), así que se
implementa `Clone` a mano. Cambiar la línea del derive y agregar los campos:

```rust
/// Estado de un panel de archivos. Lo serializable se persiste; `entries` no
/// (se re-lista al abrir) y `history` tampoco (arranca limpio cada sesión).
#[derive(Debug)]
pub struct FilePaneState {
    pub current_dir: PathBuf,
    pub entries: Vec<Entry>,
    pub sort: SortSpec,
    pub view: ViewMode,
    pub focused: Option<usize>,
    pub selected: Vec<usize>,
    /// Ancla de la selección por rango (Shift). Efímero, NO se persiste.
    pub anchor: Option<usize>,
    pub history: NavHistory,
    /// Si es `false`, el panel oculta las carpetas (muestra solo archivos).
    pub show_dirs: bool,
    /// Estado de tabla: columnas (orden/visibilidad/ancho) + filtros por columna.
    pub table: TableState,
    /// Rutas resaltadas como "recién aparecidas" (estado de presentación efímero; NO se
    /// persiste). El render las tiñe; la interacción/refresh las limpia.
    pub highlighted: std::collections::HashSet<std::path::PathBuf>,
    /// Espejo runtime del setting "archivos nuevos al final" (lo setea la UI). NO se
    /// persiste (vive en settings.json); existe para que `view_indices` agrupe IGUAL
    /// que el render y los índices nunca se desalineen.
    pub group_new_at_end: bool,
    /// Caché de los índices de vista (filtrados+ordenados). Recompute PEREZOSO bajo
    /// `&self` vía `RefCell`; se invalida comparando una firma O(1) de los inputs. NO se
    /// clona (cada panel reconstruye el suyo) ni se persiste. Efímero de presentación.
    view_cache: std::cell::RefCell<Option<ViewCache>>,
    /// Contador de recomputes de la vista (solo para tests; mide aciertos del caché).
    #[cfg(test)]
    view_recomputes: std::cell::Cell<u32>,
}

/// Caché de la vista: la firma de los inputs con que se calculó + los índices resultantes.
#[derive(Debug)]
struct ViewCache {
    signature: u64,
    indices: Vec<usize>,
}
```

- [ ] **Step 4: Implementar Clone manual (caché vacío en el clon)**

Agregar tras el `struct ViewCache` (o donde calce en el archivo):

```rust
impl Clone for FilePaneState {
    fn clone(&self) -> Self {
        FilePaneState {
            current_dir: self.current_dir.clone(),
            entries: self.entries.clone(),
            sort: self.sort,
            view: self.view,
            focused: self.focused,
            selected: self.selected.clone(),
            anchor: self.anchor,
            history: self.history.clone(),
            show_dirs: self.show_dirs,
            table: self.table.clone(),
            highlighted: self.highlighted.clone(),
            group_new_at_end: self.group_new_at_end,
            // El caché NO se arrastra: el clon lo reconstruye a demanda.
            view_cache: std::cell::RefCell::new(None),
            #[cfg(test)]
            view_recomputes: std::cell::Cell::new(0),
        }
    }
}
```

- [ ] **Step 5: Inicializar los campos nuevos en `new()`**

En `FilePaneState::new`, agregar al literal del struct (tras `group_new_at_end: false,`):

```rust
            view_cache: std::cell::RefCell::new(None),
            #[cfg(test)]
            view_recomputes: std::cell::Cell::new(0),
```

- [ ] **Step 6: Implementar la firma O(1) y el recompute cacheado**

Reemplazar el método `view_indices` (hoy delega a `view_indices_ordered`) por la versión
cacheada, y renombrar el cálculo crudo a `compute_view_indices`. La firma captura todo lo
que cambia la vista SIN iterar las entries (O(1)):

```rust
    /// Firma O(1) de los inputs que determinan la vista. Si no cambia entre llamadas, el
    /// caché es válido. Captura: nº de entries, sort, filtros (su hash), agrupar-al-final,
    /// y nº de resaltadas (solo afecta el orden si `group_new_at_end`). No itera entries.
    fn view_signature(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.entries.len().hash(&mut h);
        // SortSpec es Copy + Hash si deriva Hash; si no, hashear sus campos.
        self.sort.key.hash(&mut h);
        self.sort.ascending.hash(&mut h);
        // Filtros: el BTreeMap es ordenado, así que su hash es estable.
        for (k, v) in &self.table.filters {
            k.hash(&mut h);
            v.hash(&mut h);
        }
        self.group_new_at_end.hash(&mut h);
        // El conjunto de resaltadas solo cambia el orden si se agrupan al final.
        if self.group_new_at_end {
            self.highlighted.len().hash(&mut h);
        }
        h.finish()
    }

    /// Las entries VISIBLES (índices en `entries`): filtradas y ordenadas. CACHEADO: si la
    /// firma de los inputs no cambió desde el último cálculo, devuelve el caché (clon del
    /// Vec, barato). Es el único espacio de índices que usan foco/selección/teclado/render.
    pub fn view_indices(&self) -> Vec<usize> {
        let sig = self.view_signature();
        // ¿Caché válido? (misma firma). Si sí, servirlo.
        if let Some(c) = self.view_cache.borrow().as_ref() {
            if c.signature == sig {
                return c.indices.clone();
            }
        }
        // Recalcular, guardar y devolver.
        let indices = self.compute_view_indices(self.group_new_at_end);
        #[cfg(test)]
        self.view_recomputes.set(self.view_recomputes.get() + 1);
        *self.view_cache.borrow_mut() = Some(ViewCache {
            signature: sig,
            indices: indices.clone(),
        });
        indices
    }

    /// Acceso de TEST al contador de recomputes (cuántas veces se recalculó la vista).
    #[cfg(test)]
    pub fn view_recomputes_for_test(&self) -> u32 {
        self.view_recomputes.get()
    }
```

Y renombrar el método de cálculo crudo: cambiar la firma de `view_indices_ordered` a
`compute_view_indices` (mismo cuerpo). Localizar:

```rust
    pub fn view_indices_ordered(&self, new_items_at_end: bool) -> Vec<usize> {
```
cambiar a (privado, el cuerpo NO cambia):

```rust
    /// Cálculo CRUDO de los índices de vista (filtrar → ordenar → agrupar-al-final).
    /// No cachea; lo invoca `view_indices`. El cuerpo es el cálculo original.
    fn compute_view_indices(&self, new_items_at_end: bool) -> Vec<usize> {
```

- [ ] **Step 7: Revisar usos de `view_indices_ordered` fuera de file_pane.rs**

Run (PowerShell): `Select-String -Path crates\ui\src\*.rs,crates\ui\src\**\*.rs -Pattern "view_indices_ordered"`
Expected: probablemente solo el test `view_al_final_mueve_resaltadas_al_fondo` en
file_pane.rs lo usa directamente. Si algún test lo llama, cambiar esa llamada a
`compute_view_indices` (sigue existiendo, ahora privado → accesible desde el `mod tests`
del mismo archivo). Si la UI lo usaba, reemplazar por `view_indices()` (ya agrupa por
`group_new_at_end`). Verifica que NO quede ningún uso externo roto.

- [ ] **Step 8: Verificar SortKey/ColumnFilter implementan Hash**

La firma hashea `sort.key`, `sort.ascending`, y los `ColumnFilter`. Verificar que esos
tipos derivan `Hash`:

Run (PowerShell): `Select-String -Path crates\core\src\fs_model.rs -Pattern "enum SortKey" -Context 0,1; Select-String -Path crates\core\src\filter.rs -Pattern "enum ColumnFilter" -Context 0,1; Select-String -Path crates\core\src\columns.rs -Pattern "enum ColumnKind" -Context 0,1`
Expected: ver sus `#[derive(...)]`. Si a `SortKey`, `ColumnKind` o `ColumnFilter` les
FALTA `Hash`, agregar `Hash` al derive de ese enum (es aditivo, sin romper nada). Para
`ColumnFilter` que puede contener `f32`/`BTreeSet<String>`: si tiene un `f32` no es
`Hash`-able directamente — en ese caso, NO hashear el filtro completo: hashear solo la
COMBINACIÓN de columnas filtradas y un discriminante, así:

```rust
        // (Reemplazo del bucle de filtros si ColumnFilter no es Hash:)
        for (k, v) in &self.table.filters {
            k.hash(&mut h);
            std::mem::discriminant(v).hash(&mut h);
            // Texto del filtro (lo que el usuario tipea) si aplica:
            if let crate::filter::ColumnFilter::Text { contains, .. } = v {
                contains.hash(&mut h);
            }
        }
```

Elegir la rama que compile según el `ColumnFilter` real. (Si deriva `Hash` limpio, usar
el bucle del Step 6; si no, este.)

- [ ] **Step 9: Run tests de core**

Run (PowerShell): `cargo test -p naygo-core 2>&1 | Select-String "test result|FAILED|error\["`
Expected: todas `ok`, incluidos los 3 nuevos tests de caché y los existentes de
`file_pane` (`view_indices`, `view_al_final_...`).

- [ ] **Step 10: clippy + fmt + commit**

Run (PowerShell): `cargo clippy --workspace --all-targets -- -D warnings; cargo fmt --all`
Expected: clippy limpio.

```bash
git add crates/core/src/workspace/file_pane.rs
git commit -F - <<'EOF'
perf(core): cachear los indices de vista del panel (recompute solo al cambiar inputs)

view_indices() cachea el Vec de indices filtrados+ordenados; se recalcula solo cuando
cambia una firma O(1) de los inputs (len de entries, sort, filtros, group_new_at_end,
len de highlighted). RefCell para recompute perezoso bajo &self; Clone manual no
arrastra el cache. Antes se recalculaba en cada llamada (cada frame desde el panel).
Tests: el cache se reusa, se invalida por sort/entries/filtro/group, y el clon no
hereda cache viejo.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 2 — ui: file_panel usa índices cacheados (no clona ni reordena por frame)

**Files:**
- Modify: `crates/ui/src/panes/file_panel.rs`

**Diseño:** hoy `show` construye `let mut view: Vec<Entry>` clonando `all_entries` y
reordenando cada frame, y luego indexa `view[i]`. El cambio: tomar `view_idx =
f.view_indices()` (cacheado, `Vec<usize>` en `entries`), clonar UNA vez el slice de
entries para el closure (necesario por el borrow de `&mut workspace` en el `TableBuilder`),
y mapear las posiciones de vista a entries por índice. La clave: NO reordenar, NO
recalcular filtros por frame — eso ya vino hecho y cacheado de core.

- [ ] **Step 1: Reemplazar el pipeline por frame por los índices cacheados**

En `crates/ui/src/panes/file_panel.rs`, localizar el bloque que hoy construye la vista
(tras clonar `all_entries`):

```rust
    let all_entries: Vec<Entry> = f.entries.clone();
    let highlighted: std::collections::HashSet<std::path::PathBuf> = f.highlighted.clone();

    // Conteo de extensiones sobre TODAS las entries actuales (no las filtradas):
    // así el menú de filtro de tipo muestra todas las opciones disponibles.
    let ext_counts = naygo_core::filter::extension_counts(&all_entries);

    // Pipeline en memoria: filtrar (solo si hay filtros) → ordenar. No muta el
    // estado del panel.
    let mut view: Vec<Entry> = if table.filters.is_empty() {
        all_entries.clone()
    } else {
        all_entries
            .iter()
            .filter(|e| naygo_core::filter::matches(e, &table.filters))
            .cloned()
            .collect()
    };
    naygo_core::sort::sort_entries(&mut view, &sort);

    // Modo "al final": agrupar las entries resaltadas (nuevas) al final, estable.
    // `sort_by_key` con bool es estable → primero las no resaltadas (false), luego las
    // resaltadas (true), sin alterar el orden relativo dentro de cada grupo.
    if new_items_at_end && !highlighted.is_empty() {
        view.sort_by_key(|e| highlighted.contains(&e.path));
    }
```

Reemplazarlo por (usa los índices cacheados de core; `view` pasa a ser `Vec<Entry>`
construido por índice — UNA clonada, sin filtrar ni ordenar aquí):

```rust
    let highlighted: std::collections::HashSet<std::path::PathBuf> = f.highlighted.clone();

    // Índices de la vista (filtrados+ordenados) YA cacheados por core: no se recalcula
    // ni se reordena por frame (era la causa del consumo alto en carpetas grandes). El
    // `group_new_at_end` ya lo aplicó core dentro de `view_indices`.
    let view_idx = f.view_indices();
    // La `view` que el resto de `show` consume (por valor, para el closure de `body.rows`
    // que corre con `&mut workspace` prestado): se materializa clonando SOLO las entries
    // visibles, en su orden final. Para carpetas enormes esto sigue siendo un clon, pero
    // ocurre una vez por frame PINTADO y sin reordenar; egui_extras virtualiza las filas.
    let view: Vec<Entry> = view_idx
        .iter()
        .filter_map(|&real| f.entries.get(real).cloned())
        .collect();

    // Conteo de extensiones para el menú de filtro de tipo: se calcula sobre TODAS las
    // entries, pero SOLO cuando hace falta (al pintar; el menú lo usa si se abre). Es
    // O(n) pero sin allocaciones por entry. (Se mantiene como antes para no cambiar el
    // menú de columna; si se vuelve un costo, se cachea aparte.)
    let ext_counts = naygo_core::filter::extension_counts(&f.entries);

    // `new_items_at_end` ya fue aplicado por core en `view_indices`; aquí no se reordena.
    let _ = new_items_at_end;
```

NOTA: tras este cambio, `view` ya viene en el orden final (filtrado+ordenado+agrupado),
así que el resto de `show` que indexa `view[i]` / `view.get(i)` y empuja a `rows` SIGUE
IGUAL — `i` sigue siendo "posición en la vista", consistente con los índices de
foco/selección de core (que vienen del mismo `view_indices`). No tocar el resto.

- [ ] **Step 2: Quitar el `sort`/`table` ya no usados para reordenar (si quedan warnings)**

Tras el Step 1, las variables `sort` y `table` quizá sigan usándose (el header de
columnas usa `sort` y `table`). Verificar con el compilador. Si `sort` queda sin uso,
NO borrar su captura si el header la necesita; el build dirá si sobra. Mantener
`let table = f.table.clone();` y `let sort = f.sort;` si el resto los usa (encabezados,
menús). Solo se elimina el RE-ORDEN por frame, no las lecturas.

- [ ] **Step 3: Build**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo build -p naygo-ui 2>&1 | Select-String "error|warning:|Finished"`
Expected: `Finished`. Si hay warning de variable sin usar (`new_items_at_end`, `sort`),
resolver: si `sort` se sigue usando en el header, dejarlo; si `new_items_at_end` queda
sin uso real, quitar el parámetro NO (rompería la firma); el `let _ = new_items_at_end;`
del Step 1 lo silencia.

- [ ] **Step 4: Puertas completas**

Run (PowerShell): `cargo test --workspace 2>&1 | Select-String "test result:|FAILED"; cargo clippy --workspace --all-targets -- -D warnings 2>&1 | Select-String "error|warning:|Finished" | Select-Object -Last 2; cargo fmt --all`
Expected: tests `ok`, clippy limpio.

- [ ] **Step 5: Verificación en vivo (local) — sin regresión visual**

Run (PowerShell): `cargo build -p naygo-ui; .\target\debug\naygo.exe "C:\Windows\System32"`
Criterio: la carpeta lista normal, el orden de columnas funciona (clic en encabezado
reordena), el filtro de tipo filtra, el foco/selección por teclado funciona. (El efecto
de RENDIMIENTO se mide en la VM de Nicolás; aquí solo se valida que NO hay regresión
funcional ni visual.) Cerrar la app.

- [ ] **Step 6: Commit**

```bash
git add crates/ui/src/panes/file_panel.rs
git commit -F - <<'EOF'
perf(ui): el panel usa los indices de vista cacheados (no clona+ordena por frame)

file_panel::show ya no clona TODAS las entries ni las reordena en cada frame: pide los
indices a core (view_indices, cacheados) y materializa solo la vista por indice. En
carpetas grandes esto elimina cientos de miles de allocaciones+comparaciones por
segundo durante hover/resize/listado (la causa real del consumo alto en VM sin GPU).
Sin cambio funcional ni visual; foco/seleccion/orden/filtro intactos.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
```

---

### Tarea 3 — Cierre

- [ ] **Step 1: Pasada final de puertas**

Run (PowerShell): `Stop-Process -Name naygo -Force -ErrorAction SilentlyContinue; cargo fmt --all --check; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings; cargo build -p naygo-ui`
Expected: fmt sin diff, todas las líneas `test result: ok`, clippy limpio, build `Finished`.

- [ ] **Step 2: Regenerar distribución**

Run (PowerShell): `powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1`
y luego: `& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" /DMyAppVersion=0.1.0 installer\naygo.iss`
Verifica timestamps frescos en `dist\`.

- [ ] **Step 3: Avisar a Nicolás + memoria + autorización**

Resumen para Nicolás: en la VM, hacer clic en un directorio / arrastrar el borde de una
columna en carpetas grandes ya NO debe disparar el consumo alto (se eliminó el recompute
por frame). Actualizar la memoria del proyecto y pedir merge/push del branch
`modo-bajo-consumo` (que ya acumula el modo de bajo consumo + este caché).

---

## Autoevaluación del plan (hecha)

- Cubre el spec: caché en core con firma O(1) [T1], invalidación por sort/entries/
  filtro/group [T1 tests], Clone sin arrastrar caché [T1], UI deja de clonar+ordenar por
  frame [T2]. `ext_counts` se mantiene como estaba (spec lo permitía; no se cachea en v1).
- Sin placeholders: código real en cada paso.
- Consistencia de tipos: `view_indices()->Vec<usize>` (sin cambio de firma pública);
  `compute_view_indices` (privado, ex-`view_indices_ordered`); `ViewCache{signature,
  indices}`; `view_recomputes_for_test` solo en `#[cfg(test)]`.
- Riesgo señalado: si `ColumnFilter` no es `Hash` (contiene f32), el Step 8 da la rama
  alternativa (discriminante + texto). El hueco teórico "rename in-place mismo len+sort"
  es transitorio y auto-corrige (lo documenta el spec); aceptable v1.
- `view_indices_ordered` → `compute_view_indices`: el Step 7 verifica que no queden usos
  externos rotos (el test `view_al_final_...` usa el nombre viejo y hay que actualizarlo).
