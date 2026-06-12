# Plan de trabajo — Lote de ajustes de Nicolás (2026-06-12)

> **Para el agente ejecutor (Opus 4.8 o similar):** ejecuta las tareas EN ORDEN,
> una por una, con commit al final de cada una. Lee ANTES de empezar: `CLAUDE.md`
> (reglas del proyecto, no negociables) y la memoria del proyecto. Cada tarea
> tiene checkboxes; márcalas al completar.

**Objetivo:** cerrar el lote de ajustes pedido por Nicolás tras probar la
path-bar/favoritos: teclado completo de lista, split automático de paneles,
anchos de columnas configurables, acciones multi-panel (spec aprobada), panel
Preview y pulido visual de Configuración.

**Stack:** Rust, egui 0.34.3 + egui_extras (TableBuilder) + egui_dock 0.19.
Crates: `naygo-core` (puro, sin egui/Windows), `naygo-platform` (Win32),
`naygo-ui` (egui, sin lógica de negocio).

---

## Reglas operativas (aprendidas a golpes — NO te las saltes)

1. **Puertas antes de CADA commit**: `cargo test --workspace` (lee TODAS las
   líneas `test result:` — un awk/select truncado ya ocultó un fallo una vez),
   `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`.
2. **`cargo build -p naygo-ui` explícito antes de cualquier prueba en vivo**:
   test/clippy NO regeneran el bin (un agente ya probó contra un exe viejo y
   reportó falsos negativos).
3. **Mata `naygo.exe` ANTES de compilar** (`Stop-Process -Name naygo -Force`,
   tolerante): si el exe está en uso, el build falla con Access denied y un
   relanzamiento corre el binario VIEJO.
4. **Verificación en vivo**: criterios DUROS observables (panel Propiedades,
   barra de estado, archivos en disco), no resaltados de fila (pueden ser
   hover). El input sintético NO entrega Escape ni chords con modificador
   (Ctrl+X, Shift+clic): pruébalos con `PostMessage` WM_KEYDOWN al hwnd, o
   déjalos anotados para prueba manual de Nicolás.
5. **Convenciones**: header de copyright en archivos nuevos; comentarios en
   español explicando el PORQUÉ; cero texto hardcodeado (i18n ES + EN SIEMPRE
   en paridad); el hilo de UI no hace I/O de disco (workers + canales; mira
   `SizeJob`/`PathAutocomplete` en `app.rs` como patrón); legibilidad sobre
   brevedad.
6. **Commits**: uno por tarea, mensaje en español estilo del repo, con
   `Co-Authored-By: <tu modelo>`. NO pushees a main sin autorización de
   Nicolás en la sesión.
7. El patrón del repo para UI→estado: los paneles NO mutan `NaygoApp` mientras
   pintan; acumulan señales (`PaneRequest`, `Vec<Action>`, vectores `&mut`) que
   `NaygoApp` procesa tras pintar. Síguelo en todo lo nuevo.

---

### Tarea 1 — Teclado completo de lista

**Qué:** AvPag/RePag (PageDown/PageUp), Inicio/Fin (Home/End), selección por
teclado: `Shift+↑/↓` extiende la selección desde el ancla, `Shift+AvPag/RePag/
Inicio/Fin` ídem por bloques, `Ctrl+↑/↓` mueve el foco SIN tocar la selección,
`Ctrl+Espacio` togglea el ítem con foco (estándar Windows/Explorer).

**Archivos:** `crates/core/src/keymap.rs` (acciones nuevas + chords default +
labels `action.*` + ACTUALIZA el test de conteo de acciones),
`crates/core/src/workspace/file_pane.rs` (métodos de movimiento/selección
puros + tests), `crates/ui/src/app.rs` (`handle_input`: agrega
PageUp/PageDown/Home/End a la lista `KEYS` y a la captura de atajos; brazos
nuevos en `apply_action`), `crates/ui/src/input.rs` (KeyCodes nuevos + test),
i18n es/en.

