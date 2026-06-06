# Naygo — Fase 2C-i: Ventana de Configuración + i18n (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-06. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Las fases **1**, **2A** (layout dinámico) y **2B** (íconos + fila "..") están
completas. El bloque visual se cierra con **2C**, dividido en dos sub-fases:

- **2C-i (ESTE documento):** ventana de **Configuración** con secciones (un viewport
  separado del SO) + **i18n** (catálogo de strings ES/EN; todo el texto deja de
  estar hardcoded; cambio de idioma en caliente).
- **2C-ii (después):** temas + color sets + "packs" (íconos+color combinados),
  recoloreado del set monocromo.

**Premisa rectora (de Nicolás):** respuesta rápida y fluida. En 2C-i: `t(clave)`
(resolver un texto) es un lookup O(1) en el catálogo del idioma activo, sin I/O ni
alocación por llamada; los archivos de idioma se cargan UNA vez (al arrancar / al
cambiar idioma). La ventana de Configuración es un viewport aparte que no afecta el
render del listado.

### Qué entra en 2C-i

- **`core::i18n`** (lógica pura, testeable): catálogo clave→texto por idioma,
  `t(clave)` con fallback, carga de catálogos embebidos (ES/EN) y de archivos
  sueltos (`lang/*.json` al lado del `.exe`), cambio de idioma en caliente.
- **`pick_default_language`**: detección del idioma del SO (función pura que recibe
  el string de locale; la lectura real del SO se inyecta).
- **Migración de TODO el texto hardcoded a claves** (`menu.*`, `toolbar.*`,
  `pane.*`, `inspector.*`, `settings.*`, `template.*`). ES + EN completos.
- **`Settings.language`** (persistido).
- **Ventana de Configuración** como **viewport separado del SO** (egui
  multi-viewport), con secciones: Apariencia, Paneles, Atajos (solo-lectura),
  Idioma, Avanzado. Reemplaza el menú ⚙ inline actual.
- **Migrar las opciones del menú ⚙** (set de íconos, fila "..", posición de barra,
  solo-íconos) a la ventana, agrupadas por sección.
- **Selector de idioma** en caliente.

### Qué NO entra en 2C-i

Temas + color sets completos (la sección Apariencia muestra un **placeholder** de
tema Claro/Oscuro/Sistema sin efecto real todavía — el motor de temas es 2C-ii);
packs (íconos+color); recoloreado del set monocromo; **edición** de atajos
configurables (la sección Atajos es solo-lectura: muestra el mapa actual); el
multi-ventana del explorador (aunque la ventana de Configuración valida el
mecanismo de multi-viewport de egui); persistencia del reacomodo manual del dock
(deuda de 2A). Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

Idea rectora intacta: **`core` decide el QUÉ (textos por clave, puro y testeable);
`ui` decide el CÓMO se ve (la ventana, el render).** El listado no paga nada en
runtime por i18n más allá de un lookup por texto.

### Capa `core` — módulo nuevo `i18n` (lógica pura)

- **`LangId`**: identificador de idioma, p. ej. un `String` corto (`"es"`, `"en"`)
  o un newtype `LangId(String)`. Serializable.
- **`Catalog`**: un `HashMap<String, String>` (clave → texto) para un idioma, más
  su `LangId`.
- **`I18n`**: el estado de internacionalización.
  - `active: Catalog` — el idioma activo.
  - `fallback: Catalog` — el idioma de respaldo (ES embebido), siempre presente.
  - `available: Vec<LangId>` — idiomas cargados (para el selector).
  - `t(&self, key: &str) -> &str`: texto de `key` en el activo; si falta, en el
    fallback; si tampoco, **devuelve `key`** (visible, nunca panic).
  - `set_language(&mut self, lang: LangId)`: cambia el catálogo activo (de los
    cargados).
  - Construcción: `I18n::load(dir, lang)` — parte de los embebidos (ES/EN vía
    `include_str!`), luego escanea `dir/lang/*.json` y los carga/mergea (un archivo
    suelto puede añadir un idioma nuevo o sobreescribir claves de uno embebido),
    activa `lang`.
- **`pick_default_language(locale: &str, available: &[LangId]) -> LangId`**: dado el
  string de locale del SO (p. ej. `"es-CL"`, `"en-US"`, `"fr-FR"`), elige: si
  empieza con `"es"` → ES; si hay un idioma disponible que matchee el prefijo → ese;
  si no → EN (fallback internacional). Pura y testeable.
