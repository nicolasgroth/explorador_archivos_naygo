# Naygo — Drag & Drop + pulidos + autoría — Diseño (Entrega 1)

> Drag & drop completo: interno entre paneles + con el SO (Explorer↔Naygo, OLE).
> Más pulidos chicos (persistencia del dock, fila "..", minors de multi-selección) y
> autoría explícita en los scripts. Una rama, sub-faseada, merge al final.

Autor: Nicolás Groth / ISGroth (Chile), 2026, MIT.

## Contexto y punto de partida

- **DnD interno (egui)**: ya se usa para reordenar columnas en `file_panel.rs`
  (`dnd_drag_source(id, payload, |ui|)`, `resp.dnd_release_payload::<usize>()`,
  `cell_resp.dnd_hover_payload::<usize>()` + línea de inserción). Patrón conocido a
  replicar para archivos.
- **Motor de ops**: `crates/ui/src/ops_actions.rs::transfer(kind_move: bool, sources:
  Vec<PathBuf>, dest_dir: PathBuf) -> OpRequest`. El DnD interno dispara `transfer`.
  `delete`/`create` también ahí. El motor (core::ops) ya cancela, journaliza, resuelve
  conflictos. NO reimplementar nada de ops.
- **Multi-selección** (recién mergeada): `FilePaneState.selected: Vec<usize>` (pos de
  vista), `selected_paths()` en app.rs mapea a `Vec<PathBuf>`. El arrastre desde la
  celda del NOMBRE quedó RESERVADO para DnD (la fase multi-selección NO dibuja
  rubber-band sobre el nombre). El DnD interno arranca ahí.
- **OLE**: `Win32_System_Ole` ya habilitado en `crates/platform/Cargo.toml`. NO hay
  nada de `IDropTarget`/`IDataObject`/`IDropSource`/`DoDragDrop`/`RegisterDragDrop` —
  todo el interop con el SO es CÓDIGO NUEVO. Patrón COM a seguir: `trash.rs` /
  `context_menu.rs` (CoInitializeEx apartment + balance RPC_E_CHANGED_MODE, unsafe
  acotado, errores tipados, nunca panic).
- **HWND**: capturado en `NaygoApp.hwnd: Option<isize>` (de shell-B, vía
  raw-window-handle). `RegisterDragDrop` necesita el HWND.
- **Dock persistence (deuda 2A)**: `crates/ui/src/dock_translate.rs` tiene
  `to_dock_state` (Workspace→DockState) y `dock_pane_ids`, pero NO un `from_dock_state`
  (DockState→layout). Consecuencia: paneles agregados con ➕ y reacomodos arrastrando
  tabs NO sobreviven al reinicio. `Workspace.layout` (SerializableDockLayout) se
  persiste pero no se actualiza desde el DockState vivo.
- **Fila ".."**: en file_panel `DisplayRow::Parent` ya usa `IconKey::Folder` y se
  activa con clic en cualquier celda (parece resuelto; VERIFICAR que se ve como una
  fila normal — si ya está bien, no tocar).
- **Minors de multi-selección** (del holistic review): M2 clic-derecho sobre una fila
  FUERA de la selección no la reduce a ese ítem (Explorer sí); M5 "Espacio" hardcoded
  en `chord_text` (input.rs) en vez de un símbolo neutro; M3 (rubber-band vs scroll) y
  M4 (temp-leak window) — verificar/cerrar si triviales.
- **Scripts**: `scripts/build-release.ps1` tiene header MIT pero conviene autor
  explícito; `crates/ui/src/bin/gen_icons.rs` y cualquier otro script/bin.

## Decisiones tomadas (brainstorm 2026-06-09, con companion visual)

- **Alcance DnD**: COMPLETO — interno + Explorer→Naygo (recibir) + Naygo→Explorer (sacar).
- **Mover vs copiar (estilo Windows)**: arrastrar = Mover (mismo disco); otro disco =
  Copiar; Ctrl = forzar Copiar; Shift = forzar Mover.
- **Botón del mouse**: IZQUIERDO = acción directa según la regla. DERECHO = menú al
  soltar ("Mover aquí / Copiar aquí / Cancelar").
- **Feedback (estilo B)**: banner fino arriba del panel destino ("Soltar para
  mover/copiar aquí") + badge con el conteo de ítems siguiendo el cursor. (NO teñir el
  panel entero.)
- **Multi-selección**: si hay varios seleccionados, el arrastre lleva TODOS.
- **Entrega**: una rama `feat/dnd-pulidos`, sub-faseada, merge al final.
- **OLE**: implementar completo; Nicolás valida con arrastres reales; el holistic final
  cubre el refcount COM.

## Componentes

### A1 · DnD interno entre paneles (egui)

