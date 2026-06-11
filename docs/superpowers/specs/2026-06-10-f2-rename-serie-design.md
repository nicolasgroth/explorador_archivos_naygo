# R1 — F2 rename inline en serie — Diseño

> Autor: Nicolás Groth / ISGroth — 2026-06-10. MIT License.
> Fase 1 de 3 del plan rename+undo aprobado (R1 F2 serie → R2 undo → R3 batch).

## Qué es

F2 deja de abrir un diálogo: edita el nombre **inline sobre la fila** (como Explorer),
con dos superpoderes pedidos por Nicolás:

- **Ciclo de selección con F2** (estando editando): 1ª vez pre-selecciona el NOMBRE
  (sin extensión), 2ª la EXTENSIÓN, 3ª TODO, y el ciclo se repite. Carpetas o nombres
  sin extensión: todas las etapas seleccionan todo.
- **Rename en serie con ↑/↓**: estando editando, ↑/↓ CONFIRMA el nombre actual y abre
  la edición en el ítem de arriba/abajo (con el nombre pre-seleccionado de nuevo).

Enter confirma; Esc cancela; perder el foco (clic afuera) confirma.

## Diseño

### Estado (`NaygoApp`)
`inline_rename: Option<InlineRename { pane: PaneId, pos: usize /* pos en la VISTA */,
text: String, stage: u8, focus_pending: bool, select_pending: bool }>`.
`begin_rename()` (Action::Rename/F2 global) lo inicializa con `f.focused` +
`focused_view_entry()` en vez de abrir `PendingDialog::Rename` (el diálogo viejo se
elimina de ese camino).

### Render (file_panel, celda Nombre)
Si `inline_rename` apunta a este panel+fila: ícono + `TextEdit::singleline` (id fijo
por panel) en lugar del label. `focus_pending` → `request_focus()`;
`select_pending` → setear el rango de selección en `TextEditState` según `stage`
(rangos por CHARS con `rsplit_once('.')`; sin extensión → todo).

### Teclas (consumidas ANTES del TextEdit con `consume_key`)
- F2 → `stage = (stage+1) % 3` + `select_pending`.
- ↑/↓ → marca commit + travel ±1.
- Esc → cancela.
- Enter / lost_focus → commit.
El handler global de input se SUSPENDE mientras `inline_rename.is_some()` (mismo
patrón que `pending_dialog`/`shortcut_capture`).

### Commit y travel (tras pintar la tabla, donde vive la `view`)
- Commit: si `text` difiere del nombre actual y es válido (no vacío, sin `\/:*?"<>|`)
  → `PaneRequest::CommitRename { id, source, new_name }` → la app lo convierte en
  `ops_actions::rename` + `start_op` (idéntico al accept del diálogo viejo; cuando
  exista R2, queda journaled gratis). Nombre inválido → status `op.invalid_name`, se
  sigue editando. Sin cambios → no-op.
- Travel: nueva posición = `pos ± 1` acotada a `0..view.len()` (la vista son solo
  entries; la fila ".." no participa). Estado nuevo con el nombre de esa entry,
  `stage = 0`, focus+select pendientes.

### Tolerancia
La vista puede reordenarse cuando el rename async aterriza (watcher): igual que
Explorer, el viaje usa las posiciones de la vista actual. Si la fila editada
desaparece (carpeta cambió), el estado se descarta sin crash.

## Verificación
En vivo (computer-use) sobre una carpeta de prueba con archivos descartables: F2
edita inline con nombre pre-seleccionado; F2 cicla nombre→ext→todo; ↓ confirma y baja;
Esc cancela; Enter confirma; el rename real ocurre (fs) y se refleja.
