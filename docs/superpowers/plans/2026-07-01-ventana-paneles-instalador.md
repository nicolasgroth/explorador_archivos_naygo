# Mejoras ventana/paneles/instalador — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Divisores de paneles que mueven solo a sus vecinos (resto fijo) manteniendo responsive, ventana que recuerda tamaño/posición/maximizado, la X que esconde a bandeja por defecto, autostart que puede arrancar minimizado, e instalador multiidioma con selección de idioma de la app; más extras UX (doble-clic 50/50, cursor de resize, restaurar layout).

**Architecture:** El modelo de layout en `naygo-core` pasa de un split binario con `fraction` a un split N-ario con pesos relativos (reparto local entre vecinos + proporcional al redimensionar). La geometría de ventana y los nuevos flags viven en `Settings` (settings.json) con migración de `CONFIG_VERSION`. Bandeja/autostart reutilizan `tray.rs` y `autostart.rs` existentes. El instalador (Inno Setup) suma idiomas y un paso de idioma de la app que escribe un settings.json mínimo.

**Tech Stack:** Rust, Slint (UI), crate `windows` (Win32: registro, geometría de ventana), Inno Setup (`installer/naygo.iss`), serde_json.

**Rama:** `feat/ventana-paneles-instalador` (ya creada; el spec ya está comiteado ahí).

**Convención de commits:** cada commit termina con la firma de coautoría estándar del repo. Header de archivo Naygo (Naygo — … / Copyright / SPDX) en archivos nuevos.

**Build:** `cargo build`/`cargo test` desde la raíz. Para compilar la UI Slint usar `CARGO_BUILD_JOBS=2` si el `i-slint-compiler` crashea bajo paralelismo alto (deps SVG/PDF). Tests de core: `cargo test -p naygo-core`.

---

## Fase 1 — Modelo de divisores N-ario con pesos (core)

Reemplaza `DockNode::Split { dir, fraction, first, second }` por
`DockNode::Split { dir, children: Vec<DockNode>, weights: Vec<f32> }`. Es el cambio
de fondo del lote. Todo en `crates/core/src/workspace/layout.rs`. TDD: cada
operación con su test antes que la implementación.

### Task 1: Redefinir `DockNode::Split` N-ario y `SplitStep`

**Files:**
- Modify: `crates/core/src/workspace/layout.rs:24-64` (enum `DockNode`, `SplitStep`)

- [ ] **Step 1: Cambiar la variante `Split` y `SplitStep`**

En `layout.rs`, reemplazar la variante `Split` del enum `DockNode`:

```rust
    /// Un split: una fila (Horizontal) o columna (Vertical) de N≥2 hijos, cada uno con
    /// un peso relativo (`weights[i]`). El ancho/alto de cada hijo se reparte por
    /// `weights[i] / Σweights`. Los divisores viven entre hijos consecutivos: hay
    /// `children.len() - 1` divisores por split. Invariantes: `children.len() ==
    /// weights.len() >= 2`; `weights[i] > 0`. Un split que quedaría con un solo hijo se
    /// colapsa a ese hijo (ver `remove_in`).
    Split {
        dir: SplitDir,
        children: Vec<DockNode>,
        weights: Vec<f32>,
    },
```

Reemplazar el enum `SplitStep` (antes `First`/`Second`) por un índice de hijo:

```rust
/// Un paso en la ruta a un split anidado: por cuál hijo (índice) se baja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitStep(pub usize);
```

- [ ] **Step 2: Compilar para ver el alcance de la rotura**

Run: `cargo build -p naygo-core 2>&1 | head -40`
Expected: FALLA — múltiples errores E0026/E0027/E0559 en `place`, `handles`,
`split_area`, `set_fraction`, `fraction_at`, `split_in`, `remove_in`, `collect`,
`swap_children_in`, `subtree_contains`, y los tests. Es esperado: las siguientes
tasks los arreglan uno a uno.

- [ ] **Step 3: Commit (WIP compilable al final de la fase)**

No commitear aún: el crate no compila. Se commitea al final de la Task 8, cuando
todo el módulo compila y los tests pasan. (Este step queda como recordatorio; no
ejecutar `git commit` con el build roto.)

### Task 2: `split_area` N-ario → `child_rects`

**Files:**
- Modify: `crates/core/src/workspace/layout.rs` (reemplazar `split_area`, líneas ~433-481)
- Test: `crates/core/src/workspace/layout.rs` (módulo `tests`)

- [ ] **Step 1: Escribir el test que falla**

Añadir al módulo `tests` de `layout.rs`:

```rust
    #[test]
    fn child_rects_reparte_por_pesos_horizontal() {
        // 3 hijos con pesos 1:2:1 en 800px de ancho (barras de 4px entre medio).
        // Ancho útil = 800 - 2*4 = 792. Reparto: 198 / 396 / 198.
        let rects = child_rects(
            Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 },
            SplitDir::Horizontal,
            &[1.0, 2.0, 1.0],
        );
        assert_eq!(rects.len(), 3);
        assert!((rects[0].w - 198.0).abs() < 1.0, "1º ~198: {}", rects[0].w);
        assert!((rects[1].w - 396.0).abs() < 1.0, "2º ~396: {}", rects[1].w);
        assert!((rects[2].w - 198.0).abs() < 1.0, "3º ~198: {}", rects[2].w);
        // Sin solaparse y en orden.
        assert!(rects[1].x > rects[0].x + rects[0].w - 0.1);
        assert!(rects[2].x > rects[1].x + rects[1].w - 0.1);
        // Mismo alto para todos.
        assert!(rects.iter().all(|r| (r.h - 600.0).abs() < 0.01));
    }
```

- [ ] **Step 2: Ejecutar el test para ver que no compila/falla**

Run: `cargo test -p naygo-core child_rects_reparte_por_pesos_horizontal 2>&1 | head -20`
Expected: FALLA a compilar — `cannot find function child_rects`.

- [ ] **Step 3: Implementar `child_rects`**

Reemplazar la función `split_area` por `child_rects` en `layout.rs`:

```rust
/// Reparte `area` entre N hijos según sus pesos y la orientación, descontando la barra
/// `SPLIT_BAR` entre cada par de hijos. Devuelve un rect por hijo, en orden. Los pesos se
/// normalizan (Σ); un hijo nunca recibe menos de 1.0 px de lado útil (el render por software
/// castea f32→i32 y paniquea con geometrías degeneradas). Pura.
fn child_rects(area: Rect, dir: SplitDir, weights: &[f32]) -> Vec<Rect> {
    let n = weights.len();
    debug_assert!(n >= 2, "un split tiene ≥2 hijos");
    let sum: f32 = weights.iter().copied().filter(|w| *w > 0.0).sum();
    let sum = if sum > 0.0 { sum } else { n as f32 };
    let bars = SPLIT_BAR * (n as f32 - 1.0);
    let mut out = Vec::with_capacity(n);
    match dir {
        SplitDir::Horizontal => {
            let usable = (area.w - bars).max(n as f32); // ≥1px por hijo
            let mut x = area.x;
            for (i, w) in weights.iter().enumerate() {
                let cw = (usable * (w.max(0.0) / sum)).max(1.0);
                out.push(Rect { x, y: area.y, w: cw, h: area.h });
                x += cw;
                if i + 1 < n {
                    x += SPLIT_BAR;
                }
            }
        }
        SplitDir::Vertical => {
            let usable = (area.h - bars).max(n as f32);
            let mut y = area.y;
            for (i, w) in weights.iter().enumerate() {
                let ch = (usable * (w.max(0.0) / sum)).max(1.0);
                out.push(Rect { x: area.x, y, w: area.w, h: ch });
                y += ch;
                if i + 1 < n {
                    y += SPLIT_BAR;
                }
            }
        }
    }
    out
}
```

- [ ] **Step 4: Ejecutar el test**

Run: `cargo test -p naygo-core child_rects_reparte_por_pesos_horizontal 2>&1 | head -20`
Expected: PASA (si el resto del crate ya compilara; puede seguir sin compilar por
otras funciones — en ese caso, continuar con las tasks siguientes y correr este test
junto con los demás en la Task 8).

### Task 3: `place` / `pane_rects` N-ario

**Files:**
- Modify: `crates/core/src/workspace/layout.rs` (función `place`, líneas ~410-431)

- [ ] **Step 1: Reescribir `place`**

```rust
/// Coloca recursivamente `node` dentro de `area`, acumulando los rects de las hojas.
fn place(node: &DockNode, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
    match node {
        DockNode::Leaf(id) => out.push((*id, area)),
        DockNode::Tabs { members, .. } => {
            for id in members {
                out.push((*id, area));
            }
        }
        DockNode::Split { dir, children, weights } => {
            let rects = child_rects(area, *dir, weights);
            for (child, r) in children.iter().zip(rects) {
                place(child, r, out);
            }
        }
    }
}
```

- [ ] **Step 2: Compilar (aún puede fallar por otras funciones)**

Run: `cargo build -p naygo-core 2>&1 | grep -c "error\[" || true`
Expected: el número de errores baja respecto a la Task 1 (place ya no aparece).

### Task 4: `handles` y `split_handles` N-ario (un handle por divisor)

**Files:**
- Modify: `crates/core/src/workspace/layout.rs` (función `handles`, líneas ~485-522; `SplitHandle` ~66-72)

- [ ] **Step 1: Extender `SplitHandle` con el índice de divisor**

Reemplazar el struct `SplitHandle`:

```rust
/// Un splitter arrastrable: la ruta al split, cuál divisor interno es (`divider`: 0..N-2),
/// el rect de su barra y su orientación.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitHandle {
    pub path: Vec<SplitStep>,
    pub divider: usize,
    pub rect: Rect,
    pub dir: SplitDir,
}
```

- [ ] **Step 2: Escribir el test que falla**

```rust
    #[test]
    fn split_handles_uno_por_divisor() {
        // Un split de 3 hijos → 2 divisores.
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Leaf(PaneId(2)),
                    DockNode::Leaf(PaneId(3)),
                ],
                weights: vec![1.0, 1.0, 1.0],
            }),
        };
        let area = Rect { x: 0.0, y: 0.0, w: 900.0, h: 600.0 };
        let hs = l.split_handles(area);
        assert_eq!(hs.len(), 2, "3 hijos → 2 divisores");
        assert_eq!(hs[0].divider, 0);
        assert_eq!(hs[1].divider, 1);
        assert!(hs[0].rect.x < hs[1].rect.x, "el 1er divisor está a la izquierda");
    }
