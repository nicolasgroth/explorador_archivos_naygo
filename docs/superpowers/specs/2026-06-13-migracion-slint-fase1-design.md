# Migración a Slint — Fase 1: esqueleto navegable (`naygo-ui-slint`) — Diseño

> Primera fase de la migración de la capa UI de egui a Slint (modo retenido +
> renderizador por software), decidida tras medir en la VM de Nicolás: egui ~79% CPU al
> mover el mouse vs 3.7% del prototipo Slint. El contrato de paridad funcional vive en
> `docs/migracion-slint/CONTRATO-PARIDAD-FUNCIONAL.md` y gobierna TODA la migración:
> cero pérdida de funcionalidad.

## Objetivo de la Fase 1

Un crate nuevo `naygo-ui-slint` (binario `naygo-slint`) que convive con `naygo-ui`
(egui) sin tocarlo, y entrega **un panel de archivos navegable** con teclado, orden y
selección múltiple, consumiendo el core/platform reales. Es el esqueleto sobre el que
las fases 2–6 agregan multi-panel, ops, configuración, integraciones y distribución.

**Criterio de "Fase 1 terminada":**
- Lista una carpeta real (async, streaming, vía `naygo-core`), navega (doble clic en
  carpeta, botón subir, Backspace), abre archivos con la app por defecto.
- Tabla con columnas Nombre/Extensión/Tamaño/Modificado, virtualizada.
- Orden por columna (clic en encabezado, ▲/▼).
- Selección: clic / Ctrl+clic / Shift+clic; teclado completo de lista (flechas, Enter,
  AvPag/RePag, Inicio/Fin, Shift/Ctrl variantes, Espacio, Ctrl+A, typeahead).
- Path-bar mínima (breadcrumbs clicables + subir).
- **Medición en la VM de Nicolás:** mover el mouse sobre la lista mantiene la CPU baja
  (objetivo: del orden del prototipo, ~ <10%, no los ~79% de egui).

NO incluye (fases siguientes): multi-panel/docking (F2), operaciones+diálogos (F3),
configuración+editor de atajos (F4), OLE/menú nativo/papelera/tray (F5), pulido+
distribución (F6).

## Arquitectura

Crate `crates/ui-slint/` (paquete `naygo-ui-slint`, binario `naygo-slint`). Depende de
`naygo-core` y `naygo-platform`. CERO lógica de negocio: traduce gestos↔core.

Organización por responsabilidad DESDE el inicio (evita el `app.rs` monolítico de
`naygo-ui`):

```
crates/ui-slint/
  Cargo.toml
  build.rs                  # compila los .slint
  ui/
    app-window.slint        # ventana + layout vertical (toolbar, path-bar, panel)
    file-panel.slint        # la tabla (ListView virtualizada) + encabezados
    path-bar.slint          # breadcrumbs + botón subir (mínima en F1)
    toolbar.slint           # mínima en F1 (atrás/adelante/arriba/refrescar) — opcional F1
    types.slint             # structs compartidos (RowData) y enums
  src/
    main.rs                 # arma ventana, conecta callbacks → controller, run()
    controller.rs           # estado de la app (FilePaneState) + handlers de callbacks
    bridge.rs               # Entry/índices de vista ↔ RowData (modelo Slint). PURO + tests
    listing.rs              # pump async: spawn_listing + slint::Timer que drena por lotes
    keys.rs                 # mapea slint::platform::Key + modifiers → keymap::Chord
```

## Modelo de datos y data flow

### El modelo de la tabla
El core da `FilePaneState::view_indices()` → `Vec<usize>` (índices cacheados, filtrados+
ordenados) sobre `entries`. `bridge::rows_from_view(&FilePaneState)` los convierte en un
`Vec<RowData>` que se mete en un `slint::VecModel<RowData>`:

```
// types.slint
struct RowData {
    name: string,
    ext: string,
    size: string,        // ya formateado por core::format::human_size
    modified: string,    // ya formateado
    is-dir: bool,
    selected: bool,      // refleja FilePaneState.selected (posición de vista)
    focused: bool,       // refleja FilePaneState.focused
}
```

La `ListView` de Slint VIRTUALIZA (solo materializa filas visibles) → carpetas grandes
sin costo de pintado, como egui_extras.

### Flujo de un listado (streaming async, rendimiento)
1. Gesto de navegar (doble clic en carpeta / subir / breadcrumb) → callback Slint →
   `controller`: `FilePaneState::navigate_to(dir)` + `listing::start(dir, ...)`.
2. `listing::start` lanza `core::spawn_listing(dir, token)` (worker) y enciende un
   `slint::Timer` periódico (~30 ms) **solo mientras hay listado activo**.
3. Cada tick del timer: drena TODO lo acumulado en el canal en un lote
   (`while let Ok(msg) = rx.try_recv()`), aplica a `f.entries`; al `ListingMsg::Done`
   ordena (`core::sort`), apaga el timer y reconstruye el `VecModel` desde
   `view_indices()`. Resultados de un path ya no vigente se descartan (token/época).
