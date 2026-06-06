# Naygo — Fase 2B: Íconos + entrada ".." (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-05. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

La **Fase 2A** entregó el layout dinámico: paneles independientes componibles,
navegación atrás/adelante por panel, plantillas, barra de íconos con posición
configurable, persistencia. Pero el panel de archivos sigue mostrando **glifos de
texto provisionales** (`[D]` para carpeta, `[?]` para "otro", espacios para
archivo) en vez de íconos reales.

La **Fase 2B** (este documento) reemplaza esos glifos por **íconos reales** y agrega
la **fila virtual `..`** estilo Total Commander. Es la segunda de las tres
sub-fases visuales:

- 2A — Layout dinámico (completa).
- **2B — Íconos + entrada ".." (ESTE documento).**
- 2C — i18n + temas + color sets (después).

**Premisa rectora (de Nicolás):** la app es de **respuesta rápida y fluida**. En
2B esto es crítico y guía la decisión central: **los íconos se resuelven y suben a
GPU una sola vez** (al arrancar o al cambiar de set); el listado solo dibuja
texturas ya cacheadas. **Cero decodificación de imágenes por frame o por archivo.**
El costo es por-set-cargado, no por-archivo-listado.

### Qué entra en 2B

- **`core::icon_kind`** (lógica pura, testeable): mapea un `Entry` a una **clave de
  ícono semántica** (`IconKey`), no a un archivo de imagen. La categoría de archivo
  sale de la extensión vía un `HashMap` O(1).
- **`ui::icons`** (`IconProvider`): carga el **set activo** de íconos desde assets
  embebidos, decodifica cada imagen una vez, la registra como textura egui, y la
  cachea por `IconKey`. Pintar = dibujar la textura cacheada.
- **Tres sets de íconos embebidos y seleccionables** por el usuario: **Flat Color
  Icons** (multicolor, default), **Fluent Emoji** (Microsoft, look Windows 11),
  **Monocromo** (Lucide/Tabler, temable). Cambiar el set recarga el provider una
  vez; el listado vuela igual con cualquiera.
- **Íconos en el file panel**: la columna Nombre pasa a `[ícono] nombre`. Se
  eliminan los glifos `[D]`/`[?]`.
- **Íconos de unidad de disco** en el árbol/breadcrumb (genéricos por tipo de
  unidad en 2B).
- **Fila virtual `..`** (estilo minimalista, `📁 ..` arriba del todo): UI pura, no
  una `Entry` real; sube al directorio padre al activarla (Enter/doble clic);
  **opcional** (default **on**), toggle en Configuración.
- **`Settings`**: `icon_set: IconSet` (Flat/Fluent/Mono) + `show_parent_entry: bool`,
  persistidos en `settings.json`, con toggles en el menú ⚙.
- **Hueco arquitectónico** para íconos del Shell de Windows (`platform`): una
  interfaz que el `IconProvider` consultará en el futuro, sin implementarla en 2B.

### Qué NO entra en 2B

Íconos reales del Shell de Windows (`platform::shell` — hueco previsto, no
implementado); detección fina del tipo de unidad real (en 2B, ícono genérico de
disco); miniaturas de imágenes (fase aparte); animaciones de íconos; i18n (el texto
sigue hardcoded en español — 2C); temas/color sets completos (2C, aunque el set
monocromo ya se recolorea con el color base disponible); persistencia del reacomodo
manual del dock (deuda conocida de 2A, sigue pendiente).

---

## 2. Arquitectura

Idea rectora intacta: **`core` decide QUÉ ícono (puro, testeable, portable); `ui`
decide CÓMO se ve (GPU, set activo); `platform` queda con el hueco para el Shell.**
La velocidad del listado no depende de nada de esto en runtime — solo dibuja
texturas ya cargadas.

### Capa `core` — módulo nuevo `icon_kind` (lógica pura)