```

- [ ] **Step 3: Reescribir `handles`**

```rust
/// Recorre el árbol acumulando un handle por CADA divisor interno de cada split. La barra
/// ocupa el hueco `SPLIT_BAR` entre dos hijos consecutivos.
fn handles(node: &DockNode, area: Rect, path: &mut Vec<SplitStep>, out: &mut Vec<SplitHandle>) {
    if let DockNode::Split { dir, children, weights } = node {
        let rects = child_rects(area, *dir, weights);
        // Un divisor entre el hijo i y el i+1: la barra está en el hueco entre sus rects.
        for i in 0..children.len().saturating_sub(1) {
            let a = rects[i];
            let bar = match dir {
                SplitDir::Horizontal => Rect {
                    x: a.x + a.w,
                    y: a.y,
                    w: SPLIT_BAR,
                    h: a.h,
                },
                SplitDir::Vertical => Rect {
                    x: a.x,
                    y: a.y + a.h,
                    w: a.w,
                    h: SPLIT_BAR,
                },
            };
            out.push(SplitHandle {
                path: path.clone(),
                divider: i,
                rect: bar,
                dir: *dir,
            });
        }
        for (i, child) in children.iter().enumerate() {
            path.push(SplitStep(i));
            handles(child, rects[i], path, out);
            path.pop();
        }
    }
}
```

- [ ] **Step 4: Ejecutar el test (junto con los demás en Task 8 si el crate aún no compila)**

Run: `cargo test -p naygo-core split_handles_uno_por_divisor 2>&1 | head -20`
Expected: PASA cuando el crate compile.

### Task 5: `set_divider` (mover un divisor = transferir peso local)

Reemplaza `set_fraction`. Mover el divisor `i` transfiere peso entre los hijos `i` e
`i+1`, manteniendo `weights[i] + weights[i+1]` constante → los demás no se mueven.

**Files:**
- Modify: `crates/core/src/workspace/layout.rs` (reemplazar `set_fraction`, líneas ~224-246)

- [ ] **Step 1: Escribir el test que falla**

```rust
    #[test]
    fn set_divider_solo_afecta_a_los_dos_vecinos() {
        // 3 hijos iguales (1:1:1). Mover el divisor 0 para que el hijo 0 quede al 10% del
        // par (0,1). El hijo 2 NO debe cambiar de ancho.
        let mut l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Leaf(PaneId(2)),
                    DockNode::Leaf(PaneId(3)),
                ],
                weights: vec![1.0, 1.0, 1.0],
            }),
        };
        let area = Rect { x: 0.0, y: 0.0, w: 900.0, h: 600.0 };
        let w2_antes = l.pane_rects(area).iter().find(|(id, _)| *id == PaneId(3)).unwrap().1.w;
        // frac_local = 0.1 → el hijo 0 toma el 10% de (peso0+peso1)=2.0 → 0.2; el 1 → 1.8.
        l.set_divider(&[], 0, 0.1);
        let rects = l.pane_rects(area);
        let w0 = rects.iter().find(|(id, _)| *id == PaneId(1)).unwrap().1.w;
        let w1 = rects.iter().find(|(id, _)| *id == PaneId(2)).unwrap().1.w;
        let w2 = rects.iter().find(|(id, _)| *id == PaneId(3)).unwrap().1.w;
        assert!(w0 < w1, "el hijo 0 quedó más chico que el 1");
        assert!((w2 - w2_antes).abs() < 1.0, "el hijo 2 (no vecino) no cambia: {w2} vs {w2_antes}");
    }
```

- [ ] **Step 2: Ejecutar para ver que falla**

Run: `cargo test -p naygo-core set_divider_solo_afecta_a_los_dos_vecinos 2>&1 | head -20`
Expected: FALLA a compilar — `no method named set_divider`.

- [ ] **Step 3: Implementar `set_divider`**

Reemplazar `set_fraction` por:

```rust
    /// Mueve el divisor `divider` (0..N-2) del split en `path`: `frac_local` ∈ [0.05, 0.95]
    /// es la proporción que toma el hijo `divider` DENTRO del par (`divider`, `divider+1`).
    /// Transfiere peso solo entre esos dos hijos (su suma se conserva) → el resto no se mueve.
    /// No-op si la ruta no apunta a un split o el índice de divisor está fuera de rango.
    pub fn set_divider(&mut self, path: &[SplitStep], divider: usize, frac_local: f32) {
        let Some(root) = self.root.as_mut() else { return };
        let mut node = root;
        for SplitStep(i) in path {
            match node {
                DockNode::Split { children, .. } => {
                    let Some(child) = children.get_mut(*i) else { return };
                    node = child;
                }
                DockNode::Leaf(_) | DockNode::Tabs { .. } => return,
            }
        }
        if let DockNode::Split { weights, .. } = node {
            if divider + 1 >= weights.len() {
                return;
            }
            let f = frac_local.clamp(0.05, 0.95);
            let pair = weights[divider] + weights[divider + 1];
            weights[divider] = pair * f;
            weights[divider + 1] = pair * (1.0 - f);
        }
    }
```

- [ ] **Step 4: Ejecutar el test**

Run: `cargo test -p naygo-core set_divider_solo_afecta_a_los_dos_vecinos 2>&1 | head -20`
Expected: PASA cuando el crate compile.

### Task 6: `divider_at` (posición del puntero → frac local + barra-fantasma)

Reemplaza `fraction_at`. Calcula, para un divisor dado, la fracción local bajo el
puntero y el rect de la barra-fantasma (para el preview en vivo del arrastre).

**Files:**
- Modify: `crates/core/src/workspace/layout.rs` (reemplazar `fraction_at`, líneas ~254-323)

- [ ] **Step 1: Escribir el test que falla**

```rust
    #[test]
    fn divider_at_mide_local_al_par() {
        // 3 hijos 1:1:1 en 900px. El divisor 1 (entre hijo 1 y 2). El par ocupa la mitad
        // derecha ~[300..900]. Puntero a mitad de ESE par debe dar frac_local ~0.5.
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Leaf(PaneId(2)),
                    DockNode::Leaf(PaneId(3)),
                ],
                weights: vec![1.0, 1.0, 1.0],
            }),
        };
        let area = Rect { x: 0.0, y: 0.0, w: 900.0, h: 600.0 };
        // Rects aprox: hijo1 [0..~297], hijo2 [~301..~598], hijo3 [~602..900].
        // El par (1,2) va de ~301 a ~900. Su centro ~ (301+900)/2 = 600.
        let (f, bar) = l.divider_at(&[], 1, area, 600.0, 300.0).unwrap();
        assert!((f - 0.5).abs() < 0.05, "frac local ~0.5 en el centro del par: {f}");
        assert!((bar.x - 598.0).abs() < 6.0, "la barra cae cerca de x~600: {}", bar.x);
        // Clamp en el extremo izquierdo del par.
        let (fmin, _) = l.divider_at(&[], 1, area, 0.0, 300.0).unwrap();
        assert!((fmin - 0.05).abs() < 0.001, "clamp mínimo");
        // Divisor fuera de rango → None.
        assert!(l.divider_at(&[], 9, area, 100.0, 100.0).is_none());
    }
```

- [ ] **Step 2: Ejecutar para ver que falla**

Run: `cargo test -p naygo-core divider_at_mide_local_al_par 2>&1 | head -20`
Expected: FALLA a compilar — `no method named divider_at`.

- [ ] **Step 3: Implementar `divider_at`**

Reemplazar `fraction_at` por:

```rust
    /// Dado el split en `path`, el divisor `divider` (entre hijos `divider` y `divider+1`), el
    /// `area` total y el puntero `(px,py)`, calcula (frac_local, bar_rect): la fracción CLAMP
    /// [0.05,0.95] que el corte pondría DENTRO del par de hijos adyacentes, y el rect de la
    /// barra-fantasma resultante. Se mide sobre el sub-rect del PAR (hijo divider ∪ hijo
    /// divider+1 ∪ la barra entre ellos), que es lo correcto para "solo vecinos". `None` si la
    /// ruta no es un split o el divisor está fuera de rango. Lo usan el preview del arrastre y
    /// el commit, para que coincidan.
    pub fn divider_at(
        &self,
        path: &[SplitStep],
        divider: usize,
        area: Rect,
        px: f32,
        py: f32,
    ) -> Option<(f32, Rect)> {
        let root = self.root.as_ref()?;
        // Bajar al split objetivo recortando el sub-rect en cada paso.
        let mut node = root;
        let mut sub = area;
        for SplitStep(i) in path {
            if let DockNode::Split { dir, children, weights } = node {
                let rects = child_rects(sub, *dir, weights);
                node = children.get(*i)?;
                sub = *rects.get(*i)?;
            } else {
                return None;
            }
        }
        let DockNode::Split { dir, children, weights } = node else {
            return None;
        };
        if divider + 1 >= children.len() {
            return None;
        }
        // Rect del par (hijo divider ∪ barra ∪ hijo divider+1).
        let rects = child_rects(sub, *dir, weights);
        let a = rects[divider];
        let b = rects[divider + 1];
        let half = SPLIT_BAR / 2.0;
        let (f, bar) = match dir {
            SplitDir::Horizontal => {
                let pair_x = a.x;
                let pair_w = (b.x + b.w) - a.x;
                let f = if pair_w <= 0.0 { 0.5 } else { ((px - pair_x) / pair_w).clamp(0.05, 0.95) };
                let bx = pair_x + pair_w * f - half;
                (f, Rect { x: bx, y: a.y, w: SPLIT_BAR, h: a.h })
            }
            SplitDir::Vertical => {
                let pair_y = a.y;
                let pair_h = (b.y + b.h) - a.y;
                let f = if pair_h <= 0.0 { 0.5 } else { ((py - pair_y) / pair_h).clamp(0.05, 0.95) };
                let by = pair_y + pair_h * f - half;
                (f, Rect { x: a.x, y: by, w: a.w, h: SPLIT_BAR })
            }
        };
        Some((f, bar))
    }
