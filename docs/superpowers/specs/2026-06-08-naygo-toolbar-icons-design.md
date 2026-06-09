# Naygo — Fase toolbar-icons: estilo de toolbar + íconos reales + packs sueltos (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-08. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Fase de pulido visual del sprint. Hoy: la toolbar (`crates/ui/src/toolbar.rs`) usa **glifos
Unicode** (`◀▶▲⟳⧉✂📋🗑🗋🗀➕⚙`) pintados por la fuente del sistema (inconsistentes, mezcla
símbolos+emoji); los íconos de tipo-archivo en `assets/icons/{flat,fluent,mono}/*.png` son
**placeholders** (rectángulos de color de `gen_icons.rs`, NO íconos reales); no hay íconos de
acción para la toolbar; el set de íconos es un enum cerrado `IconSet {Flat,Fluent,Mono}`.

Esta fase entrega TRES cosas en una (el usuario eligió no sub-fasear):
1. **Estilo de toolbar configurable**: Glifos (color del tema por defecto, sobrescribible
   por el usuario) vs Pack (usa el set de íconos activo).
2. **Íconos REALES** en los 3 sets embebidos: tipo-archivo (14 IconKey) + acción de toolbar
   (~12 nuevos), extraídos de los zips. Cierra la tarea pendiente task_55c9844d (reemplazar
   los placeholders).
3. **Packs de íconos sueltos** extensibles (`<config>/icons/<nombre>/`), como temas/lang.

### Decisiones tomadas en el brainstorm

1. **Config "estilo de toolbar":** `Glyphs` (default) | `Pack`. Con Glyphs, el COLOR es el
   del tema por defecto y **sobrescribible** por el usuario (color-picker). El override
   aplica **solo a glifos**. Los packs se ven con su diseño: el set **Mono (lucide) se tiñe**
   con el color del tema (monocromo); **Flat/Fluent multicolor** sin tinte.
2. **Íconos de acción (~12):** Back, Forward, Up, Refresh, Copy, Cut, Paste, Delete,
   NewFile, NewFolder, AddPane, Settings.
3. **Fuentes por set (una c/u, livianas):** Flat = flat-color-icons (MIT), Fluent =
   fluentui-emoji (MIT), Mono = lucide (ISC). **NO se usa** material-design (4.7 GB).
4. **Packs sueltos:** descubrir `<config>/icons/<nombre>/*.png`, listarlos en el selector
   junto a los embebidos, cargar por nombre. Patrón `ThemeCatalog`.
5. **Una sola fase** (código + assets reales juntos). Tolerante: si un asset no se extrae,
   cae al placeholder/`unknown` — la toolbar nunca queda rota.

### Qué entra

- `core::icon_kind`: `IconKey::Action(ActionIcon)` (~12 variantes) + nombre-de-archivo.
- `core::config`: `toolbar_icon_style`, `toolbar_glyph_color: Option<ThemeColor>`; `icon_set`
  pasa de enum a id string (catálogo embebidos + sueltos), con migración serde.
- `core::icon_set` (o ampliar config): `IconSetCatalog` (patrón ThemeCatalog) — embebidos +
  sueltos descubiertos.
- `ui::icons`: `IconProvider` carga acción + tipo del set activo (embebido o suelto); tinte
  de Mono al pintar.
- `ui::toolbar`: render Glifos (color resuelto) vs Pack (Image, tint Mono).
- `ui::settings_window/appearance`: selector Glifos/Pack + color-picker de glifos; el
  selector de set lista embebidos + sueltos.
- **assets**: extraer de los zips los PNGs reales (tipo + acción) a
  `assets/icons/{flat,fluent,mono}/`, reemplazando placeholders. NOTICE de licencias.
- i18n ES/EN.

### Qué NO entra

- material-design-icons (4.7 GB) ni tabler/vscode (no se usan; quedan los zips, gitignored).
- Tinte de Flat/Fluent (son multicolor; solo Mono y glifos se tiñen).
- Commitear los zips (siguen en `.gitignore`).
- shell-B (queda pendiente del sprint).
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

### Capa `core`

**`core::icon_kind`:**
```rust
/// Ícono de una acción de la toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionIcon {
    Back, Forward, Up, Refresh,
    Copy, Cut, Paste, Delete,
    NewFile, NewFolder, AddPane, Settings,
}
impl ActionIcon {
    pub fn all() -> &'static [ActionIcon];
    /// Nombre de archivo (sin extensión) en el set, p. ej. "action_back".
    pub fn file_name(self) -> &'static str;
}
/// `IconKey` gana la variante de acción.
pub enum IconKey {
    Folder, File(FileCategory), Drive(DriveKind), Unknown,
    Action(ActionIcon),
}
```
El `file_name` global (en `ui::icons::assets`) gana el brazo `IconKey::Action(a) => a.file_name()`.