- **`IconKey`**: la clave semántica de un ícono. Variantes:
  - `Folder` — carpeta normal.
  - `ParentDir` — la fila `..`.
  - `File(FileCategory)` — archivo de una categoría.
  - `Drive(DriveKind)` — unidad de disco.
  - `Unknown` — fallback genérico.
- **`FileCategory`**: categoría semántica de un archivo, derivada de su extensión:
  `Image`, `Video`, `Audio`, `Document`, `Code`, `Archive`, `Executable`,
  `Model3D`, `Font`, `Generic`. (Lista acotada y ampliable; `Generic` cubre lo no
  clasificado.)
- **`DriveKind`**: `Fixed`, `Removable`, `Network`, `Optical`, `Unknown`. (En 2B
  solo se usa `Unknown`/`Fixed` genérico; la detección real es de `platform`.)
- **`category_for_extension(ext: &str) -> FileCategory`**: lookup **O(1)** vía un
  `HashMap<&'static str, FileCategory>` construido una vez (lazy/`OnceLock`).
  Case-insensitive (normaliza a minúsculas). Extensión desconocida → `Generic`.
- **`icon_key_for(entry: &Entry) -> IconKey`**: carpeta → `Folder`; archivo →
  `File(category_for_extension(ext))`; `Other` → `Unknown` (o `File(Generic)`,
  decidir en el plan — `Unknown` es lo correcto semánticamente).
- **Pureza:** no toca disco, no conoce egui ni texturas. Testeable al 100%:
  "dame un `Entry`/una extensión, te digo la clave".

### Capa `ui` — módulo nuevo `icons` (`IconProvider`)

- **`IconSet`** (enum, también en `config`): `Flat`, `Fluent`, `Mono`. Serializable.
- **`IconProvider`**: dueño de las texturas del set activo.
  - En la construcción (o al cambiar de set): para cada `IconKey` relevante, toma
    los **bytes embebidos** del asset correspondiente al set activo
    (`include_bytes!`), los decodifica una vez (`image` crate / SVG raster), y
    registra la textura en el `egui::Context` (`ctx.load_texture`), guardando el
    `TextureHandle` en un `HashMap<IconKey, TextureHandle>`.
  - **`texture(&self, key: IconKey) -> &TextureHandle`**: devuelve la textura
    cacheada; si la clave no tiene asset, devuelve el **genérico** (`Unknown`),
    también cacheado. Nunca falla ni decodifica en caliente.
  - **`reload(&mut self, set: IconSet, ctx: &egui::Context)`**: recarga el atlas
    para el set nuevo (operación única, no por-frame).
- **Mapa `IconKey` → asset**: una tabla que, dado el set activo y la clave, da el
  asset embebido. Esta correspondencia (qué archivo para qué clave) se puede extraer
  a una función testeable sin GPU.
- **Render en el file panel**: la columna Nombre dibuja
  `ui.image((handle.id(), egui::vec2(16.0, 16.0)))` + el nombre, alineados. Tamaño
  fijo ≈16px en 2B (escalable con el tema en 2C). Se eliminan `kind_glyph` y los
  `[D]`/`[?]`.
- **Hueco Shell**: el `IconProvider` se diseña para, en el futuro, consultar primero
  un `ShellIconSource` opcional (de `platform`) y caer al set embebido si no
  responde o está desactivado. En 2B ese campo no existe aún o es siempre `None`.

### Capa `platform` — hueco previsto (NO implementado en 2B)

Se documenta (no se codifica) la interfaz futura `ShellIconSource` que resolverá el
ícono real del Shell de Windows por archivo (`SHGetFileInfo`), con timeout para
discos de red. El `IconProvider` ya prevé consultarla. 2B no la implementa; solo
mantiene la frontera limpia para que enchufarla luego no reescriba la UI.

### Assets

- Tres sets embebidos bajo `assets/icons/{flat,fluent,mono}/`, incluidos en el
  binario vía `include_bytes!` (o un mecanismo de embebido como `rust-embed` si
  resulta más limpio — decidir en el plan). Íconos pequeños (16–32px de origen, o
  SVG); el peso extra del `.exe` es acotado.
