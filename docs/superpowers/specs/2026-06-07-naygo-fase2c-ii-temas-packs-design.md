# Naygo — Fase 2C-ii: temas / color sets / packs (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-07. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Cierra el bloque visual de Naygo: un sistema de **temas (color sets)** intercambiables
en caliente, cargables de archivos (como i18n), con una paleta propia completa; más
**packs** (preset que activa tema + set de íconos juntos). Conecta con la Fase 2E: el
color de acento, hoy hardcoded (`0x2f81f7`), pasa a salir del tema activo.

**Premisa rectora:** respuesta rápida y fluida + bajo consumo. Aplicar un tema es una
operación una-vez-por-cambio (no por frame); cero costo en runtime. Los íconos no se
recolorean (son assets cacheados en GPU); el tema solo afecta el "chrome" (colores de
la UI).

### Decisiones tomadas en el brainstorm

1. **Tema e icon set son INDEPENDIENTES**; un "pack" es un atajo que setea ambos a la
   vez, tras lo cual el usuario puede ajustar cada uno por separado. (El `icon_set`
   actual, enum `Flat`/`Fluent`/`Mono`, queda como está.)
2. **Paleta completa Naygo**: cada tema define un set amplio de tokens propios + la base
   claro/oscuro de egui. (No solo acento.)
3. **Acento FIJO dentro de cada tema** (no hay un selector de acento aparte). Para otro
   acento → otro tema o un tema soltado.
4. **4 temas de fábrica embebidos**: Dark Blue (default), Dark Teal, Light, High
   Contrast.
5. **Temas soltables**: archivos **JSON** en una carpeta (patrón i18n), tolerantes a
   campos faltantes (caen al default del base). Sin dependencias nuevas (serde_json).
6. **Selector** en Configuración → Apariencia: **tarjetas con preview** (swatches de la
   paleta) por tema; cambio **en caliente** (al instante, como el idioma).

### Qué entra

- `core::theme`: `Theme` (paleta completa), `ThemeBase`, `ThemeColor` (hex), `ThemeId`,
  `ThemeCatalog` (4 embebidos + sueltos), parseo/serde/fallback. Puro, testeado.
- `core::theme` (packs): `Pack { name, theme, icon_set }`, `PackCatalog` (embebidos +
  sueltos).
- `core::config::Settings`: campo `theme: ThemeId` (`#[serde(default)]`).
- `ui::theme_apply`: traduce `Theme` → `egui::Visuals` + aplica a `ctx`; expone los
  tokens propios a los paneles vía un `ActiveTheme` compartido.
- `ui`: selector de tema (tarjetas) + sección de packs en Apariencia; hot-swap en
  `app.rs`; reemplazar los colores hardcoded de 2E por tokens del tema.
- i18n: claves de UI nuevas (ES/EN). Los NOMBRES de tema/pack no se traducen (vienen
  del JSON, para que los soltados funcionen).

### Qué NO entra

- Recolorear íconos por tema (son assets; el icon set se elige aparte).
- Editor de temas dentro de la app (se editan soltando/editando JSON).
- Selector de acento independiente (acento fijo en el tema).
- Validación de contraste/accesibilidad de temas soltados (responsabilidad del autor;
  Naygo no cae, solo se ve mal).
- Watcher, ops, etc. Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

Idea rectora: el tema es DATOS en `core` (puro, serializable, testeable); la `ui` lo
traduce a `egui::Visuals` y lo aplica al `Context`. Sigue el patrón de i18n (embebidos
+ sueltos + hot-swap) y el de íconos (resolver activo + recargar al cambiar).

### Capa `core` — módulo nuevo `theme`

```rust
/// Base visual: de ella egui deriva los neutros.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeBase { Light, Dark }

/// Color RGB serializado como "#RRGGBB".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeColor { pub r: u8, pub g: u8, pub b: u8 }
// serde: Serialize/Deserialize a/desde "#rrggbb" (impl manual o with).
// Parseo tolerante: "#rrggbb" (con o sin '#'); inválido → error de ese campo.

/// Identificador de un tema (su nombre-clave estable, como LangId).
pub struct ThemeId(pub String);

/// Paleta completa de un tema (tokens propios de Naygo).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,           // visible; NO se traduce
    pub base: ThemeBase,
    pub accent: ThemeColor,     // selección activa, panel activo, indicadores, drop line
    pub panel_bg: ThemeColor,
    pub row_bg: ThemeColor,
    pub row_alt_bg: ThemeColor, // fila alternada (striped)
    pub text: ThemeColor,
    pub text_dim: ThemeColor,
    pub selection_bg: ThemeColor,
    pub active_bar: ThemeColor, // barra del panel/fila activa
    pub error: ThemeColor,
    pub border: ThemeColor,
}
```

