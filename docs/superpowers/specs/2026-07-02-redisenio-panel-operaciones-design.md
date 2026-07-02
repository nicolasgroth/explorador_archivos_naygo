# Rediseño visual del panel de operaciones — Diseño

> Autor: Nicolás Groth <ngroth@gmail.com> · ISGroth · 2026 · MIT
> Fecha: 2026-07-02

## Contexto y problema

El panel de operaciones de archivo (copiar / mover / borrar / comprimir / extraer),
implementado en `crates/ui-slint/ui/ops-panel.slint`, es **funcionalmente correcto** pero
**visualmente poco pulido**. Nicolás lo describe como "poco profesional". El diagnóstico sobre
el código actual:

- **Botones planos con borde de caja** (`OpButton`: `border-width: 1px`, alto fijo 24px, texto
  11px). Los cuatro controles (Pausar/Saltar/Cancelar/Cancelar-todo) tienen el mismo peso visual;
  no hay jerarquía. El "Cancelar todo" rojo del header compite con el título.
- **Volcado de datos**: los 6 datos de una op en curso (transferido, velocidad, pico, transcurrido,
  restante, %) se apilan como líneas `etiqueta: valor` todas a 11px. El ojo no encuentra qué es
  importante.
- **Espaciado apretado** (`padding: 9px`, `spacing: 5px`): todo pegado, sin aire.
- **Cero animación**: el % y la barra saltan bruscamente al cambiar.

**Objetivo**: que el panel se vea y se sienta como un aplicativo de última generación, validado
por diseñadores, **sin sobrecargar el sistema** (Naygo prioriza bajo consumo y corre con render
por software, sin GPU). No se cambia NINGUNA funcionalidad ni callback; es puramente presentación
(más un flag de config nuevo).

## Principios que se respetan

- **Bajo consumo primero**: las animaciones baratas (interpolar un valor que cambió) son
  declarativas (`animate` de Slint) y solo redibujan durante la transición → 0% CPU en reposo. Las
  animaciones **continuas** (que redibujan ~30-60 veces/s mientras hay algo en curso) quedan
  **detrás de un toggle**, apagado por defecto.
- **Íconos con `Path`, no glifos de fuente**: el render por software puede no pintar glifos; ya es
  convención del proyecto (ver los íconos actuales de `ops-panel.slint`).
- **Colores del `Theme` actual**: no se inventan colores nuevos; todo sale de `theme.slint`
  (`Theme.accent`, `Theme.row-bg`, `Theme.panel-bg`, `Theme.text`, `Theme.text-dim`,
  `Theme.border`, `Theme.selection-bg`, `Theme.error`, `Theme.highlight`). Si hace falta un matiz
  intermedio (p. ej. el fondo de una mini-tarjeta más oscuro que `row-bg`), se deriva de un token
  existente, no se hardcodea un hex.
- **i18n**: todo texto nuevo por clave, en los 10 idiomas.

## Lenguaje visual (común a las 4 zonas)

1. **Chip de ícono de tipo** (30×30px, `border-radius: 8px`, fondo derivado del acento tenue):
   copiar, mover, borrar, comprimir, extraer. Un `Path` por tipo. La op ya sabe su tipo (se
   deriva de `op.label` o se añade un campo al VM; ver más abajo). Da identidad inmediata a cada
   tarjeta/fila.
2. **Botones sin caja**:
   - Acción principal (Pausar/Reanudar): **chip suave** con fondo `Theme.selection-bg` al reposo
     tenue / hover más marcado, `border-radius: 8px`, sin borde.
   - Acción secundaria (Saltar): **texto-botón** (sin fondo salvo hover), con su ícono.
   - Cancelar (destructivo): **ícono X solo**, `Theme.error`, con `tooltip` "Cancelar". No ocupa
     ancho con texto rojo.
3. **Aire**: `padding: 14px` en tarjetas, `border-radius: 12px` en tarjetas, chips `8px`,
   `spacing` interno 8–12px.
4. **Barra de progreso**: estilo píldora (`border-radius` alto), sin borde de caja, track
   `Theme.panel-bg`, relleno `Theme.accent` (o `Theme.highlight` si pausada).

## Zona EN CURSO (`OpCard`) — el foco principal

Estructura de arriba a abajo:

1. **Cabecera** (`HorizontalLayout`, align center):
   - Chip de ícono de tipo (30px).
   - Columna: verbo en curso ("Copiando" / "Moviendo" / …) a 13px `Theme.text` weight 500 +
     nombre de archivo actual a 12px `Theme.text-dim` con `overflow: elide`.
   - Espaciador.
   - **% grande**: 20px weight 500 `Theme.text`, con el signo `%` a 13px `Theme.text-dim`. Es el
     dato protagonista.
2. **Barra de progreso** (píldora).
3. **Grilla de datos 3+2** (los 6 datos, cada uno en una mini-tarjeta):
   - Fila de 3: **Transferido** · **Velocidad** · **Pico**.
   - Fila de 2: **Transcurrido** · **Restante**.
   - Cada mini-tarjeta: fondo derivado (más oscuro que la tarjeta), `border-radius: 8px`,
     `padding: 8px 10px`. **Etiqueta arriba-izquierda** (11px `Theme.text-dim`), **valor
     abajo-derecha** (`text-align: right`, 13px weight 500 `Theme.text`; la unidad — "MB",
     "MB/s", "/196.8 MB" — en 11px `Theme.text-dim` inline).
4. **Fila de controles** (`HorizontalLayout`): Pausar/Reanudar (chip) · Saltar (texto-botón) ·
   espaciador · X cancelar (ícono).

## Zona CALCULANDO (`PlanningCard`)

Mismo lenguaje: chip de ícono de tipo + verbo + "Calculando…" con el `ScanDot` (punto de acento
estático — **sin spinner**, bajo consumo) + contador en vivo (`op.status`) + X cancelar. Mismo
padding/radio que `OpCard`.

## Zona EN COLA (`QueuedRow`)

Fila compacta: chip de ícono pequeño (o el ícono sin chip) + etiqueta + "— en espera"
(`Theme.text-dim`) + espaciador + X cancelar. Sin borde de caja; separación por espaciado.
`border-radius: 8px`.

## Zona HISTORIAL (`HistoryRow`)

Fila: **ícono de resultado** (✓ para "hecho", ⚠ para "con fallos" — `Path`, color
`Theme.accent`/`Theme.error`) + etiqueta + status ya formateado + segunda línea con nombres inline
(1–2) o enlace "Ver N archivos" (3+). Se conserva la lógica de alto adaptable actual.

## Header del panel (`OpsPanel`)

- Título "Operaciones" (13px weight 500 `Theme.text`).
- **Chip-contador** redondeado ("1 activa" / resumen) con fondo `Theme.selection-bg` tenue,
  `Theme.text-dim`. Sustituye el texto suelto "N en curso · M en cola" y el resumen largo; el
  detalle puede ir en `tooltip` del chip.
- Espaciador.
- **X "Cancelar todo"** (ícono, `Theme.error`, tooltip). Solo visible con algo activo. Conserva
  el flujo actual: pide confirmación vía el modal existente antes de cancelar.

## Animaciones

Dos categorías, con trato distinto:

### Baratas — SIEMPRE activas (0% CPU en reposo)

Declarativas con `animate` de Slint; solo redibujan durante la transición de un valor que cambió:
- La **barra de progreso** (`width` del relleno) y el **% numérico** se interpolan suave
  (~200ms, `ease-out`) cuando cambia el valor, en vez de saltar. (Para el número: animar una
  propiedad `float` interna y mostrar `round()` de ella.)
- Las **tarjetas/filas** que entran o salen de una zona hacen un **fade corto** (~150ms sobre
  `opacity`).

Estas no dependen de ningún toggle: son el pulido base. En reposo (sin cambios) no hay ningún
redibujo, así que no cuestan CPU.

### Costosas / continuas — detrás del toggle "Activar animaciones"

- **Brillo animado de la barra de progreso**: un reflejo/gradiente tenue que recorre la barra
  mientras una op está en curso. Es una animación **continua** (redibuja constantemente mientras
  hay algo activo) → cuesta CPU.
- Se controla con un **flag de config nuevo y GLOBAL** (`animations_enabled`), pensado para
  gobernar TODA futura animación costosa de cualquier panel (no solo esta barra). Apagado por
  defecto.

## Config: nuevo flag `animations_enabled`

- **Core** (`crates/core/src/config/mod.rs`): campo `pub animations_enabled: bool` en `Settings`,
  con `#[serde(default = "default_animations_enabled")]` y `fn default_animations_enabled() -> bool
  { false }`. Inicializarlo en los dos sitios de construcción de `Settings` que hoy setean
  `auto_highlight_code`/`size_no_subdirs` (el default de fábrica y el de los tests). Añadir el test
  de round-trip/serde si el módulo tiene uno análogo por campo.
