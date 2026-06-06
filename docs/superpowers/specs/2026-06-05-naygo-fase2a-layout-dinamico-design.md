# Naygo — Fase 2A: Layout dinámico (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-05. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

La **Fase 1** entregó un esqueleto navegable: un workspace de 3 crates
(`naygo-core` / `naygo-platform` / `naygo-ui`), motor de listado por streaming
cancelable, y una UI con egui_dock de layout fijo (un árbol, **un** panel de
archivos, un inspector) con navegación por teclado y type-ahead.

Tras ver la Fase 1 corriendo, Nicolás pidió evolucionar hacia un explorador
**estilo Directory Opus**: paneles realmente dinámicos y componibles, navegación
atrás/adelante como un navegador (incluidos los botones laterales del mouse), y
plantillas de disposición guardables.

El trabajo visual/layout se dividió en **tres sub-fases independientes**, cada una
con su propio spec → plan → build:

- **2A — Layout dinámico (ESTE documento):** paneles independientes componibles,
  plantillas (built-in + propias, con recientes y favoritos), navegación
  atrás/adelante por panel (toolbar + teclado + botones del mouse), barra de
  íconos con posición configurable, persistencia del workspace.
- **2B — Íconos:** reemplazar los glifos de texto (`[D]`) por sets de íconos
  temables + íconos de tipo de archivo y de unidad. (Spec propio, después.)
- **2C — i18n + temas + color sets:** catálogo de strings (ES + EN), temas y
  color sets intercambiables en caliente. (Spec propio, después.)

**Premisa rectora (de Nicolás):** la app debe ser de **respuesta rápida y fluida**.
Toda decisión de esta fase se subordina a eso: la lógica pesada vive en `core`
testeable y en workers; el hilo de UI solo pinta y despacha. Múltiples paneles =
múltiples listados en paralelo, sin bloquear la UI.

### Qué entra en 2A

- **Paneles independientes y componibles.** Tres *tipos* de panel —`Files`, `Tree`,
  `Inspector`— que el usuario agrega, quita y arrastra libremente (egui_dock ya es
  dockable). Composiciones arbitrarias: 1 panel pelado; 3 paneles + 3 árboles +
  inspector; etc.
- **Estado por panel.** Cada panel `Files` tiene su propia carpeta, entries, sort,
  vista, foco, selección, **su propio historial de navegación**, y un filtro
  `show_dirs`.
- **Inspector** que **sigue al panel activo** (un inspector sirve a todos los
  paneles; se actualiza al cambiar el panel con foco).
- **Navegación atrás/adelante por panel:** historial propio de cada panel `Files`;
  disparada por botones de toolbar, teclas (`Alt+←`/`Alt+→`, `Backspace`=arriba) y
  **botones laterales del mouse** (`PointerButton::Extra1`=atrás,
  `Extra2`=adelante), siempre sobre el panel activo.
- **Plantillas de layout:** presets built-in (Minimalista, Clásico, Dual-pane,
  Power-user) + plantillas que el usuario guarda. Acceso rápido vía **★ favoritos**
  (marcables) y **🕘 recientes** (automáticos), desplegados desde un ícono como
  combobox compacto.
- **Barra de íconos** con acciones (atrás/adelante/arriba/refrescar, Layouts,
  agregar panel, configuración). Botones **solo-ícono** con tooltips por defecto;
  **posición configurable** (superior u lateral).
- **Persistencia** del workspace (disposición + carpeta por panel + panel activo +
  filtros + settings de barra) en JSON portable al lado del `.exe`, tolerante a
  corrupción.
- **Breadcrumb clicable** por panel `Files`.

### Qué NO entra en 2A (recordatorio explícito)

