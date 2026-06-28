# Íconos: galería de preview + toolbar en caliente + enlace Apariencia↔Íconos — diseño

> Naygo — explorador de archivos. Copyright (c) 2026 Nicolás Groth / ISGroth. MIT.
> Continuación de la feature de íconos personalizables (rama `feat/iconos-personalizables`).
> Hace que el toolbar respete el set de íconos elegido (hoy usa vectores fijos),
> reemplaza el combo de set por una galería de tarjetas con preview (estilo temas),
> y enlaza la selección en Apariencia con la pestaña Íconos para personalizar.

## Motivación

Tras la primera entrega (5 sets de fábrica + personalización por objeto + .naygoset),
Nicolás probó en la VM y reportó:

1. **El toolbar no cambia al cambiar de set.** Causa verificada: ~18 botones del
   toolbar se dibujan con `Path` vectorial **hardcodeado** (`draw-back`, `draw-settings`,
   …, en `app-window.slint`) que ignora el set activo. Solo paneles/árbol/discos
   consumen el set.
2. **Quiere un preview del set al seleccionarlo**, parecido a las tarjetas de tema
   (que muestran una combinación de colores).
3. **El combo "Set de íconos" en Apariencia y la pestaña Íconos están desconectados**
   (duplicación que confunde). Quiere que el "editor" (pestaña Íconos) esté enlazado a
   Apariencia y preseleccione el set actual.

## Objetivos

1. **Toolbar 100% del set activo**: reemplazar los `Path` fijos por los íconos del set
   (teñidos al tema), de modo que cambiar set/tema se note en el toolbar, en caliente.
2. **Galería de tarjetas de sets** con preview de ~5 íconos de muestra, estilo la
   galería de temas, reemplazando el combo.
3. **Enlace Apariencia↔Íconos**: botón "Personalizar" en cada tarjeta que activa ese set
   y salta a la pestaña Íconos con el set preseleccionado.

### No-objetivos (YAGNI)

- No se rediseñan los íconos de archivos en los paneles (ya funcionan).
- No se cambia el formato `.naygoset` ni el modelo de overrides.
- No se agrega un set "vectorial" como opción.

## Decisiones tomadas en brainstorming

- Toolbar usa el set elegido (no se mantienen vectores fijos como alternativa).
- Alcance **(a)**: toolbar 100% del set — se agregan las claves de acción que faltan
  para cubrir TODOS los botones del toolbar, y se regeneran los 5 sets.
- Tarjeta de set = ~5 íconos de muestra + nombre + botón Personalizar (no grid de 33,
  no combo+preview).
- Galería en Apariencia + botón Personalizar que salta a Íconos.
- "Personalizar" **activa** el set (caliente) Y salta a Íconos preseleccionado.

## Claves de acción nuevas

Cruce de los 18 `draw-*` del toolbar contra los 20 `ActionIcon` existentes: 12 ya
tienen clave (back, forward, up, refresh, settings, terminal, tabs, layouts, panel,
swap→SwapPanes, clone→ClonePath, new→NewFolder, plus→AddPane). Quedan **6 huérfanos**
sin `ActionIcon` (hoy solo Path):

| draw-* | Nueva variante `ActionIcon` | `file_name` | Concepto |
|--------|------------------------------|-------------|----------|
| `home`    | `Home`        | `action_home`     | Ir a carpeta inicio |
| `search`  | `Search`      | `action_search`   | Búsqueda (lupa) |
| `eye`     | `ShowHidden`  | `action_show_hidden` | Mostrar ocultos (ojo) |
| `history` | `History`     | `action_history`  | Historial (reloj) |
| `star`    | `Favorites`   | `action_favorites`| Favoritos (estrella) |
| `split`   | `Split`       | `action_split`    | Dividir paneles |

`ActionIcon` pasa de 20 a 26 variantes; el set de 33 a **39 claves** (39 PNG por set,
195 en total).

## Arquitectura

Respeta las 3 capas. La lógica de claves vive en `core`; la galería y el toolbar en `ui`.

