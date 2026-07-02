# Rediseño visual del panel de operaciones — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rediseñar visualmente el panel de operaciones de archivo (`ops-panel.slint`) para que se vea de última generación —íconos de tipo, % protagonista, datos en mini-tarjetas, botones sin caja, más aire, coherente en las 4 zonas— con animaciones baratas siempre activas (0% CPU en reposo) y un brillo continuo opcional tras un flag de config global.

**Architecture:** Casi todo vive en `crates/ui-slint/ui/ops-panel.slint` (componentes Slint puros). Se añade un campo `op_kind: i32` al VM `OpRowVm` (poblado desde el `OpKind`/`plan_kind` que Rust ya conoce) para pintar el ícono y el verbo de tipo. Un flag `animations_enabled` en `core::config` (default false) gobierna las animaciones costosas. Ningún callback ni modelo de datos cambia; los tests existentes del panel deben pasar sin tocar.

**Tech Stack:** Rust, Slint 1.16 (backend winit, render por software), serde, i18n JSON por clave en 10 idiomas.

**Convenciones (leer antes de empezar):**
- PowerShell 5.1: NO usar `&&` ni `||`; usar `;` o líneas separadas. Compilar UI con `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint`.
- Header Naygo + SPDX solo en archivos NUEVOS (aquí no se crean archivos nuevos; no tocar headers existentes).
- Comentarios en español NEUTRAL (tú/impersonal, NUNCA voseo: nada de "podés/tenés/hacés/fijate").
- Nombres de código en inglés. Íconos con `Path` (no glifos). Colores SIEMPRE de `theme.slint` (`Theme.accent`, `Theme.row-bg`, `Theme.panel-bg`, `Theme.text`, `Theme.text-dim`, `Theme.border`, `Theme.selection-bg`, `Theme.error`, `Theme.highlight`); no hardcodear hex.
- i18n: texto visible SIEMPRE por clave, en los 10 JSON. El test `todos_los_idiomas_tienen_las_claves_de_es` exige paridad. Tests que aseveran texto comparan contra la CLAVE, nunca el literal español (CI corre en inglés).
- GIT: solo `git add <rutas exactas>` + `git commit`. PROHIBIDO reset/restore/stash/checkout/clean, `add -A`, `add .`, `git rm`, push, `commit --amend`. NO tocar `CLAUDE.md` ni `favorites.rs`.
- Clippy con `-D warnings` (lo exige el CI). `cargo fmt` antes de cada commit.

---

## Fase 1 — Flag de config `animations_enabled`

### Task 1: Añadir el flag `animations_enabled` a `core::config`

**Files:**
- Modify: `crates/core/src/config/mod.rs`

Contexto: los flags booleanos de `Settings` siguen un patrón fijo. Referencias existentes:
`size_no_subdirs` (campo en ~224-225, default fn en ~475-477, init de fábrica en ~530, init de
test en ~823) y `auto_highlight_code` (campo ~297-298, default fn ~359-361, init fábrica ~548,
init test ~857). Se replica ese patrón para `animations_enabled` (default **false**).

- [ ] **Step 1: Escribir el test del default**

Buscar en `crates/core/src/config/mod.rs` el módulo `#[cfg(test)]` y añadir:

```rust
    #[test]
    fn animations_enabled_default_es_false() {
        let s = Settings::default();
        assert!(!s.animations_enabled, "las animaciones costosas vienen apagadas");
    }
```

- [ ] **Step 2: Ejecutar para ver que falla**

Run: `cargo test -p naygo-core animations_enabled_default_es_false 2>&1 | tail -8`
Expected: FALLA a compilar (el campo `animations_enabled` no existe).

- [ ] **Step 3: Añadir el campo, su default y los inits**

En el struct `Settings`, junto a `auto_highlight_code`:

```rust
    /// Activa animaciones adicionales de mayor costo (p. ej. el brillo continuo de la barra del
    /// panel de operaciones). Flag GLOBAL: pensado para gobernar cualquier animación costosa
    /// futura de la app. Las transiciones baratas (interpolar un valor que cambió) NO dependen de
    /// esto: son siempre activas y no cuestan CPU en reposo. Default false (prioridad: bajo consumo).
    #[serde(default = "default_animations_enabled")]
    pub animations_enabled: bool,
```

La función default (junto a `default_auto_highlight_code`):

```rust
/// Default de `animations_enabled`: false (las animaciones costosas vienen apagadas por consumo).
fn default_animations_enabled() -> bool {
    false
}
```

En el init de fábrica de `Settings` (el que setea `auto_highlight_code: true,`), añadir:

```rust
            animations_enabled: false,
```

En el init de test de `Settings` (el bloque de test que setea `auto_highlight_code: false,` alrededor de la línea 857), añadir también:

```rust
            animations_enabled: true,
```
(en el bloque de test se usa un valor distinto al default a propósito, para verificar el round-trip; si ese bloque de test no setea todos los campos explícitamente sino que usa `..Default::default()`, entonces NO añadir nada ahí — comprobar la forma real del bloque antes de editar).

- [ ] **Step 4: Ejecutar el test**

Run: `cargo test -p naygo-core animations_enabled 2>&1 | tail -8`
Expected: PASS.

- [ ] **Step 5: Verificar que el resto de config sigue verde**

Run: `cargo test -p naygo-core config 2>&1 | grep "test result:"`
Expected: ok, 0 failed.

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy -p naygo-core --all-targets 2>&1 | tail -4`
Expected: sin warnings.

```bash
cargo fmt -p naygo-core
git add crates/core/src/config/mod.rs
git commit -m "$(cat <<'EOF'
feat(core): flag global animations_enabled (default off) para animaciones costosas

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 2 — `op_kind` en el VM de filas de operación

### Task 2: Añadir `op-kind` a `OpRowVm` y poblarlo desde Rust

**Files:**
- Modify: `crates/ui-slint/ui/types.slint` (struct `OpRowVm`, ~293-314)
- Modify: `crates/ui-slint/src/ops_ctrl.rs` (struct `OpRowData` ~2049; construcción de filas ~1516-1539 y las demás ramas que arman `OpRowData` para cola/historial/planning; enum `OpKind` ~185)