Íconos reales (siguen los glifos `[D]` provisionales — llegan en 2B); i18n (texto
sigue hardcoded en español — llega en 2C); temas/color sets (2C); drag&drop con el
SO; operaciones de archivo (copiar/mover/eliminar/renombrar/crear); cálculo de
tamaño de carpeta; menú contextual nativo; **filtro de texto rápido por panel**
(se reserva el campo en el modelo pero sin UI ni lógica en 2A); la pantalla de
Configuración completa (2A persiste settings y define dónde se administrarán, pero
la UI de administración fina de plantillas/historial/filtros es un pulido posterior
— en 2A basta con guardar/aplicar/marcar favorito/borrar desde el combobox).

---

## 2. Arquitectura

Idea rectora de Fase 1 intacta: **separar lógica pura (testeable, sin UI/Windows)
del pintado; el hilo de UI nunca hace I/O**. 2A mueve el estado de "un panel" a
"un workspace de N paneles" y materializa el módulo `config` que el spec del núcleo
ya preveía.

### Capa `core` — módulo nuevo `workspace` (lógica pura, testeable al 100%)

- **`PaneId`**: identificador único y estable de cada panel (p. ej. `u64`
  incremental). Permite referirse a un panel sin depender de su posición.
- **`PanePurpose`**: qué tipo de panel es — `Files`, `Tree`, `Inspector`.
  Serializable.
- **`NavHistory`** (pieza pura, muy testeable): historial atrás/adelante de un
  panel. Internamente una pila de rutas + un cursor.
  - `current() -> Option<&Path>`
  - `push(path)`: navegar a una ruta nueva; **trunca** la rama de "adelante" (como
    un navegador).
  - `back() -> Option<&Path>` / `forward() -> Option<&Path>`: mueven el cursor sin
    perder elementos; `None` si no hay a dónde ir.
  - `can_back() -> bool` / `can_forward() -> bool` (para habilitar/deshabilitar
    botones).
  - Límite configurable de profundidad (p. ej. 256) para no crecer sin tope.
- **`FilePaneState`**: el estado de un panel de archivos. Extiende lo que en Fase 1
  era `PaneState` (en `fs_model`):
  - `current_dir`, `entries`, `sort`, `view`, `focused`, `selected` (ya existían),
  - **`history: NavHistory`** (nuevo),
  - **`show_dirs: bool`** (nuevo; filtro: si es `false`, el panel oculta carpetas),
  - **`text_filter: Option<String>`** (nuevo, RESERVADO: siempre `None` en 2A; sin
    UI ni efecto todavía — sólo se persiste para evitar refactor en una fase futura).
  - Métodos de navegación que combinan `history` + `current_dir`:
    `navigate_to(path)` (push + set dir), `go_back()`, `go_forward()`, `go_up()`.
- **`PaneNode`**: un panel concreto en el workspace = `{ id: PaneId, purpose:
  PanePurpose, files: Option<FilePaneState> }` (sólo los `Files` llevan
  `FilePaneState`; `Tree`/`Inspector` no necesitan ese estado pesado en 2A).
- **`Workspace`**: la colección de paneles + el panel activo.
  - `panes: Vec<PaneNode>` (el *orden/disposición* visual la maneja egui_dock en la
    capa ui; `Workspace` guarda el contenido y una representación serializable del
    layout — ver `config`).
  - `active: PaneId`.
  - `active_files_mut() -> Option<&mut FilePaneState>`: el panel `Files` activo, o
    el que el inspector debe reflejar.
  - `add_pane(purpose) -> PaneId`, `remove_pane(id)`, `set_active(id)`.
- **`LayoutTemplate`**: una disposición nombrada y serializable.
  - `{ name: String, builtin: bool, favorite: bool, panes: Vec<TemplatePane>,
    layout: SerializableDockLayout }`.
  - `TemplatePane` describe un panel del preset (su `PanePurpose` y, para `Files`,
    su carpeta inicial: home, una ruta fija, o "heredar del activo").
  - Constructores de los built-in: `minimalista()`, `clasico()`, `dual_pane()`,
    `power_user()`.