- **Lógica pura (core)**: una función que decide la acción del drop dado (tecla
  modificadora, ¿mismo disco?) → `DropAction { Move, Copy }`. Testeable:
  `decide_drop_action(ctrl: bool, shift: bool, same_drive: bool) -> DropAction`
  (Shift→Move, Ctrl→Copy, si no: same_drive→Move else Copy). Vive en `core` (p. ej.
  `core::ops` o un `core::dnd` pequeño). "mismo disco" = comparar el prefijo de
  unidad de origen y destino (helper en core, Windows: la letra; tolerante).
- **UI (file_panel)**: la celda del NOMBRE de cada fila es `dnd_drag_source` con un
  payload que identifica el arrastre (las rutas seleccionadas, o un marcador para que
  app.rs resuelva `selected_paths()` al soltar). El panel destino detecta el hover de
  un payload de archivos (`dnd_hover_payload`) → pinta el banner B + badge. Al soltar
  (`dnd_release_payload`) → resolver la acción (mismo/distinto disco + modificadores) →
  con botón izquierdo ejecutar directo; con botón derecho, diferir un menú
  "Mover/Copiar/Cancelar". Dispara `transfer(kind_move, sources, dest_dir)`.
  NOTA: el arrastre debe distinguirse del rubber-band (que arranca en celdas NO-nombre)
  y de la selección por clic. El nombre = drag de archivos; el resto = rubber-band; el
  clic = selección. Coexistencia a resolver con cuidado (el plan detalla la mecánica
  egui: `dnd_drag_source` sobre la celda nombre vs el `Sense` de fila/fondo).
- El destino es el PANEL (su carpeta `current_dir`), no una fila concreta (soltar
  sobre una carpeta-fila para entrar en ella es un nice-to-have futuro; esta entrega
  suelta en la carpeta del panel destino).

### A2 · DnD con el SO (OLE/COM — platform, código nuevo)

Módulo nuevo `platform::dnd` (o `ole_dnd`), todo COM, patrón trash.rs:
- **Recibir (Explorer→Naygo)**: implementar un `IDropTarget` COM (DragEnter/DragOver/
  DragLeave/Drop) registrado con `RegisterDragDrop(hwnd, target)` al iniciar (y
  `RevokeDragDrop` al cerrar). En `Drop`, extraer las rutas del `IDataObject`
  (CF_HDROP), y entregarlas a la app (vía un canal/Sender, drenado por frame como los
  demás `pump_*`) con la posición → el panel bajo el cursor recibe el transfer.
  CUIDADO: OLE usa el apartment del hilo de UI; `RegisterDragDrop` requiere
  `OleInitialize` (no solo CoInitialize) — verificar el init correcto y su balance.
- **Sacar (Naygo→Explorer)**: al iniciar un drag de archivos desde el panel (cuando el
  destino del egui-drag NO es otro panel de Naygo, o siempre como mecanismo paralelo),
  construir un `IDataObject` con CF_HDROP de las rutas seleccionadas + un `IDropSource`,
  y llamar `DoDragDrop(...)` (bloqueante mientras dura el arrastre) → la app destino
  (Explorer/correo/editor) recibe los archivos. `DoDragDrop` devuelve el efecto
  (copy/move) → si move, Naygo re-lista.
  NOTA TÉCNICA: integrar `DoDragDrop` (modal, bucle propio de OLE) con el frame loop de
  egui es delicado — el plan debe resolver CÓMO se dispara (al detectar el inicio de un
  drag sobre la celda nombre cuyo destino sale de la ventana). Posible enfoque: el
  egui-drag interno maneja el caso intra-Naygo; para "sacar", detectar que el puntero
  salió de la ventana y entonces ceder a `DoDragDrop`. Si la integración resulta
  demasiado frágil, el plan puede empezar el drag OLE SIEMPRE al arrastrar desde el
  nombre (DoDragDrop maneja tanto el drop dentro como fuera) — DECIDIR en el plan según
  lo que funcione con egui/winit; documentar el enfoque elegido.
- Tolerante: cualquier fallo COM → se reporta, no crashea. Nunca panic. Liberar todo
  (refcount) con cuidado (Rust `windows` crate maneja Drop de interfaces; los HGLOBAL
  de CF_HDROP se liberan apropiadamente).

### B · Pulidos chicos

- **Dock persistence**: `from_dock_state(&DockState) -> SerializableDockLayout` en
  `dock_translate.rs`; al guardar el workspace (o al cerrar), leer el DockState vivo de
  egui_dock y persistirlo, para que ➕ y reacomodos sobrevivan. Verificar el formato de
  `SerializableDockLayout` y que el round-trip to/from sea consistente (test).