Contexto: `OpRowData` (Rust, ops_ctrl.rs:2049) se mapea 1:1 a `OpRowVm` (Slint, types.slint:293).
Cada op activa conoce su tipo por `o.plan_kind: OpKind` (el enum `OpKind` está en ops_ctrl.rs, con
variantes `Copy`, `Move`, `Delete { .. }`, `Compress { .. }`, `Extract`, y quizá otras — verificar
la lista completa leyendo el enum antes de mapear). Se añade un `op_kind: i32` a ambos structs con
la convención: **0=copiar, 1=mover, 2=borrar, 3=comprimir, 4=extraer, 5=otro**.

- [ ] **Step 1: Añadir el campo al struct Slint**

En `crates/ui-slint/ui/types.slint`, dentro de `OpRowVm` (tras `kind: int,` en la línea ~309):

```slint
    op-kind: int,          // tipo de operación para el ícono/verbo: 0=copiar 1=mover 2=borrar 3=comprimir 4=extraer 5=otro
```

- [ ] **Step 2: Añadir el campo al struct Rust**

En `crates/ui-slint/src/ops_ctrl.rs`, dentro de `pub struct OpRowData` (tras `pub kind: i32,` ~2066):

```rust
    /// Tipo de operación para el ícono/verbo del panel: 0=copiar 1=mover 2=borrar 3=comprimir 4=extraer 5=otro.
    pub op_kind: i32,
```

- [ ] **Step 3: Helper para mapear OpKind → i32**

En `crates/ui-slint/src/ops_ctrl.rs`, añadir una función libre (o método) cerca del enum `OpKind`.
Ajustar los brazos a las variantes REALES del enum (leerlas primero); ejemplo con las conocidas:

```rust
/// Mapea el tipo de operación al código que consume el panel (`OpRowVm.op-kind`).
/// 0=copiar 1=mover 2=borrar 3=comprimir 4=extraer 5=otro.
pub fn op_kind_code(kind: &OpKind) -> i32 {
    match kind {
        OpKind::Copy => 0,
        OpKind::Move => 1,
        OpKind::Delete { .. } => 2,
        OpKind::Compress { .. } => 3,
        OpKind::Extract => 4,
        // Cualquier variante restante (Paste u otras) cae en "otro".
        _ => 5,
    }
}
```

- [ ] **Step 4: Poblar `op_kind` en TODAS las ramas que construyen `OpRowData`**

En la construcción de filas EN CURSO (ops_ctrl.rs ~1516-1539) añadir dentro del literal `OpRowData { ... }`:

```rust
                    op_kind: op_kind_code(&o.plan_kind),
```

Repetir en las OTRAS ramas que construyen `OpRowData` (cola, historial, planning). Localizarlas
buscando `OpRowData {` en el archivo. Para cada una, usar el `plan_kind`/`kind` de la op que esa
rama tenga a mano. Si alguna rama de HISTORIAL ya no tiene el `OpKind` disponible (op terminada y
descartada), usar el que guarde el registro de historial; si el historial NO conserva el tipo,
poblar `op_kind: 5` (otro) en esa rama y anotarlo como limitación (el ícono de historial usa el de
resultado ✓/⚠, no el de tipo — ver Task 6, así que 5 es aceptable ahí).