```

- [ ] **Step 4: Ejecutar el test**

Run: `cargo test -p naygo-core divider_at_mide_local_al_par 2>&1 | head -20`
Expected: PASA cuando el crate compile.

### Task 7: Adaptar `split_in`, `remove_in`, `collect`, `stack_in`, `activate_in`, `collect_groups`, `swap_children_in`, `subtree_contains`

**Files:**
- Modify: `crates/core/src/workspace/layout.rs` (funciones auxiliares que recorren `Split`)

- [ ] **Step 1: Reescribir todas las funciones que hacían match sobre `Split { first, second }`**

`split_in` — dividir una hoja crea un split de 2 hijos con pesos iguales; recorre `children`:

```rust
fn split_in(node: &mut DockNode, leaf: PaneId, dir: SplitDir, new_id: PaneId) {
    match node {
        DockNode::Leaf(id) if *id == leaf => {
            *node = DockNode::Split {
                dir,
                children: vec![DockNode::Leaf(leaf), DockNode::Leaf(new_id)],
                weights: vec![1.0, 1.0],
            };
        }
        DockNode::Leaf(_) => {}
        DockNode::Tabs { members, .. } if members.contains(&leaf) => {
            let group = node.clone();
            *node = DockNode::Split {
                dir,
                children: vec![group, DockNode::Leaf(new_id)],
                weights: vec![1.0, 1.0],
            };
        }
        DockNode::Tabs { .. } => {}
        DockNode::Split { children, .. } => {
            for c in children.iter_mut() {
                split_in(c, leaf, dir, new_id);
            }
        }
    }
}
```

`remove_in` — quitar una hoja; si un split queda con 1 hijo colapsa a ese hijo, con 0
desaparece; los pesos se recortan a la par:

```rust
fn remove_in(node: DockNode, id: PaneId) -> Option<DockNode> {
    match node {
        DockNode::Leaf(leaf) => (leaf != id).then_some(DockNode::Leaf(leaf)),
        DockNode::Tabs { mut members, mut active } => {
            members.retain(|m| *m != id);
            match members.len() {
                0 => None,
                1 => Some(DockNode::Leaf(members[0])),
                n => {
                    if active >= n {
                        active = n - 1;
                    }
                    Some(DockNode::Tabs { members, active })
                }
            }
        }
        DockNode::Split { dir, children, weights } => {
            let mut kept: Vec<DockNode> = Vec::new();
            let mut kw: Vec<f32> = Vec::new();
            for (c, w) in children.into_iter().zip(weights) {
                if let Some(c) = remove_in(c, id) {
                    kept.push(c);
                    kw.push(w);
                }
            }
            match kept.len() {
                0 => None,
                1 => Some(kept.into_iter().next().unwrap()),
                _ => Some(DockNode::Split { dir, children: kept, weights: kw }),
            }
        }
    }
}
```

`collect`:

```rust
fn collect(node: &DockNode, out: &mut Vec<PaneId>) {
    match node {
        DockNode::Leaf(id) => out.push(*id),
        DockNode::Tabs { members, .. } => out.extend_from_slice(members),
        DockNode::Split { children, .. } => {
            for c in children {
                collect(c, out);
            }
        }
    }
}
```

`stack_in`:

```rust
fn stack_in(node: &mut DockNode, onto: PaneId, new_id: PaneId) {
    match node {
        DockNode::Leaf(id) if *id == onto => {
            *node = DockNode::Tabs { members: vec![*id, new_id], active: 1 };
        }
        DockNode::Leaf(_) => {}
        DockNode::Tabs { members, active } if members.contains(&onto) => {
            members.push(new_id);
            *active = members.len() - 1;
        }
        DockNode::Tabs { .. } => {}
        DockNode::Split { children, .. } => {
            for c in children.iter_mut() {
                stack_in(c, onto, new_id);
            }
        }
    }
}
```

`activate_in`:

```rust
fn activate_in(node: &mut DockNode, member: PaneId) {
    match node {
        DockNode::Leaf(_) => {}
        DockNode::Tabs { members, active } => {
            if let Some(pos) = members.iter().position(|m| *m == member) {
                *active = pos;
            }
        }
        DockNode::Split { children, .. } => {
            for c in children.iter_mut() {
                activate_in(c, member);
            }
        }
    }
}
```

`collect_groups`:

```rust
fn collect_groups(node: &DockNode, out: &mut Vec<(Vec<PaneId>, usize)>) {
    match node {
        DockNode::Leaf(_) => {}
        DockNode::Tabs { members, active } => out.push((members.clone(), *active)),
        DockNode::Split { children, .. } => {
            for c in children {
                collect_groups(c, out);
            }
        }
    }
}
```

`subtree_contains`:

```rust
fn subtree_contains(node: &DockNode, id: PaneId) -> bool {
    match node {
        DockNode::Leaf(leaf) => *leaf == id,
        DockNode::Tabs { members, .. } => members.contains(&id),
        DockNode::Split { children, .. } => children.iter().any(|c| subtree_contains(c, id)),
    }
}
```

`swap_children_in` — el "swap" ahora invierte el orden de los dos hijos (y sus pesos)
del split que separa `a` de `b`. Con N hijos, busca los dos índices que contienen a
`a` y a `b` y los intercambia:

```rust
fn swap_children_in(node: &mut DockNode, a: PaneId, b: PaneId) -> bool {
    if let DockNode::Split { children, weights, .. } = node {
        let ia = children.iter().position(|c| subtree_contains(c, a));
        let ib = children.iter().position(|c| subtree_contains(c, b));
        if let (Some(ia), Some(ib)) = (ia, ib) {
            if ia != ib {
                children.swap(ia, ib);
                weights.swap(ia, ib);
                return true;
            }
        }
        return children.iter_mut().any(|c| swap_children_in(c, a, b));
    }
    false
}
```

- [ ] **Step 2: Compilar el módulo (sin tests aún)**

Run: `cargo build -p naygo-core 2>&1 | grep "error\[" | head -20 || echo "SIN ERRORES DE BUILD"`
Expected: eventualmente "SIN ERRORES DE BUILD" (los tests viejos aún pueden no
compilar; se arreglan en Task 8).

### Task 8: Migración serde (`fraction` viejo → `weights`) + adaptar tests

**Files:**
- Modify: `crates/core/src/workspace/layout.rs` (deserialización + módulo `tests`)

- [ ] **Step 1: Escribir el test de migración que falla**

```rust
    #[test]
    fn migra_split_binario_viejo_a_pesos() {
        // Formato viejo en disco: { "Split": { "dir": "Horizontal", "fraction": 0.25,
        // "first": {"Leaf": 1}, "second": {"Leaf": 2} } }
        let viejo = r#"{"root":{"Split":{"dir":"Horizontal","fraction":0.25,"first":{"Leaf":1},"second":{"Leaf":2}}}}"#;
        let l: SerializableDockLayout = serde_json::from_str(viejo).unwrap();
        // Debe leerse como split de 2 hijos con pesos 0.25 : 0.75.
        if let Some(DockNode::Split { children, weights, .. }) = &l.root {
            assert_eq!(children.len(), 2);
            assert!((weights[0] - 0.25).abs() < 0.001);
            assert!((weights[1] - 0.75).abs() < 0.001);
        } else {
            panic!("debe migrar a Split N-ario");
        }
    }
```

- [ ] **Step 2: Ejecutar para ver que falla**

Run: `cargo test -p naygo-core migra_split_binario_viejo_a_pesos 2>&1 | head -20`
Expected: FALLA — el serde derivado no reconoce `fraction`/`first`/`second`.

- [ ] **Step 3: Implementar la deserialización tolerante**

Cambiar `DockNode` para NO derivar `Deserialize` directamente en la variante `Split`
antigua. Estrategia: un enum intermedio `DockNodeWire` con `#[serde(untagged)]` que
acepta ambas formas, y un `impl<'de> Deserialize<'de> for DockNode` que convierte. Para
mantener el plan acotado y robusto, se usa una forma "espejo" con serde:

Añadir arriba del enum `DockNode`, y cambiar su derive de `Deserialize`:

```rust
// `DockNode` deriva Serialize (formato nuevo) pero implementa Deserialize a mano para
// aceptar TAMBIÉN el formato viejo (Split binario con `fraction`/`first`/`second`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum DockNode { /* … variantes, SIN derivar Deserialize … */ }

#[derive(Deserialize)]
enum DockNodeWire {
    Leaf(PaneId),
    Tabs { members: Vec<PaneId>, active: usize },
    // Forma NUEVA (N-aria).
    Split {
        dir: SplitDir,
        #[serde(default)]
        children: Vec<DockNodeWire>,
        #[serde(default)]
        weights: Vec<f32>,
        // Forma VIEJA (binaria): presentes solo en JSON antiguo.
        #[serde(default)]
        fraction: Option<f32>,
        #[serde(default)]
        first: Option<Box<DockNodeWire>>,
        #[serde(default)]
        second: Option<Box<DockNodeWire>>,
    },
}

impl From<DockNodeWire> for DockNode {
    fn from(w: DockNodeWire) -> Self {
        match w {
            DockNodeWire::Leaf(id) => DockNode::Leaf(id),
            DockNodeWire::Tabs { members, active } => DockNode::Tabs { members, active },
            DockNodeWire::Split { dir, children, weights, fraction, first, second } => {
                // Forma vieja: hay first+second → convertir a 2 hijos con pesos.
                if let (Some(f), Some(s)) = (first, second) {
                    let frac = fraction.unwrap_or(0.5).clamp(0.05, 0.95);
                    return DockNode::Split {
                        dir,
                        children: vec![(*f).into(), (*s).into()],
                        weights: vec![frac, 1.0 - frac],
                    };
                }
                // Forma nueva.
                let children: Vec<DockNode> = children.into_iter().map(Into::into).collect();
                let mut weights = weights;
                // Defensa: pesos ausentes o de largo distinto → iguales.
                if weights.len() != children.len() {
                    weights = vec![1.0; children.len()];
                }
                DockNode::Split { dir, children, weights }
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for DockNode {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        DockNodeWire::deserialize(d).map(Into::into)
    }
}
```

