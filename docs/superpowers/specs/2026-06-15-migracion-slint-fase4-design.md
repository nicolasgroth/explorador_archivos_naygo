# Migración a Slint — Fase 4: configuración + atajos + temas/i18n + persistencia — Diseño

> Cuarta fase de la migración egui→Slint. Las Fases 1–3 (panel navegable, multi-panel/docking/
> paneles especiales/tabs/drag, operaciones de archivo) están en `main` y verificadas. Esta
> fase agrega la **configuración completa**, el **editor de atajos**, **temas + i18n** (con
> arquitectura de **packs extensibles por el usuario** e **import/export `.zip`**), y la
> **persistencia del layout** (recordar los paneles abiertos al cerrar/abrir). Gobernada por
> el contrato de paridad (`docs/migracion-slint/CONTRATO-PARIDAD-FUNCIONAL.md`) y por el
> lineamiento del proyecto: "i18n y temas desde el día uno; ningún texto hardcoded".

## Decisiones de Nicolás (alcance)

- F4 **completa de una**: persistir layout + ventana de configuración + editor de atajos +
  temas/i18n (ES/EN).
- **Extensibilidad**: temas e idiomas deben poder crecer con **packs propios del usuario**
  (idiomas personalizados; temas con colores e íconos propios).
- **Import/Export**: por **archivo `.zip`** por pack (idioma, tema) y para la config del app.
- **i18n**: externalizar **TODOS** los textos de los `.slint` a claves; ES + EN incluidos.

## Contexto: el modelo ya existe en core (se reusa)

- `core::config`: `Settings` (~30 campos), `load_settings_flagged`/`save_settings`,
  `WorkspacePersist { version, layout, active, files: Vec<(PaneId, FilePanePersist)>,
  purposes: Vec<(PaneId, PanePurpose)> }`, `load_workspace_flagged`/`save_workspace`,
  `load_templates`/`save_templates`, `load_keymap`/`save_keymap`, `portable_dir()`. Carga
  tolerante: archivo corrupto → respaldo `.bad` + defaults (nunca crashea).
- `core::keymap`: `Action` (enum de todas las acciones), `Chord`/`KeyCode`, `KeyMap`
  (`action_for`, `defaults`), serializable.
- `core::i18n`: `I18n::load(dir, lang)` (ES/EN embebidos + `dir/lang/*.json` de usuario),
  `t(key)`, `set_language`, idiomas disponibles. `Catalog::from_json`. **Ya soporta packs de
  idioma del usuario** (soltar `lang/<code>.json`).
- `core::theme`: `Theme`, `ThemeId`, `ThemeColor`, `ThemeCatalog::load(dir)` (temas embebidos
  + temas de usuario en `dir/theme/<id>/`). **Ya soporta packs de tema del usuario**.
- La capa egui (`crates/ui`) ya orquesta todo esto: `rebuild_workspace`/`remap_layout`
  (reconstruir el workspace desde `WorkspacePersist`), la ventana de settings, el editor de
  atajos. Es la referencia a portar; la lógica pura se reusa de core.

**Conclusión:** la F4 reusa el modelo de core y reescribe la ORQUESTACIÓN + la UI en Slint.
La persistencia del layout es casi "cablear" (el core ya tiene el tipo y save/load).

## 1. Arquitectura

Módulo nuevo `crates/ui-slint/src/config_ctrl.rs` con la struct `ConfigCtrl`, dueña de:
`settings: Settings`, `i18n: I18n`, `themes: ThemeCatalog` + tema activo, `keymap: KeyMap`,
`config_dir: PathBuf`. Expone getters (→ VMs para Slint) y setters que **persisten al
cambiar** (`save_settings`/`save_keymap`) y aplican en caliente (idioma/tema). `WorkspaceCtrl`
lo posee (y ya posee `OpsCtrl`); el `keymap` de `WorkspaceCtrl` pasa a venir de `ConfigCtrl`.

Globals Slint nuevos: `Tr` (i18n: una propiedad string por clave) y `Theme` (colores). Los
`.slint` dejan de usar texto literal y hex; usan `Tr.<clave>` y `Theme.<color>`. Rust setea
esas propiedades al arrancar y al cambiar idioma/tema.

Regla de oro intacta: I/O fuera del hilo de UI (los `save_*` son escrituras chicas de JSON;
se hacen en el hilo de UI sin bloquear perceptible, igual que egui — son KB).

## 2. Persistencia del layout (recordar paneles abiertos)