### Capa `core`

- **`icon_kind.rs`**: agregar las 6 variantes a `enum ActionIcon`, a `ActionIcon::all()`,
  y a `ActionIcon::file_name()` (con los `action_*` de la tabla). Esto propaga
  automáticamente a `icons::all_keys()`, `file_name`, y la conversión key↔string
  (`icon_source.rs`), que ya derivan de `ActionIcon`.
- **`icon_source.rs`**: `key_from_string` debe reconocer los 6 nuevos `action_*`
  (ya lo hace vía `ActionIcon::all()` — verificar que el match directo no los excluya).
- **`bin/gen_icons.rs`**: agregar las 6 claves al mapeo de CADA set (lucide, tabler,
  material, flat-color, mono), verificando el nombre SVG real en cada zip (mismo proceso
  que la entrega anterior: `unzip -l` + sustituto documentado si falta). mono reusa
  lucide. Sugerencias por verificar: home→`house`/`home`, search→`search`, eye→`eye`,
  history→`history`/`clock`, star→`star`, split→`columns`/`layout-columns`.

### Generación

Correr `cargo run -p naygo-core --bin gen_icons --features gen-icons` para regenerar
los 5 sets con las 39 claves. Commitear los 30 PNG nuevos (6 × 5 sets).

> GOTCHA de orden (igual que antes): el generador debe correrse y los PNG commitearse
> ANTES de compilar `icons/mod.rs` (que los embebe con `include_bytes!` vía `set_table!`).
> El `set_table!` macro en `icons/mod.rs` lista las claves explícitamente — hay que
> agregar las 6 nuevas entradas a la macro.

### Capa `ui-slint`

**Toolbar (app-window.slint):** la infraestructura YA existe — el componente de botón
tiene `in property <image> icon` (línea ~85) y ya pinta `if root.icon.width > 0 &&
!root.draw-terminal: Image { source: root.icon }` (línea ~149). El problema es que
cuando un `draw-*` está en `true`, el bloque `Path` correspondiente pisa al `Image`
(y el `!root.draw-*` lo excluye). **El cambio:** quitar los `draw-*` (las props y sus
bloques `Path`) y dejar que cada botón pinte su `icon` (el `Image` del set). El toolbar
construye sus botones pasando una `image` a cada uno; ese valor debe venir del
`IconCache`. Rust ya tiene `refresh_toolbar_icons` (hoy no-op porque los íconos eran
Path): se cablea para que, por cada botón del toolbar, tome `c.icons.get(key)` (la
`IconKey` correspondiente — incluidas las 6 nuevas) y lo pase a la `image` de ese botón
vía las props/el modelo que alimenta la fila de botones. Verificar en `app-window.slint`
cómo se instancian los botones (propiedades sueltas `ic-*` en `AppWindow`, o un modelo)
y cablear en consecuencia; el `icon-tint`/teñido lo aporta el `IconCache` (PNG ya teñido),
así que el botón ya no necesita teñir por su cuenta para esos íconos.

**Galería de tarjetas (nueva, en Apariencia):**
- VM nuevo en `types.slint`: `IconSetCardVm { id: string, label: string, samples: [image],
  selected: bool }` (paralelo a `ThemeCardVm`). `samples` = ~5 imágenes de muestra
  (folder, file_image, action_copy, action_settings, action_back) del set, teñidas.
- `SettingsVm` gana `icon-set-cards: [IconSetCardVm]`.
- En `config-window.slint`, sección Apariencia (cat 3): reemplazar el `Field` del combo
  `Tr.cfg-icon-set` por una galería `for card in vm.icon-set-cards` con el mismo markup
  que la galería de temas (tarjeta con muestras + nombre + botón Personalizar + borde de
  acento si `selected`). Click en cuerpo → `set-icon-set(card.id)`.
- La misma galería reemplaza el combo de "Set base" en la pestaña Íconos (cat 9).
- Rust llena `icon-set-cards` en `build_settings_vm`: por cada set del catálogo, renderiza
  las ~5 muestras con `render_for_set` (ya existe) y arma el VM.