- [ ] **Step 5: Compilar (valida el mapeo Rust↔Slint)**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -8`
Expected: compila. Si Slint se queja de que falta poblar `op-kind` en algún `OpRowVm`, buscar
dónde se convierte `OpRowData`→`OpRowVm` (en `main.rs` o un `to_op_row_vm`) y añadir
`op-kind: d.op_kind,` en el mapeo.

- [ ] **Step 6: Tests + clippy + commit**

Run: `cargo test -p naygo-ui-slint 2>&1 | grep "test result:"`
Expected: ok, 0 failed (los tests del panel siguen verdes: no cambió ningún callback).
Run: `cargo clippy -p naygo-ui-slint --all-targets 2>&1 | tail -4`
Expected: sin warnings.

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/ui/types.slint crates/ui-slint/src/ops_ctrl.rs
git commit -m "$(cat <<'EOF'
feat(ui): op-kind en OpRowVm (tipo de operación) para el ícono y el verbo del panel

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 3 — Reescritura visual de `ops-panel.slint`

> Toda esta fase es un solo archivo: `crates/ui-slint/ui/ops-panel.slint`. Se reescriben los
> componentes existentes conservando sus `in property` y `callback` (para no romper `OpsPanel`).
> Colores solo de `Theme`. Compilar tras cada task para no acumular errores de Slint.

### Task 3: Íconos de tipo (Path) + chip contenedor

**Files:**
- Modify: `crates/ui-slint/ui/ops-panel.slint`

Contexto: hoy el archivo ya define íconos con `Path` (`PauseIcon`, `PlayIcon`, `SkipIcon`,
`CancelIcon`). Se añaden íconos de TIPO de operación con el mismo estilo, y un chip que los envuelve.

- [ ] **Step 1: Añadir los íconos de tipo (Path)**

Tras `CancelIcon` (línea ~98) añadir cinco íconos simples con `Path` (viewbox 12×12), estilo
coherente con los existentes. Ejemplo (ajustar trazos a gusto, mantener 12×12 y `tint`):

```slint
// Copiar: dos hojas superpuestas.
component CopyIcon inherits Rectangle {
    in property <color> tint: Theme.text;
    in property <length> size: 14px;
    Path {
        width: root.size; height: root.size;
        x: (parent.width - self.width) / 2; y: (parent.height - self.height) / 2;
        stroke: root.tint; stroke-width: 1.4px; fill: transparent;
        viewbox-width: 12; viewbox-height: 12;
        MoveTo { x: 4; y: 4; } LineTo { x: 4; y: 1.5; } LineTo { x: 10.5; y: 1.5; } LineTo { x: 10.5; y: 8; } LineTo { x: 8; y: 8; }
        MoveTo { x: 1.5; y: 4; } LineTo { x: 8; y: 4; } LineTo { x: 8; y: 10.5; } LineTo { x: 1.5; y: 10.5; } Close { }
    }
}
// Mover: flecha hacia la derecha.
component MoveIcon inherits Rectangle {
    in property <color> tint: Theme.text;
    in property <length> size: 14px;
    Path {
        width: root.size; height: root.size;
        x: (parent.width - self.width) / 2; y: (parent.height - self.height) / 2;
        stroke: root.tint; stroke-width: 1.4px; fill: transparent;
        viewbox-width: 12; viewbox-height: 12;
        MoveTo { x: 1.5; y: 6; } LineTo { x: 10.5; y: 6; } MoveTo { x: 7; y: 2.5; } LineTo { x: 10.5; y: 6; } LineTo { x: 7; y: 9.5; }
    }
}
// Borrar: papelera simple.
component DeleteIcon inherits Rectangle {
    in property <color> tint: Theme.text;
    in property <length> size: 14px;
    Path {
        width: root.size; height: root.size;
        x: (parent.width - self.width) / 2; y: (parent.height - self.height) / 2;
        stroke: root.tint; stroke-width: 1.4px; fill: transparent;
        viewbox-width: 12; viewbox-height: 12;
        MoveTo { x: 2.5; y: 3; } LineTo { x: 9.5; y: 3; } MoveTo { x: 3.5; y: 3; } LineTo { x: 4; y: 10.5; } LineTo { x: 8; y: 10.5; } LineTo { x: 8.5; y: 3; } MoveTo { x: 5; y: 3; } LineTo { x: 5; y: 1.5; } LineTo { x: 7; y: 1.5; } LineTo { x: 7; y: 3; }
    }
}
// Comprimir: caja con cremallera (una línea vertical punteada como trazos).
component CompressIcon inherits Rectangle {
    in property <color> tint: Theme.text;
    in property <length> size: 14px;
    Path {
        width: root.size; height: root.size;
        x: (parent.width - self.width) / 2; y: (parent.height - self.height) / 2;
        stroke: root.tint; stroke-width: 1.4px; fill: transparent;
        viewbox-width: 12; viewbox-height: 12;
        MoveTo { x: 2; y: 1.5; } LineTo { x: 10; y: 1.5; } LineTo { x: 10; y: 10.5; } LineTo { x: 2; y: 10.5; } Close { }
        MoveTo { x: 6; y: 2; } LineTo { x: 6; y: 4; } MoveTo { x: 6; y: 5.5; } LineTo { x: 6; y: 7.5; }
    }
}
// Extraer: caja con flecha saliendo hacia arriba.
component ExtractIcon inherits Rectangle {
    in property <color> tint: Theme.text;
    in property <length> size: 14px;
    Path {
        width: root.size; height: root.size;
        x: (parent.width - self.width) / 2; y: (parent.height - self.height) / 2;
        stroke: root.tint; stroke-width: 1.4px; fill: transparent;
        viewbox-width: 12; viewbox-height: 12;
        MoveTo { x: 2; y: 6; } LineTo { x: 2; y: 10.5; } LineTo { x: 10; y: 10.5; } LineTo { x: 10; y: 6; }
        MoveTo { x: 6; y: 8; } LineTo { x: 6; y: 1.5; } MoveTo { x: 3.5; y: 4; } LineTo { x: 6; y: 1.5; } LineTo { x: 8.5; y: 4; }
    }
}
```

- [ ] **Step 2: Añadir el chip contenedor de ícono de tipo**

```slint
// Chip cuadrado-redondeado que envuelve el ícono del tipo de operación (identidad visual de la
// tarjeta). Elige el ícono según `op-kind` (0=copiar 1=mover 2=borrar 3=comprimir 4=extraer 5=otro).
component OpKindChip inherits Rectangle {
    in property <int> op-kind;
    in property <length> box: 30px;
    width: root.box; height: root.box;
    border-radius: 8px;
    background: Theme.selection-bg;
    if root.op-kind == 0: CopyIcon { tint: Theme.accent; }
    if root.op-kind == 1: MoveIcon { tint: Theme.accent; }
    if root.op-kind == 2: DeleteIcon { tint: Theme.accent; }
    if root.op-kind == 3: CompressIcon { tint: Theme.accent; }
    if root.op-kind == 4: ExtractIcon { tint: Theme.accent; }
    if root.op-kind == 5: CopyIcon { tint: Theme.text-dim; }
}
```

- [ ] **Step 3: Compilar**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -8`
Expected: compila (los componentes nuevos aún no se usan; solo se declaran).

- [ ] **Step 4: Commit**

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/ui/ops-panel.slint
git commit -m "$(cat <<'EOF'
feat(ui): íconos de tipo de operación (Path) + chip contenedor en el panel de ops

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

### Task 4: Botones sin caja + barra de progreso píldora + mini-tarjeta de dato

**Files:**
- Modify: `crates/ui-slint/ui/ops-panel.slint`

Contexto: se reescriben `OpButton` y `ProgressBar` (existentes) y se añade `DataCell`
(reemplaza el uso de `DataLine`). Se conservan las mismas `in property`/`callback` de `OpButton`
para no romper a los llamadores.

- [ ] **Step 1: Reescribir `OpButton` sin borde de caja**

Reemplazar el componente `OpButton` (líneas ~100-140) por una versión sin borde, con tres estilos
según un `variant`. Mantener TODAS las properties actuales (`label`, `danger`, `pressed`,
`icon-size`, `show-pause/play/skip/cancel`) y añadir `variant`:

```slint
// Botón de control sin caja. `variant`: 0=primario (chip con fondo), 1=secundario (solo texto,
// fondo en hover), 2=solo-ícono (para Cancelar; el `label` se ignora y va como tooltip afuera).
component OpButton inherits Rectangle {
    in property <string> label;
    in property <bool> danger: false;
    in property <int> variant: 0;
    callback pressed();
    in property <length> icon-size: 13px;
    in property <bool> show-pause: false;
    in property <bool> show-play: false;
    in property <bool> show-skip: false;
    in property <bool> show-cancel: false;

    height: 30px;
    border-radius: 8px;
    background: root.variant == 0
        ? (btn-touch.has-hover ? Theme.selection-bg.darker(0.15) : Theme.selection-bg)
        : (btn-touch.has-hover ? Theme.selection-bg : transparent);

    HorizontalLayout {
        padding-left: root.variant == 2 ? 6px : 12px;
        padding-right: root.variant == 2 ? 6px : 12px;
        spacing: 6px;
        alignment: center;
        if root.show-pause: PauseIcon { width: root.icon-size; height: root.icon-size; tint: Theme.text; }
        if root.show-play: PlayIcon { width: root.icon-size; height: root.icon-size; tint: Theme.accent; }
        if root.show-skip: SkipIcon { width: root.icon-size; height: root.icon-size; tint: Theme.text-dim; }
        if root.show-cancel: CancelIcon { width: root.icon-size; height: root.icon-size; tint: Theme.error; }
        if root.variant != 2 && root.label != "": Text {
            text: root.label;
            color: root.variant == 1 ? Theme.text-dim : Theme.text;
            font-size: 12px;
            vertical-alignment: center;
        }
    }
    btn-touch := TouchArea {
        mouse-cursor: pointer;
        clicked => { root.pressed(); }
    }
}
```

Nota: `.darker(0.15)` es una función de color válida de Slint sobre un `brush`; si el compilador se
queja con `Theme.selection-bg` (por ser property de una global), usar `Theme.selection-bg` a secas
para hover y `transparent`/`Theme.row-bg` para reposo — ajustar al compilar. No hardcodear hex.

- [ ] **Step 2: Reescribir `ProgressBar` estilo píldora (sin borde, con slot de brillo opcional)**

Reemplazar `ProgressBar` (~154-171) por:

```slint
// Barra de progreso estilo píldora. `percent` en 0..100. `glow` enciende el brillo continuo
// (solo si el usuario activó animaciones; lo decide el llamador). El relleno se ANIMA al cambiar.
component ProgressBar inherits Rectangle {
    in property <float> percent;
    in property <color> fill: Theme.accent;
    in property <bool> glow: false;
    height: 6px;
    background: Theme.panel-bg;
    border-radius: self.height / 2;
    clip: true;
    fill-rect := Rectangle {
        x: 0; y: 0;
        width: parent.width * (max(0, min(100, root.percent)) / 100);
        height: parent.height;
        background: root.fill;
        border-radius: self.height / 2;
        // Transición barata: al cambiar el % la barra se desliza suave (no salta). Solo redibuja
        // durante la animación; en reposo, 0 costo.
        animate width { duration: 200ms; easing: ease-out; }
    }
    // Brillo continuo OPCIONAL (Task 7 lo enciende con el flag). Se declara aquí como slot vacío;
    // la animación real se añade en Task 7.
}
```

- [ ] **Step 3: Añadir `DataCell` (mini-tarjeta etiqueta-arriba / valor-derecha)**

Tras `ProgressBar`, añadir (y dejar `DataLine` en el archivo por si otro sitio lo usa; si no lo usa
nadie más, se puede borrar — comprobar con búsqueda):

```slint
// Mini-tarjeta de un dato: etiqueta arriba-izquierda (tenue) + valor abajo-derecha (destacado).
// La unidad va en `unit` (tenue, inline tras el valor). Fondo derivado del panel para separarse
// de la tarjeta contenedora.
component DataCell inherits Rectangle {
    in property <string> caption;
    in property <string> value;
    in property <string> unit;
    background: Theme.panel-bg;
    border-radius: 8px;
    VerticalLayout {
        padding: 8px;
        spacing: 3px;
        Text {
            text: root.caption;
            color: Theme.text-dim;
            font-size: 11px;
            overflow: elide;
        }
        HorizontalLayout {
            Rectangle { horizontal-stretch: 1; }
            Text {
                text: root.value;
                color: Theme.text;
                font-size: 13px;
                font-weight: 500;
                vertical-alignment: center;
            }
            if root.unit != "": Text {
                text: " " + root.unit;
                color: Theme.text-dim;
                font-size: 11px;
                vertical-alignment: center;
            }
        }
    }
}
```