> **Aislamiento del layout de egui_dock:** `core` no debe depender de egui_dock.
> Se define en `core` un tipo propio **`SerializableDockLayout`** (un árbol de
> splits con orientación/fracción y, en las hojas, los `PaneId`/`PanePurpose`). La
> capa `ui` traduce entre `SerializableDockLayout` y el `DockState<PaneId>` de
> egui_dock. Así `core` permanece testeable y portable, y la persistencia no
> depende del formato interno de egui_dock.

### Capa `core` — módulo nuevo `config` (persistencia JSON portable)

Materializa lo previsto en el spec del núcleo. Tres archivos independientes (uno
corrupto no tumba a los otros), serde_json, ubicados junto al `.exe` (portable):

- **`workspace.json`**: el `Workspace` serializado — disposición
  (`SerializableDockLayout`), carpeta de cada `Files`, panel activo, `show_dirs` y
  `text_filter` por panel. **No** persiste `NavHistory` (arranca limpio cada
  sesión). **No** persiste `entries` (se re-listan al abrir).
- **`templates.json`**: las plantillas del usuario (`builtin: false`) + el flag
  `favorite` + la lista de **recientes** (IDs/nombres con timestamp). Las built-in
  son código, no se guardan aquí.
- **`settings.json`**: posición de la barra (`Top` | `Side`), `icon_only: bool`.

API de `config`:
- `load_workspace() -> Workspace` — parsea `workspace.json`; **tolerante**: si
  falta, está corrupto, o es de versión incompatible → devuelve el workspace de la
  plantilla **Dual-pane** (default) y loguea el problema.
- `save_workspace(&Workspace)`.
- `load_templates() -> TemplateStore` / `save_templates(&TemplateStore)`.
- `load_settings() -> Settings` / `save_settings(&Settings)`.
- Cada `load_*` nunca falla hacia el llamador: error → default + log. Versionado:
  un campo `version` permite migrar/descartar en el futuro.

### Capa `ui`

- **`app.rs`**: `NaygoApp` ahora tiene un `Workspace` (no un solo `PaneState`) y un
  `DockState<PaneId>` de egui_dock. En el arranque: `config::load_workspace()` →
  traducir a `DockState` → lanzar un worker de listing por cada `Files` en su
  carpeta. En el cierre (o tras cada cambio relevante): `config::save_workspace()`.
- **`docking.rs`**: el `TabViewer` despacha por `PaneId` → busca el `PaneNode` en el
  workspace y pinta según `PanePurpose` (Files/Tree/Inspector). El inspector lee el
  panel `Files` **activo**.
- **`panes/`**: `file_panel`, `tree_panel`, `inspector_panel` (ya existen) +
  ajustes: el file panel respeta `show_dirs`, muestra breadcrumb, y marca el panel
  activo. El árbol del Fase 1 sigue siendo esqueleto (su versión expandible real es
  trabajo posterior; en 2A muestra la ubicación y permite subir).
- **`toolbar.rs`** (nuevo): la barra de íconos (atrás/adelante/arriba/refrescar,
  Layouts, agregar panel, configuración). Renderiza arriba o al costado según
  `Settings.bar_position`. Botones solo-ícono con `tooltip` (texto provisional
  hardcoded hasta 2C). Atrás/adelante se habilitan según `can_back/can_forward` del
  panel activo.
- **`templates_menu.rs`** (nuevo): el combobox que despliega el ícono "Layouts" —
  secciones Recientes / Míos / Built-in + "Guardar disposición actual…" + marcar
  favorito / borrar (sólo plantillas propias).
- **`input.rs`**: amplía el mapeo — agrega `Alt+←`=atrás, `Alt+→`=adelante, y lee
  `PointerButton::Extra1/Extra2` del mouse (egui los expone) → atrás/adelante del
  panel activo. El mapeo sigue siendo función pura testeable donde se pueda.

### Flujo de datos (cómo sigue siendo fluido)

