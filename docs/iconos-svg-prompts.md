# Prompts para generar los íconos de Naygo (SVG)

> Pasa estos prompts a una IA generadora de íconos (o a un diseñador). Genera **un SVG por
> ícono**. Cuando los tengas, guárdalos y avísale al asistente: él los integra (Slint carga SVG
> directo con `@image-url`, así escalan sin pérdida y reemplazan los íconos dibujados a mano).

## Estilo (pon este encabezado al inicio de CADA pedido)

> Generate a single flat-style line icon as clean SVG, **24×24 viewBox**, **2px stroke**, rounded
> line caps and joins, **stroke only** (no fill) using `currentColor` so it can be recolored,
> centered with ~2px padding, minimal and modern (Lucide / Tabler style). Output **only** the SVG
> code, nothing else. The icon represents:

## Íconos (uno por archivo)

Completa el encabezado de arriba con cada descripción. El **nombre de archivo** es el que espera
el código (carpeta `assets/icons/flat/`, formato final PNG 16×16 + 32×32 o SVG):

| Archivo destino        | Descripción a agregar al prompt |
|------------------------|---------------------------------|
| `action_up`            | "an up arrow above a folder — go to parent directory" |
| `action_add_pane`      | "a rounded square split into two vertical halves with a small plus sign — add/split panel" |
| `action_panel`         | "two stacked rectangular panels with a small downward chevron — add a special panel" |
| `action_swap_panes`    | "two horizontal arrows curving in opposite directions between two panels — swap panes" |
| `action_clone_path`    | "two overlapping folders with a small arrow — clone folder to another panel" |
| `action_tabs`          | "two browser-style tabs on top of a panel — stack as tab" |
| `action_layouts`       | "a 2x2 grid of rectangles of different sizes — panel layout templates" |
| `action_new_folder`    | "a folder with a plus sign — create new folder" |
| `action_terminal`      | "a console window outline with a '>' prompt and a cursor underscore" |
| `action_settings`      | "a gear / cog wheel" |
| `action_eject`         | "an eject symbol — a triangle pointing up over a horizontal bar" |
| `action_refresh`       | "a circular arrow — refresh / reload" |
| `action_back`          | "a left-pointing arrow — go back" |
| `action_forward`       | "a right-pointing arrow — go forward" |

## Opcional: variante "rellena" del activo

Si quieres que el botón seleccionado se vea relleno, pide además una variante con `fill` del mismo
ícono. Igual con `currentColor` alcanza para recolorearlos según el tema activo.

## Notas de integración

- Mantén el `viewBox="0 0 24 24"` y `currentColor` — así el asistente los tiñe con el color del
  tema sin editar el SVG.
- Evita `<text>` o fuentes embebidas (no se rasterizan bien); todo con `<path>`/`<line>`/`<rect>`.
- Si el generador da `width`/`height` fijos, no importa: lo que vale es el `viewBox` y los paths.