- **Parsing tolerante**: un `lang/*.json` ilegible se ignora (log + se omite ese
  idioma); los embebidos garantizan que ES y EN siempre existen. Nunca panic.
- Formato JSON plano: `{ "menu.file.copy": "Copiar", "pane.empty": "Vacío" }`.

### Capa `core` — `config`

- `Settings.language: LangId` (persistido en `settings.json`, con `#[serde(default)]`
  retro-compatible como los campos de 2B). El default se calcula con
  `pick_default_language` al primer arranque (sin settings previo).

### Capa `ui`

- **`I18n` en `NaygoApp`**: un campo `i18n: I18n`. Se construye en `new()` leyendo
  el locale del SO (vía `std::env`/API ligera) para el default, o el
  `Settings.language` persistido si existe. Un helper `tr(&self, key) ->` para
  abreviar `self.i18n.t(key)` en el render.
- **Migración de textos**: todos los literales de UI (`"Carpetas"`, `"Subir un
  nivel"`, `"Listando…"`, `"{n} elementos"`, `"Nada seleccionado."`, los labels de
  la toolbar, los nombres de plantilla built-in, etc.) → `t("clave")`. Los textos
  con interpolación (`"{n} elementos"`) usan un patrón simple: la clave da el
  formato (`"pane.count"` → `"{n} elementos"` / `"{n} items"`) y la UI sustituye
  `{n}`. Los **nombres de plantillas del usuario** quedan literales (no se traducen).
- **`ui::settings_window` (nuevo)**: la ventana de Configuración.
  - Estado en `NaygoApp`: `settings_open: bool`, `settings_section: SettingsSection`.
  - `SettingsSection` enum: `Appearance, Panes, Shortcuts, Language, Advanced`.
  - Se muestra como **viewport separado** con `ctx.show_viewport_immediate(...)` (la
    forma `immediate` permite capturar `&mut` al estado en el closure — VERIFICAR la
    API exacta de multi-viewport de egui 0.34 contra la fuente antes de implementar;
    es el punto de mayor riesgo). Layout interno: `SidePanel` de secciones +
    `CentralPanel` que despacha a `show_<section>`.
  - Las secciones (`appearance`, `panes`, `shortcuts`, `language`, `advanced`) son
    funciones enfocadas, una por archivo o agrupadas en `settings_window/`.
  - Cerrar la ventana (la X del SO o un botón) pone `settings_open = false`.
- **Toolbar**: el botón ⚙ ahora hace `settings_open = true` (abre la ventana). El
  menú inline de 2A/2B se elimina; sus opciones viven en la ventana.
- **Reactividad en caliente**: cambiar idioma, set de íconos, fila "..", posición de
  barra desde la ventana surte efecto en el siguiente frame de la ventana principal
  (los estados ya son reactivos en 2A/2B; el idioma se hace reactivo aquí: el render
  llama `t()` cada frame contra el catálogo activo).

### Secciones de la ventana (contenido en 2C-i)

- **Apariencia**: set de íconos (Flat/Fluent/Mono), tema (**placeholder** Claro/
  Oscuro/Sistema, sin efecto — nota "completo en 2C-ii"), botones solo-ícono.
- **Paneles**: mostrar fila "..", posición de la barra (Arriba/Al costado).
- **Atajos**: **solo-lectura** — lista el mapa de teclas actual (↑↓, Enter,
  Backspace, Alt+←/→, Tab, Esc, type-ahead). La edición llega en una fase posterior.
- **Idioma**: selector de los idiomas disponibles (ES, EN, + los de `lang/`), cambia
  en caliente.
- **Avanzado**: mínimo — ruta de la carpeta de config (portable), versión de la app.
  Crece después.

---

## 3. Modelo de interacción

- **Abrir Configuración**: botón ⚙ en la toolbar → abre la ventana del SO separada
  (si ya está abierta, la trae al frente). Se puede dejar abierta al lado de la
  ventana principal.
- **Navegar secciones**: clic en la lista izquierda.
- **Cambiar idioma**: sección Idioma → seleccionar → toda la UI cambia de idioma en
  caliente (siguiente frame).