- Cada campo de color con `#[serde(default = "...")]` para tolerar faltantes: el default
  depende del `base` (un helper da los neutros de claro u oscuro). Estrategia simple:
  deserializar a un `ThemeRaw` con `Option<ThemeColor>` por campo, luego resolver contra
  el default del base. Mantiene la tolerancia sin perder la paleta completa.
- **`ThemeCatalog`** (paralelo a `i18n::Catalog`):
  - 4 temas embebidos como JSON `include_str!` en `core/src/theme/builtin/*.json`.
  - Temas sueltos desde `<config_dir>/themes/*.json` (el id = nombre del archivo sin
    extensión, como `lang/<id>.json`).
  - API: `load(config_dir, active: &ThemeId) -> ThemeCatalog`, `available() -> &[ThemeId]`,
    `get(&ThemeId) -> &Theme`, `default_id() -> ThemeId` (Dark Blue). Id desconocido →
    `get` cae al default.
- **Packs**: `Pack { name: String, theme: ThemeId, icon_set: IconSet }`; `PackCatalog`
  (embebidos en `core/src/theme/packs/*.json` + `<config_dir>/packs/*.json`). 4 packs
  embebidos (uno por tema con un icon set que combine).

### Capa `core::config`

`Settings` gana `pub theme: ThemeId` con `#[serde(default = "default_theme")]`
(default = Dark Blue id). `icon_set` sin cambios. Retro-compat: settings viejo sin
`theme` cae al default.

### Capa `ui` — `theme_apply.rs` (nuevo) + `ActiveTheme`

- `struct ActiveTheme { id: ThemeId, theme: Theme }` guardado en `NaygoApp` (como
  `icons`/`i18n`). Expone `accent()`, `active_bar()`, `text_dim()`, etc. como
  `egui::Color32` (conversión `ThemeColor → Color32`).
- `fn apply(theme: &Theme, ctx: &egui::Context)`: parte de `egui::Visuals::light()` o
  `::dark()` según `base`, sobrescribe los campos relevantes:
  - `visuals.panel_fill` / `window_fill` ← panel_bg.
  - `visuals.selection.bg_fill` ← selection_bg; `selection.stroke`/acento ← accent.
  - `visuals.extreme_bg_color` (striped alterno) ← row_alt_bg.
  - `visuals.override_text_color` ← text (y los `widgets.*.fg_stroke` según haga falta).
  - `visuals.hyperlink_color` ← accent. `widgets.*.bg_fill`/bordes ← border donde aplique.
  - `ctx.set_visuals(visuals)`.
  (El mapeo exacto a campos de `egui::Visuals` 0.34 se verifica contra la fuente al
  implementar; el objetivo es que el chrome refleje la paleta.)
- **Hot-swap** (en `app.rs::ui()`, patrón idéntico al de íconos en ~app.rs:610):
  `if self.active_theme.id != self.settings.theme { self.active_theme = resolve(...); theme_apply::apply(&self.active_theme.theme, ctx); }`.

### Conexión 2E (reemplazar hardcoded)

Los usos de `Color32::from_rgb(0x2f,0x81,0xf7)` (barra del panel activo en `docking.rs`
title(); selección/barra de fila y línea de drop en `file_panel.rs`; resaltado del nodo
activo en `tree_panel.rs`; ≡ de filtro) pasan a leer del `ActiveTheme` (que se pasa a
esos paneles como ya se pasa `icons`/`i18n`). El rojo de error (`tree.access_denied`,
"sin coincidencias" no usa color) ← `error` token.

### Paneles reciben `&ActiveTheme`

`NaygoTabViewer` y las funciones `show` de los paneles ganan un `&ActiveTheme` (mismo
patrón que `icons`). Sin lógica nueva; solo lectura de colores.