- [ ] **Step 4: Compilar**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -12`
Expected: compila. `OpCard`/`PlanningCard`/etc. aún usan las properties viejas de `OpButton`
(siguen válidas). Si `animate width`/`ease-out` da error de sintaxis, revisar: la forma Slint es
`animate width { duration: 200ms; easing: ease-out; }` dentro del elemento cuya `width` cambia.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/ui/ops-panel.slint
git commit -m "$(cat <<'EOF'
feat(ui): botones sin caja + barra píldora animada + mini-tarjeta de dato (ops)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

### Task 5: Reescribir la tarjeta EN CURSO (`OpCard`)

**Files:**
- Modify: `crates/ui-slint/ui/ops-panel.slint`

Contexto: `OpCard` (líneas ~195-301) usa hoy `DataLine` apiladas y `OpButton` con caja. Se reescribe
con: cabecera (chip + verbo + archivo + % grande), barra, grilla 3+2 de `DataCell`, controles sin
caja. Se conservan las `in property <OpRowVm> op` y los callbacks `pause/resume/skip/cancel(int)`.
Se recibe además `animations-enabled` para pasar `glow` a la barra (Task 7 la usa; aquí ya se cablea).

- [ ] **Step 1: Añadir el verbo por tipo como función de i18n**

La cabecera muestra un verbo en gerundio ("Copiando…"). En vez de `op.label` (sustantivo), se usa
una property de `Tr` por tipo. Las claves se crean en Task 8; aquí se asume que existirán
`Tr.ops-verb-copy`, `Tr.ops-verb-move`, `Tr.ops-verb-delete`, `Tr.ops-verb-compress`,
`Tr.ops-verb-extract` y un genérico `Tr.ops-verb-other`. En `OpCard` se elige por `op.op-kind`.

- [ ] **Step 2: Reescribir el cuerpo de `OpCard`**

Reemplazar el `VerticalLayout` interno de `OpCard` (todo el bloque `VerticalLayout { padding: 9px; ... }`)
por:

```slint
    in property <bool> animations-enabled: false;

    background: Theme.row-bg;
    border-radius: 12px;

    VerticalLayout {
        padding: 14px;
        spacing: 12px;

        // Cabecera: chip de tipo + verbo/archivo + % grande.
        HorizontalLayout {
            spacing: 9px;
            OpKindChip { op-kind: root.op.op-kind; y: (parent.height - self.height) / 2; }
            VerticalLayout {
                horizontal-stretch: 1;
                spacing: 1px;
                Text {
                    text: root.op.op-kind == 0 ? Tr.ops-verb-copy
                        : root.op.op-kind == 1 ? Tr.ops-verb-move
                        : root.op.op-kind == 2 ? Tr.ops-verb-delete
                        : root.op.op-kind == 3 ? Tr.ops-verb-compress
                        : root.op.op-kind == 4 ? Tr.ops-verb-extract
                        : Tr.ops-verb-other;
                    color: Theme.text;
                    font-size: 13px;
                    font-weight: 500;
                    overflow: elide;
                }
                if root.op.current-file != "": Text {
                    text: root.op.current-file;
                    color: Theme.text-dim;
                    font-size: 12px;
                    overflow: elide;
                }
            }
            HorizontalLayout {
                spacing: 1px;
                y: (parent.height - self.height) / 2;
                Text {
                    text: round(root.op.percent);
                    color: Theme.text;
                    font-size: 20px;
                    font-weight: 500;
                    vertical-alignment: center;
                }
                Text {
                    text: "%";
                    color: Theme.text-dim;
                    font-size: 13px;
                    vertical-alignment: bottom;
                }
            }
        }

        // Barra de progreso (amarilla si pausada, acento si avanza). `glow` solo si el usuario
        // activó animaciones (brillo continuo — Task 7).
        ProgressBar {
            percent: root.op.percent;
            fill: root.op.paused ? Theme.highlight : Theme.accent;
            glow: root.animations-enabled && !root.op.paused;
        }

        // Datos en grilla 3+2. Fila de 3: transferido · velocidad · pico.
        HorizontalLayout {
            spacing: 8px;
            DataCell {
                horizontal-stretch: 1;
                caption: Tr.ops-copied;
                value: root.op.bytes-done;
                unit: "/ " + root.op.bytes-total;
            }
            DataCell {
                horizontal-stretch: 1;
                caption: Tr.ops-speed;
                value: root.op.speed != "" ? root.op.speed : "—";
            }
            DataCell {
                horizontal-stretch: 1;
                caption: Tr.ops-peak;
                value: root.op.speed-peak != "" ? root.op.speed-peak : "—";
            }
        }
        // Fila de 2: transcurrido · restante.
        HorizontalLayout {
            spacing: 8px;
            DataCell {
                horizontal-stretch: 1;
                caption: Tr.ops-elapsed;
                value: root.op.elapsed != "" ? root.op.elapsed : "—";
            }
            DataCell {
                horizontal-stretch: 1;
                caption: Tr.ops-remaining;
                value: root.op.eta != "" ? root.op.eta : "—";
            }
        }

        // Controles: Pausar (chip) · Saltar (texto) · espaciador · X cancelar (ícono + tooltip).
        HorizontalLayout {
            spacing: 8px;
            OpButton {
                variant: 0;
                label: root.op.paused ? Tr.ops-resume : Tr.ops-pause;
                show-pause: !root.op.paused;
                show-play: root.op.paused;
                pressed => {
                    if (root.op.paused) { root.resume(root.op.index); }
                    else { root.pause(root.op.index); }
                }
            }
            OpButton {
                variant: 1;
                label: Tr.ops-skip;
                show-skip: true;
                pressed => { root.skip(root.op.index); }
            }
            Rectangle { horizontal-stretch: 1; }
            cancel-btn := OpButton {
                variant: 2;
                danger: true;
                show-cancel: true;
                pressed => { root.cancel(root.op.index); }
            }
        }
    }
```

Nota tooltip del cancelar: si el proyecto tiene un patrón de tooltip propio para íconos (buscar
`hover-tip`/`tooltip` en otros .slint), aplicarlo a `cancel-btn` mostrando `Tr.ops-cancel`. Si no
hay uno trivial, dejar el ícono sin tooltip (aceptable: la X es universal); no bloquear por esto.

- [ ] **Step 3: Compilar**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -15`
Expected: FALLA porque `Tr.ops-verb-*` aún no existen. Si es el único error, continuar a Task 8
para crearlas y volver; o crear PRIMERO las claves (recomendado: hacer Task 8 antes que 5). Para
mantener el orden, este plan hace Task 8 (i18n) ANTES de compilar OpCard — ver nota al final de la
fase. Alternativamente, comentar temporalmente el uso de `Tr.ops-verb-*` con `op.label` para
compilar y validar el layout, luego revertir en Task 8.

- [ ] **Step 4: Commit (tras compilar OK, posiblemente después de Task 8)**

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/ui/ops-panel.slint
git commit -m "$(cat <<'EOF'
feat(ui): tarjeta EN CURSO rediseñada (chip, % grande, grilla 3+2, botones sin caja)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

### Task 6: Reescribir CALCULANDO, EN COLA, HISTORIAL y el header

**Files:**
- Modify: `crates/ui-slint/ui/ops-panel.slint`

- [ ] **Step 1: `PlanningCard` con el nuevo lenguaje**

Reemplazar el cuerpo de `PlanningCard` (~306-364): `padding: 14px; border-radius: 12px;`, cabecera
con `OpKindChip { op-kind: root.op.op-kind; }` + verbo + a la derecha el `ScanDot` (se conserva) +
`Tr.ops-calculating`; contador `op.status` debajo; controles con la X cancelar `variant: 2`.
Mantener la property `op` y el callback `cancel(int)`.

