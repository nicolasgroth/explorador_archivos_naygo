# Ajustes post-0.3.0: menú abrir, metadata, tamaño de carpeta, split, instalador

> Spec de diseño. Fecha: 2026-07-02. Autor: Nicolás Groth / ISGroth.
> Estado: aprobado, pendiente de plan de implementación.

## Contexto

Lote de 8 ajustes pedidos por Nicolás tras validar 0.3.0 en la VM. Todos sobre la app
Windows. La mayoría reutiliza maquinaria que ya existe (navegación entre paneles,
cálculo de tamaño async, `open_default`, la crate `image`, el instalador con AppId
fijo). El único subsistema nuevo real es el de **metadata por tipo**; el resto es
cablear o pulir. Estado del código verificado antes de este diseño (informe de mapeo):

- Menú contextual propio en `crates/ui-slint/ui/context-menu.slint` (plano, SIN
  submenús hoy) + handlers en `crates/ui-slint/src/workspace_ctrl/context.rs`.
- Abrir carpeta en otro panel: `open_in_pane`, `resolve_target`, `target_candidates`,
  `other_files_panes` YA existen (`layout_panes.rs`, `workspace/mod.rs`). Abrir en
  Explorador: `open_default(dir)` YA existe (`platform/open.rs`).
- Cálculo de tamaño de carpeta async cancelable (F3): `spawn_dir_size` + `start_calc_size`
  + `pump_sizes` + `size_status` YA existen (`core/sizing.rs`, `workspace_ctrl/navigation.rs`).
- Split de panel: `add_pane_split()` fija `SplitDir::Horizontal`; `add_pane_split_dir(dir,
  first)` YA acepta dirección. `pane_rects(area)` da el `Rect` de cada panel.
- Metadata: NO existe nada más allá de tamaño/fechas. La crate `image` (ya en el árbol)
  lee dimensiones (`img.width()/height()`).
- Instalador (`installer/naygo.iss`): `AppId` fijo (línea 19), `CloseApplications=yes`
  (42), `RestartApplications=no` (43), `Flags: ignoreversion` (77+). Página de idioma de
  la app en `[Code]` (10 idiomas, líneas ~155-164).

## Alcance

| # | Ajuste | Complejidad |
|---|--------|-------------|
| A | Idiomas bilingües en el instalador (nombre en inglés entre paréntesis) | Trivial |
| B | Submenú "Abrir ▸" en el menú contextual (otro panel / nuevo / Explorador) + infra de submenús | Medio |
| C | Botón "Calcular" del tamaño en Propiedades de una carpeta | Bajo |
| D | Split del panel nuevo por el lado más largo | Bajo |
| E | Sistema de metadata por tipo (extensible) + proveedores imagen y exe-versión, en Propiedades y Preview | Medio |
| F | Instalador: confirmar/pulir detección + cierre + actualización (ya funciona) | Trivial |
| G | Nueva carpeta: Ctrl+Enter / doble-Enter = "Crear" | Bajo |
| H | Atajos rápidos "abrir en otro panel / nuevo" en carpetas (Shift+Enter, clic-medio) | Bajo |

Fuera de alcance: EXIF completo (cámara/GPS/fecha de captura), diferencial binario del
instalador, submenús para archivos (solo carpetas), bilingüe en el selector de idioma del
wizard de Inno.

---

## Sección 1 — Submenú "Abrir ▸" + atajos (B, H)

### Infraestructura de submenús

El `ContextMenu` de Slint es plano hoy. Se añade soporte de **un nivel de submenú**: una
entrada marcada como "tiene submenú" que, al hover/clic, despliega un panel secundario
adyacente con sus propias entradas. Se diseña como componente reutilizable (`SubMenu` o
un flag en la fila) para que futuros submenús sean baratos. El submenú se cierra al
elegir una entrada o al salir del menú; hereda tema y posición relativa a su fila.

### La entrada "Abrir ▸" (solo cuando el objetivo es una CARPETA)

Cuando el clic derecho cae sobre una carpeta, la primera entrada es un submenú **"Abrir
▸"** con:

- **Abrir** — navega el panel activo a la carpeta (igual que doble-clic). Reusa la
  navegación existente del panel activo.
- **Abrir en otro panel** — `resolve_target(origin, area)`: 1 otro panel Files → abre
  directo (`open_in_pane`); 2+ → muestra el selector 1..9 existente (el del drag entre
  paneles); 0 → crea un panel nuevo (split por lado largo, ver Sección 4).
- **Abrir en panel nuevo** — crea un panel nuevo con esa carpeta (split por lado largo).
- **Abrir en el Explorador de Windows** — `open_default(dir)` (ya existe).

Para **archivos** (no carpetas), NO se muestra el submenú "Abrir ▸": se conservan las
entradas actuales "Abrir" y "Abrir con…" tal cual. La distinción carpeta/archivo ya la
sabe el menú (`folder_mode` / el `kind` del target).

### Atajos rápidos (H)

Sobre la carpeta enfocada en un panel Files:
- **Shift+Enter** → "abrir en otro panel" (mismo `resolve_target`).
- **Clic-medio** → "abrir en panel nuevo".