- `WorkspaceCtrl::session_persist() -> WorkspacePersist`: arma el persist desde el workspace
  vivo — `layout` (el `SerializableDockLayout` actual), `active` (panel activo), `files`
  (por cada panel Files, su `FilePanePersist` = carpeta actual + estado relevante), `purposes`
  (tipo de cada panel, para reconstruir Tree/Inspector/etc.).
- `save_session()`: `config::save_workspace(config_dir, &persist)`. Se llama en el callback de
  CIERRE de la ventana (Slint `window().on_close_requested` o equivalente) y, defensivo, tras
  cada navegación con un debounce (no en cada tecla).
- `load_session()` al arrancar: `config::load_workspace_flagged()` → si hay persist válido,
  reconstruye el workspace (crear paneles con sus purposes y carpetas, rearmar el layout con
  remap de ids, fijar el activo), arranca el listado de cada Files y el DirTree de cada Tree.
  Si no hay (primer uso) o estaba corrupto (.bad), arranca con el default actual (un Files en
  HOME). Se porta la lógica de `rebuild_workspace`/`remap_layout` de egui a un helper
  reutilizable (idealmente en core, para una sola fuente de verdad).

## 3. Ventana de configuración

Componente Slint `config-window.slint` (overlay con velo + tarjeta grande con barra lateral de
categorías + panel de contenido). Categorías:
- **General**: posición de barra, solo-íconos, mostrar fila "..", caché de carpetas (nº).
- **Operaciones**: modo (paralelo/cola), confirmar papelera, mostrar resumen, tamaño no
  recursivo.
- **Pegado**: plantillas de nombre (texto/imagen), extensión de texto, formato de imagen,
  calidad JPG.
- **Apariencia**: tema (selector) + idioma (selector) — §5.
- **Atajos**: el editor — §4.
- **Importar/Exportar** — §6.
`ConfigCtrl` expone un `SettingsVm` (espejo de los campos editables). Cada control de la UI, al
cambiar, llama un setter (`set_ops_mode`, `set_confirm_trash`, …) que actualiza `settings`,
llama `save_settings` y aplica en caliente. Se abre desde un botón en la toolbar (engranaje, con
texto "Config" por la lección de glifos) o un atajo.

## 4. Editor de atajos

Dentro de la categoría "Atajos": una fila por `Action` con su etiqueta i18n y su chord actual
(texto legible, ej. "Ctrl+C"). Por fila:
- **Cambiar**: entra en modo captura; la próxima combinación de teclas (vía un `FocusScope` que
  lee `keys::chord_from`) se propone como nuevo chord.
- **Conflicto**: si el chord ya pertenece a otra acción, se avisa ("ya usado por X") y se
  ofrece reasignar (se lo quita a X) o cancelar.
- **Reset**: vuelve esa acción a su chord por defecto (`KeyMap::defaults`).
Persiste con `save_keymap`. El `KeyMap` activo (de `ConfigCtrl`) es el que usa `on_key`. VM:
`ShortcutRowVm { action_key, label, chord_text, conflict }`.

## 5. Temas + i18n (con packs de usuario)

### i18n
`ConfigCtrl` posee `I18n::load(config_dir, lang)`. Se define un global Slint `Tr` con una
propiedad `string` por cada clave de texto usada en la UI. Rust, al arrancar y al cambiar de
idioma, setea cada propiedad con `i18n.t(clave)`. Se externalizan **todos** los textos de los
`.slint` a `Tr.<clave>` (reusando las claves que ya existen en `core/i18n/es.json`/`en.json`;
las que falten se agregan a ambos catálogos embebidos). Cambiar idioma: `i18n.set_language` +
re-set de todas las propiedades de `Tr` + `save_settings`. Idiomas de usuario: `lang/<code>.json`
en `config_dir` (ya cargados por `I18n::load`); aparecen en el selector. Generación de `Tr`:
la lista de claves se mantiene en un solo lugar (un módulo Rust `i18n_keys` con la lista, y el
`Tr.slint` con las propiedades correspondientes) para no desincronizar.