**Enlace Apariencia→Íconos:**
- Callback nuevo `personalize-icon-set(string)` en `config-window.slint`.
- El botón "Personalizar" de cada tarjeta lo invoca con el `card.id`.
- Handler Rust: activa el set (`c.config.set_icon_set(id)` + re-aplica cache + repaint)
  y setea la categoría activa a Íconos: `cfg_win.set_cat(9)` (el `cat` es
  `property <int>`, seteable desde Rust con el setter generado).

### Limpieza de duplicación

Con la galería, se elimina el combo `Tr.cfg-icon-set` de Apariencia y el combo de set
de la pestaña Íconos; ambos puntos usan la galería. La pestaña Íconos queda: galería
(set base) arriba + grilla de objetos + botones import/export/reset debajo.

### i18n

- 6 claves `icons.obj.action_{home,search,show_hidden,history,favorites,split}` en es/en
  (labels de la grilla), manteniendo parity (hay test `es_en_tienen_las_mismas_claves`).
- Clave para el botón de tarjeta si hace falta (reusar `settings.icons.section_custom` /
  un `settings.icons.personalize`). Texto neutral, sin voseo.

## Componentes y responsabilidades

| Unidad | Responsabilidad | Depende de |
|--------|-----------------|------------|
| `core::icon_kind` | +6 ActionIcon + file_name | — |
| `core::bin::gen_icons` | mapeo SVG de las 6 claves × 5 sets | zips |
| `core::icons` (set_table!) | embeber las 39 claves | assets |
| `ui::app-window.slint` | toolbar con Image del set (sin Path) | IconCache |
| `ui::main.rs refresh_toolbar_icons` | cablear ic-* desde IconCache | core::icons |
| `ui::types.slint` | IconSetCardVm + icon-set-cards | — |
| `ui::config-window.slint` | galería en Apariencia + Íconos; botón Personalizar | VM |
| `ui::main.rs build_settings_vm` | llenar icon-set-cards (render muestras) | render_for_set |
| `ui::main.rs` | handler personalize-icon-set (activar + saltar a cat 9) | config_ctrl |

## Errores

- Conforme a las reglas del proyecto: render de muestras tolerante (si un PNG falta,
  `render_for_set` cae al fallback embebido, nunca panic).
- El cambio de set/tema invalida el `IconCache` (ya corregido: `set_overrides`/`set_tint`
  hacen `map.clear()`); el toolbar se repinta vía `refresh_toolbar_icons`.

## Testing

- `core`: los tests existentes (`cada_set_de_fabrica_cubre_las_39_claves` — antes 33;
  actualizar el número; round-trip key↔string sobre `all_keys()`) cubren las 6 claves
  nuevas automáticamente. Agregar assert de que `ActionIcon::all()` tiene 26.
- Generador: el test de cobertura `*_cubre_todas_las_claves` (uno por set) ahora exige
  las 39 — actualizar la lista `ALL_KEYS` del generador.
- UI (galería, toolbar, enlace): verificación por compilación `--bins` + prueba visual
  en la VM (cambiar set en Apariencia → toolbar cambia; Personalizar → salta a Íconos).

## Trade-offs decididos

- **Toolbar PNG vs vector**: se elige PNG del set. Los íconos a 16px pierden un pelín de
  nitidez vs el vector, pero ganan coherencia (cambian con el set/tema) — que es el pedido.
- **Alcance (a) vs (b)**: (a) toolbar 100% del set, regenerando con 6 claves nuevas.
  Más trabajo que (b), pero deja el toolbar totalmente coherente con el set elegido.
- **Galería reemplaza combo** (no coexisten): elimina la duplicación que confunde hoy.

## Fuera de alcance / fases futuras

- Las otras 3 mejoras pedidas en la misma conversación (expulsar USB con panel abierto,
  auditoría i18n + idiomas nuevos, preview de ZIP) van en specs/ramas separadas.
- Comprimir/descomprimir de verdad: fase mayor futura.