Solo aplican a carpetas; sobre un archivo no hacen nada nuevo (o mantienen su
comportamiento actual si lo tuvieran). Van en `workspace_ctrl/input.rs` y el manejo de
clic del panel.

### Impacto

- `context-menu.slint`: componente/lógica de submenú + la entrada "Abrir ▸" y sus hijas.
- `types.slint`: `ContextMenuVm` gana los flags necesarios (p. ej. `target_is_folder`,
  y lo que el submenú requiera).
- `context.rs`: handlers `ctx_open_other_pane`, `ctx_open_new_pane`,
  `ctx_open_explorer` (este ya existe como `ctx_open_explorer`) — todos reusando
  `resolve_target`/`open_in_pane`/`add_pane_split`/`open_default`.
- `input.rs` + manejo de clic: Shift+Enter y clic-medio.

Riesgo: la infra de submenús es lo único nuevo de UI; la lógica de destino ya está
probada. El submenú debe respetar el patrón de foco de los modales de Slint del proyecto.

---

## Sección 2 — Sistema de metadata por tipo (E)

### Arquitectura (en `core`, módulo nuevo `metadata`)

- **Trait `MetadataProvider`**: `fn read(&self, path: &Path) -> Vec<MetadataField>` donde
  `MetadataField { label_key: &'static str, value: String }` (`label_key` es una clave
  i18n, p. ej. `"meta.dimensions"`). Puro y testeable. Un proveedor puede devolver vacío
  si no aplica o falla (tolerante — nunca panic; el filesystem es hostil).
- **Registro**: mapa extensión (lowercase) → proveedor. `metadata_for(path) -> Vec<
  MetadataField>` busca el proveedor por extensión y delega; sin proveedor → vacío.
  Añadir un tipo = registrar un proveedor. El registro vive en `core`; los proveedores
  que necesiten Windows se inyectan desde `platform` (ver abajo).

### Proveedores del primer entregable

1. **Imágenes** (png/jpg/jpeg/gif/bmp/webp/ico) — `core`, con la crate `image` ya
   presente: `MetadataField("meta.dimensions", "1920 × 1080")` y
   `MetadataField("meta.color_type", "RGBA")` (o equivalente). Lee solo la cabecera si es
   posible (barato). Cero deps nuevas. Tests con imágenes de fixture pequeñas.
2. **Ejecutables** (exe/dll) — `platform` (Win32 `GetFileVersionInfo`/`VerQueryValue`):
   `MetadataField("meta.file_version", "0.3.0.0")`, `MetadataField("meta.product_name",
   "…")` cuando existan. Se registra como proveedor vía un adaptador que el `core` acepta
   (p. ej. el registro admite proveedores inyectados en runtime, o `platform` expone una
   función que la UI conecta al registro al arrancar). Stub no-Windows que devuelve vacío.

Incluir un proveedor `core`-puro y uno `platform` valida que la arquitectura soporta
ambos casos (era el objetivo del diseño extensible).

### Ejecución

La lectura toca disco (decodificar cabecera / leer version info) → corre **bajo demanda
en un worker async**, NO en el listado ni en cada selección. Se dispara al mostrar
Propiedades o Preview de un archivo; muestra "Leyendo…" mientras llega; cancelable si el
usuario cambia de archivo. Reusa el patrón de workers (canal + waker) del proyecto.

### Dónde se muestra

- **Propiedades (Inspector)**: sección "Detalles" bajo los campos actuales, lista los
  `MetadataField` (etiqueta i18n + valor). Vacía si no hay proveedor para ese tipo.
- **Vista previa (Preview)**: los mismos campos junto al preview (dimensiones es natural
  ahí).

### Impacto

- `core/src/metadata/`: trait + registro + proveedor imagen + tests.
- `platform`: proveedor exe-versión (Win32) + stub no-Windows.
- `ui-slint`: worker de metadata (dispatch + drenaje en el tick), secciones en
  `inspector-panel.slint` y `preview-panel.slint`, claves i18n de las etiquetas en los 10
  idiomas.

YAGNI: EXIF, audio, video, PDF-props quedan como proveedores futuros (el diseño los
admite sin recargar).

---

## Sección 3 — Botón "Calcular tamaño" en Propiedades (C)

En el Inspector, cuando el objeto es una **carpeta** (tamaño vacío hoy), un enlace/botón
**"Calcular"**. Al pulsarlo: dispara `start_calc_size` sobre el objeto enfocado (la misma
maquinaria de F3), muestra "Calculando…" con parcial en vivo (`size_status`) y el total
al terminar. Cancelable (cambiar de objeto/cerrar cancela el token). Solo cablea el botón
del Inspector a lo existente. Impacto: `inspector-panel.slint` (botón + estado), handler
en `main.rs`. Sin lógica nueva de cálculo.

---

## Sección 4 — Split por el lado más largo (D)