**Diseño:**
- [ ] Acciones: `FocusPageUp/FocusPageDown/FocusHome/FocusEnd` (+ variantes
  `Select*` con Shift en el chord, `FocusUpKeep/FocusDownKeep` para Ctrl+↑/↓,
  `ToggleFocused` para Ctrl+Espacio). Mira cómo están modeladas las flechas
  actuales antes de decidir si replicas el patrón existente (hay acciones de
  foco hoy: estúdialas y sé consistente).
- [ ] Tamaño de página: el `handle_input` no conoce el alto del panel. Calcula
  las filas visibles reales: `NaygoApp` puede capturar por frame el alto del
  cuerpo de la tabla (file_panel ya devuelve señales; agrega un
  `visible_rows: usize` por panel calculado como `alto_disponible / ROW_HEIGHT`
  y guárdalo en un mapa por PaneId). Fallback si no hay dato: 20.
- [ ] En core (`FilePaneState`): `focus_page(delta_pages, rows_per_page)`,
  `focus_home()`, `focus_end()`, `extend_selection_to_focus()` — todos sobre
  POSICIONES DE VISTA (`view_indices()`), con clamp; el ancla ya existe
  (`anchor`, lo usa `select_range_to`). TESTS de cada uno (lista de 100,
  page=20: PageDown desde 0 → 20; End → 99; Shift+End desde ancla 5 →
  selección 5..=99; Ctrl+↓ no toca `selected`).
- [ ] Scroll: al mover el foco por teclado la tabla debe SEGUIRLO. Revisa cómo
  hoy las flechas hacen visible la fila enfocada (busca `scroll_to_row` o
  similar en file_panel; si no existe, usa `TableBuilder::scroll_to_row` de
  egui_extras con el foco cuando cambió por teclado este frame).
- [ ] Verificación en vivo (criterio: Propiedades + barra de estado): navega a
  una carpeta grande (`C:\Users\ngrot` sirve), End → Propiedades muestra el
  último; Home → el primero; PageDown ×2 avanza ~2 páginas; selección con
  Shift por PostMessage o anótala para Nicolás.
- [ ] Puertas + commit.

### Tarea 2 — Agregar panel divide en vez de apilar pestaña

**Qué:** hoy `add_pane`/`add_pane_of` hacen `push_to_focused_leaf` (pestaña).
Deben DIVIDIR el leaf enfocado al 50%.

**Archivos:** `crates/ui/src/app.rs` (los dos métodos), posiblemente
`crates/ui/src/dock_translate.rs` para entender cómo se reconstruye el layout.

**Diseño:**
- [ ] Usa `DockState::main_surface_mut().split_right(node_index, 0.5, vec![id])`
  (o `split_below` si el leaf es más alto que ancho — calcula con el rect del
  nodo si egui_dock lo expone; si no, `split_right` SIEMPRE es aceptable v1).
  Necesitas el `NodeIndex` del leaf enfocado: `focused_leaf()` del DockState.
  Sin leaf enfocado → comportamiento actual como fallback.
- [ ] OJO: el workspace persiste el layout (`save_workspace` /
  `dock_translate`); verifica que el layout dividido sobreviva un
  reinicio (cerrar y volver a abrir la app en vivo).
- [ ] Verificación en vivo: ➕ y ▾→Historial crean paneles DIVIDIENDO (se ve
  lado a lado, no como pestaña apilada). Reinicia la app → el layout persiste.
- [ ] Puertas + commit.

### Tarea 3 — Anchos de columnas: automático/fijo + guardar default

**Qué:** algunos paneles quedan con anchos desproporcionados. Configuración →
Paneles gana: (a) modo de ancho `Automático` (la tabla reparte según contenido,
`Column::auto()` de egui_extras) vs `Fijo` (comportamiento actual con anchos
del `TableState`); (b) botón «Usar el panel activo como predeterminado»: toma
el `TableState` COMPLETO del panel activo (columnas visibles, orden y anchos) y
lo guarda en settings como plantilla para paneles NUEVOS.