- **Cambiar opciones**: efecto en caliente; persisten al cerrar (vía `save()`).
- **Cerrar**: la X de la ventana del SO o un botón "Cerrar".

---

## 4. Manejo de errores

Filosofía del proyecto (hostil, nunca cae):
- `lang/*.json` corrupto/ausente → se ignora ese archivo, ES/EN embebidos siempre
  presentes, log discreto.
- Clave de texto inexistente → se muestra la clave (visible, no rompe).
- Idioma persistido inexistente (borraron su `lang/`) → cae al default detectado.
- Si crear el viewport de Configuración fallara (caso raro de egui) → log, la app
  principal sigue, se reintenta al reabrir.
- JSON de `Settings` con `language` inválido → `#[serde(default)]` + default
  detectado (tolerancia de `config` ya existente).

---

## 5. Testing

- **`core::i18n`**: `t()` resuelve clave del activo; clave faltante → fallback →
  la clave; cargar un `Catalog` desde JSON; JSON inválido → vacío + sin panic;
  `set_language` cambia el activo; merge de un archivo suelto sobre el embebido
  (añade idioma / sobreescribe clave).
- **`pick_default_language`**: `"es-CL"→es`, `"es"→es`, `"en-US"→en`, `"fr-FR"→en`
  (fallback), `""→en`.
- **`config`**: round-trip de `Settings.language`; idioma inválido → default.
- **UI** (viewport de Configuración, render de secciones, multi-viewport): la lógica
  con estado (qué sección activa, el enum, `settings_open`) se extrae a algo
  testeable; el render del viewport se valida manualmente.

Meta: build limpio + tests pasando + clippy limpio antes de cada commit.

---

## 6. Estructura de archivos prevista (incremental sobre 2B)

```
crates/core/src/
├── lib.rs                 # + re-exports de i18n
├── i18n/
│   ├── mod.rs             # LangId, Catalog, I18n, t(), set_language, load
│   └── detect.rs          # pick_default_language (puro)
├── config/mod.rs          # + Settings.language
└── ...                    # resto sin cambios

crates/ui/src/
├── settings_window/
│   ├── mod.rs             # ventana viewport: SettingsSection, despacho, show()
│   ├── appearance.rs      # sección Apariencia
│   ├── panes.rs           # sección Paneles
│   ├── shortcuts.rs       # sección Atajos (solo-lectura)
│   ├── language.rs        # sección Idioma
│   └── advanced.rs        # sección Avanzado
├── app.rs                 # + i18n, settings_open, settings_section; abre el viewport
├── toolbar.rs             # ⚙ abre la ventana (quita el menú inline); textos vía t()
├── panes/*.rs             # textos vía t()
├── docking.rs             # títulos de tab vía t()
└── ...

assets/lang/               # (embebidos vía include_str! desde aquí o crates/core/i18n/)
├── es.json
└── en.json
```

> Nota: los JSON embebidos (es/en) pueden vivir en `crates/core/src/i18n/` (junto al
> módulo, embebidos con `include_str!`) o en `assets/lang/`. Decidir ruta exacta en
> el plan; lo importante es que ES/EN van embebidos y `lang/` al lado del `.exe` es
> para idiomas sueltos del usuario.

---

## 7. Dependencias

Sin dependencias nuevas previstas: `serde_json` (ya está) parsea los catálogos;
egui 0.34 ya provee multi-viewport (`show_viewport_immediate`/`deferred`). La
lectura del locale del SO se hace con `std` (variables de entorno como `LANG`/
`LC_ALL` en Unix; en Windows, `GetUserDefaultLocaleName` vía el crate `windows` que
ya está en `platform`, o una heurística desde env). **Decidir en el plan** si la
detección de locale en Windows usa el crate `windows` (en `platform`) o una
aproximación por env var; la función `pick_default_language` es pura igual y recibe
el string ya leído.

> A confirmar en el plan: API exacta de multi-viewport de egui 0.34
> (`show_viewport_immediate` y el manejo de estado a través del límite del
> viewport), y de dónde se lee el locale del SO.

---

## Fuera de alcance (recordatorio — NO en 2C-i)

Temas/color sets reales (2C-ii), packs íconos+color (2C-ii), recoloreado del mono
(2C-ii), edición de atajos (Atajos es solo-lectura), multi-ventana del explorador,
deuda de persistencia del dock (2A). Nunca: reproducción de media, edición de
archivos.
