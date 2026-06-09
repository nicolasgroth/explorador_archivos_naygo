# Naygo — Pulido / bugfix post-prueba — Diseño

> Fase corta de corrección tras la primera prueba de Nicolás en uso real. Cuatro
> frentes: (1) repaint reactivo (clics que no toman al cargar + hover lento),
> (2) íconos Flat completos, (3) vista previa del pack en Configuración,
> (4) límites de ventana + contenido responsive.

Autor: Nicolás Groth / ISGroth (Chile), 2026, MIT.

## Contexto y hallazgos de la prueba

Nicolás probó Naygo (release/instalador) y reportó:
- Al cargar, los clics sobre archivos/carpetas NO toman; recién tras marcar algo
  empieza a responder. El hover (línea azul) tarda en pintarse — "lento". Sospecha
  (correcta) que se relaciona con el arranque.
- Pack de íconos Flat: 7 íconos salen como cuadros vacíos.
- Al elegir "Pack de íconos" no hay forma de ver/cambiar qué pack se usa.
- La ventana de Configuración se achica demasiado y el contenido se corta (no
  responsive). El explorador y sus paneles tampoco tienen tope mínimo.

Estado verificado en el código:
- `crates/ui/src/main.rs`: `ViewportBuilder` SIN `with_min_inner_size` (sin tope mínimo).
- `crates/ui/src/app.rs`:
  - `impl eframe::App for NaygoApp` usa `logic()` + `ui()` (no el `update` deprecado).
  - `logic` (~2296): si `self.splash.is_some()` → `ctx.request_repaint(); return;`
    (no procesa input ni pumps durante el splash — correcto).
  - Tras el splash hace `pump_all/tree/ops/disk/sizing/watchers/devices/paste`.
  - Repaint condicional (~2323): `if any_listing_active() || any_tree_listing_active()
    || ... { ctx.request_repaint(); }` y `request_repaint_after(500ms)` (~2336) si hay
    watchers/device. NO hay repaint continuo en idle (correcto para bajo consumo) — pero
    NO hay repaint mientras el puntero está sobre un panel, así que el hover se siente
    lento (egui repinta una vez por evento de movimiento, no continuo) y el primer
    frame tras el splash/carga puede "perder" clics hasta que llega otro evento.
  - `fn any_listing_active(&self) -> bool` (933).
- `assets/icons/flat/`: 7 PNG de ~230 bytes = placeholders (action_copy, action_cut,
  action_paste, action_new_file, action_new_folder, file_font, file_model3d). Los
  demás son íconos reales. Fluent y Mono están 26/26 reales.
- `crates/ui/src/settings_window/`: la sección Apariencia tiene el toggle Glifos/Pack
  y el selector de Set (Flat/Fluent/Mono) por separado; sin preview de los íconos del
  pack ni ScrollArea de respaldo.
- `egui_dock` 0.19: maneja los splits del dock; verificar su API de tamaño mínimo de
  nodo/tab al implementar.

## Decisiones tomadas (brainstorm 2026-06-09)

- **Repaint**: enfoque REACTIVO AFINADO (no continuo global). Repaint inmediato tras el
  splash y mientras hay listados; repaint continuo SOLO mientras el puntero está sobre
  un panel de archivos (para hover fluido); idle real (mouse afuera, sin jobs) sigue sin
  repintar → bajo consumo intacto.
- **Íconos Flat**: completar los 7 faltantes con una fuente libre que SÍ los tenga
  (fluentui-emoji, colorido como Flat; lucide como fallback). Flat queda 26/26.
- **Selector de pack**: VISTA PREVIA de los íconos del set activo en Configuración al
  elegir "Pack de íconos" + dejar claro que el set se elige en "Set de íconos". (Visual:
  Nicolás revisa antes de fijar.)
- **Límites de ventana**: mínimos permisivos — principal **640×400**, Configuración
  **460×360**, paneles del dock **~150px**; Configuración con ScrollArea + contenido
  responsive.

## Componentes

### 1. Repaint reactivo afinado (`crates/ui/src/app.rs`)

- **Hover fluido**: en `logic` (o `ui`), tras los pumps, si el puntero está sobre el
  área de la app (o específicamente sobre un panel de archivos), `ctx.request_repaint()`.
  Forma simple y robusta: `if ctx.input(|i| i.pointer.has_pointer()) { ctx.request_repaint(); }`
  — repinta continuo mientras el mouse está dentro de la ventana, idle cuando sale.
  (Si se quiere acotar SOLO a paneles de archivos, hace falta saber el rect del panel;
  empezar con "puntero dentro de la ventana" que es simple y cubre el síntoma; afinar a
  panel-only solo si el consumo en idle-con-mouse-dentro molesta — verificar el método
  real de egui 0.34: `i.pointer.has_pointer()` / `i.pointer.latest_pos().is_some()` /
  `ctx.is_pointer_over_area()`.)
- **Primer frame post-splash**: al cerrar el splash (`self.splash = None`), solicitar un
  repaint explícito para que el frame siguiente renderice y procese input de inmediato
  (sin esperar un evento). Confirmar que el frame inmediatamente posterior al splash ya
  corre los pumps y atiende clics.