- **Licencias**: Flat Color Icons (MIT), Fluent Emoji (MIT), Lucide/Tabler
  (ISC/MIT) — todas compatibles con la MIT del proyecto. **Verificar licencias y
  formatos reales contra la fuente durante la implementación** (subagente Explore),
  y registrar la atribución requerida en un `assets/icons/LICENSES.md`.

### Capa `ui`/`core` — la fila `..`

- **Lógica pura (en `core` o en una función testeable de `ui`)**:
  `should_show_parent_entry(current_dir, show_parent_entry_setting) -> bool` =
  hay padre **y** la opción está activa.
- **Render (file panel)**: si corresponde, pinta una fila `[ícono ParentDir] ..`
  arriba del todo, visualmente separada (o simplemente como primera fila). Es **UI
  pura**, NO entra en `FilePaneState.entries`.
- **Activación**: Enter/doble clic sobre la fila `..` emite
  `PaneRequest::NavigateTo { id, dir: parent }` — la misma acción que
  `Backspace`/`GoUp`.
- **Foco y type-ahead (cuidado crítico)**: `FilePaneState.focused: Option<usize>` y
  el type-ahead indexan `entries`, que NO incluye la fila `..`. El file panel maneja
  el foco de la fila `..` **por separado** del índice de entry (p. ej. un
  `focused_row` que distingue `ParentRow` de `Entry(usize)`), o reserva el foco de
  esa fila sin tocar `entries`. **El type-ahead ignora la fila `..`** (no es un
  nombre buscable). La navegación por teclado de 2A (↑↓ entre filas, Enter) debe
  seguir funcionando, tratando la fila `..` como una fila más arriba de la entrada 0
  cuando está visible. El diseño exacto del foco se detalla en el plan; el invariante
  es: **no romper los índices a `entries` ni el type-ahead**.

---

## 3. Modelo de interacción

- **Íconos**: aparecen automáticamente en cada fila del file panel (tipo de
  archivo), en el árbol/breadcrumb (unidad), y donde haya un elemento de filesystem.
- **Fila `..`** (si está activa y hay padre): primera fila del file panel; ↑↓ la
  alcanzan; Enter/doble clic sube al padre. Type-ahead la salta.
- **Cambiar de set de íconos**: menú ⚙ → "Set de íconos" → Flat / Fluent /
  Monocromo. Aplica recargando el provider (un instante); persiste en `settings.json`.
- **Mostrar/ocultar la fila `..`**: menú ⚙ → checkbox "Mostrar fila .." (default on);
  persiste.

---

## 4. Rendimiento (la razón de la arquitectura)

- **Carga**: al arrancar (y al cambiar de set), el `IconProvider` decodifica y sube
  a GPU el set completo **una vez**. Esto es lo "lento" aceptable (Nicolás: la
  configuración puede ser más lenta).
- **Listado/render**: pintar una fila es `ui.image(handle)` — referencia a una
  textura GPU ya cargada. **Costo idéntico sea cual sea el set**; no hay decodificación
  ni I/O por archivo ni por frame. Tener 3 sets disponibles no afecta la velocidad
  del listado, solo el peso del `.exe` y el instante de recarga al cambiar.
- **Mapa de extensiones**: `HashMap` O(1), no cadenas de `if`. Ícono genérico
  cacheado para extensiones desconocidas.
- **Sin regresión**: el motor de listing por streaming de Fase 1 y el modelo
  multi-panel de 2A no cambian; 2B solo añade el dibujo del ícono por fila.

---

## 5. Manejo de errores

Filosofía del proyecto (el filesystem/los assets son hostiles, la app nunca cae):
- Asset de ícono corrupto/ausente al cargar → esa clave cae al genérico
  (`Unknown`), se loguea, no crashea.
- `IconSet` inválido en `settings.json` → cae al default (Flat) vía la tolerancia
  ya existente de `config`.
