# Migración a Slint — Fase 2: multi-panel + docking + paneles especiales — Diseño

> Segunda fase de la migración egui→Slint. La Fase 1 (panel único navegable) está en main
> y validada en la VM (3.3% CPU al mover el mouse vs 79% egui). Esta fase agrega el
> sistema de paneles: múltiples paneles Files lado a lado, splits redimensionables, tabs,
> y los paneles especiales (Árbol, Historial, Favoritos, Propiedades, Preview). Gobernada
> por el contrato de paridad (`docs/migracion-slint/CONTRATO-PARIDAD-FUNCIONAL.md`).

## El desafío técnico central

egui_dock daba docking arbitrario (árbol de splits de cualquier profundidad + tabs
arrastrables) gratis. Slint NO tiene equivalente y su `.slint` es estáticamente
estructurado: NO se puede instanciar recursivamente un árbol `DockNode` de profundidad
arbitraria solo en markup (`for` repite un modelo PLANO, no anida).

**Solución (preserva el docking arbitrario = paridad):** computar el layout en RUST. El
árbol `SerializableDockLayout` (que ya vive en `core`, agnóstico de egui) se "aplana" a
una lista de **rects absolutos** (x/y/w/h por panel) dado el tamaño de la ventana; Slint
renderiza un `for` plano de paneles posicionados absolutamente. Los splitters son handles
arrastrables que ajustan la `fraction` del nodo en el árbol del core. Esto mantiene splits
de cualquier profundidad y es testeable en core (la matemática del layout es pura).

Más lógica al core (tested), menos a Slint. La capa Slint solo posiciona y captura gestos.

## Nuevas piezas en core (puras, con tests)

`crate core::workspace::layout` gana operaciones de árbol (hoy solo tiene `pane_ids`):
- `pane_rects(&layout, area: Rect) -> Vec<(PaneId, Rect)>`: recorre el árbol y reparte
  `area` según `fraction` y `dir` de cada split → rect de cada hoja. PURO.
- `split_rects(&layout, area: Rect) -> Vec<SplitHandle>`: los rects+orientación de cada
  splitter arrastrable (para hit-test y para ajustar la fracción). PURO.
- `set_fraction(&mut layout, split_id, fraction)`: ajusta la proporción de un split
  (clamp 0.05..0.95). Identifica el split por una ruta/índice estable.
- `split_leaf(&mut layout, leaf, dir, new_id)`: divide una hoja en dos (la nueva queda al
  lado/abajo) — para "agregar panel divide el leaf enfocado".
- `remove_leaf(&mut layout, id)`: quita una hoja y colapsa el split degenerado (cerrar
  panel/tab).
Todas con tests (un layout conocido → rects esperados; ajustar fracción mueve el borde;
split/remove producen el árbol esperado).

`Rect` propio en core (`struct Rect { x, y, w, h }`, f32) para no depender de egui/slint.

## Tabs (apilar paneles en un grupo)

egui_dock permite varios paneles en el mismo "leaf" como tabs. Para F2: una hoja del
árbol puede tener VARIOS `PaneId` (un `Leaf` pasa de un id a una lista + índice activo).
El render dibuja una barra de tabs arriba del rect de la hoja y muestra el panel activo.
Clic en tab lo activa; botón × cierra (usa `remove_leaf`/quita-de-hoja). (Si esto infla
F2, los tabs van a una sub-fase 2c; ver decomposición.)

## Paneles especiales (Árbol, Historial, Favoritos, Propiedades, Preview)

Cada uno es un componente `.slint` propio + su bridge a core (todos esos datos ya existen
en core/NaygoApp-equivalente). En F2 se renderizan con su contenido real:
- **Árbol**: `core::tree::DirTree` (lazy, watcher) → modelo plano de nodos con nivel de
  sangría (mismo truco de aplanar: un `for` plano con `indent = nivel*14px`). Clic navega
  el panel Files activo; ▶/▼ expande/colapsa; favoritos anclados arriba; barra de uso de
  disco por unidad.
- **Historial** (undo): lista de ops; "Deshacer"/"Deshacer hasta aquí".
- **Favoritos**: lista + recientes; clic navega; clic derecho quita.
- **Propiedades** (inspector): metadata del ítem enfocado del panel activo.
- **Preview**: texto/imagen/mensaje del archivo enfocado (worker + debounce, ya en core).