4. En reposo el timer está apagado → **0 trabajo, 0 repaints** (la propiedad clave para
   el bajo consumo sin GPU). Slint repinta solo lo que cambió.

**Por qué timer-drena-lote y no `invoke_from_event_loop` por evento:** el worker emite un
`Entry` por archivo; con push por evento, una carpeta de 5000 archivos = 5000 wake-ups
del event loop. El timer agrupa en lotes ~30 ms (≈30fps), coherente con el cap que ya
aplicamos en egui, y se apaga al terminar. `invoke_from_event_loop` se reserva para
despertares puntuales (p. ej. el waker de un watcher en fases futuras).

### Selección, foco, orden, teclado (TODO reusa core)
- Selección: `select_single/select_toggle/select_range_to` (ya en core, testeados).
- Foco/teclado de lista: `move_focus_extend`, `focus_page/home/end`, `move_focus_keep`
  (ya en core, testeados en la fase teclado).
- Orden: fija `FilePaneState.sort` (alterna asc/desc) → `view_indices()` recalcula.
- El `controller` traduce gestos Slint → estas llamadas y luego refresca el modelo.
  CERO lógica nueva de selección/orden/teclado.

## Interacción (Fase 1)

### Mouse
- Clic en fila → `select_single(pos)`. Ctrl+clic → `select_toggle`. Shift+clic →
  `select_range_to`. (Modificadores vienen en el `PointerEvent` de Slint.)
- Doble clic: carpeta → navegar; archivo → `naygo_platform::open::open_default`.
- Clic en encabezado de columna → ordenar por esa columna (toggle asc/desc).
- Clic en breadcrumb → navegar a ese segmento. Botón ↑ → subir al padre.

### Teclado
Un `FocusScope` en `file-panel.slint` captura teclas; callback `key-pressed` → Rust.
`keys::chord_from(event)` arma un `keymap::Chord` (tecla + ctrl/shift/alt) y
`keymap.action_for(&chord)` resuelve la `Action`. F1 maneja el subconjunto navegable:
`MoveUp/Down`, `Activate` (Enter), `GoUp`, `FocusPageUp/Down`, `FocusHome/End`,
`ExtendUp/Down` + variantes de bloque, `FocusUpKeep/DownKeep`, `ToggleSelect`,
`ToggleFocused`, `SelectAll`. Typeahead: texto que no dispara acción salta al primer
ítem que empieza con lo tecleado (reset ~500 ms) — reusa la lógica de core si existe, o
se implementa en `controller` (búsqueda lineal sobre la vista).

El resto de acciones (Copy/Cut/Paste/Delete/Rename/…) se reconocen pero son no-op en F1
(se conectan en F3); NO se pierden del keymap, solo aún no tienen efecto.

### Scroll-to-focus
Al mover el foco por teclado, el `controller` pide a la `ListView` que la fila enfocada
quede visible (Slint expone `viewport-y`/`visible-*`; se calcula el offset como en egui).

## Errores y bordes
- `ListingMsg::Error` (permiso/ruta caída) → status discreto en la UI, sin crash.
- Filesystem hostil: cubierto por core; la UI solo refleja `Result`.
- Path inexistente al navegar: no peta; vuelve al estado anterior con aviso.

## Testing
- `bridge.rs`: tests PUROS (sin Slint) — `Entry`/índices → `RowData` (nombre, ext, size
  formateado, flags selected/focused correctos según `FilePaneState`).
- `keys.rs`: tests puros — `slint Key + modifiers → Chord` esperado.
- Selección/orden/teclado: ya cubiertos por los tests de `naygo-core` (no se duplican).
- Verificación viva (Nicolás, en la VM): listar carpeta grande, navegar, teclado, orden,
  selección; **medir CPU al mover el mouse** (criterio de éxito de rendimiento).

## Build y distribución (F1)
- `Cargo.toml`: `slint` con `default-features=false` + `backend-winit` +
  `renderer-software` (sin GPU); `slint-build` en build-deps. (Igual que el proto, ya
  validado compilando.)
- Se agrega `crates/ui-slint` a los members del workspace.
- F1 NO toca el instalador ni reemplaza el binario `naygo` (eso es F6). Se corre con
  `cargo run -p naygo-ui-slint`. El render por software se fuerza/valida con
  `SLINT_BACKEND=winit-software`.

## Riesgos y decisiones abiertas
- **Scroll-to-row exacto en Slint `ListView`:** la API de scroll programático puede
  diferir de egui; si `ListView` no expone un "scroll a índice" cómodo, se calcula el
  `viewport-y` manualmente (índice × alto de fila). Se resuelve en el plan.
- **Alto de fila fijo:** se asume fijo (~22 px) para virtualización y cálculo de scroll,
  igual que egui.
- **Formato de fecha:** hoy core formatea fechas de forma provisional (epoch); F1 reusa
  lo que core da, sin mejorar el formato (eso es ortogonal a la migración).
