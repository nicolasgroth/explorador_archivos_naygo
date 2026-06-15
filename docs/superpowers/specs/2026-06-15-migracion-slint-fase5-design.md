# Fase 5 (Slint): integraciones con el SO — Diseño

> Migración egui→Slint de Naygo. Esta fase **cablea** en la capa Slint las integraciones con
> Windows que ya viven en `naygo-platform`, y **porta** tray/splash (hoy solo en la capa egui).
> Cero pérdida de funcionalidad (contrato `docs/migracion-slint/CONTRATO-PARIDAD-FUNCIONAL.md`).

## Contexto y alcance

Ya hecho en fases previas (NO se re-hace): menú contextual nativo del Shell (F3), papelera
recuperable (F3, vía `core::ops` + `platform::trash`), abrir / "Abrir con…" (F3), detección de
unidades + espacio (F2b), locale del SO (F4).

**Falta cablear en Slint (alcance de F5):**
1. Watcher de carpeta (auto-refresh + resaltado de archivos nuevos).
2. Watcher de dispositivos (USB: enchufar/quitar unidades).
3. Drag&drop OLE — **sacar** archivos de Naygo hacia el Explorer/escritorio/otra app.
4. Drag&drop OLE — **recibir** archivos soltados sobre un panel (copia/mueve a esa carpeta).
5. Tray (bandeja) + «cerrar a bandeja» + autostart (iniciar con Windows).
6. Splash de arranque (release).

**Principio rector:** toda la lógica pesada está en `naygo-platform` (`dnd`, `dir_watch`,
`device_watch`, `autostart`, `trash`) y en `naygo-core`; F5 la **cablea**. El hilo de UI nunca
hace I/O: workers en hilos propios + canales. Bajo consumo: los watchers están **dormidos** salvo
ante un evento real, que despierta la UI con un *waker* (en Slint: `slint::Weak` +
`invoke_from_event_loop`). Subordinado a la velocidad (lema del proyecto).

**Decisiones de Nicolás (brainstorm):** todo el bloque en sub-fases; drag&drop en ambas
direcciones; watcher refresca **y** resalta lo nuevo (respeta `highlight_duration` /
`new_items_at_end`); tray **opt-in** respetando `tray_enabled` / `close_to_tray`.

## Arquitectura — 6 sub-fases verticales

Cada sub-fase: compila + tests + verificación en vivo (binario real, esta máquina) + commit.
Nicolás mide **rendimiento en la VM** al cierre.

APIs de `naygo-platform` que se reusan (verificadas):
- `dir_watch::watch(dir: &Path, tx: Sender<Vec<DirEvent>>, waker: Waker) -> WatchHandle`
  (`Waker = Arc<dyn Fn() + Send + Sync>`; el Drop del handle detiene el watcher; handle inerte
  si no se puede vigilar).
- `device_watch` (ventana oculta WM_DEVICECHANGE → eventos de unidad por canal).
- `dnd::start_drag(paths: &[PathBuf]) -> Result<DragOutcome, DndError>` (bloqueante: corre el
  bucle OLE DoDragDrop hasta soltar; `DragOutcome` = copiado/movido/cancelado).
- `autostart` (entrada Run del registro: set/clear/is_enabled).
- `trash::move_to_trash` (ya consumido por core::ops en F3).

### 5A — Watcher de carpeta
- **Data flow:** al `start_listing(id, dir)` arrancar también `dir_watch::watch(dir, tx, waker)`
  y guardar el `WatchHandle` en `watchers: HashMap<PaneId, WatchHandle>`. Un canal compartido
  `Sender<Vec<DirEvent>>`; nuevo `pump_watch()` en el tick drena los `DirEvent` y los aplica al
  `FilePaneState` del panel (reusa la lógica de aplicar eventos del core que ya usa egui:
  add/remove/rename + marca de aparición para resaltar). Los nuevos se pintan con el color
  `highlight` del tema durante `settings.highlight_duration`; respeta `settings.new_items_at_end`.
- **Waker (Slint):** `slint::Weak<AppWindow>` capturado en un `Arc<dyn Fn()+Send+Sync>` que hace
  `invoke_from_event_loop(|| { if let Some(ui)=weak.upgrade() { /* re-tick */ } })`. Despierta la
  UI dormida solo ante un evento real (preserva 0% CPU en reposo).
- **Ciclo de vida:** al navegar, el `WatchHandle` viejo del panel se reemplaza (Drop lo detiene);
  al cerrar el panel se quita del mapa.
- **Errores:** `watch` da handle inerte si la carpeta no se puede vigilar (red/permiso) — el
  panel sigue, solo no auto-refresca.
- **Testing:** el aplicar-evento es del core (ya testeado). Test de integración ui-slint con
  tempdir: navegar, crear un archivo, drenar `pump_watch`, verificar que aparece y queda marcado
  como reciente.

### 5B — Watcher de dispositivos (USB)
- **Data flow:** `device_watch` corre su hilo con ventana oculta; emite "unidad agregada/quitada"
  por canal. El tick drena: re-lee unidades y refresca el `DirTree` de los paneles Tree; si un
  panel Files estaba parado en una unidad que desapareció, lo reubica a una ruta válida (HOME).
- **Robustez:** sacar un USB con un panel adentro NO tira la app (Result tipado, reubicación).
  Si la ventana oculta no se puede crear, queda inerte.
- **Testing:** la detección real la verifica Nicolás (hardware). Se testea la lógica pura
  "reubicar panel si su raíz ya no existe" con rutas sintéticas.