- [ ] **Step 4: Adaptar TODOS los tests viejos del módulo al nuevo constructor**

Buscar en el módulo `tests` cada literal `DockNode::Split { dir, fraction, first, second }`
y reescribirlo a `DockNode::Split { dir, children: vec![...], weights: vec![...] }`.
También `SplitStep::First`/`Second` → `SplitStep(0)`/`SplitStep(1)`, y las llamadas a
`set_fraction`/`fraction_at` → `set_divider`/`divider_at` con su índice de divisor.
Ejemplo de conversión (test `split_recolecta_en_orden`):

```rust
    #[test]
    fn split_recolecta_en_orden() {
        let l = SerializableDockLayout {
            root: Some(DockNode::Split {
                dir: SplitDir::Horizontal,
                children: vec![
                    DockNode::Leaf(PaneId(1)),
                    DockNode::Split {
                        dir: SplitDir::Horizontal,
                        children: vec![DockNode::Leaf(PaneId(2)), DockNode::Leaf(PaneId(3))],
                        weights: vec![1.0, 1.0],
                    },
                ],
                weights: vec![1.0, 1.0],
            }),
        };
        assert_eq!(l.pane_ids(), vec![PaneId(1), PaneId(2), PaneId(3)]);
    }
```

Aplicar la misma mecánica a: `pane_rects_split_horizontal_reparte_por_fraction`,
`pane_rects_split_vertical`, `split_handles_y_set_fraction` (renombrar a
`split_handles_y_set_divider`, usar `set_divider(&path, 0, 0.25)`),
`fraction_at_mide_dentro_del_subrect_del_split` (borrar: reemplazado por
`divider_at_mide_local_al_par`), `set_fraction_clampa` (→ `set_divider_clampa`,
usando `set_divider(&[], 0, 2.0)` y verificando `weights` clampeados),
`split_leaf_divide_la_hoja`, `remove_leaf_colapsa_el_split`,
`split_leaf_dentro_de_un_grupo_divide_el_grupo`. Mantener intactos los tests de
`Tabs`, `drop_hit`, `drop_zones` y los round-trip serde (agregar el de migración).

- [ ] **Step 5: Ejecutar TODA la suite de core**

Run: `cargo test -p naygo-core 2>&1 | tail -25`
Expected: PASS — todos los tests del crate, incluidos los nuevos
(`child_rects_*`, `split_handles_uno_por_divisor`, `set_divider_*`,
`divider_at_mide_local_al_par`, `migra_split_binario_viejo_a_pesos`).

- [ ] **Step 6: Commit de la Fase 1**

```bash
git add crates/core/src/workspace/layout.rs
git commit -m "$(cat <<'EOF'
feat(core): modelo de divisores N-ario con pesos (resto fijo + responsive)

DockNode::Split pasa de binario (fraction/first/second) a N-ario
(children + weights). Mover un divisor transfiere peso solo entre sus
dos hijos vecinos (su suma se conserva) → el resto de paneles no se
mueve. Al redimensionar, cada hijo toma weight/Σweights del área →
responsive proporcional. Deserialización tolerante migra el formato
binario viejo (fraction) a pesos.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

### Task 9: Cablear la UI al nuevo modelo (wrappers + handlers Slint)

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl/layout_panes.rs:54-71` (wrappers)
- Modify: `crates/ui-slint/ui/types.slint:502-509` (`SplitVm`: agregar `divider`)
- Modify: `crates/ui-slint/src/main.rs:957-1006` (construcción de `SplitVm`)
- Modify: `crates/ui-slint/src/main.rs:5299-5340` (handlers `on_split_drag`/`on_split_commit`)
- Modify: `crates/ui-slint/ui/app-window.slint:427,519-529,1388-1392` (paso de `divider` al callback)

- [ ] **Step 1: Actualizar los wrappers de `WorkspaceCtrl`**

En `layout_panes.rs`, reemplazar `set_fraction` y `fraction_at`:

```rust
    pub fn set_divider(&mut self, path: &[SplitStep], divider: usize, frac_local: f32) {
        self.ws.layout.set_divider(path, divider, frac_local);
    }

    pub fn divider_at(
        &self,
        path: &[SplitStep],
        divider: usize,
        area: Rect,
        px: f32,
        py: f32,
    ) -> Option<(f32, naygo_core::workspace::layout::Rect)> {
        self.ws.layout.divider_at(path, divider, area, px, py)
    }
```

(`split_handles` no cambia de firma; ahora cada `SplitHandle` trae `divider`.)

- [ ] **Step 2: Agregar `divider` a `SplitVm`**

En `types.slint`, dentro de `export struct SplitVm { … }`, agregar el campo:

```slint
    divider: int,
```

- [ ] **Step 3: Poblar `divider` al construir los `SplitVm`**

En `main.rs` (las dos construcciones ~957 y ~994), dentro del `.map(|(i, h)| SplitVm { … })`,
agregar `divider: h.divider as i32,` junto a los campos existentes (que ya copian
`h.rect`/`h.dir`). El `index` que se pasa al callback sigue siendo el índice del
handle en el `Vec` (identifica el handle completo: su `path` + `divider`).

- [ ] **Step 4: Usar `divider` en los handlers**

En `main.rs`, `on_split_drag` (~5299) y `on_split_commit` (~5322): tras localizar el
handle por `index`, pasar `h.divider` a las nuevas funciones. Ejemplo `on_split_drag`:

```rust
        ui.on_split_drag(move |index, px, py| {
            let c = ctrl.borrow();
            let area = /* … igual que hoy … */;
            let handles = c.split_handles(area);
            if let Some(h) = handles.get(index as usize) {
                if let Some((_f, bar)) = c.divider_at(&h.path.clone(), h.divider, area, px, py) {
                    ui.set_splitpreview_x(bar.x);
                    ui.set_splitpreview_y(bar.y);
                    ui.set_splitpreview_w(bar.w);
                    ui.set_splitpreview_h(bar.h);
                }
            }
        });
```

Y `on_split_commit` (~5322):

```rust
        ui.on_split_commit(move |index, px, py| {
            {
                let mut c = ctrl.borrow_mut();
                let area = /* … igual que hoy … */;
                let handles = c.split_handles(area);
                if let Some(h) = handles.get(index as usize) {
                    let path = h.path.clone();
                    let divider = h.divider;
                    if let Some((f, _bar)) = c.divider_at(&path, divider, area, px, py) {
                        c.set_divider(&path, divider, f);
                    }
                }
            }
            ui.set_splitpreview_w(0.0);
            ui.set_splitpreview_h(0.0);
            sync_layout();
        });
```

(Respetar el patrón de scope del `borrow_mut` en un bloque antes de `sync_layout()`,
por el footgun de doble-borrow del RefCell ya conocido en este código.)

- [ ] **Step 5: Compilar la UI**

Run: `CARGO_BUILD_JOBS=2 cargo build -p naygo-ui-slint 2>&1 | tail -20`
Expected: compila sin errores.

- [ ] **Step 6: Test de regresión del controlador**

En `crates/ui-slint/src/workspace_ctrl/tests.rs` (~1623 ya usa `split_handles`), añadir
un test que verifique el reparto "solo vecinos" a nivel controlador: construir 3 paneles
en fila, mover el divisor 0 vía `set_divider`, y comprobar por `pane_rects` que el 3er
panel no cambió de ancho. Run:

```
cargo test -p naygo-ui-slint solo_vecinos 2>&1 | tail -15
```
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/ui-slint/src/workspace_ctrl/layout_panes.rs crates/ui-slint/ui/types.slint crates/ui-slint/src/main.rs crates/ui-slint/ui/app-window.slint crates/ui-slint/src/workspace_ctrl/tests.rs
git commit -m "$(cat <<'EOF'
feat(ui): cablear divisores N-ario (divider index + set_divider/divider_at)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 2 — Extras UX de divisores (doble-clic 50/50, cursor, hover)

### Task 10: Doble-clic en un divisor = 50/50 entre sus vecinos

**Files:**
- Modify: `crates/ui-slint/ui/app-window.slint` (TouchArea de la barra de split)
- Modify: `crates/ui-slint/src/main.rs` (nuevo callback `on_split_reset`)

- [ ] **Step 1: Declarar el callback en la Window Slint**

En `app-window.slint`, junto a `split-drag`/`split-commit`, declarar:

```slint
    callback split-reset(int); // index del handle → repartir 50/50 sus dos vecinos
```

- [ ] **Step 2: Disparar el callback con doble-clic en la barra**

En el `TouchArea` de cada barra de split (donde ya se maneja el drag), agregar el
manejo de doble-clic. Slint expone `double-clicked` en `TouchArea`:

```slint
        ta := TouchArea {
            // … pointer-event / moved existentes para el drag …
            double-clicked => { root.split-reset(split.index); }
        }
```

- [ ] **Step 3: Implementar el handler en Rust**

En `main.rs`, junto a los otros handlers de split:

```rust
        ui.on_split_reset(move |index| {
            {
                let mut c = ctrl.borrow_mut();
                let area = /* … igual que on_split_commit … */;
                let handles = c.split_handles(area);
                if let Some(h) = handles.get(index as usize) {
                    let path = h.path.clone();
                    let divider = h.divider;
                    c.set_divider(&path, divider, 0.5);
                }
            }
            sync_layout();
        });
```

- [ ] **Step 4: Compilar**

Run: `CARGO_BUILD_JOBS=2 cargo build -p naygo-ui-slint 2>&1 | tail -15`
Expected: compila.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/ui/app-window.slint crates/ui-slint/src/main.rs
git commit -m "$(cat <<'EOF'
feat(ui): doble-clic en un divisor reparte 50/50 sus dos paneles vecinos

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