- **Primer listado**: confirmar que `any_listing_active()` es `true` durante el primer
  listado por streaming del arranque (si no, los resultados llegan pero no se repinta
  hasta un evento). Si hay un hueco, solicitar repaint mientras haya un listado recién
  lanzado.
- NO introducir repaint continuo incondicional (violaría bajo consumo). El idle con el
  mouse FUERA de la ventana no debe repintar.

### 2. Íconos Flat completos (assets)

- Reemplazar los 7 placeholders en `assets/icons/flat/` por íconos reales rasterizados
  (~32px) de una fuente libre: preferir **fluentui-emoji** (colorido, combina con Flat)
  para `action_copy/cut/paste/new_file/new_folder`, `file_font`, `file_model3d`; usar
  **lucide** si fluentui no tiene un equivalente claro (pero entonces sería monocromo —
  preferir fluentui para coherencia de color). Nombres EXACTOS que `assets.rs` espera.
- Actualizar `assets/icons/NOTICE.md` si se suma una fuente nueva a Flat.
- Tolerante: si alguno no se logra, dejar un genérico decente del propio pack (NO el
  cuadro vacío). Reportar cuáles se completaron y con qué fuente.
- Los zips NO se commitean (sigue el `.gitignore`).

### 3. Vista previa del pack en Configuración (`settings_window`)

- En la sección Apariencia, cuando `toolbar_icon_style == Pack` (o siempre, junto al
  selector de Set), mostrar una FILA DE PREVIEW: varios íconos del set activo
  (p. ej. folder, file_image, file_code, action_copy, action_settings) renderizados con
  el `IconProvider`, a ~24px. Así el usuario ve cómo se ve el pack antes de aplicar.
- Texto/etiqueta que aclare: el pack se elige en "Set de íconos" (Flat/Fluent/Mono); el
  toggle Glifos/Pack decide si la toolbar usa glifos o esos íconos.
- (Visual: Nicolás revisa el layout del preview antes de fijarlo.)

### 4. Límites de ventana + responsive

- **Ventana principal** (`main.rs`): `ViewportBuilder::with_min_inner_size([640.0, 400.0])`.
- **Configuración** (`settings_window`): el viewport de settings con
  `with_min_inner_size([460.0, 360.0])`; envolver el contenido de cada sección en un
  `egui::ScrollArea::vertical()` para que no se corte si la ventana es chica; revisar
  que las tarjetas de tema / selectores se reacomoden (wrap) en anchos chicos.
- **Paneles del dock**: tamaño mínimo de ~150px por panel al arrastrar el divisor.
  Verificar la API de `egui_dock` 0.19 (`Style`/`DockState` puede tener un
  `min_window_width`/separación mínima; si no lo expone limpio, documentar y dejar el
  mínimo de la ventana como red de seguridad).

## Verificación (reparto)

- **El agente**: `cargo build --workspace`, `cargo test --workspace`, `clippy
  --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` verdes;
  confirmar que los íconos Flat ya no son de 230 bytes (tienen contenido real); que
  `main.rs` setea el mínimo; que no se commitea ningún zip.
- **Nicolás (visual/manual)**: al cargar, los clics toman de inmediato; el hover pinta
  la línea sin lag; achicar la ventana principal y Configuración respeta el mínimo y el
  contenido no se corte/scrollea; el pack Flat se ve completo (sin cuadros); el preview
  del pack en Configuración se ve bien.

## Fuera de alcance (explícito)

- **Multi-selección** (rubber-band + Ctrl/Shift) — PRÓXIMA fase, su propio brainstorm
  visual. (El campo `f.selected` ya existe reservado.)
- Inline rename (F2 editando en la fila), selección por teclado (Shift+flechas, Ctrl+A,
  Espacio), drag&drop interno entre paneles, agrupar — backlog.
- Firma de código.

## Notas de riesgo / cuidado para el plan

- **Método de "puntero dentro" en egui 0.34**: verificar el nombre real
  (`i.pointer.has_pointer()` vs `latest_pos().is_some()` vs `ctx.is_pointer_over_area()`).
  Elegir el que signifique "el mouse está sobre la ventana/un área" para acotar el
  repaint continuo. Medir que el idle SIN mouse encima no repinte (no romper bajo consumo).
- **Primer frame post-splash**: el `return` temprano en `logic` durante el splash no debe
  dejar un frame muerto al cerrarlo; al limpiar `self.splash` pedir repaint.
- **egui_dock 0.19 min size**: confirmar API; si no hay tamaño mínimo por panel, el
  mínimo de ventana (640×400) es la red de seguridad y se documenta la limitación.
- **Íconos**: extraer de los zips ya presentes en `assets/icons/*.zip` (NO commitearlos);
  fluentui para color. Verificar que los nombres calcen con `assets.rs`.
- **ScrollArea en Configuración**: no romper el layout actual de las secciones; envolver
  sin cambiar la lógica.
- **Documentación**: comentar el porqué del repaint condicional (es sutil — un tercero
  podría "simplificarlo" a repaint continuo y romper bajo consumo). Nota en el código.