El "controller" de F1 se generaliza a un `Workspace` (varios `FilePaneState` + el árbol/
historial/favoritos/preview compartidos), siguiendo el modelo del `NaygoApp` de egui pero
en módulos por responsabilidad.

## Toolbar (F2)

Se completa con lo que opera sobre paneles: atrás/adelante (+ menú de historial), arriba,
refrescar, agregar panel (divide), menú ▾ de otros paneles, multi-panel (swap/clonar),
strip de unidades, nueva ventana. (Ops de archivo y ajustes: F3/F4.)

## Decomposición de la Fase 2 (sub-fases shippables)

Por tamaño, F2 se entrega en 3 sub-fases, cada una funcional y medible:
- **2a — Splits multi-panel:** core layout (pane_rects/split_rects/set_fraction/split_leaf/
  remove_leaf + tests); render del árbol en Slint con splitters arrastrables; toolbar
  "agregar panel" (divide) + activar panel con clic; persistencia del layout. Varios
  paneles Files lado a lado, redimensionables. (SIN tabs, SIN paneles especiales aún.)
- **2b — Paneles especiales:** Árbol, Historial, Favoritos, Propiedades, Preview, cada uno
  con su componente + bridge; menú ▾ para agregarlos; el árbol con watcher.
- **2c — Tabs + reacomodo + multi-panel acciones:** apilar paneles como tabs en un grupo;
  **drag-to-rearrange (arrastrar un tab a otro grupo / dividir soltándolo)** — requisito de
  paridad confirmado por Nicolás (2026-06-13); swap/clonar; selector 1..9; plantillas de
  layout. El reacomodo se apoya en `split_leaf`/`remove_leaf` del core + hit-test de zonas
  de drop (sobre qué grupo y en qué borde se suelta) calculado en Rust.

Cada sub-fase: spec breve (o sección de este spec) → plan → ejecución con puertas →
verificación viva + CPU en la VM. Este spec cubre 2a en detalle; 2b/2c se planifican al
llegar (su diseño está esbozado arriba).

## Testing
- core: tests de `pane_rects`, `split_rects`, `set_fraction`, `split_leaf`, `remove_leaf`
  (layouts conocidos, fracciones, profundidad). Aplanado del árbol (Tree) a modelo con
  sangría.
- ui-slint: bridge layout→modelo de rects (puro). Selección de panel activo.
- Verificación viva (Nicolás, VM): varios paneles, arrastrar splitter, agregar/cerrar
  panel, layout persiste al reiniciar; CPU bajo al interactuar.

## Riesgos / decisiones
- **Drag de splitter sin GPU:** el arrastre repinta; al ser modo retenido Slint repinta lo
  afectado, no toda la ventana. Se mide en 2a; si el arrastre cuesta, se limita el
  refresco del fraction a ~30fps (mismo criterio que el hover/listado).
- **Identidad estable de splits:** `set_fraction`/`split_leaf` referencian un split por su
  RUTA en el árbol (secuencia de first/second) o un id incremental; se decide en el plan
  de 2a (la ruta es estable mientras el árbol no cambie entre el hit-test y el ajuste, que
  es el caso de un drag).
- **Drag-to-rearrange tabs (mover un tab a otro grupo):** REQUISITO DE PARIDAD (Nicolás,
  2026-06-13), no opcional. Es lo más caro del docking. Se implementa en 2c: arrastrar un
  tab muestra zonas de drop (sobre qué grupo, y borde izq/der/arr/ab para dividir, o el
  centro para apilar); al soltar, Rust recompone el árbol (`split_leaf` para dividir o
  mover-a-hoja para apilar) y re-lista solo lo necesario. El hit-test de las zonas de drop
  se calcula en Rust desde los rects (puro, testeable). Si bajo render por software el
  arrastre cuesta, se limita el preview del drop a ~30fps (mismo criterio que el resto).

## Fuera de alcance (otras fases)
Operaciones de archivo + diálogos + progreso (F3); Configuración + editor de atajos (F4);
OLE/menú nativo/papelera/tray/splash (F5); pulido + distribución + retiro de egui (F6).
