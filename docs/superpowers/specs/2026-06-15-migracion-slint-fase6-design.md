# Fase 6 (Slint): pulido visual + paridad total + retiro de egui — Diseño

> Última fase de la migración egui→Slint de Naygo. Lleva el look de Slint al nivel del de egui
> (íconos de color por tipo, toolbar pulida, columnas inteligentes, rename en cadena), completa
> la paridad funcional pendiente, mantiene/extiende los packs importables (temas+íconos+idiomas),
> y retira la capa egui. Contrato: `docs/migracion-slint/CONTRATO-PARIDAD-FUNCIONAL.md`.

## Principio rector (NO negociable)
**El default es liviano y rápido: cero GPU, respuesta instantánea.** El render de Slint es por
software. Por eso:
- Los íconos son **PNG 32×32 pre-rasterizados y cacheados** a `slint::Image` una sola vez (no se
  re-decodifica por frame). El costo de pintar una fila no cambia respecto de hoy: un bitmap
  cacheado en vez de un emoji.
- Sin animaciones en el default. El resaltado/hover es un cambio de color instantáneo.
- Todo lo "vistoso" (gradientes, animaciones, chips) queda detrás de un toggle opcional —
  **fuera de alcance de F6 salvo que sobre tiempo**; F6 entrega el nivel "B · pulido liviano".
- El FUNCIONAMIENTO siempre manda sobre lo visual (lema del proyecto).

## Decisiones de Nicolás (brainstorm)
- Default visual = "B · pulido liviano" (íconos de color por tipo, toolbar con íconos).
- Set de íconos: los 3 (flat/fluent/mono) disponibles en Configuración, **default Flat**.
- Packs importables deben incluir **sets de íconos propios**, no solo temas (colores) e idiomas.
- Columnas: agregar/quitar/reordenar; formato de tamaño configurable, **default Automático
  legible** (512 B / 1.5 KB / 20.2 MB); identación/alineación inteligente por tipo de columna.
- Rename inline (ciclo F2 nombre/ext/todo) + **rename EN CADENA con ↑/↓**, seleccionando el
  **nombre sin extensión** al saltar al siguiente archivo.
- Paridad TOTAL con lo que ya existía en egui.

## Reutilización del core (todo ya existe y está testeado)
- `core::icon_kind`: `icon_key_for(&Entry) -> IconKey`, `category_for_extension`, `IconKey`/
  `FileCategory`/`ActionIcon`/`DriveKind`.
- `crates/ui/src/icons/assets.rs` (HOY en egui, se MUEVE a core): `bytes_for(IconSet, IconKey)
  -> &[u8]` (PNGs embebidos de los 3 sets), `file_name(IconKey)`, `all_keys()`. Es lógica PURA
  (solo `include_bytes!` + `naygo-core`), encaja en `naygo-core::icons`.
- `core::columns`: `TableState { columns: Vec<ColumnSpec>, filters }`, `ColumnKind`
  (Name/Extension/Size/Modified/Created), `visible_columns`, `toggle_visible`, reorder, anchos,
  `MIN/MAX_COLUMN_WIDTH`. `FilePaneState.table` ya persiste (F4).
- `core::format::human_size` (auto) — se extiende con un `SizeFormat` configurable.
- `core::ops::actions::rename(path, new_name) -> OpRequest` para el rename real.
- `config::Settings.icon_set: IconSet` (ya existe el campo).

## Arquitectura — 6 sub-fases verticales
Cada sub-fase: compila + tests + verificación en vivo (binario real, esta máquina, Win32/
PrintWindow + computer-use) + commit. Nicolás mide rendimiento en la VM al cierre.

### 6A — Íconos de archivo de color (cacheados)
- **Mover assets a core:** `crates/ui/src/icons/assets.rs` → `crates/core/src/icons/mod.rs`
  (renombrar referencias de ruta de los `include_bytes!`: de `../../../../assets/...` a
  `../../../assets/...`). Re-exportar desde egui (`crates/ui/src/icons/assets.rs` pasa a
  `pub use naygo_core::icons::*;`) para no romperlo. `IconSet` ya vive en `config`.
