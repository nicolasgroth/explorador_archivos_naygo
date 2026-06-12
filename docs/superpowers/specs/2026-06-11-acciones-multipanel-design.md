# Acciones multi-panel + selector numérico de panel — Diseño

> Fase 4 de la serie navegación. Idea de Nicolás (2026-06-11): Ctrl+doble-clic
> abre la subcarpeta en OTRO panel; íconos para intercambiar (swap) y clonar la
> carpeta entre paneles; y con 3+ paneles, elegir el destino oscureciendo cada
> panel candidato con un número grande y presionando 1-9 (teclas rápidas
> siempre). Mismo patrón probado que `tmux display-panes`.

## Regla central: el destino

Toda acción «hacia otro panel» resuelve su destino así:
- **2 paneles Files** → el OTRO, directo, sin preguntar (cero fricción en el
  caso común dual-pane).
- **3+ paneles Files** → **modo selector**: overlay que OSCURECE cada panel
  candidato (todos los Files menos el origen) y pinta un número grande (1..9,
  orden visual izquierda→derecha, arriba→abajo). Se elige con `1..9` (fila
  superior o numpad) o CLIC sobre el panel; `Esc` cancela. Máximo 9 candidatos
  (más allá no se numeran; no es un caso real).
- **1 panel Files** → la acción abre primero un segundo panel (split del
  actual, como ➕) y usa ese como destino (solo para «abrir en otro panel» y
  «clonar»; swap se deshabilita con un solo panel).

El selector vive en `NaygoApp` como `pending_pane_pick: Option<PanePick>`
(`PanePick { action, origin, candidates }`); mientras existe, suspende el input
global (mismo trato que los modales) y se pinta como overlay por encima del
dock. Es INFRAESTRUCTURA: futuras acciones («copiar a panel N», «mover a panel
N») reutilizan el mismo mecanismo.

## Las tres acciones

1. **Ctrl+doble-clic en una carpeta** del listado → la abre en el panel
   destino (el actual NO navega). El panel destino queda activo? No: el origen
   conserva el foco (estás explorando desde ahí); el destino solo navega.
2. **Swap ⇄** (ícono en la toolbar, junto a ⟳): intercambia las carpetas de
   los dos paneles (origen=activo ↔ destino). Re-lista ambos — con el caché de
   la fase 2, el intercambio es visualmente instantáneo. Historiales: cada
   panel hace `navigate_to` de su carpeta nueva (el swap es navegación, se
   puede deshacer con atrás).
3. **Clonar path** (ícono junto al swap): el panel destino navega a la carpeta
   del panel activo.

## Implementación (esqueleto)

- `PaneRequest::OpenInOther { from: PaneId, dir: PathBuf }` emitido por
  file_panel ante doble-clic con Ctrl (el handler de `double_clicked` ya
  existe; bifurca por modificador).
- `NaygoApp::resolve_target(origin) -> Target` (Direct(id) | Pick(candidates)
  | NeedsSplit) — función pura sobre la lista de paneles Files (testeable en
  core si la lógica vive en workspace; preferir `workspace.other_files_panes
  (origin)`).
- Overlay: en `ui()`, si `pending_pane_pick`, pintar sobre el rect de cada
  candidato (los rects se capturan al pintar cada file_panel en un
  `HashMap<PaneId, Rect>` por frame) un `rect_filled` semitransparente + el
  número centrado (FontId grande); leer `1..9`/Esc/clic del input.
- Toolbar: dos íconos nuevos (⇄ y ⧉→? elegir glifos; estilo Pack requiere 2
  `ActionIcon` nuevos con sus PNG placeholder en los 3 sets).
- i18n ES/EN: tooltips + status del selector («elige el panel destino: 1-9,
  Esc cancela»).

## Tests

`resolve_target` (1/2/3+ paneles, origen excluido), numeración por orden
visual (función pura sobre rects), swap intercambia `current_dir` y ambos
historiales reciben push. Overlay/teclas: manual en vivo.

## Fuera de alcance

Copiar/mover archivos «a panel N» (la infraestructura queda lista); recordar
el último destino elegido; drag&drop de tabs como destino.