---

## 3. Flujo de datos

Arranque: `ThemeCatalog::load(config_dir, settings.theme)` → resolver `ActiveTheme` →
`theme_apply::apply` una vez. (Igual que i18n carga su catálogo.)

Por frame (en `ui()`): si `active_theme.id != settings.theme`, recargar el tema activo
y aplicarlo; si no, nada. Los paneles leen los tokens del `ActiveTheme` al pintar.

Cambio de tema (selector de tarjetas): clic setea `settings.theme`; el hot-swap del
frame siguiente lo aplica (inmediato a la vista). Tema soltado nuevo aparece como
tarjeta extra automáticamente (sale del catálogo).

Pack: activar un pack escribe `settings.theme` + `settings.icon_set`; el hot-swap de
tema y el de íconos (ya existente) aplican ambos. Luego el usuario puede ajustar cada
uno por separado.

Persistencia: `settings.theme` se persiste (config). El catálogo NO se persiste (se
relee de embebidos + carpeta en cada arranque, como i18n).

---

## 4. Manejo de errores / casos límite

- Tema/pack suelto con JSON inválido → se ignora ese archivo (log discreto); los demás
  cargan. No rompe el catálogo.
- Campo de color faltante o hex inválido → ese token cae al default del base; el tema
  carga igual.
- `settings.theme` apunta a un tema inexistente (borrado) → fallback a default, sin
  pánico.
- Pack que referencia tema/icon set inexistente → al activarlo, cada parte inexistente
  cae a su default; no crashea.
- Contraste ilegible en un tema soltado → no se valida (responsabilidad del autor);
  Naygo no cae.

---

## 5. Testing

- **`core::theme`** (el grueso, sin egui): parseo hex ↔ `ThemeColor` (válido/ inválido);
  round-trip serde de `Theme`; fallback de campos faltantes (Option→default del base);
  los 4 temas embebidos parsean; `ThemeCatalog` (embebidos presentes; id desconocido →
  default; suelto añade/ sobrescribe por id); `default_id` = Dark Blue.
- **`core::theme` packs**: `Pack` round-trip; `PackCatalog` (embebidos; suelto;
  referencia inexistente tolerada al resolver).
- **`core::config`**: `Settings` con `theme`, round-trip + `#[serde(default)]` (settings
  viejo sin `theme` → default).
- **`ui::theme_apply`**: si surge una pieza pura (`ThemeColor → Color32`), test simple;
  la aplicación a `ctx` y el selector son validación manual.
- **UI** (selector de tarjetas, hot-swap, acento desde el tema): validación manual.

Meta de siempre: build limpio + tests + clippy antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── theme/
│   ├── mod.rs              # Theme, ThemeBase, ThemeColor, ThemeId, ThemeCatalog
│   ├── pack.rs            # Pack, PackCatalog
│   ├── builtin/           # dark-blue.json, dark-teal.json, light.json, high-contrast.json
│   └── packs/             # 4 packs embebidos *.json
├── config/mod.rs          # + Settings.theme (serde default)
├── lib.rs                 # + pub mod theme; re-exports
└── i18n/{es,en}.json      # + claves de UI del selector (Tema, Packs, …)

crates/ui/src/
├── theme_apply.rs         # NUEVO: Theme→Visuals + apply(ctx); ActiveTheme + Color32 getters
├── settings_window/appearance.rs  # + selector de tema (tarjetas) + sección de packs
├── app.rs                 # + ActiveTheme + hot-swap del tema; pasar &ActiveTheme a paneles
├── docking.rs             # title() del panel activo usa accent del tema
├── panes/file_panel.rs    # selección/barra/línea de drop/≡ usan tokens del tema
├── panes/tree_panel.rs    # resaltado nodo activo usa tokens del tema
└── ...
```

---

## 7. Dependencias

Ninguna nueva. Todo con `serde`/`serde_json` (ya presentes) y egui 0.34.3. El mapeo
`Theme → egui::Visuals` se verifica contra la fuente de egui 0.34.3 al implementar.

---

## Fuera de alcance (recordatorio)

Recolorear íconos por tema, editor de temas in-app, selector de acento aparte,
validación de contraste, watcher/ops. Nunca: reproducción de media, edición de archivos.