### 5C — Drag&drop OLE (sacar)
- **Data flow:** un arrastre sostenido sobre filas YA seleccionadas (botón presionado + umbral de
  movimiento) dispara `dnd::start_drag(paths_seleccionadas)`. Es **bloqueante** (bucle OLE nativo
  hasta soltar) y devuelve `DragOutcome`; ese es el comportamiento esperado del SO.
- **Slint:** distinguir "clic" de "inicio de arrastre" por umbral con botón presionado, SIN pisar
  los gestos ya cableados (selección, doble-clic de F2/F3, grip de reordenar paneles de F2c). El
  arrastre OLE se inicia desde el cuerpo de una fila seleccionada, no desde la barra de título del
  panel (que ya es el asa de reordenar).
- **Errores:** si `start_drag` falla (COM), no-op discreto.

### 5D — Drag&drop OLE (recibir)
- **Data flow:** registrar el HWND de Naygo como *drop target* del Shell (`RegisterDragDrop` +
  un `IDropTarget`); al soltar archivos externos, leer CF_HDROP (rutas) y mapear el punto de
  drop al panel bajo el cursor → lanzar una op de copia (o mover si Shift) a la carpeta de ese
  panel, **reusando el engine de ops de F3** (diálogos de conflicto + panel de progreso +
  cancelación). 
- **Complejidad/riesgo (honesto):** registrar el drop-target en el HWND de winit es lo más
  delicado de F5. Si choca con el manejo de winit, el fallback acotado es: drop a nivel de
  **ventana** → va al **panel activo** (no por panel bajo el cursor). Se documenta si se toma el
  fallback.
- **Errores:** drop de algo que no son archivos (texto/imagen) → se ignora o se delega al pegado
  de F3 según formato; sin crashear.

### 5E — Tray + cerrar-a-bandeja + autostart
- **Tray:** módulo nuevo `crates/ui-slint/src/tray.rs` (porta `crates/ui/src/tray.rs`, crate
  `tray-icon`): ícono en bandeja con el ícono del .exe + menú **Abrir** / **Salir** + canal
  `TrayMsg`. El tick drena: *Abrir* → muestra/eleva la ventana; *Salir* → `quit_event_loop`. Se
  crea solo si `settings.tray_enabled`.
- **Arreglo del cierre (deuda de F4):** hoy `on_close_requested` siempre devuelve `HideWindow` →
  el proceso queda colgado. Nuevo:
  - `close_to_tray == true` **y** tray activo → `HideWindow` (oculto a bandeja, vivo a propósito).
  - resto → guardar sesión + **`slint::quit_event_loop()`** (la app termina de verdad).
  Helper puro `should_quit_on_close(close_to_tray: bool, tray_active: bool) -> bool` (testeable).
  La persistencia ya no depende del cierre (huella por tick, F4); guardar acá es refuerzo.
- **Autostart:** toggle "Iniciar con Windows" en Configuración → General → `platform::autostart`
  (entrada Run del registro). Persistido en Settings (campo nuevo si no existe, con
  `#[serde(default)]`).
- **Errores:** tray que no se puede crear → app normal sin ícono. Autostart que falla (permiso) →
  no-op + aviso discreto.
- **Testing:** `should_quit_on_close` (puro). Toggle autostart con guard `#[cfg(windows)]` /
  registro condicional. La bandeja real la verifica Nicolás.

### 5F — Splash de arranque (release)
- **Data flow:** en release, mostrar brevemente una ventana de bienvenida (logo + "Naygo" +
  "© 2026 Nicolás Groth / ISGroth") que se cierra sola a ~1–1.5 s o cuando la ventana principal
  está lista. En debug se omite (arranque directo). Componente `splash.slint` o ventana mínima
  cerrada por timer. NO retrasa el arranque real (la principal carga por detrás).
- **Cierre de fase:** verificación integral en vivo (watchers, drag sacar/recibir, tray,
  autostart, splash), gate del workspace, `graphify update .`, dist + memoria, push.

## Manejo de errores (transversal)
Coherente con "el filesystem es hostil": cada integración degrada limpio (handle inerte, no-op,
reubicar panel) en vez de crashear. La cancelación universal se respeta (las ops por drop son
cancelables como las de F3).

## Testing (transversal)
- Unit/integración donde la lógica es pura: aplicar-evento del watcher (core), reubicar-panel,
  `should_quit_on_close`, autostart con guard.
- Verificación en vivo (binario real, esta máquina, Win32/computer-use) para lo que toca el SO.
- Nicolás mide **rendimiento en la VM** al cierre (clave de la migración).

## Convenciones (obligatorias)
- Header en archivos nuevos; nombres en inglés; comentarios/commits en español.
- Antes de leer/grepear: `graphify query`. Tras cambios: `graphify update .`.
- Gate antes de cada commit: `cargo test --workspace` + `cargo clippy --workspace --all-targets
  -- -D warnings` + `cargo fmt --all -- --check`. Stage explícito (no `git add -A`; CLAUDE.md y
  graphify-out/ no se commitean). Commits con `Co-Authored-By: Claude Opus 4.8`.
- i18n: todo texto nuevo a claves (`Tr` + catálogos es/en en paridad). Colores vía `Theme`.

## Fuera de alcance (F6)
Paridad fina restante, instalador, retiro de la capa egui, rubber-band/Ctrl+arrastre de
selección, reordenar columnas por arrastre del encabezado (si quedaran pendientes).