**Archivos:** `crates/core/src/config/mod.rs` (Settings: `column_width_mode:
enum {Auto, Fixed}` con serde default + `default_table: Option<TableState>`;
TableState ya es serializable — verifica), `crates/core/src/columns.rs` (lee
antes de tocar), `crates/ui/src/panes/file_panel.rs` (al armar el
`TableBuilder`, bifurca `Column::auto().clip(true)` vs el actual
`Column::initial(...)` según el modo), `crates/ui/src/settings_window/panes.rs`
(la sección Paneles existe — agrégale el selector y el botón),
`crates/ui/src/app.rs` (los paneles nuevos nacen con `default_table` si existe),
i18n es/en.

**Diseño:**
- [ ] En modo Auto NO emitas `SetColumnWidth` (el resize manual no aplica);
  deshabilita `resizable` o ignora los measured_widths para no pelear con egui.
- [ ] El botón de guardar default vive en Configuración → Paneles, con un hint
  que diga qué hace; al guardarlo, status «guardado» (i18n).
- [ ] Tests core de round-trip del nuevo Settings (el test
  `settings_round_trip` existe: extiéndelo).
- [ ] Verificación en vivo: cambiar a Auto → columnas se reparten; volver a
  Fijo → vuelven los anchos; guardar default → ➕ panel nuevo nace con esas
  columnas/anchos.
- [ ] Puertas + commit.

### Tarea 4 — Acciones multi-panel + selector numérico (spec aprobada)

**Sigue al pie de la letra** `docs/superpowers/specs/
2026-06-11-acciones-multipanel-design.md` (léela completa; tiene el esqueleto
de implementación, la regla del destino 2/3+/1 paneles, y los tests pedidos).

Resumen de anclas del código: el doble-clic está en `file_panel.rs` (busca
`double_clicked` — ya bifurca dir/archivo; agrega la variante con
`modifiers.ctrl` → `PaneRequest::OpenInOther`); los rects por panel para el
overlay captúralos en un `HashMap<PaneId, egui::Rect>` que ya puedes poblar
desde el `pane_rect` calculado al final de `file_panel::show` (pásalo de
vuelta vía una señal); el overlay píntalo en `NaygoApp::ui` DESPUÉS del dock
(usa `egui::Area`/painter de foreground); los números 1..9 léelos en `logic`
mientras `pending_pane_pick` exista (suspende el input global como los
modales). Íconos ⇄ y clonar en la toolbar: sigue el patrón `btn!` y agrega 2
`ActionIcon` nuevos (con PNG placeholder en los 3 sets — mira
`assets/gen_icons` o deja el fallback unknown, la toolbar no debe romperse;
JAMÁS commitees zips de assets).

- [ ] Implementación según spec + tests core (`resolve_target`, numeración,
  swap con historiales).
- [ ] Verificación en vivo: con 2 paneles, Ctrl+doble-clic (vía PostMessage si
  el sintético falla) abre en el otro; swap intercambia (instantáneo por el
  caché); con 3 paneles el overlay numera y `2` elige (números sí llegan
  sintéticos, son teclas sin modificador).
- [ ] Puertas + commit.

### Tarea 5 — Panel Preview (vista previa liviana y cancelable)

**Qué:** nuevo `PanePurpose::Preview`: muestra una vista previa del archivo
ENFOCADO del panel activo. SOLO extensiones livianas; SIEMPRE async; la
navegación JAMÁS se bloquea (requisito explícito de Nicolás).

**Diseño (mini-spec):**
- Texto (`txt, log, md, json, xml, csv, toml, yaml, ini, rs, py, js, html`):
  primeras 100 líneas o 64 KB, lo que llegue primero; conversión lossy; si se
  truncó, una línea final «··· (archivo más largo)» (i18n). Monospace.