### Task 11: Cursor de resize + resaltado en hover/arrastre

**Files:**
- Modify: `crates/ui-slint/ui/app-window.slint` (barra de split: `mouse-cursor` + color por hover)

- [ ] **Step 1: Cursor según orientación**

En el `TouchArea` de la barra, fijar `mouse-cursor` según `split.dir` (el `SplitVm` ya
trae `dir`; Slint expone `MouseCursor.col-resize` / `MouseCursor.row-resize`). Si `dir`
llega como enum/int, mapear: Horizontal (barra vertical, arrastra en X) → `col-resize`;
Vertical (barra horizontal, arrastra en Y) → `row-resize`:

```slint
        ta := TouchArea {
            mouse-cursor: split.is-horizontal ? MouseCursor.col-resize : MouseCursor.row-resize;
            // …
        }
```

Si `SplitVm` no expone un booleano de orientación, agregar `is-horizontal: bool` a
`SplitVm` en `types.slint` y poblarlo en `main.rs`
(`is_horizontal: matches!(h.dir, SplitDir::Horizontal)`).

- [ ] **Step 2: Resaltado en hover/arrastre**

El `Rectangle` de la barra usa el color de acento del tema cuando `ta.has-hover` o
mientras se arrastra; si no, el color neutro actual:

```slint
        Rectangle {
            background: ta.has-hover || ta.pressed ? root.accent-color : root.split-bar-color;
            // … geometría existente …
        }
```

Usar la propiedad de acento del tema ya disponible en la Window (la misma que usan los
anillos de foco). Si no hay una propiedad `accent-color` expuesta a este scope,
reutilizar la que ya emplean los modales/foco (verificar el nombre real en
`app-window.slint`; no introducir un color hardcoded).

- [ ] **Step 3: Compilar y revisar visualmente en el binario**

Run: `CARGO_BUILD_JOBS=2 cargo build -p naygo-ui-slint 2>&1 | tail -15`
Expected: compila. (La verificación visual del cursor/hover la hace Nicolás en la VM;
anotarlo en el resumen de la fase.)

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/ui/app-window.slint crates/ui-slint/ui/types.slint crates/ui-slint/src/main.rs
git commit -m "$(cat <<'EOF'
feat(ui): cursor de redimensionar y resaltado de acento al hover/arrastrar divisores

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 3 — Recordar geometría de la ventana

### Task 12: `WindowGeometry` en Settings + validación de visibilidad (core, puro)

**Files:**
- Modify: `crates/core/src/config/mod.rs` (nuevo struct + campo en `Settings`)
- Test: `crates/core/src/config/mod.rs` (módulo `tests`)

- [ ] **Step 1: Definir `WindowGeometry` y añadir el campo a `Settings`**

En `config/mod.rs`, antes del struct `Settings`, añadir:

```rust
/// Geometría persistida de la ventana principal, para restaurarla entre sesiones.
/// Coordenadas en px físicos (las que da/consume el SO). `maximized` recuerda el estado;
/// `width/height/x/y` guardan SIEMPRE el rect "restaurado" (des-maximizado), para volver a
/// un tamaño sensato al des-maximizar.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

impl WindowGeometry {
    /// ¿El rect de la ventana intersecta de forma USABLE alguno de los monitores dados?
    /// `monitors` son rects (x,y,w,h) en las mismas coords físicas. "Usable" = al menos
    /// `MIN_VISIBLE` px de la ventana caen dentro de algún monitor, así una ventana en un
    /// monitor desconectado o fuera de pantalla se detecta como no visible. Puro y testeable.
    pub fn is_visible_on(&self, monitors: &[(i32, i32, u32, u32)]) -> bool {
        const MIN_VISIBLE: i32 = 64;
        let (wx, wy, ww, wh) = (self.x, self.y, self.width as i32, self.height as i32);
        monitors.iter().any(|&(mx, my, mw, mh)| {
            // Ancho/alto del solape entre el rect de la ventana y el del monitor.
            let ox = (wx + ww).min(mx + mw as i32) - wx.max(mx);
            let oy = (wy + wh).min(my + mh as i32) - wy.max(my);
            ox >= MIN_VISIBLE && oy >= MIN_VISIBLE
        })
    }
}
```

Y en `Settings`, al final de los campos (antes del cierre del struct en la línea
~269), añadir:

```rust
    /// Geometría de la ventana principal (tamaño/posición/maximizado) para restaurar al
    /// abrir. `None` = nunca se guardó (primera vez) → la app usa el tamaño por defecto.
    #[serde(default)]
    pub window: Option<WindowGeometry>,
```

- [ ] **Step 2: Escribir el test de visibilidad que falla**

```rust
    #[test]
    fn window_geometry_visibilidad_multimonitor() {
        let primario = (0i32, 0i32, 1920u32, 1080u32);
        // Ventana bien dentro del primario.
        let g = WindowGeometry { width: 800, height: 600, x: 100, y: 100, maximized: false };
        assert!(g.is_visible_on(&[primario]));
        // Ventana en un segundo monitor a la derecha, que YA no está.
        let g2 = WindowGeometry { width: 800, height: 600, x: 3000, y: 100, maximized: false };
        assert!(!g2.is_visible_on(&[primario]), "fuera de todo monitor → no visible");
        // El mismo rect, con el segundo monitor presente, sí es visible.
        let secundario = (1920i32, 0i32, 1920u32, 1080u32);
        assert!(g2.is_visible_on(&[primario, secundario]));
    }
```

- [ ] **Step 3: Ejecutar el test**

Run: `cargo test -p naygo-core window_geometry_visibilidad_multimonitor 2>&1 | head -20`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/config/mod.rs
git commit -m "$(cat <<'EOF'
feat(core): WindowGeometry en Settings + validación de visibilidad multi-monitor

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

### Task 13: Leer/fijar geometría de la ventana (platform, Win32)

Módulo nuevo en `platform` para leer el placement de la ventana (tamaño/pos/maximizado)
y enumerar monitores vía Win32, más aplicar un placement. Slint no expone posición de
forma uniforme; el HWND sí (`GetWindowPlacement`/`SetWindowPlacement`,
`EnumDisplayMonitors`).

**Files:**
- Create: `crates/platform/src/window_geometry.rs`
- Modify: `crates/platform/src/lib.rs` (declarar `pub mod window_geometry;`)

- [ ] **Step 1: Crear el módulo con la API y stubs no-Windows**

Crear `crates/platform/src/window_geometry.rs` con header Naygo y:

```rust
//! Lee/aplica la geometría de la ventana principal vía Win32 sobre su HWND, y enumera los
//! monitores conectados. Slint no da posición de forma portable; el HWND sí. Tolerante: si
//! algo falla, devuelve None y el llamador cae al tamaño por defecto.

/// (width, height, x, y, maximized) en px físicos. Igual forma que core::config::WindowGeometry
/// pero sin dependencia cruzada (platform no depende de core en este punto).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

#[cfg(windows)]
mod windows_impl {
    use super::Placement;
    use windows::Win32::Foundation::{HWND, LPARAM, RECT, BOOL, TRUE};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, SetWindowPlacement, SHOW_WINDOW_CMD, SW_MAXIMIZE, SW_SHOWNORMAL,
        WINDOWPLACEMENT,
    };

    /// Lee el placement de la ventana `hwnd`. El `rcNormalPosition` es el rect "restaurado"
    /// (des-maximizado), justo lo que queremos guardar. `showCmd == SW_MAXIMIZE` → maximizada.
    pub fn get(hwnd: isize) -> Option<Placement> {
        unsafe {
            let hwnd = HWND(hwnd as *mut _);
            let mut wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            GetWindowPlacement(hwnd, &mut wp).ok()?;
            let r = wp.rcNormalPosition;
            Some(Placement {
                width: (r.right - r.left).max(0) as u32,
                height: (r.bottom - r.top).max(0) as u32,
                x: r.left,
                y: r.top,
                maximized: wp.showCmd == SW_MAXIMIZE.0 as u32,
            })
        }
    }

    /// Aplica un placement a `hwnd`: fija el rect restaurado y, si corresponde, maximiza.
    pub fn set(hwnd: isize, p: Placement) {
        unsafe {
            let hwnd = HWND(hwnd as *mut _);
            let wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                showCmd: if p.maximized { SW_MAXIMIZE.0 as u32 } else { SW_SHOWNORMAL.0 as u32 },
                rcNormalPosition: RECT {
                    left: p.x,
                    top: p.y,
                    right: p.x + p.width as i32,
                    bottom: p.y + p.height as i32,
                },
                ..Default::default()
            };
            let _ = SetWindowPlacement(hwnd, &wp);
        }
    }

    /// Rects (x,y,w,h) de todos los monitores conectados.
    pub fn monitors() -> Vec<(i32, i32, u32, u32)> {
        unsafe extern "system" fn cb(m: HMONITOR, _dc: HDC, _r: *mut RECT, data: LPARAM) -> BOOL {
            let out = &mut *(data.0 as *mut Vec<(i32, i32, u32, u32)>);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(m, &mut mi).as_bool() {
                let r = mi.rcMonitor;
                out.push((r.left, r.top, (r.right - r.left) as u32, (r.bottom - r.top) as u32));
            }
            TRUE
        }
        let mut out: Vec<(i32, i32, u32, u32)> = Vec::new();
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut out as *mut _ as isize));
        }
        out
    }
}

#[cfg(windows)]
pub use windows_impl::{get, monitors, set};

#[cfg(not(windows))]
pub fn get(_hwnd: isize) -> Option<Placement> { None }
#[cfg(not(windows))]
pub fn set(_hwnd: isize, _p: Placement) {}
#[cfg(not(windows))]
pub fn monitors() -> Vec<(i32, i32, u32, u32)> { Vec::new() }
```

- [ ] **Step 2: Declarar el módulo**

En `crates/platform/src/lib.rs`, junto a los otros `pub mod`, añadir:

```rust
pub mod window_geometry;
```

- [ ] **Step 3: Verificar las features del crate `windows`**

Las APIs usadas requieren estas features en `crates/platform/Cargo.toml` (dependencia
`windows`): `Win32_UI_WindowsAndMessaging`, `Win32_Graphics_Gdi`, `Win32_Foundation`.
Comprobar que estén; si falta alguna, añadirla a la lista `features = [...]`.

Run: `cargo build -p naygo-platform 2>&1 | tail -25`
Expected: compila. Si falla por símbolos no encontrados (feature ausente), añadir la
feature que el error nombre y recompilar.

- [ ] **Step 4: Commit**

```bash
git add crates/platform/src/window_geometry.rs crates/platform/src/lib.rs crates/platform/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(platform): leer/aplicar geometría de ventana y enumerar monitores (Win32)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

### Task 14: Guardar geometría al cerrar y restaurar al abrir (UI)

**Files:**
- Modify: `crates/ui-slint/src/main.rs` (obtener el HWND de la ventana; restaurar tras crear la UI; guardar en el cierre ~5354 y en el tick)

- [ ] **Step 1: Obtener el HWND de la ventana Slint**

Tras crear `ui` (AppWindow) y antes del event loop, obtener el handle nativo. Slint
expone el handle vía `raw-window-handle` (`ui.window().window_handle()`); del
`RawWindowHandle::Win32` se extrae `hwnd`. Añadir un helper local en `main.rs`:

```rust
    #[cfg(windows)]
    fn hwnd_of(win: &slint::Window) -> Option<isize> {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let h = win.window_handle().ok()?;
        match h.as_raw() {
            RawWindowHandle::Win32(w) => Some(isize::from(w.hwnd)),
            _ => None,
        }
    }
    #[cfg(not(windows))]
    fn hwnd_of(_win: &slint::Window) -> Option<isize> { None }
```

Verificar que `raw-window-handle` esté disponible (Slint lo reexporta o está como dep;
si no, agregarlo al `Cargo.toml` de `naygo-ui-slint` con la misma versión que use
Slint). Si la API de handle difiere en la versión de Slint del proyecto, usar la que
exponga esa versión para obtener el HWND (objetivo: un `isize` del HWND).

- [ ] **Step 2: Restaurar geometría al abrir**

Tras cargar settings y crear la ventana, antes de `ui.run()`, si hay
`settings.window`, validar y aplicar:

```rust
    #[cfg(windows)]
    if let Some(g) = ctrl.borrow().config.settings.window {
        if let Some(hwnd) = hwnd_of(&ui.window()) {
            let mons = naygo_platform::window_geometry::monitors();
            let core_g = naygo_core::config::WindowGeometry {
                width: g.width, height: g.height, x: g.x, y: g.y, maximized: g.maximized,
            };
            let placement = if core_g.is_visible_on(&mons) {
                naygo_platform::window_geometry::Placement {
                    width: g.width, height: g.height, x: g.x, y: g.y, maximized: g.maximized,
                }
            } else {
                // Fuera de pantalla: centrar en el monitor principal con el tamaño guardado.
                let (mx, my, mw, mh) = mons.first().copied().unwrap_or((0, 0, 1920, 1080));
                let x = mx + ((mw as i32 - g.width as i32) / 2).max(0);
                let y = my + ((mh as i32 - g.height as i32) / 2).max(0);
                naygo_platform::window_geometry::Placement {
                    width: g.width, height: g.height, x, y, maximized: g.maximized,
                }
            };
            naygo_platform::window_geometry::set(hwnd, placement);
        }
    }
```

- [ ] **Step 3: Guardar geometría al cerrar y en el tick**

En el handler de cierre de ventana (main.rs ~5354, donde se lee `close_to_tray`),
ANTES de decidir salir/esconder, capturar y persistir la geometría:

```rust
        #[cfg(windows)]
        if let Some(hwnd) = hwnd_of(&ui_weak.upgrade().unwrap().window()) {
            if let Some(p) = naygo_platform::window_geometry::get(hwnd) {
                let mut c = ctrl.borrow_mut();
                c.config.settings.window = Some(naygo_core::config::WindowGeometry {
                    width: p.width, height: p.height, x: p.x, y: p.y, maximized: p.maximized,
                });
                c.config.save();
            }
        }
```

(Usar el weak handle ya disponible en ese scope; respetar el patrón de scope del
`borrow_mut` para no chocar con otros borrows del cierre.)

- [ ] **Step 4: Compilar**

Run: `CARGO_BUILD_JOBS=2 cargo build -p naygo-ui-slint 2>&1 | tail -20`
Expected: compila.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/src/main.rs crates/ui-slint/Cargo.toml
git commit -m "$(cat <<'EOF'
feat(ui): recordar y restaurar tamaño/posición/maximizado de la ventana

Guarda la geometría al cerrar (rect restaurado + maximizado) y la
restaura al abrir, validando que caiga en un monitor conectado; si no,
centra en el principal con el tamaño guardado.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 4 — La X esconde a bandeja por defecto (migración)

### Task 15: Default `close_to_tray=true` + migración de CONFIG_VERSION

**Files:**
- Modify: `crates/core/src/config/mod.rs` (default del campo, `CONFIG_VERSION`, migración en `load_settings_flagged`)
- Test: `crates/core/src/config/mod.rs` (módulo `tests`)

- [ ] **Step 1: Cambiar el default de `close_to_tray` a `true`**

En `Settings`, el campo `close_to_tray` hoy usa `#[serde(default)]` (→ `false`).
Cambiarlo a un default con función explícita:

```rust
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
```

Y añadir la función y usarla también en el `impl Default for Settings` (buscar dónde se
construye el default de `close_to_tray` y ponerlo en `true`):

```rust
/// Default de `close_to_tray`: true (la X esconde a la bandeja en vez de salir).
fn default_close_to_tray() -> bool {
    true
}
```

- [ ] **Step 2: Escribir el test de migración que falla**

```rust
    #[test]
    fn migracion_v1_a_v2_fuerza_close_to_tray() {
        let dir = tempfile::tempdir().unwrap();
        // settings.json v1 con close_to_tray:false explícito.
        let mut s = Settings::default();
        s.version = 1;
        s.close_to_tray = false;
        std::fs::write(
            dir.path().join("settings.json"),
            serde_json::to_string_pretty(&s).unwrap(),
        ).unwrap();
        let loaded = load_settings(dir.path());
        assert_eq!(loaded.version, CONFIG_VERSION, "se migra a la versión nueva");
        assert!(loaded.close_to_tray, "la migración fuerza close_to_tray=true una vez");
    }
```

- [ ] **Step 3: Subir `CONFIG_VERSION` e implementar la migración**

Cambiar `const CONFIG_VERSION: u32 = 1;` a `2`. En `load_settings_flagged`, el brazo
`Some(mut s) if s.version == CONFIG_VERSION` solo corre para settings YA en v2. Añadir
un brazo previo que migre v1 → v2 antes de ese:

```rust
        Some(mut s) if s.version == 1 => {
            // Migración v1 → v2: forzar close_to_tray=true una vez (la X esconde a bandeja
            // por defecto). Se hace explícita para que instalaciones existentes adopten el
            // nuevo comportamiento sin que el usuario toque nada.
            s.close_to_tray = true;
            s.version = CONFIG_VERSION;
            // Continúa con las mismas normalizaciones que un settings al día:
            let normalized = normalize_icon_set_id(&s.icon_set);
            s.icon_set = crate::icon_set::IconSetCatalog::load(dir).resolve(&normalized);
            if !s.preview_text_exts_legacy.is_empty() && s.preview_rules.is_empty() {
                s.preview_rules = crate::preview::rules_from_csv(&s.preview_text_exts_legacy);
            }
            if s.preview_rules.is_empty() {
                s.preview_rules = crate::preview::default_preview_rules();
            }
            s.preview_text_exts_legacy.clear();
            s
        }
```

(El brazo existente `Some(mut s) if s.version == CONFIG_VERSION` queda igual, para
settings ya en v2. El brazo `Some(_)` restante captura versiones futuras/desconocidas →
default, como hoy.)

- [ ] **Step 4: Ejecutar los tests de config**

Run: `cargo test -p naygo-core config 2>&1 | tail -20`
Expected: PASS — `migracion_v1_a_v2_fuerza_close_to_tray` y los tests de config
existentes (incluido el de corrupto→default, que no se ve afectado).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/config/mod.rs
git commit -m "$(cat <<'EOF'
feat(core): la X esconde a bandeja por defecto (close_to_tray=true + migración v1→v2)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

### Task 16: Exponer "cerrar a bandeja" visible en Config

**Files:**
- Modify: la ventana de Config (`config-window.slint`) — asegurar el toggle visible con su tooltip
- Verificar: las claves i18n `slint.cfg.close_to_tray` y `slint.cfg.tip.close_to_tray` ya existen (i18n_keys.rs:354,528) en los 10 `*.json`

- [ ] **Step 1: Confirmar que el toggle está presente y visible**

El handler (`on_set_close_to_tray`, main.rs:2819) y las claves i18n ya existen. Abrir
`config-window.slint`, localizar el toggle de `close_to_tray` y confirmar que está en
una pestaña visible (General o Integración) con su tooltip `tip.close_to_tray`. Si el
toggle no estuviera renderizado (solo cableado), añadir el `CheckBox`/toggle en la
sección Integración, siguiendo el patrón de los toggles vecinos (p. ej. `tray_enabled`).

- [ ] **Step 2: Verificar textos i18n en los 10 idiomas**

Comprobar que `slint.cfg.close_to_tray` y `slint.cfg.tip.close_to_tray` tienen valor en
`crates/core/src/i18n/{de,en,es,fr,hi,it,ja,ko,pt,zh}.json`. Si falta en alguno, añadir
la traducción (no dejar la clave en inglés donde haya idioma; ver GOTCHA de no pisar
claves ya traducidas). El texto ES: "Al cerrar la ventana, mantener Naygo en la bandeja
del sistema en vez de salir."