Cada panel `Files` mantiene su propio canal con su worker de listing (patrón de
Fase 1, sin cambios). Agregar un panel = lanzar un worker más; cambiar de carpeta
en un panel = cancelar su worker y lanzar otro. El hilo de UI drena los canales de
todos los paneles por `try_recv` (no bloqueante) cada frame. **N paneles listan en
paralelo sin que la UI se trabe.**

### Principio de unidades pequeñas

`NavHistory`, `FilePaneState`, `Workspace`, `LayoutTemplate`, `config` son unidades
con una responsabilidad y una interfaz clara, testeables de forma aislada. El
`SerializableDockLayout` desacopla `core` de egui_dock. La UI se parte en
`toolbar` / `templates_menu` / `docking` / paneles, cada archivo enfocado.

---

## 3. Modelo de interacción

### Paneles
- **Agregar panel:** ícono `➕` → elegir tipo (Files / Tree / Inspector); aparece en
  el dock, arrastrable.
- **Quitar panel:** botón de cerrar del tab (egui_dock).
- **Reacomodar:** arrastrar tabs (egui_dock nativo).
- **Panel activo:** clic en un panel o `Tab` cicla entre paneles `Files`; el activo
  se marca visualmente (p. ej. borde). El inspector refleja el activo.

### Navegación (sobre el panel activo)
- `Enter` / doble clic: entrar a carpeta / abrir archivo (abrir con default llega
  con `platform::shell`, fase posterior; en 2A muestra aviso "pendiente").
- `Backspace`: subir un nivel (entra al historial).
- **`Alt+←` / botón lateral 1 del mouse:** atrás.
- **`Alt+→` / botón lateral 2 del mouse:** adelante.
- Botones de toolbar equivalentes, deshabilitados cuando no hay a dónde ir.
- `↑↓`, type-ahead, `Esc` (cancelar listado): como en Fase 1.

### Plantillas
- Ícono **`▦ Layouts`** → combobox: **🕘 Recientes**, **👤 Míos**, **📋 Built-in**,
  y **💾 Guardar disposición actual…**.
- Aplicar una plantilla recompone los paneles (lanza listados nuevos).
- Marcar/desmarcar **favorito** (★) y **borrar** disponibles para plantillas
  propias en el combobox; la administración fina (renombrar, limpiar recientes,
  editar filtros) vive en **Configuración** (UI de config = pulido posterior).

### Barra
- Posición **superior** o **lateral**, elegible por el usuario (persistido).
- Botones **solo-ícono** con tooltip (default). (Texto opcional = pulido posterior.)
- `⚙` abre Configuración (en 2A, mínima: posición de barra; el resto se irá
  llenando en fases siguientes).

---

## 4. Persistencia y manejo de errores

### Persistencia
- `workspace.json`, `templates.json`, `settings.json` — JSON portable al lado del
  `.exe`, independientes entre sí, con campo `version`.
- Se guarda el workspace al cerrar y tras cambios relevantes (cambiar plantilla,
  agregar/quitar panel, cambiar carpeta de un panel). Throttle razonable para no
  escribir en cada tecla.
- **No** se persisten: `NavHistory` (limpio por sesión), `entries` (se re-listan).

### Errores (filosofía del proyecto: el filesystem es hostil, la app nunca cae)
- JSON ausente / corrupto / versión incompatible → se ignora ese archivo, se cae al
  default (Dual-pane / settings default), se loguea. Nunca crashea.
- Carpeta persistida que ya no existe (disco desconectado, ruta borrada) → ese panel
  abre en `home` con aviso discreto; el resto del workspace carga normal.
- Layout con un `PanePurpose` desconocido (de una versión futura) → se omite ese
  panel, el resto se restaura.
- Un worker de listing que falla (permiso, disco caído) → ese panel muestra el
  error en su estado; los demás paneles siguen vivos (aislamiento por panel +
  cancelación de Fase 1).

---

## 5. Testing

La ganancia de tener `core` puro se mantiene y crece:

- **`NavHistory`**: `push` trunca la rama de adelante; `back`/`forward` mueven el
  cursor; `can_back`/`can_forward`; límites (atrás sin historial → `None`); tope de
  profundidad.
- **`FilePaneState`**: `navigate_to`/`go_back`/`go_forward`/`go_up` actualizan
  `current_dir` e `history` coherentemente; `show_dirs` filtra (la lógica de filtro,
  pura).
- **`Workspace`**: `add_pane`/`remove_pane`/`set_active`; `active_files_mut`
  apunta al panel correcto; quitar el panel activo reasigna el activo.
- **`LayoutTemplate`**: cada built-in produce la composición esperada; guardar la
  disposición actual como plantilla; round-trip de `SerializableDockLayout`.
- **Recientes/favoritos** (`TemplateStore`): aplicar registra en recientes (orden
  por timestamp, tope N, sin duplicados); marcar favorito persiste.
- **`config`**: round-trip de los 3 archivos; parsing tolerante (corrupto/ausente/
  versión vieja → default sin panic).
- **UI** (barra, combobox, traducción `SerializableDockLayout`↔`DockState`, mapeo de
  botones del mouse): validación manual; la lógica testeable se extrae a `core`.

Meta de siempre: build limpio + tests pasando + clippy limpio antes de cada commit.

---

## 6. Estructura de archivos prevista (incremental sobre Fase 1)

```
crates/core/src/
├── lib.rs                # + re-exports de workspace y config
├── cancel.rs             # (existe)
├── fs_model.rs           # (existe) — PaneState queda; FilePaneState lo extiende
├── sort.rs               # (existe)
├── listing.rs            # (existe)
├── workspace/
│   ├── mod.rs            # Workspace, PaneNode, PaneId, PanePurpose
│   ├── nav_history.rs    # NavHistory (puro, muy testeado)
│   ├── file_pane.rs      # FilePaneState (+ show_dirs, text_filter reservado, nav)
│   ├── template.rs       # LayoutTemplate, built-ins, TemplateStore (recientes/fav)
│   └── layout.rs         # SerializableDockLayout (desacople de egui_dock)
└── config/
    └── mod.rs            # load/save de workspace.json / templates.json / settings.json

crates/ui/src/
├── app.rs                # Workspace + DockState<PaneId>; carga/guarda config
├── docking.rs            # TabViewer despacha por PaneId/PanePurpose
├── toolbar.rs            # barra de íconos (posición configurable, solo-ícono)
├── templates_menu.rs     # combobox de plantillas (recientes/favoritos/built-in)
├── input.rs              # + Alt+←/→, PointerButton::Extra1/Extra2
└── panes/
    ├── file_panel.rs     # respeta show_dirs, breadcrumb, marca activo
    ├── tree_panel.rs     # (esqueleto, como Fase 1)
    └── inspector_panel.rs# refleja el panel Files activo
```

---

## 7. Dependencias

Sin dependencias nuevas previstas más allá de las de Fase 1 (`eframe`/`egui`
0.34, `egui_dock` 0.19, `serde`/`serde_json`, `tracing`). El mapeo de botones del
mouse usa `egui::PointerButton::Extra1/Extra2` (ya disponible). A confirmar en el
plan si hace falta algo para timestamps de "recientes" (probablemente pasar el
tiempo desde la capa ui, ya que `core` evita `std::time::SystemTime::now()` en
lógica pura para mantener tests deterministas — el timestamp se inyecta).

---

## Fuera de alcance (recordatorio — NO en 2A)

Íconos reales (2B), i18n (2C), temas/color sets (2C), drag&drop con el SO,
operaciones de archivo, tamaño de carpeta, menú contextual nativo, árbol expandible
con lazy-load, filtro de texto rápido por panel (campo reservado, sin UI), UI
completa de Configuración, botones de toolbar con texto. Nunca: reproducción de
media, edición de archivos.