- La fila `..` en una raíz sin padre (`C:\`, un share `\\srv`) → no se muestra.
- Decodificar un asset no debe poder colgar el arranque: si un set entero falla, se
  cae al set por defecto y se loguea.

---

## 6. Testing

La ganancia de `core` puro se mantiene:
- **`icon_kind`**: `category_for_extension` (`.stl`→Model3D, `.JPG`→Image
  case-insensitive, `.zip`→Archive, `.exe`→Executable, `.rs`→Code, sin extensión o
  desconocida→Generic); `icon_key_for` (carpeta→Folder, archivo→File(cat),
  Other→Unknown).
- **fila `..`**: `should_show_parent_entry` (con/sin padre; opción on/off);
  la acción de activación (emite NavigateTo al padre correcto) extraída a algo
  testeable.
- **mapa `IconKey`→asset**: que cada `IconKey` relevante tenga un asset en cada set,
  y que la clave sin asset caiga al genérico (testeable sin GPU sobre la tabla).
- **`config`**: round-trip de `Settings` con `icon_set` y `show_parent_entry`;
  tolerancia (set inválido → default).
- **UI/GPU** (carga real de texturas, render del ícono, foco de la fila `..`):
  validación manual; lo que tenga lógica pura se extrae a funciones testeables.

Meta de siempre: build limpio + tests pasando + clippy limpio antes de cada commit.

---

## 7. Estructura de archivos prevista (incremental sobre 2A)

```
crates/core/src/
├── lib.rs                 # + re-exports de icon_kind
├── icon_kind.rs           # IconKey, FileCategory, DriveKind, category_for_extension, icon_key_for
├── config/mod.rs          # + IconSet, Settings { icon_set, show_parent_entry }
└── ...                    # (resto de 2A sin cambios)

crates/ui/src/
├── icons/
│   ├── mod.rs             # IconProvider (carga/cachea texturas, reload, texture())
│   └── assets.rs          # mapa IconKey×IconSet → bytes embebidos (include_bytes!)
├── panes/file_panel.rs    # + dibuja íconos por fila; + fila ".."; quita glifos
├── panes/tree_panel.rs    # + ícono de unidad (genérico)
├── toolbar.rs             # + en ⚙: selector de set de íconos + toggle fila ".."
├── app.rs                 # + IconProvider en NaygoApp; recarga al cambiar set
└── ...

assets/icons/
├── flat/                  # Flat Color Icons (MIT)
├── fluent/                # Fluent Emoji (MIT)
├── mono/                  # Lucide/Tabler (ISC/MIT)
└── LICENSES.md            # atribución de los tres sets
```

---

## 8. Dependencias

- Posible: el crate **`image`** (MIT/Apache) para decodificar PNG, o un rasterizador
  SVG (`resvg`/`usvg`, MPL/libre) si los sets se distribuyen como SVG. **Decidir en
  el plan** según el formato real de los assets elegidos (PNG embebido es lo más
  simple y rápido de cargar; SVG permite escalar pero agrega un rasterizador).
  Recomendación inicial: **PNG embebido a 1–2 tamaños** (16px/32px) por simplicidad
  y velocidad; SVG solo si se quiere escalado fino, que es más propio de 2C.
- `egui` ya provee `ctx.load_texture` / `ui.image`. Sin dependencias nuevas para el
  render.
- Mecanismo de embebido: `include_bytes!` (cero deps) o `rust-embed` (más ergonómico
  para muchos archivos) — decidir en el plan.

> A confirmar en el plan: formato exacto de los assets (PNG vs SVG), crate de
> decodificación, y verificación de licencias contra la fuente.

---

## Fuera de alcance (recordatorio — NO en 2B)

Íconos reales del Shell de Windows (hueco previsto), detección fina de tipo de
unidad, miniaturas de imágenes, animaciones de íconos, i18n (2C), temas/color sets
completos (2C), escalado fino de íconos con el tema (2C), persistencia del reacomodo
manual del dock (deuda de 2A). Nunca: reproducción de media, edición de archivos.