- **UI** (`crates/ui-slint/ui/config-window.slint`): un toggle "Activar animaciones" en una pestaña
  coherente (Apariencia, o donde vivan los ajustes visuales; si no hay una clara, Avanzado). Con el
  ícono `?`/tooltip estándar del proyecto: **"Activa animaciones adicionales (como el brillo de la
  barra de progreso en el panel de operaciones). Consumen un poco más de CPU."**
- **Cableado**: el `OpsPanel` recibe una `in property <bool> animations-enabled` (poblada desde la
  config, como los otros ajustes que ya llegan al panel), y la barra de progreso enciende el brillo
  solo si es `true`. El nombre del flag es genérico a propósito para reutilizarlo.

## Alcance de archivos

- `crates/ui-slint/ui/ops-panel.slint` — el grueso: reescribir `OpButton`, `OpCard`,
  `PlanningCard`, `QueuedRow`, `HistoryRow`, `ProgressBar`, el header de `OpsPanel`; añadir chips de
  ícono de tipo (nuevos componentes `Path`), la grilla de mini-tarjetas, las animaciones, y la
  `in property animations-enabled`.
- `crates/core/src/config/mod.rs` — flag `animations_enabled` (+ default + inits + test).
- `crates/ui-slint/ui/config-window.slint` — toggle + tooltip.
- `crates/ui-slint/ui/i18n.slint` + `crates/ui-slint/src/i18n_keys.rs` — properties de `Tr` para el
  texto nuevo (verbos "Copiando/Moviendo/…" si se separan de `label`, etiquetas de mini-tarjetas si
  cambian, el label y tooltip del toggle).
- `crates/core/src/i18n/*.json` (10 idiomas) — claves nuevas.
- Posible: `crates/ui-slint/ui/types.slint` (`OpRowVm`) — si se añade un campo `op-kind`/`verb` para
  el ícono de tipo y el verbo; y el poblado en `crates/ui-slint/src/` donde se arma `OpRowVm`.
- `crates/ui-slint/src/main.rs` — pasar `animations-enabled` al `OpsPanel` (una línea, como los
  otros ajustes).

**Nada de lógica de negocio cambia**: los callbacks `pause/resume/skip/cancel/cancel-all/show-files`
y los modelos `running-rows/queued-rows/history-rows/planning-rows` se conservan idénticos.

## Cómo se determina el ícono/verbo de tipo

`OpRowVm` hoy trae `label` (ya traducido, p. ej. "Copiar"). Para el ícono y el verbo en gerundio se
elige UNA de estas vías (a decidir en el plan, preferir la más limpia):
- **(a)** Añadir a `OpRowVm` un `op-kind: int` (0 copiar, 1 mover, 2 borrar, 3 comprimir,
  4 extraer) que Rust ya conoce al construir la fila; el `.slint` mapea kind→ícono y kind→verbo
  (vía `Tr`). Es la vía limpia y i18n-correcta.
- **(b)** Derivar el ícono comparando `label` contra los `Tr` conocidos en el `.slint` (frágil,
  evitar).

Se recomienda **(a)**.

## Testing

- El código es UI declarativa (Slint); la mayor parte se valida visualmente en la VM (Nicolás).
- **Core**: el flag `animations_enabled` se cubre con un test de default (=false) y, si existe el
  patrón, uno de round-trip serde (igual que `auto_highlight_code`).
- **i18n**: el test de paridad `todos_los_idiomas_tienen_las_claves_de_es` cubre que las claves
  nuevas estén en los 10 idiomas.
- **Regresión**: `cargo test --workspace` y `cargo clippy --workspace --all-targets -- -D warnings`
  deben seguir verdes (886 tests actuales + los nuevos).
- **No romper** los tests que ya ejercen el panel de ops (conflicto, cancelar-todo, detalle de
  archivos): como no cambia ningún callback ni modelo, deben pasar sin tocar.

## Fuera de alcance (YAGNI)

- No se tocan los modales de operaciones (`op-dialogs.slint`, `op-file-list-modal.slint`) salvo que
  compartan un componente reescrito.
- No se añaden tipos de animación más allá del brillo de la barra (el flag queda listo para futuros,
  pero no se implementan ahora).
- No se cambia el comportamiento de scroll, ni el orden de zonas, ni la lógica de conteo.