### Temas
`ConfigCtrl` posee `ThemeCatalog::load(config_dir)` + el tema activo (`Theme`). Global Slint
`Theme` con propiedades de color: `bg`, `panel`, `panel-active`, `panel-border`, `row-sel`,
`row-hover`, `accent`, `text`, `text-dim`, `text-strong`, `danger`, `warn`, `dir-fg`, … (el
conjunto que hoy está hardcodeado en los `.slint`). Rust setea esas propiedades desde los
`ThemeColor` del tema activo. Los `.slint` reemplazan cada hex por `Theme.<color>`. Cambiar
tema: re-set de las propiedades + `save_settings`. Temas de usuario: `theme/<id>/colors.json`
(+ íconos opcionales, a futuro) en `config_dir` (ya cargados por `ThemeCatalog::load`); aparecen
en el selector. Mapa `ThemeColor → propiedad de Theme` centralizado en un helper Rust.

## 6. Import/Export (.zip)

Dep nueva: `zip` (crate). Selector de archivo nativo: dep `rfd` (diálogos de archivo del SO),
o el del Shell vía `platform` — se decide en el plan (probablemente `rfd`, multiplataforma y
liviano).
- **Exportar**: empaqueta en un `.zip` que el usuario guarda donde elija:
  - idioma → `lang/<code>.json`;
  - tema → la carpeta `theme/<id>/` completa;
  - config del app → `settings.json` + `keymap.json`.
- **Importar**: el usuario elige un `.zip`; se **valida** el contenido (estructura esperada:
  un `<code>.json` de idioma, o `colors.json` de tema, o `settings.json`/`keymap.json`); se
  descomprime a la carpeta correspondiente de `config_dir`; se recarga el catálogo (i18n/theme)
  o se aplican los settings. Errores (zip inválido/estructura inesperada) se reportan discretos
  sin crashear. Botones en la categoría "Importar/Exportar" de la ventana de config.

## 7. Testing

- **core**: ya hay tests de config/keymap/i18n/theme. Agregar: round-trip de `WorkspacePersist`
  (serializar el workspace vivo y reconstruirlo da el mismo árbol/carpetas/activo); validación
  de un pack importado (estructura correcta/incorrecta).
- **config_ctrl (ui-slint)**: cada setter persiste (escribe settings.json) y aplica; cambiar
  idioma re-resuelve `t`; cambiar tema cambia los colores expuestos; conflicto de atajos se
  detecta; `session_persist`/`load_session` reconstruye el workspace (paneles, carpetas, activo).
- **Verificación viva (Claude, esta máquina, Win32 + PrintWindow / input real)**: abrir config,
  cambiar una opción y confirmar que persiste al reabrir; reasignar un atajo y usarlo; cambiar
  idioma y ver la UI traducida; cambiar tema y ver los colores; cerrar con varios paneles y
  reabrir restaurándolos; importar y exportar un pack de idioma y uno de tema.
- **Rendimiento (Nicolás, VM)**: CPU en reposo y al abrir/usar la config sin subir el consumo.

## 8. Puertas

`cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` +
`cargo fmt --all -- --check` antes de cada commit. Commits en español, rutas EXPLÍCITAS (sin
`CLAUDE.md` ni `graphify-out/`). `graphify update .` tras cambios de código. Verificación
funcional por Claude en esta máquina antes de pedir nada; Nicolás solo mide rendimiento en la VM.

## 9. Riesgos / decisiones

- **Externalizar TODOS los textos** es la parte más extensa (toca cada `.slint`). Se hace por
  archivo, con gate verde en cada paso. La lista de claves se centraliza para no desincronizar
  `Tr.slint` con el módulo Rust de claves.
- **Aplicar temas en caliente**: reemplazar cada hex por `Theme.<color>` en los `.slint` es
  mecánico pero amplio; se hace por archivo. Un color que falte en el tema → fallback al
  embebido (default), nunca un color inválido.
- **Persistencia del layout**: reusar `rebuild_workspace`/`remap_layout` de egui; portarla a
  core evita duplicar. Si reconstruir falla (carpeta de un panel ya no existe), ese panel cae a
  HOME en vez de romper la sesión.
- **zip + rfd**: deps nuevas, ambas MIT/Apache (cumplen "cero regalías"). Se confirman licencias
  en el plan.
- **i18n y temas crecen**: la arquitectura de packs (carpetas `lang/` y `theme/`) y el
  import/export `.zip` quedan listos desde F4; agregar un idioma/tema = importar un zip o soltar
  el archivo/carpeta.

## Fuera de alcance (otras fases)
Integraciones SO: OLE drag&drop, menú nativo ya hecho en F3, papelera ya hecha, tray/autostart,
splash, watcher (F5). Pulido final + retiro de egui + instalador (F6). Batch-rename avanzado,
paleta de comandos, miniaturas (pulido).