- **IconCache (ui-slint):** módulo nuevo `crates/ui-slint/src/icons.rs` con
  `IconCache { map: HashMap<(IconSet, IconKey), slint::Image> }` y
  `get(&mut self, set, key) -> Image`: si falta, decodifica el PNG (`icons::bytes_for`) con el
  crate `image` a RGBA → `slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(...))`,
  cachea y devuelve. WorkspaceCtrl posee un `IconCache`. **Garantía de rendimiento:** ~28 PNGs ×
  1 set = decodificados una vez; las filas reusan el `Image` (clonar un `slint::Image` es barato,
  comparte el buffer).
- **Filas:** `PlainRow`/`RowData` ganan `icon: Image`. El bridge resuelve por entry
  (`icon_key_for`). `TreeRow`/`NavRow` (favoritos) y el disco usan el ícono cacheado también.
  El file-panel pinta `Image` en vez del emoji 📁.
- **Config:** combo "Set de íconos" (Flat/Fluent/Mono) en Configuración → Apariencia
  (`ConfigCtrl::set_icon_set`); al cambiar, invalida/repuebla el cache y refresca.
- **Errores:** PNG ilegible → cae a `unknown` (lo cubre `bytes_for`). Si `image` falla, ícono
  vacío (no crashea).
- **Testing:** `bytes_for`/`all_keys` ya testeados en core (se mueven con sus tests). Test del
  IconCache: `get` dos veces de la misma clave devuelve el mismo handle (cacheado).

### 6B — Toolbar con íconos + pulido liviano
- Los botones de la toolbar muestran su PNG de acción (`IconKey::Action(...)`) + tooltip (ya
  existe `Tr.toolbar-*`). Respeta `Settings.icon_only` (solo ícono vs ícono+texto) y
  `toolbar_icon_style` (glifos vs pack — ya en Settings).
- Pulido liviano: espaciado de filas, padding de la toolbar, jerarquía del header. SIN
  animaciones. Reusa los tokens del global `Theme`.
- **Testing:** visual en vivo (no unit). Confirmar que la toolbar y las filas se ven con íconos.

### 6C — Columnas dinámicas + formato inteligente
- **Render dinámico:** el file-panel deja de tener 4 columnas fijas; pinta las
  `table.visible_columns()` en su orden, con su ancho. El header y las filas iteran las columnas
  del `TableState` del panel. (El `TableState` ya persiste por panel.)
- **Agregar/quitar:** menú en el header (clic derecho o un botón "Columnas…"): toggles de
  Name/Extension/Size/Modified/Created (Name nunca se oculta). Reusa `TableState::toggle_visible`.
- **Reordenar:** arrastrar el encabezado de una columna la reordena (reusa el reorder de
  `TableState`). Resize de ancho arrastrando el borde (respeta MIN/MAX).
- **Formato de tamaño:** nuevo `core::format::SizeFormat { Auto, Bytes, Kb, Mb }` + `format_size
  (bytes, SizeFormat)`. Default `Auto` (= `human_size`). Bytes con separadores de miles. Campo
  `Settings.size_format` + combo en Configuración. El bridge formatea la celda Size según el
  ajuste.
- **Identación/alineación:** Size alineado a la derecha; Name a la izquierda con su ícono;
  fechas a la izquierda. Cada `ColumnKind` define su alineación.
- **Testing:** `format_size` (Auto/Bytes/Kb/Mb) en core; `toggle_visible`/reorder ya testeados.
  Test del bridge: las celdas se generan en el orden de las columnas visibles.

### 6D — Rename inline + en cadena
- **Rename inline (F2):** sobre la fila enfocada, F2 (o el menú contextual → Renombrar) abre un
  editor en la celda Name con el **ciclo de selección**: 1ª pulsación = nombre sin extensión, 2ª =
  solo extensión, 3ª = todo. Sin extensión (carpetas, dotfiles) → siempre todo. (Lógica de rangos
  pura: portar `apply_rename_selection` de egui a un helper de core testeable, o a keys/bridge.)
- **Commit:** Enter confirma → `ops::actions::rename(path, nuevo) -> start_op` (reusa el engine de
  F3, con su validación de nombre). Esc cancela.