- **Fila ".."**: verificar que se ve como fila normal; si ya está, no tocar (documentar).
- **M2**: en el menú contextual de fila, si la fila clickeada-con-derecho NO está en
  `selected`, hacer `select_single(i)` antes de mostrar el menú (como Windows).
- **M5**: `chord_text` (input.rs) — "Espacio" → un símbolo/etiqueta neutra (o vía i18n
  si el editor de atajos lo permite; mínimo: consistente con "Alt+"/"↑").
- **M3/M4**: verificar; si triviales, cerrar (M4: limpiar el temp del rubber-band start
  incondicionalmente en drag_stopped). Si no, anotar como deuda.

### C · Autoría en scripts

- Header consistente en `scripts/build-release.ps1` (y cualquier script/bin como
  `gen_icons.rs`): `Creado por Nicolás Groth <ngroth@gmail.com> para ISGroth.` + la
  línea MIT existente. Pequeño, solo comentarios.

## Sub-faseo (orden de ejecución dentro de la rama)

1. A1 — DnD interno (lógica pura decide_drop_action + UI egui + transfer).
2. A2-recibir — IDropTarget (Explorer→Naygo).
3. A2-sacar — IDataObject + DoDragDrop (Naygo→Explorer).
4. B — pulidos (dock persistence, M2, M5, fila "..", M3/M4).
5. C — autoría en scripts.
Merge al final cuando todo está verde + holistic review (con foco en el refcount OLE).

## Verificación (reparto)

- **El agente**: tests PUROS (decide_drop_action; from_dock_state round-trip; M2 lógica
  si aislable); build/clippy --all-targets/fmt; i18n parity si se tocan claves.
- **Nicolás (visual/manual)**: arrastrar entre paneles (banner B, badge, mover/copiar
  por tecla/disco, botón derecho→menú); arrastrar desde Explorer a Naygo; arrastrar de
  Naygo a Explorer/correo; reiniciar y verificar que el layout del dock (➕/reacomodos)
  sobrevive; clic-derecho fuera de la selección la reduce.

## Fuera de alcance (explícito)

- "Acerca de…" + easter egg (ENTREGA 2, su brainstorm).
- Ícono en bandeja + iniciar con Windows (ENTREGA 3, arquitectónico).
- Soltar sobre una carpeta-FILA para entrar en ella (nice-to-have futuro; esta entrega
  suelta en la carpeta del panel destino).
- Inline rename F2, capas posteriores (miniaturas, visor, comprimir, batch-rename,
  caché, paleta Ctrl+P).

## Notas de riesgo / cuidado para el plan

- **DnD interno vs rubber-band vs clic** (A1): el nombre = drag de archivos
  (`dnd_drag_source`), las otras celdas/vacío = rubber-band (ya hecho), el clic =
  selección. Resolver la coexistencia con egui sin que se roben el gesto. Verificar que
  `dnd_drag_source` sobre la celda nombre no rompa el clic de selección de esa celda.
- **OLE init** (A2): `RegisterDragDrop`/`DoDragDrop` requieren `OleInitialize` (STA),
  no solo `CoInitializeEx`. Verificar el init correcto en el hilo de UI (eframe/winit ya
  inicializó algo — cuidar el balance, como en trash.rs). Esto es lo más delicado.
- **DoDragDrop modal vs egui loop** (A2-sacar): `DoDragDrop` corre su propio bucle
  modal y bloquea; integrarlo con el frame de egui es el punto más incierto. El plan
  debe decidir el disparo (al salir de la ventana, o siempre al arrastrar el nombre) y
  documentar; si una vía no funciona con winit, probar la otra. Posible que el
  egui-internal-drag y el OLE-drag necesiten un único punto de decisión "este drag es
  intra-app o sale al SO".
- **IDropTarget lifetime** (A2-recibir): el objeto COM debe vivir mientras la ventana
  exista; `RevokeDragDrop` al cerrar. El Sender al canal de la app dentro del COM
  object (como device_watch pasa el Sender). Refcount cuidado.
- **Refcount/HGLOBAL** (A2): CF_HDROP en HGLOBAL (DROPFILES), igual que ya se hace en
  `platform::clipboard::write_files` (de paste) — REUSAR ese código de construcción de
  DROPFILES si aplica (no duplicar). El holistic final revisa el refcount.
- **Dock round-trip** (B): `from_dock_state` debe producir un layout que `to_dock_state`
  reconstruya equivalente; test de round-trip. Cuidar paneles vacíos/cerrados.
- **`transfer` ya existe**: el DnD solo lo invoca; NO tocar el motor de ops.
- **Multi-selección en el drag**: usar `selected_paths()` (ya da la selección completa).
- **Documentación**: doc-comments del módulo OLE (el flujo COM), por ser lo más
  delicado y nuevo.