- [ ] **Step 3: Compilar la UI**

Run: `CARGO_BUILD_JOBS=2 cargo build -p naygo-ui-slint 2>&1 | tail -15`
Expected: compila.

- [ ] **Step 4: Commit (si hubo cambios)**

```bash
git add crates/ui-slint/ui/config-window.slint crates/core/src/i18n
git commit -m "$(cat <<'EOF'
feat(ui): exponer 'cerrar a bandeja' en Config con tooltip en los 10 idiomas

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 5 — Autostart puede arrancar minimizado en bandeja

### Task 17: Flag `autostart_minimized` + arg `--tray` en el CLI (core)

**Files:**
- Modify: `crates/core/src/config/mod.rs` (campo `autostart_minimized`)
- Modify: `crates/core/src/cli.rs` (flag `--tray` en `CliArgs`/`parse_args`)
- Test: `crates/core/src/cli.rs` (módulo `tests`)

- [ ] **Step 1: Añadir el campo a `Settings`**

```rust
    /// Al iniciar con Windows (autostart), arrancar minimizado en la bandeja (sin mostrar la
    /// ventana). Solo tiene efecto si `autostart` está activo. Default true (caso pedido).
    #[serde(default = "default_autostart_minimized")]
    pub autostart_minimized: bool,
```

Y la función default:

```rust
/// Default de `autostart_minimized`: true.
fn default_autostart_minimized() -> bool {
    true
}
```

(Actualizar el `impl Default for Settings` para inicializar el campo en `true`.)

- [ ] **Step 2: Escribir el test del flag `--tray` que falla**

En `cli.rs`, módulo `tests`:

```rust
    #[test]
    fn parse_flag_tray() {
        let a = parse_args(&[s("--tray")], |_| true);
        assert!(a.tray, "--tray activa el arranque minimizado");
        // Sin el flag, por defecto false.
        assert!(!parse_args(&[s("D:\\dir")], |_| true).tray);
        // Combina con una carpeta.
        let b = parse_args(&[s("--tray"), s("D:\\dir")], |_| true);
        assert!(b.tray && b.dir == Some(PathBuf::from("D:\\dir")));
    }
```

- [ ] **Step 3: Ejecutar para ver que falla**

Run: `cargo test -p naygo-core parse_flag_tray 2>&1 | head -15`
Expected: FALLA a compilar — `CliArgs` no tiene campo `tray`.

- [ ] **Step 4: Añadir el flag `tray` a `CliArgs` y `parse_args`**

En el struct `CliArgs`, añadir `pub tray: bool,`. En `parse_args`, añadir el brazo del
match (junto a `--help`/`--version`):

```rust
            "--tray" => out.tray = true,
```

- [ ] **Step 5: Ejecutar el test**

Run: `cargo test -p naygo-core parse_flag_tray 2>&1 | head -15`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/config/mod.rs crates/core/src/cli.rs
git commit -m "$(cat <<'EOF'
feat(core): flag autostart_minimized + arg CLI --tray (arranque en bandeja)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

### Task 18: Autostart escribe `--tray`; arranque minimizado en la UI

**Files:**
- Modify: `crates/platform/src/autostart.rs` (`set_enabled` acepta args extra)
- Modify: `crates/ui-slint/src/config_ctrl.rs:508` (`set_autostart` pasa `--tray` según el flag)
- Modify: `crates/ui-slint/src/main.rs` (al arrancar: si `--tray` y `autostart_minimized`, no mostrar la ventana; sincronizar `autostart` con el registro)

- [ ] **Step 1: `set_enabled` con argumentos opcionales**

En `autostart.rs`, cambiar la firma de `set_enabled` para incluir los args a añadir al
comando (manteniendo compatibilidad de llamada mínima). En `windows_impl`:

```rust
    /// Activa/desactiva el inicio con Windows para el exe ACTUAL. `extra_args` se anexan al
    /// comando (p. ej. `["--tray"]` para arrancar minimizado). Vacío = solo el exe.
    pub fn set_enabled(on: bool, extra_args: &[&str]) -> Result<(), String> {
        // … igual que hoy hasta construir `exe` …
        // La rama `if on { … }` cambia el data escrito:
        //   let mut cmd = format!("\"{exe}\"");
        //   for a in extra_args { cmd.push(' '); cmd.push_str(a); }
        //   let data_w = wide(&cmd);
        // … resto igual …
    }
```

Actualizar también el stub no-Windows: `pub fn set_enabled(_on: bool, _extra_args: &[&str]) -> Result<(), String>`.

- [ ] **Step 2: `set_autostart` del config pasa `--tray` según el flag**

En `config_ctrl.rs:508`, donde `set_autostart` llama a `naygo_platform::autostart::set_enabled`,
pasar `--tray` cuando `autostart_minimized` esté activo:

```rust
    pub fn set_autostart(&mut self, on: bool) {
        let args: &[&str] = if self.settings.autostart_minimized { &["--tray"] } else { &[] };
        if let Err(e) = naygo_platform::autostart::set_enabled(on, args) {
            tracing::warn!("autostart: {e}");
        }
        self.settings.autostart = on;
        self.save();
    }
```

Además, añadir un setter `set_autostart_minimized(v)` que, si autostart ya está activo,
reescribe la entrada Run con/sin `--tray` para que el cambio surta efecto de inmediato:

```rust
    pub fn set_autostart_minimized(&mut self, v: bool) {
        self.settings.autostart_minimized = v;
        self.save();
        if self.settings.autostart {
            let args: &[&str] = if v { &["--tray"] } else { &[] };
            let _ = naygo_platform::autostart::set_enabled(true, args);
        }
    }
```

- [ ] **Step 3: Sincronizar `autostart` con el registro al arrancar**

En `main.rs`, tras cargar settings, sembrar `settings.autostart` desde el registro real
(fuente de verdad; cubre el caso "el instalador creó la entrada Run"):

```rust
    #[cfg(windows)]
    {
        let reg_on = naygo_platform::autostart::is_enabled();
        let mut c = ctrl.borrow_mut();
        if c.config.settings.autostart != reg_on {
            c.config.settings.autostart = reg_on;
            c.config.save();
        }
    }