- **Rename EN CADENA:** con el editor abierto, ↑/↓ CONFIRMA el nombre actual y abre el editor del
  archivo anterior/siguiente, seleccionando el **nombre sin extensión** (decisión de Nicolás). Se
  mantiene el modo edición a lo largo de la lista.
- **Slint:** el editor es un `LineEdit` sobre la celda Name del row enfocado (estado de rename a
  nivel AppWindow: `rename-pane`, `rename-pos`, `rename-text`, `rename-stage`). Captura ↑/↓/Enter/
  Esc. Reusa el patrón de la path-bar (F-bis).
- **Testing:** los rangos del ciclo F2 (helper puro) en core; el rename en cadena (avanzar foco +
  reabrir) con un test del controlador.

### 6E — Packs de usuario con íconos (extensión de F4)
- **Estado actual (F4):** `packs::{export_lang, export_theme, export_config, import_zip}` empaqueta
  `lang/<code>.json`, `themes/<id>.json`, `settings.json`/`keybindings.json`.
- **Extensión:** un pack de TEMA puede traer su propio **set de íconos**. Estructura del .zip:
  `themes/<id>.json` (+ opcional `icons/<id>/<name>.png` para los íconos del tema). `import_zip`
  detecta y extrae los PNGs a `<config_dir>/icons/<id>/`; `export_theme` los incluye si existen.
- **Carga de íconos de usuario:** el `IconCache`/`icons::bytes_for` se extiende para buscar
  PRIMERO en `<config_dir>/icons/<set>/<name>.png` (set de usuario) y caer a los embebidos. Así un
  set de íconos importado se usa sin recompilar (paralelo a como los temas se cargan de
  `themes/*.json`). El catálogo de sets disponibles incluye los embebidos + los de usuario.
- **Testing:** round-trip export/import de un tema CON íconos (un PNG de prueba); el IconCache
  prefiere el de usuario si existe.

### 6F — Paridad restante + retiro de egui + cierre
- **Repaso del contrato:** implementar lo pendiente que la lista marque sin tachar —
  rubber-band (selección por rectángulo desde zona vacía), Ctrl+arrastre aditivo, y cualquier
  gesto/atajo faltante. (Inventario al iniciar la sub-fase con un diff contra el contrato.)
- **Retiro de egui:** una vez verificada la paridad, el binario Slint hereda el nombre `naygo`
  (renombrar `[[bin]]` o el paquete según corresponda), se retira `crates/ui` (egui) y `naygo-ui`
  del workspace, se limpian deps egui-only. (La capa egui ya solo existía para comparar.)
- **Distribución:** el `dist/` y el instalador apuntan al binario final; CRT estático/portable
  (ver [[project-distribucion-naygo]]).
- **Cierre:** verificación integral en vivo (íconos, columnas, rename en cadena, packs), gate del
  workspace, `graphify update .`, dist + memoria, push.

## Manejo de errores (transversal)
Coherente con "el filesystem/SO es hostil": PNG ilegible → ícono unknown; rename que falla
(permiso, nombre inválido) → el engine de ops lo reporta discreto; columna/pack corrupto → se
ignora sin crashear.

## Testing (transversal)
- Unit/integración donde la lógica es pura: format_size, rangos del ciclo F2, IconCache,
  columnas (ya testeadas), packs con íconos.
- Verificación en vivo (binario real) para lo visual y los gestos.
- Nicolás mide rendimiento en la VM al cierre (clave de toda la migración: el default debe seguir
  ~0% CPU en reposo).

## Convenciones (obligatorias)
- `graphify query` antes de leer/grepear; `graphify update .` tras cambios.
- Gate antes de cada commit: `cargo test --workspace` + `cargo clippy --workspace --all-targets
  -- -D warnings` + `cargo fmt --all -- --check`. Stage explícito (CLAUDE.md y graphify-out/ no
  se commitean). Commits con `Co-Authored-By: Claude Opus 4.8`.
- i18n: texto nuevo a claves (Tr + es/en en paridad). Colores vía Theme.
- Header en archivos nuevos.

## Fuera de alcance
El nivel visual "C · vistoso" (gradientes, animaciones de hover, chips) — queda como mejora
opcional posterior, detrás de un toggle, sin comprometer el default liviano.