**`core::config`:**
```rust
/// Estilo de los íconos de la barra de herramientas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolbarIconStyle { Glyphs, Pack }
```
- `toolbar_icon_style: ToolbarIconStyle` (default `Glyphs`), serde aditivo.
- `toolbar_glyph_color: Option<ThemeColor>` (default `None` = usar el color del tema;
  `Some(hex)` = color del usuario). `ThemeColor` ya es serializable (#rrggbb).
- **`icon_set` pasa de `IconSet` enum a un id string** (`IconSetId(String)` o `String`), para
  soportar sueltos. Los embebidos tienen ids `"flat"/"fluent"/"mono"`. **Migración serde:**
  un `settings.json` viejo con `"icon_set":"Flat"` (el enum serializado) se acepta y mapea a
  `"flat"`; id desconocido → `"flat"`. (El test existente `icon_set: IconSet::Flat` se
  actualiza al id.)

**`core::icon_set` (módulo nuevo, patrón `ThemeCatalog`):**
```rust
/// Un set de íconos disponible: embebido o suelto.
pub struct IconSetInfo { pub id: String, pub label: String, pub builtin: bool }
/// Catálogo de sets: los 3 embebidos + los sueltos de `<config>/icons/<nombre>/`.
pub struct IconSetCatalog { sets: Vec<IconSetInfo> }
impl IconSetCatalog {
    pub fn load(dir: &Path, active: &str) -> IconSetCatalog; // descubre sueltos (read_dir)
    pub fn available(&self) -> &[IconSetInfo];
    pub fn contains(&self, id: &str) -> bool;
}
```
- `load` siempre incluye flat/fluent/mono (builtin); además, cada subcarpeta de
  `<config>/icons/` es un set suelto con id = nombre de carpeta. Tolerante (read_dir falla →
  solo embebidos).

### Capa `ui`

**`ui::icons` (IconProvider + assets):**
- `assets::bytes_for(set_id, key)` para los EMBEBIDOS (include_bytes! por "flat"/"fluent"/
  "mono" + key incluyendo `action_*`). Para SUELTOS, leer `<config>/icons/<id>/<keyname>.png`
  del disco. Un descubridor en `IconProvider` decide embebido vs suelto por id.
- `IconProvider::new(ctx, set_id, config_dir)` / `reload(ctx, set_id)`: carga el set activo
  (todas las IconKey, tipo + acción) a texturas. Falta un PNG (suelto parcial o asset
  ausente) → cae al `unknown` embebido. `texture(IconKey) -> &TextureHandle`.
- **Tinte:** el IconProvider no tiñe; el tinte de Mono se aplica al PINTAR
  (`egui::Image::new(tex).tint(color)`), porque depende del tema/estilo en runtime.

**`ui::toolbar`:** `icon_button` se bifurca por `settings.toolbar_icon_style`:
- `Glyphs`: `RichText::new(glyph).color(resolved_glyph_color)` donde `resolved =
  toolbar_glyph_color || theme.accent/text`. Cada botón conoce su glifo (hoy) + su ActionIcon.
- `Pack`: `ui.add(egui::Image::new(icons.texture(IconKey::Action(a))).fit_to_exact_size(...))`,
  con `.tint(theme_color)` SOLO si el set activo es Mono. Tooltip + enabled como hoy.
El strip de discos (de shell-A) sigue como está (botones de letra); fuera de alcance retocar.

**`ui::settings_window/appearance`:** 
- Selector segmentado Glifos/Pack (`selectable_value` sobre `toolbar_icon_style`).
- Color de glifos: un `color_edit_button_srgba` (egui) + un botón "usar color del tema" que
  pone `toolbar_glyph_color = None`. Visible/relevante cuando el estilo es Glifos.
- El selector de "Set de íconos" pasa a listar `IconSetCatalog::available()` (embebidos +
  sueltos) en vez del enum fijo.

### Assets (lo manual, con fallback)

- Extraer de los zips a `assets/icons/{flat,fluent,mono}/`: por set, los 14 de tipo
  (`folder`, `drive`, `unknown`, `file_*`) + 12 de acción (`action_*`) = 26 PNG; reemplazan
  los placeholders. Tamaño objetivo ~32px (coherente con lo actual). Nombres EXACTOS que
  `file_name` espera.
- **Tolerante:** si un ícono no se extrae bien, se deja el placeholder anterior o el
  `unknown`; nunca toolbar/listado rotos. `gen_icons.rs` (placeholders) se conserva como
  fallback/herramienta, no se borra.
- **Licencias:** flat-color (MIT), fluentui (MIT), lucide (ISC) — todas libres. Añadir un
  `assets/icons/NOTICE.md` con la atribución. Los zips NO se commitean (`.gitignore` ya).

### Lo que NO cambia

El file panel sigue pintando vía `icons.texture(key)` (ahora con assets reales). El sistema
de temas, ops, etc. intactos.

---

## 3. Flujo de datos

**Arranque:** `IconSetCatalog::load(config_dir, settings.icon_set)` → lista de sets;
`IconProvider::new(ctx, settings.icon_set, config_dir)` carga el set activo (tipo + acción).
**Toolbar (cada frame):** por botón, según `toolbar_icon_style`: Glyphs → glifo coloreado;
Pack → `Image` de `texture(Action(a))` (tint si Mono). **Cambiar set/estilo/color (Settings):**
mutar `settings.*`; al cambiar el set, `IconProvider::reload`; persistir.

## 4. Manejo de errores / casos límite

- **PNG suelto corrupto / faltante** → fallback al `unknown` embebido. Sin crash.
- **`<config>/icons/` ausente** → solo embebidos.
- **Set activo (suelto) borrado entre sesiones** → id desconocido → "flat".
- **Asset embebido no extraído** (build) → placeholder/`unknown`. Toolbar nunca rota.
- **Glyph color None** → color del tema (resuelto por frame). `Some` → color fijo.
- **Estilo Pack + set Mono** → tinte por tema; Flat/Fluent sin tinte.
- **settings.json viejo con `icon_set` enum** → migra al id string.

## 5. Testing

- **`core::icon_kind`**: `ActionIcon::all()` cubre las 12; `file_name` único por acción
  (test como el de tipo-archivo); `IconKey::Action` round-trips.
- **`core::config`**: round-trip de `toolbar_icon_style`, `toolbar_glyph_color` (None/Some),
  `icon_set` como id string; migración de un settings viejo con el enum; id desconocido →
  "flat".
- **`core::icon_set::IconSetCatalog`** (puro, tempfile): embebidos siempre presentes;
  descubre sueltos de un dir simulado (carpetas/PNGs); read_dir ausente → solo embebidos;
  `contains` correcto.
- **`ui::icons::assets`**: `file_name`/`bytes_for` cubren las nuevas claves de acción; cada
  (set embebido, key) resuelve a bytes o al `unknown`.
- **UI**: manual (alternar Glifos/Pack; color-picker; cambiar de set incl. uno suelto;
  toolbar con íconos reales; Mono teñido por tema; pack suelto parcial cae al fallback).

Meta: build + tests + clippy + fmt verde antes de cada commit. Licencias verificadas y en
NOTICE.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── icon_kind.rs     # + ActionIcon + IconKey::Action
├── icon_set.rs      # NUEVO: IconSetCatalog (patrón ThemeCatalog)
├── config/mod.rs    # + ToolbarIconStyle + toolbar_glyph_color; icon_set → id string + migración
├── lib.rs           # + pub mod icon_set;
└── i18n/{es,en}.json # + settings.toolbar.* (estilo/color)

crates/ui/src/
├── icons/mod.rs     # IconProvider: set por id (embebido/suelto), carga acción+tipo, config_dir
├── icons/assets.rs  # file_name + bytes_for: claves action_*; include_bytes! por id embebido
├── toolbar.rs       # icon_button: Glyphs (color) vs Pack (Image, tint Mono)
├── settings_window/appearance.rs  # selector Glifos/Pack + color-picker; set list del catálogo
└── (gen_icons.rs se conserva como fallback)

assets/icons/
├── {flat,fluent,mono}/  # PNGs REALES (tipo + action_*), reemplazan placeholders
└── NOTICE.md        # NUEVO: atribución de licencias (flat-color MIT, fluentui MIT, lucide ISC)
```

---

## 7. Dependencias

Ninguna nueva de terceros. `image` (ya en core/ui) para decodificar; egui `color_edit_button_srgba`
+ `Image::tint` (ya en egui). Sin chrono. Los íconos se extraen de zips locales (no se
commitean). Licencias libres (MIT/ISC).

---

## Fuera de alcance (recordatorio)

material-design/tabler/vscode icons, tinte de Flat/Fluent, commitear zips, shell-B. Nunca:
reproducción de media, edición de archivos.