```

- [ ] **Step 4: Arranque minimizado**

En `main.rs`, al parsear los args (ya se usa `parse_args_real`), leer `cli.tray`. Si
`cli.tray && settings.autostart_minimized`, no mostrar la ventana al inicio: en Slint,
en vez de `ui.run()` directo, arrancar el event loop con la ventana oculta
(`ui.window().hide()` tras crearla, o no llamar `show()`), dejando solo la bandeja
(que ya se crea en main.rs:1130). El clic en el ícono (TrayMsg::Open, ya cableado)
muestra la ventana. Concretar según la API de la versión de Slint: si `run()` fuerza
mostrar, usar `ui.window().set_minimized(true)` o esconder inmediatamente en el primer
tick cuando `cli.tray` esté activo. Anotar el mecanismo elegido en el resumen.

- [ ] **Step 5: Actualizar el único caller de `set_enabled` en tests/otros**

Buscar cualquier otra llamada a `autostart::set_enabled(` en el árbol y actualizarla a
la nueva firma (pasar `&[]` donde no aplique `--tray`).

Run: `cargo build 2>&1 | tail -25`
Expected: compila todo el workspace.

- [ ] **Step 6: Cablear el setter nuevo a la UI de Config**

Añadir el handler `on_set_autostart_minimized` en main.rs (patrón `cfg_setter!` como
los vecinos, ~2809/2819) y el toggle en `config-window.slint` (habilitado solo cuando
`autostart` está activo), con claves i18n `slint.cfg.autostart_minimized` +
`slint.cfg.tip.autostart_minimized` en los 10 idiomas. Texto ES: "Al iniciar con
Windows, arrancar minimizado en la bandeja."

Run: `CARGO_BUILD_JOBS=2 cargo build -p naygo-ui-slint 2>&1 | tail -15`
Expected: compila.

- [ ] **Step 7: Commit**

```bash
git add crates/platform/src/autostart.rs crates/ui-slint/src/config_ctrl.rs crates/ui-slint/src/main.rs crates/ui-slint/ui/config-window.slint crates/core/src/i18n
git commit -m "$(cat <<'EOF'
feat: autostart puede arrancar minimizado en bandeja (--tray) + sync con el registro

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 6 — Instalador: idiomas del wizard + idioma de la app

### Task 19: Ampliar `[Languages]` a los 7 idiomas oficiales de Inno

**Files:**
- Modify: `installer/naygo.iss:45-47`

- [ ] **Step 1: Reemplazar el bloque `[Languages]`**

```ini
[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "it"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "pt"; MessagesFile: "compiler:Languages\PortugueseBrazilian.isl"
Name: "ja"; MessagesFile: "compiler:Languages\Japanese.isl"
```

(hi/ko/zh no tienen `.isl` oficial → el wizard sale en inglés para esos; el idioma de
la app sí los ofrece, ver Task 21.)

- [ ] **Step 2: Compilar el instalador a mano para validar los .isl**

Requiere Inno Setup instalado (ISCC.exe). Run (PowerShell):
`& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\naygo.iss`
Expected: compila sin "file not found" en los `.isl` (todos son estándar de Inno 6). Si
alguno faltara en la instalación de Inno, quitar ese idioma y anotarlo.

- [ ] **Step 3: Commit**

```bash
git add installer/naygo.iss
git commit -m "$(cat <<'EOF'
feat(installer): wizard en 7 idiomas oficiales de Inno (en/es/de/fr/it/pt/ja)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

### Task 20: `[CustomMessages]` para las tareas y el paso de idioma

**Files:**
- Modify: `installer/naygo.iss` (nuevos `[CustomMessages]` + traducir las Descriptions hardcoded)

- [ ] **Step 1: Añadir `[CustomMessages]` multi-idioma**

Antes de `[Tasks]`, añadir un bloque con las cadenas nuevas en los 7 idiomas del wizard
(usar el prefijo de idioma; las que no se declaren caen al `en`). Como mínimo en `en` y
`es` (los demás pueden reusar `en` si no hay traductor de confianza para el instalador):

```ini
[CustomMessages]
en.StartupWin=Start Naygo when Windows starts
es.StartupWin=Iniciar Naygo al arrancar Windows
en.OpenWithFolders=Register Naygo in 'Open with' for folders
es.OpenWithFolders=Registrar Naygo en 'Abrir con' para carpetas
en.CtxMenuFolders=Add 'Open in Naygo' to the folder context menu
es.CtxMenuFolders=Agregar 'Abrir en Naygo' al menú contextual de carpetas
en.AppLangPage=Naygo language
es.AppLangPage=Idioma de Naygo
en.AppLangPrompt=Choose the language Naygo will start in:
es.AppLangPrompt=Elige el idioma con el que Naygo se iniciará:
```

- [ ] **Step 2: Usar los CustomMessages en `[Tasks]`**

Reemplazar las Descriptions hardcoded en español de `openwith`/`ctxmenu` por
`{cm:OpenWithFolders}` / `{cm:CtxMenuFolders}`, y añadir la tarea de autostart:

```ini
[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "startupwin"; Description: "{cm:StartupWin}"; Flags: unchecked
Name: "openwith"; Description: "{cm:OpenWithFolders}"; Flags: unchecked
Name: "ctxmenu"; Description: "{cm:CtxMenuFolders}"; Flags: unchecked
```

- [ ] **Step 3: Compilar el instalador**

Run: `& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\naygo.iss`
Expected: compila; las tareas muestran los textos traducidos según el idioma del wizard.

- [ ] **Step 4: Commit**

```bash
git add installer/naygo.iss
git commit -m "$(cat <<'EOF'
feat(installer): CustomMessages multi-idioma + tarea 'iniciar con Windows'

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

### Task 21: Autostart desde el instalador (registro) con `--tray`

**Files:**
- Modify: `installer/naygo.iss` (`[Registry]` de la clave Run condicionada a la tarea)

- [ ] **Step 1: Añadir la entrada Run condicionada a `startupwin`**

En `[Registry]`, añadir (el `--tray` para arrancar minimizado, coherente con el default
`autostart_minimized=true` de la app; la app sincroniza `Settings.autostart` desde el
registro al arrancar, Task 18 Step 3):

```ini
; Iniciar con Windows (clave Run del usuario). --tray = arrancar minimizado en bandeja.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Naygo"; ValueData: """{app}\{#MyAppExe}"" --tray"; Flags: uninsdeletevalue; Tasks: startupwin
```

- [ ] **Step 2: Compilar el instalador**

Run: `& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\naygo.iss`
Expected: compila.

- [ ] **Step 3: Commit**

```bash
git add installer/naygo.iss
git commit -m "$(cat <<'EOF'
feat(installer): tarea 'iniciar con Windows' escribe la clave Run con --tray

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

### Task 22: Paso de idioma de la app + escribir settings.json mínimo

Página personalizada con un combo de los 10 idiomas, preseleccionada por el idioma de
Windows; al terminar la instalación, si NO existe settings.json, escribe
`{ "language": "xx" }`.

**Files:**
- Modify: `installer/naygo.iss` (sección `[Code]` en Pascal Script)

- [ ] **Step 1: Añadir la sección `[Code]` con la página y el mapeo de idioma**

```pascal
[Code]
var
  LangPage: TInputOptionWizardPage;

// Los 10 idiomas de Naygo, en el mismo orden que las opciones del combo.
function NaygoLangCount(): Integer; begin Result := 10; end;
function NaygoLangId(Index: Integer): String;
begin
  case Index of
    0: Result := 'en';
    1: Result := 'es';
    2: Result := 'de';
    3: Result := 'fr';
    4: Result := 'it';
    5: Result := 'pt';
    6: Result := 'ja';
    7: Result := 'hi';
    8: Result := 'ko';
    9: Result := 'zh';
  else Result := 'en';
  end;
end;

// Mapea el LangID primario de Windows (GetUILanguage & $3FF) a un índice de NaygoLangId.
function DetectWindowsLangIndex(): Integer;
var prim: Integer;
begin
  prim := GetUILanguage() and $3FF;
  case prim of
    $09: Result := 0; // inglés
    $0A: Result := 1; // español
    $07: Result := 2; // alemán
    $0C: Result := 3; // francés
    $10: Result := 4; // italiano
    $16: Result := 5; // portugués
    $11: Result := 6; // japonés
    $39: Result := 7; // hindi
    $12: Result := 8; // coreano
    $04: Result := 9; // chino
  else Result := 0;
  end;
end;

procedure InitializeWizard();
var i: Integer;
begin
  LangPage := CreateInputOptionPage(wpSelectTasks,
    ExpandConstant('{cm:AppLangPage}'), '',
    ExpandConstant('{cm:AppLangPrompt}'), True, False);
  LangPage.Add('English');
  LangPage.Add('Español');
  LangPage.Add('Deutsch');
  LangPage.Add('Français');
  LangPage.Add('Italiano');
  LangPage.Add('Português');
  LangPage.Add('日本語');
  LangPage.Add('हिन्दी');
  LangPage.Add('한국어');
  LangPage.Add('中文');
  i := DetectWindowsLangIndex();
  LangPage.SelectedValueIndex := i;
end;

// Tras copiar archivos: si NO existe settings.json, escribir uno mínimo con el idioma.
procedure CurStepChanged(CurStep: TSetupStep);
var
  path, content, lang: String;
begin
  if CurStep = ssPostInstall then
  begin
    path := ExpandConstant('{app}\settings.json');
    if not FileExists(path) then
    begin
      lang := NaygoLangId(LangPage.SelectedValueIndex);
      content := '{' + #13#10 + '  "version": 2,' + #13#10 +
                 '  "language": "' + lang + '"' + #13#10 + '}';
      SaveStringToFile(path, content, False);
    end;
  end;
end;
```

Nota: el settings.json mínimo incluye `"version": 2` (el `CONFIG_VERSION` nuevo) para
que la app lo tome como al día y complete el resto por defaults sin re-migrar. Si el
esquema exige más campos obligatorios, la app los completa vía `#[serde(default)]` (el
loader es tolerante); `language` y `version` bastan.

- [ ] **Step 2: Compilar el instalador**

Run: `& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\naygo.iss`
Expected: compila sin errores de Pascal Script.

- [ ] **Step 3: Verificar el campo `version` esperado por el loader**

Confirmar que `CONFIG_VERSION` en `crates/core/src/config/mod.rs` es `2` (Task 15). El
`"version": 2` del settings mínimo debe coincidir, para que la app no lo trate como
incompatible. Si en el futuro sube, actualizar este número en el `.iss`.

- [ ] **Step 4: Commit**

```bash
git add installer/naygo.iss
git commit -m "$(cat <<'EOF'
feat(installer): paso de idioma de la app (10 idiomas) preseleccionado por el SO

Escribe un settings.json mínimo con el idioma elegido solo si no existe,
para no pisar la config de una reinstalación.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 7 — Integración final

### Task 23: Suite completa + regenerar dist

**Files:** —

- [ ] **Step 1: Suite completa del workspace**

Run: `cargo test 2>&1 | tail -30`
Expected: todos los tests pasan (core + ui-slint + platform). Prestar atención a los
tests de layout migrados y a los de config (migración v1→v2).

- [ ] **Step 2: Clippy**

Run: `cargo clippy --all-targets 2>&1 | tail -30`
Expected: sin warnings nuevos (correr uno mismo tras cualquier subagente, GOTCHA
conocido).

- [ ] **Step 3: Regenerar dist (portable + instalador)**

Run: `powershell -File scripts/build-release.ps1`
Expected: genera el portable y el instalador en `dist/` con la versión actual. (Nicolás
prueba desde `dist/`; regenerar SIEMPRE tras cambios de código.)

- [ ] **Step 4: Commit final (si el build tocó archivos versionados como dist manifest)**

```bash
git add -A
git commit -m "$(cat <<'EOF'
chore: regenerar dist (portable + instalador) del lote ventana/paneles/instalador

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
EOF
)"
```

---

## Verificación visual pendiente (Nicolás, en la VM)

Estos puntos NO se pueden verificar por tests; requieren la VM:

- [ ] Mover un divisor externo: los paneles NO adyacentes quedan fijos; los vecinos se
  reparten. Al redimensionar la ventana, todo escala proporcional.
- [ ] Doble-clic en un divisor → 50/50 de sus dos vecinos.
- [ ] Cursor ↔/↕ y resaltado de acento al pasar/arrastrar un divisor.
- [ ] Cerrar maximizado y reabrir el portable → abre maximizado. Cerrar en tamaño
  ventana y reabrir → mismo tamaño y posición. Desconectar un monitor y reabrir → la
  ventana aparece centrada y visible.
- [ ] Presionar la X → la app se esconde en la bandeja (no se cierra). Menú bandeja →
  Salir cierra de verdad.
- [ ] Activar "iniciar con Windows" + "arrancar minimizado" → reiniciar Windows → Naygo
  aparece solo en la bandeja.
- [ ] Instalador: wizard en varios idiomas; paso de idioma de la app preseleccionado
  según el idioma de Windows; marcar "iniciar con Windows"; primera ejecución en el
  idioma elegido.

---

## Notas de cobertura (self-review)

- **D (divisores)**: Fase 1 (Tasks 1-8 core) + Task 9 (UI). ✓
- **E1 (doble-clic 50/50)**: Task 10. ✓  **E2 (feedback visual)**: Task 11. ✓
  **E3 (restaurar layout)**: cubierto por la migración serde de la Task 8 + persistencia
  existente (`session_persist`/`load_session`); sin código nuevo, verificado en Task 23. ✓
- **C (geometría de ventana)**: Fase 3 (Tasks 12-14). ✓
- **B3 (X→bandeja)**: Fase 4 (Tasks 15-16). ✓
- **B2 (autostart minimizado)**: Fase 5 (Tasks 17-18). ✓
- **B1 (instalador autostart)**: Task 21. ✓
- **A (instalador idiomas)**: Tasks 19-22. ✓