- Imagen (`jpg, jpeg, png, bmp, gif, webp, ico`): decodificada en el worker
  (crate `image` ya es dependencia de core), reescalada ANTES de subirla como
  textura si excede ~1024px por lado (no subas texturas gigantes), pintada
  proporcional al panel. Tope de archivo: 20 MB (más grande → mensaje «muy
  grande para previsualizar»).
- Cualquier otra extensión, o video/audio: mensaje neutro «sin vista previa»
  (i18n). NUNCA leas el archivo.
- Worker: `std::thread` + `mpsc` + `CancellationToken` — al cambiar el foco se
  CANCELA el job anterior y se lanza el nuevo con debounce ~150 ms (mira
  `PathAutocomplete` en app.rs: mismo esqueleto). El resultado llega por canal
  y `pump_preview` (en `logic`) lo aplica; resultados de un path ya no enfocado
  se DESCARTAN.
- Config: lista de extensiones de texto editable en Configuración (sección
  nueva «Previsualización»), como String separada por comas en Settings con
  default razonable; imágenes fijas v1.
- Estructura: `core/src/preview.rs` (clasificación extensión→tipo + truncado de
  texto, PURO con tests), `platform` no se toca, `ui/src/panes/preview_panel.rs`
  (pinta según el estado), wiring en docking/menú ▾/workspace como cualquier
  panel (patrón History/Favorites, commits fd7b7eb y 76ba9da).
- [ ] Implementación + tests core (clasificación, truncado por líneas y por
  bytes, extensión desconocida → None).
- [ ] Verificación en vivo: panel Preview abierto; enfocar un .txt muestra las
  primeras líneas; enfocar un .png muestra la imagen; enfocar un .exe muestra
  «sin vista previa»; moverse RÁPIDO con flechas por una carpeta grande no
  congela (criterio: la barra de estado sigue actualizando fluida).
- [ ] Puertas + commit.

### Tarea 6 — Pulido visual de Configuración (CON gate de Nicolás)

**Qué:** los `selectable_value` se ven toscos. Crear un widget reutilizable
`segmented(ui, &mut valor, &[(valor, etiqueta)])` en un módulo
`ui/src/widgets.rs`: grupo tipo "pill" (fondo redondeado contenedor, opción
activa con fondo de acento suave y texto fuerte, hover sutil, altura cómoda
~26px, animación de egui si es gratis). Aplicarlo a TODAS las secciones de
settings_window que hoy usan selectable_value en fila.

- [ ] **GATE OBLIGATORIO**: implementa el widget, aplícalo a UNA sección
  (Avanzado→Operaciones), toma screenshot en vivo y MUÉSTRASELO a Nicolás con
  1-2 variantes (p. ej. pill con borde vs sin borde). NO apliques al resto ni
  commitees hasta su OK (regla del proyecto: el diseño visual lo decide él).
- [ ] Tras el OK: aplicar a todo, puertas + commit.

### Tarea 7 — Cierre del lote

- [ ] Pasada final de puertas completas.
- [ ] Regenerar distribución: `powershell -ExecutionPolicy Bypass -File
  scripts\build-release.ps1` y luego, si ISCC no está en PATH:
  `& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" /DMyAppVersion=0.1.0
  installer\naygo.iss`. Verifica timestamps frescos en `dist\`.
- [ ] Avisar a Nicolás: qué probar manualmente (chords con modificador, Esc,
  autocompletado de la path-bar — el input sintético no los cubre) y que el
  RELEASE ya trae todo (venía probando un exe viejo).
- [ ] Actualizar la memoria del proyecto (estado del backlog) y pedir
  autorización para push si no se pusheó por tarea.

---

## Autoevaluación del plan (hecha)

- Cubre los 6 pedidos de Nicolás (2026-06-12) + el cierre de distribución.
- El doble-clic-abre-archivo NO está en el plan: ya se arregló (fffe02e).
- Decisiones delegadas al ejecutor están señaladas (modelar acciones de foco
  consistente con las existentes; split_below vs split_right si hay datos de
  aspecto) — todo lo demás es concreto.
- Tarea 6 tiene gate humano explícito; ninguna otra requiere parar.