- [ ] **Step 2: `QueuedRow` compacta con ícono**

Reemplazar `QueuedRow` (~367-400): quitar `border-width`/`border-color`, `border-radius: 8px`,
`height: 34px`. Contenido: un ícono de tipo pequeño (usar `OpKindChip { op-kind: root.op.op-kind; box: 22px; }`
o directamente el ícono a 14px sin chip) + `op.label` + "— " + `Tr.ops-waiting` (stretch) +
`OpButton { variant: 2; danger: true; show-cancel: true; }`. Mantener property y callback.

- [ ] **Step 3: Íconos de resultado para HISTORIAL**

Añadir dos íconos `Path` (junto a los otros): `DoneIcon` (check, `tint: Theme.accent`) y
`FailIcon` (triángulo con `!`, `tint: Theme.error`). El VM no distingue hoy éxito/fallo por un bool
explícito; se detecta si `op.status` sugiere fallo. Como el status es texto ya formateado y
traducido, NO parsear el texto. En su lugar: mostrar SIEMPRE `DoneIcon` en historial (el detalle de
fallos ya va en el `status`/enlace). Si se quiere distinguir, añadir en Task 2bis un `failed: bool`
al VM — PERO eso es scope extra; para este plan, historial usa `DoneIcon` fijo y su color es
`Theme.text-dim` (neutro), evitando afirmar éxito/fallo con el ícono. Documentarlo en el commit.

- [ ] **Step 4: `HistoryRow` con ícono neutro**

Reescribir `HistoryRow` (~405-462) conservando el alto adaptable (28/44px), property `op` y callback
`show-files`. Añadir al inicio de la primera línea un íconito neutro (`DoneIcon { tint: Theme.text-dim; }`
a 14px) antes de la etiqueta. Mantener la segunda línea (nombres inline o enlace "Ver N archivos").

- [ ] **Step 5: Header de `OpsPanel` con chip-contador**

En `OpsPanel` (bloque del header, ~502-532): mantener el título `Tr.ops-title`. Reemplazar el texto
de resumen suelto por un **chip-contador**: un `Rectangle { background: Theme.selection-bg; border-radius: 20px; ... }`
con un `Text` dentro que muestre el resumen compacto (p. ej. `running-count + " " + Tr.ops-in-progress`;
si `planning-count>0` anteponer el tramo calculando; el detalle largo puede ir en `status`/tooltip).
El "Cancelar todo" pasa a `OpButton { variant: 2; danger: true; show-cancel: true; }` (ícono X), y
va a la derecha con su tooltip si el patrón existe. Conservar la condición de visibilidad
`(planning-count + running-count + queued-count) > 0` y el callback `cancel-all`.

- [ ] **Step 6: Pasar `animations-enabled` a `OpCard` desde `OpsPanel`**

`OpsPanel` gana `in property <bool> animations-enabled: false;`. En el `for r in root.running-rows: OpCard { ... }`
añadir `animations-enabled: root.animations-enabled;`.

- [ ] **Step 7: Compilar**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -15`
Expected: compila (asumiendo las claves i18n de Task 8 ya creadas).

- [ ] **Step 8: Commit**

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/ui/ops-panel.slint
git commit -m "$(cat <<'EOF'
feat(ui): calculando/cola/historial/header del panel de ops con el nuevo lenguaje visual

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 4 — Animaciones

### Task 7: Fades de entrada/salida + brillo continuo opcional

**Files:**
- Modify: `crates/ui-slint/ui/ops-panel.slint`

- [ ] **Step 1: Fade en las tarjetas/filas al aparecer**

En los `for` de `OpsPanel` (planning/running/queued/history), cada elemento puede aparecer/desaparecer
al cambiar los modelos. Añadir a `OpCard`, `PlanningCard`, `QueuedRow` un fade barato de opacidad al
construirse. En Slint, un patrón simple: property `opacity` con `animate` disparado por un
`init => { self.shown = true; }`. Ejemplo dentro de `OpCard` (root):

```slint
    property <bool> shown: false;
    opacity: root.shown ? 1.0 : 0.0;
    init => { root.shown = true; }
    animate opacity { duration: 150ms; easing: ease-out; }
```

Aplicar el mismo bloque a `PlanningCard` y `QueuedRow`. (Es barato: anima una sola vez al aparecer.)

- [ ] **Step 2: Brillo continuo opcional en la barra**

En `ProgressBar`, añadir un reflejo que recorre el relleno SOLO si `glow`. Se hace con un
`Rectangle` claro y estrecho dentro de `fill-rect`, cuya `x` se anima en bucle. Para que sea
**continuo** hace falta un `Timer` o una animación en bucle; Slint no repite `animate` por sí solo,
así que se usa un `Timer` que alterna una property entre dos valores (o una animación con
`iteration-count` si la versión lo soporta). Implementación con Timer:

```slint
    // Dentro de ProgressBar, tras fill-rect:
    property <bool> glow-phase: false;
    glow-timer := Timer {
        interval: 1200ms;
        running: root.glow;
        triggered => { root.glow-phase = !root.glow-phase; }
    }
    if root.glow: Rectangle {
        width: 40px;
        height: parent.height;
        y: 0;
        x: root.glow-phase ? (fill-rect.width - self.width) : 0px;
        background: Theme.accent.brighter(0.4);
        opacity: 0.35;
        border-radius: self.height / 2;
        animate x { duration: 1200ms; easing: ease-in-out; }
    }