Helper puro `pick_split_dir(rect: Rect) -> SplitDir`: `rect.w > rect.h` → `Horizontal`
(dos columnas, aprovecha el ancho); `rect.h > rect.w` → `Vertical` (dos filas); empate →
`Horizontal` (como hoy). *(Nota: en el modelo de layout de Naygo, `SplitDir::Horizontal`
= hijos lado a lado = corte vertical visual; se usa el enum del código, y el helper
devuelve el enum correcto para "dos columnas cuando el panel es ancho".)*

`add_pane_split()` consulta el `Rect` del panel activo (`pane_rects(area_of())`) y llama
`add_pane_split_dir(pick_split_dir(rect), false)`. Aplica al botón de nuevo panel y al
"Abrir en panel nuevo" del submenú (Sección 1). Impacto: `layout_panes.rs` + el helper
testeable. Tests: panel ancho → columnas; panel alto → filas; cuadrado → columnas.

---

## Sección 5 — Nueva carpeta: Ctrl+Enter / doble-Enter = Crear (G)

En el modal de nueva carpeta (`new-folder.slint`, un `TextEdit` multilínea):
- **Ctrl+Enter** → equivale a "Crear" (si `vm.can_create`).
- **Enter en línea vacía teniendo contenido** (doble-Enter) → también "Crear".

Ambos ignoran líneas vacías (ya lo hace `core::ops::parse_new_folders`). Detalle técnico:
el `TextEdit` de std-widgets consume Enter como salto de línea; capturar Ctrl+Enter/
doble-Enter requiere un `FocusScope` que intercepte la combinación antes del editor, o el
mecanismo de tecla que exponga esta versión de Slint. **A verificar en implementación**;
fallback: `FocusScope` envolvente. Es el único punto del lote con incertidumbre de API.
Impacto: `new-folder.slint` + handler.

---

## Sección 6 — Idiomas bilingües en el instalador (A)

Página de idioma de la app (`[Code]` del `.iss`): añadir el nombre en inglés entre
paréntesis salvo donde es obvio:
- `English`
- `Español (Spanish)`
- `Deutsch (German)`
- `Français (French)`
- `Italiano (Italian)`
- `Português (Portuguese)`
- `日本語 (Japanese)`
- `हिन्दी (Hindi)`
- `한국어 (Korean)`
- `中文 (Chinese)`

El selector de idioma del **wizard** de Inno (primer diálogo) usa los nombres nativos de
los `.isl` y no es trivialmente editable → se deja como está (decisión de Nicolás). Solo
cambian los textos del `[Code]`; sin lógica.

---

## Sección 7 — Instalador: detección/cierre/actualización (F)

Ya funciona hoy (verificado): `AppId` fijo → detecta versión previa y actualiza en el
mismo directorio; `CloseApplications=yes` → ofrece cerrar Naygo si está corriendo;
`ignoreversion` → reemplaza el .exe. El diferencial binario no aplica a un .exe único de
~26 MB (sería sobre-ingeniería). Pulido: verificar que el mensaje de "cerrar la
aplicación" salga traducido y que la versión se muestre en el wizard. Sin cambios
estructurales.

---

## Tabla de decisiones

| # | Decisión |
|---|----------|
| A | Bilingüe en la página de idioma de la app; wizard de Inno sin tocar |
| B | Submenú "Abrir ▸" solo en carpetas (Abrir/otro panel/nuevo/Explorador); infra de submenús nueva |
| C | Botón "Calcular" manual con progreso, reusa F3 |
| D | `pick_split_dir`: ancho→columnas, alto→filas, empate→columnas; aplica a nuevo panel y a "abrir en panel nuevo" |
| E | `metadata` por tipo (trait+registro); proveedores imagen (core) + exe-versión (platform); async bajo demanda; en Propiedades y Preview; EXIF diferido |
| F | Instalador ya detecta/cierra/actualiza; solo pulido de textos |
| G | Ctrl+Enter y doble-Enter crean en nueva carpeta (ignoran líneas vacías) |
| H | Shift+Enter = otro panel; clic-medio = panel nuevo (solo carpetas) |

## Orden de construcción

1. **A** (instalador bilingüe — trivial, aislado).
2. **D** (`pick_split_dir` + cableado — bajo, base para "abrir en panel nuevo").
3. **C** (botón calcular — bajo, aislado).
4. **G** (Ctrl+Enter nueva carpeta — bajo, aislado).
5. **B + H** (submenú "Abrir ▸" + atajos — usa D para el panel nuevo).
6. **E** (sistema de metadata — el más grande; core → platform → UI).
7. **F** (pulido instalador).
8. Integración: suite completa + regenerar dist.

## Principios respetados

- Reutilización: la mayoría cablea funciones ya probadas (resolve_target, sizing,
  open_default, add_pane_split_dir).
- Bajo consumo: metadata y tamaño de carpeta son bajo demanda, en worker, cancelables.
- Capas: metadata en core (trait+registro+imagen), Win32 (exe-versión) en platform, UI en
  ui-slint. El registro admite proveedores de plataforma sin que core dependa de Windows.
- i18n desde el día uno: submenú, etiquetas de metadata y textos nuevos en los 10 idiomas.
- Sin telemetría. Regenerar dist tras los cambios.