```

Nota: `Theme.accent.brighter(0.4)` — si el compilador rechaza `brighter` sobre la property global,
usar `white` con `opacity: 0.2` como reflejo. Verificar que `Timer` está importado
(`import { Timer } ...` no hace falta: `Timer` es built-in de Slint; si no, usar el patrón de timer
que ya use el proyecto — buscar `Timer {` en otros .slint). Si `Timer` no está disponible en la
versión, el brillo se implementa como una animación `animate x` que rebota entre 0 y el ancho con
un ping-pong manual vía el timer; si NADA de esto es viable con la versión de Slint, dejar el brillo
como un simple cambio de `opacity` del relleno pulsando con Timer (menos vistoso pero válido), y
anotarlo.

El punto CLAVE: cuando `glow == false` (default, y siempre que el usuario no active el flag), el
`Timer` tiene `running: false` y el `if root.glow` no instancia nada → **0 costo**.

- [ ] **Step 3: Compilar y verificar que sin flag no anima**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -12`
Expected: compila. Revisar por lectura que con `glow: false` el Timer no corre y el reflejo no se
instancia.

- [ ] **Step 4: Commit**

```bash
cargo fmt -p naygo-ui-slint
git add crates/ui-slint/ui/ops-panel.slint
git commit -m "$(cat <<'EOF'
feat(ui): fades de entrada baratos + brillo continuo opcional de la barra (tras flag)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Fase 5 — i18n y cableado del flag

### Task 8: Claves i18n nuevas (10 idiomas) + properties de `Tr`

**Files:**
- Modify: `crates/core/src/i18n/{es,en,de,fr,it,hi,ja,ko,pt,zh}.json`
- Modify: `crates/ui-slint/ui/i18n.slint` (global `Tr`)
- Modify: `crates/ui-slint/src/i18n_keys.rs` (función `apply`)

Contexto: los verbos y las etiquetas del toggle se usan directo en el `.slint`, así que necesitan
property en `Tr` + cableado en `i18n_keys.rs`. Patrón existente: property `in property <string> x;`
en `Tr`, y `tr.set_x(c.t("clave").into());` en `apply`. Las etiquetas de `DataCell` (copied, speed,
peak, elapsed, remaining) YA existen como `Tr.ops-copied` etc. (se reutilizan; no crear nuevas).

- [ ] **Step 1: Añadir las claves nuevas a los 10 JSON**

Claves nuevas (verbos + toggle de config). Añadir a `es.json` y traducir a los otros 9. Valores ES/EN:

```
"slint.ops.verb_copy"      ES "Copiando"        EN "Copying"
"slint.ops.verb_move"      ES "Moviendo"        EN "Moving"
"slint.ops.verb_delete"    ES "Eliminando"      EN "Deleting"
"slint.ops.verb_compress"  ES "Comprimiendo"    EN "Compressing"
"slint.ops.verb_extract"   ES "Extrayendo"      EN "Extracting"
"slint.ops.verb_other"     ES "Procesando"      EN "Processing"
"slint.cfg.animations"           ES "Activar animaciones"   EN "Enable animations"
"slint.cfg.tip_animations"       ES "Activa animaciones adicionales (como el brillo de la barra de progreso del panel de operaciones). Consumen un poco más de CPU."   EN "Enables extra animations (such as the operations panel progress-bar glow). They use a little more CPU."
```

Verificar el prefijo real que usan las claves del panel de ops en los JSON (buscar
`"slint.ops.` en `es.json`) y respetarlo. Traducir a de/fr/it/hi/ja/ko/pt/zh de forma natural
(no dejar en español ni copiar el inglés por descuido). NO pisar claves ya existentes.

- [ ] **Step 2: Añadir las properties a `Tr` (i18n.slint)**

En `crates/ui-slint/ui/i18n.slint`, en la global `Tr`, junto a las otras `ops-*`:

```slint
    in property <string> ops-verb-copy;
    in property <string> ops-verb-move;
    in property <string> ops-verb-delete;
    in property <string> ops-verb-compress;
    in property <string> ops-verb-extract;
    in property <string> ops-verb-other;
    in property <string> cfg-animations;
    in property <string> cfg-tip-animations;
```

- [ ] **Step 3: Cablear en `i18n_keys.rs`**

En `crates/ui-slint/src/i18n_keys.rs`, función `apply`, junto a los otros `tr.set_ops_*`:

```rust
    tr.set_ops_verb_copy(c.t("slint.ops.verb_copy").into());
    tr.set_ops_verb_move(c.t("slint.ops.verb_move").into());
    tr.set_ops_verb_delete(c.t("slint.ops.verb_delete").into());
    tr.set_ops_verb_compress(c.t("slint.ops.verb_compress").into());
    tr.set_ops_verb_extract(c.t("slint.ops.verb_extract").into());
    tr.set_ops_verb_other(c.t("slint.ops.verb_other").into());
    tr.set_cfg_animations(c.t("slint.cfg.animations").into());
    tr.set_cfg_tip_animations(c.t("slint.cfg.tip_animations").into());
```

- [ ] **Step 4: Compilar (ahora OpCard/PlanningCard de Fases 3 compilan con los verbos)**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -12`
Expected: compila.

- [ ] **Step 5: Test de paridad i18n + commit**

Run: `cargo test -p naygo-core i18n 2>&1 | grep "test result:"`
Expected: ok (incluye `todos_los_idiomas_tienen_las_claves_de_es`).

```bash
cargo fmt --all
git add crates/core/src/i18n crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs
git commit -m "$(cat <<'EOF'
feat(i18n): verbos de operación + toggle de animaciones en los 10 idiomas

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

### Task 9: Toggle "Activar animaciones" en Config + cablear `animations_enabled` al panel

**Files:**
- Modify: `crates/ui-slint/ui/config-window.slint` (ConfigVm + una fila `Field` con `Switch`)
- Modify: `crates/ui-slint/src/main.rs` (poblar el VM del toggle; pasar `animations-enabled` al `OpsPanel`; guardar el flag al alternar)
- Modify: `crates/ui-slint/src/config_ctrl.rs` o donde se manejen los `set_*` de config (seguir el patrón de `set-auto-highlight-code`)

Contexto: patrón de un toggle de config. En `config-window.slint`: `in property <bool> auto-highlight-code: true;`
+ `callback set-auto-highlight-code(bool);` en el ConfigVm/ventana, y en la pestaña una fila
`Field { label: Tr.cfg-...; tip: Tr.cfg-tip-...; Switch { checked: root.vm.x; toggled => { root.set-x(self.checked); } } }`.
El handler Rust del callback persiste el flag en la config y re-aplica. Buscar `set_auto_highlight_code`
en `crates/ui-slint/src/` para ver el handler y replicarlo para `animations_enabled`.

- [ ] **Step 1: Property + callback en el ConfigVm (config-window.slint)**

Añadir junto a `auto-highlight-code`:

```slint
    in property <bool> animations-enabled: false;
    callback set-animations-enabled(bool);
```

- [ ] **Step 2: Fila del toggle en la pestaña adecuada**

Elegir la pestaña de ajustes visuales/apariencia (o la de "Avanzado" si no hay una clara — buscar
las `if root.cat == N:` para ubicarla). Añadir:

```slint
                                Field {
                                    label: Tr.cfg-animations;
                                    tip: Tr.cfg-tip-animations;
                                    Switch { checked: root.vm.animations-enabled; toggled => { root.set-animations-enabled(self.checked); } }
                                }
```

- [ ] **Step 3: Handler Rust del callback + poblar el VM**

En `main.rs` (o `config_ctrl.rs`), donde se conecta `on_set_auto_highlight_code`, añadir el análogo
`on_set_animations_enabled`: setear `settings.animations_enabled = v`, persistir la config (mismo
guardado que usa `auto_highlight_code`), re-aplicar i18n/tema si aplica. Y donde se puebla el
ConfigVm desde `settings`, poblar `animations-enabled: settings.animations_enabled`.

- [ ] **Step 4: Pasar el flag al `OpsPanel`**

Donde `main.rs` instancia/actualiza el `OpsPanel` (buscar `set_running_rows`/`OpsPanel` o la
property del panel), pasar `animations-enabled` desde `settings.animations_enabled`. Como los otros
ajustes del panel, se refresca cuando cambia la config. Si el `OpsPanel` está embebido en
`app-window.slint`, exponer y propagar `animations-enabled` como se hace con otros ajustes globales.

- [ ] **Step 5: Compilar + tests + clippy**

Run: `$env:CARGO_BUILD_JOBS=2; cargo build -p naygo-ui-slint 2>&1 | tail -12`
Expected: compila.
Run: `cargo test -p naygo-ui-slint 2>&1 | grep "test result:"`
Expected: ok, 0 failed.
Run: `cargo clippy -p naygo-ui-slint --all-targets 2>&1 | tail -4`
Expected: sin warnings.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add crates/ui-slint/ui/config-window.slint crates/ui-slint/src/main.rs crates/ui-slint/src/config_ctrl.rs
git commit -m "$(cat <<'EOF'
feat(ui): toggle "Activar animaciones" en Config, cableado al brillo del panel de ops

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```
(ajustar la lista de `git add` a los archivos realmente tocados; si el handler vive en otro módulo, añadirlo.)

---

## Fase 6 — Verificación integral y dist

### Task 10: Suite completa + clippy estricto + regenerar dist

**Files:** ninguno (verificación).

- [ ] **Step 1: Suite completa del workspace**

Run: `cargo test --workspace 2>&1 | grep -E "test result:|FAILED"`
Expected: todos ok, 0 failed.

- [ ] **Step 2: Clippy estricto (como el CI)**

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -E "warning:|error" | head`
Expected: vacío (0 warnings).

- [ ] **Step 3: fmt check**

Run: `cargo fmt --all -- --check`
Expected: sin diffs.

- [ ] **Step 4: Regenerar dist**

Run: `powershell -File scripts\build-release.ps1` (o el invocador que corresponda en el entorno).
Expected: `Naygo-<version>-portable.zip` y `Naygo-<version>-setup.exe` en `dist/`, sin warnings de idioma.

- [ ] **Step 5: (sin commit — dist no se commitea) Verificación visual pendiente en VM**

Anotar para Nicolás: probar en la VM el panel de operaciones (copiar varios archivos grandes) —
tarjeta en curso, calculando, cola, historial; alternar el toggle "Activar animaciones" y ver el
brillo; confirmar que en reposo (sin ops) no hay consumo de CPU anómalo.

---

## Notas de orden de ejecución

- **Task 8 (i18n) debe ejecutarse antes de compilar las Fases 3/Task 5-6** (OpCard usa `Tr.ops-verb-*`).
  El subagente que haga Task 5 puede: (a) crear primero las claves (adelantar Task 8), o (b) usar
  temporalmente `op.label` y luego sustituir por los verbos en Task 8. Preferir (a): hacer Task 8
  inmediatamente después de Task 2, antes de Task 5. El controlador puede reordenar 8 antes de 5.
- Todo lo demás es secuencial dentro de `ops-panel.slint`; compilar tras cada task evita acumular
  errores de Slint (que son poco descriptivos).

## Self-review (cobertura del spec)

- Íconos de tipo + chip → Task 3 ✓ · Botones sin caja → Task 4/5/6 ✓ · % protagonista → Task 5 ✓ ·
  Grilla 3+2 etiqueta-izq/valor-der → Task 4 (DataCell) + Task 5 ✓ · Barra píldora → Task 4 ✓ ·
  4 zonas coherentes → Task 5+6 ✓ · Header con chip-contador + X cancelar-todo → Task 6 ✓ ·
  Animaciones baratas siempre activas (barra/% interpolados + fades) → Task 4 (animate width) + Task 7 (fades) ✓ ·
  Brillo continuo tras flag → Task 7 + Task 9 ✓ · Flag global `animations_enabled` default off → Task 1 ✓ ·
  Toggle en Config con tooltip de CPU → Task 9 + Task 8 (i18n) ✓ · op-kind en VM (vía a) → Task 2 ✓ ·
  i18n 10 idiomas → Task 8 ✓ · Suite + dist → Task 10 ✓.
- Limitación consciente: HISTORIAL usa un íconito neutro (no ✓/⚠ por éxito/fallo) porque el VM no
  expone hoy un `failed: bool` y no se parsea el status traducido; documentado en Task 6 Step 3.
